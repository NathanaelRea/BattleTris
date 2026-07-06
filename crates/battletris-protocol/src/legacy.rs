//! Original BattleTris packet protocol support.

use std::{
    fmt, io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use crate::PlayerIdentity;

/// Header size for the original `PacketBuffer` framing.
pub const LEGACY_HEADER_LEN: usize = 2 * LEGACY_C_ULONG_LEN;
/// Conservative maximum legacy packet payload accepted before allocation.
pub const LEGACY_MAX_PAYLOAD_LEN: u32 = 64 * 1024;
/// Fixed payload length of the original `BTScore::writebuf` record.
pub const LEGACY_SCORE_PAYLOAD_LEN: usize = 6 * 4;
/// Fixed payload length for original signed short delta packets.
pub const LEGACY_SHORT_PAYLOAD_LEN: usize = 2;
/// Fixed byte count before cells in an original `BT_BOARD` payload.
pub const LEGACY_BOARD_PAYLOAD_HEADER_LEN: usize = 3 * 2;
/// Original board width used by the C++ client.
pub const LEGACY_BOARD_WIDTH: u16 = 10;
/// Original board height used by the C++ client.
pub const LEGACY_BOARD_HEIGHT: u16 = 28;
/// Original number of arsenal slots.
pub const LEGACY_ARSENAL_SIZE: usize = 10;
/// Original `BT_MAX_WEAPONS` sentinel value.
pub const LEGACY_MAX_WEAPON_SENTINEL: u16 = 34;
/// Original `BT_NO_WPN` empty arsenal slot token.
pub const LEGACY_NO_WEAPON_TOKEN: u16 = 35;
/// Highest concrete original weapon token value.
pub const LEGACY_LAST_WEAPON_TOKEN: u16 = LEGACY_MAX_WEAPON_SENTINEL - 1;

/// Fixed byte length of `BTDBRecord::key_`, including its trailing NUL slot.
pub const LEGACY_DB_KEY_LEN: usize = 281;
/// Fixed byte length of `BTNetworkEntry::userName_`, including its trailing NUL slot.
pub const LEGACY_USERNAME_LEN: usize = 264;
/// Fixed byte length of `BTNetworkEntry::hostName_`, including its trailing NUL slot.
pub const LEGACY_HOSTNAME_LEN: usize = 256;
/// Fixed byte length of the original `BTNetworkEntry` wire record.
pub const LEGACY_NETWORK_ENTRY_LEN_32: usize =
    LEGACY_DB_KEY_LEN + LEGACY_USERNAME_LEN + LEGACY_HOSTNAME_LEN + 4 * 4 + 5 * 2;
/// Native `unsigned long` size used by the locally built original C++ daemon/client.
pub const LEGACY_C_ULONG_LEN: usize = std::mem::size_of::<usize>();
/// Fixed byte length of `BTNetworkEntry` for the locally built original C++ daemon/client.
pub const LEGACY_NETWORK_ENTRY_LEN: usize =
    LEGACY_DB_KEY_LEN + LEGACY_USERNAME_LEN + LEGACY_HOSTNAME_LEN + 4 * LEGACY_C_ULONG_LEN + 5 * 2;

const LEGACY_MAJOR_VERSION: u16 = 1;
const LEGACY_MINOR_VERSION: u16 = 0;
const LEGACY_MAX_WEAPONS: u16 = 34;

/// Original `BTNetworkStatus` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum LegacyNetworkStatus {
    /// Status was not known.
    Unknown = 0,
    /// Player is waiting for a challenge.
    Waiting = 1,
    /// Player is currently in a game.
    Playing = 2,
}

impl LegacyNetworkStatus {
    fn from_u16(value: u16) -> Self {
        match value {
            1 => Self::Waiting,
            2 => Self::Playing,
            _ => Self::Unknown,
        }
    }
}

/// Original `BTToken` values from `BTProtocol.H`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LegacyToken {
    /// Null packet indicates that connection is over.
    Null = 0,
    /// A network error has occurred.
    Err = 1,
    /// Score delta packet.
    Score = 10,
    /// Opponent score delta packet.
    OpponentScore = 11,
    /// Line count increase packet.
    Line = 12,
    /// Board snapshot packet.
    Board = 13,
    /// Arsenal snapshot packet.
    Arsenal = 14,
    /// Funds delta packet.
    Funds = 15,
    /// Timed weapon activation packet.
    WeaponOn = 16,
    /// Weapon launch packet.
    WeaponLaunch = 17,
    /// Timed weapon expiration packet.
    WeaponOff = 18,
    /// Opponent death packet.
    Dead = 19,
    /// Opponent entered Bazaar packet.
    StartBazaar = 20,
    /// Opponent finished Bazaar packet.
    EndBazaar = 21,
    /// Local game-over packet.
    GameOver = 22,
    /// Airslide packet.
    Airslide = 23,
    /// Lawyer rise-up packet.
    Lawyer = 24,
    /// Ping/status packet.
    Ping = 30,
    /// Peer busy response.
    Busy = 31,
    /// Challenge packet carrying a `BTNetworkEntry`.
    Challenge = 32,
    /// Challenge accepted response.
    Accept = 33,
    /// Challenge denied response.
    Deny = 34,
    /// Start-game synchronization packet.
    Start = 35,
    /// Pause packet.
    Pause = 50,
    /// Idiot packet.
    Idiot = 51,
    /// Condor off packet.
    CondorOff = 52,
    /// Local client connection request.
    Local = 60,
    /// Remote client connection request.
    Remote = 61,
    /// Cookie accepted packet.
    CookieGood = 62,
    /// Cookie rejected packet.
    CookieBad = 63,
    /// Server accepted client packet.
    Accepted = 64,
    /// Server rejected client packet.
    Rejected = 65,
    /// Server command packet.
    ObeyMe = 66,
    /// Slave acknowledgement packet.
    IObey = 67,
    /// New client packet.
    NewClient = 68,
    /// Client accepted packet.
    ClientOk = 69,
    /// Client rejected packet.
    ClientBad = 70,
    /// Disconnect packet.
    Disconnect = 71,
    /// Slave shutdown packet.
    Harikari = 72,
    /// Cookie query packet.
    QueryCookie = 73,
    /// Connection query packet.
    QueryConnection = 74,
    /// Network DB query packet.
    QueryNetworkDb = 75,
    /// Player DB query packet.
    QueryPlayerDb = 76,
    /// Verify query packet.
    QueryVerify = 77,
    /// Update query packet.
    QueryUpdate = 78,
    /// Result query packet.
    QueryResult = 79,
    /// Cookie response packet.
    ResponseCookie = 80,
    /// Verify response packet.
    ResponseVerify = 81,
    /// DB length response packet.
    ResponseDbLen = 82,
    /// Network DB response packet.
    ResponseNetworkDb = 83,
    /// Player DB response packet.
    ResponsePlayerDb = 84,
}

