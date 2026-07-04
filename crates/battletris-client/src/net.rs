//! Client-side networking boundary.
//!
//! Bevy systems should exchange [`NetworkCommand`] and [`NetworkEvent`] values
//! with this module instead of owning sockets, Tokio handles, or blocking work.

#[cfg(feature = "lan-discovery")]
use std::collections::HashMap;
use std::{collections::BTreeMap, net::SocketAddr, thread, time::Duration};

use battletris_core::{
    game::{BattleEvent, Command, GamePhase, LoggedEvent, PlayerId, ShoppingError, TwoPlayerGame},
    weapons::WeaponToken,
};
use battletris_protocol::{
    ArsenalEntry, ArsenalSnapshot, BazaarBuy, BazaarDone, BazaarRemove,
    BoardSnapshot as WireBoardSnapshot, Challenge, DirectConnection, Disconnect, GameChecksum,
    GameOver, Heartbeat, HostedGameStart, HostedJoinRequest, HostedPlayer, HostedSessionCancel,
    HostedSessionId, HostedSessionStatus, HostedSessionStatusRequest, InputCommand,
    JoinedDirectGame, LanAdvertisement, LobbyEntry, LobbyList, LobbyListRequest, LobbyRegister,
    PlayerIdentity, PlayerInput, PlayerSlot, ProtocolError, RankedRecords, RankedRecordsRequest,
    RankedResultAccepted, RankedResultClaim, RankedResultPending, RankedResultRejected,
    ScoreSnapshot, StartGame, TickWatermark, WireMessage,
};
#[cfg(feature = "lan-discovery")]
use battletris_protocol::{LAN_DISCOVERY_SERVICE, PROTOCOL_MAJOR};
use tokio::{
    net::{TcpListener, TcpStream},
    runtime::Runtime,
    sync::{mpsc, oneshot},
    time::timeout,
};

/// Default bounded channel depth for network commands and events.
pub const DEFAULT_CHANNEL_CAPACITY: usize = 128;
/// Deterministic network tick duration. This matches the existing 10 ms fixed step.
pub const NETWORK_TICK_MS: u64 = 10;
/// Initial LAN-safe delay for local inputs in deterministic lockstep ticks.
pub const DEFAULT_INPUT_DELAY_TICKS: u64 = 6;
const CHECKSUM_HISTORY_TICKS: usize = 2_048;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(8);
const CHALLENGE_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
const PEER_IDLE_TIMEOUT: Duration = Duration::from_secs(20);
#[cfg(feature = "lan-discovery")]
const LAN_BROWSE_TIMEOUT: Duration = Duration::from_secs(3);

/// How the current network game was established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkMode {
    /// Manual direct TCP host/join game.
    Direct,
    /// Self-hosted lobby issued metadata, while gameplay remains direct TCP.
    Hosted,
}

/// Final hosted ranked result status known by the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalResultStatus {
    /// No final result has been submitted or received.
    None,
    /// Game is unranked and no hosted claim should be submitted.
    Unranked,
    /// First claim was accepted and the server is waiting for the peer.
    Pending(RankedResultPending),
    /// Server recorded the matching dual claim.
    Recorded,
    /// Server rejected the claim or the game became ineligible.
    Rejected(String),
}

/// Server-owned hosted session metadata attached to a network session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedSessionMetadata {
    /// Server-issued hosted session id.
    pub session_id: HostedSessionId,
    /// Hosted player identity for player one.
    pub player_one: HostedPlayer,
    /// Hosted player identity for player two.
    pub player_two: HostedPlayer,
}

/// Deterministic metadata for an established network game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkSession {
    /// Session establishment mode.
    pub mode: NetworkMode,
    /// Local player's protocol slot.
    pub local_slot: PlayerSlot,
    /// Friendly peer identity for direct games.
    pub peer_identity: PlayerIdentity,
    /// Shared protocol seed; player two uses `base_seed.wrapping_add(1)`.
    pub base_seed: u64,
    /// Whether hosted ranked claims may be submitted.
    pub ranked: bool,
    /// Optional server-owned hosted metadata.
    pub hosted: Option<HostedSessionMetadata>,
    /// Server/community ranking label.
    pub community_label: Option<String>,
    /// Local deterministic network tick.
    pub current_tick: u64,
    /// Latest peer tick watermark.
    pub peer_watermark: Option<u64>,
    /// Final ranked-result status.
    pub final_result_status: FinalResultStatus,
}

/// Actionable context emitted when a peer checksum disagrees with local state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesyncReport {
    /// Tick covered by the remote checksum.
    pub tick: u64,
    /// Local player's slot.
    pub local_slot: PlayerSlot,
    /// Latest local input known to the lockstep adapter.
    pub latest_local_input: Option<PlayerInput>,
    /// Latest remote input known to the lockstep adapter.
    pub latest_remote_input: Option<PlayerInput>,
    /// Highest local tick through which input has been sent.
    pub local_watermark: u64,
    /// Highest peer tick through which input has been reported.
    pub peer_watermark: Option<u64>,
    /// Local deterministic checksum at the comparison point.
    pub local_checksum: u64,
    /// Peer-reported checksum.
    pub remote_checksum: u64,
    /// Local deterministic event count.
    pub local_event_count: u64,
    /// Peer-reported event count.
    pub remote_event_count: u64,
    /// Local score/funds/line snapshots for both players.
    pub score_snapshots: [ScoreSnapshot; 2],
    /// Local board snapshots for both players.
    pub board_snapshots: [WireBoardSnapshot; 2],
    /// Local arsenal snapshots for both players.
    pub arsenal_snapshots: [ArsenalSnapshot; 2],
}

/// LAN discovery availability advertised by a direct host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanAvailability {
    /// Host is accepting a direct peer challenge.
    Available,
    /// Host exists but is not currently joinable.
    Busy,
    /// Advertisement did not include a recognized state.
    Unknown,
}

/// One DNS-SD LAN discovery entry suitable for join UI rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanDiscoveryEntry {
    /// DNS-SD service instance name.
    pub instance_name: String,
    /// Hostname reported by DNS-SD.
    pub hostname: String,
    /// Best socket address parsed from the resolved service.
    pub addr: Option<SocketAddr>,
    /// Direct TCP port reported by the advertisement.
    pub port: u16,
    /// Advertised player display name.
    pub display_name: String,
    /// Advertised protocol major version.
    pub protocol_major: u16,
    /// Advertised protocol minor version.
    pub protocol_minor: u16,
    /// Whether the entry matches this client's protocol major version.
    pub compatible: bool,
    /// Advertised availability state.
    pub availability: LanAvailability,
}

impl NetworkSession {
    /// Builds direct-game session metadata from a [`StartGame`] message.
    #[must_use]
    pub fn direct(local_slot: PlayerSlot, peer_identity: PlayerIdentity, start: StartGame) -> Self {
        Self {
            mode: NetworkMode::Direct,
            local_slot,
            peer_identity,
            base_seed: start.seed,
            ranked: start.ranked,
            hosted: None,
            community_label: None,
            current_tick: 0,
            peer_watermark: None,
            final_result_status: if start.ranked {
                FinalResultStatus::None
            } else {
                FinalResultStatus::Unranked
            },
        }
    }

    /// Builds hosted-game session metadata from a server-issued start message.
    #[must_use]
    pub fn hosted(
        local_slot: PlayerSlot,
        peer_identity: PlayerIdentity,
        start: HostedGameStart,
    ) -> Self {
        Self {
            mode: NetworkMode::Hosted,
            local_slot,
            peer_identity,
            base_seed: start.seed,
            ranked: start.ranked,
            hosted: Some(HostedSessionMetadata {
                session_id: start.session_id,
                player_one: start.player_one,
                player_two: start.player_two,
            }),
            community_label: Some(start.community_label),
            current_tick: 0,
            peer_watermark: None,
            final_result_status: if start.ranked {
                FinalResultStatus::None
            } else {
                FinalResultStatus::Unranked
            },
        }
    }
}

/// Builds a deterministic hosted ranked result claim from final client game state.
pub fn build_ranked_result_claim(
    session: &NetworkSession,
    game: &TwoPlayerGame,
    duration_ticks: u64,
) -> Result<RankedResultClaim, String> {
    if session.mode != NetworkMode::Hosted || !session.ranked {
        return Err("network session is not hosted ranked".to_string());
    }
    if game.phase() != GamePhase::GameOver {
        return Err("game is not over".to_string());
    }
    let hosted = session
        .hosted
        .as_ref()
        .ok_or_else(|| "hosted session metadata is missing".to_string())?;
    let reporter_player_id = hosted_player_id_for_slot(session.local_slot, hosted).to_string();
    let (winner, loser) = final_winner_loser(game)?;
    let winner_player_id = hosted_player_id_for_slot(winner, hosted).to_string();
    let loser_player_id = hosted_player_id_for_slot(loser, hosted).to_string();
    let winner_state = game.player(core_player(winner));
    let loser_state = game.player(core_player(loser));

    Ok(RankedResultClaim {
        session_id: hosted.session_id.clone(),
        reporter_player_id,
        winner_player_id,
        loser_player_id,
        winner_score: nonnegative_score(winner_state.score()),
        winner_lines: u64::from(winner_state.lines()),
        winner_funds: i64::from(winner_state.funds()),
        loser_score: nonnegative_score(loser_state.score()),
        loser_lines: u64::from(loser_state.lines()),
        loser_funds: i64::from(loser_state.funds()),
        duration_secs: duration_ticks.saturating_mul(NETWORK_TICK_MS) / 1_000,
        duration_ticks,
        event_count: game.event_log().len() as u64,
        final_checksum: game.deterministic_checksum(),
    })
}

fn final_winner_loser(game: &TwoPlayerGame) -> Result<(PlayerSlot, PlayerSlot), String> {
    game.event_log()
        .iter()
        .rev()
        .find_map(|logged| match logged.event {
            BattleEvent::GameOver { winner, loser } => {
                Some((wire_player(winner), wire_player(loser)))
            }
            _ => None,
        })
        .ok_or_else(|| "game-over event is missing".to_string())
}

fn hosted_player_id_for_slot(slot: PlayerSlot, hosted: &HostedSessionMetadata) -> &str {
    match slot {
        PlayerSlot::One => &hosted.player_one.player_id,
        PlayerSlot::Two => &hosted.player_two.player_id,
    }
}

fn nonnegative_score(score: i32) -> u64 {
    u64::try_from(score).unwrap_or(0)
}

const fn wire_player(player: PlayerId) -> PlayerSlot {
    match player {
        PlayerId::One => PlayerSlot::One,
        PlayerId::Two => PlayerSlot::Two,
    }
}

/// Renderable lifecycle state for networking UI.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkLifecycleState {
    /// No network operation is active.
    Idle,
    /// Listening for a direct peer.
    Hosting { bind_addr: SocketAddr },
    /// Connecting to a direct peer.
    Joining { peer_addr: SocketAddr },
    /// A direct challenge is waiting for host accept/deny.
    Challenged { challenge: Challenge },
    /// A game is connected and ready for deterministic simulation.
    Connected { session: Box<NetworkSession> },
    /// Disconnect command was sent and cleanup is underway.
    Disconnecting,
    /// Last operation failed.
    Error { message: String },
}

