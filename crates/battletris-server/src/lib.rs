//! Self-hosted BattleTris lobby and ranked-result authority.
//!
//! Gameplay transport stays direct while ranked trust moves to a server-owned
//! session and result-verification boundary. The server issues lobby sessions
//! and seeds, admits only matching protocol-major clients, and records ranked
//! results only after both players submit matching claims.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use battletris_db::{CommunityLabel, GameResult, OpponentKind, PlayerId, PlayerStore};
use battletris_protocol::{
    HostedGameStart, HostedPlayer, HostedSessionId, HostedSessionStatus, HostedSessionStatusKind,
    LobbyEntry, LobbyRegister, RankedPlayerRecord, RankedRecords, RankedResultClaim,
    PROTOCOL_MAJOR, PROTOCOL_MINOR,
};

/// Result type for server authority operations.
pub type Result<T> = std::result::Result<T, ServerError>;

/// Server-side lobby and ranked-result failures.
#[derive(Debug)]
pub enum ServerError {
    /// Client protocol major version does not match this server.
    ProtocolMajorMismatch {
        /// Client major version.
        client_major: u16,
        /// Server major version.
        server_major: u16,
    },
    /// A required string field was empty.
    InvalidLobbyEntry(&'static str),
    /// The referenced session does not exist.
    UnknownSession(HostedSessionId),
    /// The referenced session is no longer accepting the requested operation.
    SessionUnavailable(HostedSessionId),
    /// The player is not one of the two session participants.
    WrongParticipant(String),
    /// The result does not identify the two session participants as winner/loser.
    InvalidResultParticipants,
    /// The second claim for a session did not match the first claim.
    ResultClaimMismatch,
    /// The persistence layer rejected a ranked write.
    Persistence(battletris_db::Error),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProtocolMajorMismatch {
                client_major,
                server_major,
            } => write!(
                f,
                "protocol major mismatch: client {client_major}, server {server_major}"
            ),
            Self::InvalidLobbyEntry(field) => write!(f, "invalid lobby entry field: {field}"),
            Self::UnknownSession(session_id) => write!(f, "unknown session {}", session_id.0),
            Self::SessionUnavailable(session_id) => {
                write!(f, "session {} is unavailable", session_id.0)
            }
            Self::WrongParticipant(player_id) => write!(f, "wrong participant {player_id}"),
            Self::InvalidResultParticipants => write!(f, "invalid result participants"),
            Self::ResultClaimMismatch => write!(f, "ranked result claims do not match"),
            Self::Persistence(err) => write!(f, "persistence error: {err}"),
        }
    }
}

impl std::error::Error for ServerError {}

impl From<battletris_db::Error> for ServerError {
    fn from(value: battletris_db::Error) -> Self {
        Self::Persistence(value)
    }
}

/// Result of submitting a hosted ranked result claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationOutcome {
    /// The first matching participant claim was accepted; the server is waiting
    /// for the other participant before mutating records.
    AwaitingPeer,
    /// Both participants submitted matching claims and the server recorded the result.
    Recorded,
}

/// In-memory self-hosted lobby authority for one community label.
#[derive(Debug)]
pub struct HostedLobbyServer {
    community_label: CommunityLabel,
    next_session_number: u64,
    next_seed: u64,
    sessions: BTreeMap<HostedSessionId, Session>,
    stale_sessions: BTreeSet<HostedSessionId>,
}

impl HostedLobbyServer {
    /// Creates an empty hosted lobby authority.
    #[must_use]
    pub const fn new(community_label: CommunityLabel, first_seed: u64) -> Self {
        Self {
            community_label,
            next_session_number: 1,
            next_seed: first_seed,
            sessions: BTreeMap::new(),
            stale_sessions: BTreeSet::new(),
        }
    }