impl LegacyToken {
    /// Converts a raw original token value into a known token.
    #[must_use]
    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Null),
            1 => Some(Self::Err),
            10 => Some(Self::Score),
            11 => Some(Self::OpponentScore),
            12 => Some(Self::Line),
            13 => Some(Self::Board),
            14 => Some(Self::Arsenal),
            15 => Some(Self::Funds),
            16 => Some(Self::WeaponOn),
            17 => Some(Self::WeaponLaunch),
            18 => Some(Self::WeaponOff),
            19 => Some(Self::Dead),
            20 => Some(Self::StartBazaar),
            21 => Some(Self::EndBazaar),
            22 => Some(Self::GameOver),
            23 => Some(Self::Airslide),
            24 => Some(Self::Lawyer),
            30 => Some(Self::Ping),
            31 => Some(Self::Busy),
            32 => Some(Self::Challenge),
            33 => Some(Self::Accept),
            34 => Some(Self::Deny),
            35 => Some(Self::Start),
            50 => Some(Self::Pause),
            51 => Some(Self::Idiot),
            52 => Some(Self::CondorOff),
            60 => Some(Self::Local),
            61 => Some(Self::Remote),
            62 => Some(Self::CookieGood),
            63 => Some(Self::CookieBad),
            64 => Some(Self::Accepted),
            65 => Some(Self::Rejected),
            66 => Some(Self::ObeyMe),
            67 => Some(Self::IObey),
            68 => Some(Self::NewClient),
            69 => Some(Self::ClientOk),
            70 => Some(Self::ClientBad),
            71 => Some(Self::Disconnect),
            72 => Some(Self::Harikari),
            73 => Some(Self::QueryCookie),
            74 => Some(Self::QueryConnection),
            75 => Some(Self::QueryNetworkDb),
            76 => Some(Self::QueryPlayerDb),
            77 => Some(Self::QueryVerify),
            78 => Some(Self::QueryUpdate),
            79 => Some(Self::QueryResult),
            80 => Some(Self::ResponseCookie),
            81 => Some(Self::ResponseVerify),
            82 => Some(Self::ResponseDbLen),
            83 => Some(Self::ResponseNetworkDb),
            84 => Some(Self::ResponsePlayerDb),
            _ => None,
        }
    }
}

/// One original framed packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyPacket {
    /// Original token value.
    pub token: LegacyToken,
    /// Packet payload bytes.
    pub payload: Vec<u8>,
}

impl LegacyPacket {
    /// Creates an empty packet for tokens without payloads.
    #[must_use]
    pub fn empty(token: LegacyToken) -> Self {
        Self {
            token,
            payload: Vec::new(),
        }
    }
}

/// Original full `BTScore` wire payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyScorePayload {
    /// Local score in the sender's `BTScore` record.
    pub score: u32,
    /// Opponent score in the sender's `BTScore` record.
    pub opponent_score: u32,
    /// Local cleared line count in the sender's `BTScore` record.
    pub lines: u32,
    /// Opponent cleared line count in the sender's `BTScore` record.
    pub opponent_lines: u32,
    /// Local spendable funds in the sender's `BTScore` record.
    pub funds: i32,
    /// Opponent spendable funds in the sender's `BTScore` record.
    pub opponent_funds: i32,
}

impl LegacyScorePayload {
    /// Encodes this score record as six big-endian 32-bit fields.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(LEGACY_SCORE_PAYLOAD_LEN);
        bytes.extend_from_slice(&self.score.to_be_bytes());
        bytes.extend_from_slice(&self.opponent_score.to_be_bytes());
        bytes.extend_from_slice(&self.lines.to_be_bytes());
        bytes.extend_from_slice(&self.opponent_lines.to_be_bytes());
        bytes.extend_from_slice(&self.funds.to_be_bytes());
        bytes.extend_from_slice(&self.opponent_funds.to_be_bytes());
        bytes
    }

    /// Decodes an original full `BTScore` wire payload.
    pub fn decode(bytes: &[u8]) -> Result<Self, LegacyError> {
        require_len(
            "legacy score payload",
            LEGACY_SCORE_PAYLOAD_LEN,
            bytes.len(),
        )?;
        let mut offset = 0;
        Ok(Self {
            score: read_u32(bytes, &mut offset),
            opponent_score: read_u32(bytes, &mut offset),
            lines: read_u32(bytes, &mut offset),
            opponent_lines: read_u32(bytes, &mut offset),
            funds: read_i32(bytes, &mut offset),
            opponent_funds: read_i32(bytes, &mut offset),
        })
    }
}

/// Original signed 16-bit payload used by legacy delta-style score tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyShortPayload {
    /// Signed short value carried by the packet.
    pub value: i16,
}

impl LegacyShortPayload {
    /// Encodes this payload as one big-endian signed short.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.value.to_be_bytes().to_vec()
    }

    /// Decodes one big-endian signed short payload.
    pub fn decode(bytes: &[u8]) -> Result<Self, LegacyError> {
        require_len(
            "legacy short payload",
            LEGACY_SHORT_PAYLOAD_LEN,
            bytes.len(),
        )?;
        Ok(Self {
            value: i16::from_be_bytes(bytes.try_into().expect("slice length checked")),
        })
    }
}

/// Original `BT_BOARD` wire payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBoardPayload {
    /// Weapon/motivation token associated with this board snapshot.
    pub motivation: u16,
    /// Board height in cells.
    pub height: u16,
    /// Board width in cells.
    pub width: u16,
    /// Row-major legacy cell IDs.
    pub cells: Vec<u32>,
}

impl LegacyBoardPayload {
    /// Builds a board payload after validating the row-major cell count.
    pub fn new(
        motivation: u16,
        height: u16,
        width: u16,
        cells: Vec<u32>,
    ) -> Result<Self, LegacyError> {
        validate_board_cell_count(width, height, cells.len())?;
        Ok(Self {
            motivation,
            height,
            width,
            cells,
        })
    }

