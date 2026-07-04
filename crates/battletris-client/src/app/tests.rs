use super::*;
use battletris_protocol::{PlayerIdentity, PlayerSlot, StartGame};

#[test]
fn next_piece_preview_does_not_advance_core_state() {
    let game = TwoPlayerGame::new(GameSeed::from_u64(1), GameSeed::from_u64(2));
    let first = game.player(PlayerId::One).next_piece_kind_preview();
    let second = game.player(PlayerId::One).next_piece_kind_preview();

    assert_eq!(first, second);
}

#[test]
fn network_reducer_tracks_listening_addresses() {
    let mut state = ClientNetworkState::default();
    let bind_addr = "0.0.0.0:4405".parse().unwrap();
    let share_addr = "192.168.1.20:4405".parse().unwrap();

    reduce_client_network_event(
        &mut state,
        NetworkEvent::Listening {
            bind_addr,
            share_addr,
        },
    );

    assert_eq!(
        state.lifecycle,
        NetworkLifecycleState::Hosting { bind_addr }
    );
    assert_eq!(state.listening_bind_addr, Some(bind_addr));
    assert_eq!(state.listening_share_addr, Some(share_addr));
    assert!(state.pending_challenge.is_none());
}

#[test]
fn network_reducer_tracks_pending_challenge() {
    let mut state = ClientNetworkState::default();
    let challenge = Challenge {
        challenger: PlayerIdentity {
            display_name: "Joiner".to_string(),
        },
        message: "play?".to_string(),
        hosted_session_id: None,
        hosted_player_id: None,
    };

    reduce_client_network_event(
        &mut state,
        NetworkEvent::IncomingChallenge {
            challenge: challenge.clone(),
        },
    );

    assert_eq!(state.pending_challenge, Some(challenge.clone()));
    assert_eq!(
        state.lifecycle,
        NetworkLifecycleState::Challenged { challenge }
    );
}

#[test]
fn network_reducer_tracks_connected_session_and_watermark() {
    let mut state = ClientNetworkState::default();
    let session = NetworkSession::direct(
        PlayerSlot::One,
        PlayerIdentity {
            display_name: "Peer".to_string(),
        },
        StartGame {
            receiving_peer_slot: PlayerSlot::One,
            seed: 99,
            ranked: false,
        },
    );

    reduce_client_network_event(
        &mut state,
        NetworkEvent::Connected {
            session: Box::new(session.clone()),
        },
    );
    reduce_client_network_event(
        &mut state,
        NetworkEvent::TickWatermark(TickWatermark {
            player: PlayerSlot::Two,
            through_tick: 42,
        }),
    );

    assert_eq!(state.connected_session.as_ref().unwrap().base_seed, 99);
    assert_eq!(
        state.connected_session.as_ref().unwrap().peer_watermark,
        Some(42)
    );
    assert_eq!(state.result_status, FinalResultStatus::Unranked);
}

#[test]
fn networked_local_game_uses_session_seed_and_slot() {
    let session = NetworkSession::direct(
        PlayerSlot::Two,
        PlayerIdentity {
            display_name: "Host".to_string(),
        },
        StartGame {
            receiving_peer_slot: PlayerSlot::Two,
            seed: 1234,
            ranked: false,
        },
    );

    let local = LocalGame::new_networked(session.clone());

    assert_eq!(local.local_player, PlayerId::Two);
    assert_eq!(local.mode, LocalGameMode::DirectConnect);
    assert!(local.computer.is_none());
    assert!(local.network_lockstep.is_some());
    assert_eq!(local.network_session.as_ref().unwrap().base_seed, 1234);
    assert_eq!(local.game.mode(), GameMode::HumanVsHuman);
    let expected_status = network_session_status_label(&session, None);
    assert_eq!(
        local.status_message.as_deref(),
        Some(expected_status.as_str())
    );
}

