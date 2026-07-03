//! Network protocol types and serialization boundaries.
//!
//! This crate will define versioned wire messages, fixed-width framing,
//! challenge/start/play/bazaar/game-over flows, and compatibility tests derived
//! from the legacy protocol. It must keep wire messages separate from local core
//! events so transports can change without changing gameplay rules.

use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io, net::SocketAddr};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

/// Fixed frame magic used by every BattleTris rewrite protocol frame.
pub const MAGIC: [u8; 4] = *b"BTRS";
/// Current supported protocol major version.
pub const PROTOCOL_MAJOR: u16 = 1;
/// Current supported protocol minor version.
pub const PROTOCOL_MINOR: u16 = 0;
/// Header size in bytes: magic, version, kind, flags, and payload length.
pub const HEADER_LEN: usize = 16;
/// Conservative maximum postcard payload accepted before allocation.
pub const MAX_PAYLOAD_LEN: u32 = 64 * 1024;

/// Per-frame flags. No flags are currently assigned.
pub const FLAG_NONE: u16 = 0;

/// DNS-SD service name advertised by best-effort LAN discovery adapters.
pub const LAN_DISCOVERY_SERVICE: &str = "_battletris._tcp.local.";

/// Protocol capability token for the required direct TCP transport.
pub const CAPABILITY_DIRECT_TCP: &str = "direct-tcp";

/// Protocol capability token for optional LAN discovery.
pub const CAPABILITY_LAN_DISCOVERY: &str = "lan-discovery";

/// Protocol capability token for self-hosted lobby/ranked authority.
pub const CAPABILITY_SELF_HOSTED_LOBBY: &str = "self-hosted-lobby";

/// A protocol frame header encoded with fixed-width big-endian fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Protocol major version carried by this frame.
    pub major: u16,
    /// Protocol minor version carried by this frame.
    pub minor: u16,
    /// Public message kind discriminant.
    pub kind: u16,
    /// Reserved per-frame flags.
    pub flags: u16,
    /// Payload length in bytes.
    pub payload_len: u32,
}

impl FrameHeader {
    /// Creates a current-version header for a message kind and payload length.
    #[must_use]
    pub const fn new(kind: MessageKind, payload_len: u32) -> Self {
        Self {
            major: PROTOCOL_MAJOR,
            minor: PROTOCOL_MINOR,
            kind: kind as u16,
            flags: FLAG_NONE,
            payload_len,
        }
    }
}

/// Stable public message kinds encoded in the frame header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MessageKind {
    /// Initial peer greeting and version/capability advertisement.
    Hello = 1,
    /// Challenge request before accepting a match.
    Challenge = 2,
    /// Challenge acceptance.
    ChallengeAccepted = 3,
    /// Challenge denial.
    ChallengeDenied = 4,
    /// Deterministic match start parameters.
    StartGame = 5,
    /// Player input for a deterministic game tick.
    PlayerInput = 6,
    /// Full score/funds/line snapshot.
    ScoreSnapshot = 7,
    /// Full board snapshot.
    BoardSnapshot = 8,
    /// Full arsenal snapshot.
    ArsenalSnapshot = 9,
    /// Arsenal launch notification.
    WeaponLaunch = 10,
    /// Timed weapon activation notification.
    WeaponActive = 11,
    /// Timed weapon expiration notification.
    WeaponExpired = 12,
    /// Player finished bazaar shopping.
    BazaarDone = 13,
    /// Bazaar state snapshot.
    BazaarState = 14,
    /// Final game-over result.
    GameOver = 15,
    /// Pause or resume notification.
    Pause = 16,
    /// Graceful disconnect notification.
    Disconnect = 17,
    /// Register a hosted/self-hosted lobby presence entry.
    LobbyRegister = 18,
    /// Request the hosted/self-hosted lobby list.
    LobbyListRequest = 19,
    /// Hosted/self-hosted lobby list response.
    LobbyList = 20,
    /// Server-issued deterministic hosted game start.
    HostedGameStart = 21,
    /// Participant claim for a hosted ranked result.
    RankedResultClaim = 22,
    /// Server accepted and recorded a ranked result.
    RankedResultAccepted = 23,
    /// Server rejected a ranked result claim.
    RankedResultRejected = 24,
}

impl MessageKind {
    /// Converts a raw frame discriminant to a known message kind.
    #[must_use]
    pub const fn from_u16(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::Hello),
            2 => Some(Self::Challenge),
            3 => Some(Self::ChallengeAccepted),
            4 => Some(Self::ChallengeDenied),
            5 => Some(Self::StartGame),
            6 => Some(Self::PlayerInput),
            7 => Some(Self::ScoreSnapshot),
            8 => Some(Self::BoardSnapshot),
            9 => Some(Self::ArsenalSnapshot),
            10 => Some(Self::WeaponLaunch),
            11 => Some(Self::WeaponActive),
            12 => Some(Self::WeaponExpired),
            13 => Some(Self::BazaarDone),
            14 => Some(Self::BazaarState),
            15 => Some(Self::GameOver),
            16 => Some(Self::Pause),
            17 => Some(Self::Disconnect),
            18 => Some(Self::LobbyRegister),
            19 => Some(Self::LobbyListRequest),
            20 => Some(Self::LobbyList),
            21 => Some(Self::HostedGameStart),
            22 => Some(Self::RankedResultClaim),
            23 => Some(Self::RankedResultAccepted),
            24 => Some(Self::RankedResultRejected),
            _ => None,
        }
    }
}

/// Stable peer slot used by protocol messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerSlot {
    /// First peer/player slot.
    One,
    /// Second peer/player slot.
    Two,
}

/// Protocol-owned piece of player identity for friendly direct-connect games.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerIdentity {
    /// User-facing name displayed to the peer.
    pub display_name: String,
}

/// Server-scoped player identity used by hosted/self-hosted communities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedPlayer {
    /// Stable player id within the hosting server/community.
    pub player_id: String,
    /// User-facing display name.
    pub display_name: String,
}

/// Server-issued hosted game session id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HostedSessionId(pub String);