    /// Encodes this board payload using original big-endian fields.
    pub fn encode(&self) -> Result<Vec<u8>, LegacyError> {
        validate_board_cell_count(self.width, self.height, self.cells.len())?;
        let payload_len = LEGACY_BOARD_PAYLOAD_HEADER_LEN + self.cells.len() * 4;
        validate_legacy_payload_len(payload_len)?;

        let mut bytes = Vec::with_capacity(payload_len);
        bytes.extend_from_slice(&self.motivation.to_be_bytes());
        bytes.extend_from_slice(&self.height.to_be_bytes());
        bytes.extend_from_slice(&self.width.to_be_bytes());
        for cell in &self.cells {
            bytes.extend_from_slice(&cell.to_be_bytes());
        }
        Ok(bytes)
    }

    /// Decodes an original `BT_BOARD` wire payload.
    pub fn decode(bytes: &[u8]) -> Result<Self, LegacyError> {
        if bytes.len() < LEGACY_BOARD_PAYLOAD_HEADER_LEN {
            return Err(LegacyError::PayloadLengthMismatch {
                context: "legacy board payload header",
                expected: LEGACY_BOARD_PAYLOAD_HEADER_LEN,
                actual: bytes.len(),
            });
        }
        if !(bytes.len() - LEGACY_BOARD_PAYLOAD_HEADER_LEN).is_multiple_of(4) {
            return Err(LegacyError::PayloadLengthMismatch {
                context: "legacy board cell payload",
                expected: LEGACY_BOARD_PAYLOAD_HEADER_LEN
                    + ((bytes.len() - LEGACY_BOARD_PAYLOAD_HEADER_LEN) / 4 + 1) * 4,
                actual: bytes.len(),
            });
        }

        let mut offset = 0;
        let motivation = read_u16(bytes, &mut offset);
        let height = read_u16(bytes, &mut offset);
        let width = read_u16(bytes, &mut offset);
        let cell_count = (bytes.len() - LEGACY_BOARD_PAYLOAD_HEADER_LEN) / 4;
        validate_board_cell_count(width, height, cell_count)?;

        let mut cells = Vec::with_capacity(cell_count);
        for _ in 0..cell_count {
            cells.push(read_u32(bytes, &mut offset));
        }

        Ok(Self {
            motivation,
            height,
            width,
            cells,
        })
    }
}

/// One original arsenal slot entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyArsenalSlot {
    /// Original `BTWeaponToken`, or [`LEGACY_NO_WEAPON_TOKEN`] for an empty slot.
    pub weapon_token: u16,
    /// Quantity stacked in the arsenal slot.
    pub quantity: u16,
}

/// Original `BT_ARSENAL` wire payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyArsenalPayload {
    /// Ordered arsenal slots.
    pub slots: Vec<LegacyArsenalSlot>,
}

impl LegacyArsenalPayload {
    /// Builds an arsenal payload after validating slot count and weapon tokens.
    pub fn new(slots: Vec<LegacyArsenalSlot>) -> Result<Self, LegacyError> {
        validate_arsenal_slots(&slots)?;
        Ok(Self { slots })
    }

    /// Encodes this arsenal payload using original big-endian fields.
    pub fn encode(&self) -> Result<Vec<u8>, LegacyError> {
        validate_arsenal_slots(&self.slots)?;
        let payload_len = 2 + self.slots.len() * 4;
        validate_legacy_payload_len(payload_len)?;

        let mut bytes = Vec::with_capacity(payload_len);
        bytes.extend_from_slice(&(self.slots.len() as u16).to_be_bytes());
        for slot in &self.slots {
            bytes.extend_from_slice(&slot.weapon_token.to_be_bytes());
            bytes.extend_from_slice(&slot.quantity.to_be_bytes());
        }
        Ok(bytes)
    }

    /// Decodes an original `BT_ARSENAL` wire payload.
    pub fn decode(bytes: &[u8]) -> Result<Self, LegacyError> {
        if bytes.len() < 2 {
            return Err(LegacyError::PayloadLengthMismatch {
                context: "legacy arsenal slot count",
                expected: 2,
                actual: bytes.len(),
            });
        }
        let mut offset = 0;
        let slot_count = usize::from(read_u16(bytes, &mut offset));
        if slot_count > LEGACY_ARSENAL_SIZE {
            return Err(LegacyError::InvalidArsenalSlotCount {
                max: LEGACY_ARSENAL_SIZE,
                actual: slot_count,
            });
        }

        let expected_len = 2 + slot_count * 4;
        require_len("legacy arsenal payload", expected_len, bytes.len())?;
        let mut slots = Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            slots.push(LegacyArsenalSlot {
                weapon_token: read_u16(bytes, &mut offset),
                quantity: read_u16(bytes, &mut offset),
            });
        }
        validate_arsenal_slots(&slots)?;
        Ok(Self { slots })
    }
}

/// Original one-weapon-token wire payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyWeaponPayload {
    /// Original concrete `BTWeaponToken` value.
    pub weapon_token: u16,
}

impl LegacyWeaponPayload {
    /// Builds a weapon payload after validating the concrete token range.
    pub fn new(weapon_token: u16) -> Result<Self, LegacyError> {
        validate_concrete_weapon_token(weapon_token)?;
        Ok(Self { weapon_token })
    }

    /// Encodes this payload as one big-endian unsigned short.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.weapon_token.to_be_bytes().to_vec()
    }

    /// Decodes one big-endian concrete weapon token payload.
    pub fn decode(bytes: &[u8]) -> Result<Self, LegacyError> {
        require_len("legacy weapon payload", 2, bytes.len())?;
        Self::new(u16::from_be_bytes(
            bytes.try_into().expect("slice length checked"),
        ))
    }
}

/// Validates the empty payload used by legacy pause, Bazaar, and terminal packets.
pub fn validate_legacy_empty_payload(bytes: &[u8]) -> Result<(), LegacyError> {
    require_len("legacy empty payload", 0, bytes.len())
}

/// Original direct-challenge identity payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyNetworkEntry {
    /// DB key bytes decoded as a C string.
    pub key: String,
    /// Player/user name.
    pub user_name: String,
    /// Host name or address string.
    pub host_name: String,
    /// Unix timestamp seconds.
    pub timestamp: u32,
    /// Sender process id.
    pub pid: u32,
    /// Classful IPv4 network portion from `inet_netof`.
    pub addrnet: u32,
    /// Classful IPv4 local-host portion from `inet_lnaof`.
    pub addrlna: u32,
    /// Listening TCP port.
    pub port: u16,
    /// Highest legacy weapon token advertised by the peer.
    pub max_weapon: u16,
    /// Legacy protocol major version.
    pub major: u16,
    /// Legacy protocol minor version.
    pub minor: u16,
    /// Legacy availability status.
    pub status: LegacyNetworkStatus,
}

