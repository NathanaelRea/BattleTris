//! Operator-facing self-hosted lobby and ranked-result authority.

use std::{env, fmt, net::SocketAddr, process, sync::Arc, time::Instant};

use battletris_db::{CommunityLabel, PersistencePaths, PlayerStore};
use battletris_protocol::{
    legacy::{
        read_legacy_packet, write_legacy_packet, LegacyNetworkEntry, LegacyPacket, LegacyToken,
        LEGACY_C_ULONG_LEN,
    },
    read_message_with_header, write_message, HostedSessionStatus, HostedSessionStatusKind,
    LobbyList, ProtocolError, RankedResultAccepted, RankedResultPending, RankedResultRejected,
    WireMessage,
};
use battletris_server::{
    HostedLobbyServer, LegacyRosterServer, VerificationOutcome, LEGACY_ROSTER_IDLE_TIMEOUT,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Mutex,
    time::timeout,
};

type DynError = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("battletris-server: {error}");
        process::exit(1);
    }
}

async fn run() -> Result<(), DynError> {
    let config = Config::from_env()?;
    if let Some(parent) = config.db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let modern_listener =
        bind_listener(Protocol::Modern, config.modern_listen, config.modern_source).await?;
    let legacy_listener =
        bind_listener(Protocol::Legacy, config.legacy_listen, config.legacy_source).await?;
    let modern_local = modern_listener.local_addr()?;
    let legacy_local = legacy_listener.local_addr()?;
    let state = Arc::new(ServerState {
        lobby: Mutex::new(HostedLobbyServer::new(config.community, config.first_seed)),
        legacy_roster: Mutex::new(LegacyRosterServer::new()),
        store: Mutex::new(PlayerStore::open(&config.db_path)?),
        next_legacy_connection_id: std::sync::atomic::AtomicU64::new(1),
    });

    eprintln!(
        "battletris-server protocol=modern local={} event=bind db={}",
        modern_local,
        config.db_path.display()
    );
    eprintln!(
        "battletris-server protocol=legacy local={} event=bind",
        legacy_local
    );

    let modern_state = Arc::clone(&state);
    let modern_task = tokio::spawn(async move {
        loop {
            let (stream, peer) = modern_listener.accept().await?;
            eprintln!(
                "battletris-server protocol=modern local={modern_local} peer={peer} event=accept"
            );
            let state = Arc::clone(&modern_state);
            tokio::spawn(async move {
                if let Err(error) =
                    handle_modern_connection(stream, state, modern_local, peer).await
                {
                    eprintln!("battletris-server protocol=modern local={modern_local} peer={peer} event=error error={error}");
                }
            });
        }
        #[allow(unreachable_code)]
        Ok::<(), std::io::Error>(())
    });

    let legacy_state = Arc::clone(&state);
    let legacy_task = tokio::spawn(async move {
        loop {
            let (stream, peer) = legacy_listener.accept().await?;
            eprintln!(
                "battletris-server protocol=legacy local={legacy_local} peer={peer} event=accept"
            );
            let state = Arc::clone(&legacy_state);
            tokio::spawn(async move {
                if let Err(error) =
                    handle_legacy_connection(stream, state, legacy_local, peer).await
                {
                    eprintln!("battletris-server protocol=legacy local={legacy_local} peer={peer} event=error error={error}");
                }
            });
        }
        #[allow(unreachable_code)]
        Ok::<(), std::io::Error>(())
    });

    tokio::select! {
        result = modern_task => {
            result??;
        }
        result = legacy_task => {
            result??;
        }
        result = tokio::signal::ctrl_c() => {
            result?;
            eprintln!("battletris-server event=shutdown reason=ctrl-c");
        }
    }

    Ok(())
}