/// Commands sent from Bevy/UI systems to networking tasks.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkCommand {
    /// Listen for one direct peer.
    Host {
        bind_addr: SocketAddr,
        identity: PlayerIdentity,
    },
    /// Advertise a joinable direct host through best-effort LAN DNS-SD.
    StartLanAdvertising {
        identity: PlayerIdentity,
        share_addr: SocketAddr,
    },
    /// Withdraw the current LAN DNS-SD advertisement, if any.
    StopLanAdvertising,
    /// Browse best-effort LAN DNS-SD advertisements.
    BrowseLan,
    /// Join a direct peer by socket address.
    Join {
        peer_addr: SocketAddr,
        identity: PlayerIdentity,
        challenge_text: String,
    },
    /// Join a hosted direct peer after receiving server-owned start metadata.
    JoinHostedDirect {
        peer_addr: SocketAddr,
        identity: PlayerIdentity,
        challenge_text: String,
        hosted_session_id: HostedSessionId,
        hosted_player_id: String,
        hosted_start: HostedGameStart,
    },
    /// Register an available hosted lobby entry with the self-hosted server.
    RegisterLobby {
        server_addr: SocketAddr,
        request: LobbyRegister,
    },
    /// Browse currently available hosted lobby entries.
    BrowseLobby {
        server_addr: SocketAddr,
        ranked_only: bool,
    },
    /// Fetch server-owned ranked records for the hosted community.
    FetchRankedRecords { server_addr: SocketAddr, limit: u16 },
    /// Ask the server to start a hosted session for a joiner.
    StartHostedGame {
        server_addr: SocketAddr,
        session_id: HostedSessionId,
        joiner: HostedPlayer,
    },
    /// Poll server-owned hosted session status.
    PollHostedStatus {
        server_addr: SocketAddr,
        session_id: HostedSessionId,
        requester_player_id: String,
    },
    /// Cancel an available hosted lobby session.
    CancelHostedSession {
        server_addr: SocketAddr,
        session_id: HostedSessionId,
        requester_player_id: String,
    },
    /// Accept an incoming challenge.
    Accept { seed: u64, ranked: bool },
    /// Accept an incoming challenge using server-owned hosted metadata.
    AcceptHosted { hosted_start: HostedGameStart },
    /// Deny an incoming challenge.
    Deny { reason: String },
    /// Send a scheduled deterministic player input.
    SendScheduledInput(PlayerInput),
    /// Advertise local input completeness through a deterministic tick.
    SendTickWatermark(TickWatermark),
    /// Send sparse liveness and local deterministic progress.
    SendHeartbeat(Heartbeat),
    /// Send a whole-game checksum diagnostic.
    SendChecksum(GameChecksum),
    /// Send final game-over metadata.
    SendGameOver(GameOver),
    /// Send a Bazaar buy intent.
    SendBazaarBuy(BazaarBuy),
    /// Send a Bazaar remove intent.
    SendBazaarRemove(BazaarRemove),
    /// Send a Bazaar done intent.
    SendBazaarDone { player: PlayerSlot },
    /// Submit a hosted ranked result claim.
    SubmitResult {
        server_addr: SocketAddr,
        claim: RankedResultClaim,
    },
    /// Cancel the active host/join/challenge operation.
    Cancel,
    /// Gracefully disconnect from the peer.
    Disconnect { reason: String },
}

/// Events sent from networking tasks back to Bevy/UI systems.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkEvent {
    /// Direct host is listening.
    Listening {
        bind_addr: SocketAddr,
        share_addr: SocketAddr,
    },
    /// Local direct host is being advertised through LAN discovery.
    LanAdvertisingStarted { advertisement: LanAdvertisement },
    /// Local LAN discovery advertisement was withdrawn.
    LanAdvertisingStopped,
    /// LAN discovery browse returned currently resolved entries.
    LanDiscoveryEntries { entries: Vec<LanDiscoveryEntry> },
    /// LAN discovery is disabled or unavailable; manual direct IP remains usable.
    LanDiscoveryUnavailable { reason: String },
    /// A direct challenge arrived.
    IncomingChallenge { challenge: Challenge },
    /// Network game connected.
    Connected { session: Box<NetworkSession> },
    /// Server registered this hosted lobby entry.
    LobbyRegistered(LobbyEntry),
    /// Server returned the hosted lobby list.
    LobbyList(LobbyList),
    /// Server started a hosted game for the local participant.
    HostedGameStarted(HostedGameStart),
    /// Server returned hosted session status for a participant.
    HostedSessionStatus(HostedSessionStatus),
    /// Remote scheduled input arrived.
    InputReceived(PlayerInput),
    /// Peer advertised deterministic progress.
    TickWatermark(TickWatermark),
    /// Peer sent sparse liveness and deterministic progress.
    Heartbeat(Heartbeat),
    /// Peer Bazaar buy intent arrived.
    BazaarBuy(BazaarBuy),
    /// Peer Bazaar remove intent arrived.
    BazaarRemove(BazaarRemove),
    /// Peer Bazaar done intent arrived.
    BazaarDone { player: PlayerSlot },
    /// Peer checksum diagnostic arrived.
    Checksum(GameChecksum),
    /// Peer checksum disagreed with local deterministic state.
    DesyncDetected(Box<DesyncReport>),
    /// Game-over message arrived.
    GameOver(GameOver),
    /// Hosted ranked result is pending a peer claim.
    PendingResult(RankedResultPending),
    /// Hosted ranked result was recorded by the server.
    RecordedResult(RankedResultAccepted),
    /// Hosted ranked result was rejected by the server.
    RejectedResult(RankedResultRejected),
    /// Server-owned ranked records arrived.
    RankedRecords(RankedRecords),
    /// Network lifecycle changed.
    StateChanged(NetworkLifecycleState),
    /// Recoverable or terminal networking error.
    Error { message: String },
}

/// Bevy-owned typed handles for the networking boundary.
#[derive(Debug)]
pub struct NetworkChannels {
    command_tx: mpsc::Sender<NetworkCommand>,
    event_rx: mpsc::Receiver<NetworkEvent>,
}

impl NetworkChannels {
    /// Creates bounded command/event channels.
    #[must_use]
    pub fn bounded(capacity: usize) -> (Self, NetworkIo) {
        let (command_tx, command_rx) = mpsc::channel(capacity);
        let (event_tx, event_rx) = mpsc::channel(capacity);
        (
            Self {
                command_tx,
                event_rx,
            },
            NetworkIo {
                command_rx,
                event_tx,
            },
        )
    }

    /// Returns a cloneable command sender for UI systems.
    #[must_use]
    pub fn command_sender(&self) -> mpsc::Sender<NetworkCommand> {
        self.command_tx.clone()
    }

    /// Non-blocking receive for Bevy update systems.
    pub fn try_recv_event(&mut self) -> Result<NetworkEvent, mpsc::error::TryRecvError> {
        self.event_rx.try_recv()
    }
}

/// Networking-task side of [`NetworkChannels`].
#[derive(Debug)]
pub struct NetworkIo {
    command_rx: mpsc::Receiver<NetworkCommand>,
    event_tx: mpsc::Sender<NetworkEvent>,
}