/// Metadata advertised for best-effort LAN discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanAdvertisement {
    /// DNS-SD service name.
    pub service: &'static str,
    /// TCP port that accepts the same direct protocol as manual connect.
    pub port: u16,
    /// TXT-style key/value metadata.
    pub txt: BTreeMap<String, String>,
}

impl LanAdvertisement {
    /// Builds TXT metadata for a locally hosted direct-connect game.
    #[must_use]
    pub fn available(identity: &PlayerIdentity, port: u16) -> Self {
        let mut txt = BTreeMap::new();
        txt.insert("protocol_major".to_string(), PROTOCOL_MAJOR.to_string());
        txt.insert("protocol_minor".to_string(), PROTOCOL_MINOR.to_string());
        txt.insert("display_name".to_string(), identity.display_name.clone());
        txt.insert("port".to_string(), port.to_string());
        txt.insert("state".to_string(), "available".to_string());

        Self {
            service: LAN_DISCOVERY_SERVICE,
            port,
            txt,
        }
    }
}

/// Initial peer greeting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    /// Highest protocol major version supported by the sender.
    pub major: u16,
    /// Highest protocol minor version supported by the sender.
    pub minor: u16,
    /// Friendly local identity.
    pub identity: PlayerIdentity,
    /// Capability tokens understood by the sender.
    pub capabilities: Vec<String>,
}

/// A direct challenge request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Challenge {
    /// Challenging player identity.
    pub challenger: PlayerIdentity,
    /// Optional short challenge text.
    pub message: String,
}

/// Acceptance of a challenge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChallengeAccepted {
    /// Accepting player identity.
    pub accepter: PlayerIdentity,
}

/// Denial of a challenge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChallengeDenied {
    /// User-facing denial reason.
    pub reason: String,
}

/// Deterministic match start parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartGame {
    /// Local slot assigned to the receiving peer.
    pub receiving_peer_slot: PlayerSlot,
    /// Deterministic game seed shared by both peers.
    pub seed: u64,
    /// Whether this direct game may submit ranked results later.
    pub ranked: bool,
}

/// Input command token carried over the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputCommand {
    /// Move the active piece left.
    MoveLeft,
    /// Move the active piece right.
    MoveRight,
    /// Rotate the active piece clockwise.
    RotateClockwise,
    /// Rotate the active piece counter-clockwise.
    RotateCounterClockwise,
    /// Start fast drop.
    StartFastDrop,
    /// Stop fast drop.
    StopFastDrop,
    /// Launch an arsenal slot. Slot labels use `1..9,0`; normalized index is `0..9`.
    LaunchWeapon {
        /// Normalized arsenal slot index `0..9`.
        slot_index: u8,
    },
}

/// One player input at a deterministic simulation tick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerInput {
    /// Player that issued the input.
    pub player: PlayerSlot,
    /// Deterministic tick number.
    pub tick: u64,
    /// Input command.
    pub command: InputCommand,
}

/// Full score, funds, and line-count snapshot for one player.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreSnapshot {
    /// Player that owns this score.
    pub player: PlayerSlot,
    /// Display score.
    pub score: i32,
    /// Spendable funds. Negative values are legal for legacy Reagan behavior.
    pub funds: i32,
    /// Total lines cleared by this player.
    pub lines: u32,
}

/// Full row-major board snapshot using protocol-owned legacy-compatible cell IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardSnapshot {
    /// Player that owns this board.
    pub player: PlayerSlot,
    /// Legacy motivation field preserved for swap/recon adapters.
    pub motivation: i32,
    /// Board width in cells.
    pub width: u16,
    /// Board height in cells.
    pub height: u16,
    /// Row-major signed cell IDs.
    pub cells: Vec<i16>,
}

impl BoardSnapshot {
    /// Creates a board snapshot after validating the row-major cell count.
    pub fn new(
        player: PlayerSlot,
        motivation: i32,
        width: u16,
        height: u16,
        cells: Vec<i16>,
    ) -> Result<Self, ProtocolError> {
        let expected = usize::from(width) * usize::from(height);
        if cells.len() != expected {
            return Err(ProtocolError::InvalidSnapshotCellCount {
                expected,
                actual: cells.len(),
            });
        }

        Ok(Self {
            player,
            motivation,
            width,
            height,
            cells,
        })
    }
}

/// One arsenal slot entry in a full arsenal snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArsenalEntry {
    /// Stable legacy weapon token ID.
    pub weapon: u8,
    /// Quantity stacked in the slot.
    pub quantity: u16,
}

/// Full ten-slot arsenal snapshot. `None` entries preserve holes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArsenalSnapshot {
    /// Player that owns this arsenal.
    pub player: PlayerSlot,
    /// Ten entries preserving slot order and holes.
    pub slots: [Option<ArsenalEntry>; 10],
}

/// Arsenal launch notification after local slot consumption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponLaunch {
    /// Launching player.
    pub launcher: PlayerSlot,
    /// Target player.
    pub target: PlayerSlot,
    /// Stable legacy weapon token ID.
    pub weapon: u8,
    /// Consumed arsenal slot index `0..9`.
    pub slot_index: u8,
    /// Deterministic launch sequence number.
    pub sequence: u64,
}

/// Timed weapon activation snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponActive {
    /// Player affected by the weapon.
    pub target: PlayerSlot,
    /// Stable legacy weapon token ID.
    pub weapon: u8,
    /// Remaining target-player line clears after stacking.
    pub remaining_lines: u32,
}

/// Timed weapon expiration notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponExpired {
    /// Player affected by the weapon.
    pub target: PlayerSlot,
    /// Stable legacy weapon token ID.
    pub weapon: u8,
}

/// Player finished bazaar shopping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BazaarDone {
    /// Player that clicked Done.
    pub player: PlayerSlot,
}

/// Bazaar progress snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BazaarState {
    /// Whether player one is done shopping.
    pub player_one_done: bool,
    /// Whether player two is done shopping.
    pub player_two_done: bool,
}