#[test]
fn client_network_tick_loop_runs_past_checksum_tick_without_desync() {
    let mut host = TestClientPeer::new("Ada");
    let mut joiner = TestClientPeer::new("Ben");
    connect_test_client_peers(&mut host, &mut joiner, 123);

    for frame in 0..260 {
        pump_test_client_pair(&mut host, &mut joiner);
        if frame == 7 {
            schedule_network_input(
                &mut host.local,
                &mut host.runtime,
                &mut host.state,
                InputCommand::MoveLeft,
            );
        }
        if frame == 11 {
            schedule_network_input(
                &mut joiner.local,
                &mut joiner.runtime,
                &mut joiner.state,
                InputCommand::RotateClockwise,
            );
        }

        tick_test_client_peer(&mut host);
        tick_test_client_peer(&mut joiner);
        pump_test_client_pair(&mut host, &mut joiner);

        assert!(
            !host.local.network_failed_closed,
            "host failed closed at frame {frame}: status={:?} error={:?}",
            host.local.status_message, host.state.last_error
        );
        assert!(
            !joiner.local.network_failed_closed,
            "joiner failed closed at frame {frame}: status={:?} error={:?}",
            joiner.local.status_message, joiner.state.last_error
        );
        assert!(
            host.state.last_error.is_none(),
            "host: {:?}",
            host.state.last_error
        );
        assert!(
            joiner.state.last_error.is_none(),
            "joiner: {:?}",
            joiner.state.last_error
        );
    }

    catch_up_test_client_pair(&mut host, &mut joiner);

    let host_tick = host.local.network_lockstep.as_ref().unwrap().current_tick();
    let joiner_tick = joiner
        .local
        .network_lockstep
        .as_ref()
        .unwrap()
        .current_tick();
    assert!(host_tick > 100, "host tick only reached {host_tick}");
    assert_eq!(host_tick, joiner_tick);
    assert_eq!(
        host.local.game.deterministic_checksum(),
        joiner.local.game.deterministic_checksum()
    );

    host.shutdown();
    joiner.shutdown();
}

#[test]
fn network_weapon_slot_labels_map_to_protocol_indices() {
    assert_eq!(slot_label_to_index(1), 0);
    assert_eq!(slot_label_to_index(9), 8);
    assert_eq!(slot_label_to_index(0), 9);
}

#[test]
fn direct_challenge_acceptance_is_unranked() {
    let start = StartGame {
        receiving_peer_slot: PlayerSlot::One,
        seed: 7,
        ranked: false,
    };
    let session = NetworkSession::direct(
        PlayerSlot::One,
        PlayerIdentity {
            display_name: "Peer".to_string(),
        },
        start,
    );

    assert!(!session.ranked);
    assert_eq!(session.final_result_status, FinalResultStatus::Unranked);
}

#[test]
fn network_reducer_surfaces_result_rejection() {
    let mut state = ClientNetworkState::default();

    reduce_client_network_event(
        &mut state,
        NetworkEvent::RejectedResult(RankedResultRejected {
            session_id: None,
            reason: "mismatch".to_string(),
        }),
    );

    assert_eq!(state.last_error.as_deref(), Some("mismatch"));
    assert_eq!(
        state.result_status,
        FinalResultStatus::Rejected("mismatch".to_string())
    );
}

#[test]
fn visual_fixture_names_are_canonical() {
    let ids = VisualFixture::ALL
        .into_iter()
        .map(VisualFixture::id)
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            "startup",
            "challenge",
            "sleep",
            "about",
            "roster",
            "settings",
            "game-playing",
            "game-bazaar",
            "game-over",
            "game-recon",
            "board-cells",
        ]
    );
    assert_eq!(
        VisualFixture::from_id("game-bazaar"),
        Some(VisualFixture::GameBazaar)
    );
    assert!(VisualFixture::from_id("bazaar").is_none());
}

#[test]
fn game_over_fixture_builds_game_over_state() {
    let local = visual_local_game(VisualFixture::GameOver, DEFAULT_ERNIE_LEVEL);

    assert_eq!(VisualFixture::GameOver.screen(), ClientScreen::Game);
    assert_eq!(local.game.phase(), GamePhase::GameOver);
    assert_eq!(
        legacy_game_message_label(&local, ContentMode::Rated),
        "Nice loss, shithead."
    );
}

#[test]
fn headless_capture_cli_parses_fixture_theme_and_output() {
    let config = ClientRunConfig::parse(
        vec![
            OsString::from("headless"),
            OsString::from("capture"),
            OsString::from("--fixture"),
            OsString::from("game-recon"),
            OsString::from("--theme=high-contrast"),
            OsString::from("--output"),
            OsString::from("target/visual/current/game-recon.png"),
        ],
        None,
    )
    .expect("headless capture CLI parses");

    assert!(config.deterministic_capture);
    assert_eq!(config.content_mode, ContentMode::Normal);
    match config.capture.expect("capture spec") {
        VisualCaptureSpec::One {
            fixture,
            theme,
            output,
        } => {
            assert_eq!(fixture, VisualFixture::GameRecon);
            assert_eq!(theme, ThemeChoice::HighContrast);
            assert_eq!(
                output,
                PathBuf::from("target/visual/current/game-recon.png")
            );
        }
        other => panic!("unexpected capture spec: {other:?}"),
    }
}

#[test]
fn client_cli_defaults_to_normal_content_mode() {
    let config = ClientRunConfig::parse(Vec::new(), None).expect("empty CLI parses");

    assert_eq!(config.content_mode, ContentMode::Normal);
    assert!(!config.deterministic_capture);
    assert!(config.capture.is_none());
}

