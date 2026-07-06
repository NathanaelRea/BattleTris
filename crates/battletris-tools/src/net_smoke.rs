//! Headless networking smoke commands.

use std::{fmt::Write as _, hash::Hasher, net::SocketAddr, process, time::Duration};

use battletris_core::{
    game::{Command, TwoPlayerGame},
    rng::GameSeed,
};
use battletris_protocol::{
    accept_direct_game, derive_player_seeds, join_direct_game,
    legacy::{
        LegacyConnection, LegacyNetworkEntry, LegacyPacket, LegacyToken, LEGACY_C_ULONG_LEN,
        LEGACY_NETWORK_ENTRY_LEN,
    },
    read_message, write_message, DirectConnection, Disconnect, GameChecksum, Hello,
    HostedJoinRequest, HostedPlayer, HostedSessionStatus, HostedSessionStatusKind,
    HostedSessionStatusRequest, InputCommand, LobbyEntry, LobbyList, LobbyListRequest,
    LobbyRegister, PlayerIdentity, PlayerInput, PlayerSlot, RankedRecordsRequest,
    RankedResultAccepted, RankedResultClaim, RankedResultPending, RankedResultRejected,
    TickWatermark, WireMessage, CAPABILITY_DIRECT_TCP, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};
use tokio::{
    net::{TcpListener, TcpStream},
    time::timeout,
};

const SMOKE_TIMEOUT: Duration = Duration::from_secs(10);
const SCRIPT_SEED: u64 = 0x5eed_0000_0000_0002;
const SCRIPT_TICK_MS: u64 = 10;

/// Runs the `net-smoke` command group.
pub fn run(args: Vec<String>) {
    if args.is_empty() || args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return;
    }

    let result = tokio::runtime::Runtime::new()
        .expect("net-smoke runtime starts")
        .block_on(async { run_async(args).await });
    if let Err(error) = result {
        eprintln!("net-smoke: {error}");
        process::exit(1);
    }
}

async fn run_async(mut args: Vec<String>) -> Result<(), String> {
    let command = args.remove(0);
    match command.as_str() {
        "direct-loopback" => direct_loopback().await,
        "direct-host" => {
            let listen = parse_flag_addr(&mut args, "--listen")?;
            ensure_no_extra_args(&args)?;
            direct_host(listen).await
        }
        "direct-join" => {
            let addr = parse_flag_addr(&mut args, "--addr")?;
            ensure_no_extra_args(&args)?;
            let checksum = direct_join(addr).await?;
            println!("direct-join ok checksum={checksum:#018x}");
            Ok(())
        }
        "hosted-lobby" => {
            let server = parse_flag_addr(&mut args, "--server")?;
            ensure_no_extra_args(&args)?;
            hosted_lobby(server).await.map(|_| ())
        }
        "legacy-roster" => {
            let server = parse_flag_addr(&mut args, "--server")?;
            let share = parse_flag_addr(&mut args, "--share")?;
            ensure_no_extra_args(&args)?;
            legacy_roster(server, share).await
        }
        "ranked-result" => {
            let server = parse_flag_addr(&mut args, "--server")?;
            ensure_no_extra_args(&args)?;
            ranked_result(server).await
        }
        other => Err(format!("unknown net-smoke command {other}")),
    }
}

async fn direct_loopback() -> Result<(), String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|error| format!("bind loopback listener failed: {error}"))?;
    let addr = listener
        .local_addr()
        .map_err(|error| format!("read loopback listener address failed: {error}"))?;
    let host = tokio::spawn(async move { host_with_listener(listener).await });
    let join = tokio::spawn(async move { direct_join(addr).await });

    let host_checksum = join_task(host, "direct host").await?;
    let join_checksum = join_task(join, "direct join").await?;
    if host_checksum != join_checksum {
        return Err(format!(
            "loopback checksum mismatch: host {host_checksum:#018x}, join {join_checksum:#018x}"
        ));
    }
    println!("direct-loopback ok checksum={host_checksum:#018x}");
    Ok(())
}

async fn direct_host(listen: SocketAddr) -> Result<(), String> {
    let listener = TcpListener::bind(listen)
        .await
        .map_err(|error| format!("bind {listen} failed: {error}"))?;
    let actual = listener
        .local_addr()
        .map_err(|error| format!("read listener address failed: {error}"))?;
    eprintln!("net-smoke direct-host listening on {actual}");
    let checksum = host_with_listener(listener).await?;
    println!("direct-host ok checksum={checksum:#018x}");
    Ok(())
}

