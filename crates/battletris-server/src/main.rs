//! Operator-facing self-hosted lobby and ranked-result authority.

use std::{env, net::SocketAddr, process, sync::Arc};

use battletris_db::{CommunityLabel, PersistencePaths, PlayerStore};
use battletris_protocol::{
    read_message, write_message, LobbyList, RankedResultAccepted, RankedResultRejected,
    WireMessage, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};
use battletris_server::{HostedLobbyServer, VerificationOutcome};
use tokio::{net::TcpListener, sync::Mutex};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("battletris-server: {error}");
        process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    if let Some(parent) = config.db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = TcpListener::bind(config.listen).await?;
    let state = Arc::new(ServerState {
        lobby: Mutex::new(HostedLobbyServer::new(config.community, config.first_seed)),
        store: Mutex::new(PlayerStore::open(&config.db_path)?),
    });

    eprintln!(
        "battletris-server listening on {} with records at {}",
        listener.local_addr()?,
        config.db_path.display()
    );

    loop {
        let (stream, peer) = listener.accept().await?;
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, state).await {
                eprintln!("battletris-server: {peer}: {error}");
            }
        });
    }
}

async fn handle_connection(
    mut stream: tokio::net::TcpStream,
    state: Arc<ServerState>,
) -> Result<(), Box<dyn std::error::Error>> {
    let message = read_message(&mut stream)
        .await
        .map_err(|error| format!("protocol read failed: {error:?}"))?;
    let reply = match message {
        WireMessage::LobbyRegister(request) => {
            let mut lobby = state.lobby.lock().await;
            match lobby.register_host(request, PROTOCOL_MAJOR, PROTOCOL_MINOR) {
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
        WireMessage::RankedResultClaim(claim) => {
            let session_id = claim.session_id.clone();
            let mut lobby = state.lobby.lock().await;
            let mut store = state.store.lock().await;
            match lobby.submit_ranked_result(claim, &mut store) {
                Ok(VerificationOutcome::Recorded) => {
                    WireMessage::RankedResultAccepted(RankedResultAccepted { session_id })
                }
                Ok(VerificationOutcome::AwaitingPeer) => {
                    WireMessage::RankedResultRejected(RankedResultRejected {
                        session_id: Some(session_id),
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
    store: Mutex<PlayerStore>,
}

struct Config {
    listen: SocketAddr,
    db_path: std::path::PathBuf,
    community: CommunityLabel,
    first_seed: u64,
}

impl Config {
    fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let mut args = env::args().skip(1);
        let mut listen = "127.0.0.1:4404".parse()?;
        let mut db_path = PersistencePaths::new()?.player_db_file;
        let mut community = CommunityLabel::local();
        let mut first_seed = 1;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--listen" => listen = required_value(&mut args, "--listen")?.parse()?,
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
            listen,
            db_path,
            community,
            first_seed,
        })
    }
}

fn required_value(
    args: &mut impl Iterator<Item = String>,
    flag: &'static str,
) -> Result<String, Box<dyn std::error::Error>> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value").into())
}

fn print_help() {
    println!(
        "Usage: battletris-server [--listen ADDR:PORT] [--db PATH] [--community LABEL] [--seed N]"
    );
}