#[test]
fn client_cli_accepts_rated_long_and_legacy_short_flags() {
    let long =
        ClientRunConfig::parse(vec![OsString::from("--rated")], None).expect("rated CLI parses");
    let short =
        ClientRunConfig::parse(vec![OsString::from("-r")], None).expect("legacy rated CLI parses");

    assert_eq!(long.content_mode, ContentMode::Rated);
    assert_eq!(short.content_mode, ContentMode::Rated);
    assert!(!long.deterministic_capture);
    assert!(!short.deterministic_capture);
}

#[test]
fn legacy_cli_flags_apply_session_overrides() {
    let config = ClientRunConfig::parse(
        vec![
            OsString::from("-s"),
            OsString::from("-m"),
            OsString::from("-X"),
            OsString::from("-S"),
            OsString::from("127.0.0.2"),
            OsString::from("-P"),
            OsString::from("4410"),
            OsString::from("-p"),
            OsString::from("-a"),
        ],
        None,
    )
    .expect("legacy flags parse");
    let mut settings = ClientSettings {
        settings_path: None,
        ..Default::default()
    };

    settings.content_mode = config.content_mode;
    config.session_overrides.apply_to(&mut settings);

    assert_eq!(settings.content_mode, ContentMode::Normal);
    assert_eq!(settings.screen, ClientScreen::Sleep);
    assert_eq!(settings.sound_pack, SoundPackChoice::Muted);
    assert!(!settings.lobby_enabled);
    assert_eq!(settings.lobby_addr, "127.0.0.2:4410");
}

#[test]
fn xrm_overrides_apply_known_legacy_resources() {
    let config = ClientRunConfig::parse(
        vec![
            OsString::from("-xrm"),
            OsString::from("BattleTris*sleep: True"),
            OsString::from("-xrm"),
            OsString::from("BattleTris*r_rated: on"),
            OsString::from("-xrm"),
            OsString::from("BattleTris*mute: yes"),
            OsString::from("-xrm"),
            OsString::from("BattleTris*no_server: 1"),
            OsString::from("--xrm=BattleTris*serverHost: 127.0.0.3"),
            OsString::from("-xrm"),
            OsString::from("BattleTris.serverPort: 4411"),
            OsString::from("-xrm"),
            OsString::from("BattleTris*background: red"),
        ],
        None,
    )
    .expect("xrm overrides parse");
    let mut settings = ClientSettings {
        settings_path: None,
        ..Default::default()
    };

    settings.content_mode = config.content_mode;
    config.session_overrides.apply_to(&mut settings);

    assert_eq!(settings.content_mode, ContentMode::Rated);
    assert_eq!(settings.screen, ClientScreen::Sleep);
    assert_eq!(settings.sound_pack, SoundPackChoice::Muted);
    assert!(!settings.lobby_enabled);
    assert_eq!(settings.lobby_addr, "127.0.0.3:4411");
}

#[test]
fn server_host_override_rejects_non_socket_hostnames() {
    let error = ClientRunConfig::parse(
        vec![OsString::from("-S"), OsString::from("battletris.example")],
        None,
    )
    .expect_err("hostname server override is rejected until DNS addresses are supported");

    assert!(error.contains("invalid server host/port override"));
}

#[test]
fn rated_flag_is_session_only_and_not_persisted() {
    let mut settings = ClientSettings {
        content_mode: ContentMode::Rated,
        settings_path: None,
        ..Default::default()
    };
    settings.apply_persisted(PersistedClientSettings::default());

    assert_eq!(settings.content_mode, ContentMode::Rated);
    let encoded = toml::to_string_pretty(&settings.persisted()).expect("settings encode");
    assert!(!encoded.contains("content-mode"));
}

#[test]
fn rated_flag_can_wrap_headless_capture_without_changing_capture_semantics() {
    let config = ClientRunConfig::parse(
        vec![
            OsString::from("--rated"),
            OsString::from("headless"),
            OsString::from("capture"),
            OsString::from("--fixture=board-cells"),
            OsString::from("--theme"),
            OsString::from("original"),
            OsString::from("--output"),
            OsString::from("target/visual/current/board-cells-rated.png"),
        ],
        None,
    )
    .expect("rated headless capture CLI parses");

    assert!(config.deterministic_capture);
    assert_eq!(config.content_mode, ContentMode::Rated);
    match config.capture.expect("capture spec") {
        VisualCaptureSpec::One {
            fixture,
            theme,
            output,
        } => {
            assert_eq!(fixture, VisualFixture::BoardCells);
            assert_eq!(theme, ThemeChoice::Original);
            assert_eq!(
                output,
                PathBuf::from("target/visual/current/board-cells-rated.png")
            );
        }
        other => panic!("unexpected capture spec: {other:?}"),
    }
}