/// Final game-over result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameOver {
    /// Winning player.
    pub winner: PlayerSlot,
    /// Losing player.
    pub loser: PlayerSlot,
    /// Deterministic tick or event sequence where the game ended.
    pub sequence: u64,
}

/// Pause state notification. Legacy had a toggle token; the rewrite sends state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pause {
    /// `true` when paused, `false` when resumed.
    pub paused: bool,
}

/// Graceful disconnect notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disconnect {
    /// User-facing reason.
    pub reason: String,
}

/// Registers a player as available in a self-hosted lobby.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyRegister {
    /// Hosted player identity.
    pub player: HostedPlayer,
    /// Direct TCP address peers should use for gameplay transport.
    pub direct_addr: String,
    /// Whether the advertised game requests ranked server verification.
    pub ranked: bool,
}

/// Requests currently available lobby entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyListRequest {
    /// If true, omit unranked entries.
    pub ranked_only: bool,
}

/// One hosted lobby listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyEntry {
    /// Server-issued session id.
    pub session_id: HostedSessionId,
    /// Hosting player.
    pub host: HostedPlayer,
    /// Direct TCP address peers should use for gameplay transport.
    pub direct_addr: String,
    /// Whether this entry is eligible for ranked result verification.
    pub ranked: bool,
    /// Host protocol major version admitted by the server.
    pub protocol_major: u16,
    /// Host protocol minor version admitted by the server.
    pub protocol_minor: u16,
}

/// Hosted lobby list response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyList {
    /// Available sessions.
    pub entries: Vec<LobbyEntry>,
}

/// Server-issued deterministic hosted game start parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedGameStart {
    /// Server-issued session id.
    pub session_id: HostedSessionId,
    /// Player one identity.
    pub player_one: HostedPlayer,
    /// Player two identity.
    pub player_two: HostedPlayer,
    /// Deterministic seed assigned by the hosting server.
    pub seed: u64,
    /// Whether matching result claims may update server records.
    pub ranked: bool,
    /// Server/community ranking label.
    pub community_label: String,
}

/// Participant claim for a completed hosted ranked game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankedResultClaim {
    /// Server-issued session id.
    pub session_id: HostedSessionId,
    /// Player id of the client submitting the claim.
    pub reporter_player_id: String,
    /// Winning player id.
    pub winner_player_id: String,
    /// Losing player id.
    pub loser_player_id: String,
    /// Winner final score.
    pub winner_score: u64,
    /// Winner final line count.
    pub winner_lines: u64,
    /// Winner final funds.
    pub winner_funds: i64,
    /// Loser final score.
    pub loser_score: u64,
    /// Loser final line count.
    pub loser_lines: u64,
    /// Loser final funds.
    pub loser_funds: i64,
    /// Game duration in seconds.
    pub duration_secs: u64,
    /// Deterministic event count observed by the reporter.
    pub event_count: u64,
}

/// Server accepted and recorded a hosted ranked result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankedResultAccepted {
    /// Recorded session id.
    pub session_id: HostedSessionId,
}

/// Server rejected a hosted ranked result claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankedResultRejected {
    /// Rejected session id, if the claim included a known shape.
    pub session_id: Option<HostedSessionId>,
    /// User-facing rejection reason.
    pub reason: String,
}

/// Every known public wire message. These are intentionally distinct from local core events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireMessage {
    /// Initial peer greeting.
    Hello(Hello),
    /// Challenge request.
    Challenge(Challenge),
    /// Challenge acceptance.
    ChallengeAccepted(ChallengeAccepted),
    /// Challenge denial.
    ChallengeDenied(ChallengeDenied),
    /// Match start parameters.
    StartGame(StartGame),
    /// Player input at a deterministic tick.
    PlayerInput(PlayerInput),
    /// Score/funds/line snapshot.
    ScoreSnapshot(ScoreSnapshot),
    /// Board snapshot.
    BoardSnapshot(BoardSnapshot),
    /// Arsenal snapshot.
    ArsenalSnapshot(ArsenalSnapshot),
    /// Weapon launch notification.
    WeaponLaunch(WeaponLaunch),
    /// Timed weapon activation snapshot.
    WeaponActive(WeaponActive),
    /// Timed weapon expiration notification.
    WeaponExpired(WeaponExpired),
    /// Bazaar done notification.
    BazaarDone(BazaarDone),
    /// Bazaar progress snapshot.
    BazaarState(BazaarState),
    /// Game-over result.
    GameOver(GameOver),
    /// Pause/resume state.
    Pause(Pause),
    /// Graceful disconnect.
    Disconnect(Disconnect),
    /// Hosted/self-hosted lobby registration.
    LobbyRegister(LobbyRegister),
    /// Hosted/self-hosted lobby list request.
    LobbyListRequest(LobbyListRequest),
    /// Hosted/self-hosted lobby list response.
    LobbyList(LobbyList),
    /// Server-issued hosted game start.
    HostedGameStart(HostedGameStart),
    /// Participant ranked result claim.
    RankedResultClaim(RankedResultClaim),
    /// Server accepted and recorded a ranked result.
    RankedResultAccepted(RankedResultAccepted),
    /// Server rejected a ranked result claim.
    RankedResultRejected(RankedResultRejected),
}