async fn handle_legacy_connection(
    mut stream: TcpStream,
    state: Arc<ServerState>,
    local: SocketAddr,
    peer: SocketAddr,
) -> Result<(), DynError> {
    let connection_id = state
        .next_legacy_connection_id
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let result =
        handle_registered_legacy_connection(&mut stream, &state, connection_id, local, peer).await;
    if state.legacy_roster.lock().await.remove_owner(connection_id) {
        eprintln!("battletris-server protocol=legacy local={local} peer={peer} event=disconnect action=removed-roster-entry");
    } else {
        eprintln!("battletris-server protocol=legacy local={local} peer={peer} event=disconnect");
    }
    result
}

async fn handle_registered_legacy_connection(
    stream: &mut TcpStream,
    state: &Arc<ServerState>,
    connection_id: u64,
    local: SocketAddr,
    peer: SocketAddr,
) -> Result<(), DynError> {
    write_legacy_packet(stream, &LegacyPacket::empty(LegacyToken::Accepted)).await?;
    eprintln!("battletris-server protocol=legacy local={local} peer={peer} event=registration stage=accepted-sent");

    let first_packet = read_legacy_packet_with_timeout(stream, local, peer, "registration").await?;
    if first_packet.token != LegacyToken::QueryConnection {
        write_legacy_packet(stream, &LegacyPacket::empty(LegacyToken::Rejected)).await?;
        eprintln!("battletris-server protocol=legacy local={local} peer={peer} event=malformed-packet stage=registration token={:?}", first_packet.token);
        return Err(format!("expected BT_QUER_CONN, got {:?}", first_packet.token).into());
    }
    let entry = match LegacyNetworkEntry::decode(&first_packet.payload) {
        Ok(entry) => entry,
        Err(error) => {
            write_legacy_packet(stream, &LegacyPacket::empty(LegacyToken::Rejected)).await?;
            eprintln!("battletris-server protocol=legacy local={local} peer={peer} event=malformed-packet stage=registration error={error}");
            return Err(format!("malformed legacy registration: {error}").into());
        }
    };
    {
        let mut roster = state.legacy_roster.lock().await;
        let key = roster.register(connection_id, entry, Instant::now())?;
        eprintln!("battletris-server protocol=legacy local={local} peer={peer} event=registration key={key:?}");
    }

    loop {
        let packet = read_legacy_packet_with_timeout(stream, local, peer, "idle").await?;
        eprintln!(
            "battletris-server protocol=legacy local={local} peer={peer} event=query token={:?}",
            packet.token
        );
        let now = Instant::now();
        let mut roster = state.legacy_roster.lock().await;
        let expired = roster.expire_idle(now, LEGACY_ROSTER_IDLE_TIMEOUT);
        if expired > 0 {
            eprintln!("battletris-server protocol=legacy local={local} peer={peer} event=timeout action=expired-idle count={expired}");
        }
        roster.refresh_owner(connection_id, now);

        match packet.token {
            LegacyToken::QueryConnection => {
                let entry = match LegacyNetworkEntry::decode(&packet.payload) {
                    Ok(entry) => entry,
                    Err(error) => {
                        eprintln!("battletris-server protocol=legacy local={local} peer={peer} event=malformed-packet token={:?} error={error}", packet.token);
                        return Err(error.into());
                    }
                };
                let key = roster.register(connection_id, entry, now)?;
                eprintln!("battletris-server protocol=legacy local={local} peer={peer} event=registration key={key:?}");
            }
            LegacyToken::QueryNetworkDb => {
                let entries = roster.entries();
                drop(roster);
                write_legacy_db_len(stream, entries.len()).await?;
                let mut payload = Vec::new();
                for entry in entries {
                    payload.extend_from_slice(&entry.encode());
                }
                write_legacy_packet(
                    stream,
                    &LegacyPacket {
                        token: LegacyToken::ResponseNetworkDb,
                        payload,
                    },
                )
                .await?;
            }
            LegacyToken::QueryPlayerDb => {
                drop(roster);
                write_legacy_db_len(stream, 0).await?;
                write_legacy_packet(
                    stream,
                    &LegacyPacket {
                        token: LegacyToken::ResponsePlayerDb,
                        payload: Vec::new(),
                    },
                )
                .await?;
            }
            LegacyToken::QueryVerify => {
                let verified = roster.verify_key(&packet.payload);
                drop(roster);
                let value: u16 = if verified { 1 } else { 0 };
                write_legacy_packet(
                    stream,
                    &LegacyPacket {
                        token: LegacyToken::ResponseVerify,
                        payload: value.to_be_bytes().to_vec(),
                    },
                )
                .await?;
            }
            LegacyToken::QueryUpdate => {
                roster.update_owner_status(connection_id, now)?;
            }
            LegacyToken::QueryResult => {
                // Compatibility no-op: original clients submit results through this channel.
            }
            LegacyToken::Disconnect => return Ok(()),
            token => {
                eprintln!("battletris-server protocol=legacy local={local} peer={peer} event=malformed-packet token={token:?}");
                return Err(format!("unsupported legacy roster token {token:?}").into());
            }
        }
    }
}

