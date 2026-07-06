//! Desktop client application bootstrap.
//!
//! This crate hosts the Bevy application, rendering, menus, settings, audio
//! event mapping, and local keyboard input. It consumes deterministic core
//! state and events instead of owning gameplay rules.

use battletris_client::net::{
    build_ranked_result_claim, FinalResultStatus, LanAvailability, LanDiscoveryEntry,
    LegacyRemoteOpponentState, LegacyRemoteScoreUpdate, NetworkCommand, NetworkEvent,
    NetworkLifecycleState, NetworkLockstep, NetworkMode, NetworkRuntime, NetworkSession,
};
use battletris_core::{
    ai::{computer_difficulty, ComputerOpponent, BAZAAR_LEAVE_DELAY_MS, COMPUTER_DIFFICULTIES},
    board::{Board, Coord, BOARD_HEIGHT, BOARD_WIDTH},
    cell::{Cell, Pip, VisibleColor},
    game::{
        BattleEvent, Command, CoreEvent, GameMode, GamePhase, LoggedEvent, PlayerId, TwoPlayerGame,
    },
    piece::PieceKind,
    recon::{ReconLevel, ReconSnapshot},
    rng::GameSeed,
    weapons::{weapon_spec, WeaponToken, WEAPON_CATALOG},
};
use battletris_db::{CommunityLabel, PersistencePaths, PlayerStore, StreakKind};
use battletris_protocol::{
    derive_player_seeds, ArsenalEntry, ArsenalSnapshot, BazaarBuy, BazaarDone, BazaarRemove,
    BoardSnapshot as WireBoardSnapshot, Challenge, GameChecksum, GameOver, Heartbeat,
    HostedGameStart, HostedPlayer, HostedSessionStatus, HostedSessionStatusKind, InputCommand,
    LobbyEntry, LobbyList, LobbyRegister, PlayerIdentity, PlayerInput, PlayerSlot, RankedRecords,
    RankedResultPending, RankedResultRejected, ScoreSnapshot, TickWatermark, CAPABILITY_DIRECT_TCP,
    CAPABILITY_SELF_HOSTED_LOBBY, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};
use bevy::ecs::system::SystemParam;
use bevy::image::ImageSampler;
use bevy::log::{debug, error, info, warn};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::sprite::Anchor;
use bevy::text::{FontSmoothing, FontWeight, LetterSpacing, LineHeight};
use bevy::window::PrimaryWindow;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    ffi::{OsStr, OsString},
    fmt::Write as _,
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc;

mod audio;
mod cli;
mod gameplay;
mod model;
mod networking;
mod settings;
mod theme;
mod ui;
mod visual;

#[cfg(test)]
mod tests;

use self::audio::*;
use self::cli::*;
use self::gameplay::*;
use self::model::*;
use self::networking::*;
use self::settings::*;
use self::theme::*;
use self::ui::*;
use self::visual::*;

const SMOKE_SCREENSHOT_CAPTURE_DELAY_FRAMES: u16 = 30;
const SMOKE_SCREENSHOT_TIMEOUT_FRAMES: u16 = 300;
const SETTINGS_FILE_NAME: &str = "settings.toml";
const DEFAULT_MODERN_SERVER_ADDR: &str = "127.0.0.1:4405";
const DEFAULT_LEGACY_SERVER_ADDR: &str = "127.0.0.1:4404";
const CLIENT_FIXED_TICK_MS: u64 = 10;
const NETWORK_HEARTBEAT_INTERVAL_MS: u64 = 1_000;
const NETWORK_CHECKSUM_INTERVAL_MS: u64 = 5_000;
const HOSTED_STATUS_POLL_INTERVAL_MS: u64 = 1_000;
const LOBBY_BROWSE_REFRESH_INTERVAL_MS: u64 = 3_000;
const INPUT_REPEAT_INITIAL_MS: u64 = 150;
const INPUT_REPEAT_MS: u64 = 50;
const DEFAULT_ERNIE_LEVEL: usize = 7;
const LEGACY_GAME_WIDTH: f32 = 934.0;
const LEGACY_GAME_HEIGHT: f32 = 700.0;
const LEGACY_GAME_SCORE_X: f32 = 300.0;
const LEGACY_GAME_SCORE_Y: f32 = 30.0;
const LEGACY_GAME_SCORE_WIDTH: f32 = 325.0;
const LEGACY_GAME_SCORE_HEIGHT: f32 = 210.0;
const LEGACY_GAME_ARSENAL_X: f32 = 300.0;
const LEGACY_GAME_ARSENAL_Y: f32 = 270.0;
const LEGACY_GAME_ARSENAL_WIDTH: f32 = 325.0;
const LEGACY_GAME_ARSENAL_ROW_HEIGHT: f32 = 35.0;
const LEGACY_BAZAAR_WIDTH: f32 = 800.0;
const LEGACY_BAZAAR_HEIGHT: f32 = 800.0;
const LEGACY_ROSTER_WIDTH: f32 = 640.0;
const LEGACY_ROSTER_HEIGHT: f32 = 600.0;
const LEGACY_ROSTER_BIFF_WIDTH: f32 = 99.0;
const LEGACY_ROSTER_BIFF_HEIGHT: f32 = 105.0;