/// Owner for Tokio runtime/thread state used by networking.
#[derive(Debug)]
pub struct NetworkRuntime {
    channels: NetworkChannels,
    shutdown_tx: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl NetworkRuntime {
    /// Starts the networking runtime thread with bounded channels.
    #[must_use]
    pub fn start() -> Self {
        let (channels, io) = NetworkChannels::bounded(DEFAULT_CHANNEL_CAPACITY);
        Self::start_with_io(channels, io)
    }

    fn start_with_io(channels: NetworkChannels, mut io: NetworkIo) -> Self {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let thread = thread::spawn(move || {
            let runtime = Runtime::new().expect("network runtime starts");
            runtime.block_on(async move {
                run_network_loop(&mut io, &mut shutdown_rx).await;
            });
        });

        Self {
            channels,
            shutdown_tx: Some(shutdown_tx),
            thread: Some(thread),
        }
    }

    /// Returns the UI-facing channels.
    pub fn channels_mut(&mut self) -> &mut NetworkChannels {
        &mut self.channels
    }

    /// Stops the runtime thread and waits for cleanup.
    pub fn shutdown(mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

enum ActiveConnection {
    Idle,
    PendingDirect {
        pending: battletris_protocol::PendingDirectChallenge,
    },
    Connected {
        connection: DirectConnection,
        session: Box<NetworkSession>,
    },
}

async fn run_network_loop(io: &mut NetworkIo, shutdown_rx: &mut oneshot::Receiver<()>) {
    let mut active = ActiveConnection::Idle;
    let mut discovery = LanDiscoveryRuntime::default();
    loop {
        match &mut active {
            ActiveConnection::Connected {
                connection,
                session,
            } => {
                tokio::select! {
                    _ = &mut *shutdown_rx => break,
                    command = io.command_rx.recv() => {
                        let Some(command) = command else { break };
                        if handle_connected_command(command, connection, &mut discovery, io).await {
                            active = ActiveConnection::Idle;
                        }
                    }
                    message = timeout(PEER_IDLE_TIMEOUT, connection.recv()) => {
                        match message {
                            Ok(Ok(message)) => {
                                if handle_peer_message(message, session, io).await {
                                    active = ActiveConnection::Idle;
                                }
                            }
                            Ok(Err(error)) => {
                                emit_error(io, format_protocol_error("direct peer read failed", &error)).await;
                                emit_state(io, NetworkLifecycleState::Idle).await;
                                log_net("direct peer read failed");
                                active = ActiveConnection::Idle;
                            }
                            Err(_) => {
                                let detail = format!(
                                    "peer idle timeout after {}s: slot={:?} tick={} peer_watermark={:?}",
                                    PEER_IDLE_TIMEOUT.as_secs(),
                                    session.local_slot,
                                    session.current_tick,
                                    session.peer_watermark
                                );
                                emit_error(io, detail.clone()).await;
                                emit_state(io, NetworkLifecycleState::Idle).await;
                                log_net(&detail);
                                active = ActiveConnection::Idle;
                            }
                        }
                    }
                }
            }
            _ => {
                tokio::select! {
                    _ = &mut *shutdown_rx => break,
                    command = io.command_rx.recv() => {
                        let Some(command) = command else { break };
                        handle_lifecycle_command(command, &mut active, &mut discovery, io).await;
                    }
                }
            }
        }
    }
    discovery.stop_advertising(io).await;
}

async fn handle_lifecycle_command(
    command: NetworkCommand,
    active: &mut ActiveConnection,
    discovery: &mut LanDiscoveryRuntime,
    io: &mut NetworkIo,
) {
    match command {
        NetworkCommand::Host {
            bind_addr,
            identity,
        } => {
            *active = ActiveConnection::Idle;
            host_until_challenge(bind_addr, identity, active, discovery, io).await;
        }
        NetworkCommand::StartLanAdvertising {
            identity,
            share_addr,
        } => discovery.start_advertising(identity, share_addr, io).await,
        NetworkCommand::StopLanAdvertising => discovery.stop_advertising(io).await,
        NetworkCommand::BrowseLan => discovery.browse(io).await,
        NetworkCommand::Join {
            peer_addr,
            identity,
            challenge_text,
        } => {
            *active = ActiveConnection::Idle;
            join_direct(peer_addr, identity, challenge_text, active, io).await;
        }
        NetworkCommand::JoinHostedDirect {
            peer_addr,
            identity,
            challenge_text,
            hosted_session_id,
            hosted_player_id,
            hosted_start,
        } => {
            *active = ActiveConnection::Idle;
            join_hosted_direct(
                HostedDirectJoin {
                    peer_addr,
                    identity,
                    challenge_text,
                    hosted_session_id,
                    hosted_player_id,
                    hosted_start,
                },
                active,
                io,
            )
            .await;
        }
        NetworkCommand::RegisterLobby {
            server_addr,
            request,
        } => register_lobby(server_addr, request, io).await,
        NetworkCommand::BrowseLobby {
            server_addr,
            ranked_only,
        } => browse_lobby(server_addr, ranked_only, io).await,
        NetworkCommand::FetchRankedRecords { server_addr, limit } => {
            fetch_ranked_records(server_addr, limit, io).await
        }
        NetworkCommand::StartHostedGame {
            server_addr,
            session_id,
            joiner,
        } => start_hosted_game(server_addr, session_id, joiner, io).await,
        NetworkCommand::PollHostedStatus {
            server_addr,
            session_id,
            requester_player_id,
        } => poll_hosted_status(server_addr, session_id, requester_player_id, io).await,
        NetworkCommand::CancelHostedSession {
            server_addr,
            session_id,
            requester_player_id,
        } => cancel_hosted_session(server_addr, session_id, requester_player_id, io).await,
        NetworkCommand::SubmitResult { server_addr, claim } => {
            submit_ranked_result(server_addr, claim, io).await
        }
        NetworkCommand::Accept { seed, ranked } => {
            discovery.stop_advertising(io).await;
            let ActiveConnection::PendingDirect { pending } =
                std::mem::replace(active, ActiveConnection::Idle)
            else {
                emit_error(io, "no pending direct challenge to accept".to_string()).await;
                return;
            };
            match timeout(CHALLENGE_RESPONSE_TIMEOUT, pending.accept(seed, ranked)).await {
                Ok(Ok(accepted)) => {
                    let session = NetworkSession::direct(
                        PlayerSlot::One,
                        accepted.remote_identity,
                        StartGame {
                            receiving_peer_slot: PlayerSlot::One,
                            seed,
                            ranked,
                        },
                    );
                    emit_connected(io, &session).await;
                    *active = ActiveConnection::Connected {
                        connection: accepted.connection,
                        session: Box::new(session),
                    };
                }
                Ok(Err(error)) => {
                    emit_error(
                        io,
                        format_protocol_error("accept direct challenge failed", &error),
                    )
                    .await;
                    emit_state(io, NetworkLifecycleState::Idle).await;
                }
                Err(_) => {
                    emit_error(io, "timed out accepting direct challenge".to_string()).await;
                    emit_state(io, NetworkLifecycleState::Idle).await;
                }
            }
        }
        NetworkCommand::AcceptHosted { hosted_start } => {
            discovery.stop_advertising(io).await;
            let ActiveConnection::PendingDirect { pending } =
                std::mem::replace(active, ActiveConnection::Idle)
            else {
                emit_error(io, "no pending hosted challenge to accept".to_string()).await;
                return;
            };
            match timeout(
                CHALLENGE_RESPONSE_TIMEOUT,
                pending.accept(hosted_start.seed, hosted_start.ranked),
            )
            .await
            {
                Ok(Ok(accepted)) => {
                    let session = NetworkSession::hosted(
                        PlayerSlot::One,
                        accepted.remote_identity,
                        hosted_start,
                    );
                    emit_connected(io, &session).await;
                    *active = ActiveConnection::Connected {
                        connection: accepted.connection,
                        session: Box::new(session),
                    };
                }
                Ok(Err(error)) => {
                    emit_error(
                        io,
                        format_protocol_error("accept hosted challenge failed", &error),
                    )
                    .await;
                    emit_state(io, NetworkLifecycleState::Idle).await;
                }
                Err(_) => {
                    emit_error(io, "timed out accepting hosted challenge".to_string()).await;
                    emit_state(io, NetworkLifecycleState::Idle).await;
                }
            }
        }
        NetworkCommand::Deny { reason } => {
            let ActiveConnection::PendingDirect { pending } =
                std::mem::replace(active, ActiveConnection::Idle)
            else {
                emit_error(io, "no pending direct challenge to deny".to_string()).await;
                return;
            };
            if let Err(error) = pending.deny(reason).await {
                emit_error(
                    io,
                    format_protocol_error("deny direct challenge failed", &error),
                )
                .await;
            }
            emit_state(io, NetworkLifecycleState::Idle).await;
        }
        NetworkCommand::Cancel => {
            discovery.stop_advertising(io).await;
            *active = ActiveConnection::Idle;
            emit_state(io, NetworkLifecycleState::Idle).await;
        }
        NetworkCommand::Disconnect { reason } => {
            discovery.stop_advertising(io).await;
            emit_state(io, NetworkLifecycleState::Disconnecting).await;
            emit_error(io, reason).await;
            emit_state(io, NetworkLifecycleState::Idle).await;
        }
        NetworkCommand::SendScheduledInput(_)
        | NetworkCommand::SendTickWatermark(_)
        | NetworkCommand::SendHeartbeat(_)
        | NetworkCommand::SendChecksum(_)
        | NetworkCommand::SendGameOver(_)
        | NetworkCommand::SendBazaarBuy(_)
        | NetworkCommand::SendBazaarRemove(_)
        | NetworkCommand::SendBazaarDone { .. } => {}
    }
}

async fn register_lobby(server_addr: SocketAddr, request: LobbyRegister, io: &mut NetworkIo) {
    match server_request(server_addr, WireMessage::LobbyRegister(request)).await {
        Ok(WireMessage::LobbyList(LobbyList { entries })) if entries.len() == 1 => {
            let entry = entries.into_iter().next().expect("entry count checked");
            let _ = io.event_tx.send(NetworkEvent::LobbyRegistered(entry)).await;
        }
        Ok(WireMessage::LobbyList(_)) => {
            emit_error(
                io,
                "lobby registration returned an unexpected entry count".to_string(),
            )
            .await;
        }
        Ok(WireMessage::RankedResultRejected(rejected)) => emit_error(io, rejected.reason).await,
        Ok(message) => {
            emit_error(
                io,
                format!("unexpected lobby register response: {:?}", message.kind()),
            )
            .await
        }
        Err(error) => emit_error(io, error).await,
    }
}

async fn browse_lobby(server_addr: SocketAddr, ranked_only: bool, io: &mut NetworkIo) {
    match server_request(
        server_addr,
        WireMessage::LobbyListRequest(LobbyListRequest { ranked_only }),
    )
    .await
    {
        Ok(WireMessage::LobbyList(list)) => {
            let _ = io.event_tx.send(NetworkEvent::LobbyList(list)).await;
        }
        Ok(WireMessage::RankedResultRejected(rejected)) => emit_error(io, rejected.reason).await,
        Ok(message) => {
            emit_error(
                io,
                format!("unexpected lobby list response: {:?}", message.kind()),
            )
            .await
        }
        Err(error) => emit_error(io, error).await,
    }
}

async fn fetch_ranked_records(server_addr: SocketAddr, limit: u16, io: &mut NetworkIo) {
    match server_request(
        server_addr,
        WireMessage::RankedRecordsRequest(RankedRecordsRequest { limit }),
    )
    .await
    {
        Ok(WireMessage::RankedRecords(records)) => {
            let _ = io.event_tx.send(NetworkEvent::RankedRecords(records)).await;
        }
        Ok(WireMessage::RankedResultRejected(rejected)) => {
            let _ = io
                .event_tx
                .send(NetworkEvent::RejectedResult(rejected))
                .await;
        }
        Ok(message) => {
            emit_error(
                io,
                format!("unexpected ranked records response: {:?}", message.kind()),
            )
            .await
        }
        Err(error) => emit_error(io, error).await,
    }
}

async fn submit_ranked_result(
    server_addr: SocketAddr,
    claim: RankedResultClaim,
    io: &mut NetworkIo,
) {
    match server_request(server_addr, WireMessage::RankedResultClaim(claim)).await {
        Ok(WireMessage::RankedResultPending(pending)) => {
            let _ = io.event_tx.send(NetworkEvent::PendingResult(pending)).await;
        }
        Ok(WireMessage::RankedResultAccepted(accepted)) => {
            let _ = io
                .event_tx
                .send(NetworkEvent::RecordedResult(accepted))
                .await;
        }
        Ok(WireMessage::RankedResultRejected(rejected)) => {
            let _ = io
                .event_tx
                .send(NetworkEvent::RejectedResult(rejected))
                .await;
        }
        Ok(message) => {
            emit_error(
                io,
                format!("unexpected ranked result response: {:?}", message.kind()),
            )
            .await
        }
        Err(error) => emit_error(io, error).await,
    }
}

async fn start_hosted_game(
    server_addr: SocketAddr,
    session_id: HostedSessionId,
    joiner: HostedPlayer,
    io: &mut NetworkIo,
) {
    match server_request(
        server_addr,
        WireMessage::HostedJoinRequest(HostedJoinRequest { session_id, joiner }),
    )
    .await
    {
        Ok(WireMessage::HostedGameStart(start)) => {
            let _ = io
                .event_tx
                .send(NetworkEvent::HostedGameStarted(start))
                .await;
        }
        Ok(WireMessage::RankedResultRejected(rejected)) => emit_error(io, rejected.reason).await,
        Ok(message) => {
            emit_error(
                io,
                format!("unexpected hosted join response: {:?}", message.kind()),
            )
            .await
        }
        Err(error) => emit_error(io, error).await,
    }
}

async fn poll_hosted_status(
    server_addr: SocketAddr,
    session_id: HostedSessionId,
    requester_player_id: String,
    io: &mut NetworkIo,
) {
    match server_request(
        server_addr,
        WireMessage::HostedSessionStatusRequest(HostedSessionStatusRequest {
            session_id,
            requester_player_id,
        }),
    )
    .await
    {
        Ok(WireMessage::HostedSessionStatus(status)) => {
            let _ = io
                .event_tx
                .send(NetworkEvent::HostedSessionStatus(status))
                .await;
        }
        Ok(WireMessage::RankedResultRejected(rejected)) => emit_error(io, rejected.reason).await,
        Ok(message) => {
            emit_error(
                io,
                format!("unexpected hosted status response: {:?}", message.kind()),
            )
            .await
        }
        Err(error) => emit_error(io, error).await,
    }
}

async fn cancel_hosted_session(
    server_addr: SocketAddr,
    session_id: HostedSessionId,
    requester_player_id: String,
    io: &mut NetworkIo,
) {
    match server_request(
        server_addr,
        WireMessage::HostedSessionCancel(HostedSessionCancel {
            session_id,
            requester_player_id,
        }),
    )
    .await
    {
        Ok(WireMessage::HostedSessionStatus(status)) => {
            let _ = io
                .event_tx
                .send(NetworkEvent::HostedSessionStatus(status))
                .await;
        }
        Ok(WireMessage::RankedResultRejected(rejected)) => emit_error(io, rejected.reason).await,
        Ok(message) => {
            emit_error(
                io,
                format!("unexpected hosted cancel response: {:?}", message.kind()),
            )
            .await
        }
        Err(error) => emit_error(io, error).await,
    }
}

async fn host_until_challenge(
    bind_addr: SocketAddr,
    identity: PlayerIdentity,
    active: &mut ActiveConnection,
    discovery: &mut LanDiscoveryRuntime,
    io: &mut NetworkIo,
) {
    log_net(&format!("hosting direct game on {bind_addr}"));
    emit_state(io, NetworkLifecycleState::Hosting { bind_addr }).await;
    let listener = match TcpListener::bind(bind_addr).await {
        Ok(listener) => listener,
        Err(error) => {
            let message = if error.kind() == std::io::ErrorKind::AddrInUse {
                "Host bind failed: address already in use. Try another port or cancel the old host."
                    .to_string()
            } else {
                format!(
                    "Host bind failed on {bind_addr}: {error}. Try another local address or port."
                )
            };
            emit_error(io, message).await;
            emit_state(io, NetworkLifecycleState::Idle).await;
            return;
        }
    };
    let actual_addr = listener.local_addr().unwrap_or(bind_addr);
    let share_addr = share_addr_for(actual_addr);
    log_net(&format!(
        "direct host listening bind={actual_addr} share={share_addr}"
    ));
    let _ = io
        .event_tx
        .send(NetworkEvent::Listening {
            bind_addr: actual_addr,
            share_addr,
        })
        .await;
    emit_state(
        io,
        NetworkLifecycleState::Hosting {
            bind_addr: actual_addr,
        },
    )
    .await;

    loop {
        tokio::select! {
            pending = battletris_protocol::accept_pending_direct_challenge(&listener, identity.clone()) => {
                match pending {
                    Ok(pending) => {
                    discovery.stop_advertising(io).await;
                    log_net(&format!(
                        "incoming direct challenge peer={} hosted_session={:?}",
                        pending.remote_identity.display_name,
                        pending.challenge.hosted_session_id
                    ));
                    let challenge = pending.challenge.clone();
                    emit_state(io, NetworkLifecycleState::Challenged { challenge: challenge.clone() }).await;
                    let _ = io.event_tx.send(NetworkEvent::IncomingChallenge { challenge }).await;
                    *active = ActiveConnection::PendingDirect { pending };
                    break;
                }
                    Err(error) => {
                    log_net(&format_protocol_error("direct host handshake failed", &error));
                    emit_error(io, format_protocol_error("direct host handshake failed", &error)).await;
                    emit_state(io, NetworkLifecycleState::Idle).await;
                    break;
                }
                }
            }
            command = io.command_rx.recv() => {
                match command {
                    Some(NetworkCommand::Cancel) => {
                        discovery.stop_advertising(io).await;
                        emit_state(io, NetworkLifecycleState::Idle).await;
                        break;
                    }
                    Some(NetworkCommand::StartLanAdvertising { identity, share_addr }) => discovery.start_advertising(identity, share_addr, io).await,
                    Some(NetworkCommand::StopLanAdvertising) => discovery.stop_advertising(io).await,
                    Some(NetworkCommand::BrowseLan) => discovery.browse(io).await,
                    Some(NetworkCommand::RegisterLobby { server_addr, request }) => register_lobby(server_addr, request, io).await,
                    Some(NetworkCommand::BrowseLobby { server_addr, ranked_only }) => browse_lobby(server_addr, ranked_only, io).await,
                    Some(NetworkCommand::StartHostedGame { server_addr, session_id, joiner }) => start_hosted_game(server_addr, session_id, joiner, io).await,
                    Some(NetworkCommand::PollHostedStatus { server_addr, session_id, requester_player_id }) => poll_hosted_status(server_addr, session_id, requester_player_id, io).await,
                    Some(NetworkCommand::CancelHostedSession { server_addr, session_id, requester_player_id }) => cancel_hosted_session(server_addr, session_id, requester_player_id, io).await,
                    Some(NetworkCommand::FetchRankedRecords { server_addr, limit }) => fetch_ranked_records(server_addr, limit, io).await,
                    Some(other) => {
                        emit_error(io, format!("direct host canceled by {:?}", command_name(&other))).await;
                        emit_state(io, NetworkLifecycleState::Idle).await;
                        break;
                    }
                    None => break,
                }
            }
        }
    }
}

async fn join_direct(
    peer_addr: SocketAddr,
    identity: PlayerIdentity,
    challenge_text: String,
    active: &mut ActiveConnection,
    io: &mut NetworkIo,
) {
    log_net(&format!("joining direct peer {peer_addr}"));
    emit_state(io, NetworkLifecycleState::Joining { peer_addr }).await;
    let joined = tokio::select! {
        joined = timeout(CONNECT_TIMEOUT + HANDSHAKE_TIMEOUT, battletris_protocol::join_direct_game(peer_addr, identity, challenge_text)) => joined,
        command = io.command_rx.recv() => {
            match command {
                Some(NetworkCommand::Cancel) => emit_state(io, NetworkLifecycleState::Idle).await,
                Some(other) => {
                    emit_error(io, format!("direct join canceled by {:?}", command_name(&other))).await;
                    emit_state(io, NetworkLifecycleState::Idle).await;
                }
                None => {}
            }
            return;
        }
    };

    match joined {
        Ok(Ok(joined)) => connect_joined(joined, active, io).await,
        Ok(Err(ProtocolError::ChallengeDenied { reason })) => {
            emit_error(io, format!("Challenge denied: {reason}.")).await;
            emit_state(io, NetworkLifecycleState::Idle).await;
        }
        Ok(Err(error)) => {
            log_net(&format_protocol_error("direct join failed", &error));
            emit_error(io, format_protocol_error("direct join failed", &error)).await;
            emit_state(io, NetworkLifecycleState::Idle).await;
        }
        Err(_) => {
            log_net(&format!("timed out connecting to direct peer {peer_addr}"));
            emit_error(
                io,
                format!(
                    "Join timed out. Check the host share address and firewall. Tried {peer_addr}."
                ),
            )
            .await;
            emit_state(io, NetworkLifecycleState::Idle).await;
        }
    }
}

struct HostedDirectJoin {
    peer_addr: SocketAddr,
    identity: PlayerIdentity,
    challenge_text: String,
    hosted_session_id: HostedSessionId,
    hosted_player_id: String,
    hosted_start: HostedGameStart,
}

async fn join_hosted_direct(
    request: HostedDirectJoin,
    active: &mut ActiveConnection,
    io: &mut NetworkIo,
) {
    let HostedDirectJoin {
        peer_addr,
        identity,
        challenge_text,
        hosted_session_id,
        hosted_player_id,
        hosted_start,
    } = request;
    emit_state(io, NetworkLifecycleState::Joining { peer_addr }).await;
    log_net(&format!(
        "joining hosted direct peer {peer_addr} session={}",
        hosted_session_id.0
    ));
    let challenge = Challenge {
        challenger: identity,
        message: challenge_text,
        hosted_session_id: Some(hosted_session_id),
        hosted_player_id: Some(hosted_player_id),
    };
    let joined = tokio::select! {
        joined = timeout(CONNECT_TIMEOUT + HANDSHAKE_TIMEOUT, battletris_protocol::join_direct_game_with_challenge(peer_addr, challenge)) => joined,
        command = io.command_rx.recv() => {
            match command {
                Some(NetworkCommand::Cancel) => emit_state(io, NetworkLifecycleState::Idle).await,
                Some(other) => {
                    emit_error(io, format!("hosted direct join canceled by {:?}", command_name(&other))).await;
                    emit_state(io, NetworkLifecycleState::Idle).await;
                }
                None => {}
            }
            return;
        }
    };

    match joined {
        Ok(Ok(joined)) => {
            let session = NetworkSession::hosted(
                joined.start.receiving_peer_slot,
                joined.remote_identity,
                hosted_start,
            );
            emit_connected(io, &session).await;
            *active = ActiveConnection::Connected {
                connection: joined.connection,
                session: Box::new(session),
            };
        }
        Ok(Err(ProtocolError::ChallengeDenied { reason })) => {
            emit_error(io, format!("Challenge denied: {reason}.")).await;
            emit_state(io, NetworkLifecycleState::Idle).await;
        }
        Ok(Err(error)) => {
            emit_error(
                io,
                format_protocol_error("hosted direct join failed", &error),
            )
            .await;
            emit_state(io, NetworkLifecycleState::Idle).await;
        }
        Err(_) => {
            emit_error(
                io,
                format!("Join timed out. Check the host share address and firewall. Tried hosted direct peer {peer_addr}."),
            )
            .await;
            emit_state(io, NetworkLifecycleState::Idle).await;
        }
    }
}

async fn connect_joined(
    joined: JoinedDirectGame,
    active: &mut ActiveConnection,
    io: &mut NetworkIo,
) {
    let session = NetworkSession::direct(
        joined.start.receiving_peer_slot,
        joined.remote_identity,
        joined.start,
    );
    emit_connected(io, &session).await;
    *active = ActiveConnection::Connected {
        connection: joined.connection,
        session: Box::new(session),
    };
}

async fn handle_connected_command(
    command: NetworkCommand,
    connection: &mut DirectConnection,
    discovery: &mut LanDiscoveryRuntime,
    io: &mut NetworkIo,
) -> bool {
    let message = match command {
        NetworkCommand::StartLanAdvertising { .. } => {
            emit_error(
                io,
                "cannot advertise a LAN host while connected".to_string(),
            )
            .await;
            None
        }
        NetworkCommand::StopLanAdvertising => {
            discovery.stop_advertising(io).await;
            None
        }
        NetworkCommand::BrowseLan => {
            discovery.browse(io).await;
            None
        }
        NetworkCommand::SendScheduledInput(input) => Some(WireMessage::PlayerInput(input)),
        NetworkCommand::SendTickWatermark(watermark) => Some(WireMessage::TickWatermark(watermark)),
        NetworkCommand::SendHeartbeat(heartbeat) => Some(WireMessage::Heartbeat(heartbeat)),
        NetworkCommand::SendChecksum(checksum) => Some(WireMessage::GameChecksum(checksum)),
        NetworkCommand::SendGameOver(game_over) => Some(WireMessage::GameOver(game_over)),
        NetworkCommand::SendBazaarBuy(buy) => Some(WireMessage::BazaarBuy(buy)),
        NetworkCommand::SendBazaarRemove(remove) => Some(WireMessage::BazaarRemove(remove)),
        NetworkCommand::SendBazaarDone { player } => {
            Some(WireMessage::BazaarDone(BazaarDone { player }))
        }
        NetworkCommand::Disconnect { reason } => {
            Some(WireMessage::Disconnect(Disconnect { reason }))
        }
        NetworkCommand::Cancel => Some(WireMessage::Disconnect(Disconnect {
            reason: "canceled".to_string(),
        })),
        NetworkCommand::FetchRankedRecords { server_addr, limit } => {
            fetch_ranked_records(server_addr, limit, io).await;
            None
        }
        NetworkCommand::SubmitResult { server_addr, claim } => {
            submit_ranked_result(server_addr, claim, io).await;
            None
        }
        NetworkCommand::Host { .. }
        | NetworkCommand::Join { .. }
        | NetworkCommand::JoinHostedDirect { .. }
        | NetworkCommand::RegisterLobby { .. }
        | NetworkCommand::BrowseLobby { .. }
        | NetworkCommand::StartHostedGame { .. }
        | NetworkCommand::PollHostedStatus { .. }
        | NetworkCommand::CancelHostedSession { .. }
        | NetworkCommand::Accept { .. }
        | NetworkCommand::AcceptHosted { .. }
        | NetworkCommand::Deny { .. } => {
            emit_error(
                io,
                "network command is not valid while connected".to_string(),
            )
            .await;
            None
        }
    };

    if let Some(message) = message {
        if let Err(error) = connection.send(&message).await {
            emit_error(
                io,
                format_protocol_error("send direct peer message failed", &error),
            )
            .await;
            emit_state(io, NetworkLifecycleState::Idle).await;
            return true;
        }
        if matches!(message, WireMessage::Disconnect(_)) {
            emit_state(io, NetworkLifecycleState::Idle).await;
            return true;
        }
    }
    false
}

async fn handle_peer_message(
    message: WireMessage,
    session: &mut NetworkSession,
    io: &mut NetworkIo,
) -> bool {
    match message {
        WireMessage::PlayerInput(input) => {
            let _ = io.event_tx.send(NetworkEvent::InputReceived(input)).await;
        }
        WireMessage::TickWatermark(watermark) => {
            session.peer_watermark = Some(watermark.through_tick);
            let _ = io
                .event_tx
                .send(NetworkEvent::TickWatermark(watermark))
                .await;
        }
        WireMessage::Heartbeat(heartbeat) => {
            session.peer_watermark = Some(heartbeat.watermark_tick);
            let _ = io.event_tx.send(NetworkEvent::Heartbeat(heartbeat)).await;
        }
        WireMessage::BazaarBuy(buy) => {
            let _ = io.event_tx.send(NetworkEvent::BazaarBuy(buy)).await;
        }
        WireMessage::BazaarRemove(remove) => {
            let _ = io.event_tx.send(NetworkEvent::BazaarRemove(remove)).await;
        }
        WireMessage::BazaarDone(done) => {
            let _ = io
                .event_tx
                .send(NetworkEvent::BazaarDone {
                    player: done.player,
                })
                .await;
        }
        WireMessage::GameChecksum(checksum) => {
            let _ = io.event_tx.send(NetworkEvent::Checksum(checksum)).await;
        }
        WireMessage::GameOver(game_over) => {
            let _ = io.event_tx.send(NetworkEvent::GameOver(game_over)).await;
        }
        WireMessage::RankedResultPending(pending) => {
            session.final_result_status = FinalResultStatus::Pending(pending.clone());
            let _ = io.event_tx.send(NetworkEvent::PendingResult(pending)).await;
        }
        WireMessage::Disconnect(disconnect) => {
            emit_error(
                io,
                format!(
                    "Peer disconnected. The online game has ended. Reason: {}",
                    disconnect.reason
                ),
            )
            .await;
            emit_state(io, NetworkLifecycleState::Idle).await;
            return true;
        }
        other => {
            emit_error(
                io,
                format!("unexpected direct peer message: {:?}", other.kind()),
            )
            .await;
        }
    }
    false
}

async fn emit_connected(io: &mut NetworkIo, session: &NetworkSession) {
    log_net(&format!(
        "connected mode={:?} local_slot={:?} peer={} seed={} ranked={} session={:?}",
        session.mode,
        session.local_slot,
        session.peer_identity.display_name,
        session.base_seed,
        session.ranked,
        session.hosted.as_ref().map(|hosted| &hosted.session_id.0)
    ));
    let session = Box::new(session.clone());
    let _ = io
        .event_tx
        .send(NetworkEvent::Connected {
            session: session.clone(),
        })
        .await;
    emit_state(io, NetworkLifecycleState::Connected { session }).await;
}

async fn server_request(
    server_addr: SocketAddr,
    message: WireMessage,
) -> Result<WireMessage, String> {
    let mut stream = timeout(CONNECT_TIMEOUT, TcpStream::connect(server_addr))
        .await
        .map_err(|_| format!("Lobby server unavailable. Direct IP can still be used. Timed out connecting to {server_addr}."))?
        .map_err(|error| format!("Lobby server unavailable. Direct IP can still be used. Connect to {server_addr} failed: {error}."))?;
    battletris_protocol::write_message(&mut stream, &message)
        .await
        .map_err(|error| format_protocol_error("write lobby server request failed", &error))?;
    timeout(
        HANDSHAKE_TIMEOUT,
        battletris_protocol::read_message(&mut stream),
    )
    .await
    .map_err(|_| "Lobby server unavailable. Direct IP can still be used. Timed out waiting for lobby server response.".to_string())?
    .map_err(|error| format_protocol_error("read lobby server response failed", &error))
}

async fn emit_state(io: &mut NetworkIo, state: NetworkLifecycleState) {
    let _ = io.event_tx.send(NetworkEvent::StateChanged(state)).await;
}

async fn emit_error(io: &mut NetworkIo, message: String) {
    log_net(&format!("error: {message}"));
    let _ = io.event_tx.send(NetworkEvent::Error { message }).await;
}

fn log_net(message: &str) {
    eprintln!("battletris-client net: {message}");
}

fn format_protocol_error(context: &str, error: &ProtocolError) -> String {
    match error {
        ProtocolError::ChallengeDenied { reason } => {
            format!("{context}: challenge denied: {reason}")
        }
        _ => format!("{context}: {error:?}"),
    }
}

fn share_addr_for(bind_addr: SocketAddr) -> SocketAddr {
    if bind_addr.ip().is_unspecified() {
        SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            bind_addr.port(),
        )
    } else {
        bind_addr
    }
}

#[derive(Default)]
struct LanDiscoveryRuntime {
    #[cfg(feature = "lan-discovery")]
    daemon: Option<mdns_sd::ServiceDaemon>,
    #[cfg(feature = "lan-discovery")]
    advertised_fullname: Option<String>,
}

impl LanDiscoveryRuntime {
    async fn start_advertising(
        &mut self,
        identity: PlayerIdentity,
        share_addr: SocketAddr,
        io: &mut NetworkIo,
    ) {
        #[cfg(feature = "lan-discovery")]
        {
            self.stop_advertising(io).await;
            let daemon = match self.daemon() {
                Ok(daemon) => daemon,
                Err(reason) => {
                    emit_lan_unavailable(io, reason).await;
                    return;
                }
            };
            let advertisement = LanAdvertisement::available(&identity, share_addr.port());
            let service_info = match service_info_for(&identity, share_addr, &advertisement) {
                Ok(service_info) => service_info,
                Err(reason) => {
                    emit_lan_unavailable(io, reason).await;
                    return;
                }
            };
            let fullname = service_info.get_fullname().to_string();
            match daemon.register(service_info) {
                Ok(()) => {
                    self.advertised_fullname = Some(fullname);
                    let _ = io
                        .event_tx
                        .send(NetworkEvent::LanAdvertisingStarted { advertisement })
                        .await;
                }
                Err(error) => {
                    emit_lan_unavailable(io, format!("LAN advertisement failed: {error}")).await
                }
            }
        }

        #[cfg(not(feature = "lan-discovery"))]
        {
            let _ = (identity, share_addr);
            emit_lan_unavailable(
                io,
                "LAN discovery support is not compiled in; use manual direct IP".to_string(),
            )
            .await;
        }
    }

    async fn stop_advertising(&mut self, io: &mut NetworkIo) {
        #[cfg(feature = "lan-discovery")]
        {
            let Some(fullname) = self.advertised_fullname.take() else {
                return;
            };
            if let Some(daemon) = &self.daemon {
                if let Err(error) = daemon.unregister(&fullname) {
                    emit_lan_unavailable(
                        io,
                        format!("LAN advertisement withdrawal failed: {error}"),
                    )
                    .await;
                    return;
                }
            }
            let _ = io.event_tx.send(NetworkEvent::LanAdvertisingStopped).await;
        }

        #[cfg(not(feature = "lan-discovery"))]
        {
            let _ = io;
        }
    }

    async fn browse(&mut self, io: &mut NetworkIo) {
        #[cfg(feature = "lan-discovery")]
        {
            let daemon = match self.daemon() {
                Ok(daemon) => daemon,
                Err(reason) => {
                    emit_lan_unavailable(io, reason).await;
                    return;
                }
            };
            let receiver = match daemon.browse(LAN_DISCOVERY_SERVICE) {
                Ok(receiver) => receiver,
                Err(error) => {
                    emit_lan_unavailable(io, format!("LAN browse failed: {error}")).await;
                    return;
                }
            };
            let deadline = tokio::time::Instant::now() + LAN_BROWSE_TIMEOUT;
            let mut entries = BTreeMap::<String, LanDiscoveryEntry>::new();
            loop {
                let now = tokio::time::Instant::now();
                if now >= deadline {
                    break;
                }
                match timeout(deadline - now, receiver.recv_async()).await {
                    Ok(Ok(mdns_sd::ServiceEvent::ServiceResolved(service))) => {
                        if let Some(entry) = entry_from_resolved_service(&service) {
                            entries.insert(entry.instance_name.clone(), entry);
                        }
                    }
                    Ok(Ok(mdns_sd::ServiceEvent::ServiceRemoved(_, fullname))) => {
                        entries.remove(&fullname);
                    }
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        emit_lan_unavailable(io, format!("LAN browse receiver closed: {error}"))
                            .await;
                        return;
                    }
                    Err(_) => break,
                }
            }
            let _ = daemon.stop_browse(LAN_DISCOVERY_SERVICE);
            let _ = io
                .event_tx
                .send(NetworkEvent::LanDiscoveryEntries {
                    entries: entries.into_values().collect(),
                })
                .await;
        }

        #[cfg(not(feature = "lan-discovery"))]
        emit_lan_unavailable(
            io,
            "LAN discovery support is not compiled in; use manual direct IP".to_string(),
        )
        .await;
    }

    #[cfg(feature = "lan-discovery")]
    fn daemon(&mut self) -> Result<mdns_sd::ServiceDaemon, String> {
        if self.daemon.is_none() {
            self.daemon = Some(
                mdns_sd::ServiceDaemon::new()
                    .map_err(|error| format!("LAN discovery unavailable: {error}"))?,
            );
        }
        Ok(self
            .daemon
            .as_ref()
            .expect("daemon just initialized")
            .clone())
    }
}

async fn emit_lan_unavailable(io: &mut NetworkIo, reason: String) {
    let _ = io
        .event_tx
        .send(NetworkEvent::LanDiscoveryUnavailable { reason })
        .await;
}

#[cfg(feature = "lan-discovery")]
fn service_info_for(
    identity: &PlayerIdentity,
    share_addr: SocketAddr,
    advertisement: &LanAdvertisement,
) -> Result<mdns_sd::ServiceInfo, String> {
    let instance = lan_instance_name(&identity.display_name);
    let host_name = format!("battletris-{}.local.", share_addr.port());
    let txt = advertisement
        .txt
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<HashMap<_, _>>();
    mdns_sd::ServiceInfo::new(
        advertisement.service,
        &instance,
        &host_name,
        share_addr.ip().to_string(),
        advertisement.port,
        txt,
    )
    .map(mdns_sd::ServiceInfo::enable_addr_auto)
    .map_err(|error| format!("invalid LAN advertisement: {error}"))
}

#[cfg(feature = "lan-discovery")]
fn entry_from_resolved_service(service: &mdns_sd::ResolvedService) -> Option<LanDiscoveryEntry> {
    let protocol_major = property_u16(service, "protocol_major")?;
    let protocol_minor = property_u16(service, "protocol_minor").unwrap_or(0);
    let port = property_u16(service, "port").unwrap_or_else(|| service.get_port());
    let address = service
        .get_addresses_v4()
        .into_iter()
        .next()
        .map(|ip| SocketAddr::new(std::net::IpAddr::V4(ip), port));
    let display_name = service
        .get_property_val_str("display_name")
        .unwrap_or("BattleTris Host")
        .to_string();
    let availability = parse_lan_availability(service.get_property_val_str("state"));

    Some(LanDiscoveryEntry {
        instance_name: service.get_fullname().to_string(),
        hostname: service.get_hostname().to_string(),
        addr: address,
        port,
        display_name,
        protocol_major,
        protocol_minor,
        compatible: protocol_major == PROTOCOL_MAJOR,
        availability,
    })
}

#[cfg(feature = "lan-discovery")]
fn property_u16(service: &mdns_sd::ResolvedService, key: &str) -> Option<u16> {
    service.get_property_val_str(key)?.parse().ok()
}

/// Parses DNS-SD TXT availability metadata into a renderable LAN state.
#[must_use]
pub fn parse_lan_availability(value: Option<&str>) -> LanAvailability {
    match value {
        Some("available") => LanAvailability::Available,
        Some("busy") => LanAvailability::Busy,
        _ => LanAvailability::Unknown,
    }
}

/// Sanitizes a player display name into a bounded DNS-SD instance name.
#[must_use]
pub fn lan_instance_name(display_name: &str) -> String {
    let trimmed = display_name.trim();
    let name = if trimmed.is_empty() {
        "BattleTris Host"
    } else {
        trimmed
    };
    name.chars().take(30).collect()
}

fn command_name(command: &NetworkCommand) -> &'static str {
    match command {
        NetworkCommand::Host { .. } => "host",
        NetworkCommand::StartLanAdvertising { .. } => "start LAN advertising",
        NetworkCommand::StopLanAdvertising => "stop LAN advertising",
        NetworkCommand::BrowseLan => "browse LAN",
        NetworkCommand::Join { .. } => "join",
        NetworkCommand::JoinHostedDirect { .. } => "join hosted direct",
        NetworkCommand::RegisterLobby { .. } => "register lobby",
        NetworkCommand::BrowseLobby { .. } => "browse lobby",
        NetworkCommand::FetchRankedRecords { .. } => "fetch ranked records",
        NetworkCommand::StartHostedGame { .. } => "start hosted game",
        NetworkCommand::PollHostedStatus { .. } => "poll hosted status",
        NetworkCommand::CancelHostedSession { .. } => "cancel hosted session",
        NetworkCommand::Accept { .. } => "accept",
        NetworkCommand::AcceptHosted { .. } => "accept hosted",
        NetworkCommand::Deny { .. } => "deny",
        NetworkCommand::SendScheduledInput(_) => "send scheduled input",
        NetworkCommand::SendTickWatermark(_) => "send tick watermark",
        NetworkCommand::SendHeartbeat(_) => "send heartbeat",
        NetworkCommand::SendChecksum(_) => "send checksum",
        NetworkCommand::SendGameOver(_) => "send game over",
        NetworkCommand::SendBazaarBuy(_) => "send bazaar buy",
        NetworkCommand::SendBazaarRemove(_) => "send bazaar remove",
        NetworkCommand::SendBazaarDone { .. } => "send bazaar done",
        NetworkCommand::SubmitResult { .. } => "submit result",
        NetworkCommand::Cancel => "cancel",
        NetworkCommand::Disconnect { .. } => "disconnect",
    }
}