#[test]
fn headless_capture_accepts_rated_flag_in_command_options() {
    let config = ClientRunConfig::parse(
        vec![
            OsString::from("headless"),
            OsString::from("capture"),
            OsString::from("--fixture"),
            OsString::from("game-over"),
            OsString::from("--theme=original"),
            OsString::from("--rated"),
            OsString::from("--output"),
            OsString::from("target/visual/rated/game-over.png"),
        ],
        None,
    )
    .expect("rated headless capture option parses");

    assert!(config.deterministic_capture);
    assert_eq!(config.content_mode, ContentMode::Rated);
    match config.capture.expect("capture spec") {
        VisualCaptureSpec::One {
            fixture,
            theme,
            output,
        } => {
            assert_eq!(fixture, VisualFixture::GameOver);
            assert_eq!(theme, ThemeChoice::Original);
            assert_eq!(output, PathBuf::from("target/visual/rated/game-over.png"));
        }
        other => panic!("unexpected capture spec: {other:?}"),
    }
}

#[test]
fn capture_all_jobs_cover_every_fixture_for_one_theme() {
    let config = ClientRunConfig::parse(
        vec![
            OsString::from("headless"),
            OsString::from("capture-all"),
            OsString::from("--theme"),
            OsString::from("original"),
            OsString::from("--out-dir"),
            OsString::from("target/visual/current"),
        ],
        None,
    )
    .expect("headless capture-all CLI parses");
    let themes = ThemePacks::load(&assets_dir());
    let capture = config
        .capture
        .expect("capture spec")
        .to_capture(&themes, ThemeChoice::HighContrast);

    assert_eq!(capture.jobs.len(), VisualFixture::ALL.len());
    assert_eq!(capture.jobs[0].fixture, VisualFixture::Startup);
    assert_eq!(capture.jobs[0].theme, ThemeChoice::Original);
    assert_eq!(
        capture.jobs[0].path,
        PathBuf::from("target/visual/current/startup.png")
    );
    assert_eq!(capture.jobs[0].expected_width, 640);
    assert_eq!(capture.jobs[0].expected_height, 600);
    assert_eq!(
        capture.jobs.last().unwrap().path,
        PathBuf::from("target/visual/current/board-cells.png")
    );
}

#[test]
fn smoke_screenshot_uses_shared_startup_capture_spec() {
    let config = ClientRunConfig::parse(
        Vec::new(),
        Some(OsString::from("target/visual/current/startup.png")),
    )
    .expect("smoke env parses");

    assert!(!config.deterministic_capture);
    match config.capture.expect("capture spec") {
        VisualCaptureSpec::Smoke { path } => {
            assert_eq!(path, PathBuf::from("target/visual/current/startup.png"));
        }
        other => panic!("unexpected capture spec: {other:?}"),
    }
}

#[test]
fn hud_mentions_core_state_and_preview() {
    let local = LocalGame::new_human_vs_human();
    let recon = ReconPanel::default();
    let hud = player_hud(&local, &recon, PlayerId::One);

    assert!(hud.contains("score 0"));
    assert!(hud.contains("funds 0"));
    assert!(hud.contains("bazaar in 20"));
    assert!(hud.contains("next "));
    assert!(hud.contains("arsenal empty"));
}

#[test]
fn bazaar_shortcut_keys_are_affordable_intro_weapons() {
    for (token, _) in bazaar_catalog_keys() {
        assert!(battletris_core::weapons::weapon_spec(token).price <= 125);
    }
}

#[test]
fn bazaar_sorted_catalog_exposes_every_weapon_by_price() {
    let rows = sorted_weapon_catalog();

    assert_eq!(rows.len(), WEAPON_CATALOG.len());
    assert_eq!(rows.first().unwrap().token, WeaponToken::FlipOut);
    assert_eq!(rows.last().unwrap().token, WeaponToken::Swap);
    assert!(rows.windows(2).all(|pair| pair[0].price <= pair[1].price));
}

#[test]
fn bazaar_mouse_rows_select_any_catalog_weapon() {
    let themes = ThemePacks::load(&assets_dir());
    let theme = themes.get(ThemeChoice::Original);
    let first = bazaar_catalog_token_at(Vec2::new(-370.0, 195.0), theme);
    let last = bazaar_catalog_token_at(Vec2::new(-370.0, -365.0), theme);

    assert_eq!(first, Some(WeaponToken::FlipOut));
    assert_eq!(last, Some(WeaponToken::Swap));
}