async fn host_with_listener(listener: TcpListener) -> Result<u64, String> {
    let accepted = timeout(
        SMOKE_TIMEOUT,
        accept_direct_game(&listener, identity("Smoke Host"), SCRIPT_SEED, false),
    )
    .await
    .map_err(|_| "direct host timed out waiting for challenge".to_string())?
    .map_err(|error| format!("direct host handshake failed: {error:?}"))?;
    run_direct_script(accepted.connection, PlayerSlot::One).await
}

async fn direct_join(addr: SocketAddr) -> Result<u64, String> {
    let joined = timeout(
        SMOKE_TIMEOUT,
        join_direct_game(addr, identity("Smoke Join"), "net-smoke".to_string()),
    )
    .await
    .map_err(|_| format!("direct join timed out connecting to {addr}"))?
    .map_err(|error| format!("direct join handshake failed: {error:?}"))?;
    if joined.start.seed != SCRIPT_SEED || joined.start.receiving_peer_slot != PlayerSlot::Two {
        return Err("direct join received unexpected start metadata".to_string());
    }
    run_direct_script(joined.connection, PlayerSlot::Two).await
}

async fn run_direct_script(
    mut connection: DirectConnection,
    local_slot: PlayerSlot,
) -> Result<u64, String> {
    let mut game = game_from_seed(SCRIPT_SEED);
    let peer_slot = opposite_slot(local_slot);

    for tick in 0..8 {
        let local = scripted_input(local_slot, tick);
        connection
            .send(&WireMessage::PlayerInput(local.clone()))
            .await
            .map_err(|error| format!("send player input failed: {error:?}"))?;
        let remote = recv_player_input(&mut connection, peer_slot, tick).await?;

        apply_ordered_inputs(&mut game, local, remote)?;
        game.tick_player(core_player(PlayerSlot::One), SCRIPT_TICK_MS);
        game.tick_player(core_player(PlayerSlot::Two), SCRIPT_TICK_MS);

        connection
            .send(&WireMessage::TickWatermark(TickWatermark {
                player: local_slot,
                through_tick: tick,
            }))
            .await
            .map_err(|error| format!("send tick watermark failed: {error:?}"))?;
        let remote_watermark = recv_tick_watermark(&mut connection, peer_slot).await?;
        if remote_watermark.through_tick != tick {
            return Err(format!(
                "peer watermark mismatch at tick {tick}: {}",
                remote_watermark.through_tick
            ));
        }
    }

    let checksum = checksum_game(&game);
    connection
        .send(&WireMessage::GameChecksum(GameChecksum {
            reporter: local_slot,
            tick: 7,
            checksum,
            event_count: game.event_log().len() as u64,
        }))
        .await
        .map_err(|error| format!("send checksum failed: {error:?}"))?;
    let remote_checksum = recv_checksum(&mut connection, peer_slot).await?;
    if remote_checksum.checksum != checksum
        || remote_checksum.event_count != game.event_log().len() as u64
    {
        return Err(format!(
            "peer checksum mismatch: local {checksum:#018x}/{} remote {:#018x}/{}",
            game.event_log().len(),
            remote_checksum.checksum,
            remote_checksum.event_count
        ));
    }

    connection
        .send(&WireMessage::Disconnect(Disconnect {
            reason: "net-smoke complete".to_string(),
        }))
        .await
        .map_err(|error| format!("send disconnect failed: {error:?}"))?;
    recv_disconnect(&mut connection).await?;
    Ok(checksum)
}

async fn recv_player_input(
    connection: &mut DirectConnection,
    player: PlayerSlot,
    tick: u64,
) -> Result<PlayerInput, String> {
    match timeout(SMOKE_TIMEOUT, connection.recv())
        .await
        .map_err(|_| "timed out waiting for player input".to_string())?
        .map_err(|error| format!("receive player input failed: {error:?}"))?
    {
        WireMessage::PlayerInput(input) if input.player == player && input.tick == tick => {
            Ok(input)
        }
        WireMessage::PlayerInput(input) => Err(format!(
            "unexpected player input {:?} at tick {}, expected {:?} at tick {tick}",
            input.player, input.tick, player
        )),
        message => Err(format!("expected player input, got {:?}", message.kind())),
    }
}