    /// Registers a host as discoverable and returns its lobby entry.
    pub fn register_host(
        &mut self,
        request: LobbyRegister,
        protocol_major: u16,
        protocol_minor: u16,
    ) -> Result<LobbyEntry> {
        ensure_protocol_major(protocol_major)?;
        validate_host_registration(&request)?;

        let session_id = self.next_session_id();
        let entry = LobbyEntry {
            session_id: session_id.clone(),
            host: request.player.clone(),
            direct_addr: request.direct_addr.clone(),
            ranked: request.ranked,
            protocol_major,
            protocol_minor,
        };
        self.sessions.insert(
            session_id,
            Session {
                host: request.player,
                direct_addr: request.direct_addr,
                ranked: request.ranked,
                protocol_major,
                protocol_minor,
                state: SessionState::Available,
                pending_claim: None,
            },
        );
        Ok(entry)
    }

    /// Lists currently discoverable sessions.
    #[must_use]
    pub fn lobby_entries(&self, ranked_only: bool) -> Vec<LobbyEntry> {
        self.sessions
            .iter()
            .filter_map(|(session_id, session)| {
                if session.state != SessionState::Available || (ranked_only && !session.ranked) {
                    return None;
                }
                Some(LobbyEntry {
                    session_id: session_id.clone(),
                    host: session.host.clone(),
                    direct_addr: session.direct_addr.clone(),
                    ranked: session.ranked,
                    protocol_major: session.protocol_major,
                    protocol_minor: session.protocol_minor,
                })
            })
            .collect()
    }

    /// Returns server-owned ranked records for this community.
    pub fn ranked_records(&self, store: &PlayerStore, limit: u16) -> Result<RankedRecords> {
        let limit = usize::from(limit.clamp(1, 200));
        let records = store
            .roster_by_rank(&self.community_label)?
            .into_iter()
            .take(limit)
            .map(|profile| RankedPlayerRecord {
                player_id: profile.player_id.as_str().to_string(),
                display_name: profile.display_name,
                rank: profile.rank,
                wins: profile.wins,
                losses: profile.losses,
                high_score: profile.high_score,
                high_lines: profile.high_lines,
                high_funds: profile.high_funds,
            })
            .collect();
        Ok(RankedRecords {
            community_label: self.community_label.as_str().to_string(),
            records,
        })
    }

    /// Starts a hosted game for a lobby session and creates participant records.
    pub fn start_game(
        &mut self,
        session_id: &HostedSessionId,
        joiner: HostedPlayer,
        joiner_protocol_major: u16,
        store: &PlayerStore,
    ) -> Result<HostedGameStart> {
        ensure_protocol_major(joiner_protocol_major)?;
        validate_hosted_player(&joiner)?;

        let seed = self.next_seed;
        self.next_seed = self.next_seed.wrapping_add(1);
        if !self.sessions.contains_key(session_id) {
            return Err(self.missing_session_error(session_id));
        }
        let session = self
            .sessions
            .get_mut(session_id)
            .expect("session existence checked");
        if session.state != SessionState::Available {
            return Err(ServerError::SessionUnavailable(session_id.clone()));
        }
        if session.host.player_id == joiner.player_id {
            return Err(ServerError::WrongParticipant(joiner.player_id));
        }

        create_player(store, &session.host, &self.community_label)?;
        create_player(store, &joiner, &self.community_label)?;

        let start = HostedGameStart {
            session_id: session_id.clone(),
            player_one: session.host.clone(),
            player_two: joiner,
            seed,
            ranked: session.ranked,
            community_label: self.community_label.as_str().to_string(),
        };
        session.state = SessionState::InProgress {
            joiner: start.player_two.clone(),
            start: Box::new(start.clone()),
        };

        Ok(start)
    }