impl WireMessage {
    /// Returns this message's public frame kind.
    #[must_use]
    pub const fn kind(&self) -> MessageKind {
        match self {
            Self::Hello(_) => MessageKind::Hello,
            Self::Challenge(_) => MessageKind::Challenge,
            Self::ChallengeAccepted(_) => MessageKind::ChallengeAccepted,
            Self::ChallengeDenied(_) => MessageKind::ChallengeDenied,
            Self::StartGame(_) => MessageKind::StartGame,
            Self::PlayerInput(_) => MessageKind::PlayerInput,
            Self::ScoreSnapshot(_) => MessageKind::ScoreSnapshot,
            Self::BoardSnapshot(_) => MessageKind::BoardSnapshot,
            Self::ArsenalSnapshot(_) => MessageKind::ArsenalSnapshot,
            Self::WeaponLaunch(_) => MessageKind::WeaponLaunch,
            Self::WeaponActive(_) => MessageKind::WeaponActive,
            Self::WeaponExpired(_) => MessageKind::WeaponExpired,
            Self::BazaarDone(_) => MessageKind::BazaarDone,
            Self::BazaarState(_) => MessageKind::BazaarState,
            Self::GameOver(_) => MessageKind::GameOver,
            Self::Pause(_) => MessageKind::Pause,
            Self::Disconnect(_) => MessageKind::Disconnect,
            Self::LobbyRegister(_) => MessageKind::LobbyRegister,
            Self::LobbyListRequest(_) => MessageKind::LobbyListRequest,
            Self::LobbyList(_) => MessageKind::LobbyList,
            Self::HostedGameStart(_) => MessageKind::HostedGameStart,
            Self::RankedResultClaim(_) => MessageKind::RankedResultClaim,
            Self::RankedResultAccepted(_) => MessageKind::RankedResultAccepted,
            Self::RankedResultRejected(_) => MessageKind::RankedResultRejected,
        }
    }
}

/// A raw decoded frame whose kind may be unknown to this crate version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFrame {
    /// Validated frame header.
    pub header: FrameHeader,
    /// Raw payload bytes.
    pub payload: Vec<u8>,
}

/// Protocol encoding and decoding failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// Frame is shorter than the fixed header.
    FrameTooShort {
        /// Actual frame bytes.
        actual: usize,
    },
    /// Frame magic did not match [`MAGIC`].
    BadMagic {
        /// Magic read from the frame.
        actual: [u8; 4],
    },
    /// Header major version is unsupported.
    UnsupportedMajor {
        /// Major version found in the frame.
        major: u16,
    },
    /// Payload length exceeds [`MAX_PAYLOAD_LEN`].
    PayloadTooLarge {
        /// Payload length found in the header.
        len: u32,
    },
    /// Frame byte count does not match the advertised payload length.
    LengthMismatch {
        /// Required full frame length.
        expected: usize,
        /// Actual full frame length.
        actual: usize,
    },
    /// Message kind is not known to this protocol version.
    UnknownKind {
        /// Raw message kind.
        kind: u16,
    },
    /// Postcard payload serialization failed.
    Encode(String),
    /// Postcard payload deserialization failed.
    Decode(String),
    /// Board snapshot cell count did not match width times height.
    InvalidSnapshotCellCount {
        /// Expected cell count.
        expected: usize,
        /// Actual cell count.
        actual: usize,
    },
    /// Transport I/O failed while reading or writing a frame.
    Io(String),
    /// A peer sent an unexpected message for the current session step.
    UnexpectedMessage {
        /// Expected message kind or flow state.
        expected: &'static str,
        /// Actual message kind received.
        actual: MessageKind,
    },
    /// The remote peer does not support this protocol major version.
    IncompatiblePeerVersion {
        /// Peer-advertised major version.
        major: u16,
        /// Peer-advertised minor version.
        minor: u16,
    },
}