async fn recv_tick_watermark(
    connection: &mut DirectConnection,
    player: PlayerSlot,
) -> Result<TickWatermark, String> {
    match timeout(SMOKE_TIMEOUT, connection.recv())
        .await
        .map_err(|_| "timed out waiting for tick watermark".to_string())?
        .map_err(|error| format!("receive tick watermark failed: {error:?}"))?
    {
        WireMessage::TickWatermark(watermark) if watermark.player == player => Ok(watermark),
        message => Err(format!("expected tick watermark, got {:?}", message.kind())),
    }
}

async fn recv_checksum(
    connection: &mut DirectConnection,
    reporter: PlayerSlot,
) -> Result<GameChecksum, String> {
    match timeout(SMOKE_TIMEOUT, connection.recv())
        .await
        .map_err(|_| "timed out waiting for checksum".to_string())?
        .map_err(|error| format!("receive checksum failed: {error:?}"))?
    {
        WireMessage::GameChecksum(checksum) if checksum.reporter == reporter => Ok(checksum),
        message => Err(format!("expected checksum, got {:?}", message.kind())),
    }
}

async fn recv_disconnect(connection: &mut DirectConnection) -> Result<(), String> {
    match timeout(SMOKE_TIMEOUT, connection.recv())
        .await
        .map_err(|_| "timed out waiting for disconnect".to_string())?
        .map_err(|error| format!("receive disconnect failed: {error:?}"))?
    {
        WireMessage::Disconnect(_) => Ok(()),
        message => Err(format!("expected disconnect, got {:?}", message.kind())),
    }
}

async fn hosted_lobby(server: SocketAddr) -> Result<HostedGameStartPair, String> {
    let entry = register_host(server, "ada", "Ada", true).await?;
    let listed = request_lobby_list(server, false).await?;
    if !listed
        .entries
        .iter()
        .any(|candidate| candidate.session_id == entry.session_id)
    {
        return Err("registered hosted session was not listed".to_string());
    }

    let join_start = request_hosted_join(server, &entry, player("ben", "Ben")).await?;
    let host_status = request_hosted_status(server, &entry, "ada").await?;
    let HostedSessionStatusKind::Started(host_start) = host_status.status else {
        return Err("host status did not report started session".to_string());
    };
    if host_start != join_start {
        return Err("host and joiner received different hosted start metadata".to_string());
    }
    println!(
        "hosted-lobby ok session={} seed={} ranked={}",
        join_start.session_id.0, join_start.seed, join_start.ranked
    );
    Ok(HostedGameStartPair { entry, join_start })
}

async fn legacy_roster(server: SocketAddr, share: SocketAddr) -> Result<(), String> {
    let stream = timeout(SMOKE_TIMEOUT, TcpStream::connect(server))
        .await
        .map_err(|_| format!("timed out connecting to legacy server {server}"))?
        .map_err(|error| format!("connect to legacy server {server} failed: {error}"))?;
    let mut connection = LegacyConnection::from_stream(stream);
    let accepted = timeout(SMOKE_TIMEOUT, connection.recv())
        .await
        .map_err(|_| "timed out waiting for legacy server accept".to_string())?
        .map_err(|error| format!("legacy server accept read failed: {error}"))?;
    if accepted.token != LegacyToken::Accepted {
        return Err(format!("expected BT_ACCEPTED, got {:?}", accepted.token));
    }

    let entry = LegacyNetworkEntry::waiting(identity("Smoke Legacy"), share, std::process::id(), 1);
    connection
        .send(&LegacyPacket {
            token: LegacyToken::QueryConnection,
            payload: entry.encode(),
        })
        .await
        .map_err(|error| format!("legacy register failed: {error}"))?;
    connection
        .send(&LegacyPacket::empty(LegacyToken::QueryNetworkDb))
        .await
        .map_err(|error| format!("legacy roster query failed: {error}"))?;
    let count_packet = connection
        .recv()
        .await
        .map_err(|error| format!("legacy roster length read failed: {error}"))?;
    if count_packet.token != LegacyToken::ResponseDbLen {
        return Err(format!(
            "expected BT_RESP_DBLEN, got {:?}",
            count_packet.token
        ));
    }
    if count_packet.payload.len() != 4 && count_packet.payload.len() != LEGACY_C_ULONG_LEN {
        return Err(format!(
            "unexpected legacy roster length payload size {}",
            count_packet.payload.len()
        ));
    }
    let count = u32::from_be_bytes(count_packet.payload[..4].try_into().unwrap()) as usize;
    let db_packet = connection
        .recv()
        .await
        .map_err(|error| format!("legacy roster read failed: {error}"))?;
    if db_packet.token != LegacyToken::ResponseNetworkDb {
        return Err(format!("expected BT_RESP_NETDB, got {:?}", db_packet.token));
    }
    let expected_len = count * LEGACY_NETWORK_ENTRY_LEN;
    if db_packet.payload.len() != expected_len {
        return Err(format!(
            "legacy roster payload size mismatch: expected {expected_len}, got {}",
            db_packet.payload.len()
        ));
    }
    println!("legacy-roster ok entries={count}");
    for chunk in db_packet.payload.chunks_exact(LEGACY_NETWORK_ENTRY_LEN) {
        let entry = LegacyNetworkEntry::decode(chunk)
            .map_err(|error| format!("legacy roster entry decode failed: {error}"))?;
        println!("{} {}:{}", entry.user_name, entry.host_name, entry.port);
    }
    connection
        .send(&LegacyPacket::empty(LegacyToken::Disconnect))
        .await
        .map_err(|error| format!("legacy disconnect failed: {error}"))?;
    Ok(())
}