    /// Returns server-owned status for a hosted lobby session participant.
    pub fn session_status(
        &self,
        session_id: &HostedSessionId,
        requester_player_id: &str,
    ) -> Result<HostedSessionStatus> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| self.missing_session_error(session_id))?;
        match &session.state {
            SessionState::Available => {
                if requester_player_id != session.host.player_id {
                    return Err(ServerError::WrongParticipant(
                        requester_player_id.to_string(),
                    ));
                }
                Ok(HostedSessionStatus {
                    session_id: session_id.clone(),
                    status: HostedSessionStatusKind::WaitingForPeer,
                })
            }
            SessionState::InProgress { joiner, start } => {
                if requester_player_id != session.host.player_id
                    && requester_player_id != joiner.player_id
                {
                    return Err(ServerError::WrongParticipant(
                        requester_player_id.to_string(),
                    ));
                }
                Ok(HostedSessionStatus {
                    session_id: session_id.clone(),
                    status: HostedSessionStatusKind::Started((**start).clone()),
                })
            }
            SessionState::Completed => Ok(HostedSessionStatus {
                session_id: session_id.clone(),
                status: HostedSessionStatusKind::Unavailable {
                    reason: "session completed".to_string(),
                },
            }),
        }
    }

    /// Marks a session stale after a disconnect, timeout, or operator action.
    pub fn expire_session(&mut self, session_id: &HostedSessionId) -> bool {
        if self.sessions.remove(session_id).is_some() {
            self.stale_sessions.insert(session_id.clone());
            return true;
        }
        false
    }

    /// Submits one participant's ranked result claim.
    pub fn submit_ranked_result(
        &mut self,
        claim: RankedResultClaim,
        store: &mut PlayerStore,
    ) -> Result<VerificationOutcome> {
        if !self.sessions.contains_key(&claim.session_id) {
            return Err(self.missing_session_error(&claim.session_id));
        }
        let session = self
            .sessions
            .get_mut(&claim.session_id)
            .expect("session existence checked");
        let SessionState::InProgress { joiner, .. } = &session.state else {
            return Err(ServerError::SessionUnavailable(claim.session_id.clone()));
        };
        if !session.ranked {
            return Err(ServerError::SessionUnavailable(claim.session_id.clone()));
        }
        validate_claim(&claim, &session.host, joiner)?;

        match &session.pending_claim {
            None => {
                session.pending_claim = Some(claim);
                Ok(VerificationOutcome::AwaitingPeer)
            }
            Some(first_claim) => {
                if first_claim.reporter_player_id == claim.reporter_player_id {
                    return Err(ServerError::WrongParticipant(claim.reporter_player_id));
                }
                if !claims_match(first_claim, &claim) {
                    return Err(ServerError::ResultClaimMismatch);
                }
                let result = game_result_from_claim(&claim, &self.community_label)?;
                let recorded = store.record_game_result(&result)?;
                debug_assert!(recorded, "validated hosted ranked result should record");
                session.state = SessionState::Completed;
                session.pending_claim = None;
                Ok(VerificationOutcome::Recorded)
            }
        }
    }

    fn next_session_id(&mut self) -> HostedSessionId {
        let value = self.next_session_number;
        self.next_session_number += 1;
        HostedSessionId(format!("session-{value}"))
    }

    fn missing_session_error(&self, session_id: &HostedSessionId) -> ServerError {
        if self.stale_sessions.contains(session_id) {
            return ServerError::SessionUnavailable(session_id.clone());
        }
        ServerError::UnknownSession(session_id.clone())
    }
}