impl LegacyNetworkEntry {
    /// Builds a waiting entry for direct manual-IP legacy challenges.
    #[must_use]
    pub fn waiting(
        identity: PlayerIdentity,
        share_addr: SocketAddr,
        pid: u32,
        timestamp: u32,
    ) -> Self {
        let socket_v4 = match share_addr {
            SocketAddr::V4(addr) => addr,
            SocketAddr::V6(addr) => SocketAddrV4::new(Ipv4Addr::LOCALHOST, addr.port()),
        };
        let (addrnet, addrlna) = inet_net_lna(*socket_v4.ip());
        let host_name = socket_v4.ip().to_string();
        let user_name = identity.display_name;
        let key = legacy_key(&user_name, &host_name, socket_v4.port());
        Self {
            key,
            user_name,
            host_name,
            timestamp,
            pid,
            addrnet,
            addrlna,
            port: socket_v4.port(),
            max_weapon: LEGACY_MAX_WEAPONS,
            major: LEGACY_MAJOR_VERSION,
            minor: LEGACY_MINOR_VERSION,
            status: LegacyNetworkStatus::Waiting,
        }
    }

    /// Encodes the record with the original fixed C-string sizes and big-endian numbers.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(LEGACY_NETWORK_ENTRY_LEN);
        put_fixed_c_string(&mut bytes, &self.key, LEGACY_DB_KEY_LEN);
        put_fixed_c_string(&mut bytes, &self.user_name, LEGACY_USERNAME_LEN);
        put_fixed_c_string(&mut bytes, &self.host_name, LEGACY_HOSTNAME_LEN);
        put_legacy_c_ulong(&mut bytes, self.timestamp);
        put_legacy_c_ulong(&mut bytes, self.pid);
        put_legacy_c_ulong(&mut bytes, self.addrnet);
        put_legacy_c_ulong(&mut bytes, self.addrlna);
        bytes.extend_from_slice(&self.port.to_be_bytes());
        bytes.extend_from_slice(&self.max_weapon.to_be_bytes());
        bytes.extend_from_slice(&self.major.to_be_bytes());
        bytes.extend_from_slice(&self.minor.to_be_bytes());
        bytes.extend_from_slice(&(self.status as u16).to_be_bytes());
        bytes
    }

    /// Decodes a full original `BTNetworkEntry` record.
    pub fn decode(bytes: &[u8]) -> Result<Self, LegacyError> {
        if bytes.len() < LEGACY_NETWORK_ENTRY_LEN_32 {
            return Err(LegacyError::EntryTooShort {
                expected: LEGACY_NETWORK_ENTRY_LEN_32,
                actual: bytes.len(),
            });
        }
        let long_len = if bytes.len() >= LEGACY_NETWORK_ENTRY_LEN {
            LEGACY_C_ULONG_LEN
        } else {
            4
        };
        let mut offset = 0;
        let key = read_fixed_c_string(bytes, &mut offset, LEGACY_DB_KEY_LEN);
        let user_name = read_fixed_c_string(bytes, &mut offset, LEGACY_USERNAME_LEN);
        let host_name = read_fixed_c_string(bytes, &mut offset, LEGACY_HOSTNAME_LEN);
        let timestamp = read_legacy_c_ulong(bytes, &mut offset, long_len);
        let pid = read_legacy_c_ulong(bytes, &mut offset, long_len);
        let addrnet = read_legacy_c_ulong(bytes, &mut offset, long_len);
        let addrlna = read_legacy_c_ulong(bytes, &mut offset, long_len);
        let port = read_u16(bytes, &mut offset);
        let max_weapon = read_u16(bytes, &mut offset);
        let major = read_u16(bytes, &mut offset);
        let minor = read_u16(bytes, &mut offset);
        let status = LegacyNetworkStatus::from_u16(read_u16(bytes, &mut offset));
        Ok(Self {
            key,
            user_name,
            host_name,
            timestamp,
            pid,
            addrnet,
            addrlna,
            port,
            max_weapon,
            major,
            minor,
            status,
        })
    }
}

/// Established legacy direct TCP stream after challenge/start handshake.
#[derive(Debug)]
pub struct LegacyConnection {
    stream: TcpStream,
}

impl LegacyConnection {
    /// Wraps an established TCP stream.
    #[must_use]
    pub fn from_stream(stream: TcpStream) -> Self {
        let _ = stream.set_nodelay(true);
        Self { stream }
    }

    /// Sends one legacy packet.
    pub async fn send(&mut self, packet: &LegacyPacket) -> Result<(), LegacyError> {
        write_legacy_packet(&mut self.stream, packet).await
    }

    /// Receives one legacy packet.
    pub async fn recv(&mut self) -> Result<LegacyPacket, LegacyError> {
        read_legacy_packet(&mut self.stream).await
    }
}

/// Host-side pending legacy challenge.
#[derive(Debug)]
pub struct PendingLegacyChallenge {
    /// Established TCP connection.
    pub connection: LegacyConnection,
    /// Decoded challenger entry.
    pub challenger: LegacyNetworkEntry,
}

impl PendingLegacyChallenge {
    /// Accepts the challenge and completes the original `BT_START` exchange.
    pub async fn accept(mut self) -> Result<AcceptedLegacyGame, LegacyError> {
        self.connection
            .send(&LegacyPacket::empty(LegacyToken::Accept))
            .await?;
        self.connection
            .send(&LegacyPacket::empty(LegacyToken::Start))
            .await?;
        expect_legacy_token(self.connection.recv().await?, LegacyToken::Start)?;
        Ok(AcceptedLegacyGame {
            connection: self.connection,
            challenger: self.challenger,
        })
    }

    /// Denies the challenge using the original empty `BT_DENY` response.
    pub async fn deny(mut self) -> Result<(), LegacyError> {
        self.connection
            .send(&LegacyPacket::empty(LegacyToken::Deny))
            .await
    }
}

/// Host-side accepted legacy game.
#[derive(Debug)]
pub struct AcceptedLegacyGame {
    /// Established legacy connection.
    pub connection: LegacyConnection,
    /// Decoded challenger entry.
    pub challenger: LegacyNetworkEntry,
}

/// Join-side accepted legacy game.
#[derive(Debug)]
pub struct JoinedLegacyGame {
    /// Established legacy connection.
    pub connection: LegacyConnection,
}