async fn ranked_result(server: SocketAddr) -> Result<(), String> {
    let started = hosted_lobby(server).await?;
    let claim = claim_for(&started.join_start, "ada", 0x1234);
    match server_request(server, WireMessage::RankedResultClaim(claim.clone())).await? {
        WireMessage::RankedResultPending(RankedResultPending { session_id, .. })
            if session_id == started.entry.session_id => {}
        message => return Err(format!("expected pending result, got {:?}", message.kind())),
    }
    match server_request(
        server,
        WireMessage::RankedResultClaim(RankedResultClaim {
            reporter_player_id: "ben".to_string(),
            ..claim
        }),
    )
    .await?
    {
        WireMessage::RankedResultAccepted(RankedResultAccepted { session_id })
            if session_id == started.entry.session_id => {}
        message => {
            return Err(format!(
                "expected accepted result, got {:?}",
                message.kind()
            ))
        }
    }
    match server_request(
        server,
        WireMessage::RankedRecordsRequest(RankedRecordsRequest { limit: 10 }),
    )
    .await?
    {
        WireMessage::RankedRecords(records)
            if records
                .records
                .iter()
                .any(|record| record.player_id == "ada" && record.wins == 1) => {}
        message => return Err(format!("expected ranked records, got {:?}", message.kind())),
    }

    let mismatch = hosted_lobby(server).await?;
    let first = claim_for(&mismatch.join_start, "ada", 0x5678);
    let mut second = claim_for(&mismatch.join_start, "ben", 0x5678);
    second.winner_score += 1;
    expect_pending(server, first).await?;
    match server_request(server, WireMessage::RankedResultClaim(second)).await? {
        WireMessage::RankedResultRejected(RankedResultRejected { reason, .. })
            if reason.contains("claims do not match") => {}
        message => {
            return Err(format!(
                "expected mismatched claim rejection, got {:?}",
                message.kind()
            ))
        }
    }

    println!("ranked-result ok");
    Ok(())
}

async fn register_host(
    server: SocketAddr,
    id: &str,
    name: &str,
    ranked: bool,
) -> Result<LobbyEntry, String> {
    match server_request(
        server,
        WireMessage::LobbyRegister(LobbyRegister {
            player: player(id, name),
            direct_addr: "127.0.0.1:4405".to_string(),
            ranked,
        }),
    )
    .await?
    {
        WireMessage::LobbyList(LobbyList { entries }) if entries.len() == 1 => {
            Ok(entries.into_iter().next().expect("entry count checked"))
        }
        WireMessage::RankedResultRejected(rejected) => Err(rejected.reason),
        message => Err(format!(
            "expected lobby registration response, got {:?}",
            message.kind()
        )),
    }
}

async fn request_lobby_list(server: SocketAddr, ranked_only: bool) -> Result<LobbyList, String> {
    match server_request(
        server,
        WireMessage::LobbyListRequest(LobbyListRequest { ranked_only }),
    )
    .await?
    {
        WireMessage::LobbyList(list) => Ok(list),
        message => Err(format!("expected lobby list, got {:?}", message.kind())),
    }
}