impl From<io::Error> for ProtocolError {
    fn from(value: io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

/// A direct TCP protocol connection.
#[derive(Debug)]
pub struct DirectConnection {
    stream: TcpStream,
}

impl DirectConnection {
    /// Opens a direct TCP connection to a peer.
    pub async fn connect(addr: SocketAddr) -> Result<Self, ProtocolError> {
        Ok(Self {
            stream: TcpStream::connect(addr).await?,
        })
    }

    /// Wraps an accepted TCP stream.
    #[must_use]
    pub const fn from_stream(stream: TcpStream) -> Self {
        Self { stream }
    }

    /// Sends one framed wire message.
    pub async fn send(&mut self, message: &WireMessage) -> Result<(), ProtocolError> {
        write_message(&mut self.stream, message).await
    }

    /// Receives one framed wire message.
    pub async fn recv(&mut self) -> Result<WireMessage, ProtocolError> {
        read_message(&mut self.stream).await
    }
}

/// Result of accepting a direct challenge and sending deterministic start data.
#[derive(Debug)]
pub struct AcceptedDirectGame {
    /// Established direct protocol connection.
    pub connection: DirectConnection,
    /// Remote peer identity from its hello message.
    pub remote_identity: PlayerIdentity,
    /// Remote challenge request.
    pub challenge: Challenge,
}

/// Result of joining a direct challenge.
#[derive(Debug)]
pub struct JoinedDirectGame {
    /// Established direct protocol connection.
    pub connection: DirectConnection,
    /// Remote peer identity from its hello message.
    pub remote_identity: PlayerIdentity,
    /// Deterministic start parameters assigned by the host.
    pub start: StartGame,
}

/// Accepts one direct TCP peer and performs hello/challenge/accept/start host flow.
pub async fn accept_direct_game(
    listener: &TcpListener,
    host_identity: PlayerIdentity,
    seed: u64,
    ranked: bool,
) -> Result<AcceptedDirectGame, ProtocolError> {
    let (stream, _) = listener.accept().await?;
    let mut connection = DirectConnection::from_stream(stream);

    connection.send(&hello_for(host_identity.clone())).await?;
    let remote_identity = expect_hello(connection.recv().await?)?;
    let challenge = match connection.recv().await? {
        WireMessage::Challenge(challenge) => challenge,
        message => {
            return Err(ProtocolError::UnexpectedMessage {
                expected: "challenge",
                actual: message.kind(),
            });
        }
    };

    connection
        .send(&WireMessage::ChallengeAccepted(ChallengeAccepted {
            accepter: host_identity,
        }))
        .await?;
    connection
        .send(&WireMessage::StartGame(StartGame {
            receiving_peer_slot: PlayerSlot::Two,
            seed,
            ranked,
        }))
        .await?;

    Ok(AcceptedDirectGame {
        connection,
        remote_identity,
        challenge,
    })
}

/// Connects to a direct TCP peer and performs hello/challenge/start join flow.
pub async fn join_direct_game(
    addr: SocketAddr,
    identity: PlayerIdentity,
    challenge_text: String,
) -> Result<JoinedDirectGame, ProtocolError> {
    let mut connection = DirectConnection::connect(addr).await?;

    connection.send(&hello_for(identity.clone())).await?;
    let remote_identity = expect_hello(connection.recv().await?)?;
    connection
        .send(&WireMessage::Challenge(Challenge {
            challenger: identity,
            message: challenge_text,
        }))
        .await?;

    match connection.recv().await? {
        WireMessage::ChallengeAccepted(_) => {}
        WireMessage::ChallengeDenied(_denied) => {
            return Err(ProtocolError::UnexpectedMessage {
                expected: "challenge accepted",
                actual: MessageKind::ChallengeDenied,
            });
        }
        message => {
            return Err(ProtocolError::UnexpectedMessage {
                expected: "challenge accepted",
                actual: message.kind(),
            });
        }
    }

    let start = match connection.recv().await? {
        WireMessage::StartGame(start) => start,
        message => {
            return Err(ProtocolError::UnexpectedMessage {
                expected: "start game",
                actual: message.kind(),
            });
        }
    };

    Ok(JoinedDirectGame {
        connection,
        remote_identity,
        start,
    })
}

/// Reads a single framed wire message from an async stream.
pub async fn read_message<R>(reader: &mut R) -> Result<WireMessage, ProtocolError>
where
    R: AsyncRead + Unpin,
{
    let mut header_bytes = [0_u8; HEADER_LEN];
    reader.read_exact(&mut header_bytes).await?;

    let raw_header = decode_raw_frame_header(&header_bytes)?;
    let mut frame = Vec::with_capacity(HEADER_LEN + raw_header.payload_len as usize);
    frame.extend_from_slice(&header_bytes);
    frame.resize(HEADER_LEN + raw_header.payload_len as usize, 0);
    reader.read_exact(&mut frame[HEADER_LEN..]).await?;
    decode_message(&frame)
}

/// Writes a single framed wire message to an async stream.
pub async fn write_message<W>(writer: &mut W, message: &WireMessage) -> Result<(), ProtocolError>
where
    W: AsyncWrite + Unpin,
{
    let frame = encode_message(message)?;
    writer.write_all(&frame).await?;
    writer.flush().await?;
    Ok(())
}

fn hello_for(identity: PlayerIdentity) -> WireMessage {
    WireMessage::Hello(Hello {
        major: PROTOCOL_MAJOR,
        minor: PROTOCOL_MINOR,
        identity,
        capabilities: vec![
            CAPABILITY_DIRECT_TCP.to_string(),
            CAPABILITY_LAN_DISCOVERY.to_string(),
            CAPABILITY_SELF_HOSTED_LOBBY.to_string(),
        ],
    })
}

fn expect_hello(message: WireMessage) -> Result<PlayerIdentity, ProtocolError> {
    match message {
        WireMessage::Hello(hello) => {
            if hello.major != PROTOCOL_MAJOR {
                return Err(ProtocolError::IncompatiblePeerVersion {
                    major: hello.major,
                    minor: hello.minor,
                });
            }
            Ok(hello.identity)
        }
        message => Err(ProtocolError::UnexpectedMessage {
            expected: "hello",
            actual: message.kind(),
        }),
    }
}

/// Encodes one known wire message into a complete frame.
pub fn encode_message(message: &WireMessage) -> Result<Vec<u8>, ProtocolError> {
    let payload = encode_payload(message)?;
    if payload.len() > MAX_PAYLOAD_LEN as usize {
        return Err(ProtocolError::PayloadTooLarge {
            len: payload.len() as u32,
        });
    }

    let header = FrameHeader::new(message.kind(), payload.len() as u32);
    let mut frame = BytesMut::with_capacity(HEADER_LEN + payload.len());
    put_header(&mut frame, header);
    frame.extend_from_slice(&payload);
    Ok(frame.to_vec())
}

/// Decodes a complete frame into a known wire message.
pub fn decode_message(frame: &[u8]) -> Result<WireMessage, ProtocolError> {
    let raw = decode_raw_frame(frame)?;
    let kind = MessageKind::from_u16(raw.header.kind).ok_or(ProtocolError::UnknownKind {
        kind: raw.header.kind,
    })?;
    decode_payload(kind, &raw.payload)
}

/// Decodes and validates a frame envelope without requiring the message kind to be known.
pub fn decode_raw_frame(frame: &[u8]) -> Result<RawFrame, ProtocolError> {
    if frame.len() < HEADER_LEN {
        return Err(ProtocolError::FrameTooShort {
            actual: frame.len(),
        });
    }

    let header = decode_raw_frame_header(&frame[..HEADER_LEN])?;

    let expected = HEADER_LEN + header.payload_len as usize;
    if frame.len() != expected {
        return Err(ProtocolError::LengthMismatch {
            expected,
            actual: frame.len(),
        });
    }

    Ok(RawFrame {
        header,
        payload: frame[HEADER_LEN..].to_vec(),
    })
}

fn put_header(frame: &mut BytesMut, header: FrameHeader) {
    frame.extend_from_slice(&MAGIC);
    frame.put_u16(header.major);
    frame.put_u16(header.minor);
    frame.put_u16(header.kind);
    frame.put_u16(header.flags);
    frame.put_u32(header.payload_len);
}

fn encode_payload(message: &WireMessage) -> Result<Vec<u8>, ProtocolError> {
    match message {
        WireMessage::Hello(value) => to_stdvec(value),
        WireMessage::Challenge(value) => to_stdvec(value),
        WireMessage::ChallengeAccepted(value) => to_stdvec(value),
        WireMessage::ChallengeDenied(value) => to_stdvec(value),
        WireMessage::StartGame(value) => to_stdvec(value),
        WireMessage::PlayerInput(value) => to_stdvec(value),
        WireMessage::ScoreSnapshot(value) => to_stdvec(value),
        WireMessage::BoardSnapshot(value) => to_stdvec(value),
        WireMessage::ArsenalSnapshot(value) => to_stdvec(value),
        WireMessage::WeaponLaunch(value) => to_stdvec(value),
        WireMessage::WeaponActive(value) => to_stdvec(value),
        WireMessage::WeaponExpired(value) => to_stdvec(value),
        WireMessage::BazaarDone(value) => to_stdvec(value),
        WireMessage::BazaarState(value) => to_stdvec(value),
        WireMessage::GameOver(value) => to_stdvec(value),
        WireMessage::Pause(value) => to_stdvec(value),
        WireMessage::Disconnect(value) => to_stdvec(value),
        WireMessage::LobbyRegister(value) => to_stdvec(value),
        WireMessage::LobbyListRequest(value) => to_stdvec(value),
        WireMessage::LobbyList(value) => to_stdvec(value),
        WireMessage::HostedGameStart(value) => to_stdvec(value),
        WireMessage::RankedResultClaim(value) => to_stdvec(value),
        WireMessage::RankedResultAccepted(value) => to_stdvec(value),
        WireMessage::RankedResultRejected(value) => to_stdvec(value),
    }
}

fn decode_raw_frame_header(header: &[u8]) -> Result<FrameHeader, ProtocolError> {
    if header.len() < HEADER_LEN {
        return Err(ProtocolError::FrameTooShort {
            actual: header.len(),
        });
    }

    let magic: [u8; 4] = header[0..4].try_into().expect("slice length checked");
    if magic != MAGIC {
        return Err(ProtocolError::BadMagic { actual: magic });
    }

    let header = FrameHeader {
        major: u16::from_be_bytes(header[4..6].try_into().expect("slice length checked")),
        minor: u16::from_be_bytes(header[6..8].try_into().expect("slice length checked")),
        kind: u16::from_be_bytes(header[8..10].try_into().expect("slice length checked")),
        flags: u16::from_be_bytes(header[10..12].try_into().expect("slice length checked")),
        payload_len: u32::from_be_bytes(header[12..16].try_into().expect("slice length checked")),
    };

    if header.major != PROTOCOL_MAJOR {
        return Err(ProtocolError::UnsupportedMajor {
            major: header.major,
        });
    }
    if header.payload_len > MAX_PAYLOAD_LEN {
        return Err(ProtocolError::PayloadTooLarge {
            len: header.payload_len,
        });
    }

    Ok(header)
}

fn decode_payload(kind: MessageKind, payload: &[u8]) -> Result<WireMessage, ProtocolError> {
    match kind {
        MessageKind::Hello => from_bytes(payload).map(WireMessage::Hello),
        MessageKind::Challenge => from_bytes(payload).map(WireMessage::Challenge),
        MessageKind::ChallengeAccepted => from_bytes(payload).map(WireMessage::ChallengeAccepted),
        MessageKind::ChallengeDenied => from_bytes(payload).map(WireMessage::ChallengeDenied),
        MessageKind::StartGame => from_bytes(payload).map(WireMessage::StartGame),
        MessageKind::PlayerInput => from_bytes(payload).map(WireMessage::PlayerInput),
        MessageKind::ScoreSnapshot => from_bytes(payload).map(WireMessage::ScoreSnapshot),
        MessageKind::BoardSnapshot => from_bytes(payload).map(WireMessage::BoardSnapshot),
        MessageKind::ArsenalSnapshot => from_bytes(payload).map(WireMessage::ArsenalSnapshot),
        MessageKind::WeaponLaunch => from_bytes(payload).map(WireMessage::WeaponLaunch),
        MessageKind::WeaponActive => from_bytes(payload).map(WireMessage::WeaponActive),
        MessageKind::WeaponExpired => from_bytes(payload).map(WireMessage::WeaponExpired),
        MessageKind::BazaarDone => from_bytes(payload).map(WireMessage::BazaarDone),
        MessageKind::BazaarState => from_bytes(payload).map(WireMessage::BazaarState),
        MessageKind::GameOver => from_bytes(payload).map(WireMessage::GameOver),
        MessageKind::Pause => from_bytes(payload).map(WireMessage::Pause),
        MessageKind::Disconnect => from_bytes(payload).map(WireMessage::Disconnect),
        MessageKind::LobbyRegister => from_bytes(payload).map(WireMessage::LobbyRegister),
        MessageKind::LobbyListRequest => from_bytes(payload).map(WireMessage::LobbyListRequest),
        MessageKind::LobbyList => from_bytes(payload).map(WireMessage::LobbyList),
        MessageKind::HostedGameStart => from_bytes(payload).map(WireMessage::HostedGameStart),
        MessageKind::RankedResultClaim => from_bytes(payload).map(WireMessage::RankedResultClaim),
        MessageKind::RankedResultAccepted => {
            from_bytes(payload).map(WireMessage::RankedResultAccepted)
        }
        MessageKind::RankedResultRejected => {
            from_bytes(payload).map(WireMessage::RankedResultRejected)
        }
    }
}

fn to_stdvec<T>(value: &T) -> Result<Vec<u8>, ProtocolError>
where
    T: Serialize,
{
    postcard::to_stdvec(value).map_err(|error| ProtocolError::Encode(error.to_string()))
}

fn from_bytes<'a, T>(payload: &'a [u8]) -> Result<T, ProtocolError>
where
    T: Deserialize<'a>,
{
    postcard::from_bytes(payload).map_err(|error| ProtocolError::Decode(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(name: &str) -> PlayerIdentity {
        PlayerIdentity {
            display_name: name.to_string(),
        }
    }

    fn hosted_player(id: &str, name: &str) -> HostedPlayer {
        HostedPlayer {
            player_id: id.to_string(),
            display_name: name.to_string(),
        }
    }

    fn fixture_messages() -> Vec<WireMessage> {
        vec![
            WireMessage::Hello(Hello {
                major: PROTOCOL_MAJOR,
                minor: PROTOCOL_MINOR,
                identity: identity("Ada"),
                capabilities: vec!["direct-tcp".to_string()],
            }),
            WireMessage::Challenge(Challenge {
                challenger: identity("Ada"),
                message: "battle?".to_string(),
            }),
            WireMessage::ChallengeAccepted(ChallengeAccepted {
                accepter: identity("Ben"),
            }),
            WireMessage::ChallengeDenied(ChallengeDenied {
                reason: "busy".to_string(),
            }),
            WireMessage::StartGame(StartGame {
                receiving_peer_slot: PlayerSlot::Two,
                seed: 0x0102_0304_0506_0708,
                ranked: true,
            }),
            WireMessage::PlayerInput(PlayerInput {
                player: PlayerSlot::One,
                tick: 42,
                command: InputCommand::LaunchWeapon { slot_index: 9 },
            }),
            WireMessage::ScoreSnapshot(ScoreSnapshot {
                player: PlayerSlot::Two,
                score: 1200,
                funds: -25,
                lines: 21,
            }),
            WireMessage::BoardSnapshot(
                BoardSnapshot::new(
                    PlayerSlot::One,
                    7,
                    10,
                    28,
                    (0..280)
                        .map(|index| if index == 279 { 24 } else { 0 })
                        .collect(),
                )
                .expect("valid board fixture"),
            ),
            WireMessage::ArsenalSnapshot(ArsenalSnapshot {
                player: PlayerSlot::One,
                slots: [
                    Some(ArsenalEntry {
                        weapon: 5,
                        quantity: 2,
                    }),
                    None,
                    Some(ArsenalEntry {
                        weapon: 28,
                        quantity: 1,
                    }),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ],
            }),
            WireMessage::WeaponLaunch(WeaponLaunch {
                launcher: PlayerSlot::One,
                target: PlayerSlot::Two,
                weapon: 5,
                slot_index: 0,
                sequence: 11,
            }),
            WireMessage::WeaponActive(WeaponActive {
                target: PlayerSlot::Two,
                weapon: 28,
                remaining_lines: 10,
            }),
            WireMessage::WeaponExpired(WeaponExpired {
                target: PlayerSlot::Two,
                weapon: 28,
            }),
            WireMessage::BazaarDone(BazaarDone {
                player: PlayerSlot::One,
            }),
            WireMessage::BazaarState(BazaarState {
                player_one_done: true,
                player_two_done: false,
            }),
            WireMessage::GameOver(GameOver {
                winner: PlayerSlot::One,
                loser: PlayerSlot::Two,
                sequence: 99,
            }),
            WireMessage::Pause(Pause { paused: true }),
            WireMessage::Disconnect(Disconnect {
                reason: "bye".to_string(),
            }),
            WireMessage::LobbyRegister(LobbyRegister {
                player: hosted_player("ada", "Ada"),
                direct_addr: "127.0.0.1:4404".to_string(),
                ranked: true,
            }),
            WireMessage::LobbyListRequest(LobbyListRequest { ranked_only: true }),
            WireMessage::LobbyList(LobbyList {
                entries: vec![LobbyEntry {
                    session_id: HostedSessionId("session-1".to_string()),
                    host: hosted_player("ada", "Ada"),
                    direct_addr: "127.0.0.1:4404".to_string(),
                    ranked: true,
                    protocol_major: PROTOCOL_MAJOR,
                    protocol_minor: PROTOCOL_MINOR,
                }],
            }),
            WireMessage::HostedGameStart(HostedGameStart {
                session_id: HostedSessionId("session-1".to_string()),
                player_one: hosted_player("ada", "Ada"),
                player_two: hosted_player("ben", "Ben"),
                seed: 77,
                ranked: true,
                community_label: "main-server".to_string(),
            }),
            WireMessage::RankedResultClaim(RankedResultClaim {
                session_id: HostedSessionId("session-1".to_string()),
                reporter_player_id: "ada".to_string(),
                winner_player_id: "ada".to_string(),
                loser_player_id: "ben".to_string(),
                winner_score: 1200,
                winner_lines: 20,
                winner_funds: 300,
                loser_score: 800,
                loser_lines: 14,
                loser_funds: 200,
                duration_secs: 180,
                event_count: 44,
            }),
            WireMessage::RankedResultAccepted(RankedResultAccepted {
                session_id: HostedSessionId("session-1".to_string()),
            }),
            WireMessage::RankedResultRejected(RankedResultRejected {
                session_id: Some(HostedSessionId("session-1".to_string())),
                reason: "mismatched claims".to_string(),
            }),
        ]
    }

    #[test]
    fn all_public_messages_round_trip_with_expected_kinds() {
        for message in fixture_messages() {
            let encoded = encode_message(&message).expect("message encodes");
            let raw = decode_raw_frame(&encoded).expect("frame decodes");
            assert_eq!(raw.header.kind, message.kind() as u16);
            assert_eq!(raw.header.major, PROTOCOL_MAJOR);
            assert_eq!(raw.header.minor, PROTOCOL_MINOR);
            assert_eq!(raw.header.flags, FLAG_NONE);
            assert_eq!(raw.header.payload_len as usize, raw.payload.len());
            assert_eq!(decode_message(&encoded).expect("message decodes"), message);
        }
    }

    #[test]
    fn frame_header_uses_fixed_big_endian_layout() {
        let message = WireMessage::Pause(Pause { paused: true });
        let encoded = encode_message(&message).expect("message encodes");
        assert_eq!(&encoded[..4], b"BTRS");
        assert_eq!(&encoded[4..6], &PROTOCOL_MAJOR.to_be_bytes());
        assert_eq!(&encoded[6..8], &PROTOCOL_MINOR.to_be_bytes());
        assert_eq!(&encoded[8..10], &(MessageKind::Pause as u16).to_be_bytes());
        assert_eq!(&encoded[10..12], &FLAG_NONE.to_be_bytes());
        assert_eq!(&encoded[12..16], &1_u32.to_be_bytes());
        assert_eq!(encoded[16], 1);
    }

    #[test]
    fn rejects_bad_magic_unsupported_major_length_mismatch_and_large_payloads() {
        let mut encoded =
            encode_message(&WireMessage::Pause(Pause { paused: true })).expect("message encodes");

        let mut bad_magic = encoded.clone();
        bad_magic[0] = b'X';
        assert!(matches!(
            decode_message(&bad_magic),
            Err(ProtocolError::BadMagic { .. })
        ));

        let mut bad_major = encoded.clone();
        bad_major[5] = 2;
        assert_eq!(
            decode_message(&bad_major),
            Err(ProtocolError::UnsupportedMajor { major: 2 })
        );

        encoded.pop();
        assert!(matches!(
            decode_message(&encoded),
            Err(ProtocolError::LengthMismatch { .. })
        ));

        let mut too_large = Vec::from(MAGIC);
        too_large.extend_from_slice(&PROTOCOL_MAJOR.to_be_bytes());
        too_large.extend_from_slice(&PROTOCOL_MINOR.to_be_bytes());
        too_large.extend_from_slice(&(MessageKind::Pause as u16).to_be_bytes());
        too_large.extend_from_slice(&FLAG_NONE.to_be_bytes());
        too_large.extend_from_slice(&(MAX_PAYLOAD_LEN + 1).to_be_bytes());
        assert_eq!(
            decode_raw_frame(&too_large),
            Err(ProtocolError::PayloadTooLarge {
                len: MAX_PAYLOAD_LEN + 1,
            })
        );
    }

    #[test]
    fn raw_frame_allows_unknown_kind_but_message_decode_rejects_it() {
        let mut frame = Vec::from(MAGIC);
        frame.extend_from_slice(&PROTOCOL_MAJOR.to_be_bytes());
        frame.extend_from_slice(&PROTOCOL_MINOR.to_be_bytes());
        frame.extend_from_slice(&999_u16.to_be_bytes());
        frame.extend_from_slice(&FLAG_NONE.to_be_bytes());
        frame.extend_from_slice(&0_u32.to_be_bytes());

        let raw = decode_raw_frame(&frame).expect("unknown raw frame is still skippable");
        assert_eq!(raw.header.kind, 999);
        assert_eq!(
            decode_message(&frame),
            Err(ProtocolError::UnknownKind { kind: 999 })
        );
    }

    #[test]
    fn validates_board_snapshot_cell_count() {
        assert_eq!(
            BoardSnapshot::new(PlayerSlot::One, 0, 10, 28, vec![0; 279]),
            Err(ProtocolError::InvalidSnapshotCellCount {
                expected: 280,
                actual: 279,
            })
        );
    }

    #[test]
    fn representative_challenge_play_bazaar_game_over_flow_round_trips() {
        let flow = vec![
            WireMessage::Hello(Hello {
                major: PROTOCOL_MAJOR,
                minor: PROTOCOL_MINOR,
                identity: identity("Ada"),
                capabilities: vec!["direct-tcp".to_string(), "lan-discovery".to_string()],
            }),
            WireMessage::Challenge(Challenge {
                challenger: identity("Ada"),
                message: String::new(),
            }),
            WireMessage::ChallengeAccepted(ChallengeAccepted {
                accepter: identity("Ben"),
            }),
            WireMessage::StartGame(StartGame {
                receiving_peer_slot: PlayerSlot::One,
                seed: 1234,
                ranked: false,
            }),
            WireMessage::PlayerInput(PlayerInput {
                player: PlayerSlot::One,
                tick: 12,
                command: InputCommand::MoveLeft,
            }),
            WireMessage::ScoreSnapshot(ScoreSnapshot {
                player: PlayerSlot::One,
                score: 28,
                funds: 150,
                lines: 20,
            }),
            WireMessage::BazaarState(BazaarState {
                player_one_done: false,
                player_two_done: false,
            }),
            WireMessage::BazaarDone(BazaarDone {
                player: PlayerSlot::One,
            }),
            WireMessage::BazaarDone(BazaarDone {
                player: PlayerSlot::Two,
            }),
            WireMessage::GameOver(GameOver {
                winner: PlayerSlot::Two,
                loser: PlayerSlot::One,
                sequence: 400,
            }),
            WireMessage::Disconnect(Disconnect {
                reason: "complete".to_string(),
            }),
        ];

        for expected in flow {
            let frame = encode_message(&expected).expect("flow message encodes");
            assert_eq!(
                decode_message(&frame).expect("flow message decodes"),
                expected
            );
        }
    }

    #[test]
    fn representative_hosted_lobby_and_ranked_result_flow_round_trips() {
        let session_id = HostedSessionId("session-42".to_string());
        let ada = hosted_player("ada", "Ada");
        let ben = hosted_player("ben", "Ben");
        let flow = vec![
            WireMessage::LobbyRegister(LobbyRegister {
                player: ada.clone(),
                direct_addr: "192.0.2.10:4404".to_string(),
                ranked: true,
            }),
            WireMessage::LobbyListRequest(LobbyListRequest { ranked_only: true }),
            WireMessage::LobbyList(LobbyList {
                entries: vec![LobbyEntry {
                    session_id: session_id.clone(),
                    host: ada.clone(),
                    direct_addr: "192.0.2.10:4404".to_string(),
                    ranked: true,
                    protocol_major: PROTOCOL_MAJOR,
                    protocol_minor: PROTOCOL_MINOR,
                }],
            }),
            WireMessage::HostedGameStart(HostedGameStart {
                session_id: session_id.clone(),
                player_one: ada.clone(),
                player_two: ben,
                seed: 0x55,
                ranked: true,
                community_label: "garage-server".to_string(),
            }),
            WireMessage::RankedResultClaim(RankedResultClaim {
                session_id: session_id.clone(),
                reporter_player_id: "ada".to_string(),
                winner_player_id: "ada".to_string(),
                loser_player_id: "ben".to_string(),
                winner_score: 2000,
                winner_lines: 24,
                winner_funds: -400,
                loser_score: 1500,
                loser_lines: 18,
                loser_funds: -250,
                duration_secs: 300,
                event_count: 75,
            }),
            WireMessage::RankedResultAccepted(RankedResultAccepted { session_id }),
        ];

        for expected in flow {
            let frame = encode_message(&expected).expect("hosted message encodes");
            assert_eq!(
                decode_message(&frame).expect("hosted message decodes"),
                expected
            );
        }
    }
}