/// Accepts one TCP peer and decodes its legacy challenge packet.
pub async fn accept_pending_legacy_challenge(
    listener: &TcpListener,
) -> Result<PendingLegacyChallenge, LegacyError> {
    let (stream, _) = listener.accept().await?;
    let mut connection = LegacyConnection::from_stream(stream);
    let packet = connection.recv().await?;
    if packet.token != LegacyToken::Challenge {
        return Err(LegacyError::UnexpectedToken {
            expected: LegacyToken::Challenge,
            actual: packet.token,
        });
    }
    let challenger = LegacyNetworkEntry::decode(&packet.payload)?;
    Ok(PendingLegacyChallenge {
        connection,
        challenger,
    })
}

/// Connects to a legacy host and completes the original challenge/start join flow.
pub async fn join_legacy_game(
    peer_addr: SocketAddr,
    entry: LegacyNetworkEntry,
) -> Result<JoinedLegacyGame, LegacyError> {
    let stream = TcpStream::connect(peer_addr).await?;
    stream.set_nodelay(true)?;
    let mut connection = LegacyConnection::from_stream(stream);
    connection
        .send(&LegacyPacket {
            token: LegacyToken::Challenge,
            payload: entry.encode(),
        })
        .await?;

    match connection.recv().await? {
        LegacyPacket {
            token: LegacyToken::Accept,
            ..
        } => {}
        LegacyPacket {
            token: LegacyToken::Deny,
            ..
        } => return Err(LegacyError::ChallengeDenied),
        LegacyPacket {
            token: LegacyToken::Busy,
            ..
        } => return Err(LegacyError::PeerBusy),
        packet => {
            return Err(LegacyError::UnexpectedToken {
                expected: LegacyToken::Accept,
                actual: packet.token,
            })
        }
    }

    connection
        .send(&LegacyPacket::empty(LegacyToken::Start))
        .await?;
    expect_legacy_token(connection.recv().await?, LegacyToken::Start)?;

    Ok(JoinedLegacyGame { connection })
}

/// Reads one original `PacketBuffer` packet from an async stream.
pub async fn read_legacy_packet<R>(reader: &mut R) -> Result<LegacyPacket, LegacyError>
where
    R: AsyncRead + Unpin,
{
    let mut header = [0_u8; LEGACY_HEADER_LEN];
    reader.read_exact(&mut header).await?;
    let token = u32::from_be_bytes(header[0..4].try_into().expect("slice length checked"));
    let payload_len = u32::from_be_bytes(
        header[LEGACY_C_ULONG_LEN..LEGACY_C_ULONG_LEN + 4]
            .try_into()
            .expect("slice length checked"),
    );
    if payload_len > LEGACY_MAX_PAYLOAD_LEN {
        return Err(LegacyError::PayloadTooLarge { len: payload_len });
    }
    let token = LegacyToken::from_u32(token).ok_or(LegacyError::UnknownToken { token })?;
    let mut payload = vec![0; payload_len as usize];
    if !payload.is_empty() {
        reader.read_exact(&mut payload).await?;
    }
    Ok(LegacyPacket { token, payload })
}

/// Writes one original `PacketBuffer` packet to an async stream.
pub async fn write_legacy_packet<W>(
    writer: &mut W,
    packet: &LegacyPacket,
) -> Result<(), LegacyError>
where
    W: AsyncWrite + Unpin,
{
    if packet.payload.len() > LEGACY_MAX_PAYLOAD_LEN as usize {
        return Err(LegacyError::PayloadTooLarge {
            len: packet.payload.len() as u32,
        });
    }
    write_legacy_c_ulong(writer, packet.token as u32).await?;
    write_legacy_c_ulong(writer, packet.payload.len() as u32).await?;
    if !packet.payload.is_empty() {
        writer.write_all(&packet.payload).await?;
    }
    writer.flush().await?;
    Ok(())
}

async fn write_legacy_c_ulong<W>(writer: &mut W, value: u32) -> Result<(), LegacyError>
where
    W: AsyncWrite + Unpin,
{
    writer.write_all(&value.to_be_bytes()).await?;
    if LEGACY_C_ULONG_LEN > 4 {
        let padding = [0_u8; 8];
        writer.write_all(&padding[..LEGACY_C_ULONG_LEN - 4]).await?;
    }
    Ok(())
}

/// Legacy protocol encoding, decoding, and handshake failures.
#[derive(Debug)]
pub enum LegacyError {
    /// Packet payload length exceeds [`LEGACY_MAX_PAYLOAD_LEN`].
    PayloadTooLarge {
        /// Payload length from the packet header.
        len: u32,
    },
    /// Packet token is not defined by `BTProtocol.H`.
    UnknownToken {
        /// Raw packet token.
        token: u32,
    },
    /// A fixed-size `BTNetworkEntry` payload was too short.
    EntryTooShort {
        /// Required byte count.
        expected: usize,
        /// Actual byte count.
        actual: usize,
    },
    /// A gameplay payload had an unexpected byte count.
    PayloadLengthMismatch {
        /// Payload family being decoded.
        context: &'static str,
        /// Required byte count.
        expected: usize,
        /// Actual byte count.
        actual: usize,
    },
    /// A `BT_BOARD` payload dimensions and cell count do not match.
    InvalidBoardCellCount {
        /// Expected width times height cell count.
        expected: usize,
        /// Actual cell count.
        actual: usize,
    },
    /// A `BT_ARSENAL` payload carried too many slots.
    InvalidArsenalSlotCount {
        /// Maximum accepted slot count.
        max: usize,
        /// Actual slot count.
        actual: usize,
    },
    /// A weapon token was outside the original concrete weapon range.
    InvalidWeaponToken {
        /// Invalid weapon token.
        token: u16,
    },
    /// A packet arrived out of order for the legacy handshake.
    UnexpectedToken {
        /// Expected token.
        expected: LegacyToken,
        /// Actual token.
        actual: LegacyToken,
    },
    /// The peer denied the challenge.
    ChallengeDenied,
    /// The peer reported that it was busy.
    PeerBusy,
    /// Transport I/O failed.
    Io(io::Error),
}

impl fmt::Display for LegacyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PayloadTooLarge { len } => write!(formatter, "legacy payload too large: {len}"),
            Self::UnknownToken { token } => write!(formatter, "unknown legacy token: {token}"),
            Self::EntryTooShort { expected, actual } => write!(
                formatter,
                "legacy network entry too short: expected {expected} bytes, got {actual}"
            ),
            Self::PayloadLengthMismatch {
                context,
                expected,
                actual,
            } => write!(
                formatter,
                "{context} has wrong length: expected {expected} bytes, got {actual}"
            ),
            Self::InvalidBoardCellCount { expected, actual } => write!(
                formatter,
                "legacy board cell count mismatch: expected {expected} cells, got {actual}"
            ),
            Self::InvalidArsenalSlotCount { max, actual } => write!(
                formatter,
                "legacy arsenal slot count too large: maximum {max}, got {actual}"
            ),
            Self::InvalidWeaponToken { token } => {
                write!(formatter, "invalid legacy weapon token: {token}")
            }
            Self::UnexpectedToken { expected, actual } => write!(
                formatter,
                "unexpected legacy token: expected {expected:?}, got {actual:?}"
            ),
            Self::ChallengeDenied => write!(formatter, "legacy challenge denied"),
            Self::PeerBusy => write!(formatter, "legacy peer is busy"),
            Self::Io(error) => write!(formatter, "legacy I/O error: {error}"),
        }
    }
}

