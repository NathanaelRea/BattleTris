//! Direct TCP networking MVP integration tests over loopback and in-memory streams.

use battletris_core::{
    game::{Command, PlayerId, TwoPlayerGame},
    rng::GameSeed,
};
use battletris_protocol::{
    accept_direct_game, accept_pending_direct_challenge, derive_player_seeds, join_direct_game,
    join_direct_game_with_challenge, read_message, write_message, BazaarDone, BazaarState,
    Challenge, DirectConnection, Disconnect, GameOver, HostedSessionId, InputCommand,
    LanAdvertisement, PlayerIdentity, PlayerInput, PlayerSlot, ProtocolError, ScoreSnapshot,
    WireMessage, PROTOCOL_MAJOR,
};
use tokio::{net::TcpListener, time::Duration};

fn identity(name: &str) -> PlayerIdentity {
    PlayerIdentity {
        display_name: name.to_string(),
    }
}

#[tokio::test]
async fn direct_host_can_deny_pending_challenge_with_reason() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener address");

    let host = tokio::spawn(async move {
        let pending = accept_pending_direct_challenge(&listener, identity("Ada"))
            .await
            .expect("pending challenge");
        assert_eq!(pending.challenge.message, "battle?");
        pending
            .deny("busy".to_string())
            .await
            .expect("deny challenge");
    });

    let join = tokio::spawn(async move {
        join_direct_game(addr, identity("Ben"), "battle?".to_string()).await
    });

    host.await.expect("host task");
    assert!(matches!(
        join.await.expect("join task"),
        Err(ProtocolError::ChallengeDenied { reason }) if reason == "busy"
    ));
}

#[tokio::test]
async fn hosted_direct_challenge_carries_server_session_metadata() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener address");

    let host = tokio::spawn(async move {
        let pending = accept_pending_direct_challenge(&listener, identity("Ada"))
            .await
            .expect("pending hosted challenge");
        assert_eq!(
            pending.challenge.hosted_session_id,
            Some(HostedSessionId("session-1".to_string()))
        );
        assert_eq!(pending.challenge.hosted_player_id.as_deref(), Some("ben"));
        pending.deny("verified metadata".to_string()).await.unwrap();
    });

    let join = tokio::spawn(async move {
        join_direct_game_with_challenge(
            addr,
            Challenge {
                challenger: identity("Ben"),
                message: "hosted battle".to_string(),
                hosted_session_id: Some(HostedSessionId("session-1".to_string())),
                hosted_player_id: Some("ben".to_string()),
            },
        )
        .await
    });

    host.await.expect("host task");
    assert!(matches!(
        join.await.expect("join task"),
        Err(ProtocolError::ChallengeDenied { reason }) if reason == "verified metadata"
    ));
}

fn game_from_seed(seed: u64) -> TwoPlayerGame {
    let (player_one_seed, player_two_seed) = derive_player_seeds(seed);
    TwoPlayerGame::new(
        GameSeed::from_u64(player_one_seed),
        GameSeed::from_u64(player_two_seed),
    )
}

fn apply_input(game: &mut TwoPlayerGame, input: &PlayerInput) {
    let player = player_id(input.player);
    match input.command {
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
            let label = if slot_index == 9 { 0 } else { slot_index + 1 };
            let _ = game.launch_weapon_slot(player, label);
        }
    }
}

fn player_id(slot: PlayerSlot) -> PlayerId {
    match slot {
        PlayerSlot::One => PlayerId::One,
        PlayerSlot::Two => PlayerId::Two,
    }
}

fn score_snapshot(game: &TwoPlayerGame, player: PlayerSlot) -> ScoreSnapshot {
    let loop_state = game.player(player_id(player));
    ScoreSnapshot {
        player,
        score: loop_state.score(),
        funds: loop_state.funds(),
        lines: loop_state.lines(),
    }
}

async fn recv_input(connection: &mut DirectConnection) -> PlayerInput {
    match connection.recv().await.expect("peer message received") {
        WireMessage::PlayerInput(input) => input,
        other => panic!("expected player input, got {:?}", other.kind()),
    }
}

async fn recv_score(connection: &mut DirectConnection) -> ScoreSnapshot {
    match connection.recv().await.expect("score received") {
        WireMessage::ScoreSnapshot(score) => score,
        other => panic!("expected score snapshot, got {:?}", other.kind()),
    }
}