impl Drop for NetworkRuntime {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Reducer for deterministic lifecycle state transitions.
#[must_use]
pub fn reduce_state(
    state: &NetworkLifecycleState,
    command: &NetworkCommand,
) -> NetworkLifecycleState {
    match command {
        NetworkCommand::Host { bind_addr, .. } => NetworkLifecycleState::Hosting {
            bind_addr: *bind_addr,
        },
        NetworkCommand::Join { peer_addr, .. } => NetworkLifecycleState::Joining {
            peer_addr: *peer_addr,
        },
        NetworkCommand::JoinHostedDirect { peer_addr, .. } => NetworkLifecycleState::Joining {
            peer_addr: *peer_addr,
        },
        NetworkCommand::Cancel => NetworkLifecycleState::Idle,
        NetworkCommand::Disconnect { .. } => NetworkLifecycleState::Disconnecting,
        NetworkCommand::Deny { reason } => NetworkLifecycleState::Error {
            message: reason.clone(),
        },
        NetworkCommand::Accept { .. }
        | NetworkCommand::StartLanAdvertising { .. }
        | NetworkCommand::StopLanAdvertising
        | NetworkCommand::BrowseLan
        | NetworkCommand::RegisterLobby { .. }
        | NetworkCommand::BrowseLobby { .. }
        | NetworkCommand::FetchRankedRecords { .. }
        | NetworkCommand::StartHostedGame { .. }
        | NetworkCommand::PollHostedStatus { .. }
        | NetworkCommand::CancelHostedSession { .. }
        | NetworkCommand::AcceptHosted { .. }
        | NetworkCommand::SendScheduledInput(_)
        | NetworkCommand::SendTickWatermark(_)
        | NetworkCommand::SendHeartbeat(_)
        | NetworkCommand::SendChecksum(_)
        | NetworkCommand::SendGameOver(_)
        | NetworkCommand::SendBazaarBuy(_)
        | NetworkCommand::SendBazaarRemove(_)
        | NetworkCommand::SendBazaarDone { .. }
        | NetworkCommand::SubmitResult { .. } => state.clone(),
    }
}

/// Error raised by the deterministic network lockstep adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockstepError {
    /// A remote input arrived for a tick that has already been simulated.
    LateInput {
        /// Tick carried by the late input.
        tick: u64,
        /// Local tick that has already been simulated.
        current_tick: u64,
    },
    /// An input claimed the wrong player slot for this peer.
    WrongPlayer {
        /// Player slot expected from this peer.
        expected: PlayerSlot,
        /// Player slot carried by the input.
        actual: PlayerSlot,
    },
    /// A protocol input could not be mapped into a valid core action.
    InvalidInput(String),
    /// A Bazaar action could not be applied to the deterministic core state.
    Bazaar(String),
}