async fn read_legacy_packet_with_timeout(
    stream: &mut TcpStream,
    local: SocketAddr,
    peer: SocketAddr,
    stage: &'static str,
) -> Result<LegacyPacket, DynError> {
    match timeout(LEGACY_ROSTER_IDLE_TIMEOUT, read_legacy_packet(stream)).await {
        Ok(Ok(packet)) => Ok(packet),
        Ok(Err(error)) => {
            eprintln!("battletris-server protocol=legacy local={local} peer={peer} event=malformed-packet stage={stage} error={error}");
            Err(error.into())
        }
        Err(_) => {
            eprintln!("battletris-server protocol=legacy local={local} peer={peer} event=timeout stage={stage}");
            Err("legacy connection idle timeout".into())
        }
    }
}

async fn write_legacy_db_len(stream: &mut TcpStream, count: usize) -> Result<(), DynError> {
    let mut payload = Vec::with_capacity(LEGACY_C_ULONG_LEN);
    payload.extend_from_slice(&(count as u32).to_be_bytes());
    payload.resize(LEGACY_C_ULONG_LEN, 0);
    write_legacy_packet(
        stream,
        &LegacyPacket {
            token: LegacyToken::ResponseDbLen,
            payload,
        },
    )
    .await?;
    Ok(())
}