#[tokio::test]
async fn direct_tcp_clients_complete_scripted_game_without_desync() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener address");

    let host = tokio::spawn(async move {
        let accepted = accept_direct_game(&listener, identity("Ada"), 55, true)
            .await
            .expect("accept direct game");
        assert_eq!(accepted.remote_identity.display_name, "Ben");
        assert_eq!(accepted.challenge.message, "battle?");

        let mut connection = accepted.connection;
        let mut game = game_from_seed(55);
        let local_input = PlayerInput {
            player: PlayerSlot::One,
            tick: 1,
            command: InputCommand::MoveLeft,
        };
        apply_input(&mut game, &local_input);
        connection
            .send(&WireMessage::PlayerInput(local_input))
            .await
            .expect("send host input");

        let remote_input = recv_input(&mut connection).await;
        apply_input(&mut game, &remote_input);
        assert_eq!(remote_input.player, PlayerSlot::Two);

        let local_score = score_snapshot(&game, PlayerSlot::One);
        connection
            .send(&WireMessage::ScoreSnapshot(local_score.clone()))
            .await
            .expect("send host score");
        let remote_score = recv_score(&mut connection).await;
        assert_eq!(remote_score, score_snapshot(&game, PlayerSlot::Two));

        connection
            .send(&WireMessage::BazaarState(BazaarState {
                player_one_done: false,
                player_two_done: false,
            }))
            .await
            .expect("send bazaar state");
        connection
            .send(&WireMessage::BazaarDone(BazaarDone {
                player: PlayerSlot::One,
            }))
            .await
            .expect("send host bazaar done");
        assert!(matches!(
            connection.recv().await.expect("client bazaar done"),
            WireMessage::BazaarDone(BazaarDone {
                player: PlayerSlot::Two
            })
        ));

        connection
            .send(&WireMessage::GameOver(GameOver {
                winner: PlayerSlot::One,
                loser: PlayerSlot::Two,
                sequence: game.event_log().len() as u64,
            }))
            .await
            .expect("send game over");
        assert!(matches!(
            connection.recv().await.expect("client disconnect"),
            WireMessage::Disconnect(_)
        ));

        game.event_log().to_vec()
    });

    let client = tokio::spawn(async move {
        let joined = join_direct_game(addr, identity("Ben"), "battle?".to_string())
            .await
            .expect("join direct game");
        assert_eq!(joined.remote_identity.display_name, "Ada");
        assert_eq!(joined.start.receiving_peer_slot, PlayerSlot::Two);
        assert_eq!(joined.start.seed, 55);

        let mut connection = joined.connection;
        let mut game = game_from_seed(joined.start.seed);
        let remote_input = recv_input(&mut connection).await;
        apply_input(&mut game, &remote_input);

        let local_input = PlayerInput {
            player: PlayerSlot::Two,
            tick: 1,
            command: InputCommand::MoveRight,
        };
        apply_input(&mut game, &local_input);
        connection
            .send(&WireMessage::PlayerInput(local_input))
            .await
            .expect("send client input");

        let remote_score = recv_score(&mut connection).await;
        assert_eq!(remote_score, score_snapshot(&game, PlayerSlot::One));
        connection
            .send(&WireMessage::ScoreSnapshot(score_snapshot(
                &game,
                PlayerSlot::Two,
            )))
            .await
            .expect("send client score");

        assert!(matches!(
            connection.recv().await.expect("bazaar state"),
            WireMessage::BazaarState(BazaarState {
                player_one_done: false,
                player_two_done: false
            })
        ));
        assert!(matches!(
            connection.recv().await.expect("host bazaar done"),
            WireMessage::BazaarDone(BazaarDone {
                player: PlayerSlot::One
            })
        ));
        connection
            .send(&WireMessage::BazaarDone(BazaarDone {
                player: PlayerSlot::Two,
            }))
            .await
            .expect("send client bazaar done");

        assert!(matches!(
            connection.recv().await.expect("game over"),
            WireMessage::GameOver(GameOver {
                winner: PlayerSlot::One,
                loser: PlayerSlot::Two,
                ..
            })
        ));
        connection
            .send(&WireMessage::Disconnect(Disconnect {
                reason: "complete".to_string(),
            }))
            .await
            .expect("send disconnect");

        game.event_log().to_vec()
    });

    let (host_log, client_log) = tokio::join!(host, client);
    assert_eq!(
        host_log.expect("host task"),
        client_log.expect("client task")
    );
}

#[tokio::test]
async fn tcp_disconnect_path_reports_io_failure() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener address");

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept stream");
        read_message(&mut stream).await
    });

    let client = tokio::spawn(async move {
        let _stream = tokio::net::TcpStream::connect(addr)
            .await
            .expect("connect then drop");
    });

    client.await.expect("client task");
    let result = tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("read returns after disconnect")
        .expect("server task");
    assert!(matches!(result, Err(ProtocolError::Io(_))));
}

#[tokio::test]
async fn score_snapshot_mismatch_is_a_desync_error_for_headless_clients() {
    let (mut left, mut right) = tokio::io::duplex(1024);
    let expected = ScoreSnapshot {
        player: PlayerSlot::One,
        score: 10,
        funds: 0,
        lines: 0,
    };
    let mismatched = ScoreSnapshot {
        score: 11,
        ..expected.clone()
    };

    let sender = tokio::spawn(async move {
        write_message(&mut left, &WireMessage::ScoreSnapshot(mismatched))
            .await
            .expect("write mismatched snapshot");
    });
    let received = match read_message(&mut right).await.expect("read snapshot") {
        WireMessage::ScoreSnapshot(score) => score,
        other => panic!("expected score snapshot, got {:?}", other.kind()),
    };
    sender.await.expect("sender task");

    assert_ne!(received, expected, "snapshot mismatch marks a desync");
}

#[test]
fn lan_advertisement_contains_required_best_effort_metadata() {
    let ad = LanAdvertisement::available(&identity("Ada"), 4404);

    assert_eq!(ad.service, battletris_protocol::LAN_DISCOVERY_SERVICE);
    assert_eq!(ad.port, 4404);
    assert_eq!(ad.txt["protocol_major"], PROTOCOL_MAJOR.to_string());
    assert_eq!(ad.txt["display_name"], "Ada");
    assert_eq!(ad.txt["port"], "4404");
    assert_eq!(ad.txt["state"], "available");
}