/// Deterministic fixed-step lockstep adapter for a connected network game.
#[derive(Debug, Clone)]
pub struct NetworkLockstep {
    local_slot: PlayerSlot,
    peer_slot: PlayerSlot,
    input_delay_ticks: u64,
    current_tick: u64,
    local_watermark: u64,
    advertised_local_watermark: Option<u64>,
    peer_watermark: Option<u64>,
    latest_local_input: Option<PlayerInput>,
    latest_remote_input: Option<PlayerInput>,
    inputs: BTreeMap<u64, ScheduledInputs>,
    pending_checksums: Vec<GameChecksum>,
    checksum_history: BTreeMap<u64, ChecksumState>,
}

impl NetworkLockstep {
    /// Creates a lockstep adapter using the default LAN-safe input delay.
    #[must_use]
    pub fn new(local_slot: PlayerSlot) -> Self {
        Self::with_input_delay(local_slot, DEFAULT_INPUT_DELAY_TICKS)
    }

    /// Creates a lockstep adapter with an explicit input delay in network ticks.
    #[must_use]
    pub fn with_input_delay(local_slot: PlayerSlot, input_delay_ticks: u64) -> Self {
        Self {
            local_slot,
            peer_slot: opposite_slot(local_slot),
            input_delay_ticks,
            current_tick: 0,
            local_watermark: 0,
            advertised_local_watermark: None,
            peer_watermark: None,
            latest_local_input: None,
            latest_remote_input: None,
            inputs: BTreeMap::new(),
            pending_checksums: Vec::new(),
            checksum_history: BTreeMap::new(),
        }
    }