#[test]
fn bundled_theme_loads_cell_atlas_contract() {
    let themes = ThemePacks::load(&assets_dir());
    let theme = themes.get(ThemeChoice::Original);

    assert_eq!(theme.cell_atlas.columns, 32);
    assert_eq!(theme.cell_atlas.rows, 1);
    assert_eq!(theme.cell_atlas.cells.empty, 0);
    assert_eq!(theme.cell_atlas.cells.visible_colors[0], 1);
    assert_eq!(theme.cell_atlas.cells.visible_colors[18], 19);
    assert_eq!(theme.cell_atlas.cells.die, [24, 25, 26, 27, 28, 29]);
    assert_eq!(
        theme.layout.rects.startup_challenge.center(),
        Vec2::new(-95.5, -200.0)
    );
    assert_eq!(theme.fonts.line_height, 1.0);
    assert_eq!(
        theme.fonts.path_for(ThemedTextFontRole::Body),
        Some("themes/original/fonts/NimbusSans-Bold.otf")
    );
    assert_eq!(
        theme.fonts.path_for(ThemedTextFontRole::Title),
        Some("themes/original/fonts/NimbusSans-Bold.otf")
    );
    assert_eq!(
        theme.fonts.path_for(ThemedTextFontRole::Button),
        Some("themes/original/fonts/NimbusSans-Bold.otf")
    );
    assert_eq!(
        theme.fonts.path_for(ThemedTextFontRole::Mono),
        Some("themes/original/fonts/NimbusMonoPS-Bold.otf")
    );
}

#[test]
fn rated_theme_assets_are_optional_with_normal_fallback() {
    let themes = ThemePacks::load(&assets_dir());
    let original = themes.get(ThemeChoice::Original);
    let high_contrast = themes.get(ThemeChoice::HighContrast);

    assert!(original.sprites.supports_rated());
    assert!(original
        .sprites
        .atlas_for(ContentMode::Rated)
        .ends_with("images/blocks-rated.png"));
    assert!(!high_contrast.sprites.supports_rated());
    assert_eq!(
        high_contrast.sprites.atlas_for(ContentMode::Rated),
        high_contrast.sprites.atlas_for(ContentMode::Normal)
    );
}

#[test]
fn content_mode_selects_standalone_gimp_sprite_paths() {
    let themes = ThemePacks::load(&assets_dir());
    let original = themes.get(ThemeChoice::Original);
    let high_contrast = themes.get(ThemeChoice::HighContrast);

    assert!(original
        .sprites
        .gimp_for(ContentMode::Normal)
        .ends_with("images/gimp.png"));
    assert!(original
        .sprites
        .gimp_for(ContentMode::Rated)
        .ends_with("images/gimp-rated.png"));
    assert_eq!(
        high_contrast.sprites.gimp_for(ContentMode::Rated),
        high_contrast.sprites.gimp_for(ContentMode::Normal)
    );
}

#[test]
fn ui_text_tone_preserves_normal_copy_and_isolates_rated_variants() {
    assert_eq!(UiTextTone::challenge_copy(ContentMode::Normal), "");
    assert_eq!(
        UiTextTone::challenge_copy(ContentMode::Rated),
        "wants a piece of yo' ass."
    );
    assert_eq!(
        UiTextTone::bazaar_waiting_copy(ContentMode::Normal, BazaarWaitingText::LocalWaiting),
        "Done. Waiting for opponent."
    );
    assert_eq!(
        UiTextTone::bazaar_waiting_copy(ContentMode::Rated, BazaarWaitingText::LocalWaiting),
        "Waiting for fat slut..."
    );
    assert_eq!(
        UiTextTone::game_result_copy(ContentMode::Normal, Some(false)),
        "Game over"
    );
    assert_eq!(
        UiTextTone::game_result_copy(ContentMode::Rated, Some(false)),
        "Nice loss, shithead."
    );
    assert_eq!(
        UiTextTone::game_result_copy(ContentMode::Rated, Some(true)),
        "Yer the shit!"
    );
}

#[test]
fn rated_game_over_message_uses_local_result() {
    let mut board = Board::empty();
    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            board.set(Coord { x, y }, Some(Cell::visible()));
        }
    }
    let local = LocalGame {
        game: TwoPlayerGame::with_boards(
            GameSeed::from_u64(1),
            board,
            GameSeed::from_u64(2),
            Board::empty(),
        ),
        computer: None,
        local_player: PlayerId::One,
        mode: LocalGameMode::LocalHumanVsHuman,
        network_session: None,
        network_lockstep: None,
        network_failed_closed: false,
        network_game_over_sent: false,
        network_result_claim_submitted: false,
        status_message: None,
    };

    assert_eq!(local.game.phase(), GamePhase::GameOver);
    assert_eq!(
        legacy_game_message_label(&local, ContentMode::Normal),
        "Game over"
    );
    assert_eq!(
        legacy_game_message_label(&local, ContentMode::Rated),
        "Nice loss, shithead."
    );
}

#[test]
fn controls_use_original_layout() {
    assert_eq!(
        controls_for(PlayerId::One, ControlScheme::Original).left,
        KeyCode::KeyJ
    );
    assert_eq!(
        controls_for(PlayerId::Two, ControlScheme::Original).left,
        KeyCode::KeyJ
    );
    assert_eq!(
        controls_for(PlayerId::One, ControlScheme::Original).fast_drop,
        KeyCode::Space
    );
}