async fn handle_modern_connection(
    mut stream: TcpStream,
    state: Arc<ServerState>,
    local: SocketAddr,
    peer: SocketAddr,
) -> Result<(), DynError> {
    let (header, message) = match timeout(
        LEGACY_ROSTER_IDLE_TIMEOUT,
        read_message_with_header(&mut stream),
    )
    .await
    {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => {
            let event = match error {
                ProtocolError::BadMagic { .. } => {
                    "malformed-packet possible-legacy-client-on-modern-port"
                }
                _ => "malformed-packet",
            };
            eprintln!("battletris-server protocol=modern local={local} peer={peer} event={event} error={error:?}");
            return Err(format!("protocol read failed: {error:?}").into());
        }
        Err(_) => {
            eprintln!("battletris-server protocol=modern local={local} peer={peer} event=timeout stage=handshake");
            return Err("modern handshake idle timeout".into());
        }
    };
    eprintln!(
        "battletris-server protocol=modern local={local} peer={peer} event=query kind={:?}",
        message.kind()
    );
    let reply = match message {
        WireMessage::LobbyRegister(request) => {
            let mut lobby = state.lobby.lock().await;
            match lobby.register_host(request, header.major, header.minor) {
                Ok(entry) => WireMessage::LobbyList(LobbyList {
                    entries: vec![entry],
                }),
                Err(error) => WireMessage::RankedResultRejected(RankedResultRejected {
                    session_id: None,
                    reason: error.to_string(),
                }),
            }
        }
        WireMessage::LobbyListRequest(request) => {
            let lobby = state.lobby.lock().await;
            WireMessage::LobbyList(LobbyList {
                entries: lobby.lobby_entries(request.ranked_only),
            })
        }
        WireMessage::RankedRecordsRequest(request) => {
            let lobby = state.lobby.lock().await;
            let store = state.store.lock().await;
            match lobby.ranked_records(&store, request.limit) {
                Ok(records) => WireMessage::RankedRecords(records),
                Err(error) => WireMessage::RankedResultRejected(RankedResultRejected {
                    session_id: None,
                    reason: error.to_string(),
                }),
            }
        }
        WireMessage::HostedJoinRequest(request) => {
            let mut lobby = state.lobby.lock().await;
            let store = state.store.lock().await;
            match lobby.start_game(&request.session_id, request.joiner, header.major, &store) {
                Ok(start) => WireMessage::HostedGameStart(start),
                Err(error) => WireMessage::RankedResultRejected(RankedResultRejected {
                    session_id: Some(request.session_id),
                    reason: error.to_string(),
                }),
            }
        }
        WireMessage::HostedSessionStatusRequest(request) => {
            let lobby = state.lobby.lock().await;
            match lobby.session_status(&request.session_id, &request.requester_player_id) {
                Ok(status) => WireMessage::HostedSessionStatus(status),
                Err(error) => WireMessage::RankedResultRejected(RankedResultRejected {
                    session_id: Some(request.session_id),
                    reason: error.to_string(),
                }),
            }
        }
        WireMessage::HostedSessionCancel(request) => {
            let mut lobby = state.lobby.lock().await;
            match lobby.cancel_session(&request.session_id, &request.requester_player_id) {
                Ok(()) => WireMessage::HostedSessionStatus(HostedSessionStatus {
                    session_id: request.session_id,
                    status: HostedSessionStatusKind::Unavailable {
                        reason: "session canceled".to_string(),
                    },
                }),
                Err(error) => WireMessage::RankedResultRejected(RankedResultRejected {
                    session_id: Some(request.session_id),
                    reason: error.to_string(),
                }),
            }
        }
        WireMessage::RankedResultClaim(claim) => {
            let session_id = claim.session_id.clone();
            let mut lobby = state.lobby.lock().await;
            let mut store = state.store.lock().await;
            match lobby.submit_ranked_result(claim, &mut store) {
                Ok(VerificationOutcome::Recorded) => {
                    WireMessage::RankedResultAccepted(RankedResultAccepted { session_id })
                }
                Ok(VerificationOutcome::AwaitingPeer) => {
                    WireMessage::RankedResultPending(RankedResultPending {
                        session_id,
                        reason: "awaiting matching peer result claim".to_string(),
                    })
                }
                Err(error) => WireMessage::RankedResultRejected(RankedResultRejected {
                    session_id: Some(session_id),
                    reason: error.to_string(),
                }),
            }
        }
        _ => WireMessage::RankedResultRejected(RankedResultRejected {
            session_id: None,
            reason: "unsupported server message".to_string(),
        }),
    };
    write_message(&mut stream, &reply)
        .await
        .map_err(|error| format!("protocol write failed: {error:?}"))?;
    Ok(())
}

struct ServerState {
    lobby: Mutex<HostedLobbyServer>,
    legacy_roster: Mutex<LegacyRosterServer>,
    store: Mutex<PlayerStore>,
    next_legacy_connection_id: std::sync::atomic::AtomicU64,
}

struct Config {
    modern_listen: SocketAddr,
    legacy_listen: SocketAddr,
    modern_source: ListenSource,
    legacy_source: ListenSource,
    db_path: std::path::PathBuf,
    community: CommunityLabel,
    first_seed: u64,
}

impl Config {
    fn from_env() -> Result<Self, DynError> {
        Self::from_args(env::args().skip(1))
    }