#[derive(Debug)]
struct Session {
    host: HostedPlayer,
    direct_addr: String,
    ranked: bool,
    protocol_major: u16,
    protocol_minor: u16,
    state: SessionState,
    pending_claim: Option<RankedResultClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SessionState {
    Available,
    InProgress {
        joiner: HostedPlayer,
        start: Box<HostedGameStart>,
    },
    Completed,
}

fn ensure_protocol_major(client_major: u16) -> Result<()> {
    if client_major != PROTOCOL_MAJOR {
        return Err(ServerError::ProtocolMajorMismatch {
            client_major,
            server_major: PROTOCOL_MAJOR,
        });
    }
    Ok(())
}

fn validate_host_registration(request: &LobbyRegister) -> Result<()> {
    validate_hosted_player(&request.player)?;
    if request.direct_addr.trim().is_empty() {
        return Err(ServerError::InvalidLobbyEntry("direct_addr"));
    }
    Ok(())
}

fn validate_hosted_player(player: &HostedPlayer) -> Result<()> {
    if player.player_id.trim().is_empty() {
        return Err(ServerError::InvalidLobbyEntry("player_id"));
    }
    if player.display_name.trim().is_empty() {
        return Err(ServerError::InvalidLobbyEntry("display_name"));
    }
    Ok(())
}

fn create_player(
    store: &PlayerStore,
    player: &HostedPlayer,
    community_label: &CommunityLabel,
) -> Result<()> {
    store.create_or_update_player(
        &PlayerId::new(player.player_id.clone())?,
        &player.display_name,
        community_label,
    )?;
    Ok(())
}

fn validate_claim(
    claim: &RankedResultClaim,
    host: &HostedPlayer,
    joiner: &HostedPlayer,
) -> Result<()> {
    let reporter = claim.reporter_player_id.as_str();
    if reporter != host.player_id && reporter != joiner.player_id {
        return Err(ServerError::WrongParticipant(
            claim.reporter_player_id.clone(),
        ));
    }

    let winner = claim.winner_player_id.as_str();
    let loser = claim.loser_player_id.as_str();
    let participants = [host.player_id.as_str(), joiner.player_id.as_str()];
    if winner == loser || !participants.contains(&winner) || !participants.contains(&loser) {
        return Err(ServerError::InvalidResultParticipants);
    }
    Ok(())
}

fn claims_match(left: &RankedResultClaim, right: &RankedResultClaim) -> bool {
    left.session_id == right.session_id
        && left.winner_player_id == right.winner_player_id
        && left.loser_player_id == right.loser_player_id
        && left.winner_score == right.winner_score
        && left.winner_lines == right.winner_lines
        && left.winner_funds == right.winner_funds
        && left.loser_score == right.loser_score
        && left.loser_lines == right.loser_lines
        && left.loser_funds == right.loser_funds
        && left.duration_secs == right.duration_secs
        && left.duration_ticks == right.duration_ticks
        && left.event_count == right.event_count
        && left.final_checksum == right.final_checksum
}

fn game_result_from_claim(
    claim: &RankedResultClaim,
    community_label: &CommunityLabel,
) -> Result<GameResult> {
    Ok(GameResult {
        community_label: community_label.clone(),
        winner_id: PlayerId::new(claim.winner_player_id.clone())?,
        loser_id: PlayerId::new(claim.loser_player_id.clone())?,
        winner_score: claim.winner_score,
        winner_lines: claim.winner_lines,
        winner_funds: claim.winner_funds,
        loser_score: claim.loser_score,
        loser_lines: claim.loser_lines,
        loser_funds: claim.loser_funds,
        duration_secs: claim.duration_secs,
        ranked: true,
        opponent_kind: OpponentKind::Human,
    })
}

/// Current server protocol version for lobby admission.
#[must_use]
pub const fn server_protocol_version() -> (u16, u16) {
    (PROTOCOL_MAJOR, PROTOCOL_MINOR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use battletris_db::STARTING_RANK;

    fn player(id: &str, name: &str) -> HostedPlayer {
        HostedPlayer {
            player_id: id.to_string(),
            display_name: name.to_string(),
        }
    }

    fn register(player: HostedPlayer, ranked: bool) -> LobbyRegister {
        LobbyRegister {
            player,
            direct_addr: "127.0.0.1:4404".to_string(),
            ranked,
        }
    }

    fn claim(session_id: &HostedSessionId, reporter: &str) -> RankedResultClaim {
        RankedResultClaim {
            session_id: session_id.clone(),
            reporter_player_id: reporter.to_string(),
            winner_player_id: "ada".to_string(),
            loser_player_id: "ben".to_string(),
            winner_score: 12_000,
            winner_lines: 40,
            winner_funds: -900,
            loser_score: 8_000,
            loser_lines: 24,
            loser_funds: -500,
            duration_secs: 180,
            duration_ticks: 18_000,
            event_count: 88,
            final_checksum: 0x88,
        }
    }

    #[test]
    fn lobby_registers_and_lists_discoverable_sessions() {
        let community = CommunityLabel::new("garage").unwrap();
        let mut server = HostedLobbyServer::new(community, 100);

        let entry = server
            .register_host(register(player("ada", "Ada"), true), PROTOCOL_MAJOR, 0)
            .unwrap();

        assert_eq!(entry.session_id.0, "session-1");
        assert_eq!(server.lobby_entries(false), vec![entry.clone()]);
        assert_eq!(server.lobby_entries(true), vec![entry]);
    }

    #[test]
    fn lobby_rejects_protocol_version_skew_and_invalid_presence() {
        let community = CommunityLabel::new("garage").unwrap();
        let mut server = HostedLobbyServer::new(community, 100);

        assert!(matches!(
            server.register_host(register(player("ada", "Ada"), true), PROTOCOL_MAJOR + 1, 0),
            Err(ServerError::ProtocolMajorMismatch { .. })
        ));
        assert!(matches!(
            server.register_host(register(player(" ", "Ada"), true), PROTOCOL_MAJOR, 0),
            Err(ServerError::InvalidLobbyEntry("player_id"))
        ));
    }

    #[test]
    fn hosted_game_start_removes_session_from_lobby_and_creates_players() {
        let store = PlayerStore::open_in_memory().unwrap();
        let community = CommunityLabel::new("garage").unwrap();
        let mut server = HostedLobbyServer::new(community, 100);
        let entry = server
            .register_host(register(player("ada", "Ada"), true), PROTOCOL_MAJOR, 0)
            .unwrap();

        let start = server
            .start_game(
                &entry.session_id,
                player("ben", "Ben"),
                PROTOCOL_MAJOR,
                &store,
            )
            .unwrap();

        assert_eq!(start.seed, 100);
        assert_eq!(start.community_label, "garage");
        assert!(server.lobby_entries(false).is_empty());
        assert!(store
            .player(
                &PlayerId::new("ada").unwrap(),
                &CommunityLabel::new("garage").unwrap()
            )
            .unwrap()
            .is_some());
        assert!(store
            .player(
                &PlayerId::new("ben").unwrap(),
                &CommunityLabel::new("garage").unwrap()
            )
            .unwrap()
            .is_some());
    }

    #[test]
    fn hosted_session_status_reports_waiting_and_started_metadata() {
        let store = PlayerStore::open_in_memory().unwrap();
        let community = CommunityLabel::new("garage").unwrap();
        let mut server = HostedLobbyServer::new(community, 100);
        let entry = server
            .register_host(register(player("ada", "Ada"), true), PROTOCOL_MAJOR, 0)
            .unwrap();

        assert_eq!(
            server
                .session_status(&entry.session_id, "ada")
                .unwrap()
                .status,
            HostedSessionStatusKind::WaitingForPeer
        );

        let start = server
            .start_game(
                &entry.session_id,
                player("ben", "Ben"),
                PROTOCOL_MAJOR,
                &store,
            )
            .unwrap();

        assert_eq!(
            server
                .session_status(&entry.session_id, "ada")
                .unwrap()
                .status,
            HostedSessionStatusKind::Started(start.clone())
        );
        assert_eq!(
            server
                .session_status(&entry.session_id, "ben")
                .unwrap()
                .status,
            HostedSessionStatusKind::Started(start)
        );
        assert!(matches!(
            server.session_status(&entry.session_id, "mallory"),
            Err(ServerError::WrongParticipant(_))
        ));
    }

    #[test]
    fn matching_ranked_result_claims_record_once() {
        let mut store = PlayerStore::open_in_memory().unwrap();
        let community = CommunityLabel::new("garage").unwrap();
        let mut server = HostedLobbyServer::new(community.clone(), 100);
        let entry = server
            .register_host(register(player("ada", "Ada"), true), PROTOCOL_MAJOR, 0)
            .unwrap();
        server
            .start_game(
                &entry.session_id,
                player("ben", "Ben"),
                PROTOCOL_MAJOR,
                &store,
            )
            .unwrap();

        assert_eq!(
            server
                .submit_ranked_result(claim(&entry.session_id, "ada"), &mut store)
                .unwrap(),
            VerificationOutcome::AwaitingPeer
        );
        assert_eq!(
            server
                .submit_ranked_result(claim(&entry.session_id, "ben"), &mut store)
                .unwrap(),
            VerificationOutcome::Recorded
        );

        let ada = store
            .player(&PlayerId::new("ada").unwrap(), &community)
            .unwrap()
            .unwrap();
        let ben = store
            .player(&PlayerId::new("ben").unwrap(), &community)
            .unwrap()
            .unwrap();
        assert_eq!(ada.rank, STARTING_RANK + 5);
        assert_eq!(ada.wins, 1);
        assert_eq!(ben.rank, STARTING_RANK - 5);
        assert_eq!(ben.losses, 1);
        assert!(matches!(
            server.submit_ranked_result(claim(&entry.session_id, "ada"), &mut store),
            Err(ServerError::SessionUnavailable(_))
        ));
        assert!(store
            .head_to_head(&ada.player_id, &ben.player_id, &community)
            .unwrap()
            .is_some());

        let records = server.ranked_records(&store, 10).unwrap();
        assert_eq!(records.community_label, "garage");
        assert_eq!(records.records.len(), 2);
        assert_eq!(records.records[0].player_id, "ada");
        assert_eq!(records.records[0].wins, 1);
    }

    #[test]
    fn mismatched_or_wrong_participant_claims_are_rejected() {
        let mut store = PlayerStore::open_in_memory().unwrap();
        let community = CommunityLabel::new("garage").unwrap();
        let mut server = HostedLobbyServer::new(community, 100);
        let entry = server
            .register_host(register(player("ada", "Ada"), true), PROTOCOL_MAJOR, 0)
            .unwrap();
        server
            .start_game(
                &entry.session_id,
                player("ben", "Ben"),
                PROTOCOL_MAJOR,
                &store,
            )
            .unwrap();

        assert!(matches!(
            server.submit_ranked_result(claim(&entry.session_id, "mallory"), &mut store),
            Err(ServerError::WrongParticipant(_))
        ));
        server
            .submit_ranked_result(claim(&entry.session_id, "ada"), &mut store)
            .unwrap();
        let mut tampered = claim(&entry.session_id, "ben");
        tampered.winner_score += 1;
        assert!(matches!(
            server.submit_ranked_result(tampered, &mut store),
            Err(ServerError::ResultClaimMismatch)
        ));
    }

    #[test]
    fn stale_sessions_reject_result_claims() {
        let mut store = PlayerStore::open_in_memory().unwrap();
        let community = CommunityLabel::new("garage").unwrap();
        let mut server = HostedLobbyServer::new(community, 100);
        let entry = server
            .register_host(register(player("ada", "Ada"), true), PROTOCOL_MAJOR, 0)
            .unwrap();

        assert!(server.expire_session(&entry.session_id));
        assert!(matches!(
            server.submit_ranked_result(claim(&entry.session_id, "ada"), &mut store),
            Err(ServerError::SessionUnavailable(_))
        ));
    }
}