#[test]
fn human_vs_computer_client_game_is_unranked() {
    let local = LocalGame::new_human_vs_computer(14);

    assert!(!local.game.is_ranked_game());
    assert!(local.computer.is_some());
    assert!(matches!(
        local.game.mode(),
        GameMode::HumanVsComputer { difficulty, .. } if difficulty.level == 14
    ));
}

#[test]
fn computer_opponent_frame_stays_visible_without_recon() {
    let local = LocalGame::new_human_vs_computer(DEFAULT_ERNIE_LEVEL);
    let mut recon = ReconPanel::default();

    assert!(player_view_visible(&local, &recon, PlayerId::One));
    assert!(player_view_visible(&local, &recon, PlayerId::Two));

    recon.manual_condor = true;
    assert!(player_view_visible(&local, &recon, PlayerId::Two));
}

#[test]
fn human_opponent_frame_is_hidden_without_recon() {
    let coord = Coord::new(0, BOARD_HEIGHT - 1).expect("fixture coordinate in bounds");
    let mut opponent_board = Board::empty();
    opponent_board.set(coord, Some(visible_cell(1)));
    let local = LocalGame {
        game: TwoPlayerGame::with_boards(
            GameSeed::from_u64(1),
            Board::empty(),
            GameSeed::from_u64(2),
            opponent_board,
        ),
        computer: None,
        local_player: PlayerId::One,
        mode: LocalGameMode::LocalHumanVsHuman,
        network_session: None,
        network_lockstep: None,
        network_failed_closed: false,
        network_game_over_sent: false,
        network_result_claim_submitted: false,
        status_message: None,
    };
    let recon = ReconPanel::default();
    let themes = ThemePacks::load(&assets_dir());
    let theme = themes.get(ThemeChoice::Original);

    assert!(player_view_visible(&local, &recon, PlayerId::One));
    assert!(!player_view_visible(&local, &recon, PlayerId::Two));
    assert_eq!(
        render_cell_sprite(&local, &recon, PlayerId::Two, coord.x, coord.y, theme).atlas_index,
        theme.cell_atlas.cells.empty
    );
}

#[test]
fn ernie_difficulty_selection_clamps_to_legacy_table() {
    let mut settings = ClientSettings {
        ernie_level: 0,
        settings_path: None,
        ..Default::default()
    };
    adjust_ernie_level(&mut settings, -1);
    assert_eq!(settings.ernie_level, 0);

    adjust_ernie_level(&mut settings, 99);
    assert_eq!(settings.ernie_level, COMPUTER_DIFFICULTIES.len() - 1);
    assert_eq!(selected_ernie_difficulty(&settings).name, "Bionic");
}

#[test]
fn held_key_repeat_uses_initial_delay_then_fixed_interval() {
    let (repeat, emit) = HeldKeyRepeat::default().observe(true, true, 0);
    assert!(emit);

    let (repeat, emit) = repeat.observe(true, false, INPUT_REPEAT_INITIAL_MS - 1);
    assert!(!emit);

    let (repeat, emit) = repeat.observe(true, false, 1);
    assert!(emit);

    let (_, emit) = repeat.observe(true, false, INPUT_REPEAT_MS);
    assert!(emit);
}

#[test]
fn sound_mapping_is_semantic_and_pack_swappable() {
    let line_clear = BattleEvent::PlayerEvent {
        player: PlayerId::One,
        event: CoreEvent::LinesCleared { lines: 1, funds: 0 },
    };

    assert_eq!(sound_event_for(&line_clear), Some(SoundEvent::LineClear));
    assert_eq!(
        sound_event_for(&BattleEvent::BazaarEntered),
        Some(SoundEvent::BazaarEntered)
    );
}

#[test]
fn client_settings_round_trip_as_toml() {
    let settings = PersistedClientSettings {
        ui_style: UiStyleChoice::Original,
        theme: ThemeChoice::HighContrast,
        sound_pack: SoundPackChoice::Muted,
        controls: ControlScheme::Original,
        pixel_scale: 1.5,
        ernie_level: 12,
        challenge_style: ChallengeStyle::Legacy,
        display_name: "Ada".to_string(),
        community_label: "garage".to_string(),
        direct_listen_addr: "0.0.0.0:4405".to_string(),
        direct_share_addr: "192.168.1.10:4405".to_string(),
        direct_join_addr: "192.168.1.10:4405".to_string(),
        lobby_addr: "127.0.0.1:4404".to_string(),
        hosted_ranked: false,
    };

    let encoded = toml::to_string_pretty(&settings).expect("settings encode");
    let decoded: PersistedClientSettings = toml::from_str(&encoded).expect("settings decode");

    assert_eq!(decoded, settings);
    assert!(encoded.contains("high-contrast"));
    assert!(encoded.contains("original"));
    assert!(encoded.contains("legacy"));
    assert!(encoded.contains("Ada"));
}