impl std::error::Error for LegacyError {}

impl From<io::Error> for LegacyError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

fn expect_legacy_token(packet: LegacyPacket, expected: LegacyToken) -> Result<(), LegacyError> {
    if packet.token == expected {
        Ok(())
    } else {
        Err(LegacyError::UnexpectedToken {
            expected,
            actual: packet.token,
        })
    }
}

fn legacy_key(user_name: &str, host_name: &str, port: u16) -> String {
    format!("{user_name}{host_name}{port}")
}

fn put_fixed_c_string(bytes: &mut Vec<u8>, value: &str, len: usize) {
    let mut fixed = vec![0; len];
    let source = value.as_bytes();
    let copy_len = source.len().min(len.saturating_sub(1));
    fixed[..copy_len].copy_from_slice(&source[..copy_len]);
    bytes.extend_from_slice(&fixed);
}

fn put_legacy_c_ulong(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
    if LEGACY_C_ULONG_LEN > 4 {
        bytes.resize(bytes.len() + LEGACY_C_ULONG_LEN - 4, 0);
    }
}

fn read_fixed_c_string(bytes: &[u8], offset: &mut usize, len: usize) -> String {
    let field = &bytes[*offset..*offset + len];
    *offset += len;
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).to_string()
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> u32 {
    let value = u32::from_be_bytes(
        bytes[*offset..*offset + 4]
            .try_into()
            .expect("slice length checked"),
    );
    *offset += 4;
    value
}

fn read_legacy_c_ulong(bytes: &[u8], offset: &mut usize, long_len: usize) -> u32 {
    let value = read_u32(bytes, offset);
    *offset += long_len.saturating_sub(4);
    value
}

fn read_i32(bytes: &[u8], offset: &mut usize) -> i32 {
    let value = i32::from_be_bytes(
        bytes[*offset..*offset + 4]
            .try_into()
            .expect("slice length checked"),
    );
    *offset += 4;
    value
}

fn read_u16(bytes: &[u8], offset: &mut usize) -> u16 {
    let value = u16::from_be_bytes(
        bytes[*offset..*offset + 2]
            .try_into()
            .expect("slice length checked"),
    );
    *offset += 2;
    value
}

fn require_len(context: &'static str, expected: usize, actual: usize) -> Result<(), LegacyError> {
    if actual == expected {
        Ok(())
    } else {
        Err(LegacyError::PayloadLengthMismatch {
            context,
            expected,
            actual,
        })
    }
}

fn validate_legacy_payload_len(len: usize) -> Result<(), LegacyError> {
    if len > LEGACY_MAX_PAYLOAD_LEN as usize {
        Err(LegacyError::PayloadTooLarge { len: len as u32 })
    } else {
        Ok(())
    }
}

fn validate_board_cell_count(width: u16, height: u16, actual: usize) -> Result<(), LegacyError> {
    let expected = usize::from(width) * usize::from(height);
    if actual == expected {
        Ok(())
    } else {
        Err(LegacyError::InvalidBoardCellCount { expected, actual })
    }
}

fn validate_arsenal_slots(slots: &[LegacyArsenalSlot]) -> Result<(), LegacyError> {
    if slots.len() > LEGACY_ARSENAL_SIZE {
        return Err(LegacyError::InvalidArsenalSlotCount {
            max: LEGACY_ARSENAL_SIZE,
            actual: slots.len(),
        });
    }

    for slot in slots {
        validate_arsenal_weapon_token(slot.weapon_token)?;
    }
    Ok(())
}

fn validate_arsenal_weapon_token(token: u16) -> Result<(), LegacyError> {
    if token <= LEGACY_LAST_WEAPON_TOKEN || token == LEGACY_NO_WEAPON_TOKEN {
        Ok(())
    } else {
        Err(LegacyError::InvalidWeaponToken { token })
    }
}

fn validate_concrete_weapon_token(token: u16) -> Result<(), LegacyError> {
    if token <= LEGACY_LAST_WEAPON_TOKEN {
        Ok(())
    } else {
        Err(LegacyError::InvalidWeaponToken { token })
    }
}