    /// Returns the next deterministic tick to simulate.
    #[must_use]
    pub const fn current_tick(&self) -> u64 {
        self.current_tick
    }

    /// Returns the highest tick through which local input has been fully sent.
    #[must_use]
    pub const fn local_watermark(&self) -> u64 {
        self.local_watermark
    }

    /// Returns the most recent peer watermark, if one has been received.
    #[must_use]
    pub const fn peer_watermark(&self) -> Option<u64> {
        self.peer_watermark
    }

    /// Returns the latest locally scheduled input, if any.
    #[must_use]
    pub const fn latest_local_input(&self) -> Option<&PlayerInput> {
        self.latest_local_input.as_ref()
    }

    /// Returns the latest remotely received input, if any.
    #[must_use]
    pub const fn latest_remote_input(&self) -> Option<&PlayerInput> {
        self.latest_remote_input.as_ref()
    }

    /// Schedules a local input after the configured delay and returns the wire message.
    pub fn schedule_local_input(&mut self, command: InputCommand) -> PlayerInput {
        let mut tick = self.current_tick.saturating_add(self.input_delay_ticks);
        if let Some(advertised) = self.advertised_local_watermark {
            tick = tick.max(advertised.saturating_add(1));
        }
        let input = PlayerInput {
            player: self.local_slot,
            tick,
            command,
        };
        self.insert_input(input.clone());
        self.latest_local_input = Some(input.clone());
        input
    }

    /// Records that all local inputs through `through_tick` have been sent.
    pub fn mark_local_watermark(&mut self, through_tick: u64) -> TickWatermark {
        self.local_watermark = self.local_watermark.max(through_tick);
        self.advertised_local_watermark = Some(
            self.advertised_local_watermark
                .map_or(self.local_watermark, |advertised| {
                    advertised.max(self.local_watermark)
                }),
        );
        TickWatermark {
            player: self.local_slot,
            through_tick: self.local_watermark,
        }
    }

    /// Schedules an input received from the remote peer.
    pub fn receive_remote_input(&mut self, input: PlayerInput) -> Result<(), LockstepError> {
        if input.player != self.peer_slot {
            return Err(LockstepError::WrongPlayer {
                expected: self.peer_slot,
                actual: input.player,
            });
        }
        if input.tick < self.current_tick {
            return Err(LockstepError::LateInput {
                tick: input.tick,
                current_tick: self.current_tick,
            });
        }
        self.latest_remote_input = Some(input.clone());
        self.insert_input(input);
        Ok(())
    }

    /// Updates the peer input completeness watermark.
    pub fn receive_peer_watermark(
        &mut self,
        watermark: TickWatermark,
    ) -> Result<(), LockstepError> {
        if watermark.player != self.peer_slot {
            return Err(LockstepError::WrongPlayer {
                expected: self.peer_slot,
                actual: watermark.player,
            });
        }
        self.peer_watermark = Some(self.peer_watermark.unwrap_or(0).max(watermark.through_tick));
        Ok(())
    }

    /// Advances every tick covered by both local and peer watermarks.
    pub fn advance_ready(&mut self, game: &mut TwoPlayerGame) -> Result<u64, LockstepError> {
        let Some(peer_watermark) = self.peer_watermark else {
            return Ok(0);
        };
        let mut advanced = 0;
        while self.current_tick <= self.local_watermark && self.current_tick <= peer_watermark {
            self.advance_one(game)?;
            advanced += 1;
        }
        Ok(advanced)
    }

    /// Applies a Bazaar purchase intent through core APIs.
    pub fn apply_bazaar_buy(
        &self,
        game: &mut TwoPlayerGame,
        buy: BazaarBuy,
    ) -> Result<(), LockstepError> {
        game.bazaar_buy(core_player(buy.player), token_from_wire(buy.weapon)?)
            .map(|_| ())
            .map_err(format_bazaar_error)
    }

    /// Applies a Bazaar removal intent through core APIs.
    pub fn apply_bazaar_remove(
        &self,
        game: &mut TwoPlayerGame,
        remove: BazaarRemove,
    ) -> Result<(), LockstepError> {
        let token = game
            .bazaar_session(core_player(remove.player))
            .and_then(|session| {
                session.staged_arsenal().slots()[usize::from(remove.slot_index)]
                    .map(|slot| slot.token)
            })
            .ok_or_else(|| {
                LockstepError::Bazaar("no staged weapon in requested slot".to_string())
            })?;
        game.bazaar_remove_staged(core_player(remove.player), token)
            .map_err(format_bazaar_error)
    }

    /// Applies a Bazaar done intent through core APIs.
    pub fn apply_bazaar_done(
        &self,
        game: &mut TwoPlayerGame,
        done: BazaarDone,
    ) -> Vec<LoggedEvent> {
        game.bazaar_done(core_player(done.player))
    }

    /// Builds a checksum diagnostic for the current whole-game state.
    #[must_use]
    pub fn checksum_message(&self, game: &TwoPlayerGame) -> GameChecksum {
        GameChecksum {
            reporter: self.local_slot,
            tick: self.current_tick.saturating_sub(1),
            checksum: game.deterministic_checksum(),
            event_count: game.event_log().len() as u64,
        }
    }

    /// Receives a peer checksum, deferring it until the covered tick has completed locally.
    pub fn receive_checksum(
        &mut self,
        game: &TwoPlayerGame,
        checksum: GameChecksum,
    ) -> Option<DesyncReport> {
        if checksum.tick >= self.current_tick {
            self.pending_checksums.push(checksum);
            return None;
        }
        self.desync_report(game, &checksum)
    }

    /// Compares any deferred peer checksums whose covered ticks are now available locally.
    pub fn drain_pending_desync_reports(&mut self, game: &TwoPlayerGame) -> Vec<DesyncReport> {
        let mut pending = std::mem::take(&mut self.pending_checksums);
        let mut reports = Vec::new();
        for checksum in pending.drain(..) {
            if checksum.tick >= self.current_tick {
                self.pending_checksums.push(checksum);
                continue;
            }
            if let Some(report) = self.desync_report(game, &checksum) {
                reports.push(report);
            }
        }
        reports
    }

    /// Compares a peer checksum with local state and returns a full desync report on mismatch.
    #[must_use]
    pub fn desync_report(
        &self,
        game: &TwoPlayerGame,
        remote: &GameChecksum,
    ) -> Option<DesyncReport> {
        let local = self.checksum_state_for(remote.tick, game)?;
        let local_checksum = local.checksum;
        let local_event_count = local.event_count;
        if remote.checksum == local_checksum && remote.event_count == local_event_count {
            return None;
        }

        Some(DesyncReport {
            tick: remote.tick,
            local_slot: self.local_slot,
            latest_local_input: self.latest_local_input.clone(),
            latest_remote_input: self.latest_remote_input.clone(),
            local_watermark: self.local_watermark,
            peer_watermark: self.peer_watermark,
            local_checksum,
            remote_checksum: remote.checksum,
            local_event_count,
            remote_event_count: remote.event_count,
            score_snapshots: [
                score_snapshot(game, PlayerSlot::One),
                score_snapshot(game, PlayerSlot::Two),
            ],
            board_snapshots: [
                board_snapshot(game, PlayerSlot::One),
                board_snapshot(game, PlayerSlot::Two),
            ],
            arsenal_snapshots: [
                arsenal_snapshot(game, PlayerSlot::One),
                arsenal_snapshot(game, PlayerSlot::Two),
            ],
        })
    }

    fn insert_input(&mut self, input: PlayerInput) {
        let bucket = self.inputs.entry(input.tick).or_default();
        match input.player {
            PlayerSlot::One => bucket.player_one.push(input.command),
            PlayerSlot::Two => bucket.player_two.push(input.command),
        }
    }

    fn checksum_state_for(&self, tick: u64, game: &TwoPlayerGame) -> Option<ChecksumState> {
        if tick >= self.current_tick {
            return None;
        }
        self.checksum_history.get(&tick).copied().or_else(|| {
            (tick == self.current_tick.saturating_sub(1)).then(|| ChecksumState::from(game))
        })
    }