#[test]
fn settings_path_uses_existing_source_local_file_before_project_path() {
    let root =
        std::env::temp_dir().join(format!("battletris-settings-path-{}", std::process::id()));
    fs::create_dir_all(&root).expect("temp settings path dir is created");
    let local = root.join(SETTINGS_FILE_NAME);
    fs::write(&local, "").expect("temp settings file is created");

    assert_eq!(
        select_settings_path(
            vec![root.join("missing-settings.toml"), local.clone()],
            Some(root.join("project-settings.toml")),
        ),
        Some(local.clone())
    );

    let _ = fs::remove_file(local);
    let _ = fs::remove_dir(root);
}

#[test]
fn generated_sound_pack_maps_all_semantic_events() {
    let packs = SoundPacks::load(&assets_dir());

    for event in SoundEvent::ALL {
        let loaded = packs
            .sound_for(
                SoundPackChoice::GeneratedDefault,
                ContentMode::Normal,
                event,
            )
            .expect("generated-default maps every semantic event");
        assert!(loaded.file.ends_with(".wav"));
    }
    let rated_gimp = packs
        .sound_for(
            SoundPackChoice::GeneratedDefault,
            ContentMode::Rated,
            SoundEvent::WeaponLaunchGimp,
        )
        .expect("rated overlay maps rated gimp launch");
    assert!(rated_gimp.file.contains("generated-rated"));
    let rated_fallback = packs
        .sound_for(
            SoundPackChoice::GeneratedDefault,
            ContentMode::Rated,
            SoundEvent::LineClear,
        )
        .expect("rated mode falls back to generated-default");
    assert!(rated_fallback.file.contains("generated-default"));
    assert!(packs
        .sound_for(
            SoundPackChoice::Muted,
            ContentMode::Rated,
            SoundEvent::LineClear
        )
        .is_none());
}

#[test]
fn lobby_registration_preview_uses_protocol_identity() {
    let settings = ClientSettings {
        display_name: "Ada Lovelace".to_string(),
        direct_listen_addr: "0.0.0.0:4405".to_string(),
        direct_share_addr: "192.168.1.10:4405".to_string(),
        hosted_ranked: false,
        ..Default::default()
    };

    let preview = lobby_registration_preview(&settings);

    assert_eq!(preview.player.player_id, "ada-lovelace");
    assert_eq!(preview.player.display_name, "Ada Lovelace");
    assert_eq!(preview.direct_addr, "192.168.1.10:4405");
    assert!(!preview.ranked);
}

#[test]
fn share_address_never_persists_unspecified_bind_address() {
    let mut settings = ClientSettings::default();
    settings.apply_persisted(PersistedClientSettings {
        direct_listen_addr: "0.0.0.0:4405".to_string(),
        direct_share_addr: "0.0.0.0:4405".to_string(),
        ..Default::default()
    });

    assert_eq!(settings.direct_listen_addr, "0.0.0.0:4405");
    assert!(!socket_addr_is_unspecified(&settings.direct_share_addr));
    assert!(settings.direct_share_addr.ends_with(":4405"));
}

#[test]
fn invalid_network_settings_fall_back_to_safe_defaults() {
    let mut settings = ClientSettings::default();
    settings.apply_persisted(PersistedClientSettings {
        direct_listen_addr: "".to_string(),
        direct_share_addr: "not an address".to_string(),
        direct_join_addr: "also bad".to_string(),
        lobby_addr: "".to_string(),
        ..Default::default()
    });

    assert_eq!(settings.direct_listen_addr, "0.0.0.0:4405");
    assert!(!socket_addr_is_unspecified(&settings.direct_share_addr));
    assert_eq!(settings.direct_join_addr, "127.0.0.1:4405");
    assert_eq!(settings.lobby_addr, "127.0.0.1:4404");
}

#[test]
fn persisted_pixel_scale_is_sanitized() {
    let mut settings = ClientSettings::default();
    settings.apply_persisted(PersistedClientSettings {
        pixel_scale: f32::NAN,
        ..Default::default()
    });
    assert_eq!(settings.pixel_scale, 1.0);

    settings.apply_persisted(PersistedClientSettings {
        pixel_scale: 99.0,
        ..Default::default()
    });
    assert_eq!(settings.pixel_scale, 2.0);

    settings.apply_persisted(PersistedClientSettings {
        ernie_level: 99,
        ..Default::default()
    });
    assert_eq!(settings.ernie_level, COMPUTER_DIFFICULTIES.len() - 1);
}

struct TestClientPeer {
    runtime: ClientNetworkRuntime,
    state: ClientNetworkState,
    local: LocalGame,
    clock: ClientTickClock,
    settings: ClientSettings,
}