async fn request_hosted_join(
    server: SocketAddr,
    entry: &LobbyEntry,
    joiner: HostedPlayer,
) -> Result<battletris_protocol::HostedGameStart, String> {
    match server_request(
        server,
        WireMessage::HostedJoinRequest(HostedJoinRequest {
            session_id: entry.session_id.clone(),
            joiner,
        }),
    )
    .await?
    {
        WireMessage::HostedGameStart(start) => Ok(start),
        WireMessage::RankedResultRejected(rejected) => Err(rejected.reason),
        message => Err(format!(
            "expected hosted game start, got {:?}",
            message.kind()
        )),
    }
}

async fn request_hosted_status(
    server: SocketAddr,
    entry: &LobbyEntry,
    requester: &str,
) -> Result<HostedSessionStatus, String> {
    match server_request(
        server,
        WireMessage::HostedSessionStatusRequest(HostedSessionStatusRequest {
            session_id: entry.session_id.clone(),
            requester_player_id: requester.to_string(),
        }),
    )
    .await?
    {
        WireMessage::HostedSessionStatus(status) => Ok(status),
        WireMessage::RankedResultRejected(rejected) => Err(rejected.reason),
        message => Err(format!(
            "expected hosted session status, got {:?}",
            message.kind()
        )),
    }
}

async fn expect_pending(server: SocketAddr, claim: RankedResultClaim) -> Result<(), String> {
    match server_request(server, WireMessage::RankedResultClaim(claim)).await? {
        WireMessage::RankedResultPending(_) => Ok(()),
        message => Err(format!("expected pending result, got {:?}", message.kind())),
    }
}

async fn server_request(server: SocketAddr, message: WireMessage) -> Result<WireMessage, String> {
    let mut stream = timeout(SMOKE_TIMEOUT, tokio::net::TcpStream::connect(server))
        .await
        .map_err(|_| format!("timed out connecting to server {server}"))?
        .map_err(|error| format!("connect to server {server} failed: {error}"))?;
    write_message(&mut stream, &message)
        .await
        .map_err(|error| format!("write server request failed: {error:?}"))?;
    timeout(SMOKE_TIMEOUT, read_message(&mut stream))
        .await
        .map_err(|_| "timed out waiting for server response".to_string())?
        .map_err(|error| format!("read server response failed: {error:?}"))
}

fn game_from_seed(seed: u64) -> TwoPlayerGame {
    let (one, two) = derive_player_seeds(seed);
    TwoPlayerGame::new(GameSeed::from_u64(one), GameSeed::from_u64(two))
}

fn scripted_input(player: PlayerSlot, tick: u64) -> PlayerInput {
    let command = match (player, tick) {
        (PlayerSlot::One, 0) => InputCommand::MoveLeft,
        (PlayerSlot::Two, 0) => InputCommand::MoveRight,
        (PlayerSlot::One, 1) => InputCommand::RotateClockwise,
        (PlayerSlot::Two, 1) => InputCommand::RotateCounterClockwise,
        (_, 2) => InputCommand::StartFastDrop,
        (_, 6) => InputCommand::StopFastDrop,
        (PlayerSlot::One, _) => InputCommand::MoveRight,
        (PlayerSlot::Two, _) => InputCommand::MoveLeft,
    };
    PlayerInput {
        player,
        tick,
        command,
    }
}

fn apply_ordered_inputs(
    game: &mut TwoPlayerGame,
    first: PlayerInput,
    second: PlayerInput,
) -> Result<(), String> {
    let (one, two) = match first.player {
        PlayerSlot::One => (first, second),
        PlayerSlot::Two => (second, first),
    };
    if one.player != PlayerSlot::One || two.player != PlayerSlot::Two || one.tick != two.tick {
        return Err("scripted input ordering invariant failed".to_string());
    }
    game.command(core_player(PlayerSlot::One), core_command(one.command)?);
    game.command(core_player(PlayerSlot::Two), core_command(two.command)?);
    Ok(())
}

fn core_command(command: InputCommand) -> Result<Command, String> {
    match command {
        InputCommand::MoveLeft => Ok(Command::MoveLeft),
        InputCommand::MoveRight => Ok(Command::MoveRight),
        InputCommand::RotateClockwise => Ok(Command::RotateClockwise),
        InputCommand::RotateCounterClockwise => Ok(Command::RotateCounterClockwise),
        InputCommand::StartFastDrop => Ok(Command::StartFastDrop),
        InputCommand::StopFastDrop => Ok(Command::StopFastDrop),
        InputCommand::LaunchWeapon { .. } => {
            Err("smoke script does not launch weapons".to_string())
        }
    }
}