    fn from_args(args: impl IntoIterator<Item = String>) -> Result<Self, DynError> {
        let mut args = args.into_iter();
        let mut modern_listen = "127.0.0.1:4405".parse()?;
        let mut legacy_listen = "0.0.0.0:4404".parse()?;
        let mut modern_source = ListenSource::Default;
        let mut legacy_source = ListenSource::Default;
        let mut db_path = PersistencePaths::new()?.player_db_file;
        let mut community = CommunityLabel::local();
        let mut first_seed = 1;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--modern-listen" => {
                    modern_listen = required_value(&mut args, "--modern-listen")?.parse()?;
                    modern_source = ListenSource::Flag;
                }
                "--legacy-listen" => {
                    legacy_listen = required_value(&mut args, "--legacy-listen")?.parse()?;
                    legacy_source = ListenSource::Flag;
                }
                "--listen" => {
                    return Err(
                        "--listen is no longer supported; use --modern-listen or --legacy-listen"
                            .into(),
                    )
                }
                "--db" => db_path = required_value(&mut args, "--db")?.into(),
                "--community" => {
                    community = CommunityLabel::new(required_value(&mut args, "--community")?)?
                }
                "--seed" => first_seed = required_value(&mut args, "--seed")?.parse()?,
                "--help" | "-h" => {
                    print_help();
                    process::exit(0);
                }
                other => return Err(format!("unknown argument {other}").into()),
            }
        }

        Ok(Self {
            modern_listen,
            legacy_listen,
            modern_source,
            legacy_source,
            db_path,
            community,
            first_seed,
        })
    }
}

#[derive(Clone, Copy)]
enum Protocol {
    Modern,
    Legacy,
}

impl fmt::Display for Protocol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Modern => formatter.write_str("modern"),
            Self::Legacy => formatter.write_str("legacy"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListenSource {
    Default,
    Flag,
}

impl fmt::Display for ListenSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => formatter.write_str("default"),
            Self::Flag => formatter.write_str("flag"),
        }
    }
}

async fn bind_listener(
    protocol: Protocol,
    addr: SocketAddr,
    source: ListenSource,
) -> Result<TcpListener, DynError> {
    TcpListener::bind(addr).await.map_err(|error| {
        format!("{protocol} listener bind {addr} failed (enabled by {source}): {error}").into()
    })
}

fn required_value(
    args: &mut impl Iterator<Item = String>,
    flag: &'static str,
) -> Result<String, DynError> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value").into())
}

fn print_help() {
    println!(
        "Usage: battletris-server [--modern-listen ADDR:PORT] [--legacy-listen ADDR:PORT] [--db PATH] [--community LABEL] [--seed N]"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_uses_phase_four_default_listeners() {
        let config = Config::from_args(Vec::new()).expect("config parses");

        assert_eq!(
            config.modern_listen,
            "127.0.0.1:4405".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            config.legacy_listen,
            "0.0.0.0:4404".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(config.modern_source, ListenSource::Default);
        assert_eq!(config.legacy_source, ListenSource::Default);
    }

    #[test]
    fn config_accepts_protocol_specific_listener_flags() {
        let config = Config::from_args([
            "--modern-listen".to_string(),
            "127.0.0.1:5505".to_string(),
            "--legacy-listen".to_string(),
            "127.0.0.1:5504".to_string(),
        ])
        .expect("config parses");

        assert_eq!(
            config.modern_listen,
            "127.0.0.1:5505".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            config.legacy_listen,
            "127.0.0.1:5504".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(config.modern_source, ListenSource::Flag);
        assert_eq!(config.legacy_source, ListenSource::Flag);
    }

    #[test]
    fn config_rejects_ambiguous_listen_flag() {
        let Err(error) = Config::from_args(["--listen".to_string(), "127.0.0.1:4405".to_string()])
        else {
            panic!("--listen should be rejected");
        };

        assert!(error.to_string().contains("--modern-listen"));
        assert!(error.to_string().contains("--legacy-listen"));
    }
}