impl TestClientPeer {
    fn new(display_name: &str) -> Self {
        Self {
            runtime: ClientNetworkRuntime::default(),
            state: ClientNetworkState::default(),
            local: LocalGame::new_human_vs_human(),
            clock: ClientTickClock::default(),
            settings: ClientSettings {
                screen: ClientScreen::Challenge,
                display_name: display_name.to_string(),
                settings_path: None,
                ..Default::default()
            },
        }
    }

    fn shutdown(self) {
        self.runtime.runtime.shutdown();
    }
}

fn connect_test_client_peers(host: &mut TestClientPeer, joiner: &mut TestClientPeer, seed: u64) {
    try_send_network_command(
        &mut host.runtime,
        &mut host.state,
        NetworkCommand::Host {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            identity: direct_identity(&host.settings),
        },
    );
    wait_for_test_client_pair(host, joiner, |host, _| {
        host.state.listening_share_addr.is_some()
    });
    let peer_addr = host.state.listening_share_addr.unwrap();

    try_send_network_command(
        &mut joiner.runtime,
        &mut joiner.state,
        NetworkCommand::Join {
            peer_addr,
            identity: direct_identity(&joiner.settings),
            challenge_text: "battle?".to_string(),
        },
    );
    wait_for_test_client_pair(host, joiner, |host, _| {
        host.state.pending_challenge.is_some()
    });

    try_send_network_command(
        &mut host.runtime,
        &mut host.state,
        NetworkCommand::Accept {
            seed,
            ranked: false,
        },
    );
    wait_for_test_client_pair(host, joiner, |host, joiner| {
        host.local.is_networked() && joiner.local.is_networked()
    });
}

fn tick_test_client_peer(peer: &mut TestClientPeer) {
    peer.clock.gameplay_elapsed_ms = peer
        .clock
        .gameplay_elapsed_ms
        .saturating_add(CLIENT_FIXED_TICK_MS);
    peer.clock.network_checksum_elapsed_ms = NETWORK_CHECKSUM_INTERVAL_MS - CLIENT_FIXED_TICK_MS;
    tick_network_game(
        &mut peer.local,
        &mut peer.clock,
        &mut peer.runtime,
        &mut peer.state,
        &peer.settings,
    );
}

fn wait_for_test_client_pair(
    host: &mut TestClientPeer,
    joiner: &mut TestClientPeer,
    mut predicate: impl FnMut(&TestClientPeer, &TestClientPeer) -> bool,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !predicate(host, joiner) {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for test client pair"
        );
        pump_test_client_pair(host, joiner);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn pump_test_client_pair(host: &mut TestClientPeer, joiner: &mut TestClientPeer) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut idle_polls = 0;
    while idle_polls < 4 {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out pumping test client pair"
        );
        let host_progress = pump_test_client_peer(host);
        let joiner_progress = pump_test_client_peer(joiner);
        if host_progress || joiner_progress {
            idle_polls = 0;
        } else {
            idle_polls += 1;
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

fn pump_test_client_peer(peer: &mut TestClientPeer) -> bool {
    let mut progressed = false;
    loop {
        match peer.runtime.runtime.channels_mut().try_recv_event() {
            Ok(event) => {
                progressed = true;
                apply_network_game_event(
                    &mut peer.local,
                    &mut peer.runtime,
                    &mut peer.state,
                    &event,
                );
                let network_game_ended = matches!(
                    &event,
                    NetworkEvent::StateChanged(NetworkLifecycleState::Idle)
                        | NetworkEvent::Error { .. }
                ) && peer.local.is_networked();
                if let Some(session) = reduce_client_network_event(&mut peer.state, event) {
                    peer.local = LocalGame::new_networked(session);
                    peer.settings.screen = ClientScreen::Game;
                    peer.clock = ClientTickClock::default();
                } else if network_game_ended {
                    peer.local.network_session = None;
                    peer.local.network_lockstep = None;
                    peer.local.network_failed_closed = true;
                    peer.settings.screen = ClientScreen::Challenge;
                }
            }
            Err(mpsc::error::TryRecvError::Empty) => return progressed,
            Err(mpsc::error::TryRecvError::Disconnected) => {
                panic!("network event channel closed")
            }
        }
    }
}

fn catch_up_test_client_pair(host: &mut TestClientPeer, joiner: &mut TestClientPeer) {
    for _ in 0..20 {
        pump_test_client_pair(host, joiner);
        let host_tick = host.local.network_lockstep.as_ref().unwrap().current_tick();
        let joiner_tick = joiner
            .local
            .network_lockstep
            .as_ref()
            .unwrap()
            .current_tick();
        if host_tick == joiner_tick {
            return;
        }
        if host_tick < joiner_tick {
            tick_test_client_peer(host);
        } else {
            tick_test_client_peer(joiner);
        }
    }
}