pub(crate) fn run() {
    let run_config = ClientRunConfig::from_env().unwrap_or_else(|error| {
        eprintln!("{error}\n\n{}", client_usage());
        std::process::exit(2);
    });
    run_client(run_config);
}

fn run_client(run_config: ClientRunConfig) {
    let mut settings = if run_config.deterministic_capture {
        ClientSettings::default()
    } else {
        ClientSettings::load_or_default()
    };
    settings.content_mode = run_config.content_mode;
    run_config.session_overrides.apply_to(&mut settings);
    if run_config.deterministic_capture {
        settings.sound_pack = SoundPackChoice::Muted;
        settings.settings_path = None;
        settings.pixel_scale = 1.0;
    }
    let themes = ThemePacks::load(&settings.assets_dir);
    let sound_packs = SoundPacks::load(&settings.assets_dir);
    let visual_capture = run_config
        .capture
        .as_ref()
        .map(|spec| spec.to_capture(&themes, settings.theme));
    if let Some(capture) = &visual_capture {
        if let Some(job) = capture.jobs.first() {
            settings.theme = job.theme;
        }
    }
    let mut local_game = LocalGame::new_human_vs_human();
    let mut recon_panel = ReconPanel::default();
    let mut bazaar_ui = BazaarUiState::default();
    let mut roster_records = if run_config.deterministic_capture {
        visual_roster_records()
    } else {
        RosterRecords::load()
    };
    if let Some(capture) = &visual_capture {
        if let Some(job) = capture.jobs.first() {
            apply_visual_fixture_state(
                job.fixture,
                &mut settings,
                &mut local_game,
                &mut recon_panel,
                &mut bazaar_ui,
                &mut roster_records,
            );
        }
    }
    let window = themes.get(settings.theme).layout.screen(settings.screen);
    let asset_file_path = settings.assets_dir.to_string_lossy().into_owned();
    let mut app = App::new();
    app.insert_resource(ClearColor(themes.get(settings.theme).screen.background))
        .insert_resource(local_game)
        .insert_resource(ClientTickClock::default())
        .insert_resource(InputRepeatState::default())
        .insert_resource(recon_panel)
        .insert_resource(bazaar_ui)
        .insert_resource(themes)
        .insert_resource(sound_packs)
        .insert_resource(settings)
        .insert_resource(SettingsEditState::default())
        .insert_resource(SoundEventState::default())
        .insert_resource(roster_records)
        .insert_resource(ClientNetworkRuntime::default())
        .insert_resource(ClientNetworkState::default())
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: asset_file_path,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "BattleTris".into(),
                        resolution: (window.width as u32, window.height as u32).into(),
                        resizable: false,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_systems(
            Startup,
            (log_content_mode, load_theme_atlases, setup).chain(),
        )
        .add_systems(Update, apply_visual_capture_fixture.before(render_game))
        .add_systems(
            Update,
            (
                update_window_layout.after(apply_visual_capture_fixture),
                pump_network_events,
                maintain_sleep_availability,
                refresh_hosted_lobby,
                refresh_server_roster,
                handle_keyboard_input,
                handle_mouse_buttons,
                drive_computer_opponent,
                tick_game,
                update_recon_panel,
                collect_sound_events,
                play_sound_events,
                render_game,
                update_theme_entities,
                update_challenge_logo_texture.after(update_theme_entities),
                update_screen_visibility,
                update_menu_button_visuals,
            ),
        )
        .add_systems(
            Update,
            (
                handle_settings_ui_interactions.after(handle_keyboard_input),
                update_settings_ui_visibility,
                update_settings_ui_dropdown_visibility
                    .after(handle_keyboard_input)
                    .after(handle_settings_ui_interactions),
                update_settings_ui_text
                    .after(handle_keyboard_input)
                    .after(handle_settings_ui_interactions),
                update_settings_ui_theme
                    .after(handle_settings_ui_interactions)
                    .after(update_theme_entities),
                update_settings_ui_visuals
                    .after(handle_keyboard_input)
                    .after(handle_settings_ui_interactions)
                    .after(update_settings_ui_theme),
            ),
        );

    if let Some(visual_capture) = visual_capture {
        app.insert_resource(visual_capture).add_systems(
            Update,
            request_visual_capture
                .after(render_game)
                .after(update_screen_visibility),
        );
    }

    app.run();
}