    fn record_checksum_state(&mut self, tick: u64, game: &TwoPlayerGame) {
        self.checksum_history
            .insert(tick, ChecksumState::from(game));
        while self.checksum_history.len() > CHECKSUM_HISTORY_TICKS {
            let Some(oldest) = self.checksum_history.keys().next().copied() else {
                break;
            };
            self.checksum_history.remove(&oldest);
        }
    }

    fn advance_one(&mut self, game: &mut TwoPlayerGame) -> Result<(), LockstepError> {
        let completed_tick = self.current_tick;
        let inputs = self.inputs.remove(&self.current_tick).unwrap_or_default();
        for command in inputs.player_one {
            apply_input_command(game, PlayerSlot::One, command)?;
        }
        for command in inputs.player_two {
            apply_input_command(game, PlayerSlot::Two, command)?;
        }
        let _ = game.tick_player(PlayerId::One, NETWORK_TICK_MS);
        let _ = game.tick_player(PlayerId::Two, NETWORK_TICK_MS);
        self.record_checksum_state(completed_tick, game);
        self.current_tick += 1;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct ChecksumState {
    checksum: u64,
    event_count: u64,
}

impl ChecksumState {
    fn from(game: &TwoPlayerGame) -> Self {
        Self {
            checksum: game.deterministic_checksum(),
            event_count: game.event_log().len() as u64,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ScheduledInputs {
    player_one: Vec<InputCommand>,
    player_two: Vec<InputCommand>,
}

fn apply_input_command(
    game: &mut TwoPlayerGame,
    player: PlayerSlot,
    command: InputCommand,
) -> Result<(), LockstepError> {
    let player = core_player(player);
    match command {
        InputCommand::MoveLeft => {
            let _ = game.command(player, Command::MoveLeft);
        }
        InputCommand::MoveRight => {
            let _ = game.command(player, Command::MoveRight);
        }
        InputCommand::RotateClockwise => {
            let _ = game.command(player, Command::RotateClockwise);
        }
        InputCommand::RotateCounterClockwise => {
            let _ = game.command(player, Command::RotateCounterClockwise);
        }
        InputCommand::StartFastDrop => {
            let _ = game.command(player, Command::StartFastDrop);
        }
        InputCommand::StopFastDrop => {
            let _ = game.command(player, Command::StopFastDrop);
        }
        InputCommand::LaunchWeapon { slot_index } => {
            let slot_label = if slot_index == 9 { 0 } else { slot_index + 1 };
            game.launch_weapon_slot(player, slot_label)
                .map_err(|error| {
                    LockstepError::InvalidInput(format!("weapon launch failed: {error:?}"))
                })?;
        }
    }
    Ok(())
}

fn token_from_wire(weapon: u8) -> Result<WeaponToken, LockstepError> {
    WeaponToken::from_legacy_id(weapon)
        .ok_or_else(|| LockstepError::InvalidInput(format!("unknown weapon token {weapon}")))
}

fn format_bazaar_error(error: ShoppingError) -> LockstepError {
    LockstepError::Bazaar(format!("bazaar action failed: {error:?}"))
}

fn score_snapshot(game: &TwoPlayerGame, player: PlayerSlot) -> ScoreSnapshot {
    let state = game.player(core_player(player));
    ScoreSnapshot {
        player,
        score: state.score(),
        funds: state.funds(),
        lines: state.lines(),
    }
}

fn board_snapshot(game: &TwoPlayerGame, player: PlayerSlot) -> WireBoardSnapshot {
    let snapshot = game.player(core_player(player)).board().snapshot();
    let cells = snapshot
        .cells
        .into_iter()
        .map(|cell| cell.map_or(0, battletris_core::cell::Cell::legacy_id))
        .collect();
    WireBoardSnapshot::new(
        player,
        0,
        snapshot.width.try_into().expect("board width fits in u16"),
        snapshot
            .height
            .try_into()
            .expect("board height fits in u16"),
        cells,
    )
    .expect("core board snapshot dimensions are valid for protocol")
}

fn arsenal_snapshot(game: &TwoPlayerGame, player: PlayerSlot) -> ArsenalSnapshot {
    let mut slots = [None; 10];
    for (index, slot) in game
        .player(core_player(player))
        .arsenal()
        .slots()
        .iter()
        .enumerate()
    {
        slots[index] = slot.map(|slot| ArsenalEntry {
            weapon: slot.token.legacy_id(),
            quantity: slot.quantity.min(u32::from(u16::MAX)) as u16,
        });
    }
    ArsenalSnapshot { player, slots }
}

const fn core_player(player: PlayerSlot) -> PlayerId {
    match player {
        PlayerSlot::One => PlayerId::One,
        PlayerSlot::Two => PlayerId::Two,
    }
}

const fn opposite_slot(player: PlayerSlot) -> PlayerSlot {
    match player {
        PlayerSlot::One => PlayerSlot::Two,
        PlayerSlot::Two => PlayerSlot::One,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use battletris_core::rng::GameSeed;
    use battletris_protocol::derive_player_seeds;
    use std::time::{Duration, Instant};

    fn identity(name: &str) -> PlayerIdentity {
        PlayerIdentity {
            display_name: name.to_string(),
        }
    }

    #[test]
    fn lifecycle_reducer_tracks_host_join_cancel_and_disconnect() {
        let host_addr = "127.0.0.1:4405".parse().unwrap();
        let join_addr = "127.0.0.1:4406".parse().unwrap();
        let state = NetworkLifecycleState::Idle;

        let state = reduce_state(
            &state,
            &NetworkCommand::Host {
                bind_addr: host_addr,
                identity: identity("Ada"),
            },
        );
        assert_eq!(
            state,
            NetworkLifecycleState::Hosting {
                bind_addr: host_addr
            }
        );

        let state = reduce_state(
            &state,
            &NetworkCommand::Join {
                peer_addr: join_addr,
                identity: identity("Ben"),
                challenge_text: "battle?".to_string(),
            },
        );
        assert_eq!(
            state,
            NetworkLifecycleState::Joining {
                peer_addr: join_addr
            }
        );

        let state = reduce_state(&state, &NetworkCommand::Cancel);
        assert_eq!(state, NetworkLifecycleState::Idle);

        let state = reduce_state(
            &state,
            &NetworkCommand::Disconnect {
                reason: "bye".to_string(),
            },
        );
        assert_eq!(state, NetworkLifecycleState::Disconnecting);
    }

    #[test]
    fn network_session_keeps_seed_ranked_and_hosted_metadata_explicit() {
        let direct = NetworkSession::direct(
            PlayerSlot::Two,
            identity("Ada"),
            StartGame {
                receiving_peer_slot: PlayerSlot::Two,
                seed: 99,
                ranked: false,
            },
        );
        assert_eq!(direct.mode, NetworkMode::Direct);
        assert_eq!(direct.base_seed, 99);
        assert_eq!(direct.final_result_status, FinalResultStatus::Unranked);

        let hosted = NetworkSession::hosted(
            PlayerSlot::One,
            identity("Ben"),
            HostedGameStart {
                session_id: HostedSessionId("session-1".to_string()),
                player_one: HostedPlayer {
                    player_id: "ada".to_string(),
                    display_name: "Ada".to_string(),
                },
                player_two: HostedPlayer {
                    player_id: "ben".to_string(),
                    display_name: "Ben".to_string(),
                },
                seed: 101,
                ranked: true,
                community_label: "garage".to_string(),
            },
        );
        assert_eq!(hosted.mode, NetworkMode::Hosted);
        assert_eq!(hosted.base_seed, 101);
        assert_eq!(hosted.community_label.as_deref(), Some("garage"));
        assert_eq!(hosted.final_result_status, FinalResultStatus::None);
    }

    #[test]
    fn bounded_channels_apply_backpressure() {
        let (mut channels, _io) = NetworkChannels::bounded(1);
        let sender = channels.command_sender();
        sender
            .try_send(NetworkCommand::Cancel)
            .expect("first command fits");
        assert!(sender.try_send(NetworkCommand::Cancel).is_err());
        assert!(matches!(
            channels.try_recv_event(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn wildcard_share_address_uses_reachable_loopback_placeholder() {
        let bind: SocketAddr = "0.0.0.0:4405".parse().unwrap();
        let share = share_addr_for(bind);

        assert_eq!(share.to_string(), "127.0.0.1:4405");
    }

    #[test]
    fn lan_availability_parses_known_states() {
        assert_eq!(
            parse_lan_availability(Some("available")),
            LanAvailability::Available
        );
        assert_eq!(parse_lan_availability(Some("busy")), LanAvailability::Busy);
        assert_eq!(
            parse_lan_availability(Some("other")),
            LanAvailability::Unknown
        );
        assert_eq!(parse_lan_availability(None), LanAvailability::Unknown);
    }

    #[test]
    fn lan_instance_name_is_nonempty_and_bounded() {
        assert_eq!(lan_instance_name("   "), "BattleTris Host");
        assert_eq!(lan_instance_name("Ada"), "Ada");
        assert_eq!(
            lan_instance_name("abcdefghijklmnopqrstuvwxyz1234567890").len(),
            30
        );
    }

    #[test]
    #[cfg(not(feature = "lan-discovery"))]
    fn lan_discovery_unavailable_when_feature_is_disabled() {
        let (mut channels, mut io) = NetworkChannels::bounded(4);
        let mut discovery = LanDiscoveryRuntime::default();
        let runtime = Runtime::new().expect("tokio runtime starts");

        runtime.block_on(discovery.start_advertising(
            identity("Ada"),
            "127.0.0.1:4405".parse().unwrap(),
            &mut io,
        ));

        assert!(matches!(
            channels.try_recv_event(),
            Ok(NetworkEvent::LanDiscoveryUnavailable { .. })
        ));
    }

    #[test]
    fn runtime_direct_host_join_accepts_loopback_challenge() {
        let mut host = NetworkRuntime::start();
        let mut joiner = NetworkRuntime::start();

        host.channels_mut()
            .command_sender()
            .try_send(NetworkCommand::Host {
                bind_addr: "127.0.0.1:0".parse().unwrap(),
                identity: identity("Ada"),
            })
            .unwrap();

        let listen = wait_for_event(host.channels_mut(), |event| match event {
            NetworkEvent::Listening { share_addr, .. } => Some(share_addr),
            _ => None,
        });

        joiner
            .channels_mut()
            .command_sender()
            .try_send(NetworkCommand::Join {
                peer_addr: listen,
                identity: identity("Ben"),
                challenge_text: "battle?".to_string(),
            })
            .unwrap();

        let challenge = wait_for_event(host.channels_mut(), |event| match event {
            NetworkEvent::IncomingChallenge { challenge } => Some(challenge),
            _ => None,
        });
        assert_eq!(challenge.challenger.display_name, "Ben");
        assert_eq!(challenge.message, "battle?");

        host.channels_mut()
            .command_sender()
            .try_send(NetworkCommand::Accept {
                seed: 123,
                ranked: false,
            })
            .unwrap();

        let host_session = wait_for_connected(host.channels_mut());
        let join_session = wait_for_connected(joiner.channels_mut());
        assert_eq!(host_session.local_slot, PlayerSlot::One);
        assert_eq!(join_session.local_slot, PlayerSlot::Two);
        assert_eq!(host_session.base_seed, 123);
        assert_eq!(join_session.base_seed, 123);

        host.shutdown();
        joiner.shutdown();
    }

    #[test]
    fn runtime_lockstep_peers_play_past_checksum_tick_without_desync() {
        let (mut host, mut joiner) = connected_lockstep_peers(123);

        for frame in 0..130 {
            if frame == 7 {
                send_scheduled_input(&mut host, InputCommand::MoveLeft);
            }
            if frame == 11 {
                send_scheduled_input(&mut joiner, InputCommand::RotateClockwise);
            }
            send_tick_watermark(&mut host);
            send_tick_watermark(&mut joiner);
            drain_runtime_pair(&mut host, &mut joiner);

            if frame % 13 == 0 {
                send_checksum(&mut host);
                send_checksum(&mut joiner);
                drain_runtime_pair(&mut host, &mut joiner);
            }
        }

        send_checksum(&mut host);
        send_checksum(&mut joiner);
        drain_runtime_pair(&mut host, &mut joiner);

        assert!(host.lockstep.current_tick() > 100);
        assert_eq!(host.lockstep.current_tick(), joiner.lockstep.current_tick());
        assert_eq!(
            host.game.deterministic_checksum(),
            joiner.game.deterministic_checksum()
        );

        host.runtime.shutdown();
        joiner.runtime.shutdown();
    }

    #[test]
    fn runtime_cancel_releases_direct_listener_port() {
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = reserved.local_addr().unwrap();
        drop(reserved);

        let mut host = NetworkRuntime::start();
        host.channels_mut()
            .command_sender()
            .try_send(NetworkCommand::Host {
                bind_addr: addr,
                identity: identity("Ada"),
            })
            .unwrap();

        let listening = wait_for_event(host.channels_mut(), |event| match event {
            NetworkEvent::Listening { bind_addr, .. } => Some(bind_addr),
            _ => None,
        });
        assert_eq!(listening, addr);

        host.channels_mut()
            .command_sender()
            .try_send(NetworkCommand::Cancel)
            .unwrap();
        wait_for_event(host.channels_mut(), |event| match event {
            NetworkEvent::StateChanged(NetworkLifecycleState::Idle) => Some(()),
            _ => None,
        });

        host.shutdown();
        std::net::TcpListener::bind(addr).expect("canceled host releases listener port");
    }

    #[test]
    fn stalled_sender_does_not_reuse_tick_after_peer_advances() {
        let mut sender = NetworkLockstep::with_input_delay(PlayerSlot::One, 2);
        let mut receiver = NetworkLockstep::with_input_delay(PlayerSlot::Two, 2);
        let mut receiver_game = TwoPlayerGame::new(GameSeed::from_u64(1), GameSeed::from_u64(2));

        let first = sender.schedule_local_input(InputCommand::MoveLeft);
        let sender_watermark = sender.mark_local_watermark(sender.current_tick());
        receiver.receive_remote_input(first).unwrap();
        receiver.receive_peer_watermark(sender_watermark).unwrap();
        receiver.mark_local_watermark(2);
        receiver.advance_ready(&mut receiver_game).unwrap();

        let second = sender.schedule_local_input(InputCommand::MoveRight);
        receiver
            .receive_remote_input(second)
            .expect("stalled sender input should not target an already simulated tick");
    }

    #[test]
    fn future_checksum_waits_until_local_tick_is_available() {
        let mut lockstep = NetworkLockstep::with_input_delay(PlayerSlot::One, 0);
        let mut game = TwoPlayerGame::new(GameSeed::from_u64(1), GameSeed::from_u64(2));
        let remote = GameChecksum {
            reporter: PlayerSlot::Two,
            tick: 1,
            checksum: 0,
            event_count: 0,
        };

        assert!(lockstep.receive_checksum(&game, remote).is_none());
        assert!(lockstep.drain_pending_desync_reports(&game).is_empty());

        lockstep.mark_local_watermark(1);
        lockstep
            .receive_peer_watermark(TickWatermark {
                player: PlayerSlot::Two,
                through_tick: 1,
            })
            .unwrap();
        lockstep.advance_ready(&mut game).unwrap();

        let reports = lockstep.drain_pending_desync_reports(&game);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].tick, 1);
    }

    #[test]
    fn late_checksum_uses_retained_tick_history() {
        let mut lockstep = NetworkLockstep::with_input_delay(PlayerSlot::One, 0);
        let mut game = TwoPlayerGame::new(GameSeed::from_u64(1), GameSeed::from_u64(2));
        lockstep.mark_local_watermark(1);
        lockstep
            .receive_peer_watermark(TickWatermark {
                player: PlayerSlot::Two,
                through_tick: 1,
            })
            .unwrap();
        lockstep.advance_ready(&mut game).unwrap();
        let old = lockstep.checksum_message(&game);

        let _ = lockstep.schedule_local_input(InputCommand::MoveLeft);
        lockstep.mark_local_watermark(lockstep.current_tick());
        lockstep
            .receive_peer_watermark(TickWatermark {
                player: PlayerSlot::Two,
                through_tick: lockstep.current_tick(),
            })
            .unwrap();
        lockstep.advance_ready(&mut game).unwrap();

        assert_ne!(old.checksum, game.deterministic_checksum());
        assert!(lockstep.receive_checksum(&game, old).is_none());
    }

    #[test]
    fn checksum_mismatch_builds_actionable_desync_report() {
        let mut lockstep = NetworkLockstep::with_input_delay(PlayerSlot::One, 2);
        let mut game = TwoPlayerGame::new(GameSeed::from_u64(1), GameSeed::from_u64(2));
        let local = lockstep.schedule_local_input(InputCommand::MoveLeft);
        let remote = PlayerInput {
            player: PlayerSlot::Two,
            tick: local.tick,
            command: InputCommand::MoveRight,
        };
        lockstep.receive_remote_input(remote.clone()).unwrap();
        lockstep.mark_local_watermark(local.tick);
        lockstep
            .receive_peer_watermark(TickWatermark {
                player: PlayerSlot::Two,
                through_tick: local.tick,
            })
            .unwrap();
        lockstep.advance_ready(&mut game).unwrap();

        let report = lockstep
            .desync_report(
                &game,
                &GameChecksum {
                    reporter: PlayerSlot::Two,
                    tick: local.tick,
                    checksum: game.deterministic_checksum() ^ 1,
                    event_count: game.event_log().len() as u64,
                },
            )
            .expect("checksum mismatch returns desync report");

        assert_eq!(report.tick, local.tick);
        assert_eq!(report.latest_local_input, Some(local));
        assert_eq!(report.latest_remote_input, Some(remote));
        assert_eq!(report.peer_watermark, Some(2));
        assert_eq!(report.score_snapshots[0].player, PlayerSlot::One);
        assert_eq!(report.board_snapshots[0].player, PlayerSlot::One);
        assert_eq!(report.arsenal_snapshots[1].player, PlayerSlot::Two);
    }

    struct LockstepPeer {
        runtime: NetworkRuntime,
        lockstep: NetworkLockstep,
        game: TwoPlayerGame,
    }

    fn connected_lockstep_peers(seed: u64) -> (LockstepPeer, LockstepPeer) {
        let mut host = NetworkRuntime::start();
        let mut joiner = NetworkRuntime::start();

        send_runtime_command(
            &mut host,
            NetworkCommand::Host {
                bind_addr: "127.0.0.1:0".parse().unwrap(),
                identity: identity("Ada"),
            },
        );
        let listen = wait_for_event(host.channels_mut(), |event| match event {
            NetworkEvent::Listening { share_addr, .. } => Some(share_addr),
            _ => None,
        });

        send_runtime_command(
            &mut joiner,
            NetworkCommand::Join {
                peer_addr: listen,
                identity: identity("Ben"),
                challenge_text: "battle?".to_string(),
            },
        );
        let _challenge = wait_for_event(host.channels_mut(), |event| match event {
            NetworkEvent::IncomingChallenge { challenge } => Some(challenge),
            _ => None,
        });
        send_runtime_command(
            &mut host,
            NetworkCommand::Accept {
                seed,
                ranked: false,
            },
        );

        let host_session = wait_for_connected(host.channels_mut());
        let join_session = wait_for_connected(joiner.channels_mut());
        let (player_one_seed, player_two_seed) = derive_player_seeds(seed);

        (
            LockstepPeer {
                runtime: host,
                lockstep: NetworkLockstep::new(host_session.local_slot),
                game: TwoPlayerGame::new(
                    GameSeed::from_u64(player_one_seed),
                    GameSeed::from_u64(player_two_seed),
                ),
            },
            LockstepPeer {
                runtime: joiner,
                lockstep: NetworkLockstep::new(join_session.local_slot),
                game: TwoPlayerGame::new(
                    GameSeed::from_u64(player_one_seed),
                    GameSeed::from_u64(player_two_seed),
                ),
            },
        )
    }

    fn send_scheduled_input(peer: &mut LockstepPeer, command: InputCommand) {
        let input = peer.lockstep.schedule_local_input(command);
        send_runtime_command(&mut peer.runtime, NetworkCommand::SendScheduledInput(input));
    }

    fn send_tick_watermark(peer: &mut LockstepPeer) {
        let watermark = peer
            .lockstep
            .mark_local_watermark(peer.lockstep.current_tick());
        send_runtime_command(
            &mut peer.runtime,
            NetworkCommand::SendTickWatermark(watermark),
        );
    }

    fn send_checksum(peer: &mut LockstepPeer) {
        send_runtime_command(
            &mut peer.runtime,
            NetworkCommand::SendChecksum(peer.lockstep.checksum_message(&peer.game)),
        );
    }

    fn send_runtime_command(runtime: &mut NetworkRuntime, command: NetworkCommand) {
        runtime
            .channels_mut()
            .command_sender()
            .try_send(command)
            .expect("runtime command is queued");
    }

    fn drain_runtime_pair(host: &mut LockstepPeer, joiner: &mut LockstepPeer) {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut idle_polls = 0;
        while idle_polls < 4 {
            assert!(Instant::now() < deadline, "timed out draining runtime pair");
            let mut progressed = false;
            progressed |= pump_peer_events(host);
            progressed |= pump_peer_events(joiner);
            progressed |= advance_peer(host);
            progressed |= advance_peer(joiner);

            if progressed {
                idle_polls = 0;
            } else {
                idle_polls += 1;
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }

    fn pump_peer_events(peer: &mut LockstepPeer) -> bool {
        let mut progressed = false;
        loop {
            match peer.runtime.channels_mut().try_recv_event() {
                Ok(event) => {
                    progressed = true;
                    apply_peer_event(peer, event);
                }
                Err(mpsc::error::TryRecvError::Empty) => return progressed,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    panic!("network event channel closed")
                }
            }
        }
    }

    fn apply_peer_event(peer: &mut LockstepPeer, event: NetworkEvent) {
        match event {
            NetworkEvent::InputReceived(input) => {
                peer.lockstep.receive_remote_input(input).unwrap();
            }
            NetworkEvent::TickWatermark(watermark) => {
                peer.lockstep.receive_peer_watermark(watermark).unwrap();
            }
            NetworkEvent::Heartbeat(heartbeat) => {
                peer.lockstep
                    .receive_peer_watermark(TickWatermark {
                        player: heartbeat.player,
                        through_tick: heartbeat.watermark_tick,
                    })
                    .unwrap();
            }
            NetworkEvent::Checksum(checksum) => {
                assert!(
                    peer.lockstep
                        .receive_checksum(&peer.game, checksum)
                        .is_none(),
                    "peer checksum should not desync"
                );
            }
            NetworkEvent::StateChanged(NetworkLifecycleState::Connected { .. })
            | NetworkEvent::Connected { .. } => {}
            NetworkEvent::StateChanged(state) => panic!("unexpected network state: {state:?}"),
            NetworkEvent::Error { message } => panic!("network error: {message}"),
            other => panic!("unexpected network event: {other:?}"),
        }
    }

    fn advance_peer(peer: &mut LockstepPeer) -> bool {
        let advanced = peer.lockstep.advance_ready(&mut peer.game).unwrap() > 0;
        let reports = peer.lockstep.drain_pending_desync_reports(&peer.game);
        assert!(reports.is_empty(), "unexpected desync reports: {reports:?}");
        advanced
    }

    fn wait_for_connected(channels: &mut NetworkChannels) -> NetworkSession {
        *wait_for_event(channels, |event| match event {
            NetworkEvent::Connected { session } => Some(session),
            _ => None,
        })
    }

    fn wait_for_event<T>(
        channels: &mut NetworkChannels,
        mut predicate: impl FnMut(NetworkEvent) -> Option<T>,
    ) -> T {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match channels.try_recv_event() {
                Ok(event) => {
                    if let Some(value) = predicate(event) {
                        return value;
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    assert!(
                        Instant::now() < deadline,
                        "timed out waiting for network event"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    panic!("network event channel closed")
                }
            }
        }
    }
}