fn inet_net_lna(ip: Ipv4Addr) -> (u32, u32) {
    let octets = ip.octets();
    let host = u32::from_be_bytes(octets);
    if octets[0] < 128 {
        (u32::from(octets[0]), host & 0x00ff_ffff)
    } else if octets[0] < 192 {
        (host >> 16, host & 0x0000_ffff)
    } else {
        (host >> 8, host & 0x0000_00ff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io;

    fn packet(token: LegacyToken, payload: &[u8]) -> LegacyPacket {
        LegacyPacket {
            token,
            payload: payload.to_vec(),
        }
    }

    #[test]
    fn token_values_match_original_protocol_header() {
        assert_eq!(LegacyToken::Null as u32, 0);
        assert_eq!(LegacyToken::Score as u32, 10);
        assert_eq!(LegacyToken::Challenge as u32, 32);
        assert_eq!(LegacyToken::Accept as u32, 33);
        assert_eq!(LegacyToken::Deny as u32, 34);
        assert_eq!(LegacyToken::Start as u32, 35);
        assert_eq!(LegacyToken::Pause as u32, 50);
        assert_eq!(LegacyToken::QueryConnection as u32, 74);
        assert_eq!(LegacyToken::ResponsePlayerDb as u32, 84);
    }

    #[tokio::test]
    async fn legacy_packets_round_trip_empty_and_payload_packets() {
        let (mut client, mut server) = io::duplex(128);
        let written = tokio::spawn(async move {
            write_legacy_packet(&mut client, &LegacyPacket::empty(LegacyToken::Accept)).await?;
            write_legacy_packet(&mut client, &packet(LegacyToken::Challenge, b"abc")).await
        });

        assert_eq!(
            read_legacy_packet(&mut server)
                .await
                .expect("empty packet reads"),
            LegacyPacket::empty(LegacyToken::Accept)
        );
        assert_eq!(
            read_legacy_packet(&mut server)
                .await
                .expect("payload packet reads"),
            packet(LegacyToken::Challenge, b"abc")
        );
        written
            .await
            .expect("writer joins")
            .expect("writer succeeds");
    }

    #[tokio::test]
    async fn legacy_header_uses_two_native_c_ulong_fields() {
        let (mut writer, mut reader) = io::duplex(64);
        write_legacy_packet(&mut writer, &packet(LegacyToken::Challenge, b"xy"))
            .await
            .expect("packet writes");
        let mut bytes = vec![0; LEGACY_HEADER_LEN + 2];
        reader.read_exact(&mut bytes).await.expect("bytes read");
        assert_eq!(&bytes[..4], &(LegacyToken::Challenge as u32).to_be_bytes());
        assert!(bytes[4..LEGACY_C_ULONG_LEN].iter().all(|byte| *byte == 0));
        assert_eq!(
            &bytes[LEGACY_C_ULONG_LEN..LEGACY_C_ULONG_LEN + 4],
            &2_u32.to_be_bytes()
        );
        assert!(bytes[LEGACY_C_ULONG_LEN + 4..LEGACY_HEADER_LEN]
            .iter()
            .all(|byte| *byte == 0));
        assert_eq!(&bytes[LEGACY_HEADER_LEN..], b"xy");
    }

    #[tokio::test]
    async fn legacy_packet_reader_rejects_unknown_and_oversized_headers() {
        let (mut unknown_writer, mut unknown_reader) = io::duplex(32);
        write_legacy_c_ulong(&mut unknown_writer, 999)
            .await
            .unwrap();
        write_legacy_c_ulong(&mut unknown_writer, 0).await.unwrap();
        assert!(matches!(
            read_legacy_packet(&mut unknown_reader).await,
            Err(LegacyError::UnknownToken { token: 999 })
        ));

        let (mut large_writer, mut large_reader) = io::duplex(32);
        write_legacy_c_ulong(&mut large_writer, LegacyToken::Challenge as u32)
            .await
            .unwrap();
        write_legacy_c_ulong(&mut large_writer, LEGACY_MAX_PAYLOAD_LEN + 1)
            .await
            .unwrap();
        assert!(matches!(
            read_legacy_packet(&mut large_reader).await,
            Err(LegacyError::PayloadTooLarge { len }) if len == LEGACY_MAX_PAYLOAD_LEN + 1
        ));
    }

    #[tokio::test]
    async fn legacy_packet_reader_rejects_malformed_short_header() {
        let (mut writer, mut reader) = io::duplex(16);
        writer.write_all(&[0, 0, 0]).await.unwrap();
        drop(writer);
        assert!(matches!(
            read_legacy_packet(&mut reader).await,
            Err(LegacyError::Io(_))
        ));
    }

    #[test]
    fn network_entry_preserves_fixed_sizes_and_big_endian_fields() {
        let entry = LegacyNetworkEntry::waiting(
            PlayerIdentity {
                display_name: "Ada".to_string(),
            },
            "192.168.1.44:4405".parse().unwrap(),
            0x0102_0304,
            0x0506_0708,
        );
        let encoded = entry.encode();
        assert_eq!(encoded.len(), LEGACY_NETWORK_ENTRY_LEN);
        assert_eq!(&encoded[LEGACY_DB_KEY_LEN..LEGACY_DB_KEY_LEN + 3], b"Ada");
        let numbers_offset = LEGACY_DB_KEY_LEN + LEGACY_USERNAME_LEN + LEGACY_HOSTNAME_LEN;
        assert_eq!(
            &encoded[numbers_offset..numbers_offset + 4],
            &0x0506_0708_u32.to_be_bytes()
        );
        assert!(
            encoded[numbers_offset + 4..numbers_offset + LEGACY_C_ULONG_LEN]
                .iter()
                .all(|byte| *byte == 0)
        );
        let pid_offset = numbers_offset + LEGACY_C_ULONG_LEN;
        assert_eq!(
            &encoded[pid_offset..pid_offset + 4],
            &0x0102_0304_u32.to_be_bytes()
        );
        let addrnet_offset = numbers_offset + 2 * LEGACY_C_ULONG_LEN;
        assert_eq!(
            &encoded[addrnet_offset..addrnet_offset + 4],
            &0x00c0_a801_u32.to_be_bytes()
        );
        let addrlna_offset = numbers_offset + 3 * LEGACY_C_ULONG_LEN;
        assert_eq!(
            &encoded[addrlna_offset..addrlna_offset + 4],
            &44_u32.to_be_bytes()
        );
        let port_offset = numbers_offset + 4 * LEGACY_C_ULONG_LEN;
        assert_eq!(
            &encoded[port_offset..port_offset + 2],
            &4405_u16.to_be_bytes()
        );

        let decoded = LegacyNetworkEntry::decode(&encoded).expect("entry decodes");
        assert_eq!(decoded, entry);
    }

    #[test]
    fn network_entry_decode_rejects_short_payloads() {
        assert!(matches!(
            LegacyNetworkEntry::decode(&vec![0; LEGACY_NETWORK_ENTRY_LEN_32 - 1]),
            Err(LegacyError::EntryTooShort { .. })
        ));
    }

    #[tokio::test]
    async fn legacy_host_and_join_complete_accept_handshake() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let addr = listener.local_addr().expect("local addr");
        let host = tokio::spawn(async move {
            let pending = accept_pending_legacy_challenge(&listener).await?;
            assert_eq!(pending.challenger.user_name, "Joiner");
            pending.accept().await
        });
        let entry = LegacyNetworkEntry::waiting(
            PlayerIdentity {
                display_name: "Joiner".to_string(),
            },
            addr,
            7,
            8,
        );
        let joined = join_legacy_game(addr, entry).await.expect("join succeeds");
        let accepted = host.await.expect("host joins").expect("host accepts");
        assert_eq!(accepted.challenger.user_name, "Joiner");
        let _ = (accepted.connection, joined.connection);
    }

    #[tokio::test]
    async fn legacy_deny_maps_to_join_error() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
        let addr = listener.local_addr().expect("local addr");
        let host = tokio::spawn(async move {
            let pending = accept_pending_legacy_challenge(&listener).await?;
            pending.deny().await
        });
        let entry = LegacyNetworkEntry::waiting(
            PlayerIdentity {
                display_name: "Joiner".to_string(),
            },
            addr,
            7,
            8,
        );
        assert!(matches!(
            join_legacy_game(addr, entry).await,
            Err(LegacyError::ChallengeDenied)
        ));
        host.await.expect("host joins").expect("host denies");
    }

    #[test]
    fn legacy_score_payload_uses_six_big_endian_32_bit_fields() {
        let score = LegacyScorePayload {
            score: 0x0102_0304,
            opponent_score: 0x0506_0708,
            lines: 0x090a_0b0c,
            opponent_lines: 0x0d0e_0f10,
            funds: -2,
            opponent_funds: 0x1122_3344,
        };

        let encoded = score.encode();
        assert_eq!(encoded.len(), LEGACY_SCORE_PAYLOAD_LEN);
        assert_eq!(&encoded[0..4], &0x0102_0304_u32.to_be_bytes());
        assert_eq!(&encoded[16..20], &(-2_i32).to_be_bytes());
        assert_eq!(
            LegacyScorePayload::decode(&encoded).expect("score decodes"),
            score
        );
        assert!(matches!(
            LegacyScorePayload::decode(&encoded[..LEGACY_SCORE_PAYLOAD_LEN - 1]),
            Err(LegacyError::PayloadLengthMismatch { .. })
        ));
    }

    #[test]
    fn legacy_short_payload_round_trips_big_endian_signed_values() {
        let payload = LegacyShortPayload { value: -1234 };
        assert_eq!(payload.encode(), (-1234_i16).to_be_bytes());
        assert_eq!(
            LegacyShortPayload::decode(&payload.encode()).expect("short decodes"),
            payload
        );
        assert!(matches!(
            LegacyShortPayload::decode(&[0]),
            Err(LegacyError::PayloadLengthMismatch { .. })
        ));
    }

    #[test]
    fn legacy_board_payload_round_trips_and_validates_cell_count() {
        let board = LegacyBoardPayload::new(5, 2, 3, vec![0, 1, 2, 0xffff_fffe, 4, 5])
            .expect("board is valid");

        let encoded = board.encode().expect("board encodes");
        assert_eq!(&encoded[0..2], &5_u16.to_be_bytes());
        assert_eq!(&encoded[2..4], &2_u16.to_be_bytes());
        assert_eq!(&encoded[4..6], &3_u16.to_be_bytes());
        assert_eq!(&encoded[6..10], &0_u32.to_be_bytes());
        assert_eq!(&encoded[18..22], &0xffff_fffe_u32.to_be_bytes());
        assert_eq!(
            LegacyBoardPayload::decode(&encoded).expect("board decodes"),
            board
        );

        assert!(matches!(
            LegacyBoardPayload::new(0, 2, 3, vec![0; 5]),
            Err(LegacyError::InvalidBoardCellCount {
                expected: 6,
                actual: 5
            })
        ));
        assert!(matches!(
            LegacyBoardPayload::decode(&encoded[..encoded.len() - 1]),
            Err(LegacyError::PayloadLengthMismatch { .. })
        ));
    }

    #[test]
    fn legacy_arsenal_payload_round_trips_holes_and_quantities() {
        let mut slots = vec![
            LegacyArsenalSlot {
                weapon_token: LEGACY_NO_WEAPON_TOKEN,
                quantity: 0,
            };
            LEGACY_ARSENAL_SIZE
        ];
        slots[0] = LegacyArsenalSlot {
            weapon_token: 0,
            quantity: 2,
        };
        slots[9] = LegacyArsenalSlot {
            weapon_token: LEGACY_LAST_WEAPON_TOKEN,
            quantity: 1,
        };
        let arsenal = LegacyArsenalPayload::new(slots).expect("arsenal is valid");

        let encoded = arsenal.encode().expect("arsenal encodes");
        assert_eq!(&encoded[0..2], &(LEGACY_ARSENAL_SIZE as u16).to_be_bytes());
        assert_eq!(&encoded[2..4], &0_u16.to_be_bytes());
        assert_eq!(&encoded[4..6], &2_u16.to_be_bytes());
        assert_eq!(&encoded[6..8], &LEGACY_NO_WEAPON_TOKEN.to_be_bytes());
        assert_eq!(
            LegacyArsenalPayload::decode(&encoded).expect("arsenal decodes"),
            arsenal
        );
    }

    #[test]
    fn legacy_arsenal_payload_rejects_bad_lengths_counts_and_tokens() {
        assert!(matches!(
            LegacyArsenalPayload::decode(&[0]),
            Err(LegacyError::PayloadLengthMismatch { .. })
        ));
        assert!(matches!(
            LegacyArsenalPayload::decode(&[0, 1, 0, 2]),
            Err(LegacyError::PayloadLengthMismatch { .. })
        ));

        let mut too_many = vec![0; 2 + (LEGACY_ARSENAL_SIZE + 1) * 4];
        too_many[1] = (LEGACY_ARSENAL_SIZE + 1) as u8;
        assert!(matches!(
            LegacyArsenalPayload::decode(&too_many),
            Err(LegacyError::InvalidArsenalSlotCount {
                max: LEGACY_ARSENAL_SIZE,
                actual
            }) if actual == LEGACY_ARSENAL_SIZE + 1
        ));

        assert!(matches!(
            LegacyArsenalPayload::new(vec![LegacyArsenalSlot {
                weapon_token: LEGACY_MAX_WEAPON_SENTINEL,
                quantity: 1,
            }]),
            Err(LegacyError::InvalidWeaponToken {
                token: LEGACY_MAX_WEAPON_SENTINEL
            })
        ));
    }

    #[test]
    fn legacy_weapon_payload_round_trips_concrete_weapon_tokens() {
        for token in 0..=LEGACY_LAST_WEAPON_TOKEN {
            let payload = LegacyWeaponPayload::new(token).expect("weapon token is valid");
            assert_eq!(payload.encode(), token.to_be_bytes());
            assert_eq!(
                LegacyWeaponPayload::decode(&payload.encode()).expect("weapon decodes"),
                payload
            );
        }

        assert!(matches!(
            LegacyWeaponPayload::new(LEGACY_NO_WEAPON_TOKEN),
            Err(LegacyError::InvalidWeaponToken {
                token: LEGACY_NO_WEAPON_TOKEN
            })
        ));
        assert!(matches!(
            LegacyWeaponPayload::decode(&[0]),
            Err(LegacyError::PayloadLengthMismatch { .. })
        ));
    }

    #[test]
    fn legacy_empty_payload_validation_covers_control_packets() {
        validate_legacy_empty_payload(&[]).expect("empty payload is valid");
        assert!(matches!(
            validate_legacy_empty_payload(&[1]),
            Err(LegacyError::PayloadLengthMismatch {
                expected: 0,
                actual: 1,
                ..
            })
        ));
    }
}