fn checksum_game(game: &TwoPlayerGame) -> u64 {
    let mut text = String::new();
    let _ = write!(
        text,
        "phase={:?};events={:?};p1=({},{},{:?},{:?});p2=({},{},{:?},{:?})",
        game.phase(),
        game.event_log(),
        game.player(core_player(PlayerSlot::One)).score(),
        game.player(core_player(PlayerSlot::One)).funds(),
        game.player(core_player(PlayerSlot::One)).lines(),
        game.player(core_player(PlayerSlot::One)).board().snapshot(),
        game.player(core_player(PlayerSlot::Two)).score(),
        game.player(core_player(PlayerSlot::Two)).funds(),
        game.player(core_player(PlayerSlot::Two)).lines(),
        game.player(core_player(PlayerSlot::Two)).board().snapshot(),
    );
    let mut hasher = Fnv64::default();
    hasher.write(text.as_bytes());
    hasher.finish()
}

#[derive(Default)]
struct Fnv64(u64);

impl Hasher for Fnv64 {
    fn write(&mut self, bytes: &[u8]) {
        if self.0 == 0 {
            self.0 = 0xcbf2_9ce4_8422_2325;
        }
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

fn claim_for(
    start: &battletris_protocol::HostedGameStart,
    reporter: &str,
    checksum: u64,
) -> RankedResultClaim {
    RankedResultClaim {
        session_id: start.session_id.clone(),
        reporter_player_id: reporter.to_string(),
        winner_player_id: start.player_one.player_id.clone(),
        loser_player_id: start.player_two.player_id.clone(),
        winner_score: 12_000,
        winner_lines: 40,
        winner_funds: 25,
        loser_score: 8_000,
        loser_lines: 22,
        loser_funds: 10,
        duration_secs: 120,
        duration_ticks: 12_000,
        event_count: 64,
        final_checksum: checksum,
    }
}

fn identity(display_name: &str) -> PlayerIdentity {
    PlayerIdentity {
        display_name: display_name.to_string(),
    }
}

fn player(id: &str, display_name: &str) -> HostedPlayer {
    HostedPlayer {
        player_id: id.to_string(),
        display_name: display_name.to_string(),
    }
}

fn core_player(player: PlayerSlot) -> battletris_core::game::PlayerId {
    match player {
        PlayerSlot::One => battletris_core::game::PlayerId::One,
        PlayerSlot::Two => battletris_core::game::PlayerId::Two,
    }
}

fn opposite_slot(player: PlayerSlot) -> PlayerSlot {
    match player {
        PlayerSlot::One => PlayerSlot::Two,
        PlayerSlot::Two => PlayerSlot::One,
    }
}

async fn join_task(
    task: tokio::task::JoinHandle<Result<u64, String>>,
    name: &str,
) -> Result<u64, String> {
    task.await
        .map_err(|error| format!("{name} task failed: {error}"))?
}

fn parse_flag_addr(args: &mut Vec<String>, flag: &str) -> Result<SocketAddr, String> {
    let Some(position) = args.iter().position(|arg| arg == flag) else {
        return Err(format!("{flag} is required"));
    };
    if position + 1 >= args.len() {
        return Err(format!("{flag} requires ADDR:PORT"));
    }
    let value = args.remove(position + 1);
    args.remove(position);
    value
        .parse()
        .map_err(|error| format!("parse {flag} value {value:?} failed: {error}"))
}

fn ensure_no_extra_args(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(format!("unexpected arguments: {}", args.join(" ")))
    }
}

fn print_help() {
    println!(
        "Usage: battletris-tools net-smoke <command>\n\nCommands:\n  direct-loopback\n  direct-host --listen ADDR:PORT\n  direct-join --addr ADDR:PORT\n  hosted-lobby --server ADDR:PORT\n  legacy-roster --server ADDR:PORT --share ADDR:PORT\n  ranked-result --server ADDR:PORT\n\nProtocol: {}.{} ({})",
        PROTOCOL_MAJOR, PROTOCOL_MINOR, CAPABILITY_DIRECT_TCP
    );

    let _hello_shape = Hello {
        major: PROTOCOL_MAJOR,
        minor: PROTOCOL_MINOR,
        identity: identity("Smoke"),
        capabilities: vec![CAPABILITY_DIRECT_TCP.to_string()],
    };
}

struct HostedGameStartPair {
    entry: LobbyEntry,
    join_start: battletris_protocol::HostedGameStart,
}
