//! Desktop client entry point.
//!
//! This crate hosts the Bevy application, rendering, menus, settings, audio
//! event mapping, and local keyboard input. It consumes deterministic core
//! state and events instead of owning gameplay rules.

use battletris_core::{
    ai::{computer_difficulty, ComputerOpponent, BAZAAR_LEAVE_DELAY_MS, COMPUTER_DIFFICULTIES},
    board::{Board, Coord, BOARD_HEIGHT, BOARD_WIDTH},
    cell::{Cell, Pip, VisibleColor},
    game::{BattleEvent, Command, CoreEvent, GameMode, GamePhase, PlayerId, TwoPlayerGame},
    piece::PieceKind,
    recon::{ReconLevel, ReconSnapshot},
    rng::GameSeed,
    weapons::{weapon_spec, WeaponToken, WEAPON_CATALOG},
};
use battletris_db::{CommunityLabel, PersistencePaths, PlayerStore, StreakKind};
use battletris_protocol::{
    HostedPlayer, LobbyRegister, CAPABILITY_DIRECT_TCP, CAPABILITY_SELF_HOSTED_LOBBY,
    PROTOCOL_MAJOR, PROTOCOL_MINOR,
};
use bevy::ecs::system::SystemParam;
use bevy::image::ImageSampler;
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
};

const SMOKE_SCREENSHOT_CAPTURE_DELAY_FRAMES: u16 = 30;
const SMOKE_SCREENSHOT_TIMEOUT_FRAMES: u16 = 300;
const SETTINGS_FILE_NAME: &str = "settings.toml";
const CLIENT_FIXED_TICK_MS: u64 = 10;
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

fn main() {
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
    let window = themes
        .get(settings.theme)
        .layout
        .screen(ClientScreen::Startup);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClientScreen {
    Startup,
    Game,
    Challenge,
    Sleep,
    About,
    Roster,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ThemeChoice {
    Original,
    HighContrast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SoundPackChoice {
    GeneratedDefault,
    Muted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentMode {
    Normal,
    Rated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChallengeMode {
    ComputerOpponent,
    HostDirect,
    JoinDirect,
    HostViaLobby,
    BrowseLobby,
}

impl ChallengeMode {
    const fn label(self) -> &'static str {
        match self {
            Self::ComputerOpponent => "Computer Opponent",
            Self::HostDirect => "Host Direct",
            Self::JoinDirect => "Join Direct",
            Self::HostViaLobby => "Host Via Lobby",
            Self::BrowseLobby => "Browse Lobby",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsField {
    DisplayName,
    CommunityLabel,
    HostBindAddress,
    ShareAddress,
    JoinAddress,
    LobbyAddress,
}

impl SettingsField {
    const ALL: [Self; 6] = [
        Self::DisplayName,
        Self::CommunityLabel,
        Self::HostBindAddress,
        Self::ShareAddress,
        Self::JoinAddress,
        Self::LobbyAddress,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::DisplayName => "display name",
            Self::CommunityLabel => "community",
            Self::HostBindAddress => "host bind",
            Self::ShareAddress => "share address",
            Self::JoinAddress => "join address",
            Self::LobbyAddress => "lobby address",
        }
    }
}

#[derive(Resource, Debug, Clone)]
struct SettingsEditState {
    field: SettingsField,
}

impl Default for SettingsEditState {
    fn default() -> Self {
        Self {
            field: SettingsField::DisplayName,
        }
    }
}

impl ContentMode {
    const fn id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Rated => "rated",
        }
    }
}

impl SoundPackChoice {
    const fn directory(self) -> &'static str {
        match self {
            Self::GeneratedDefault => "generated-default",
            Self::Muted => "muted",
        }
    }
}

impl ThemeChoice {
    const fn directory(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::HighContrast => "high-contrast",
        }
    }

    fn from_id(value: &str) -> Option<Self> {
        [Self::Original, Self::HighContrast]
            .into_iter()
            .find(|choice| choice.directory() == value)
    }
}

#[derive(Debug, Clone)]
struct ClientRunConfig {
    capture: Option<VisualCaptureSpec>,
    deterministic_capture: bool,
    content_mode: ContentMode,
}

impl ClientRunConfig {
    fn from_env() -> Result<Self, String> {
        let args = std::env::args_os().skip(1).collect::<Vec<_>>();
        if args.len() == 1 && is_help_arg(&args[0])
            || args
                .first()
                .is_some_and(|arg| arg == OsStr::new("headless"))
                && args.get(1).is_some_and(|arg| is_help_arg(arg))
        {
            println!("{}", client_usage());
            std::process::exit(0);
        }
        Self::parse(args, std::env::var_os("BATTLETRIS_SMOKE_SCREENSHOT"))
    }

    fn parse(args: Vec<OsString>, smoke_env: Option<OsString>) -> Result<Self, String> {
        let (content_mode, args) = parse_content_mode_args(args);
        if args.is_empty() {
            return Ok(Self {
                capture: smoke_env.map(|path| VisualCaptureSpec::Smoke { path: path.into() }),
                deterministic_capture: false,
                content_mode,
            });
        }

        if args
            .first()
            .is_some_and(|arg| arg == OsStr::new("headless"))
        {
            return parse_headless_args(&args[1..], content_mode);
        }

        if args.len() == 1 && is_help_arg(&args[0]) {
            return Err(client_usage());
        }

        let mut index = 0;
        let mut smoke_path = smoke_env.map(PathBuf::from);
        while index < args.len() {
            let arg = &args[index];
            if arg == OsStr::new("--smoke-screenshot") {
                index += 1;
                let Some(path) = args.get(index) else {
                    return Err("--smoke-screenshot requires a path".to_string());
                };
                smoke_path = Some(PathBuf::from(path));
            } else if let Some(path) = arg
                .to_str()
                .and_then(|arg| arg.strip_prefix("--smoke-screenshot="))
            {
                smoke_path = Some(PathBuf::from(path));
            } else {
                return Err(format!(
                    "unrecognized client argument: {}",
                    display_arg(arg)
                ));
            }
            index += 1;
        }

        Ok(Self {
            capture: smoke_path.map(|path| VisualCaptureSpec::Smoke { path }),
            deterministic_capture: false,
            content_mode,
        })
    }
}

fn parse_content_mode_args(args: Vec<OsString>) -> (ContentMode, Vec<OsString>) {
    let mut content_mode = ContentMode::Normal;
    let mut remaining = Vec::with_capacity(args.len());
    for arg in args {
        if arg == OsStr::new("--rated") || arg == OsStr::new("-r") {
            content_mode = ContentMode::Rated;
        } else {
            remaining.push(arg);
        }
    }
    (content_mode, remaining)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisualFixture {
    Startup,
    Challenge,
    Sleep,
    About,
    Roster,
    Settings,
    GamePlaying,
    GameBazaar,
    GameOver,
    GameRecon,
    BoardCells,
}

impl VisualFixture {
    const ALL: [Self; 11] = [
        Self::Startup,
        Self::Challenge,
        Self::Sleep,
        Self::About,
        Self::Roster,
        Self::Settings,
        Self::GamePlaying,
        Self::GameBazaar,
        Self::GameOver,
        Self::GameRecon,
        Self::BoardCells,
    ];

    const fn id(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Challenge => "challenge",
            Self::Sleep => "sleep",
            Self::About => "about",
            Self::Roster => "roster",
            Self::Settings => "settings",
            Self::GamePlaying => "game-playing",
            Self::GameBazaar => "game-bazaar",
            Self::GameOver => "game-over",
            Self::GameRecon => "game-recon",
            Self::BoardCells => "board-cells",
        }
    }

    fn from_id(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|fixture| fixture.id() == value)
    }

    const fn screen(self) -> ClientScreen {
        match self {
            Self::Startup => ClientScreen::Startup,
            Self::Challenge => ClientScreen::Challenge,
            Self::Sleep => ClientScreen::Sleep,
            Self::About => ClientScreen::About,
            Self::Roster => ClientScreen::Roster,
            Self::Settings => ClientScreen::Settings,
            Self::GamePlaying
            | Self::GameBazaar
            | Self::GameOver
            | Self::GameRecon
            | Self::BoardCells => ClientScreen::Game,
        }
    }
}

#[derive(Debug, Clone)]
enum VisualCaptureSpec {
    Smoke {
        path: PathBuf,
    },
    One {
        fixture: VisualFixture,
        theme: ThemeChoice,
        output: PathBuf,
    },
    All {
        theme: ThemeChoice,
        out_dir: PathBuf,
    },
}

impl VisualCaptureSpec {
    fn to_capture(&self, themes: &ThemePacks, default_theme: ThemeChoice) -> VisualCapture {
        let jobs = match self {
            Self::Smoke { path } => vec![visual_capture_job(
                VisualFixture::Startup,
                default_theme,
                path.clone(),
                themes,
            )],
            Self::One {
                fixture,
                theme,
                output,
            } => vec![visual_capture_job(*fixture, *theme, output.clone(), themes)],
            Self::All { theme, out_dir } => VisualFixture::ALL
                .into_iter()
                .map(|fixture| {
                    visual_capture_job(
                        fixture,
                        *theme,
                        out_dir.join(format!("{}.png", fixture.id())),
                        themes,
                    )
                })
                .collect(),
        };
        VisualCapture::new(jobs)
    }
}

#[derive(Resource, Debug)]
struct VisualCapture {
    jobs: Vec<VisualCaptureJob>,
    current: usize,
    applied: Option<usize>,
    frames_until_capture: u16,
    frames_since_request: u16,
    requested: bool,
}

impl VisualCapture {
    fn new(jobs: Vec<VisualCaptureJob>) -> Self {
        Self {
            jobs,
            current: 0,
            applied: None,
            frames_until_capture: SMOKE_SCREENSHOT_CAPTURE_DELAY_FRAMES,
            frames_since_request: 0,
            requested: false,
        }
    }
}

#[derive(Debug, Clone)]
struct VisualCaptureJob {
    fixture: VisualFixture,
    theme: ThemeChoice,
    path: PathBuf,
    expected_width: u32,
    expected_height: u32,
}

fn visual_capture_job(
    fixture: VisualFixture,
    theme: ThemeChoice,
    path: PathBuf,
    themes: &ThemePacks,
) -> VisualCaptureJob {
    let window = themes.get(theme).layout.fixture(fixture);
    VisualCaptureJob {
        fixture,
        theme,
        path,
        expected_width: window.width.round() as u32,
        expected_height: window.height.round() as u32,
    }
}

fn parse_headless_args(
    args: &[OsString],
    content_mode: ContentMode,
) -> Result<ClientRunConfig, String> {
    let Some(command) = args.first() else {
        return Err("headless requires a command: capture or capture-all".to_string());
    };
    if is_help_arg(command) {
        return Err(client_usage());
    }

    match command.to_str() {
        Some("capture") => parse_headless_capture_args(&args[1..], content_mode),
        Some("capture-all") => parse_headless_capture_all_args(&args[1..], content_mode),
        Some(other) => Err(format!("unrecognized headless command: {other}")),
        None => Err(format!(
            "headless command is not valid UTF-8: {}",
            display_arg(command)
        )),
    }
}

fn parse_headless_capture_args(
    args: &[OsString],
    content_mode: ContentMode,
) -> Result<ClientRunConfig, String> {
    let mut fixture = None;
    let mut theme = ThemeChoice::Original;
    let mut output = None;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        if let Some(value) = option_value(arg, "--fixture") {
            fixture = Some(parse_visual_fixture(value)?);
        } else if arg == OsStr::new("--fixture") {
            index += 1;
            fixture = Some(parse_visual_fixture(required_arg(
                args,
                index,
                "--fixture",
            )?)?);
        } else if let Some(value) = option_value(arg, "--theme") {
            theme = parse_theme_choice(value)?;
        } else if arg == OsStr::new("--theme") {
            index += 1;
            theme = parse_theme_choice(required_arg(args, index, "--theme")?)?;
        } else if let Some(value) = option_value(arg, "--output") {
            output = Some(PathBuf::from(value));
        } else if arg == OsStr::new("--output") {
            index += 1;
            output = Some(PathBuf::from(required_os_arg(args, index, "--output")?));
        } else {
            return Err(format!(
                "unrecognized headless capture argument: {}",
                display_arg(arg)
            ));
        }
        index += 1;
    }

    Ok(ClientRunConfig {
        capture: Some(VisualCaptureSpec::One {
            fixture: fixture.ok_or_else(|| "headless capture requires --fixture".to_string())?,
            theme,
            output: output.ok_or_else(|| "headless capture requires --output".to_string())?,
        }),
        deterministic_capture: true,
        content_mode,
    })
}

fn parse_headless_capture_all_args(
    args: &[OsString],
    content_mode: ContentMode,
) -> Result<ClientRunConfig, String> {
    let mut theme = ThemeChoice::Original;
    let mut out_dir = None;
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        if let Some(value) = option_value(arg, "--theme") {
            theme = parse_theme_choice(value)?;
        } else if arg == OsStr::new("--theme") {
            index += 1;
            theme = parse_theme_choice(required_arg(args, index, "--theme")?)?;
        } else if let Some(value) = option_value(arg, "--out-dir") {
            out_dir = Some(PathBuf::from(value));
        } else if arg == OsStr::new("--out-dir") {
            index += 1;
            out_dir = Some(PathBuf::from(required_os_arg(args, index, "--out-dir")?));
        } else {
            return Err(format!(
                "unrecognized headless capture-all argument: {}",
                display_arg(arg)
            ));
        }
        index += 1;
    }

    Ok(ClientRunConfig {
        capture: Some(VisualCaptureSpec::All {
            theme,
            out_dir: out_dir
                .ok_or_else(|| "headless capture-all requires --out-dir".to_string())?,
        }),
        deterministic_capture: true,
        content_mode,
    })
}

fn parse_visual_fixture(value: &str) -> Result<VisualFixture, String> {
    VisualFixture::from_id(value).ok_or_else(|| {
        format!(
            "unknown visual fixture '{value}'; expected one of: {}",
            visual_fixture_list()
        )
    })
}

fn parse_theme_choice(value: &str) -> Result<ThemeChoice, String> {
    ThemeChoice::from_id(value)
        .ok_or_else(|| format!("unknown theme '{value}'; expected original or high-contrast"))
}

fn required_arg<'a>(args: &'a [OsString], index: usize, option: &str) -> Result<&'a str, String> {
    required_os_arg(args, index, option)?
        .to_str()
        .ok_or_else(|| {
            format!(
                "{option} value is not valid UTF-8: {}",
                display_arg(&args[index])
            )
        })
}

fn required_os_arg<'a>(
    args: &'a [OsString],
    index: usize,
    option: &str,
) -> Result<&'a OsStr, String> {
    args.get(index)
        .map(OsString::as_os_str)
        .ok_or_else(|| format!("{option} requires a value"))
}

fn option_value<'a>(arg: &'a OsStr, option: &str) -> Option<&'a str> {
    arg.to_str()
        .and_then(|arg| arg.strip_prefix(option))
        .and_then(|rest| rest.strip_prefix('='))
}

fn display_arg(arg: &OsStr) -> String {
    arg.to_string_lossy().into_owned()
}

fn is_help_arg(arg: &OsStr) -> bool {
    arg == OsStr::new("--help") || arg == OsStr::new("-h")
}

fn visual_fixture_list() -> String {
    VisualFixture::ALL
        .into_iter()
        .map(VisualFixture::id)
        .collect::<Vec<_>>()
        .join(", ")
}

fn client_usage() -> String {
    format!(
        "Usage:\n  client [--rated|-r]\n  client [--rated|-r] --smoke-screenshot <path>\n  client [--rated|-r] headless capture --fixture <fixture> --theme <theme> [--rated|-r] --output <path>\n  client [--rated|-r] headless capture-all --theme <theme> [--rated|-r] --out-dir <dir>\n\nFixtures: {}\nThemes: original, high-contrast",
        visual_fixture_list()
    )
}

#[derive(Resource, Debug, Clone)]
struct SoundPacks {
    generated_default: LoadedSoundPack,
    generated_rated: LoadedSoundPack,
}

impl SoundPacks {
    fn load(assets_dir: &std::path::Path) -> Self {
        Self {
            generated_default: LoadedSoundPack::load(assets_dir, SoundPackChoice::GeneratedDefault),
            generated_rated: LoadedSoundPack::load_overlay(assets_dir, "generated-rated"),
        }
    }

    fn sound_for(
        &self,
        choice: SoundPackChoice,
        content_mode: ContentMode,
        event: SoundEvent,
    ) -> Option<&LoadedSoundEvent> {
        match (choice, content_mode) {
            (SoundPackChoice::GeneratedDefault, ContentMode::Rated) => self
                .generated_rated
                .event(event)
                .or_else(|| self.generated_default.event(event)),
            (SoundPackChoice::GeneratedDefault, ContentMode::Normal) => {
                self.generated_default.event(event)
            }
            (SoundPackChoice::Muted, _) => None,
        }
    }
}

#[derive(Debug, Clone)]
struct LoadedSoundPack {
    events: Vec<LoadedSoundEvent>,
}

impl LoadedSoundPack {
    fn load(assets_dir: &std::path::Path, choice: SoundPackChoice) -> Self {
        Self::load_from_dir(assets_dir, choice.directory(), true)
    }

    fn load_overlay(assets_dir: &std::path::Path, directory: &'static str) -> Self {
        Self::load_from_dir(assets_dir, directory, false)
    }

    fn load_from_dir(
        assets_dir: &std::path::Path,
        directory: &'static str,
        require_all_events: bool,
    ) -> Self {
        let sound_dir = assets_dir.join("sounds").join(directory);
        let manifest_path = sound_dir.join("sound-pack.toml");
        let contents = fs::read_to_string(&manifest_path).unwrap_or_else(|error| {
            panic!(
                "BattleTris sound-pack manifest {} could not be read: {error}",
                manifest_path.display()
            )
        });
        let raw: RawSoundPack = toml::from_str(&contents).unwrap_or_else(|error| {
            panic!(
                "BattleTris sound-pack manifest {} could not be parsed: {error}",
                manifest_path.display()
            )
        });
        raw.validate(&sound_dir, &manifest_path, require_all_events);
        let prefix = format!("sounds/{directory}/");
        Self {
            events: raw
                .event
                .into_iter()
                .filter_map(|event| {
                    let kind = SoundEvent::from_id(&event.id)?;
                    Some(LoadedSoundEvent {
                        kind,
                        file: format!("{prefix}{}", event.files[0]),
                    })
                })
                .collect(),
        }
    }

    fn event(&self, kind: SoundEvent) -> Option<&LoadedSoundEvent> {
        self.events.iter().find(|event| event.kind == kind)
    }
}

#[derive(Debug, Clone)]
struct LoadedSoundEvent {
    kind: SoundEvent,
    file: String,
}

#[derive(Debug, Deserialize)]
struct RawSoundPack {
    kind: String,
    format_version: u32,
    event: Vec<RawSoundEvent>,
}

impl RawSoundPack {
    fn validate(
        &self,
        sound_dir: &std::path::Path,
        manifest_path: &std::path::Path,
        require_all_events: bool,
    ) {
        if self.kind != "sound-pack" || self.format_version != 1 {
            panic!(
                "BattleTris sound-pack manifest {} has unsupported kind/version: kind={} format_version={}",
                manifest_path.display(),
                self.kind,
                self.format_version
            );
        }
        if require_all_events {
            for expected in SoundEvent::ALL {
                if !self.event.iter().any(|event| event.id == expected.id()) {
                    panic!(
                        "BattleTris sound-pack manifest {} is missing event {}",
                        manifest_path.display(),
                        expected.id()
                    );
                }
            }
        }
        for event in &self.event {
            if SoundEvent::from_id(&event.id).is_none() {
                panic!(
                    "BattleTris sound-pack manifest {} has unknown event {}",
                    manifest_path.display(),
                    event.id
                );
            }
            if event.files.is_empty() || !event.volume.is_finite() || event.volume < 0.0 {
                panic!(
                    "BattleTris sound-pack manifest {} has invalid event {}",
                    manifest_path.display(),
                    event.id
                );
            }
            for relative in &event.files {
                let path = sound_dir.join(relative);
                if !path.is_file() {
                    panic!(
                        "BattleTris sound-pack manifest {} requires missing sound {}",
                        manifest_path.display(),
                        path.display()
                    );
                }
                validate_wav_file(&path, manifest_path);
            }
        }
    }
}

fn validate_wav_file(path: &std::path::Path, manifest_path: &std::path::Path) {
    let bytes = fs::read(path).unwrap_or_else(|error| {
        panic!(
            "BattleTris sound-pack manifest {} could not read WAV {}: {error}",
            manifest_path.display(),
            path.display()
        )
    });
    if bytes.len() < 44
        || &bytes[0..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || &bytes[12..16] != b"fmt "
        || u16::from_le_bytes([bytes[20], bytes[21]]) != 1
        || u16::from_le_bytes([bytes[34], bytes[35]]) != 16
        || !bytes.windows(4).any(|chunk| chunk == b"data")
    {
        panic!(
            "BattleTris sound-pack manifest {} references undecodable PCM WAV {}",
            manifest_path.display(),
            path.display()
        );
    }
}

#[derive(Debug, Deserialize)]
struct RawSoundEvent {
    id: String,
    files: Vec<String>,
    volume: f32,
}

#[derive(Resource, Debug, Clone)]
struct ThemePacks {
    original: LoadedTheme,
    high_contrast: LoadedTheme,
}

impl ThemePacks {
    fn load(assets_dir: &std::path::Path) -> Self {
        Self {
            original: LoadedTheme::load(assets_dir, ThemeChoice::Original),
            high_contrast: LoadedTheme::load(assets_dir, ThemeChoice::HighContrast),
        }
    }

    const fn get(&self, choice: ThemeChoice) -> &LoadedTheme {
        match choice {
            ThemeChoice::Original => &self.original,
            ThemeChoice::HighContrast => &self.high_contrast,
        }
    }
}

#[derive(Debug, Clone)]
struct LoadedTheme {
    sprites: LoadedThemeSprites,
    fonts: LoadedThemeFonts,
    cell: ThemeCell,
    cell_atlas: ThemeCellAtlas,
    layout: ThemeLayout,
    palette: ThemePalette,
    screen: ThemeScreenStyle,
    button: ThemeButtonStyle,
    about: ThemeAboutStyle,
}

impl LoadedTheme {
    fn load(assets_dir: &std::path::Path, choice: ThemeChoice) -> Self {
        let theme_dir = assets_dir.join("themes").join(choice.directory());
        let manifest_path = theme_dir.join("theme.toml");
        let contents = fs::read_to_string(&manifest_path).unwrap_or_else(|error| {
            panic!(
                "BattleTris theme manifest {} could not be read: {error}",
                manifest_path.display()
            )
        });
        let raw: RawTheme = toml::from_str(&contents).unwrap_or_else(|error| {
            panic!(
                "BattleTris theme manifest {} could not be parsed: {error}",
                manifest_path.display()
            )
        });
        raw.validate(&theme_dir, &manifest_path);
        Self {
            sprites: raw.sprites.loaded(choice),
            fonts: raw.fonts.loaded(choice),
            cell: raw.cell,
            cell_atlas: raw.sprites.cell_atlas,
            layout: raw.layout,
            palette: raw.semantic.palette(&manifest_path),
            screen: raw.screen.into_style(&manifest_path),
            button: raw.semantic.button(&manifest_path),
            about: raw.about.into_style(&manifest_path),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawTheme {
    id: String,
    name: String,
    kind: String,
    format_version: u32,
    sprites: ThemeSprites,
    fonts: ThemeFonts,
    cell: ThemeCell,
    layout: ThemeLayout,
    semantic: RawThemeSemantic,
    screen: RawThemeScreenStyle,
    about: RawThemeAboutStyle,
    description: String,
    author: String,
    license: String,
    default_scale: f32,
    pixel_filtering: String,
    supports_high_contrast: bool,
    provenance: ThemeProvenance,
}

impl RawTheme {
    fn validate(&self, theme_dir: &std::path::Path, manifest_path: &std::path::Path) {
        let _accessibility_flag = self.supports_high_contrast;
        if self.kind != "theme" || self.format_version != 1 {
            panic!(
                "BattleTris theme manifest {} has unsupported kind/version: kind={} format_version={}",
                manifest_path.display(),
                self.kind,
                self.format_version
            );
        }
        if self.id.trim().is_empty()
            || self.name.trim().is_empty()
            || self.description.trim().is_empty()
            || self.author.trim().is_empty()
            || self.license.trim().is_empty()
            || self.default_scale <= 0.0
            || !matches!(self.pixel_filtering.as_str(), "nearest" | "linear")
            || self.cell.size <= 0.0
            || self.cell.gap < 0.0
            || self.cell.shadow < 0.0
            || self.layout.board.spacing <= 0.0
            || self.screen.title_font_size <= 0.0
            || self.screen.body_font_size <= 0.0
            || self.screen.button_font_size <= 0.0
            || self.fonts.line_height <= 0.0
        {
            panic!(
                "BattleTris theme manifest {} has invalid metadata or layout values",
                manifest_path.display()
            );
        }
        self.layout.validate(manifest_path);
        self.sprites.cell_atlas.validate(manifest_path);
        self.semantic.validate(manifest_path);
        if self.provenance.notes.trim().is_empty() || self.provenance.sources.is_empty() {
            panic!(
                "BattleTris theme manifest {} requires provenance notes and at least one source",
                manifest_path.display()
            );
        }
        for relative in [
            &self.sprites.atlas,
            &self.sprites.startup,
            &self.sprites.bazaar,
            &self.sprites.biff,
            &self.sprites.gimp,
            &self.sprites.crest,
        ] {
            let path = theme_dir.join(relative);
            if !path.is_file() {
                panic!(
                    "BattleTris theme manifest {} requires missing asset {}",
                    manifest_path.display(),
                    path.display()
                );
            }
        }
        if let Some(rated) = &self.sprites.rated {
            for relative in [&rated.atlas, &rated.gimp] {
                let path = theme_dir.join(relative);
                if !path.is_file() {
                    panic!(
                        "BattleTris theme manifest {} requires missing rated asset {}",
                        manifest_path.display(),
                        path.display()
                    );
                }
            }
        }
        self.sprites
            .cell_atlas
            .validate_image(theme_dir, &self.sprites.atlas, manifest_path);
        if let Some(rated) = &self.sprites.rated {
            self.sprites
                .cell_atlas
                .validate_image(theme_dir, &rated.atlas, manifest_path);
        }
        for font in [&self.fonts.ui, &self.fonts.title, &self.fonts.mono] {
            if !font.is_empty() {
                let path = theme_dir.join(font);
                if !path.is_file() {
                    panic!(
                        "BattleTris theme manifest {} requires missing font {}",
                        manifest_path.display(),
                        path.display()
                    );
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct ThemeProvenance {
    notes: String,
    sources: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ThemeSprites {
    atlas: String,
    startup: String,
    bazaar: String,
    biff: String,
    gimp: String,
    crest: String,
    rated: Option<ThemeRatedSprites>,
    cell_atlas: ThemeCellAtlas,
}

impl ThemeSprites {
    fn loaded(&self, choice: ThemeChoice) -> LoadedThemeSprites {
        let prefix = format!("themes/{}/", choice.directory());
        LoadedThemeSprites {
            atlas: format!("{prefix}{}", self.atlas),
            startup: format!("{prefix}{}", self.startup),
            bazaar: format!("{prefix}{}", self.bazaar),
            biff: format!("{prefix}{}", self.biff),
            gimp: format!("{prefix}{}", self.gimp),
            crest: format!("{prefix}{}", self.crest),
            rated: self.rated.as_ref().map(|rated| LoadedThemeRatedSprites {
                atlas: format!("{prefix}{}", rated.atlas),
                gimp: format!("{prefix}{}", rated.gimp),
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ThemeRatedSprites {
    atlas: String,
    gimp: String,
}

#[derive(Debug, Clone)]
struct LoadedThemeSprites {
    atlas: String,
    startup: String,
    bazaar: String,
    biff: String,
    gimp: String,
    crest: String,
    rated: Option<LoadedThemeRatedSprites>,
}

impl LoadedThemeSprites {
    fn atlas_for(&self, content_mode: ContentMode) -> &str {
        match (content_mode, &self.rated) {
            (ContentMode::Rated, Some(rated)) => &rated.atlas,
            _ => &self.atlas,
        }
    }

    fn gimp_for(&self, content_mode: ContentMode) -> &str {
        match (content_mode, &self.rated) {
            (ContentMode::Rated, Some(rated)) => &rated.gimp,
            _ => &self.gimp,
        }
    }

    const fn supports_rated(&self) -> bool {
        self.rated.is_some()
    }
}

#[derive(Debug, Clone)]
struct LoadedThemeRatedSprites {
    atlas: String,
    gimp: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ThemeFonts {
    ui: String,
    title: String,
    mono: String,
    line_height: f32,
    tracking: f32,
}

impl ThemeFonts {
    fn loaded(&self, choice: ThemeChoice) -> LoadedThemeFonts {
        LoadedThemeFonts {
            ui: theme_asset_path(choice, &self.ui),
            title: theme_asset_path(choice, &self.title),
            mono: theme_asset_path(choice, &self.mono),
            line_height: self.line_height,
            tracking: self.tracking,
        }
    }
}

#[derive(Debug, Clone)]
struct LoadedThemeFonts {
    ui: Option<String>,
    title: Option<String>,
    mono: Option<String>,
    line_height: f32,
    tracking: f32,
}

impl LoadedThemeFonts {
    fn path_for(&self, role: ThemedTextFontRole) -> Option<&str> {
        match role {
            ThemedTextFontRole::Title => self.title.as_deref().or(self.ui.as_deref()),
            ThemedTextFontRole::Body | ThemedTextFontRole::Button => {
                self.ui.as_deref().or(self.mono.as_deref())
            }
            ThemedTextFontRole::Mono => self.mono.as_deref().or(self.ui.as_deref()),
        }
    }
}

fn theme_asset_path(choice: ThemeChoice, relative: &str) -> Option<String> {
    if relative.is_empty() {
        None
    } else {
        Some(format!("themes/{}/{}", choice.directory(), relative))
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ThemeCell {
    size: f32,
    gap: f32,
    shadow: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ThemeCellAtlas {
    tile_width: u32,
    tile_height: u32,
    columns: u32,
    rows: u32,
    padding_x: u32,
    padding_y: u32,
    offset_x: u32,
    offset_y: u32,
    cells: ThemeCellAtlasCells,
}

impl ThemeCellAtlas {
    fn texture_count(self) -> usize {
        self.columns as usize * self.rows as usize
    }

    fn tile_size(self) -> UVec2 {
        UVec2::new(self.tile_width, self.tile_height)
    }

    fn padding(self) -> Option<UVec2> {
        Some(UVec2::new(self.padding_x, self.padding_y))
    }

    fn offset(self) -> Option<UVec2> {
        Some(UVec2::new(self.offset_x, self.offset_y))
    }

    fn validate(self, manifest_path: &std::path::Path) {
        if self.tile_width == 0 || self.tile_height == 0 || self.columns == 0 || self.rows == 0 {
            panic!(
                "BattleTris theme manifest {} has invalid cell atlas dimensions",
                manifest_path.display()
            );
        }
        if self.cells.visible_colors.len() != 19 || self.cells.die.len() != 6 {
            panic!(
                "BattleTris theme manifest {} must map 19 visible colors and 6 die faces",
                manifest_path.display()
            );
        }
        let texture_count = self.texture_count();
        let mut indices = Vec::new();
        indices.push(self.cells.empty);
        indices.extend(self.cells.visible_colors);
        indices.push(self.cells.structure);
        indices.push(self.cells.happy);
        indices.push(self.cells.frown);
        indices.push(self.cells.gimp);
        indices.extend(self.cells.die);
        indices.push(self.cells.invisible);
        indices.push(self.cells.hidden);
        let unique = indices.iter().copied().collect::<HashSet<_>>();
        if unique.len() != indices.len() || indices.iter().any(|index| *index >= texture_count) {
            panic!(
                "BattleTris theme manifest {} has duplicate or out-of-range cell atlas indices",
                manifest_path.display()
            );
        }
    }

    fn validate_image(
        self,
        theme_dir: &std::path::Path,
        atlas: &str,
        manifest_path: &std::path::Path,
    ) {
        let path = theme_dir.join(atlas);
        let (width, height) = image::image_dimensions(&path).unwrap_or_else(|error| {
            panic!(
                "BattleTris theme manifest {} requires decodable atlas {}: {error}",
                manifest_path.display(),
                path.display()
            )
        });
        let expected_width = self.offset_x
            + self.columns * self.tile_width
            + self.columns.saturating_sub(1) * self.padding_x;
        let expected_height = self.offset_y
            + self.rows * self.tile_height
            + self.rows.saturating_sub(1) * self.padding_y;
        if width < expected_width || height < expected_height {
            panic!(
                "BattleTris theme manifest {} atlas {} is {}x{}, expected at least {}x{}",
                manifest_path.display(),
                path.display(),
                width,
                height,
                expected_width,
                expected_height
            );
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ThemeCellAtlasCells {
    empty: usize,
    visible_colors: [usize; 19],
    structure: usize,
    happy: usize,
    frown: usize,
    gimp: usize,
    die: [usize; 6],
    invisible: usize,
    hidden: usize,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ThemeLayout {
    board: ThemeBoardLayout,
    screens: ThemeScreenLayouts,
    rects: ThemeLayoutRects,
}

impl ThemeLayout {
    const fn screen(&self, screen: ClientScreen) -> ThemeWindowLayout {
        match screen {
            ClientScreen::Startup => self.screens.startup,
            ClientScreen::Game => self.screens.game,
            ClientScreen::Challenge => self.screens.challenge,
            ClientScreen::Sleep => self.screens.sleep,
            ClientScreen::About => self.screens.about,
            ClientScreen::Roster => self.screens.roster,
            ClientScreen::Settings => self.screens.settings,
        }
    }

    const fn fixture(&self, fixture: VisualFixture) -> ThemeWindowLayout {
        match fixture {
            VisualFixture::Startup => self.screens.startup,
            VisualFixture::Challenge => self.screens.challenge,
            VisualFixture::Sleep => self.screens.sleep,
            VisualFixture::About => self.screens.about,
            VisualFixture::Roster => self.screens.roster,
            VisualFixture::Settings => self.screens.settings,
            VisualFixture::GamePlaying | VisualFixture::GameOver | VisualFixture::BoardCells => {
                self.screens.game
            }
            VisualFixture::GameBazaar => self.screens.bazaar,
            VisualFixture::GameRecon => self.screens.game_recon,
        }
    }

    fn validate(&self, manifest_path: &std::path::Path) {
        for (name, window) in self.screens.named() {
            if window.width <= 0.0 || window.height <= 0.0 {
                panic!(
                    "BattleTris theme manifest {} has invalid {name} screen size",
                    manifest_path.display()
                );
            }
        }
        self.rects.validate(manifest_path);
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ThemeScreenLayouts {
    startup: ThemeWindowLayout,
    challenge: ThemeWindowLayout,
    sleep: ThemeWindowLayout,
    about: ThemeWindowLayout,
    roster: ThemeWindowLayout,
    settings: ThemeWindowLayout,
    game: ThemeWindowLayout,
    game_recon: ThemeWindowLayout,
    bazaar: ThemeWindowLayout,
}

impl ThemeScreenLayouts {
    const fn named(self) -> [(&'static str, ThemeWindowLayout); 9] {
        [
            ("startup", self.startup),
            ("challenge", self.challenge),
            ("sleep", self.sleep),
            ("about", self.about),
            ("roster", self.roster),
            ("settings", self.settings),
            ("game", self.game),
            ("game_recon", self.game_recon),
            ("bazaar", self.bazaar),
        ]
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ThemeWindowLayout {
    width: f32,
    height: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ThemeBoardLayout {
    top: f32,
    player_one_left: f32,
    player_two_left: f32,
    spacing: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ThemeLayoutRects {
    startup_challenge: ThemeRect,
    startup_sleep: ThemeRect,
    startup_about: ThemeRect,
    startup_roster: ThemeRect,
    startup_quit: ThemeRect,
    startup_local_game: ThemeRect,
    startup_play_ernie: ThemeRect,
    startup_theme: ThemeRect,
    challenge_level_down: ThemeRect,
    challenge_level_up: ThemeRect,
    challenge_play_ernie: ThemeRect,
    challenge_back: ThemeRect,
    sleep_wake: ThemeRect,
    about_ok: ThemeRect,
    roster_back: ThemeRect,
    settings_back: ThemeRect,
    bazaar_catalog: ThemeRect,
    bazaar_arsenal: ThemeRect,
    bazaar_add: ThemeRect,
    bazaar_remove: ThemeRect,
    bazaar_done: ThemeRect,
}

impl ThemeLayoutRects {
    fn validate(self, manifest_path: &std::path::Path) {
        for (name, rect) in self.named() {
            if rect.width <= 0.0 || rect.height <= 0.0 {
                panic!(
                    "BattleTris theme manifest {} has invalid rect {name}",
                    manifest_path.display()
                );
            }
        }
    }

    const fn named(self) -> [(&'static str, ThemeRect); 21] {
        [
            ("startup_challenge", self.startup_challenge),
            ("startup_sleep", self.startup_sleep),
            ("startup_about", self.startup_about),
            ("startup_roster", self.startup_roster),
            ("startup_quit", self.startup_quit),
            ("startup_local_game", self.startup_local_game),
            ("startup_play_ernie", self.startup_play_ernie),
            ("startup_theme", self.startup_theme),
            ("challenge_level_down", self.challenge_level_down),
            ("challenge_level_up", self.challenge_level_up),
            ("challenge_play_ernie", self.challenge_play_ernie),
            ("challenge_back", self.challenge_back),
            ("sleep_wake", self.sleep_wake),
            ("about_ok", self.about_ok),
            ("roster_back", self.roster_back),
            ("settings_back", self.settings_back),
            ("bazaar_catalog", self.bazaar_catalog),
            ("bazaar_arsenal", self.bazaar_arsenal),
            ("bazaar_add", self.bazaar_add),
            ("bazaar_remove", self.bazaar_remove),
            ("bazaar_done", self.bazaar_done),
        ]
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ThemeRect {
    center_x: f32,
    center_y: f32,
    width: f32,
    height: f32,
}

impl ThemeRect {
    fn center(self) -> Vec2 {
        Vec2::new(self.center_x, self.center_y)
    }

    fn size(self) -> Vec2 {
        Vec2::new(self.width, self.height)
    }

    fn rect(self) -> Rect {
        Rect::from_center_size(self.center(), self.size())
    }
}

#[derive(Debug, Clone)]
struct ThemePalette {
    board_background: Color,
    empty: Color,
    structure: Color,
    happy: Color,
    frown: Color,
    gimp: Color,
    die: Color,
    invisible: Color,
    hidden: Color,
    text_secondary: Color,
    text_accent: Color,
    visible_colors: Vec<Color>,
}

#[derive(Debug, Clone)]
struct ThemeButtonStyle {
    normal: Color,
    hover: Color,
    pressed: Color,
    text: Color,
}

#[derive(Debug, Clone)]
struct ThemeScreenStyle {
    background: Color,
    title_text: Color,
    body_text: Color,
    title_font_size: f32,
    body_font_size: f32,
    button_font_size: f32,
}

#[derive(Debug, Clone)]
struct ThemeAboutStyle {
    background: Color,
    title_text: Color,
    name_text: Color,
    credit_text: Color,
    button_face: Color,
    button_highlight: Color,
    button_shadow: Color,
    button_text: Color,
}

#[derive(Debug, Deserialize)]
struct RawThemeScreenStyle {
    background: String,
    title_text: String,
    body_text: String,
    title_font_size: f32,
    body_font_size: f32,
    button_font_size: f32,
}

#[derive(Debug, Deserialize)]
struct RawThemeAboutStyle {
    background: String,
    title_text: String,
    name_text: String,
    credit_text: String,
    button_face: String,
    button_highlight: String,
    button_shadow: String,
    button_text: String,
}

#[derive(Debug, Deserialize)]
struct RawThemeSemantic {
    text: RawThemeSemanticText,
    board: RawThemeSemanticBoard,
    button: RawThemeSemanticButton,
    bazaar: RawThemeSemanticBazaar,
    weapon: RawThemeSemanticWeapon,
}

impl RawThemeSemantic {
    fn validate(&self, manifest_path: &std::path::Path) {
        for color in [
            &self.text.primary,
            &self.text.secondary,
            &self.text.accent,
            &self.text.warning,
            &self.board.background,
            &self.board.empty,
            &self.board.structure,
            &self.board.happy,
            &self.board.frown,
            &self.board.gimp,
            &self.board.die,
            &self.board.invisible,
            &self.board.hidden,
            &self.button.normal,
            &self.button.hover,
            &self.button.pressed,
            &self.button.text,
            &self.bazaar.affordable,
            &self.bazaar.unaffordable,
            &self.bazaar.selected,
            &self.weapon.active,
            &self.weapon.expired,
        ] {
            let _ = parse_hex_color(color, manifest_path);
        }
        if self.board.visible_colors.len() != 19 {
            panic!(
                "BattleTris theme manifest {} must define 19 semantic visible cell colors",
                manifest_path.display()
            );
        }
        for color in &self.board.visible_colors {
            let _ = parse_hex_color(color, manifest_path);
        }
    }

    fn palette(&self, manifest_path: &std::path::Path) -> ThemePalette {
        ThemePalette {
            board_background: parse_hex_color(&self.board.background, manifest_path),
            empty: parse_hex_color(&self.board.empty, manifest_path),
            structure: parse_hex_color(&self.board.structure, manifest_path),
            happy: parse_hex_color(&self.board.happy, manifest_path),
            frown: parse_hex_color(&self.board.frown, manifest_path),
            gimp: parse_hex_color(&self.board.gimp, manifest_path),
            die: parse_hex_color(&self.board.die, manifest_path),
            invisible: parse_hex_color(&self.board.invisible, manifest_path),
            hidden: parse_hex_color(&self.board.hidden, manifest_path),
            text_secondary: parse_hex_color(&self.text.secondary, manifest_path),
            text_accent: parse_hex_color(&self.text.accent, manifest_path),
            visible_colors: self
                .board
                .visible_colors
                .iter()
                .map(|color| parse_hex_color(color, manifest_path))
                .collect(),
        }
    }

    fn button(&self, manifest_path: &std::path::Path) -> ThemeButtonStyle {
        ThemeButtonStyle {
            normal: parse_hex_color(&self.button.normal, manifest_path),
            hover: parse_hex_color(&self.button.hover, manifest_path),
            pressed: parse_hex_color(&self.button.pressed, manifest_path),
            text: parse_hex_color(&self.button.text, manifest_path),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawThemeSemanticText {
    primary: String,
    secondary: String,
    accent: String,
    warning: String,
}

#[derive(Debug, Deserialize)]
struct RawThemeSemanticBoard {
    background: String,
    empty: String,
    structure: String,
    happy: String,
    frown: String,
    gimp: String,
    die: String,
    invisible: String,
    hidden: String,
    visible_colors: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawThemeSemanticButton {
    normal: String,
    hover: String,
    pressed: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct RawThemeSemanticBazaar {
    affordable: String,
    unaffordable: String,
    selected: String,
}

#[derive(Debug, Deserialize)]
struct RawThemeSemanticWeapon {
    active: String,
    expired: String,
}

impl RawThemeScreenStyle {
    fn into_style(self, manifest_path: &std::path::Path) -> ThemeScreenStyle {
        ThemeScreenStyle {
            background: parse_hex_color(&self.background, manifest_path),
            title_text: parse_hex_color(&self.title_text, manifest_path),
            body_text: parse_hex_color(&self.body_text, manifest_path),
            title_font_size: self.title_font_size,
            body_font_size: self.body_font_size,
            button_font_size: self.button_font_size,
        }
    }
}

impl RawThemeAboutStyle {
    fn into_style(self, manifest_path: &std::path::Path) -> ThemeAboutStyle {
        ThemeAboutStyle {
            background: parse_hex_color(&self.background, manifest_path),
            title_text: parse_hex_color(&self.title_text, manifest_path),
            name_text: parse_hex_color(&self.name_text, manifest_path),
            credit_text: parse_hex_color(&self.credit_text, manifest_path),
            button_face: parse_hex_color(&self.button_face, manifest_path),
            button_highlight: parse_hex_color(&self.button_highlight, manifest_path),
            button_shadow: parse_hex_color(&self.button_shadow, manifest_path),
            button_text: parse_hex_color(&self.button_text, manifest_path),
        }
    }
}

fn parse_hex_color(value: &str, manifest_path: &std::path::Path) -> Color {
    let Some(hex) = value.strip_prefix('#') else {
        panic!(
            "BattleTris theme manifest {} has non-hex color {value}",
            manifest_path.display()
        );
    };
    let (rgb, alpha) = match hex.len() {
        6 => (hex, "ff"),
        8 => hex.split_at(6),
        _ => panic!(
            "BattleTris theme manifest {} has invalid color {value}",
            manifest_path.display()
        ),
    };
    let red = u8::from_str_radix(&rgb[0..2], 16).expect("validated hex red");
    let green = u8::from_str_radix(&rgb[2..4], 16).expect("validated hex green");
    let blue = u8::from_str_radix(&rgb[4..6], 16).expect("validated hex blue");
    let alpha = u8::from_str_radix(alpha, 16).expect("validated hex alpha");
    Color::srgba_u8(red, green, blue, alpha)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ControlScheme {
    ModernSplit,
    LegacyInspired,
}

#[derive(Resource, Debug, Clone)]
struct ClientSettings {
    screen: ClientScreen,
    content_mode: ContentMode,
    theme: ThemeChoice,
    sound_pack: SoundPackChoice,
    controls: ControlScheme,
    pixel_scale: f32,
    ernie_level: usize,
    challenge_mode: ChallengeMode,
    display_name: String,
    community_label: String,
    direct_listen_addr: String,
    direct_share_addr: String,
    direct_join_addr: String,
    lobby_addr: String,
    hosted_ranked: bool,
    settings_path: Option<PathBuf>,
    assets_dir: PathBuf,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            screen: ClientScreen::Startup,
            content_mode: ContentMode::Normal,
            theme: ThemeChoice::Original,
            sound_pack: SoundPackChoice::GeneratedDefault,
            controls: ControlScheme::ModernSplit,
            pixel_scale: 1.0,
            ernie_level: DEFAULT_ERNIE_LEVEL,
            challenge_mode: ChallengeMode::ComputerOpponent,
            display_name: default_display_name(),
            community_label: CommunityLabel::local().as_str().to_string(),
            direct_listen_addr: "0.0.0.0:4405".to_string(),
            direct_share_addr: suggested_share_addr_for("0.0.0.0:4405"),
            direct_join_addr: "127.0.0.1:4405".to_string(),
            lobby_addr: "127.0.0.1:4404".to_string(),
            hosted_ranked: true,
            settings_path: settings_path(),
            assets_dir: assets_dir(),
        }
    }
}

impl ClientSettings {
    fn load_or_default() -> Self {
        let mut settings = Self::default();
        let Some(path) = &settings.settings_path else {
            return settings;
        };

        let Ok(contents) = fs::read_to_string(path) else {
            return settings;
        };

        match toml::from_str::<PersistedClientSettings>(&contents) {
            Ok(persisted) => settings.apply_persisted(persisted),
            Err(error) => warn!(
                "BattleTris settings file {} could not be parsed: {error}",
                path.display()
            ),
        }
        settings
    }

    fn save(&self) {
        let Some(path) = &self.settings_path else {
            return;
        };

        if let Some(parent) = path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                warn!(
                    "BattleTris settings directory {} could not be created: {error}",
                    parent.display()
                );
                return;
            }
        }

        match toml::to_string_pretty(&self.persisted()) {
            Ok(contents) => {
                if let Err(error) = fs::write(path, contents) {
                    warn!(
                        "BattleTris settings file {} could not be written: {error}",
                        path.display()
                    );
                }
            }
            Err(error) => warn!("BattleTris settings could not be serialized: {error}"),
        }
    }

    fn persisted(&self) -> PersistedClientSettings {
        PersistedClientSettings {
            theme: self.theme,
            sound_pack: self.sound_pack,
            controls: self.controls,
            pixel_scale: self.pixel_scale,
            ernie_level: self.ernie_level,
            display_name: self.display_name.clone(),
            community_label: self.community_label.clone(),
            direct_listen_addr: self.direct_listen_addr.clone(),
            direct_share_addr: self.direct_share_addr.clone(),
            direct_join_addr: self.direct_join_addr.clone(),
            lobby_addr: self.lobby_addr.clone(),
            hosted_ranked: self.hosted_ranked,
        }
    }

    fn apply_persisted(&mut self, persisted: PersistedClientSettings) {
        self.theme = persisted.theme;
        self.sound_pack = persisted.sound_pack;
        self.controls = persisted.controls;
        self.pixel_scale = sanitize_pixel_scale(persisted.pixel_scale);
        self.ernie_level = sanitize_ernie_level(persisted.ernie_level);
        self.display_name =
            sanitize_nonempty_setting(persisted.display_name, default_display_name());
        self.community_label =
            sanitize_nonempty_setting(persisted.community_label, "local".to_string());
        self.direct_listen_addr =
            sanitize_socket_setting(persisted.direct_listen_addr, "0.0.0.0:4405");
        self.direct_share_addr =
            sanitize_share_addr_setting(persisted.direct_share_addr, &self.direct_listen_addr);
        self.direct_join_addr =
            sanitize_socket_setting(persisted.direct_join_addr, "127.0.0.1:4405");
        self.lobby_addr = sanitize_socket_setting(persisted.lobby_addr, "127.0.0.1:4404");
        self.hosted_ranked = persisted.hosted_ranked;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct PersistedClientSettings {
    theme: ThemeChoice,
    sound_pack: SoundPackChoice,
    controls: ControlScheme,
    pixel_scale: f32,
    ernie_level: usize,
    display_name: String,
    community_label: String,
    direct_listen_addr: String,
    direct_share_addr: String,
    direct_join_addr: String,
    lobby_addr: String,
    hosted_ranked: bool,
}

impl Default for PersistedClientSettings {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::Original,
            sound_pack: SoundPackChoice::GeneratedDefault,
            controls: ControlScheme::ModernSplit,
            pixel_scale: 1.0,
            ernie_level: DEFAULT_ERNIE_LEVEL,
            display_name: default_display_name(),
            community_label: "local".to_string(),
            direct_listen_addr: "0.0.0.0:4405".to_string(),
            direct_share_addr: suggested_share_addr_for("0.0.0.0:4405"),
            direct_join_addr: "127.0.0.1:4405".to_string(),
            lobby_addr: "127.0.0.1:4404".to_string(),
            hosted_ranked: true,
        }
    }
}

fn log_content_mode(settings: Res<ClientSettings>, themes: Res<ThemePacks>) {
    let theme = themes.get(settings.theme);
    info!(
        "BattleTris content mode: {}; Gimp sprite: {}",
        settings.content_mode.id(),
        theme.sprites.gimp_for(settings.content_mode)
    );
}

#[derive(Resource)]
struct LocalGame {
    game: TwoPlayerGame,
    computer: Option<ComputerController>,
    local_player: PlayerId,
    status_message: Option<String>,
}

#[derive(Resource, Debug, Clone)]
struct RosterRecords {
    rows: Vec<RosterRow>,
    error: Option<String>,
}

#[derive(Debug, Clone)]
struct RosterRow {
    player_key: String,
    rank: u64,
    display_name: String,
    wins: u64,
    losses: u64,
    high_score: u64,
    high_lines: u64,
    high_funds: u64,
    streak: String,
    fastest_kill_secs: Option<u64>,
    quickest_death_secs: Option<u64>,
    longest_game_secs: Option<u64>,
}

impl RosterRecords {
    fn load() -> Self {
        let paths = match PersistencePaths::new() {
            Ok(paths) => paths,
            Err(error) => {
                return Self {
                    rows: Vec::new(),
                    error: Some(error.to_string()),
                };
            }
        };
        if let Some(parent) = paths.player_db_file.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                return Self {
                    rows: Vec::new(),
                    error: Some(format!(
                        "record directory {} could not be created: {error}",
                        parent.display()
                    )),
                };
            }
        }
        match PlayerStore::open(&paths.player_db_file)
            .and_then(|store| store.roster_by_rank(&CommunityLabel::local()))
        {
            Ok(rows) => Self {
                rows: rows
                    .into_iter()
                    .map(|profile| RosterRow {
                        player_key: profile.player_id.as_str().to_string(),
                        rank: profile.rank,
                        display_name: profile.display_name,
                        wins: profile.wins,
                        losses: profile.losses,
                        high_score: profile.high_score,
                        high_lines: profile.high_lines,
                        high_funds: profile.high_funds,
                        streak: streak_label(profile.streak_kind, profile.streak_count),
                        fastest_kill_secs: profile.fastest_kill_secs,
                        quickest_death_secs: profile.quickest_death_secs,
                        longest_game_secs: profile.longest_game_secs,
                    })
                    .collect(),
                error: None,
            },
            Err(error) => Self {
                rows: Vec::new(),
                error: Some(format!(
                    "record store {} could not be read: {error}",
                    paths.player_db_file.display()
                )),
            },
        }
    }
}

fn apply_visual_fixture_state(
    fixture: VisualFixture,
    settings: &mut ClientSettings,
    local: &mut LocalGame,
    recon: &mut ReconPanel,
    bazaar_ui: &mut BazaarUiState,
    roster: &mut RosterRecords,
) {
    settings.screen = fixture.screen();
    settings.pixel_scale = 1.0;
    settings.display_name = "Visual Fixture".to_string();
    settings.community_label = "visual".to_string();
    settings.direct_listen_addr = "127.0.0.1:4405".to_string();
    settings.lobby_addr = "127.0.0.1:4404".to_string();

    if fixture == VisualFixture::Settings {
        settings.controls = ControlScheme::LegacyInspired;
    }
    if fixture == VisualFixture::Challenge {
        settings.ernie_level = 0;
    }

    *local = visual_local_game(fixture, settings.ernie_level);
    *recon = visual_recon_panel(fixture, local);
    *bazaar_ui = visual_bazaar_ui(fixture);
    *roster = visual_roster_records();
}

fn visual_local_game(fixture: VisualFixture, ernie_level: usize) -> LocalGame {
    match fixture {
        VisualFixture::GamePlaying => visual_computer_game(
            ernie_level,
            visual_playing_board(),
            visual_opponent_board(),
            "Visual fixture: playing",
        ),
        VisualFixture::GameRecon => visual_computer_game(
            ernie_level,
            visual_playing_board(),
            visual_opponent_board(),
            "Visual fixture: Condor recon snapshot",
        ),
        VisualFixture::BoardCells => visual_computer_game(
            ernie_level,
            visual_board_cells_board(),
            Board::empty(),
            "Visual fixture: board cell catalog",
        ),
        VisualFixture::GameBazaar => visual_bazaar_game(),
        VisualFixture::GameOver => visual_game_over_game(),
        _ => LocalGame::new_human_vs_human(),
    }
}

fn visual_computer_game(
    ernie_level: usize,
    player_board: Board,
    computer_board: Board,
    status_message: &str,
) -> LocalGame {
    let difficulty = computer_difficulty(sanitize_ernie_level(ernie_level))
        .expect("legacy AI difficulty exists");
    LocalGame {
        game: TwoPlayerGame::human_vs_computer(
            GameSeed::from_u64(101),
            player_board,
            GameSeed::from_u64(202),
            computer_board,
            PlayerId::Two,
            difficulty,
        ),
        computer: Some(ComputerController::new(
            PlayerId::Two,
            GameSeed::from_u64(303),
            difficulty.level,
        )),
        local_player: PlayerId::One,
        status_message: Some(status_message.to_string()),
    }
}

fn visual_bazaar_game() -> LocalGame {
    let game = TwoPlayerGame::bazaar_fixture(
        GameSeed::from_u64(111),
        visual_playing_board(),
        650,
        GameSeed::from_u64(222),
        visual_opponent_board(),
        425,
    );
    LocalGame {
        game,
        computer: None,
        local_player: PlayerId::One,
        status_message: Some("Visual fixture: bazaar shopping".to_string()),
    }
}

fn visual_game_over_game() -> LocalGame {
    let mut local_board = Board::empty();
    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            local_board.set(Coord { x, y }, Some(Cell::visible()));
        }
    }
    LocalGame {
        game: TwoPlayerGame::with_boards(
            GameSeed::from_u64(121),
            local_board,
            GameSeed::from_u64(222),
            visual_opponent_board(),
        ),
        computer: None,
        local_player: PlayerId::One,
        status_message: None,
    }
}

fn visual_recon_panel(fixture: VisualFixture, local: &LocalGame) -> ReconPanel {
    let mut recon = ReconPanel::default();
    if fixture == VisualFixture::GameRecon {
        let target = opponent_player(local.local_player);
        recon.snapshot = Some(ReconSnapshot {
            level: ReconLevel::Condor,
            board: local.game.player(target).board().snapshot(),
            funds: 375,
        });
    }
    recon
}

fn visual_bazaar_ui(fixture: VisualFixture) -> BazaarUiState {
    if fixture == VisualFixture::GameBazaar {
        BazaarUiState {
            selected: WeaponToken::FlipOut,
            last_message: "Legacy visual fixture: bazaar shopping.".to_string(),
            visual_arsenal: Some([
                Some(WeaponToken::Gimp),
                Some(WeaponToken::FlipOut),
                Some(WeaponToken::RiseUp),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ]),
        }
    } else {
        BazaarUiState::default()
    }
}

fn visual_roster_records() -> RosterRecords {
    RosterRecords {
        rows: vec![
            RosterRow {
                player_key: "ada".to_string(),
                rank: 1,
                display_name: "Ada".to_string(),
                wins: 12,
                losses: 3,
                high_score: 48_250,
                high_lines: 82,
                high_funds: 1_450,
                streak: "5 wins".to_string(),
                fastest_kill_secs: None,
                quickest_death_secs: None,
                longest_game_secs: None,
            },
            RosterRow {
                player_key: "grace".to_string(),
                rank: 2,
                display_name: "Grace".to_string(),
                wins: 9,
                losses: 4,
                high_score: 37_600,
                high_lines: 69,
                high_funds: 1_100,
                streak: "2 wins".to_string(),
                fastest_kill_secs: None,
                quickest_death_secs: None,
                longest_game_secs: None,
            },
            RosterRow {
                player_key: "katherine".to_string(),
                rank: 3,
                display_name: "Katherine".to_string(),
                wins: 7,
                losses: 5,
                high_score: 31_900,
                high_lines: 58,
                high_funds: 980,
                streak: "1 loss".to_string(),
                fastest_kill_secs: None,
                quickest_death_secs: None,
                longest_game_secs: None,
            },
            RosterRow {
                player_key: "margaret".to_string(),
                rank: 4,
                display_name: "Margaret".to_string(),
                wins: 6,
                losses: 6,
                high_score: 28_400,
                high_lines: 51,
                high_funds: 820,
                streak: "1 win".to_string(),
                fastest_kill_secs: None,
                quickest_death_secs: None,
                longest_game_secs: None,
            },
            RosterRow {
                player_key: "radia".to_string(),
                rank: 5,
                display_name: "Radia".to_string(),
                wins: 5,
                losses: 7,
                high_score: 22_750,
                high_lines: 44,
                high_funds: 700,
                streak: "2 losses".to_string(),
                fastest_kill_secs: None,
                quickest_death_secs: None,
                longest_game_secs: None,
            },
            RosterRow {
                player_key: "evelyn".to_string(),
                rank: 6,
                display_name: "Evelyn".to_string(),
                wins: 4,
                losses: 8,
                high_score: 19_600,
                high_lines: 39,
                high_funds: 640,
                streak: "1 win".to_string(),
                fastest_kill_secs: None,
                quickest_death_secs: None,
                longest_game_secs: None,
            },
            RosterRow {
                player_key: "hedy".to_string(),
                rank: 7,
                display_name: "Hedy".to_string(),
                wins: 3,
                losses: 9,
                high_score: 16_300,
                high_lines: 33,
                high_funds: 500,
                streak: "3 losses".to_string(),
                fastest_kill_secs: None,
                quickest_death_secs: None,
                longest_game_secs: None,
            },
            RosterRow {
                player_key: "joan".to_string(),
                rank: 8,
                display_name: "Joan".to_string(),
                wins: 2,
                losses: 10,
                high_score: 11_950,
                high_lines: 26,
                high_funds: 410,
                streak: "1 loss".to_string(),
                fastest_kill_secs: None,
                quickest_death_secs: None,
                longest_game_secs: None,
            },
        ],
        error: None,
    }
}

fn visual_playing_board() -> Board {
    let mut board = Board::empty();
    for x in 0..BOARD_WIDTH {
        if x != 4 {
            board.set(
                Coord::new(x, BOARD_HEIGHT - 1).expect("fixture coordinate in bounds"),
                Some(visible_cell((x % 7 + 1) as u8)),
            );
        }
    }
    for x in 2..BOARD_WIDTH {
        board.set(
            Coord::new(x, BOARD_HEIGHT - 2).expect("fixture coordinate in bounds"),
            Some(visible_cell(((x + 2) % 7 + 1) as u8)),
        );
    }
    for (x, cell) in [
        (0, Cell::Happy),
        (1, Cell::die(Pip::new(4).expect("valid pip"))),
        (2, Cell::Gimp { value: 25 }),
        (7, Cell::Structure),
        (
            8,
            Cell::Hidden {
                value: 5,
                removable: true,
            },
        ),
        (9, Cell::Invisible),
    ] {
        board.set(
            Coord::new(x, BOARD_HEIGHT - 4).expect("fixture coordinate in bounds"),
            Some(cell),
        );
    }
    board
}

fn visual_opponent_board() -> Board {
    let mut board = Board::empty();
    for y in BOARD_HEIGHT - 6..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            if (x + y).is_multiple_of(3) {
                board.set(
                    Coord::new(x, y).expect("fixture coordinate in bounds"),
                    Some(visible_cell(((x + y) % 7 + 1) as u8)),
                );
            }
        }
    }
    board.set(
        Coord::new(5, BOARD_HEIGHT - 7).expect("fixture coordinate in bounds"),
        Some(Cell::die(Pip::new(6).expect("valid pip"))),
    );
    board.set(
        Coord::new(6, BOARD_HEIGHT - 7).expect("fixture coordinate in bounds"),
        Some(Cell::Frown),
    );
    board
}

fn visual_board_cells_board() -> Board {
    let mut board = Board::empty();
    const CATALOG_START_Y: usize = 5;

    for y in CATALOG_START_Y..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            board.set(
                Coord::new(x, y).expect("fixture coordinate in bounds"),
                Some(visible_cell(((x + y) % 19 + 1) as u8)),
            );
        }
    }

    let samples = [
        visible_cell(1),
        visible_cell(2),
        visible_cell(3),
        visible_cell(4),
        visible_cell(5),
        visible_cell(6),
        visible_cell(7),
        Cell::Structure,
        Cell::Happy,
        Cell::Frown,
        Cell::Gimp { value: 150 },
        Cell::Invisible,
        Cell::Hidden {
            value: 50,
            removable: true,
        },
        Cell::die(Pip::new(1).expect("valid pip")),
        Cell::die(Pip::new(2).expect("valid pip")),
        Cell::die(Pip::new(3).expect("valid pip")),
        Cell::die(Pip::new(4).expect("valid pip")),
        Cell::die(Pip::new(5).expect("valid pip")),
        Cell::die(Pip::new(6).expect("valid pip")),
        visible_cell(8),
    ];
    for (index, cell) in samples.into_iter().enumerate() {
        board.set(
            Coord::new(index % BOARD_WIDTH, CATALOG_START_Y + index / BOARD_WIDTH)
                .expect("fixture coordinate in bounds"),
            Some(cell),
        );
    }
    board
}

fn visible_cell(color: u8) -> Cell {
    Cell::visible_with_color(VisibleColor::new(color).expect("fixture color in legacy range"))
}

impl LocalGame {
    fn new_human_vs_human() -> Self {
        Self {
            game: TwoPlayerGame::new(GameSeed::from_u64(1), GameSeed::from_u64(2)),
            computer: None,
            local_player: PlayerId::One,
            status_message: None,
        }
    }

    fn new_human_vs_computer(level: usize) -> Self {
        let difficulty =
            computer_difficulty(sanitize_ernie_level(level)).expect("legacy AI difficulty exists");
        Self {
            game: TwoPlayerGame::human_vs_computer(
                GameSeed::from_u64(1),
                Board::empty(),
                GameSeed::from_u64(2),
                Board::empty(),
                PlayerId::Two,
                difficulty,
            ),
            computer: Some(ComputerController::new(
                PlayerId::Two,
                GameSeed::from_u64(42),
                difficulty.level,
            )),
            local_player: PlayerId::One,
            status_message: Some(format!("Playing {} Ernie", difficulty.name)),
        }
    }

    fn restart(&mut self) {
        *self = match self.game.mode() {
            GameMode::HumanVsHuman => Self::new_human_vs_human(),
            GameMode::HumanVsComputer { difficulty, .. } => {
                Self::new_human_vs_computer(difficulty.level)
            }
        };
    }
}

#[derive(Resource, Debug, Default)]
struct ClientTickClock {
    gameplay_elapsed_ms: u64,
    computer_elapsed_ms: u64,
}

#[derive(Resource, Debug, Default)]
struct InputRepeatState {
    left: [HeldKeyRepeat; 2],
    right: [HeldKeyRepeat; 2],
}

#[derive(Debug, Default, Clone, Copy)]
struct HeldKeyRepeat {
    held_ms: u64,
    next_repeat_ms: u64,
}

impl HeldKeyRepeat {
    fn observe(self, pressed: bool, just_pressed: bool, elapsed_ms: u64) -> (Self, bool) {
        if !pressed {
            return (Self::default(), false);
        }
        if just_pressed {
            return (
                Self {
                    held_ms: 0,
                    next_repeat_ms: INPUT_REPEAT_INITIAL_MS,
                },
                true,
            );
        }

        let held_ms = self.held_ms.saturating_add(elapsed_ms);
        if held_ms >= self.next_repeat_ms {
            return (
                Self {
                    held_ms,
                    next_repeat_ms: self.next_repeat_ms.saturating_add(INPUT_REPEAT_MS),
                },
                true,
            );
        }

        (Self { held_ms, ..self }, false)
    }
}

#[derive(Resource, Debug, Default)]
struct ReconPanel {
    next_log_index: usize,
    manual_condor: bool,
    snapshot: Option<ReconSnapshot>,
}

#[derive(Resource, Debug)]
struct BazaarUiState {
    selected: WeaponToken,
    last_message: String,
    visual_arsenal: Option<[Option<WeaponToken>; 10]>,
}

impl Default for BazaarUiState {
    fn default() -> Self {
        Self {
            selected: WeaponToken::Gimp,
            last_message: "Select a weapon, then Add. Click staged arsenal slots to remove."
                .to_string(),
            visual_arsenal: None,
        }
    }
}

#[derive(Debug, Clone)]
struct ComputerController {
    player: PlayerId,
    opponent: ComputerOpponent,
    elapsed_ms: u64,
    bazaar_elapsed_ms: u64,
    planned: Vec<Command>,
    shopped_this_bazaar: bool,
}

impl ComputerController {
    fn new(player: PlayerId, seed: GameSeed, level: usize) -> Self {
        Self {
            player,
            opponent: ComputerOpponent::new(seed, level),
            elapsed_ms: 0,
            bazaar_elapsed_ms: 0,
            planned: Vec::new(),
            shopped_this_bazaar: false,
        }
    }

    fn reset_for_play(&mut self) {
        self.bazaar_elapsed_ms = 0;
        self.shopped_this_bazaar = false;
    }
}

#[derive(Resource, Debug, Default)]
struct SoundEventState {
    next_log_index: usize,
    last_event: Option<SoundEvent>,
    pending_events: Vec<SoundEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SoundEvent {
    MenuAction,
    PieceLocked,
    LineClear,
    BazaarEntered,
    Purchase,
    WeaponLaunch,
    WeaponLaunchGimp,
    ChallengeIncoming,
    ChallengeRejected,
    BazaarWait,
    OpponentWait,
    GameLost,
    GameWon,
    GameDead,
    AboutEasterEgg,
    Warning,
    GameOver,
}

impl SoundEvent {
    const ALL: [Self; 17] = [
        Self::MenuAction,
        Self::PieceLocked,
        Self::LineClear,
        Self::BazaarEntered,
        Self::Purchase,
        Self::WeaponLaunch,
        Self::WeaponLaunchGimp,
        Self::ChallengeIncoming,
        Self::ChallengeRejected,
        Self::BazaarWait,
        Self::OpponentWait,
        Self::GameLost,
        Self::GameWon,
        Self::GameDead,
        Self::AboutEasterEgg,
        Self::Warning,
        Self::GameOver,
    ];

    const fn id(self) -> &'static str {
        match self {
            Self::MenuAction => "menu_action",
            Self::PieceLocked => "piece_locked",
            Self::LineClear => "line_clear",
            Self::BazaarEntered => "bazaar_entered",
            Self::Purchase => "purchase",
            Self::WeaponLaunch => "weapon_launch",
            Self::WeaponLaunchGimp => "weapon_launch_gimp",
            Self::ChallengeIncoming => "challenge_incoming",
            Self::ChallengeRejected => "challenge_rejected",
            Self::BazaarWait => "bazaar_wait",
            Self::OpponentWait => "opponent_wait",
            Self::GameLost => "game_lost",
            Self::GameWon => "game_won",
            Self::GameDead => "game_dead",
            Self::AboutEasterEgg => "about_easter_egg",
            Self::Warning => "warning",
            Self::GameOver => "game_over",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|event| event.id() == id)
    }
}

#[derive(Component)]
struct BoardCell {
    player: PlayerId,
    x: usize,
    y: usize,
}

#[derive(Resource, Debug, Clone)]
struct ThemeAtlasHandles {
    original: ThemeAtlasHandle,
    high_contrast: ThemeAtlasHandle,
}

impl ThemeAtlasHandles {
    fn get(
        &self,
        choice: ThemeChoice,
        content_mode: ContentMode,
        themes: &ThemePacks,
    ) -> &ThemeAtlasImageHandle {
        let theme = themes.get(choice);
        let handles = match choice {
            ThemeChoice::Original => &self.original,
            ThemeChoice::HighContrast => &self.high_contrast,
        };
        if content_mode == ContentMode::Rated {
            if let Some(rated) = &handles.rated {
                return rated;
            }
            warn!(
                "BattleTris rated content mode requested, but theme {:?} has no rated assets; using normal sprites",
                choice
            );
            debug_assert!(!theme.sprites.supports_rated());
        }
        &handles.normal
    }
}

#[derive(Debug, Clone)]
struct ThemeAtlasHandle {
    normal: ThemeAtlasImageHandle,
    rated: Option<ThemeAtlasImageHandle>,
}

#[derive(Debug, Clone)]
struct ThemeAtlasImageHandle {
    image: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
}

#[derive(Component)]
struct HudText {
    player: PlayerId,
}

#[derive(Component)]
struct PhaseText;

#[derive(Component)]
struct PlayingGameEntity;

#[derive(Component)]
struct LegacyGameText {
    role: LegacyGameTextRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacyGameTextRole {
    Score,
    ArsenalSlot(usize),
    Message,
}

#[derive(Component)]
struct MenuText;

#[derive(Component)]
struct GameEntity;

#[derive(Component)]
struct BazaarEntity;

#[derive(Component)]
struct BazaarText {
    role: BazaarTextRole,
}

#[derive(Component)]
struct BazaarSelectionMarker {
    role: BazaarSelectionMarkerRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BazaarSelectionMarkerRole {
    Background,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BazaarTextRole {
    Catalog,
    SelectedCatalogRow,
    Funds,
    ArsenalSlot(usize),
    Message,
    Description,
}

#[derive(Component)]
struct PlayerViewEntity {
    player: PlayerId,
}

#[derive(Component)]
struct ScreenShell;

#[derive(Component)]
struct ScreenText;

#[derive(Component)]
struct GenericScreenShell;

#[derive(Component)]
struct StartupOnlyShell;

#[derive(Component)]
struct AboutShell;

#[derive(Component)]
struct ChallengeShell;

#[derive(Component)]
struct RosterShell;

#[derive(Component)]
struct ChallengeLogo;

#[derive(Component)]
struct ChallengeSliderKnob {
    x_offset: f32,
}

#[derive(Default)]
struct ChallengeLogoTextureCache {
    original: Option<Handle<Image>>,
    high_contrast: Option<Handle<Image>>,
}

impl ChallengeLogoTextureCache {
    fn get(&self, theme: ThemeChoice) -> Option<Handle<Image>> {
        match theme {
            ThemeChoice::Original => self.original.clone(),
            ThemeChoice::HighContrast => self.high_contrast.clone(),
        }
    }

    fn set(&mut self, theme: ThemeChoice, handle: Handle<Image>) {
        match theme {
            ThemeChoice::Original => self.original = Some(handle),
            ThemeChoice::HighContrast => self.high_contrast = Some(handle),
        }
    }
}

#[derive(Component)]
struct ChallengeText {
    role: ChallengeTextRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChallengeTextRole {
    UserList,
    UserInfo,
    ComputerStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BazaarWaitingText {
    LocalWaiting,
    LocalRepeated,
    PlayerWaiting(PlayerId),
    PlayerRepeated(PlayerId),
}

struct UiTextTone;

impl UiTextTone {
    fn challenge_copy(content_mode: ContentMode) -> &'static str {
        match content_mode {
            ContentMode::Normal => "",
            ContentMode::Rated => "wants a piece of yo' ass.",
        }
    }

    fn bazaar_waiting_copy(content_mode: ContentMode, text: BazaarWaitingText) -> String {
        match (content_mode, text) {
            (ContentMode::Rated, BazaarWaitingText::LocalWaiting)
            | (ContentMode::Rated, BazaarWaitingText::PlayerWaiting(_)) => {
                "Waiting for fat slut...".to_string()
            }
            (ContentMode::Rated, BazaarWaitingText::LocalRepeated)
            | (ContentMode::Rated, BazaarWaitingText::PlayerRepeated(_)) => {
                "Fuckface is getting angsty.".to_string()
            }
            (ContentMode::Normal, BazaarWaitingText::LocalWaiting) => {
                "Done. Waiting for opponent.".to_string()
            }
            (ContentMode::Normal, BazaarWaitingText::LocalRepeated) => {
                "Already waiting for opponent.".to_string()
            }
            (ContentMode::Normal, BazaarWaitingText::PlayerWaiting(player)) => {
                format!("{} done. Waiting for opponent.", player_label(player))
            }
            (ContentMode::Normal, BazaarWaitingText::PlayerRepeated(player)) => {
                format!("{} is already waiting.", player_label(player))
            }
        }
    }

    fn bazaar_done_overlay_copy(content_mode: ContentMode) -> &'static str {
        match content_mode {
            ContentMode::Normal => {
                "Done selected. Waiting for opponent; shopping controls are dimmed."
            }
            ContentMode::Rated => "Waiting for fat slut...",
        }
    }

    fn bazaar_instructions_copy(content_mode: ContentMode) -> &'static str {
        match content_mode {
            ContentMode::Normal => "Click a row to inspect. Click Add/Remove/DONE. Number slots launch in game, remove staged here.",
            ContentMode::Rated => "Click a row to inspect. Click Add/Remove/DONE. Number slots launch in game, remove staged here.",
        }
    }

    fn game_result_copy(content_mode: ContentMode, local_won: Option<bool>) -> &'static str {
        match (content_mode, local_won) {
            (ContentMode::Rated, Some(false)) => "Nice loss, shithead.",
            (ContentMode::Rated, Some(true)) => "Yer the shit!",
            (ContentMode::Normal, _) | (ContentMode::Rated, None) => "Game over",
        }
    }
}

#[derive(Component)]
struct RosterText {
    role: RosterTextRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RosterTextRole {
    UserList,
    UserInfo1,
    UserInfo2,
    Player1Name,
    Player2Name,
    Player1Score,
    Player2Score,
}

#[derive(Component)]
struct ButtonFace;

#[derive(Component)]
struct ThemedSprite {
    role: ThemedSpriteRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemedSpriteRole {
    Startup,
    Bazaar,
    Biff,
    AboutIcon,
}

#[derive(Component)]
struct ThemedTextColor {
    role: ThemedTextColorRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemedTextColorRole {
    Secondary,
    ScreenTitle,
    ScreenBody,
    Button,
    AboutTitle,
    AboutName,
    AboutCredit,
    AboutButton,
}

#[derive(Component)]
struct ThemedTextFont {
    role: ThemedTextFontRole,
}

#[derive(Component)]
struct ThemedTextMetrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemedTextFontRole {
    Title,
    Body,
    Button,
    Mono,
}

#[derive(Component)]
struct ThemedColorSprite {
    role: ThemedColorSpriteRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemedColorSpriteRole {
    ScreenBackground,
    AboutBackground,
    ButtonHighlight,
    ButtonShadow,
}

#[derive(Component)]
struct MenuButton {
    screen: ClientScreen,
    rect: Rect,
    action: MenuAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    StartHumanVsComputer,
    GoTo(ClientScreen),
    Quit,
}

fn load_theme_atlases(
    mut commands: Commands,
    themes: Res<ThemePacks>,
    asset_server: Res<AssetServer>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    commands.insert_resource(ThemeAtlasHandles {
        original: theme_atlas_handle(
            themes.get(ThemeChoice::Original),
            &asset_server,
            &mut atlas_layouts,
        ),
        high_contrast: theme_atlas_handle(
            themes.get(ThemeChoice::HighContrast),
            &asset_server,
            &mut atlas_layouts,
        ),
    });
}

fn theme_atlas_handle(
    theme: &LoadedTheme,
    asset_server: &AssetServer,
    atlas_layouts: &mut Assets<TextureAtlasLayout>,
) -> ThemeAtlasHandle {
    let layout = TextureAtlasLayout::from_grid(
        theme.cell_atlas.tile_size(),
        theme.cell_atlas.columns,
        theme.cell_atlas.rows,
        theme.cell_atlas.padding(),
        theme.cell_atlas.offset(),
    );
    let layout = atlas_layouts.add(layout);
    ThemeAtlasHandle {
        normal: ThemeAtlasImageHandle {
            image: asset_server.load(theme.sprites.atlas_for(ContentMode::Normal).to_string()),
            layout: layout.clone(),
        },
        rated: theme
            .sprites
            .rated
            .as_ref()
            .map(|rated| ThemeAtlasImageHandle {
                image: asset_server.load(rated.atlas.clone()),
                layout,
            }),
    }
}

fn setup(
    mut commands: Commands,
    settings: Res<ClientSettings>,
    themes: Res<ThemePacks>,
    atlases: Res<ThemeAtlasHandles>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn((Camera2d, Msaa::Off));
    let theme = themes.get(settings.theme);

    spawn_screen_shell(&mut commands, theme, &asset_server);
    spawn_challenge_shell(&mut commands, theme, &asset_server);
    spawn_about_shell(&mut commands, theme, &asset_server);
    spawn_roster_shell(&mut commands, theme, &asset_server);

    spawn_player_view(
        &mut commands,
        theme,
        atlases.get(settings.theme, settings.content_mode, &themes),
        PlayerId::One,
        theme.layout.board.player_one_left,
        "Player 1",
    );
    spawn_player_view(
        &mut commands,
        theme,
        atlases.get(settings.theme, settings.content_mode, &themes),
        PlayerId::Two,
        theme.layout.board.player_two_left,
        "Player 2 / Computer",
    );
    spawn_bazaar_overlay(&mut commands, theme, &asset_server);
    spawn_legacy_game_hud(&mut commands, theme, &asset_server);

    commands.spawn((
        Text2d::new("BattleTris"),
        themed_text_font_at_size(theme, ThemedTextFontRole::Body, 22.0, &asset_server),
        TextColor(theme.palette.text_secondary),
        ThemedTextColor {
            role: ThemedTextColorRole::Secondary,
        },
        ThemedTextMetrics,
        Transform::from_xyz(0.0, -300.0, 5.0),
        PhaseText,
        PlayingGameEntity,
        GameEntity,
    ));

    commands.spawn((
        Text2d::new(""),
        themed_text_font(theme, ThemedTextFontRole::Title, &asset_server),
        TextColor(theme.screen.title_text),
        ThemedTextColor {
            role: ThemedTextColorRole::ScreenTitle,
        },
        ThemedTextFont {
            role: ThemedTextFontRole::Title,
        },
        ThemedTextMetrics,
        Transform::from_xyz(0.0, 245.0, 10.0),
        MenuText,
        GenericScreenShell,
        ScreenShell,
    ));
}

fn spawn_bazaar_overlay(commands: &mut Commands, theme: &LoadedTheme, asset_server: &AssetServer) {
    let text_assets = ThemeTextAssets {
        theme,
        asset_server,
    };
    let mut backdrop = Sprite::from_image(asset_server.load(theme.sprites.bazaar.clone()));
    backdrop.custom_size = Some(Vec2::new(490.0, 164.0));
    let art_center = bazaar_world(20.0 + 245.0, 18.0 + 82.0);
    commands.spawn((
        backdrop,
        Transform::from_xyz(art_center.x, art_center.y, 20.0),
        Visibility::Hidden,
        ThemedSprite {
            role: ThemedSpriteRole::Bazaar,
        },
        BazaarEntity,
        GameEntity,
    ));

    spawn_bazaar_panel(
        commands,
        bazaar_rect(20.0, 200.0, 300.0, 780.0),
        MotifBevel::Inset,
    );
    spawn_bazaar_scrollbar(commands, 20.0, 200.0, 300.0, 780.0, true, true);
    commands.spawn((
        Sprite::from_color(motif_red3_color(), Vec2::new(254.0, 16.0)),
        Transform::from_xyz(-249.0, bazaar_world(0.0, 210.0).y, 23.0),
        Visibility::Hidden,
        BazaarSelectionMarker {
            role: BazaarSelectionMarkerRole::Background,
        },
        BazaarEntity,
        GameEntity,
    ));
    spawn_bazaar_panel(
        commands,
        bazaar_rect(340.0, 600.0, 780.0, 780.0),
        MotifBevel::Inset,
    );
    spawn_bazaar_scrollbar(commands, 340.0, 600.0, 780.0, 780.0, true, false);
    spawn_bazaar_panel(
        commands,
        bazaar_rect(325.0, 215.0, 475.0, 245.0),
        MotifBevel::Raised,
    );
    spawn_bazaar_panel(
        commands,
        bazaar_rect(325.0, 245.0, 475.0, 315.0),
        MotifBevel::Inset,
    );
    spawn_bazaar_static_text(
        commands,
        text_assets,
        "Funds",
        bazaar_world(372.0, 235.0),
        12.0,
        motif_blue_color(),
        Anchor::CENTER,
    );
    spawn_bazaar_dynamic_text(
        commands,
        text_assets,
        BazaarTextRole::Funds,
        bazaar_world(400.0, 282.0),
        12.0,
        motif_red3_color(),
        Anchor::CENTER,
    );

    for slot in 0..10 {
        let y1 = 204.0 + slot as f32 * 30.0;
        let y2 = y1 + 24.0;
        spawn_bazaar_panel(
            commands,
            bazaar_rect(503.0, y1, 778.0, y2),
            MotifBevel::Raised,
        );
        spawn_bazaar_dynamic_text(
            commands,
            text_assets,
            BazaarTextRole::ArsenalSlot(slot),
            bazaar_world(512.0, y1 + 12.0),
            12.0,
            motif_dim_text_color(),
            Anchor::TOP_LEFT,
        );
    }

    for (label, rect) in [
        ("Add >>", bazaar_rect(340.0, 365.0, 460.0, 415.0)),
        ("<< Remove", bazaar_rect(340.0, 435.0, 460.0, 485.0)),
        ("DONE", bazaar_rect(340.0, 505.0, 460.0, 575.0)),
    ] {
        spawn_bazaar_panel(commands, rect, MotifBevel::Raised);
        let (center, _) = rect;
        spawn_bazaar_static_text(
            commands,
            text_assets,
            label,
            center,
            12.0,
            motif_blue_color(),
            Anchor::CENTER,
        );
    }

    spawn_bazaar_dynamic_text(
        commands,
        text_assets,
        BazaarTextRole::Catalog,
        bazaar_world(24.0, 205.0),
        13.3,
        motif_red3_color(),
        Anchor::TOP_LEFT,
    );
    let selected_text_position = bazaar_world(24.0, 205.0);
    commands.spawn((
        Text2d::new(""),
        text_assets.font(ThemedTextFontRole::Body, 13.3),
        TextColor(Color::WHITE),
        ThemedTextMetrics,
        Anchor::TOP_LEFT,
        Transform::from_xyz(selected_text_position.x, selected_text_position.y, 24.5),
        Visibility::Hidden,
        BazaarEntity,
        BazaarText {
            role: BazaarTextRole::SelectedCatalogRow,
        },
        BazaarSelectionMarker {
            role: BazaarSelectionMarkerRole::Text,
        },
        GameEntity,
    ));
    spawn_bazaar_dynamic_text(
        commands,
        text_assets,
        BazaarTextRole::Message,
        bazaar_world(640.0, 552.0),
        12.0,
        motif_message_green_color(),
        Anchor::CENTER,
    );
    spawn_bazaar_dynamic_text(
        commands,
        text_assets,
        BazaarTextRole::Description,
        bazaar_world(348.0, 606.0),
        12.0,
        motif_red3_color(),
        Anchor::TOP_LEFT,
    );
}

fn spawn_bazaar_panel(commands: &mut Commands, (center, size): (Vec2, Vec2), bevel: MotifBevel) {
    spawn_bazaar_rect(commands, center, size, motif_text_panel_color(), 21.0);
    spawn_bazaar_bevel(commands, center, size, 22.0, bevel);
}

fn spawn_bazaar_rect(commands: &mut Commands, center: Vec2, size: Vec2, color: Color, z: f32) {
    commands.spawn((
        Sprite::from_color(color, size),
        Transform::from_xyz(center.x, center.y, z),
        Visibility::Hidden,
        BazaarEntity,
        GameEntity,
    ));
}

fn spawn_bazaar_bevel(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    z: f32,
    bevel: MotifBevel,
) {
    let (top_left, bottom_right) = match bevel {
        MotifBevel::Raised => (motif_highlight_color(), motif_shadow_color()),
        MotifBevel::Inset => (motif_shadow_color(), motif_highlight_color()),
    };
    for (offset, bevel_size, bevel_color) in [
        (
            Vec2::new(0.0, size.y / 2.0),
            Vec2::new(size.x, 2.0),
            top_left,
        ),
        (
            Vec2::new(-size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            top_left,
        ),
        (
            Vec2::new(0.0, -size.y / 2.0),
            Vec2::new(size.x, 2.0),
            bottom_right,
        ),
        (
            Vec2::new(size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            bottom_right,
        ),
    ] {
        spawn_bazaar_rect(
            commands,
            Vec2::new(center.x + offset.x, center.y + offset.y),
            bevel_size,
            bevel_color,
            z,
        );
    }
}

fn spawn_bazaar_scrollbar(
    commands: &mut Commands,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    vertical: bool,
    horizontal: bool,
) {
    let vertical_y2 = if horizontal { y2 - 18.0 } else { y2 };
    let horizontal_x2 = if vertical { x2 - 18.0 } else { x2 };
    if vertical {
        let bar_x1 = x2 - 18.0;
        spawn_bazaar_legacy_scrollbar(commands, bazaar_rect(bar_x1, y1, x2, vertical_y2), true);
    }
    if horizontal {
        let bar_y1 = y2 - 18.0;
        spawn_bazaar_legacy_scrollbar(commands, bazaar_rect(x1, bar_y1, horizontal_x2, y2), false);
    }
}

fn spawn_bazaar_legacy_scrollbar(
    commands: &mut Commands,
    (center, size): (Vec2, Vec2),
    vertical: bool,
) {
    let parts = legacy_scrollbar_parts(center, size, vertical);
    spawn_bazaar_scrollbar_panel(
        commands,
        center,
        size,
        motif_text_panel_color(),
        21.0,
        MotifBevel::Inset,
    );
    spawn_bazaar_scrollbar_panel(
        commands,
        parts.thumb_center,
        parts.thumb_size,
        motif_button_face_color(),
        22.2,
        MotifBevel::Inset,
    );
    spawn_bazaar_arrow_button(
        commands,
        parts.leading_arrow_center,
        parts.arrow_size,
        if vertical {
            MotifArrowDirection::Up
        } else {
            MotifArrowDirection::Left
        },
    );
    spawn_bazaar_arrow_button(
        commands,
        parts.trailing_arrow_center,
        parts.arrow_size,
        if vertical {
            MotifArrowDirection::Down
        } else {
            MotifArrowDirection::Right
        },
    );
}

fn spawn_bazaar_scrollbar_panel(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    color: Color,
    z: f32,
    bevel: MotifBevel,
) {
    spawn_bazaar_rect(commands, center, size, color, z);
    spawn_bazaar_bevel(commands, center, size, z + 0.5, bevel);
}

fn spawn_bazaar_arrow_button(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    direction: MotifArrowDirection,
) {
    spawn_bazaar_scrollbar_panel(
        commands,
        center,
        size,
        motif_button_face_color(),
        23.0,
        MotifBevel::Inset,
    );
    spawn_bazaar_arrow_glyph(commands, center, direction);
}

fn spawn_bazaar_arrow_glyph(commands: &mut Commands, center: Vec2, direction: MotifArrowDirection) {
    for index in 0..3 {
        let spread = 1.0 + index as f32 * 2.0;
        let step = index as f32 * 1.6;
        let (offset, size) = match direction {
            MotifArrowDirection::Up => (Vec2::new(0.0, 2.4 - step), Vec2::new(spread, 1.0)),
            MotifArrowDirection::Down => (Vec2::new(0.0, -2.4 + step), Vec2::new(spread, 1.0)),
            MotifArrowDirection::Left => (Vec2::new(-2.4 + step, 0.0), Vec2::new(1.0, spread)),
            MotifArrowDirection::Right => (Vec2::new(2.4 - step, 0.0), Vec2::new(1.0, spread)),
        };
        spawn_bazaar_rect(
            commands,
            Vec2::new(center.x + offset.x, center.y + offset.y),
            size,
            Color::BLACK,
            24.0,
        );
    }
}

fn spawn_bazaar_static_text(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    label: &'static str,
    position: Vec2,
    font_size: f32,
    color: Color,
    anchor: Anchor,
) {
    commands.spawn((
        Text2d::new(label),
        text_assets.font(ThemedTextFontRole::Body, font_size),
        TextColor(color),
        ThemedTextMetrics,
        anchor,
        Transform::from_xyz(position.x, position.y, 24.0),
        Visibility::Hidden,
        BazaarEntity,
        GameEntity,
    ));
}

fn spawn_bazaar_dynamic_text(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    role: BazaarTextRole,
    position: Vec2,
    font_size: f32,
    color: Color,
    anchor: Anchor,
) {
    let font_role = bazaar_text_font_role(role);
    commands.spawn((
        Text2d::new(""),
        text_assets.font(font_role, font_size),
        TextColor(color),
        ThemedTextMetrics,
        anchor,
        Transform::from_xyz(position.x, position.y, 24.0),
        Visibility::Hidden,
        BazaarEntity,
        BazaarText { role },
        GameEntity,
    ));
}

fn bazaar_text_font_role(role: BazaarTextRole) -> ThemedTextFontRole {
    match role {
        BazaarTextRole::Funds => ThemedTextFontRole::Mono,
        BazaarTextRole::Catalog
        | BazaarTextRole::SelectedCatalogRow
        | BazaarTextRole::ArsenalSlot(_)
        | BazaarTextRole::Message
        | BazaarTextRole::Description => ThemedTextFontRole::Body,
    }
}

fn bazaar_rect(x1: f32, y1: f32, x2: f32, y2: f32) -> (Vec2, Vec2) {
    let center = Vec2::new(
        (x1 + x2) / 2.0 - LEGACY_BAZAAR_WIDTH / 2.0,
        LEGACY_BAZAAR_HEIGHT / 2.0 - (y1 + y2) / 2.0,
    );
    let size = Vec2::new(x2 - x1, y2 - y1);
    (center, size)
}

fn bazaar_world(x: f32, y: f32) -> Vec2 {
    Vec2::new(
        x - LEGACY_BAZAAR_WIDTH / 2.0,
        LEGACY_BAZAAR_HEIGHT / 2.0 - y,
    )
}

fn spawn_legacy_game_hud(commands: &mut Commands, theme: &LoadedTheme, asset_server: &AssetServer) {
    spawn_game_panel(
        commands,
        game_screen_rect(
            LEGACY_GAME_SCORE_X,
            LEGACY_GAME_SCORE_Y,
            LEGACY_GAME_SCORE_WIDTH,
            LEGACY_GAME_SCORE_HEIGHT,
        ),
        motif_text_panel_color(),
        1.0,
        MotifBevel::Inset,
    );
    spawn_legacy_game_text(
        commands,
        theme,
        asset_server,
        LegacyGameTextRole::Score,
        309.0,
        43.0,
        10.0,
    );

    for slot in 0..10 {
        let y = LEGACY_GAME_ARSENAL_Y + slot as f32 * LEGACY_GAME_ARSENAL_ROW_HEIGHT + 3.0;
        spawn_game_panel(
            commands,
            game_screen_rect(LEGACY_GAME_ARSENAL_X, y, LEGACY_GAME_ARSENAL_WIDTH, 30.0),
            motif_button_face_color(),
            1.0,
            MotifBevel::Raised,
        );
        spawn_legacy_game_text(
            commands,
            theme,
            asset_server,
            LegacyGameTextRole::ArsenalSlot(slot),
            311.0,
            y + 8.0,
            11.0,
        );
    }

    spawn_legacy_game_text(
        commands,
        theme,
        asset_server,
        LegacyGameTextRole::Message,
        305.0,
        630.0,
        11.0,
    );
}

fn spawn_game_panel(
    commands: &mut Commands,
    (center, size): (Vec2, Vec2),
    color: Color,
    z: f32,
    bevel: MotifBevel,
) {
    spawn_game_rect(commands, center, size, color, z);
    spawn_game_bevel(commands, center, size, z + 0.1, bevel);
}

fn spawn_game_bevel(commands: &mut Commands, center: Vec2, size: Vec2, z: f32, bevel: MotifBevel) {
    let (top_left, bottom_right) = match bevel {
        MotifBevel::Raised => (motif_highlight_color(), motif_shadow_color()),
        MotifBevel::Inset => (motif_shadow_color(), motif_highlight_color()),
    };
    for (offset, bevel_size, bevel_color) in [
        (
            Vec2::new(0.0, size.y / 2.0),
            Vec2::new(size.x, 2.0),
            top_left,
        ),
        (
            Vec2::new(-size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            top_left,
        ),
        (
            Vec2::new(0.0, -size.y / 2.0),
            Vec2::new(size.x, 2.0),
            bottom_right,
        ),
        (
            Vec2::new(size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            bottom_right,
        ),
    ] {
        spawn_game_rect(
            commands,
            Vec2::new(center.x + offset.x, center.y + offset.y),
            bevel_size,
            bevel_color,
            z,
        );
    }
}

fn spawn_game_rect(commands: &mut Commands, center: Vec2, size: Vec2, color: Color, z: f32) {
    commands.spawn((
        Sprite::from_color(color, size),
        Transform::from_xyz(center.x, center.y, z),
        Visibility::Hidden,
        PlayingGameEntity,
        GameEntity,
    ));
}

fn spawn_legacy_game_text(
    commands: &mut Commands,
    theme: &LoadedTheme,
    asset_server: &AssetServer,
    role: LegacyGameTextRole,
    x: f32,
    y: f32,
    font_size: f32,
) {
    let position = game_screen_world(x, y);
    let color = match role {
        LegacyGameTextRole::Message => Color::BLACK,
        LegacyGameTextRole::Score | LegacyGameTextRole::ArsenalSlot(_) => motif_blue_color(),
    };
    commands.spawn((
        Text2d::new(""),
        themed_text_font_at_size(
            theme,
            legacy_game_text_font_role(role),
            font_size,
            asset_server,
        ),
        TextColor(color),
        ThemedTextMetrics,
        Anchor::TOP_LEFT,
        Transform::from_xyz(position.x, position.y, 5.0),
        LegacyGameText { role },
        PlayingGameEntity,
        GameEntity,
    ));
}

fn legacy_game_text_font_role(role: LegacyGameTextRole) -> ThemedTextFontRole {
    match role {
        LegacyGameTextRole::Score | LegacyGameTextRole::ArsenalSlot(_) => ThemedTextFontRole::Mono,
        LegacyGameTextRole::Message => ThemedTextFontRole::Body,
    }
}

fn game_screen_rect(x: f32, y: f32, width: f32, height: f32) -> (Vec2, Vec2) {
    let center = game_screen_world(x + width / 2.0, y + height / 2.0);
    (center, Vec2::new(width, height))
}

fn game_screen_world(x: f32, y: f32) -> Vec2 {
    Vec2::new(x - LEGACY_GAME_WIDTH / 2.0, LEGACY_GAME_HEIGHT / 2.0 - y)
}

fn spawn_screen_shell(commands: &mut Commands, theme: &LoadedTheme, asset_server: &AssetServer) {
    commands.spawn((
        Sprite::from_color(theme.screen.background, Vec2::new(640.0, 600.0)),
        Transform::from_xyz(0.0, 0.0, -3.0),
        ThemedColorSprite {
            role: ThemedColorSpriteRole::ScreenBackground,
        },
        GenericScreenShell,
        ScreenShell,
    ));

    let mut startup_sprite = Sprite::from_image(asset_server.load(theme.sprites.startup.clone()));
    startup_sprite.custom_size = Some(Vec2::new(640.0, 440.0));
    commands.spawn((
        startup_sprite,
        Transform::from_xyz(0.0, 80.0, -2.0),
        ThemedSprite {
            role: ThemedSpriteRole::Startup,
        },
        StartupOnlyShell,
        GenericScreenShell,
        ScreenShell,
    ));

    commands.spawn((
        Sprite::from_image(asset_server.load(theme.sprites.biff.clone())),
        Transform::from_xyz(-220.0, -155.0, 1.0),
        ThemedSprite {
            role: ThemedSpriteRole::Biff,
        },
        GenericScreenShell,
        ScreenShell,
    ));

    commands.spawn((
        Text2d::new(""),
        themed_text_font(theme, ThemedTextFontRole::Body, asset_server),
        TextColor(theme.screen.body_text),
        ThemedTextColor {
            role: ThemedTextColorRole::ScreenBody,
        },
        ThemedTextFont {
            role: ThemedTextFontRole::Body,
        },
        ThemedTextMetrics,
        Transform::from_xyz(55.0, 70.0, 4.0),
        ScreenText,
        GenericScreenShell,
        ScreenShell,
    ));

    for spec in startup_buttons(theme) {
        spawn_menu_button(commands, theme, asset_server, spec);
    }
    for spec in secondary_screen_buttons(theme) {
        spawn_menu_button(commands, theme, asset_server, spec);
    }
}

fn spawn_challenge_shell(commands: &mut Commands, theme: &LoadedTheme, asset_server: &AssetServer) {
    let text_assets = ThemeTextAssets {
        theme,
        asset_server,
    };
    spawn_challenge_rect(
        commands,
        challenge_rect(0.0, 0.0, 800.0, 700.0),
        motif_page_color(),
        -3.0,
    );
    spawn_challenge_panel(
        commands,
        challenge_rect(20.0, 20.0, 400.0, 500.0),
        motif_text_panel_color(),
        0.0,
        MotifBevel::Inset,
    );
    spawn_challenge_panel(
        commands,
        challenge_rect(440.0, 320.0, 780.0, 680.0),
        motif_text_panel_color(),
        0.0,
        MotifBevel::Inset,
    );
    spawn_challenge_computer_frame(commands, text_assets);

    let logo_top_left = challenge_point(540.0, 30.0);
    let logo_size = Vec2::new(105.0, 105.0);
    let logo_center = Vec2::new(
        logo_top_left.x + logo_size.x / 2.0 - 320.0,
        300.0 - (logo_top_left.y + logo_size.y / 2.0),
    );
    commands.spawn((
        Sprite::from_image(asset_server.load(theme.sprites.biff.clone())),
        Transform::from_xyz(logo_center.x, logo_center.y, 1.0),
        Visibility::Hidden,
        ChallengeLogo,
        ChallengeShell,
        ScreenShell,
    ));

    spawn_challenge_scrollbar(commands, 380.0, 20.0, 400.0, 480.0, true);
    spawn_challenge_scrollbar(commands, 20.0, 480.0, 380.0, 500.0, false);
    spawn_challenge_scrollbar(commands, 760.0, 320.0, 780.0, 680.0, true);

    spawn_challenge_text(
        commands,
        text_assets,
        ChallengeTextRole::UserList,
        challenge_rect_center(38.0, 44.0, 382.0, 470.0),
        12.0,
        motif_red3_color(),
    );
    spawn_challenge_text(
        commands,
        text_assets,
        ChallengeTextRole::UserInfo,
        challenge_rect_center(458.0, 340.0, 762.0, 660.0),
        12.0,
        motif_red3_color(),
    );
    spawn_challenge_text(
        commands,
        text_assets,
        ChallengeTextRole::ComputerStatus,
        challenge_world(210.0, 625.0),
        11.0,
        Color::BLACK,
    );
    spawn_static_challenge_text(
        commands,
        text_assets,
        "Available for challenges",
        challenge_world(155.0, 653.0),
        11.0,
        Color::BLACK,
    );
    spawn_challenge_checkbox(commands, challenge_rect(40.0, 648.0, 52.0, 660.0), 2.0);
    spawn_challenge_slider(commands);
}

#[derive(Debug, Clone, Copy)]
enum MotifBevel {
    Raised,
    Inset,
}

#[derive(Debug, Clone, Copy)]
enum MotifArrowDirection {
    Up,
    Down,
    Left,
    Right,
}

const LEGACY_SCROLLBAR_INSET: f32 = 2.0;

#[derive(Debug, Clone, Copy)]
struct LegacyScrollbarParts {
    thumb_center: Vec2,
    thumb_size: Vec2,
    leading_arrow_center: Vec2,
    trailing_arrow_center: Vec2,
    arrow_size: Vec2,
}

fn legacy_scrollbar_parts(center: Vec2, size: Vec2, vertical: bool) -> LegacyScrollbarParts {
    let thickness = if vertical { size.x } else { size.y };
    let inset = LEGACY_SCROLLBAR_INSET.min(thickness / 3.0).max(0.0);
    let cap_extent = (thickness - inset * 2.0).max(1.0);
    let track_length = if vertical { size.y } else { size.x };
    let thumb_extent = (track_length - inset * 4.0 - cap_extent * 2.0).max(1.0);

    if vertical {
        let arrow_offset = size.y / 2.0 - inset - cap_extent / 2.0;
        LegacyScrollbarParts {
            thumb_center: center,
            thumb_size: Vec2::new(cap_extent, thumb_extent),
            leading_arrow_center: Vec2::new(center.x, center.y + arrow_offset),
            trailing_arrow_center: Vec2::new(center.x, center.y - arrow_offset),
            arrow_size: Vec2::new(cap_extent, cap_extent),
        }
    } else {
        let arrow_offset = size.x / 2.0 - inset - cap_extent / 2.0;
        LegacyScrollbarParts {
            thumb_center: center,
            thumb_size: Vec2::new(thumb_extent, cap_extent),
            leading_arrow_center: Vec2::new(center.x - arrow_offset, center.y),
            trailing_arrow_center: Vec2::new(center.x + arrow_offset, center.y),
            arrow_size: Vec2::new(cap_extent, cap_extent),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ChallengeScreenRect {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

impl ChallengeScreenRect {
    const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }
}

fn motif_page_color() -> Color {
    Color::srgba_u8(0xbf, 0xbf, 0xbf, 0xff)
}

fn motif_text_panel_color() -> Color {
    Color::srgba_u8(0xa8, 0xa8, 0xa8, 0xff)
}

fn motif_button_face_color() -> Color {
    Color::srgba_u8(0xbe, 0xbe, 0xbe, 0xff)
}

fn motif_button_hover_color() -> Color {
    Color::srgba_u8(0xd6, 0xd6, 0xd6, 0xff)
}

fn motif_button_pressed_color() -> Color {
    Color::srgba_u8(0xa8, 0xa8, 0xa8, 0xff)
}

fn motif_highlight_color() -> Color {
    Color::srgba_u8(0xe4, 0xe4, 0xe4, 0xff)
}

fn motif_shadow_color() -> Color {
    Color::srgba_u8(0x67, 0x67, 0x67, 0xff)
}

fn motif_red3_color() -> Color {
    Color::srgba_u8(0xcd, 0x00, 0x00, 0xff)
}

fn motif_blue_color() -> Color {
    Color::srgba_u8(0x00, 0x00, 0xcc, 0xff)
}

fn motif_dim_text_color() -> Color {
    Color::srgba_u8(0xc0, 0xc0, 0xc0, 0xff)
}

fn motif_message_green_color() -> Color {
    Color::srgba_u8(0x33, 0x66, 0x00, 0xff)
}

fn spawn_roster_shell(commands: &mut Commands, theme: &LoadedTheme, asset_server: &AssetServer) {
    let text_assets = ThemeTextAssets {
        theme,
        asset_server,
    };
    spawn_roster_rect(
        commands,
        roster_rect(0.0, 0.0, LEGACY_ROSTER_WIDTH, LEGACY_ROSTER_HEIGHT),
        motif_page_color(),
        -3.0,
    );

    let logo_center = roster_world(
        75.0 + LEGACY_ROSTER_BIFF_WIDTH / 2.0,
        3.0 + LEGACY_ROSTER_BIFF_HEIGHT / 2.0,
    );
    commands.spawn((
        Sprite::from_image(asset_server.load(theme.sprites.biff.clone())),
        Transform::from_xyz(logo_center.x, logo_center.y, 1.0),
        Visibility::Hidden,
        ThemedSprite {
            role: ThemedSpriteRole::Biff,
        },
        RosterShell,
        ScreenShell,
    ));

    spawn_roster_panel(
        commands,
        roster_rect(15.0, 123.0, 225.0, 547.0),
        motif_text_panel_color(),
        0.0,
        MotifBevel::Inset,
    );
    spawn_roster_panel(
        commands,
        roster_rect(255.0, 123.0, 585.0, 330.0),
        motif_text_panel_color(),
        0.0,
        MotifBevel::Inset,
    );
    spawn_roster_panel(
        commands,
        roster_rect(255.0, 341.0, 585.0, 547.0),
        motif_text_panel_color(),
        0.0,
        MotifBevel::Inset,
    );
    spawn_roster_scrollbar(commands, 15.0, 123.0, 225.0, 547.0);
    spawn_roster_scrollbar(commands, 255.0, 123.0, 585.0, 330.0);
    spawn_roster_scrollbar(commands, 255.0, 341.0, 585.0, 547.0);

    spawn_roster_static_label(
        commands,
        text_assets,
        "Head\nto\nHead",
        roster_rect(255.0, 15.0, 322.0, 120.0),
        14.0,
    );
    spawn_roster_dynamic_label(
        commands,
        text_assets,
        RosterTextRole::Player1Name,
        roster_rect(322.0, 15.0, 453.0, 67.0),
        12.0,
    );
    spawn_roster_dynamic_label(
        commands,
        text_assets,
        RosterTextRole::Player2Name,
        roster_rect(453.0, 15.0, 585.0, 67.0),
        12.0,
    );
    spawn_roster_dynamic_label(
        commands,
        text_assets,
        RosterTextRole::Player1Score,
        roster_rect(322.0, 67.0, 453.0, 120.0),
        14.0,
    );
    spawn_roster_dynamic_label(
        commands,
        text_assets,
        RosterTextRole::Player2Score,
        roster_rect(453.0, 67.0, 585.0, 120.0),
        14.0,
    );

    spawn_roster_dynamic_text(
        commands,
        text_assets,
        RosterTextRole::UserList,
        roster_world(26.0, 139.0),
        15.0,
        motif_red3_color(),
    );
    spawn_roster_dynamic_text(
        commands,
        text_assets,
        RosterTextRole::UserInfo1,
        roster_world(270.0, 139.0),
        11.0,
        Color::BLACK,
    );
    spawn_roster_dynamic_text(
        commands,
        text_assets,
        RosterTextRole::UserInfo2,
        roster_world(270.0, 357.0),
        11.0,
        Color::BLACK,
    );

    spawn_roster_static_button(
        commands,
        text_assets,
        "By Name",
        roster_rect(22.0, 555.0, 112.0, 585.0),
    );
    spawn_roster_static_button(
        commands,
        text_assets,
        "By Rank",
        roster_rect(127.0, 555.0, 217.0, 585.0),
    );
}

fn spawn_roster_static_button(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    label: &'static str,
    rect: (Vec2, Vec2),
) {
    spawn_roster_panel(
        commands,
        rect,
        motif_button_face_color(),
        1.0,
        MotifBevel::Raised,
    );
    let (center, _) = rect;
    commands.spawn((
        Text2d::new(label),
        text_assets.font(ThemedTextFontRole::Button, 12.0),
        TextColor(motif_blue_color()),
        ThemedTextMetrics,
        Transform::from_xyz(center.x, center.y, 4.0),
        Visibility::Hidden,
        RosterShell,
        ScreenShell,
    ));
}

fn spawn_roster_static_label(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    label: &'static str,
    rect: (Vec2, Vec2),
    font_size: f32,
) {
    spawn_roster_panel(
        commands,
        rect,
        motif_text_panel_color(),
        1.0,
        MotifBevel::Raised,
    );
    let (center, _) = rect;
    commands.spawn((
        Text2d::new(label),
        text_assets.font(ThemedTextFontRole::Body, font_size),
        TextColor(motif_blue_color()),
        ThemedTextMetrics,
        Transform::from_xyz(center.x, center.y - 4.0, 4.0),
        Visibility::Hidden,
        RosterShell,
        ScreenShell,
    ));
}

fn spawn_roster_dynamic_label(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    role: RosterTextRole,
    rect: (Vec2, Vec2),
    font_size: f32,
) {
    spawn_roster_panel(
        commands,
        rect,
        motif_text_panel_color(),
        1.0,
        MotifBevel::Raised,
    );
    let (center, _) = rect;
    commands.spawn((
        Text2d::new(""),
        text_assets.font(ThemedTextFontRole::Body, font_size),
        TextColor(motif_blue_color()),
        ThemedTextMetrics,
        Transform::from_xyz(center.x, center.y - 4.0, 4.0),
        Visibility::Hidden,
        RosterText { role },
        RosterShell,
        ScreenShell,
    ));
}

fn spawn_roster_dynamic_text(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    role: RosterTextRole,
    position: Vec2,
    font_size: f32,
    color: Color,
) {
    commands.spawn((
        Text2d::new(""),
        text_assets.font(ThemedTextFontRole::Body, font_size),
        TextColor(color),
        ThemedTextMetrics,
        Anchor::TOP_LEFT,
        Transform::from_xyz(position.x, position.y, 4.0),
        Visibility::Hidden,
        RosterText { role },
        RosterShell,
        ScreenShell,
    ));
}

fn spawn_roster_panel(
    commands: &mut Commands,
    (center, size): (Vec2, Vec2),
    color: Color,
    z: f32,
    bevel: MotifBevel,
) {
    spawn_roster_rect(commands, (center, size), color, z);
    spawn_roster_bevel(commands, center, size, z + 0.1, bevel);
}

fn spawn_roster_scrollbar(commands: &mut Commands, _x1: f32, y1: f32, x2: f32, y2: f32) {
    let bar_x1 = x2 - 20.0;
    let (center, size) = roster_rect(bar_x1, y1, x2, y2);
    spawn_roster_panel(
        commands,
        (center, size),
        motif_page_color(),
        2.0,
        MotifBevel::Inset,
    );
    let parts = legacy_scrollbar_parts(center, size, true);
    spawn_roster_panel(
        commands,
        (parts.thumb_center, parts.thumb_size),
        motif_button_face_color(),
        2.2,
        MotifBevel::Inset,
    );
    spawn_roster_arrow_button(
        commands,
        parts.leading_arrow_center,
        parts.arrow_size,
        MotifArrowDirection::Up,
    );
    spawn_roster_arrow_button(
        commands,
        parts.trailing_arrow_center,
        parts.arrow_size,
        MotifArrowDirection::Down,
    );
}

fn spawn_roster_arrow_button(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    direction: MotifArrowDirection,
) {
    spawn_roster_panel(
        commands,
        (center, size),
        motif_button_face_color(),
        2.4,
        MotifBevel::Inset,
    );
    spawn_roster_arrow_glyph(commands, center, direction, 3.0);
}

fn spawn_roster_arrow_glyph(
    commands: &mut Commands,
    center: Vec2,
    direction: MotifArrowDirection,
    z: f32,
) {
    for index in 0..3 {
        let spread = 1.0 + index as f32 * 2.0;
        let step = index as f32 * 1.6;
        let offset = match direction {
            MotifArrowDirection::Up => Vec2::new(0.0, 2.4 - step),
            MotifArrowDirection::Down => Vec2::new(0.0, -2.4 + step),
            MotifArrowDirection::Left | MotifArrowDirection::Right => Vec2::ZERO,
        };
        spawn_roster_rect(
            commands,
            (
                Vec2::new(center.x + offset.x, center.y + offset.y),
                Vec2::new(spread, 1.0),
            ),
            Color::BLACK,
            z,
        );
    }
}

fn spawn_roster_bevel(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    z: f32,
    bevel: MotifBevel,
) {
    let (top_left, bottom_right) = match bevel {
        MotifBevel::Raised => (motif_highlight_color(), motif_shadow_color()),
        MotifBevel::Inset => (motif_shadow_color(), motif_highlight_color()),
    };
    for (offset, bevel_size, bevel_color) in [
        (
            Vec2::new(0.0, size.y / 2.0),
            Vec2::new(size.x, 2.0),
            top_left,
        ),
        (
            Vec2::new(-size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            top_left,
        ),
        (
            Vec2::new(0.0, -size.y / 2.0),
            Vec2::new(size.x, 2.0),
            bottom_right,
        ),
        (
            Vec2::new(size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            bottom_right,
        ),
    ] {
        spawn_roster_rect(
            commands,
            (
                Vec2::new(center.x + offset.x, center.y + offset.y),
                bevel_size,
            ),
            bevel_color,
            z,
        );
    }
}

fn spawn_roster_rect(commands: &mut Commands, (center, size): (Vec2, Vec2), color: Color, z: f32) {
    commands.spawn((
        Sprite::from_color(color, size),
        Transform::from_xyz(center.x, center.y, z),
        Visibility::Hidden,
        RosterShell,
        ScreenShell,
    ));
}

fn roster_rect(x1: f32, y1: f32, x2: f32, y2: f32) -> (Vec2, Vec2) {
    let center = Vec2::new(
        (x1 + x2) / 2.0 - LEGACY_ROSTER_WIDTH / 2.0,
        LEGACY_ROSTER_HEIGHT / 2.0 - (y1 + y2) / 2.0,
    );
    let size = Vec2::new(x2 - x1, y2 - y1);
    (center, size)
}

fn roster_world(x: f32, y: f32) -> Vec2 {
    Vec2::new(
        x - LEGACY_ROSTER_WIDTH / 2.0,
        LEGACY_ROSTER_HEIGHT / 2.0 - y,
    )
}

fn spawn_challenge_rect(
    commands: &mut Commands,
    (center, size): (Vec2, Vec2),
    color: Color,
    z: f32,
) {
    commands.spawn((
        Sprite::from_color(color, size),
        Transform::from_xyz(center.x, center.y, z),
        Visibility::Hidden,
        ChallengeShell,
        ScreenShell,
    ));
}

fn spawn_challenge_panel(
    commands: &mut Commands,
    (center, size): (Vec2, Vec2),
    color: Color,
    z: f32,
    bevel: MotifBevel,
) {
    spawn_challenge_rect(commands, (center, size), color, z);
    spawn_challenge_bevel(commands, center, size, z + 0.1, bevel);
}

fn spawn_challenge_bevel(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    z: f32,
    bevel: MotifBevel,
) {
    let (top_left, bottom_right) = match bevel {
        MotifBevel::Raised => (motif_highlight_color(), motif_shadow_color()),
        MotifBevel::Inset => (motif_shadow_color(), motif_highlight_color()),
    };
    for (offset, bevel_size, bevel_color) in [
        (
            Vec2::new(0.0, size.y / 2.0),
            Vec2::new(size.x, 2.0),
            top_left,
        ),
        (
            Vec2::new(-size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            top_left,
        ),
        (
            Vec2::new(0.0, -size.y / 2.0),
            Vec2::new(size.x, 2.0),
            bottom_right,
        ),
        (
            Vec2::new(size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            bottom_right,
        ),
    ] {
        spawn_challenge_rect(
            commands,
            (
                Vec2::new(center.x + offset.x, center.y + offset.y),
                bevel_size,
            ),
            bevel_color,
            z,
        );
    }
}

fn spawn_challenge_computer_frame(commands: &mut Commands, text_assets: ThemeTextAssets) {
    spawn_challenge_rect(
        commands,
        challenge_rect(20.0, 520.0, 400.0, 680.0),
        motif_page_color(),
        0.0,
    );
    spawn_challenge_etched_frame_screen(
        commands,
        ChallengeScreenRect::new(16.0, 454.0, 320.0, 583.0),
        (23.0, 137.0),
        0.2,
    );
    spawn_static_challenge_text(
        commands,
        text_assets,
        "Play Computer",
        challenge_screen_world(76.0, 450.0),
        12.0,
        Color::BLACK,
    );
}

fn spawn_challenge_etched_frame_screen(
    commands: &mut Commands,
    rect: ChallengeScreenRect,
    title_gap: (f32, f32),
    z: f32,
) {
    let gap = Some(title_gap);
    spawn_challenge_horizontal_segments(
        commands,
        ChallengeScreenRect::new(rect.x1, rect.y1, rect.x2, rect.y1 + 1.0),
        motif_shadow_color(),
        z,
        gap,
    );
    spawn_challenge_screen_rect(
        commands,
        rect.x1,
        rect.y1,
        rect.x1 + 1.0,
        rect.y2,
        motif_shadow_color(),
        z,
    );
    spawn_challenge_screen_rect(
        commands,
        rect.x1,
        rect.y2 - 1.0,
        rect.x2,
        rect.y2,
        motif_highlight_color(),
        z,
    );
    spawn_challenge_screen_rect(
        commands,
        rect.x2 - 1.0,
        rect.y1,
        rect.x2,
        rect.y2,
        motif_highlight_color(),
        z,
    );

    spawn_challenge_horizontal_segments(
        commands,
        ChallengeScreenRect::new(rect.x1 + 1.0, rect.y1 + 1.0, rect.x2 - 1.0, rect.y1 + 2.0),
        motif_highlight_color(),
        z + 0.1,
        gap,
    );
    spawn_challenge_screen_rect(
        commands,
        rect.x1 + 1.0,
        rect.y1 + 1.0,
        rect.x1 + 2.0,
        rect.y2 - 1.0,
        motif_highlight_color(),
        z + 0.1,
    );
    spawn_challenge_screen_rect(
        commands,
        rect.x1 + 1.0,
        rect.y2 - 2.0,
        rect.x2 - 1.0,
        rect.y2 - 1.0,
        motif_shadow_color(),
        z + 0.1,
    );
    spawn_challenge_screen_rect(
        commands,
        rect.x2 - 2.0,
        rect.y1 + 1.0,
        rect.x2 - 1.0,
        rect.y2 - 1.0,
        motif_shadow_color(),
        z + 0.1,
    );
}

fn spawn_challenge_horizontal_segments(
    commands: &mut Commands,
    rect: ChallengeScreenRect,
    color: Color,
    z: f32,
    gap: Option<(f32, f32)>,
) {
    if let Some((gap_x1, gap_x2)) = gap {
        if gap_x1 > rect.x1 {
            spawn_challenge_screen_rect(commands, rect.x1, rect.y1, gap_x1, rect.y2, color, z);
        }
        if gap_x2 < rect.x2 {
            spawn_challenge_screen_rect(commands, gap_x2, rect.y1, rect.x2, rect.y2, color, z);
        }
    } else {
        spawn_challenge_screen_rect(commands, rect.x1, rect.y1, rect.x2, rect.y2, color, z);
    }
}

fn spawn_challenge_scrollbar(
    commands: &mut Commands,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    vertical: bool,
) {
    let (center, size) = challenge_rect(x1, y1, x2, y2);
    spawn_challenge_panel(
        commands,
        (center, size),
        motif_page_color(),
        2.0,
        MotifBevel::Inset,
    );

    let parts = legacy_scrollbar_parts(center, size, vertical);
    spawn_challenge_panel(
        commands,
        (parts.thumb_center, parts.thumb_size),
        motif_button_face_color(),
        2.2,
        MotifBevel::Inset,
    );

    if vertical {
        spawn_challenge_arrow_button(
            commands,
            parts.leading_arrow_center,
            parts.arrow_size,
            MotifArrowDirection::Up,
            2.4,
        );
        spawn_challenge_arrow_button(
            commands,
            parts.trailing_arrow_center,
            parts.arrow_size,
            MotifArrowDirection::Down,
            2.4,
        );
    } else {
        spawn_challenge_arrow_button(
            commands,
            parts.leading_arrow_center,
            parts.arrow_size,
            MotifArrowDirection::Left,
            2.4,
        );
        spawn_challenge_arrow_button(
            commands,
            parts.trailing_arrow_center,
            parts.arrow_size,
            MotifArrowDirection::Right,
            2.4,
        );
    }
}

fn spawn_challenge_arrow_button(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    direction: MotifArrowDirection,
    z: f32,
) {
    spawn_challenge_panel(
        commands,
        (center, size),
        motif_button_face_color(),
        z,
        MotifBevel::Inset,
    );
    spawn_challenge_arrow_glyph(commands, center, direction, z + 0.5);
}

fn spawn_challenge_arrow_glyph(
    commands: &mut Commands,
    center: Vec2,
    direction: MotifArrowDirection,
    z: f32,
) {
    for index in 0..3 {
        let spread = 1.0 + index as f32 * 2.0;
        let step = index as f32 * 1.6;
        let (offset, size) = match direction {
            MotifArrowDirection::Up => (Vec2::new(0.0, 2.4 - step), Vec2::new(spread, 1.0)),
            MotifArrowDirection::Down => (Vec2::new(0.0, -2.4 + step), Vec2::new(spread, 1.0)),
            MotifArrowDirection::Left => (Vec2::new(-2.4 + step, 0.0), Vec2::new(1.0, spread)),
            MotifArrowDirection::Right => (Vec2::new(2.4 - step, 0.0), Vec2::new(1.0, spread)),
        };
        spawn_challenge_rect(
            commands,
            (Vec2::new(center.x + offset.x, center.y + offset.y), size),
            Color::BLACK,
            z,
        );
    }
}

fn spawn_challenge_checkbox(commands: &mut Commands, rect: (Vec2, Vec2), z: f32) {
    let (center, size) = rect;
    spawn_challenge_rect(commands, (center, size), motif_page_color(), z);
    spawn_challenge_bevel(commands, center, size, z + 0.1, MotifBevel::Inset);
}

fn spawn_challenge_slider(commands: &mut Commands) {
    spawn_challenge_panel(
        commands,
        challenge_screen_rect(30.0, 502.0, 306.0, 516.0),
        motif_page_color(),
        1.0,
        MotifBevel::Inset,
    );
    spawn_challenge_slider_knob(
        commands,
        challenge_screen_world(46.0, 509.0),
        Vec2::new(28.0, 10.0),
    );
}

fn spawn_challenge_slider_knob(commands: &mut Commands, center: Vec2, size: Vec2) {
    spawn_challenge_slider_knob_rect(
        commands,
        center,
        size,
        motif_button_face_color(),
        2.0,
        center.x,
    );
    let (top_left, bottom_right) = (motif_highlight_color(), motif_shadow_color());
    for (offset, bevel_size, bevel_color) in [
        (
            Vec2::new(0.0, size.y / 2.0),
            Vec2::new(size.x, 1.0),
            top_left,
        ),
        (
            Vec2::new(-size.x / 2.0, 0.0),
            Vec2::new(1.0, size.y),
            top_left,
        ),
        (
            Vec2::new(0.0, -size.y / 2.0),
            Vec2::new(size.x, 1.0),
            bottom_right,
        ),
        (
            Vec2::new(size.x / 2.0, 0.0),
            Vec2::new(1.0, size.y),
            bottom_right,
        ),
    ] {
        spawn_challenge_slider_knob_rect(
            commands,
            Vec2::new(center.x + offset.x, center.y + offset.y),
            bevel_size,
            bevel_color,
            2.1,
            center.x,
        );
    }
}

fn spawn_challenge_slider_knob_rect(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    color: Color,
    z: f32,
    base_x: f32,
) {
    commands.spawn((
        Sprite::from_color(color, size),
        Transform::from_xyz(center.x, center.y, z),
        Visibility::Hidden,
        ChallengeSliderKnob {
            x_offset: center.x - base_x,
        },
        ChallengeShell,
        ScreenShell,
    ));
}

fn spawn_challenge_text(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    role: ChallengeTextRole,
    center: Vec2,
    font_size: f32,
    color: Color,
) {
    commands.spawn((
        Text2d::new(""),
        text_assets.font(ThemedTextFontRole::Body, font_size),
        TextColor(color),
        ThemedTextMetrics,
        Transform::from_xyz(center.x, center.y, 4.0),
        Visibility::Hidden,
        ChallengeText { role },
        ChallengeShell,
        ScreenShell,
    ));
}

fn spawn_static_challenge_text(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    text: &'static str,
    center: Vec2,
    font_size: f32,
    color: Color,
) {
    commands.spawn((
        Text2d::new(text),
        text_assets.font(ThemedTextFontRole::Body, font_size),
        TextColor(color),
        ThemedTextMetrics,
        Transform::from_xyz(center.x, center.y, 4.0),
        Visibility::Hidden,
        ChallengeShell,
        ScreenShell,
    ));
}

fn challenge_rect(x1: f32, y1: f32, x2: f32, y2: f32) -> (Vec2, Vec2) {
    let top_left = challenge_point(x1, y1);
    let bottom_right = challenge_point(x2, y2);
    let center = Vec2::new(
        (top_left.x + bottom_right.x) / 2.0 - 320.0,
        300.0 - (top_left.y + bottom_right.y) / 2.0,
    );
    let size = Vec2::new(bottom_right.x - top_left.x, bottom_right.y - top_left.y);
    (center, size)
}

fn challenge_rect_center(x1: f32, y1: f32, x2: f32, y2: f32) -> Vec2 {
    challenge_rect(x1, y1, x2, y2).0
}

fn spawn_challenge_screen_rect(
    commands: &mut Commands,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    color: Color,
    z: f32,
) {
    spawn_challenge_rect(commands, challenge_screen_rect(x1, y1, x2, y2), color, z);
}

fn challenge_screen_rect(x1: f32, y1: f32, x2: f32, y2: f32) -> (Vec2, Vec2) {
    let center = Vec2::new((x1 + x2) / 2.0 - 320.0, 300.0 - (y1 + y2) / 2.0);
    let size = Vec2::new(x2 - x1, y2 - y1);
    (center, size)
}

fn challenge_screen_world(x: f32, y: f32) -> Vec2 {
    Vec2::new(x - 320.0, 300.0 - y)
}

fn challenge_world(x: f32, y: f32) -> Vec2 {
    let point = challenge_point(x, y);
    Vec2::new(point.x - 320.0, 300.0 - point.y)
}

fn challenge_point(x: f32, y: f32) -> Vec2 {
    Vec2::new(x * 0.8, y * 6.0 / 7.0)
}

fn spawn_about_shell(commands: &mut Commands, theme: &LoadedTheme, asset_server: &AssetServer) {
    let text_assets = ThemeTextAssets {
        theme,
        asset_server,
    };
    commands.spawn((
        Sprite::from_color(theme.about.background, Vec2::new(640.0, 600.0)),
        Transform::from_xyz(0.0, 34.0, 0.0),
        Visibility::Hidden,
        ThemedColorSprite {
            role: ThemedColorSpriteRole::AboutBackground,
        },
        AboutShell,
        ScreenShell,
    ));

    for x in [113.0, 527.0] {
        commands.spawn((
            Sprite::from_image(asset_server.load(theme.sprites.crest.clone())),
            about_transform(x, 181.0, 1.0),
            Visibility::Hidden,
            ThemedSprite {
                role: ThemedSpriteRole::AboutIcon,
            },
            AboutShell,
            ScreenShell,
        ));
    }

    spawn_about_text(
        commands,
        text_assets,
        "BattleTris",
        Vec2::new(320.0, 60.0),
        12.0,
        theme.about.title_text,
        ThemedTextColorRole::AboutTitle,
    );
    spawn_about_text(
        commands,
        text_assets,
        "Version 1.0",
        Vec2::new(320.0, 124.0),
        11.0,
        theme.about.title_text,
        ThemedTextColorRole::AboutTitle,
    );
    spawn_about_text(
        commands,
        text_assets,
        "Bryan Cantrill",
        Vec2::new(320.0, 156.0),
        11.0,
        theme.about.name_text,
        ThemedTextColorRole::AboutName,
    );
    spawn_about_text(
        commands,
        text_assets,
        "Charlie Hoecker",
        Vec2::new(320.0, 190.0),
        11.0,
        theme.about.name_text,
        ThemedTextColorRole::AboutName,
    );
    spawn_about_text(
        commands,
        text_assets,
        "Mike Shapiro",
        Vec2::new(320.0, 225.0),
        11.0,
        theme.about.name_text,
        ThemedTextColorRole::AboutName,
    );
    spawn_about_text(
        commands,
        text_assets,
        "battletris@cs.brown.edu",
        Vec2::new(320.0, 261.0),
        11.0,
        theme.about.name_text,
        ThemedTextColorRole::AboutName,
    );

    for (text, y) in [
        (
            "BattleTris Copyright (c) 1993-1997 Bryan Cantrill, Charles Hoecker, Michael Shapiro.",
            306.0,
        ),
        ("Special thanks to:", 328.0),
        (
            "Libby \"Hoss the Camel\" Cantrill, for many ideas and extensive play-testing",
            351.0,
        ),
        ("Drew Davis, for great advice early on", 374.0),
        (
            "Tony, for cleaning up our empty Mountain Dew bottles",
            397.0,
        ),
        (
            "botrytis, pebbles and barney for many long and passionate nights",
            420.0,
        ),
        (
            "The original BT beta testers:  Ben, Caffer, Masi, Dave, Scott and Todd",
            443.0,
        ),
        ("and of course", 466.0),
        ("Kevin \"shouldn't there be a paren there?\" Regan", 489.0),
    ] {
        spawn_about_text(
            commands,
            text_assets,
            text,
            Vec2::new(320.0, y),
            10.0,
            theme.about.credit_text,
            ThemedTextColorRole::AboutCredit,
        );
    }

    spawn_about_button_bevel(commands, theme);
}

fn spawn_about_button_bevel(commands: &mut Commands, theme: &LoadedTheme) {
    let button = theme.layout.rects.about_ok;
    let center = button.center();
    let half = button.size() / 2.0;

    for (offset, size, color) in [
        (
            Vec2::new(0.0, half.y),
            Vec2::new(button.width, 2.0),
            theme.about.button_highlight,
        ),
        (
            Vec2::new(-half.x, 0.0),
            Vec2::new(2.0, button.height),
            theme.about.button_highlight,
        ),
        (
            Vec2::new(0.0, -half.y),
            Vec2::new(button.width, 2.0),
            theme.about.button_shadow,
        ),
        (
            Vec2::new(half.x, 0.0),
            Vec2::new(2.0, button.height),
            theme.about.button_shadow,
        ),
    ] {
        commands.spawn((
            Sprite::from_color(color, size),
            Transform::from_xyz(center.x + offset.x, center.y + offset.y, 3.5),
            Visibility::Hidden,
            ThemedColorSprite {
                role: if offset.x < 0.0 || offset.y > 0.0 {
                    ThemedColorSpriteRole::ButtonHighlight
                } else {
                    ThemedColorSpriteRole::ButtonShadow
                },
            },
            AboutShell,
            ScreenShell,
        ));
    }
}

fn spawn_about_text(
    commands: &mut Commands,
    text_assets: ThemeTextAssets,
    text: &'static str,
    position: Vec2,
    font_size: f32,
    color: Color,
    color_role: ThemedTextColorRole,
) {
    commands.spawn((
        Text2d::new(text),
        text_assets.font(ThemedTextFontRole::Body, font_size),
        TextColor(color),
        ThemedTextColor { role: color_role },
        ThemedTextMetrics,
        about_transform(position.x, position.y, 5.0),
        Visibility::Hidden,
        AboutShell,
        ScreenShell,
    ));
}

fn about_transform(x: f32, y: f32, z: f32) -> Transform {
    Transform::from_xyz(x - 320.0, 334.0 - y, z)
}

#[derive(Debug, Clone, Copy)]
struct MenuButtonSpec {
    screen: ClientScreen,
    label: &'static str,
    center: Vec2,
    size: Vec2,
    action: MenuAction,
}

fn spawn_menu_button(
    commands: &mut Commands,
    theme: &LoadedTheme,
    asset_server: &AssetServer,
    spec: MenuButtonSpec,
) {
    let button_color = if spec.screen == ClientScreen::Startup {
        motif_button_face_color()
    } else if spec.screen == ClientScreen::About {
        theme.about.button_face
    } else if spec.screen == ClientScreen::Challenge || spec.screen == ClientScreen::Roster {
        motif_button_face_color()
    } else {
        theme.button.normal
    };
    let text_color = if spec.screen == ClientScreen::Startup {
        motif_blue_color()
    } else if spec.screen == ClientScreen::About {
        theme.about.button_text
    } else if spec.screen == ClientScreen::Challenge || spec.screen == ClientScreen::Roster {
        motif_blue_color()
    } else {
        theme.button.text
    };
    commands.spawn((
        Sprite::from_color(button_color, spec.size),
        Transform::from_xyz(spec.center.x, spec.center.y, 3.0),
        ButtonFace,
        MenuButton {
            screen: spec.screen,
            rect: Rect::from_center_size(spec.center, spec.size),
            action: spec.action,
        },
        ScreenShell,
    ));
    if spec.screen == ClientScreen::Startup {
        spawn_startup_button_bevel(commands, spec.center, spec.size);
        if matches!(spec.action, MenuAction::GoTo(ClientScreen::Challenge)) {
            spawn_startup_focus_outline(commands, spec.center, spec.size);
        }
    } else if spec.screen == ClientScreen::Challenge {
        spawn_challenge_button_bevel(commands, spec.center, spec.size);
    } else if spec.screen == ClientScreen::Roster {
        spawn_roster_bevel(commands, spec.center, spec.size, 3.5, MotifBevel::Raised);
    }
    let text_font = if spec.screen == ClientScreen::Startup
        || spec.screen == ClientScreen::Challenge
        || spec.screen == ClientScreen::Roster
    {
        themed_text_font_at_size(theme, ThemedTextFontRole::Button, 12.0, asset_server)
    } else {
        themed_text_font(theme, ThemedTextFontRole::Button, asset_server)
    };
    let mut text_entity = commands.spawn((
        Text2d::new(spec.label),
        text_font,
        TextColor(text_color),
        ThemedTextColor {
            role: if spec.screen == ClientScreen::Startup {
                ThemedTextColorRole::ScreenBody
            } else if spec.screen == ClientScreen::About {
                ThemedTextColorRole::AboutButton
            } else if spec.screen == ClientScreen::Challenge || spec.screen == ClientScreen::Roster
            {
                ThemedTextColorRole::ScreenBody
            } else {
                ThemedTextColorRole::Button
            },
        },
        ThemedTextMetrics,
        Transform::from_xyz(spec.center.x, spec.center.y, 4.0),
        MenuButton {
            screen: spec.screen,
            rect: spec.rect(),
            action: spec.action,
        },
        ScreenShell,
    ));
    if spec.screen != ClientScreen::Startup
        && spec.screen != ClientScreen::Challenge
        && spec.screen != ClientScreen::Roster
    {
        text_entity.insert(ThemedTextFont {
            role: ThemedTextFontRole::Button,
        });
    }
}

fn spawn_startup_button_bevel(commands: &mut Commands, center: Vec2, size: Vec2) {
    spawn_startup_bevel(commands, center, size, 3.5, MotifBevel::Raised);
}

fn spawn_startup_focus_outline(commands: &mut Commands, center: Vec2, size: Vec2) {
    let outline_size = size + Vec2::splat(2.0);
    for (offset, segment_size) in [
        (
            Vec2::new(0.0, outline_size.y / 2.0),
            Vec2::new(outline_size.x, 2.0),
        ),
        (
            Vec2::new(-outline_size.x / 2.0, 0.0),
            Vec2::new(2.0, outline_size.y),
        ),
        (
            Vec2::new(0.0, -outline_size.y / 2.0),
            Vec2::new(outline_size.x, 2.0),
        ),
        (
            Vec2::new(outline_size.x / 2.0, 0.0),
            Vec2::new(2.0, outline_size.y),
        ),
    ] {
        commands.spawn((
            Sprite::from_color(motif_blue_color(), segment_size),
            Transform::from_xyz(center.x + offset.x, center.y + offset.y, 3.6),
            StartupOnlyShell,
            ScreenShell,
        ));
    }
}

fn spawn_startup_bevel(
    commands: &mut Commands,
    center: Vec2,
    size: Vec2,
    z: f32,
    bevel: MotifBevel,
) {
    let (top_left, bottom_right) = match bevel {
        MotifBevel::Raised => (motif_highlight_color(), motif_shadow_color()),
        MotifBevel::Inset => (motif_shadow_color(), motif_highlight_color()),
    };
    for (offset, bevel_size, bevel_color) in [
        (
            Vec2::new(0.0, size.y / 2.0),
            Vec2::new(size.x, 2.0),
            top_left,
        ),
        (
            Vec2::new(-size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            top_left,
        ),
        (
            Vec2::new(0.0, -size.y / 2.0),
            Vec2::new(size.x, 2.0),
            bottom_right,
        ),
        (
            Vec2::new(size.x / 2.0, 0.0),
            Vec2::new(2.0, size.y),
            bottom_right,
        ),
    ] {
        commands.spawn((
            Sprite::from_color(bevel_color, bevel_size),
            Transform::from_xyz(center.x + offset.x, center.y + offset.y, z),
            StartupOnlyShell,
            ScreenShell,
        ));
    }
}

impl MenuButtonSpec {
    fn rect(self) -> Rect {
        Rect::from_center_size(self.center, self.size)
    }
}

fn spawn_challenge_button_bevel(commands: &mut Commands, center: Vec2, size: Vec2) {
    spawn_challenge_bevel(commands, center, size, 3.5, MotifBevel::Raised);
}

fn startup_buttons(theme: &LoadedTheme) -> [MenuButtonSpec; 5] {
    let rects = theme.layout.rects;
    [
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "Challenge",
            center: rects.startup_challenge.center(),
            size: rects.startup_challenge.size(),
            action: MenuAction::GoTo(ClientScreen::Challenge),
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "Sleep",
            center: rects.startup_sleep.center(),
            size: rects.startup_sleep.size(),
            action: MenuAction::GoTo(ClientScreen::Sleep),
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "About",
            center: rects.startup_about.center(),
            size: rects.startup_about.size(),
            action: MenuAction::GoTo(ClientScreen::About),
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "Roster",
            center: rects.startup_roster.center(),
            size: rects.startup_roster.size(),
            action: MenuAction::GoTo(ClientScreen::Roster),
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "Quit",
            center: rects.startup_quit.center(),
            size: rects.startup_quit.size(),
            action: MenuAction::Quit,
        },
    ]
}

fn secondary_screen_buttons(theme: &LoadedTheme) -> [MenuButtonSpec; 8] {
    let rects = theme.layout.rects;
    [
        MenuButtonSpec {
            screen: ClientScreen::Challenge,
            label: "Challenge",
            center: rects.challenge_level_down.center(),
            size: rects.challenge_level_down.size(),
            action: MenuAction::GoTo(ClientScreen::Challenge),
        },
        MenuButtonSpec {
            screen: ClientScreen::Challenge,
            label: "Update",
            center: rects.challenge_level_up.center(),
            size: rects.challenge_level_up.size(),
            action: MenuAction::GoTo(ClientScreen::Challenge),
        },
        MenuButtonSpec {
            screen: ClientScreen::Challenge,
            label: "Play Ernie",
            center: rects.challenge_play_ernie.center(),
            size: rects.challenge_play_ernie.size(),
            action: MenuAction::StartHumanVsComputer,
        },
        MenuButtonSpec {
            screen: ClientScreen::Challenge,
            label: "Cancel",
            center: rects.challenge_back.center(),
            size: rects.challenge_back.size(),
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
        MenuButtonSpec {
            screen: ClientScreen::Sleep,
            label: "Wake",
            center: rects.sleep_wake.center(),
            size: rects.sleep_wake.size(),
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
        MenuButtonSpec {
            screen: ClientScreen::About,
            label: "OK",
            center: rects.about_ok.center(),
            size: rects.about_ok.size(),
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
        MenuButtonSpec {
            screen: ClientScreen::Roster,
            label: "Done",
            center: rects.roster_back.center(),
            size: rects.roster_back.size(),
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
        MenuButtonSpec {
            screen: ClientScreen::Settings,
            label: "Back",
            center: rects.settings_back.center(),
            size: rects.settings_back.size(),
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
    ]
}

fn spawn_player_view(
    commands: &mut Commands,
    theme: &LoadedTheme,
    atlas: &ThemeAtlasImageHandle,
    player: PlayerId,
    left: f32,
    label: &str,
) {
    let width = BOARD_WIDTH as f32 * theme.cell.size;
    let height = BOARD_HEIGHT as f32 * theme.cell.size;
    let center_x = left + width / 2.0;
    let center_y = theme.layout.board.top - height / 2.0;

    commands.spawn((
        Sprite::from_color(theme.palette.board_background, Vec2::new(width, height)),
        Transform::from_xyz(center_x, center_y, -1.0),
        PlayerViewEntity { player },
        GameEntity,
    ));

    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            let cell_sprite = empty_cell_sprite(theme);
            commands.spawn((
                board_cell_sprite(theme, atlas, cell_sprite),
                Transform::from_xyz(cell_x(theme, left, x), cell_y(theme, y), 0.0),
                BoardCell { player, x, y },
                PlayerViewEntity { player },
                GameEntity,
            ));
        }
    }

    let _ = (center_x, height, label);
}

#[derive(SystemParam)]
struct KeyboardInputParams<'w> {
    time: Res<'w, Time>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    local: ResMut<'w, LocalGame>,
    settings: ResMut<'w, ClientSettings>,
    settings_edit: ResMut<'w, SettingsEditState>,
    sound: ResMut<'w, SoundEventState>,
    repeat: ResMut<'w, InputRepeatState>,
    recon: ResMut<'w, ReconPanel>,
    bazaar_ui: ResMut<'w, BazaarUiState>,
    capture: Option<Res<'w, VisualCapture>>,
}

fn handle_keyboard_input(mut input: KeyboardInputParams) {
    if input.capture.is_some() {
        return;
    }
    handle_screen_shortcuts(&input.keys, &mut input.settings, &mut input.sound);
    let elapsed_ms = input.time.delta().as_millis().min(u128::from(u64::MAX)) as u64;

    match input.settings.screen {
        ClientScreen::Startup => handle_startup_input(
            &input.keys,
            &mut input.local,
            &mut input.settings,
            &mut input.sound,
        ),
        ClientScreen::Challenge => handle_challenge_input(
            &input.keys,
            &mut input.local,
            &mut input.settings,
            &mut input.sound,
        ),
        ClientScreen::Settings => {
            handle_settings_input(
                &input.keys,
                &mut input.settings,
                &mut input.settings_edit,
                &mut input.sound,
            );
        }
        ClientScreen::Game => handle_game_input(
            &input.keys,
            &mut input.local,
            &input.settings,
            &mut input.repeat,
            &mut input.recon,
            &mut input.bazaar_ui,
            elapsed_ms,
        ),
        ClientScreen::Sleep | ClientScreen::About | ClientScreen::Roster => {}
    }
}

#[derive(SystemParam)]
struct MouseButtonParams<'w, 's> {
    mouse: Res<'w, ButtonInput<MouseButton>>,
    window: Single<'w, 's, &'static Window, With<PrimaryWindow>>,
    buttons: Query<'w, 's, &'static MenuButton>,
    local: ResMut<'w, LocalGame>,
    settings: ResMut<'w, ClientSettings>,
    themes: Res<'w, ThemePacks>,
    sound: ResMut<'w, SoundEventState>,
    bazaar_ui: ResMut<'w, BazaarUiState>,
    app_exit: MessageWriter<'w, AppExit>,
    capture: Option<Res<'w, VisualCapture>>,
}

fn handle_mouse_buttons(mut input: MouseButtonParams) {
    if input.capture.is_some() {
        return;
    }
    if !input.mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = input.window.cursor_position() else {
        return;
    };
    let world = Vec2::new(
        cursor.x - input.window.width() / 2.0,
        input.window.height() / 2.0 - cursor.y,
    );
    if input.settings.screen == ClientScreen::Game && input.local.game.phase() == GamePhase::Bazaar
    {
        let theme = input.themes.get(input.settings.theme);
        handle_bazaar_click(
            world,
            theme,
            &mut input.local.game,
            &mut input.bazaar_ui,
            input.settings.content_mode,
        );
        return;
    }
    let Some(button) = input
        .buttons
        .iter()
        .find(|button| button.screen == input.settings.screen && button.rect.contains(world))
    else {
        return;
    };

    apply_menu_action(
        button.action,
        &mut input.local,
        &mut input.settings,
        &mut input.sound,
        &mut input.app_exit,
    );
}

fn apply_menu_action(
    action: MenuAction,
    local: &mut LocalGame,
    settings: &mut ClientSettings,
    sound: &mut SoundEventState,
    app_exit: &mut MessageWriter<AppExit>,
) {
    match action {
        MenuAction::StartHumanVsComputer => {
            *local = LocalGame::new_human_vs_computer(settings.ernie_level);
            settings.screen = ClientScreen::Game;
            sound.next_log_index = 0;
        }
        MenuAction::GoTo(screen) => settings.screen = screen,
        MenuAction::Quit => {
            app_exit.write(AppExit::Success);
        }
    }
    queue_sound(sound, SoundEvent::MenuAction);
}

fn queue_sound(sound: &mut SoundEventState, event: SoundEvent) {
    sound.last_event = Some(event);
    sound.pending_events.push(event);
}

fn handle_screen_shortcuts(
    keys: &ButtonInput<KeyCode>,
    settings: &mut ClientSettings,
    sound: &mut SoundEventState,
) {
    let target = if keys.just_pressed(KeyCode::F1) {
        Some(ClientScreen::Startup)
    } else if keys.just_pressed(KeyCode::F2) {
        Some(ClientScreen::Challenge)
    } else if keys.just_pressed(KeyCode::F3) {
        Some(ClientScreen::Settings)
    } else if keys.just_pressed(KeyCode::F4) {
        Some(ClientScreen::About)
    } else if keys.just_pressed(KeyCode::F5) {
        Some(ClientScreen::Roster)
    } else if keys.just_pressed(KeyCode::F6) {
        Some(ClientScreen::Sleep)
    } else if keys.just_pressed(KeyCode::Escape) {
        Some(ClientScreen::Game)
    } else {
        None
    };

    if let Some(screen) = target {
        settings.screen = screen;
        queue_sound(sound, SoundEvent::MenuAction);
    }
}

fn handle_startup_input(
    keys: &ButtonInput<KeyCode>,
    local: &mut LocalGame,
    settings: &mut ClientSettings,
    sound: &mut SoundEventState,
) {
    if keys.just_pressed(KeyCode::KeyH) {
        *local = LocalGame::new_human_vs_human();
        settings.screen = ClientScreen::Game;
        sound.next_log_index = 0;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::KeyC) {
        *local = LocalGame::new_human_vs_computer(settings.ernie_level);
        settings.screen = ClientScreen::Game;
        sound.next_log_index = 0;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::KeyT) {
        toggle_theme(settings);
        settings.save();
        queue_sound(sound, SoundEvent::MenuAction);
    }
}

fn handle_challenge_input(
    keys: &ButtonInput<KeyCode>,
    local: &mut LocalGame,
    settings: &mut ClientSettings,
    sound: &mut SoundEventState,
) {
    if keys.just_pressed(KeyCode::Digit1) {
        settings.challenge_mode = ChallengeMode::ComputerOpponent;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        settings.challenge_mode = ChallengeMode::HostDirect;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Digit3) {
        settings.challenge_mode = ChallengeMode::JoinDirect;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Digit4) {
        settings.challenge_mode = ChallengeMode::HostViaLobby;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Digit5) {
        settings.challenge_mode = ChallengeMode::BrowseLobby;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyJ) {
        adjust_ernie_level(settings, -1);
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyL) {
        adjust_ernie_level(settings, 1);
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::KeyC))
        && settings.challenge_mode == ChallengeMode::ComputerOpponent
    {
        *local = LocalGame::new_human_vs_computer(settings.ernie_level);
        settings.screen = ClientScreen::Game;
        sound.next_log_index = 0;
        queue_sound(sound, SoundEvent::MenuAction);
    }
}

fn handle_settings_input(
    keys: &ButtonInput<KeyCode>,
    settings: &mut ClientSettings,
    edit: &mut SettingsEditState,
    sound: &mut SoundEventState,
) {
    let previous = settings.persisted();

    if keys.just_pressed(KeyCode::Tab) {
        edit.field = next_settings_field(edit.field);
        queue_sound(sound, SoundEvent::MenuAction);
    }
    let selected_field = selected_settings_field_key(keys);
    if let Some(field) = selected_field {
        edit.field = field;
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Backspace) || keys.just_pressed(KeyCode::Delete) {
        settings_field_value_mut(settings, edit.field).pop();
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Enter) {
        sanitize_settings_after_edit(settings, edit.field);
        queue_sound(sound, SoundEvent::MenuAction);
    }
    let typed_character = selected_field
        .is_none()
        .then(|| text_entry_character(keys))
        .flatten();
    if let Some(ch) = typed_character {
        settings_field_value_mut(settings, edit.field).push(ch);
        queue_sound(sound, SoundEvent::MenuAction);
    }

    if typed_character.is_none() && keys.just_pressed(KeyCode::KeyT) {
        toggle_theme(settings);
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if typed_character.is_none() && keys.just_pressed(KeyCode::KeyO) {
        settings.sound_pack = match settings.sound_pack {
            SoundPackChoice::GeneratedDefault => SoundPackChoice::Muted,
            SoundPackChoice::Muted => SoundPackChoice::GeneratedDefault,
        };
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if typed_character.is_none() && keys.just_pressed(KeyCode::KeyM) {
        settings.controls = match settings.controls {
            ControlScheme::ModernSplit => ControlScheme::LegacyInspired,
            ControlScheme::LegacyInspired => ControlScheme::ModernSplit,
        };
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Equal) {
        settings.pixel_scale = sanitize_pixel_scale(settings.pixel_scale + 0.25).min(2.0);
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Minus) {
        settings.pixel_scale = sanitize_pixel_scale(settings.pixel_scale - 0.25).max(0.75);
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if typed_character.is_none() && keys.just_pressed(KeyCode::KeyR) {
        settings.hosted_ranked = !settings.hosted_ranked;
        queue_sound(sound, SoundEvent::MenuAction);
    }

    if settings.persisted() != previous {
        settings.save();
    }
}

fn next_settings_field(field: SettingsField) -> SettingsField {
    let index = SettingsField::ALL
        .iter()
        .position(|candidate| *candidate == field)
        .unwrap_or_default();
    SettingsField::ALL[(index + 1) % SettingsField::ALL.len()]
}

fn selected_settings_field_key(keys: &ButtonInput<KeyCode>) -> Option<SettingsField> {
    [
        (KeyCode::Digit1, SettingsField::DisplayName),
        (KeyCode::Digit2, SettingsField::CommunityLabel),
        (KeyCode::Digit3, SettingsField::HostBindAddress),
        (KeyCode::Digit4, SettingsField::ShareAddress),
        (KeyCode::Digit5, SettingsField::JoinAddress),
        (KeyCode::Digit6, SettingsField::LobbyAddress),
    ]
    .into_iter()
    .find_map(|(key, field)| keys.just_pressed(key).then_some(field))
}

fn text_entry_character(keys: &ButtonInput<KeyCode>) -> Option<char> {
    let shifted = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for (key, ch) in text_entry_keys(shifted) {
        if keys.just_pressed(key) {
            return Some(ch);
        }
    }
    None
}

fn text_entry_keys(shifted: bool) -> [(KeyCode, char); 44] {
    [
        (KeyCode::KeyA, if shifted { 'A' } else { 'a' }),
        (KeyCode::KeyB, if shifted { 'B' } else { 'b' }),
        (KeyCode::KeyC, if shifted { 'C' } else { 'c' }),
        (KeyCode::KeyD, if shifted { 'D' } else { 'd' }),
        (KeyCode::KeyE, if shifted { 'E' } else { 'e' }),
        (KeyCode::KeyF, if shifted { 'F' } else { 'f' }),
        (KeyCode::KeyG, if shifted { 'G' } else { 'g' }),
        (KeyCode::KeyH, if shifted { 'H' } else { 'h' }),
        (KeyCode::KeyI, if shifted { 'I' } else { 'i' }),
        (KeyCode::KeyJ, if shifted { 'J' } else { 'j' }),
        (KeyCode::KeyK, if shifted { 'K' } else { 'k' }),
        (KeyCode::KeyL, if shifted { 'L' } else { 'l' }),
        (KeyCode::KeyM, if shifted { 'M' } else { 'm' }),
        (KeyCode::KeyN, if shifted { 'N' } else { 'n' }),
        (KeyCode::KeyO, if shifted { 'O' } else { 'o' }),
        (KeyCode::KeyP, if shifted { 'P' } else { 'p' }),
        (KeyCode::KeyQ, if shifted { 'Q' } else { 'q' }),
        (KeyCode::KeyR, if shifted { 'R' } else { 'r' }),
        (KeyCode::KeyS, if shifted { 'S' } else { 's' }),
        (KeyCode::KeyT, if shifted { 'T' } else { 't' }),
        (KeyCode::KeyU, if shifted { 'U' } else { 'u' }),
        (KeyCode::KeyV, if shifted { 'V' } else { 'v' }),
        (KeyCode::KeyW, if shifted { 'W' } else { 'w' }),
        (KeyCode::KeyX, if shifted { 'X' } else { 'x' }),
        (KeyCode::KeyY, if shifted { 'Y' } else { 'y' }),
        (KeyCode::KeyZ, if shifted { 'Z' } else { 'z' }),
        (KeyCode::Digit0, '0'),
        (KeyCode::Digit1, '1'),
        (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'),
        (KeyCode::Digit4, '4'),
        (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'),
        (KeyCode::Digit7, '7'),
        (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'),
        (KeyCode::Period, '.'),
        (KeyCode::Comma, ','),
        (KeyCode::Minus, '-'),
        (KeyCode::Equal, '='),
        (KeyCode::Slash, '/'),
        (KeyCode::Semicolon, if shifted { ':' } else { ';' }),
        (KeyCode::Space, ' '),
        (KeyCode::Backslash, '\\'),
    ]
}

fn handle_game_input(
    keys: &ButtonInput<KeyCode>,
    local: &mut LocalGame,
    settings: &ClientSettings,
    repeat: &mut InputRepeatState,
    recon: &mut ReconPanel,
    bazaar_ui: &mut BazaarUiState,
    elapsed_ms: u64,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        local.restart();
        return;
    }

    if keys.just_pressed(KeyCode::KeyP) {
        if local.game.phase() == GamePhase::Paused {
            let _ = local.game.resume();
        } else {
            let _ = local.game.pause();
        }
    }

    if keys.just_pressed(KeyCode::KeyQ) {
        local.status_message =
            Some("BattleTris is owned and operated by the legacy crew.".to_string());
    }

    if keys.just_pressed(KeyCode::KeyC) && local.computer.is_some() {
        recon.manual_condor = !recon.manual_condor;
        if !recon.manual_condor {
            recon.snapshot = None;
        }
    }

    if local.game.phase() == GamePhase::Bazaar {
        handle_bazaar_input(keys, &mut local.game, bazaar_ui, settings.content_mode);
        return;
    }

    if local.game.phase() != GamePhase::Playing {
        return;
    }

    for player in [PlayerId::One, PlayerId::Two] {
        if local
            .computer
            .as_ref()
            .is_some_and(|computer| computer.player == player)
        {
            continue;
        }
        send_player_controls(
            keys,
            &mut local.game,
            player,
            settings.controls,
            repeat,
            elapsed_ms,
        );
    }

    for (label, key) in slot_keys() {
        if keys.just_pressed(key) {
            let player = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
                PlayerId::Two
            } else {
                PlayerId::One
            };
            if local
                .computer
                .as_ref()
                .is_none_or(|computer| computer.player != player)
            {
                let _ = local.game.launch_weapon_slot(player, label);
            }
        }
    }
}

fn send_player_controls(
    keys: &ButtonInput<KeyCode>,
    game: &mut TwoPlayerGame,
    player: PlayerId,
    scheme: ControlScheme,
    repeat: &mut InputRepeatState,
    elapsed_ms: u64,
) {
    let controls = controls_for(player, scheme);
    send_repeat_command(
        keys,
        game,
        player,
        controls.left,
        Command::MoveLeft,
        &mut repeat.left[client_player_index(player)],
        elapsed_ms,
    );
    send_repeat_command(
        keys,
        game,
        player,
        controls.right,
        Command::MoveRight,
        &mut repeat.right[client_player_index(player)],
        elapsed_ms,
    );
    send_press_command(
        keys,
        game,
        player,
        controls.rotate_cw,
        Command::RotateClockwise,
    );
    send_press_command(
        keys,
        game,
        player,
        controls.rotate_ccw,
        Command::RotateCounterClockwise,
    );
    send_fast_drop(keys, game, player, controls.fast_drop);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlayerControls {
    left: KeyCode,
    right: KeyCode,
    rotate_cw: KeyCode,
    rotate_ccw: KeyCode,
    fast_drop: KeyCode,
}

fn controls_for(player: PlayerId, scheme: ControlScheme) -> PlayerControls {
    match (scheme, player) {
        (ControlScheme::ModernSplit, PlayerId::One) => PlayerControls {
            left: KeyCode::ArrowLeft,
            right: KeyCode::ArrowRight,
            rotate_cw: KeyCode::ArrowUp,
            rotate_ccw: KeyCode::Slash,
            fast_drop: KeyCode::ArrowDown,
        },
        (ControlScheme::ModernSplit, PlayerId::Two) => PlayerControls {
            left: KeyCode::KeyA,
            right: KeyCode::KeyD,
            rotate_cw: KeyCode::KeyW,
            rotate_ccw: KeyCode::KeyQ,
            fast_drop: KeyCode::KeyS,
        },
        (ControlScheme::LegacyInspired, PlayerId::One) => PlayerControls {
            left: KeyCode::KeyJ,
            right: KeyCode::KeyL,
            rotate_cw: KeyCode::KeyK,
            rotate_ccw: KeyCode::KeyI,
            fast_drop: KeyCode::Space,
        },
        (ControlScheme::LegacyInspired, PlayerId::Two) => PlayerControls {
            left: KeyCode::KeyA,
            right: KeyCode::KeyD,
            rotate_cw: KeyCode::KeyW,
            rotate_ccw: KeyCode::KeyQ,
            fast_drop: KeyCode::KeyS,
        },
    }
}

fn handle_bazaar_input(
    keys: &ButtonInput<KeyCode>,
    game: &mut TwoPlayerGame,
    bazaar_ui: &mut BazaarUiState,
    content_mode: ContentMode,
) {
    if keys.just_pressed(KeyCode::Enter) {
        match game.bazaar_done(PlayerId::One) {
            events if events.is_empty() => {
                bazaar_ui.last_message = UiTextTone::bazaar_waiting_copy(
                    content_mode,
                    BazaarWaitingText::PlayerRepeated(PlayerId::One),
                )
            }
            _ => {
                bazaar_ui.last_message = UiTextTone::bazaar_waiting_copy(
                    content_mode,
                    BazaarWaitingText::PlayerWaiting(PlayerId::One),
                )
            }
        }
    }
    if keys.just_pressed(KeyCode::Space) {
        match game.bazaar_done(PlayerId::Two) {
            events if events.is_empty() => {
                bazaar_ui.last_message = UiTextTone::bazaar_waiting_copy(
                    content_mode,
                    BazaarWaitingText::PlayerRepeated(PlayerId::Two),
                )
            }
            _ => {
                bazaar_ui.last_message = UiTextTone::bazaar_waiting_copy(
                    content_mode,
                    BazaarWaitingText::PlayerWaiting(PlayerId::Two),
                )
            }
        }
    }

    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) {
        bazaar_ui.selected = adjacent_catalog_token(bazaar_ui.selected, -1);
    }
    if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) {
        bazaar_ui.selected = adjacent_catalog_token(bazaar_ui.selected, 1);
    }
    if keys.just_pressed(KeyCode::KeyA) || keys.just_pressed(KeyCode::Equal) {
        buy_selected_bazaar_weapon(game, bazaar_ui, PlayerId::One);
    }
    if keys.just_pressed(KeyCode::KeyX) || keys.just_pressed(KeyCode::Minus) {
        remove_selected_bazaar_weapon(game, bazaar_ui, PlayerId::One);
    }

    for (token, key) in bazaar_catalog_keys() {
        if keys.just_pressed(key) {
            let player = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
                PlayerId::Two
            } else {
                PlayerId::One
            };
            bazaar_ui.selected = token;
            buy_bazaar_weapon(game, bazaar_ui, player, token);
        }
    }
}

fn drive_computer_opponent(
    time: Res<Time>,
    settings: Res<ClientSettings>,
    mut local: ResMut<LocalGame>,
    mut clock: ResMut<ClientTickClock>,
    capture: Option<Res<VisualCapture>>,
) {
    if capture.is_some() {
        return;
    }
    if settings.screen != ClientScreen::Game {
        return;
    }
    clock.computer_elapsed_ms = clock
        .computer_elapsed_ms
        .saturating_add(time.delta().as_millis().min(u128::from(u64::MAX)) as u64);
    let Some(mut computer) = local.computer.take() else {
        return;
    };

    while clock.computer_elapsed_ms >= CLIENT_FIXED_TICK_MS {
        clock.computer_elapsed_ms -= CLIENT_FIXED_TICK_MS;
        match local.game.phase() {
            GamePhase::Playing => {
                computer.reset_for_play();
                drive_computer_play(CLIENT_FIXED_TICK_MS, &mut local.game, &mut computer);
            }
            GamePhase::Bazaar => {
                drive_computer_bazaar(CLIENT_FIXED_TICK_MS, &mut local.game, &mut computer);
            }
            GamePhase::Paused | GamePhase::GameOver => {}
        }
    }

    local.computer = Some(computer);
}

fn drive_computer_play(
    elapsed_ms: u64,
    game: &mut TwoPlayerGame,
    computer: &mut ComputerController,
) {
    if game.phase() != GamePhase::Playing {
        return;
    }

    for label in computer.opponent.launch_slots(
        game.player(computer.player).arsenal(),
        game.player(computer.player).lines(),
        game.player(opponent_player(computer.player)).lines(),
        computer_bazaar_line_value(game, computer.player),
    ) {
        let _ = game.launch_weapon_slot(computer.player, label);
    }

    computer.elapsed_ms = computer.elapsed_ms.saturating_add(elapsed_ms);
    let delay = computer.opponent.difficulty().delay_ms;
    if delay > 0 && computer.elapsed_ms < delay {
        return;
    }
    computer.elapsed_ms = 0;

    if computer.planned.is_empty() {
        computer.planned = game
            .player(computer.player)
            .active_piece()
            .and_then(|_| {
                computer
                    .opponent
                    .choose_placement(game.player(computer.player))
            })
            .map(|placement| {
                computer
                    .opponent
                    .commands_for_placement(game.player(computer.player), &placement)
            })
            .unwrap_or_default();
    }

    let commands_this_frame = if delay == 0 {
        computer.planned.len().max(1)
    } else {
        1
    };
    for _ in 0..commands_this_frame {
        let Some(command) = computer.planned.first().copied() else {
            break;
        };
        computer.planned.remove(0);
        let _ = game.command(computer.player, command);
    }
}

fn drive_computer_bazaar(
    elapsed_ms: u64,
    game: &mut TwoPlayerGame,
    computer: &mut ComputerController,
) {
    if !computer.shopped_this_bazaar {
        let bought = game
            .bazaar_session(computer.player)
            .map(|bazaar| {
                let mut simulated = bazaar.clone();
                computer.opponent.shop(
                    &mut simulated,
                    game.player(computer.player).lines(),
                    game.player(opponent_player(computer.player)).lines(),
                    game.player(computer.player).board(),
                )
            })
            .unwrap_or_default();
        for token in bought {
            let _ = game.bazaar_buy(computer.player, token);
        }
        computer.shopped_this_bazaar = true;
    }

    computer.bazaar_elapsed_ms = computer.bazaar_elapsed_ms.saturating_add(elapsed_ms);
    if computer.bazaar_elapsed_ms >= BAZAAR_LEAVE_DELAY_MS {
        let _ = game.bazaar_done(computer.player);
    }
}

fn tick_game(
    time: Res<Time>,
    settings: Res<ClientSettings>,
    mut local: ResMut<LocalGame>,
    mut clock: ResMut<ClientTickClock>,
    capture: Option<Res<VisualCapture>>,
) {
    if capture.is_some() {
        return;
    }
    if settings.screen != ClientScreen::Game {
        return;
    }
    clock.gameplay_elapsed_ms = clock
        .gameplay_elapsed_ms
        .saturating_add(time.delta().as_millis().min(u128::from(u64::MAX)) as u64);
    if clock.gameplay_elapsed_ms < CLIENT_FIXED_TICK_MS || local.game.phase() != GamePhase::Playing
    {
        return;
    }

    while clock.gameplay_elapsed_ms >= CLIENT_FIXED_TICK_MS {
        clock.gameplay_elapsed_ms -= CLIENT_FIXED_TICK_MS;
        let _ = local.game.tick_player(PlayerId::One, CLIENT_FIXED_TICK_MS);
        let _ = local.game.tick_player(PlayerId::Two, CLIENT_FIXED_TICK_MS);
        if local.game.phase() != GamePhase::Playing {
            break;
        }
    }
}

fn update_recon_panel(mut recon: ResMut<ReconPanel>, local: Res<LocalGame>) {
    for logged in &local.game.event_log()[recon.next_log_index..] {
        match &logged.event {
            BattleEvent::ReconUpdated {
                viewer,
                target,
                snapshot,
            } if *viewer == local.local_player
                && *target == opponent_player(local.local_player) =>
            {
                recon.snapshot = Some(snapshot.clone());
            }
            BattleEvent::ReconDisabled { viewer, target, .. }
                if *viewer == local.local_player
                    && *target == opponent_player(local.local_player) =>
            {
                recon.snapshot = None;
            }
            _ => {}
        }
    }
    recon.next_log_index = local.game.event_log().len();
}

fn collect_sound_events(
    local: Res<LocalGame>,
    settings: Res<ClientSettings>,
    mut sound: ResMut<SoundEventState>,
) {
    if settings.screen != ClientScreen::Game {
        return;
    }
    if settings.sound_pack == SoundPackChoice::Muted {
        sound.next_log_index = local.game.event_log().len();
        sound.last_event = None;
        sound.pending_events.clear();
        return;
    }

    for logged in &local.game.event_log()[sound.next_log_index..] {
        if let Some(event) = sound_event_for(&logged.event) {
            queue_sound(&mut sound, event);
        }
    }
    sound.next_log_index = local.game.event_log().len();
}

fn play_sound_events(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    settings: Res<ClientSettings>,
    sound_packs: Res<SoundPacks>,
    mut sound: ResMut<SoundEventState>,
) {
    if settings.sound_pack == SoundPackChoice::Muted {
        sound.pending_events.clear();
        return;
    }

    for event in std::mem::take(&mut sound.pending_events) {
        let Some(sound_event) =
            sound_packs.sound_for(settings.sound_pack, settings.content_mode, event)
        else {
            continue;
        };
        commands.spawn((
            AudioPlayer::new(asset_server.load(sound_event.file.clone())),
            PlaybackSettings::DESPAWN,
        ));
    }
}

type HudTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static HudText, &'static mut Text2d),
    (Without<PhaseText>, Without<MenuText>, Without<RosterText>),
>;

type PhaseTextSingle<'w, 's> = Single<
    'w,
    's,
    &'static mut Text2d,
    (
        With<PhaseText>,
        Without<HudText>,
        Without<MenuText>,
        Without<RosterText>,
    ),
>;

type MenuTextSingle<'w, 's> = Single<
    'w,
    's,
    &'static mut Text2d,
    (
        With<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<RosterText>,
    ),
>;

type ScreenTextSingle<'w, 's> = Single<
    'w,
    's,
    &'static mut Text2d,
    (
        With<ScreenText>,
        Without<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<RosterText>,
    ),
>;

type BazaarTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static BazaarText, &'static mut Text2d),
    (
        With<BazaarText>,
        Without<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<ScreenText>,
        Without<RosterText>,
    ),
>;

type LegacyGameTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static LegacyGameText, &'static mut Text2d),
    (
        Without<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<ScreenText>,
        Without<BazaarText>,
        Without<ChallengeText>,
        Without<MenuButton>,
        Without<RosterText>,
    ),
>;

type ChallengeTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static ChallengeText, &'static mut Text2d),
    (
        Without<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<ScreenText>,
        Without<BazaarText>,
        Without<MenuButton>,
        Without<RosterText>,
    ),
>;

type RosterTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static RosterText, &'static mut Text2d),
    (
        Without<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<ScreenText>,
        Without<BazaarText>,
        Without<LegacyGameText>,
        Without<ChallengeText>,
        Without<MenuButton>,
    ),
>;

type MenuButtonTextQuery<'w, 's> = Query<
    'w,
    's,
    (&'static MenuButton, &'static mut Text2d),
    (
        Without<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<ScreenText>,
        Without<BazaarText>,
        Without<ChallengeText>,
        Without<RosterText>,
    ),
>;

type ChallengeSliderKnobQuery<'w, 's> =
    Query<'w, 's, (&'static ChallengeSliderKnob, &'static mut Transform), Without<Text2d>>;

type BazaarSelectionMarkerQuery<'w, 's> = Query<
    'w,
    's,
    (&'static BazaarSelectionMarker, &'static mut Transform),
    Without<ChallengeSliderKnob>,
>;

type TextMetricsQuery<'w, 's> =
    Query<'w, 's, (&'static mut LineHeight, &'static mut LetterSpacing), With<ThemedTextMetrics>>;

type ShellVisibilityQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Visibility,
        Option<&'static MenuButton>,
        Option<&'static GenericScreenShell>,
        Option<&'static StartupOnlyShell>,
        Option<&'static AboutShell>,
        Option<&'static ChallengeShell>,
        Option<&'static RosterShell>,
    ),
    (With<ScreenShell>, Without<GameEntity>),
>;

type GameVisibilityQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Visibility,
        Option<&'static PlayerViewEntity>,
        Option<&'static BazaarEntity>,
        Option<&'static PlayingGameEntity>,
    ),
    With<GameEntity>,
>;

#[derive(SystemParam)]
struct RenderGameParams<'w, 's> {
    local: Res<'w, LocalGame>,
    settings: Res<'w, ClientSettings>,
    settings_edit: Res<'w, SettingsEditState>,
    roster: Res<'w, RosterRecords>,
    themes: Res<'w, ThemePacks>,
    atlases: Res<'w, ThemeAtlasHandles>,
    sound: Res<'w, SoundEventState>,
    bazaar_ui: Res<'w, BazaarUiState>,
    clear_color: ResMut<'w, ClearColor>,
    recon: Res<'w, ReconPanel>,
    cells: Query<'w, 's, (&'static BoardCell, &'static mut Sprite)>,
    text_metrics: TextMetricsQuery<'w, 's>,
    hud: HudTextQuery<'w, 's>,
    phase_text: PhaseTextSingle<'w, 's>,
    menu_text: MenuTextSingle<'w, 's>,
    screen_text: ScreenTextSingle<'w, 's>,
    bazaar_text: BazaarTextQuery<'w, 's>,
    legacy_game_text: LegacyGameTextQuery<'w, 's>,
    challenge_text: ChallengeTextQuery<'w, 's>,
    roster_text: RosterTextQuery<'w, 's>,
    menu_button_text: MenuButtonTextQuery<'w, 's>,
    challenge_slider_knob: ChallengeSliderKnobQuery<'w, 's>,
    bazaar_selection_marker: BazaarSelectionMarkerQuery<'w, 's>,
    reported_startup_render: Local<'s, bool>,
}

fn render_game(mut render: RenderGameParams) {
    let theme = render.themes.get(render.settings.theme);
    let atlas = render.atlases.get(
        render.settings.theme,
        render.settings.content_mode,
        &render.themes,
    );
    for (cell, mut sprite) in &mut render.cells {
        let cell_sprite = render_cell_sprite(
            &render.local,
            &render.recon,
            cell.player,
            cell.x,
            cell.y,
            theme,
        );
        sprite.image = atlas.image.clone();
        sprite.texture_atlas = Some(TextureAtlas {
            layout: atlas.layout.clone(),
            index: cell_sprite.atlas_index,
        });
        sprite.color = cell_sprite.tint;
        sprite.custom_size = Some(Vec2::splat(
            ((theme.cell.size - theme.cell.gap) * render.settings.pixel_scale).max(1.0),
        ));
    }

    for (mut line_height, mut letter_spacing) in &mut render.text_metrics {
        *line_height = LineHeight::RelativeToFont(theme.fonts.line_height);
        *letter_spacing = LetterSpacing::Px(theme.fonts.tracking);
    }

    for (hud, mut text) in &mut render.hud {
        text.0 = player_hud(&render.local, &render.recon, hud.player);
    }

    render.phase_text.0 = phase_label(&render.local, &render.settings, &render.sound);
    render.menu_text.0 = menu_label(&render.local.game, &render.settings, &render.settings_edit);
    render.screen_text.0 = screen_body_label(
        &render.local.game,
        &render.settings,
        &render.settings_edit,
        &render.roster,
    );
    for (bazaar_text, mut text) in &mut render.bazaar_text {
        text.0 = bazaar_text_label(
            bazaar_text.role,
            &render.local,
            &render.bazaar_ui,
            render.settings.content_mode,
        );
    }
    for (legacy_text, mut text) in &mut render.legacy_game_text {
        text.0 = legacy_game_text_label(&render.local, &render.settings, legacy_text.role);
    }
    for (challenge_text, mut text) in &mut render.challenge_text {
        text.0 = challenge_label(challenge_text.role, &render.settings);
    }
    for (roster_text, mut text) in &mut render.roster_text {
        text.0 = roster_text_label(&render.roster, roster_text.role);
    }
    for (button, mut text) in &mut render.menu_button_text {
        if button.screen == ClientScreen::Challenge
            && matches!(button.action, MenuAction::StartHumanVsComputer)
        {
            text.0 = format!(
                "Play {} Ernie",
                selected_ernie_difficulty(&render.settings).name
            );
        }
    }
    for (knob, mut transform) in &mut render.challenge_slider_knob {
        transform.translation.x = challenge_ernie_slider_x(&render.settings) + knob.x_offset;
    }
    for (marker, mut transform) in &mut render.bazaar_selection_marker {
        transform.translation.y = bazaar_selection_marker_y(&render.bazaar_ui, marker.role);
    }
    let menu_label_chars =
        render.menu_text.0.chars().count() + render.screen_text.0.chars().count();
    let menu_is_unhealthy = render.settings.screen != ClientScreen::Game
        && render.settings.screen != ClientScreen::Startup
        && menu_label_chars == 0;
    render.clear_color.0 = if menu_is_unhealthy {
        Color::srgb(0.5, 0.0, 0.28)
    } else if render.settings.screen == ClientScreen::Startup {
        Color::BLACK
    } else if render.settings.screen == ClientScreen::About {
        theme.about.background
    } else {
        theme.screen.background
    };

    if !*render.reported_startup_render {
        report_startup_render_health(render.settings.screen, menu_label_chars);
        *render.reported_startup_render = true;
    }
}

#[derive(SystemParam)]
struct ThemeEntityQueries<'w, 's> {
    sprites: Query<'w, 's, (&'static ThemedSprite, &'static mut Sprite)>,
    color_sprites:
        Query<'w, 's, (&'static ThemedColorSprite, &'static mut Sprite), Without<ThemedSprite>>,
    text_colors: Query<'w, 's, (&'static ThemedTextColor, &'static mut TextColor)>,
    text_fonts: Query<'w, 's, (&'static ThemedTextFont, &'static mut TextFont)>,
}

fn update_theme_entities(
    settings: Res<ClientSettings>,
    themes: Res<ThemePacks>,
    asset_server: Res<AssetServer>,
    mut active_theme: Local<Option<(ThemeChoice, ContentMode)>>,
    mut themed: ThemeEntityQueries,
) {
    let active_key = (settings.theme, settings.content_mode);
    if *active_theme == Some(active_key) {
        return;
    }
    *active_theme = Some(active_key);

    let theme = themes.get(settings.theme);
    for (sprite_theme, mut sprite) in &mut themed.sprites {
        sprite.image = asset_server.load(themed_sprite_path(
            theme,
            sprite_theme.role,
            settings.content_mode,
        ));
    }
    for (sprite_theme, mut sprite) in &mut themed.color_sprites {
        sprite.color = themed_sprite_color(theme, sprite_theme.role);
    }
    for (text_theme, mut text_color) in &mut themed.text_colors {
        text_color.0 = themed_text_color(theme, text_theme.role);
    }
    for (font_theme, mut text_font) in &mut themed.text_fonts {
        *text_font = themed_text_font(theme, font_theme.role, &asset_server);
    }
}

fn update_challenge_logo_texture(
    settings: Res<ClientSettings>,
    themes: Res<ThemePacks>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut cache: Local<ChallengeLogoTextureCache>,
    mut logos: Query<&mut Sprite, With<ChallengeLogo>>,
) {
    if logos.is_empty() {
        return;
    }

    let logo = if let Some(handle) = cache.get(settings.theme) {
        handle
    } else {
        let raw_handle: Handle<Image> =
            asset_server.load(themes.get(settings.theme).sprites.biff.clone());
        let processed = images.get(&raw_handle).map(|source| {
            let mut image = source.clone();
            quantize_motif_ppm_image(&mut image);
            image
        });
        if let Some(image) = processed {
            let handle = images.add(image);
            cache.set(settings.theme, handle.clone());
            handle
        } else {
            raw_handle
        }
    };

    for mut sprite in &mut logos {
        sprite.image = logo.clone();
    }
}

fn quantize_motif_ppm_image(image: &mut Image) {
    let Some(data) = image.data.as_mut() else {
        return;
    };
    match image.texture_descriptor.format {
        TextureFormat::Rgba8Unorm
        | TextureFormat::Rgba8UnormSrgb
        | TextureFormat::Bgra8Unorm
        | TextureFormat::Bgra8UnormSrgb => {
            for pixel in data.chunks_exact_mut(4) {
                pixel[0] = quantize_motif_ppm_component(pixel[0]);
                pixel[1] = quantize_motif_ppm_component(pixel[1]);
                pixel[2] = quantize_motif_ppm_component(pixel[2]);
            }
            image.sampler = ImageSampler::nearest();
        }
        _ => {}
    }
}

fn quantize_motif_ppm_component(value: u8) -> u8 {
    let max = u8::MAX as u16;
    let bucket = value as u16 * 4 / max;
    (bucket * max / 4) as u8
}

fn themed_sprite_path(
    theme: &LoadedTheme,
    role: ThemedSpriteRole,
    _content_mode: ContentMode,
) -> String {
    match role {
        ThemedSpriteRole::Startup => theme.sprites.startup.clone(),
        ThemedSpriteRole::Bazaar => theme.sprites.bazaar.clone(),
        ThemedSpriteRole::Biff => theme.sprites.biff.clone(),
        ThemedSpriteRole::AboutIcon => theme.sprites.crest.clone(),
    }
}

fn themed_sprite_color(theme: &LoadedTheme, role: ThemedColorSpriteRole) -> Color {
    match role {
        ThemedColorSpriteRole::ScreenBackground => theme.screen.background,
        ThemedColorSpriteRole::AboutBackground => theme.about.background,
        ThemedColorSpriteRole::ButtonHighlight => theme.about.button_highlight,
        ThemedColorSpriteRole::ButtonShadow => theme.about.button_shadow,
    }
}

fn themed_text_color(theme: &LoadedTheme, role: ThemedTextColorRole) -> Color {
    match role {
        ThemedTextColorRole::Secondary => theme.palette.text_secondary,
        ThemedTextColorRole::ScreenTitle => theme.screen.title_text,
        ThemedTextColorRole::ScreenBody => theme.screen.body_text,
        ThemedTextColorRole::Button => theme.button.text,
        ThemedTextColorRole::AboutTitle => theme.about.title_text,
        ThemedTextColorRole::AboutName => theme.about.name_text,
        ThemedTextColorRole::AboutCredit => theme.about.credit_text,
        ThemedTextColorRole::AboutButton => theme.about.button_text,
    }
}

fn themed_text_font_size(theme: &LoadedTheme, role: ThemedTextFontRole) -> f32 {
    match role {
        ThemedTextFontRole::Title => theme.screen.title_font_size,
        ThemedTextFontRole::Body => theme.screen.body_font_size,
        ThemedTextFontRole::Button => theme.screen.button_font_size,
        ThemedTextFontRole::Mono => theme.screen.body_font_size,
    }
}

#[derive(Debug, Clone, Copy)]
struct ThemeTextAssets<'a> {
    theme: &'a LoadedTheme,
    asset_server: &'a AssetServer,
}

impl ThemeTextAssets<'_> {
    fn font(self, role: ThemedTextFontRole, font_size: f32) -> TextFont {
        themed_text_font_at_size(self.theme, role, font_size, self.asset_server)
    }
}

fn themed_text_font(
    theme: &LoadedTheme,
    role: ThemedTextFontRole,
    asset_server: &AssetServer,
) -> TextFont {
    themed_text_font_at_size(
        theme,
        role,
        themed_text_font_size(theme, role),
        asset_server,
    )
}

fn themed_text_font_at_size(
    theme: &LoadedTheme,
    role: ThemedTextFontRole,
    font_size: f32,
    asset_server: &AssetServer,
) -> TextFont {
    let font = pixel_text_font(font_size);
    if let Some(path) = theme.fonts.path_for(role) {
        font.with_font(asset_server.load(path.to_string()))
    } else {
        font
    }
}

fn pixel_text_font(font_size: f32) -> TextFont {
    TextFont::from_font_size(font_size)
        .with_font_smoothing(FontSmoothing::None)
        .with_font_weight(FontWeight::BOLD)
}

fn update_window_layout(
    settings: Res<ClientSettings>,
    local: Res<LocalGame>,
    recon: Res<ReconPanel>,
    themes: Res<ThemePacks>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    let theme = themes.get(settings.theme);
    let layout = active_window_layout(&settings, &local, &recon, theme);
    let width = layout.width.round().max(1.0);
    let height = layout.height.round().max(1.0);
    if (window.resolution.width() - width).abs() > f32::EPSILON
        || (window.resolution.height() - height).abs() > f32::EPSILON
    {
        window.resolution.set(width, height);
    }
}

fn active_window_layout(
    settings: &ClientSettings,
    local: &LocalGame,
    recon: &ReconPanel,
    theme: &LoadedTheme,
) -> ThemeWindowLayout {
    if settings.screen == ClientScreen::Game {
        if local.game.phase() == GamePhase::Bazaar {
            return theme.layout.screens.bazaar;
        }
        if recon.manual_condor || recon.snapshot.is_some() {
            return theme.layout.screens.game_recon;
        }
    }
    theme.layout.screen(settings.screen)
}

fn update_screen_visibility(
    settings: Res<ClientSettings>,
    local: Res<LocalGame>,
    recon: Res<ReconPanel>,
    mut game_entities: GameVisibilityQuery,
    mut shell_entities: ShellVisibilityQuery,
) {
    let game_visible = settings.screen == ClientScreen::Game;
    let bazaar_visible = game_visible && local.game.phase() == GamePhase::Bazaar;
    for (mut visibility, player_view, bazaar_entity, playing_entity) in &mut game_entities {
        let entity_visible = if bazaar_entity.is_some() {
            bazaar_visible
        } else if bazaar_visible {
            false
        } else if playing_entity.is_some() {
            true
        } else {
            player_view.is_none_or(|view| player_view_visible(&local, &recon, view.player))
        };
        *visibility = if game_visible && entity_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for (
        mut visibility,
        button,
        generic_shell,
        startup_only,
        about_shell,
        challenge_shell,
        roster_shell,
    ) in &mut shell_entities
    {
        let visible = !game_visible
            && if let Some(button) = button {
                button.screen == settings.screen
            } else if challenge_shell.is_some() {
                settings.screen == ClientScreen::Challenge
            } else if about_shell.is_some() {
                settings.screen == ClientScreen::About
            } else if roster_shell.is_some() {
                settings.screen == ClientScreen::Roster
            } else if startup_only.is_some() {
                settings.screen == ClientScreen::Startup
            } else if generic_shell.is_some() {
                settings.screen != ClientScreen::Startup
                    && settings.screen != ClientScreen::About
                    && settings.screen != ClientScreen::Challenge
                    && settings.screen != ClientScreen::Roster
            } else {
                true
            };
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn player_view_visible(local: &LocalGame, recon: &ReconPanel, player: PlayerId) -> bool {
    player == local.local_player
        || local.computer.is_none()
        || (local.computer.is_some() && player == opponent_player(local.local_player))
        || recon.manual_condor
        || recon.snapshot.is_some()
}

fn update_menu_button_visuals(
    settings: Res<ClientSettings>,
    themes: Res<ThemePacks>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut buttons: Query<(&MenuButton, &mut Sprite), With<ButtonFace>>,
) {
    let theme = themes.get(settings.theme);
    let cursor = window.cursor_position().map(|cursor| {
        Vec2::new(
            cursor.x - window.width() / 2.0,
            window.height() / 2.0 - cursor.y,
        )
    });

    for (button, mut sprite) in &mut buttons {
        let hovered = button.screen == settings.screen
            && cursor.is_some_and(|cursor| button.rect.contains(cursor));
        sprite.color = if button.screen == ClientScreen::About {
            if hovered && mouse.pressed(MouseButton::Left) {
                theme.about.button_shadow
            } else if hovered {
                theme.about.button_highlight
            } else {
                theme.about.button_face
            }
        } else if button.screen == ClientScreen::Startup
            || button.screen == ClientScreen::Challenge
            || button.screen == ClientScreen::Roster
        {
            if hovered && mouse.pressed(MouseButton::Left) {
                motif_button_pressed_color()
            } else if hovered {
                motif_button_hover_color()
            } else {
                motif_button_face_color()
            }
        } else if hovered && mouse.pressed(MouseButton::Left) {
            theme.button.pressed
        } else if hovered {
            theme.button.hover
        } else {
            theme.button.normal
        };
    }
}

fn report_startup_render_health(screen: ClientScreen, menu_label_chars: usize) {
    info!("BattleTris render health: screen={screen:?} menu_label_chars={menu_label_chars}");
    if screen != ClientScreen::Game && screen != ClientScreen::Startup && menu_label_chars == 0 {
        error!("BattleTris render health: non-game screen has empty menu text");
    }
}

fn apply_visual_capture_fixture(
    capture: Option<ResMut<VisualCapture>>,
    mut settings: ResMut<ClientSettings>,
    mut local: ResMut<LocalGame>,
    mut recon: ResMut<ReconPanel>,
    mut bazaar_ui: ResMut<BazaarUiState>,
    mut roster: ResMut<RosterRecords>,
) {
    let Some(mut capture) = capture else {
        return;
    };
    if capture.current >= capture.jobs.len() || capture.applied == Some(capture.current) {
        return;
    }

    let job = capture.jobs[capture.current].clone();
    settings.theme = job.theme;
    apply_visual_fixture_state(
        job.fixture,
        &mut settings,
        &mut local,
        &mut recon,
        &mut bazaar_ui,
        &mut roster,
    );
    capture.applied = Some(capture.current);
    capture.frames_until_capture = SMOKE_SCREENSHOT_CAPTURE_DELAY_FRAMES;
    capture.frames_since_request = 0;
    capture.requested = false;
    info!(
        "BattleTris visual fixture applied: fixture={} theme={} output={} expected={}x{}",
        job.fixture.id(),
        job.theme.directory(),
        job.path.display(),
        job.expected_width,
        job.expected_height,
    );
}

fn request_visual_capture(
    mut commands: Commands,
    mut capture: ResMut<VisualCapture>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if capture.current >= capture.jobs.len() {
        app_exit.write(AppExit::Success);
        return;
    }
    if capture.requested {
        capture.frames_since_request = capture.frames_since_request.saturating_add(1);
        if capture.frames_since_request > SMOKE_SCREENSHOT_TIMEOUT_FRAMES {
            let job = &capture.jobs[capture.current];
            error!(
                "BattleTris visual capture timed out: fixture={} path={}",
                job.fixture.id(),
                job.path.display()
            );
            app_exit.write(AppExit::error());
        }
        return;
    }

    if capture.frames_until_capture > 0 {
        capture.frames_until_capture -= 1;
        return;
    }

    let job_index = capture.current;
    let job = capture.jobs[job_index].clone();
    info!(
        "BattleTris visual capture requested: fixture={} theme={} path={}",
        job.fixture.id(),
        job.theme.directory(),
        job.path.display()
    );
    commands.spawn(Screenshot::primary_window()).observe(
        move |screenshot: On<ScreenshotCaptured>,
              mut capture: ResMut<VisualCapture>,
              mut app_exit: MessageWriter<AppExit>| {
            if capture.current != job_index {
                error!(
                    "BattleTris visual capture received stale screenshot: requested={} current={}",
                    job_index, capture.current
                );
                app_exit.write(AppExit::error());
                return;
            }

            match save_visual_capture(&screenshot, &job) {
                Ok((width, height)) => {
                    info!(
                        "BattleTris visual capture saved: fixture={} theme={} path={} size={}x{}",
                        job.fixture.id(),
                        job.theme.directory(),
                        job.path.display(),
                        width,
                        height,
                    );
                    capture.current += 1;
                    capture.applied = None;
                    capture.requested = false;
                    capture.frames_since_request = 0;
                    capture.frames_until_capture = SMOKE_SCREENSHOT_CAPTURE_DELAY_FRAMES;
                    if capture.current >= capture.jobs.len() {
                        app_exit.write(AppExit::Success);
                    }
                }
                Err(error) => {
                    error!(
                        "BattleTris visual capture failed: fixture={} path={} error={error}",
                        job.fixture.id(),
                        job.path.display(),
                    );
                    app_exit.write(AppExit::error());
                }
            }
        },
    );
    capture.requested = true;
}

fn save_visual_capture(
    screenshot: &ScreenshotCaptured,
    job: &VisualCaptureJob,
) -> Result<(u32, u32), String> {
    ensure_parent_dir(&job.path)?;
    let image = screenshot
        .image
        .clone()
        .try_into_dynamic()
        .map_err(|error| format!("captured image could not be decoded: {error}"))?
        .to_rgb8();
    let (width, height) = image.dimensions();
    if width != job.expected_width || height != job.expected_height {
        return Err(format!(
            "captured image dimensions were {width}x{height}, expected {}x{}",
            job.expected_width, job.expected_height
        ));
    }
    validate_visual_capture_pixels(&image, job)?;
    image
        .save(&job.path)
        .map_err(|error| format!("could not save {}: {error}", job.path.display()))?;
    Ok((width, height))
}

fn validate_visual_capture_pixels(
    image: &image::RgbImage,
    job: &VisualCaptureJob,
) -> Result<(), String> {
    let (width, height) = image.dimensions();
    let total_pixels = u64::from(width) * u64::from(height);
    let mut bright_pixels = 0_u64;
    let mut min_luma = u8::MAX;
    let mut max_luma = u8::MIN;

    for pixel in image.pixels() {
        let [red, green, blue] = pixel.0;
        let luma = ((u32::from(red) * 2126 + u32::from(green) * 7152 + u32::from(blue) * 722)
            / 10_000) as u8;
        if luma > 80 {
            bright_pixels += 1;
        }
        min_luma = min_luma.min(luma);
        max_luma = max_luma.max(luma);
    }

    if bright_pixels <= total_pixels / 1_000 || max_luma.saturating_sub(min_luma) <= 40 {
        return Err(format!(
            "captured image looks blank for fixture={} theme={}: bright_pixels={bright_pixels} total_pixels={total_pixels} luma_range={min_luma}..{max_luma}",
            job.fixture.id(),
            job.theme.directory(),
        ));
    }
    Ok(())
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))
}

fn send_press_command(
    keys: &ButtonInput<KeyCode>,
    game: &mut TwoPlayerGame,
    player: PlayerId,
    key: KeyCode,
    command: Command,
) {
    if keys.just_pressed(key) {
        let _ = game.command(player, command);
    }
}

fn send_repeat_command(
    keys: &ButtonInput<KeyCode>,
    game: &mut TwoPlayerGame,
    player: PlayerId,
    key: KeyCode,
    command: Command,
    repeat: &mut HeldKeyRepeat,
    elapsed_ms: u64,
) {
    let (next, emit) = repeat.observe(keys.pressed(key), keys.just_pressed(key), elapsed_ms);
    *repeat = next;
    if emit {
        let _ = game.command(player, command);
    }
}

fn send_fast_drop(
    keys: &ButtonInput<KeyCode>,
    game: &mut TwoPlayerGame,
    player: PlayerId,
    key: KeyCode,
) {
    let command = if keys.pressed(key) {
        Command::StartFastDrop
    } else {
        Command::StopFastDrop
    };
    let _ = game.command(player, command);
}

#[derive(Debug, Clone, Copy)]
struct RenderedCellSprite {
    atlas_index: usize,
    tint: Color,
}

fn board_cell_sprite(
    theme: &LoadedTheme,
    atlas: &ThemeAtlasImageHandle,
    cell_sprite: RenderedCellSprite,
) -> Sprite {
    let mut sprite = Sprite::from_atlas_image(
        atlas.image.clone(),
        TextureAtlas {
            layout: atlas.layout.clone(),
            index: cell_sprite.atlas_index,
        },
    );
    sprite.color = cell_sprite.tint;
    sprite.custom_size = Some(Vec2::splat((theme.cell.size - theme.cell.gap).max(1.0)));
    sprite
}

fn render_cell_sprite(
    local: &LocalGame,
    recon: &ReconPanel,
    player: PlayerId,
    x: usize,
    y: usize,
    theme: &LoadedTheme,
) -> RenderedCellSprite {
    if player != local.local_player && local.computer.is_some() {
        if recon.manual_condor {
            return local
                .game
                .player(player)
                .board()
                .get(Coord { x, y })
                .map_or_else(
                    || empty_cell_sprite(theme),
                    |cell| cell_sprite(cell, false, theme),
                );
        }
        if let Some(snapshot) = &recon.snapshot {
            return snapshot
                .board
                .cells
                .get(y * snapshot.board.width + x)
                .copied()
                .flatten()
                .map_or_else(
                    || empty_cell_sprite(theme),
                    |cell| cell_sprite(cell, false, theme),
                );
        }
        return empty_cell_sprite(theme);
    }

    let piece_cell = local
        .game
        .player(player)
        .active_piece()
        .and_then(|piece| {
            piece
                .cells()
                .into_iter()
                .find(|((px, py), _)| *px == x as isize && *py == y as isize)
        })
        .map(|(_, cell)| cell);

    if let Some(cell) = piece_cell {
        return cell_sprite(cell, true, theme);
    }

    let Some(coord) = Coord::new(x, y) else {
        return empty_cell_sprite(theme);
    };
    local.game.player(player).board().get(coord).map_or_else(
        || empty_cell_sprite(theme),
        |cell| cell_sprite(cell, false, theme),
    )
}

fn player_hud(local: &LocalGame, recon: &ReconPanel, player: PlayerId) -> String {
    let game = &local.game;
    if player != local.local_player && local.computer.is_some() {
        return recon_hud(game, recon, player);
    }

    let loop_state = game.player(player);
    let mut text = format!(
        "score {}  funds {}  lines {}  bazaar in {}\nnext {}\narsenal {}\neffects {}",
        loop_state.score(),
        loop_state.funds(),
        loop_state.lines(),
        game.lines_until_bazaar(),
        piece_label(loop_state.next_piece_kind_preview()),
        arsenal_label(game, player),
        active_effects_label(game, player),
    );

    if let Some(bazaar) = game.bazaar_session(player) {
        let _ = write!(
            text,
            "\nbazaar funds {}\n{}",
            bazaar.staged_funds(),
            bazaar_catalog_label(bazaar)
        );
        let _ = write!(
            text,
            "\nstaged {}",
            arsenal_slots_label(bazaar.staged_arsenal())
        );
    }

    text
}

fn recon_hud(game: &TwoPlayerGame, recon: &ReconPanel, player: PlayerId) -> String {
    if recon.manual_condor {
        return format!(
            "Condor recon\nopponent score {}  funds {}  lines {}",
            game.player(player).score(),
            game.player(player).funds(),
            game.player(player).lines()
        );
    }
    if let Some(snapshot) = &recon.snapshot {
        return format!(
            "{:?} recon snapshot\nopponent funds {}  lines {}",
            snapshot.level,
            snapshot.funds,
            game.player(player).lines()
        );
    }
    "opponent hidden\nuse Ames/Ace/Condor or press C for Condor in computer mode".to_string()
}

fn phase_label(_local: &LocalGame, settings: &ClientSettings, _sound: &SoundEventState) -> String {
    if settings.screen != ClientScreen::Game {
        return String::new();
    }
    String::new()
}

fn legacy_game_text_label(
    local: &LocalGame,
    settings: &ClientSettings,
    role: LegacyGameTextRole,
) -> String {
    if settings.screen != ClientScreen::Game || local.game.phase() == GamePhase::Bazaar {
        return String::new();
    }
    match role {
        LegacyGameTextRole::Score => legacy_score_label(local),
        LegacyGameTextRole::ArsenalSlot(slot) => legacy_arsenal_slot_label(local, slot),
        LegacyGameTextRole::Message => legacy_game_message_label(local, settings.content_mode),
    }
}

fn legacy_score_label(local: &LocalGame) -> String {
    let game = &local.game;
    let player = local.local_player;
    let opponent = opponent_player(player);
    let own = game.player(player);
    let other = game.player(opponent);
    format!(
        "Your score:          {}\nOpponent's score:    {}\n\nYour lines:          {}\nOpponent's lines:    {}\n\nYour funds:          {}\nOpponent's funds:    {}\n\nLines 'til bazaar:   {}",
        own.score(),
        other.score(),
        own.lines(),
        other.lines(),
        own.funds(),
        other.funds(),
        game.lines_until_bazaar(),
    )
}

fn legacy_arsenal_slot_label(local: &LocalGame, slot: usize) -> String {
    let label = if slot == 9 { 0 } else { slot + 1 };
    let Some(slot) = local
        .game
        .player(local.local_player)
        .arsenal()
        .slots()
        .get(slot)
        .copied()
        .flatten()
    else {
        return format!(" {label}.  < Empty >");
    };
    let suffix = if slot.quantity > 1 {
        format!(" ({})", slot.quantity)
    } else {
        String::new()
    };
    format!(" {label}.  {}{suffix}", weapon_spec(slot.token).name)
}

fn legacy_game_message_label(local: &LocalGame, content_mode: ContentMode) -> String {
    let game = &local.game;
    if let Some(message) = latest_weapon_feedback(game) {
        return message;
    }
    if let Some(message) = &local.status_message {
        return message.clone();
    }
    match game.phase() {
        GamePhase::Playing => match game.mode() {
            GameMode::HumanVsComputer { difficulty, .. } => {
                format!("Playing {} Ernie", difficulty.name)
            }
            GameMode::HumanVsHuman => "Playing local game".to_string(),
        },
        GamePhase::Paused => "Paused...".to_string(),
        GamePhase::Bazaar => String::new(),
        GamePhase::GameOver => UiTextTone::game_result_copy(
            content_mode,
            local_game_result_for(local.game.event_log(), local.local_player),
        )
        .to_string(),
    }
}

fn menu_label(
    _game: &TwoPlayerGame,
    settings: &ClientSettings,
    settings_edit: &SettingsEditState,
) -> String {
    match settings.screen {
        ClientScreen::Startup => String::new(),
        ClientScreen::Game => String::new(),
        ClientScreen::Challenge => "Challenge".to_string(),
        ClientScreen::Sleep => "Sleep".to_string(),
        ClientScreen::About => "About BattleTris".to_string(),
        ClientScreen::Roster => String::new(),
        ClientScreen::Settings => format!(
            "Settings\nEditing {}: {}\nTab/1-6 choose  Backspace edit  Enter sanitize",
            settings_edit.field.label(),
            settings_field_value(settings, settings_edit.field),
        ),
    }
}

fn screen_body_label(
    _game: &TwoPlayerGame,
    settings: &ClientSettings,
    settings_edit: &SettingsEditState,
    _roster: &RosterRecords,
) -> String {
    match settings.screen {
        ClientScreen::Startup => String::new(),
        ClientScreen::Challenge => {
            let difficulty = selected_ernie_difficulty(settings);
            let lobby_preview = lobby_registration_preview(settings);
            format!(
                "Challenge\nMode: {}\n1 Computer Opponent  2 Host Direct  3 Join Direct\n4 Host Via Lobby  5 Browse Lobby\n\nIdentity: {} ({}) on community {}\nHost bind: {}\nShare address: {}\nJoin address: {}\nLobby: {}  Hosted ranked: {}\nProtocol v{}.{} ({}, {})\n\nDirect IP needs inbound TCP allowed on the host port.\nPeers must reach each other directly; there is no NAT traversal or relay.\n\nErnie level {} of {}  {}  {}ms delay\nUse J/Left and L/Right or Level -/+ as the slider.\nPress Enter/C only in Computer mode for unranked computer play.",
                settings.challenge_mode.label(),
                lobby_preview.player.display_name,
                lobby_preview.player.player_id,
                settings.community_label,
                settings.direct_listen_addr,
                settings.direct_share_addr,
                settings.direct_join_addr,
                settings.lobby_addr,
                settings.hosted_ranked,
                PROTOCOL_MAJOR,
                PROTOCOL_MINOR,
                CAPABILITY_DIRECT_TCP,
                CAPABILITY_SELF_HOSTED_LOBBY,
                difficulty.level,
                COMPUTER_DIFFICULTIES.len() - 1,
                difficulty.name,
                difficulty.delay_ms,
            )
        }
        ClientScreen::Sleep => {
            format!(
                "Sleep\n{} is marked available for BattleTris challenges.\nBiff is standing by on {} for direct TCP protocol v{}.{}.\nClick Wake to return to Startup.",
                settings.display_name,
                settings.direct_listen_addr,
                PROTOCOL_MAJOR,
                PROTOCOL_MINOR,
            )
        }
        ClientScreen::About => {
            "BattleTris\nVersion 1.0\nBryan Cantrill\nCharlie Hoecker\nMike Shapiro\nbattletris@cs.brown.edu\nBattleTris Copyright (c) 1993-1997 Bryan Cantrill, Charles Hoecker, Michael Shapiro.\nSpecial thanks to:\nLibby \"Hoss the Camel\" Cantrill, for many ideas and extensive play-testing\nDrew Davis, for great advice early on\nTony, for cleaning up our empty Mountain Dew bottles\nbotrytis, pebbles and barney for many long and passionate nights\nThe original BT beta testers:  Ben, Caffer, Masi, Dave, Scott and Todd\nand of course\nKevin \"shouldn't there be a paren there?\" Regan"
                .to_string()
        }
        ClientScreen::Roster => " ".to_string(),
        ClientScreen::Settings => format!(
            "Theme: {:?}  Sound: {:?}  Controls: {}  Scale: {:.2}x\nHosted ranked preference: {}\n\n{}1 display name: {}\n{}2 community: {}\n{}3 host bind: {}\n{}4 share address: {}\n{}5 join address: {}\n{}6 lobby address: {}\n\nBind controls where hosting listens. Share is what another player types or what a lobby advertises.\n0.0.0.0 is allowed for bind but never kept as a share address.\nSuggested share address: {}\nHost must allow inbound TCP on the direct port. No NAT traversal or gameplay relay exists.\n\nT theme  O sound  M controls  R hosted ranked  -/= scale\nProtocol: v{}.{}\nAssets: {}\nSettings: {}",
            settings.theme,
            settings.sound_pack,
            controls_label(settings.controls),
            settings.pixel_scale,
            settings.hosted_ranked,
            selected_settings_marker(settings_edit, SettingsField::DisplayName),
            settings.display_name,
            selected_settings_marker(settings_edit, SettingsField::CommunityLabel),
            settings.community_label,
            selected_settings_marker(settings_edit, SettingsField::HostBindAddress),
            settings.direct_listen_addr,
            selected_settings_marker(settings_edit, SettingsField::ShareAddress),
            settings.direct_share_addr,
            selected_settings_marker(settings_edit, SettingsField::JoinAddress),
            settings.direct_join_addr,
            selected_settings_marker(settings_edit, SettingsField::LobbyAddress),
            settings.lobby_addr,
            suggested_share_addr_for(&settings.direct_listen_addr),
            PROTOCOL_MAJOR,
            PROTOCOL_MINOR,
            settings.assets_dir.display(),
            settings
                .settings_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "unavailable".to_string()),
        ),
        ClientScreen::Game => String::new(),
    }
}

fn selected_settings_marker(edit: &SettingsEditState, field: SettingsField) -> &'static str {
    if edit.field == field {
        "> "
    } else {
        "  "
    }
}

fn challenge_label(role: ChallengeTextRole, settings: &ClientSettings) -> String {
    match role {
        ChallengeTextRole::UserList | ChallengeTextRole::ComputerStatus => String::new(),
        ChallengeTextRole::UserInfo => {
            UiTextTone::challenge_copy(settings.content_mode).to_string()
        }
    }
}

fn challenge_ernie_slider_x(settings: &ClientSettings) -> f32 {
    let max_level = (COMPUTER_DIFFICULTIES.len() - 1).max(1) as f32;
    let fraction = settings.ernie_level as f32 / max_level;
    challenge_screen_world(46.0 + 244.0 * fraction, 509.0).x
}

fn bazaar_selection_marker_y(ui: &BazaarUiState, role: BazaarSelectionMarkerRole) -> f32 {
    let rows = sorted_weapon_catalog();
    let index = rows
        .iter()
        .position(|spec| spec.token == ui.selected)
        .unwrap_or_default() as f32;
    let legacy_y = match role {
        BazaarSelectionMarkerRole::Background => 210.0,
        BazaarSelectionMarkerRole::Text => 205.0,
    } + index * 16.2;
    bazaar_world(0.0, legacy_y).y
}

fn sanitize_pixel_scale(pixel_scale: f32) -> f32 {
    if pixel_scale.is_finite() {
        pixel_scale.clamp(0.75, 2.0)
    } else {
        1.0
    }
}

fn sanitize_nonempty_setting(value: String, fallback: String) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        trimmed.to_string()
    }
}

fn sanitize_socket_setting(value: String, fallback: &str) -> String {
    let trimmed = sanitize_nonempty_setting(value, fallback.to_string());
    if trimmed.parse::<SocketAddr>().is_ok() {
        trimmed
    } else {
        fallback.to_string()
    }
}

fn sanitize_share_addr_setting(value: String, bind_addr: &str) -> String {
    let fallback = suggested_share_addr_for(bind_addr);
    let sanitized = sanitize_socket_setting(value, &fallback);
    if socket_addr_is_unspecified(&sanitized) {
        fallback
    } else {
        sanitized
    }
}

fn socket_addr_is_unspecified(value: &str) -> bool {
    value
        .parse::<SocketAddr>()
        .map(|addr| addr.ip().is_unspecified())
        .unwrap_or(false)
}

fn suggested_share_addr_for(bind_addr: &str) -> String {
    let bind = bind_addr
        .parse::<SocketAddr>()
        .unwrap_or_else(|_| SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4405));
    if !bind.ip().is_unspecified() {
        return bind.to_string();
    }
    SocketAddr::new(suggest_lan_ip(), bind.port()).to_string()
}

fn suggest_lan_ip() -> IpAddr {
    UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))
        .and_then(|socket| {
            let _ = socket.connect(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 80));
            socket.local_addr()
        })
        .map(|addr| addr.ip())
        .ok()
        .filter(|ip| !ip.is_unspecified())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

fn settings_field_value(settings: &ClientSettings, field: SettingsField) -> &str {
    match field {
        SettingsField::DisplayName => &settings.display_name,
        SettingsField::CommunityLabel => &settings.community_label,
        SettingsField::HostBindAddress => &settings.direct_listen_addr,
        SettingsField::ShareAddress => &settings.direct_share_addr,
        SettingsField::JoinAddress => &settings.direct_join_addr,
        SettingsField::LobbyAddress => &settings.lobby_addr,
    }
}

fn settings_field_value_mut(settings: &mut ClientSettings, field: SettingsField) -> &mut String {
    match field {
        SettingsField::DisplayName => &mut settings.display_name,
        SettingsField::CommunityLabel => &mut settings.community_label,
        SettingsField::HostBindAddress => &mut settings.direct_listen_addr,
        SettingsField::ShareAddress => &mut settings.direct_share_addr,
        SettingsField::JoinAddress => &mut settings.direct_join_addr,
        SettingsField::LobbyAddress => &mut settings.lobby_addr,
    }
}

fn sanitize_settings_after_edit(settings: &mut ClientSettings, field: SettingsField) {
    match field {
        SettingsField::DisplayName => {
            settings.display_name = sanitize_nonempty_setting(
                std::mem::take(&mut settings.display_name),
                default_display_name(),
            );
        }
        SettingsField::CommunityLabel => {
            settings.community_label = sanitize_nonempty_setting(
                std::mem::take(&mut settings.community_label),
                "local".to_string(),
            );
        }
        SettingsField::HostBindAddress => {
            settings.direct_listen_addr = sanitize_socket_setting(
                std::mem::take(&mut settings.direct_listen_addr),
                "0.0.0.0:4405",
            );
            if socket_addr_is_unspecified(&settings.direct_share_addr) {
                settings.direct_share_addr = suggested_share_addr_for(&settings.direct_listen_addr);
            }
        }
        SettingsField::ShareAddress => {
            settings.direct_share_addr = sanitize_share_addr_setting(
                std::mem::take(&mut settings.direct_share_addr),
                &settings.direct_listen_addr,
            );
        }
        SettingsField::JoinAddress => {
            settings.direct_join_addr = sanitize_socket_setting(
                std::mem::take(&mut settings.direct_join_addr),
                "127.0.0.1:4405",
            );
        }
        SettingsField::LobbyAddress => {
            settings.lobby_addr =
                sanitize_socket_setting(std::mem::take(&mut settings.lobby_addr), "127.0.0.1:4404");
        }
    }
}

fn sanitize_ernie_level(level: usize) -> usize {
    level.min(COMPUTER_DIFFICULTIES.len() - 1)
}

fn adjust_ernie_level(settings: &mut ClientSettings, step: isize) {
    let max = COMPUTER_DIFFICULTIES.len() as isize - 1;
    settings.ernie_level = (settings.ernie_level as isize + step).clamp(0, max) as usize;
    settings.save();
}

fn toggle_theme(settings: &mut ClientSettings) {
    settings.theme = match settings.theme {
        ThemeChoice::Original => ThemeChoice::HighContrast,
        ThemeChoice::HighContrast => ThemeChoice::Original,
    };
}

fn selected_ernie_difficulty(settings: &ClientSettings) -> battletris_core::ai::ComputerDifficulty {
    computer_difficulty(settings.ernie_level).expect("sanitized legacy AI difficulty exists")
}

fn default_display_name() -> String {
    std::env::var("BATTLETRIS_DISPLAY_NAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| std::env::var("USER").ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Local Player".to_string())
}

fn lobby_registration_preview(settings: &ClientSettings) -> LobbyRegister {
    LobbyRegister {
        player: HostedPlayer {
            player_id: player_id_from_display_name(&settings.display_name),
            display_name: settings.display_name.clone(),
        },
        direct_addr: settings.direct_share_addr.clone(),
        ranked: settings.hosted_ranked,
    }
}

fn player_id_from_display_name(display_name: &str) -> String {
    let mut id = String::new();
    for ch in display_name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
        } else if (ch.is_ascii_whitespace() || ch == '-' || ch == '_') && !id.ends_with('-') {
            id.push('-');
        }
    }
    let id = id.trim_matches('-');
    if id.is_empty() {
        "local-player".to_string()
    } else {
        id.to_string()
    }
}

fn computer_bazaar_line_value(game: &TwoPlayerGame, computer: PlayerId) -> u32 {
    game.player(computer)
        .lines()
        .saturating_add(game.player(opponent_player(computer)).lines())
}

fn settings_path() -> Option<PathBuf> {
    ProjectDirs::from("org", "BattleTris", "BattleTris")
        .map(|dirs| dirs.config_dir().join(SETTINGS_FILE_NAME))
}

fn assets_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("BATTLETRIS_ASSETS_DIR") {
        return PathBuf::from(path);
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(package_root) = exe_path.parent().and_then(|bin_dir| bin_dir.parent()) {
            let packaged_assets = package_root.join("assets");
            if packaged_assets.join("manifest.toml").is_file() {
                return packaged_assets;
            }
        }
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("assets")
}

fn roster_text_label(roster: &RosterRecords, role: RosterTextRole) -> String {
    match role {
        RosterTextRole::UserList => roster_user_list_label(roster),
        RosterTextRole::UserInfo1 => roster_user_info_label(roster, 0),
        RosterTextRole::UserInfo2 => " ".to_string(),
        RosterTextRole::Player1Name => roster_player_name_label(roster, 0),
        RosterTextRole::Player2Name => " ".to_string(),
        RosterTextRole::Player1Score | RosterTextRole::Player2Score => " ".to_string(),
    }
}

fn roster_user_list_label(roster: &RosterRecords) -> String {
    if let Some(error) = &roster.error {
        return format!("Records unavailable\n{}", truncate_label(error, 22));
    }
    if roster.rows.is_empty() {
        return "No records".to_string();
    }

    roster
        .rows
        .iter()
        .take(20)
        .map(|row| truncate_label(&row.player_key, 20))
        .collect::<Vec<_>>()
        .join("\n")
}

fn roster_user_info_label(roster: &RosterRecords, index: usize) -> String {
    if let Some(error) = &roster.error {
        return format!("Records unavailable:\n{}", truncate_label(error, 34));
    }
    let Some(row) = roster.rows.get(index) else {
        return if index == 0 {
            "No ranked human-vs-human\nresults have been recorded.\n\nNickname: none\nPlan: none"
                .to_string()
        } else {
            " ".to_string()
        };
    };

    format!(
        "          Name: {}\n          Rank: {}\n          Wins: {}\n        Losses: {}\n Highest score: {}\n Highest lines: {}\n Highest funds: {}\n        Streak: {}\n  Fastest kill: {}\nQuickest death: {}\n  Longest game: {}\n\nNickname: none\nPlan: none",
        truncate_label(&row.display_name, 20),
        row.rank,
        row.wins,
        row.losses,
        row.high_score,
        row.high_lines,
        row.high_funds,
        row.streak,
        roster_duration_label(row.fastest_kill_secs),
        roster_duration_label(row.quickest_death_secs),
        roster_duration_label(row.longest_game_secs),
    )
}

fn roster_duration_label(secs: Option<u64>) -> String {
    let Some(secs) = secs else {
        return "None".to_string();
    };
    let hours = (secs / 3600).min(99);
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn roster_player_name_label(roster: &RosterRecords, index: usize) -> String {
    roster
        .rows
        .get(index)
        .map(|row| truncate_label(&row.player_key, 14))
        .unwrap_or_else(|| " ".to_string())
}

fn streak_label(kind: StreakKind, count: u64) -> String {
    match kind {
        StreakKind::None => "0 wins".to_string(),
        StreakKind::Wins => format!("{count} win{}", if count == 1 { "" } else { "s" }),
        StreakKind::Losses => format!("{count} loss{}", if count == 1 { "" } else { "es" }),
    }
}

fn truncate_label(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!(
            "{}~",
            truncated
                .chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
        )
    } else {
        truncated
    }
}

fn controls_label(scheme: ControlScheme) -> &'static str {
    match scheme {
        ControlScheme::ModernSplit => "modern split (P1 arrows+/; P2 WASD+Q)",
        ControlScheme::LegacyInspired => "legacy inspired (P1 J/L/K/I+Space; P2 WASD+Q)",
    }
}

fn active_effects_label(game: &TwoPlayerGame, player: PlayerId) -> String {
    let mut labels = Vec::new();
    let effects = game.player(player).active_effects();
    for spec in WEAPON_CATALOG {
        let remaining = effects.remaining_lines(spec.token);
        if remaining > 0 {
            labels.push(format!("{}:{remaining}", short_weapon_name(spec.token)));
        }
    }

    if labels.is_empty() {
        "none".to_string()
    } else {
        labels.join(", ")
    }
}

fn latest_weapon_feedback(game: &TwoPlayerGame) -> Option<String> {
    game.event_log()
        .iter()
        .rev()
        .find_map(|logged| match &logged.event {
            BattleEvent::WeaponLaunched {
                launcher,
                target,
                token,
            } => Some(format!(
                "{:?} launched {} at {:?}",
                launcher,
                weapon_spec(*token).name,
                target,
            )),
            BattleEvent::OneShotWeaponApplied {
                launcher,
                target,
                token,
            } => Some(format!(
                "{} from {:?} hit {:?}",
                weapon_spec(*token).name,
                launcher,
                target,
            )),
            BattleEvent::TimedWeaponActivated {
                launcher,
                target,
                token,
                remaining_lines,
            } => Some(format!(
                "{} active on {:?} for {} lines from {:?}",
                weapon_spec(*token).name,
                target,
                remaining_lines,
                launcher,
            )),
            BattleEvent::TimedWeaponExpired { player, token } => Some(format!(
                "{} expired for {:?}",
                weapon_spec(*token).name,
                player,
            )),
            BattleEvent::IncomingWeaponQueued {
                launcher,
                target,
                token,
            } => Some(format!(
                "{} incoming from {:?} to {:?}",
                weapon_spec(*token).name,
                launcher,
                target,
            )),
            BattleEvent::WeaponReflected { player, token } => Some(format!(
                "Mirror reflected {} back onto {:?}",
                weapon_spec(*token).name,
                player,
            )),
            BattleEvent::WeaponNullified { player, token } => Some(format!(
                "Mirror nullified {} for {:?}",
                weapon_spec(*token).name,
                player,
            )),
            _ => None,
        })
}

fn arsenal_label(game: &TwoPlayerGame, player: PlayerId) -> String {
    arsenal_slots_label(game.player(player).arsenal())
}

fn arsenal_slots_label(arsenal: &battletris_core::weapons::Arsenal) -> String {
    let labels = arsenal
        .slots()
        .iter()
        .enumerate()
        .filter_map(|(index, slot)| {
            slot.map(|slot| {
                let label = if index == 9 {
                    "0".to_string()
                } else {
                    (index + 1).to_string()
                };
                format!(
                    "{label}:{}x{}",
                    short_weapon_name(slot.token),
                    slot.quantity
                )
            })
        })
        .collect::<Vec<_>>();

    if labels.is_empty() {
        "empty".to_string()
    } else {
        labels.join(" ")
    }
}

fn bazaar_catalog_label(bazaar: &battletris_core::weapons::Bazaar) -> String {
    bazaar_catalog_keys()
        .into_iter()
        .map(|(token, key)| {
            format!(
                "{} {:?} ${}",
                short_weapon_name(token),
                key,
                bazaar.price(token)
            )
        })
        .collect::<Vec<_>>()
        .join("  ")
}

fn bazaar_text_label(
    role: BazaarTextRole,
    local: &LocalGame,
    ui: &BazaarUiState,
    content_mode: ContentMode,
) -> String {
    if local.game.phase() != GamePhase::Bazaar {
        return String::new();
    }

    let Some(bazaar) = local.game.bazaar_session(local.local_player) else {
        return "Bazaar closed".to_string();
    };
    match role {
        BazaarTextRole::Catalog => bazaar_catalog_widget_label(bazaar),
        BazaarTextRole::SelectedCatalogRow => weapon_spec(ui.selected).name.to_string(),
        BazaarTextRole::Funds => {
            if bazaar.carter_prices() {
                format!("{}\nCarter prices", bazaar.staged_funds())
            } else {
                bazaar.staged_funds().to_string()
            }
        }
        BazaarTextRole::ArsenalSlot(slot) => bazaar_arsenal_slot_widget_label(bazaar, ui, slot),
        BazaarTextRole::Message => bazaar_message_widget_label(local, ui, content_mode),
        BazaarTextRole::Description => bazaar_description_widget_label(bazaar, ui.selected),
    }
}

fn bazaar_catalog_widget_label(bazaar: &battletris_core::weapons::Bazaar) -> String {
    sorted_weapon_catalog()
        .into_iter()
        .map(|spec| {
            let price_marker = if bazaar.price(spec.token) <= bazaar.staged_funds() {
                ""
            } else {
                " *"
            };
            format!("{}{price_marker}", spec.name)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn bazaar_arsenal_slot_widget_label(
    bazaar: &battletris_core::weapons::Bazaar,
    ui: &BazaarUiState,
    slot: usize,
) -> String {
    if let Some(visual_arsenal) = &ui.visual_arsenal {
        return visual_arsenal[slot]
            .map(|token| weapon_spec(token).name.to_string())
            .unwrap_or_else(|| "< Empty >".to_string());
    }
    let Some(slot) = bazaar.staged_arsenal().slots().get(slot).copied().flatten() else {
        return "< Empty >".to_string();
    };
    if slot.quantity > 1 {
        format!("{} ({})", weapon_spec(slot.token).name, slot.quantity)
    } else {
        weapon_spec(slot.token).name.to_string()
    }
}

fn bazaar_message_widget_label(
    local: &LocalGame,
    ui: &BazaarUiState,
    content_mode: ContentMode,
) -> String {
    if local.game.bazaar_player_done(local.local_player) {
        wrap_bazaar_description(UiTextTone::bazaar_done_overlay_copy(content_mode), 34)
    } else if ui.last_message.trim().is_empty() {
        wrap_bazaar_description(UiTextTone::bazaar_instructions_copy(content_mode), 34)
    } else {
        ui.last_message.clone()
    }
}

fn bazaar_description_widget_label(
    bazaar: &battletris_core::weapons::Bazaar,
    selected: WeaponToken,
) -> String {
    let spec = weapon_spec(selected);
    let mut text = format!(
        "Price:    {}\nDuration: {} lines\n\n",
        bazaar.price(spec.token),
        spec.line_duration,
    );
    text.push_str(&wrap_bazaar_description(spec.description, 37));
    text
}

fn wrap_bazaar_description(description: &str, width: usize) -> String {
    let mut output = String::new();
    let mut line_len = 0_usize;
    for word in description.split_whitespace() {
        let word_len = word.chars().count();
        if line_len > 0 && line_len + 1 + word_len > width {
            output.push('\n');
            line_len = 0;
        } else if line_len > 0 {
            output.push(' ');
            line_len += 1;
        }
        output.push_str(word);
        line_len += word_len;
    }
    output
}

fn sorted_weapon_catalog() -> Vec<&'static battletris_core::weapons::WeaponSpec> {
    let mut rows = WEAPON_CATALOG.iter().collect::<Vec<_>>();
    rows.sort_by_key(|spec| (spec.price, spec.token.legacy_id()));
    rows
}

fn short_weapon_name(token: WeaponToken) -> &'static str {
    match token {
        WeaponToken::FearedWeird => "Weird",
        WeaponToken::FourByFour => "4x4",
        WeaponToken::Hatter => "Hatter",
        WeaponToken::Upbyside => "Upside",
        WeaponToken::FallOut => "Fallout",
        WeaponToken::Swap => "Swap",
        WeaponToken::Lawyers => "Lawyer",
        WeaponToken::RiseUp => "Rise",
        WeaponToken::FlipOut => "Flip",
        WeaponToken::Speedy => "Speedy",
        WeaponToken::Missing => "Miss",
        WeaponToken::PieceIt => "Piece",
        WeaponToken::Blind => "Blind",
        WeaponToken::Mondale => "Tax",
        WeaponToken::Keating => "Keating",
        WeaponToken::Carter => "Carter",
        WeaponToken::Reagan => "Reagan",
        WeaponToken::Ames => "Ames",
        WeaponToken::Ace => "Ace",
        WeaponToken::Condor => "Condor",
        WeaponToken::NiceDay => "Nice",
        WeaponToken::SoLong => "NoLong",
        WeaponToken::NoDice => "NoDice",
        WeaponToken::Bug => "Bug",
        WeaponToken::Bottle => "Bottle",
        WeaponToken::NoSlide => "NoSlide",
        WeaponToken::Susan => "Susan",
        WeaponToken::Meadow => "Meadow",
        WeaponToken::Mirror => "Mirror",
        WeaponToken::Twilight => "Twilight",
        WeaponToken::Slick => "Slick",
        WeaponToken::Broken => "Broken",
        WeaponToken::Force => "Force",
        WeaponToken::Gimp => "Gimp",
    }
}

fn piece_label(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::El => "L",
        PieceKind::ReverseEl => "reverse L",
        PieceKind::SlantRight => "slant right",
        PieceKind::SlantLeft => "slant left",
        PieceKind::Long => "long",
        PieceKind::Plug => "plug",
        PieceKind::Box => "box",
        PieceKind::Die => "die",
        PieceKind::Happy => "happy",
        PieceKind::Dog => "dog",
        PieceKind::ReverseDog => "reverse dog",
        PieceKind::Cap => "cap",
        PieceKind::Wall => "wall",
        PieceKind::Tower => "tower",
        PieceKind::Star => "star",
        PieceKind::WeirdLong => "weird long",
        PieceKind::FourByFour => "four-by-four",
        PieceKind::LongDong => "long dong",
    }
}

fn cell_sprite(cell: Cell, _active: bool, theme: &LoadedTheme) -> RenderedCellSprite {
    match cell {
        Cell::Visible { color } => {
            let index = usize::from(color.get().saturating_sub(1))
                % theme.palette.visible_colors.len().max(1);
            RenderedCellSprite {
                atlas_index: theme.cell_atlas.cells.visible_colors[index],
                tint: theme
                    .palette
                    .visible_colors
                    .get(index)
                    .copied()
                    .unwrap_or(theme.palette.text_accent),
            }
        }
        Cell::Structure => RenderedCellSprite {
            atlas_index: theme.cell_atlas.cells.structure,
            tint: theme.palette.structure,
        },
        Cell::Happy => RenderedCellSprite {
            atlas_index: theme.cell_atlas.cells.happy,
            tint: theme.palette.happy,
        },
        Cell::Frown => RenderedCellSprite {
            atlas_index: theme.cell_atlas.cells.frown,
            tint: theme.palette.frown,
        },
        Cell::Gimp { .. } => RenderedCellSprite {
            atlas_index: theme.cell_atlas.cells.gimp,
            tint: theme.palette.gimp,
        },
        Cell::Die { pip } => RenderedCellSprite {
            atlas_index: theme.cell_atlas.cells.die[usize::from(pip.get() - 1)],
            tint: theme.palette.die,
        },
        Cell::Invisible => RenderedCellSprite {
            atlas_index: theme.cell_atlas.cells.invisible,
            tint: theme.palette.invisible,
        },
        Cell::Hidden { .. } => RenderedCellSprite {
            atlas_index: theme.cell_atlas.cells.hidden,
            tint: theme.palette.hidden,
        },
    }
}

fn empty_cell_sprite(theme: &LoadedTheme) -> RenderedCellSprite {
    RenderedCellSprite {
        atlas_index: theme.cell_atlas.cells.empty,
        tint: theme.palette.empty,
    }
}

fn sound_event_for(event: &BattleEvent) -> Option<SoundEvent> {
    match event {
        BattleEvent::PlayerEvent {
            event: CoreEvent::PieceLocked { .. },
            ..
        } => Some(SoundEvent::PieceLocked),
        BattleEvent::PlayerEvent {
            event: CoreEvent::LinesCleared { .. },
            ..
        } => Some(SoundEvent::LineClear),
        BattleEvent::PlayerEvent {
            event: CoreEvent::SpawnFailed { .. } | CoreEvent::HappyMissed { .. },
            ..
        } => Some(SoundEvent::Warning),
        BattleEvent::BazaarEntered => Some(SoundEvent::BazaarEntered),
        BattleEvent::BazaarPlayerDone { .. } | BattleEvent::BazaarLeft => {
            Some(SoundEvent::Purchase)
        }
        BattleEvent::WeaponLaunched {
            token: WeaponToken::Gimp,
            ..
        } => Some(SoundEvent::WeaponLaunchGimp),
        BattleEvent::WeaponLaunched { .. }
        | BattleEvent::OneShotWeaponApplied { .. }
        | BattleEvent::TimedWeaponActivated { .. }
        | BattleEvent::WeaponReflected { .. }
        | BattleEvent::WeaponNullified { .. } => Some(SoundEvent::WeaponLaunch),
        BattleEvent::TimedWeaponExpired { .. } => Some(SoundEvent::Purchase),
        BattleEvent::PlayerDied { .. } => Some(SoundEvent::GameDead),
        BattleEvent::GameOver { .. } => Some(SoundEvent::GameOver),
        BattleEvent::Paused | BattleEvent::Resumed => Some(SoundEvent::MenuAction),
        _ => None,
    }
}

fn local_game_result_for(
    log: &[battletris_core::game::LoggedEvent],
    local_player: PlayerId,
) -> Option<bool> {
    log.iter().rev().find_map(|logged| match logged.event {
        BattleEvent::GameOver { winner, loser }
            if winner == local_player || loser == local_player =>
        {
            Some(winner == local_player)
        }
        _ => None,
    })
}

const fn player_label(player: PlayerId) -> &'static str {
    match player {
        PlayerId::One => "Player 1",
        PlayerId::Two => "Player 2",
    }
}

fn cell_x(theme: &LoadedTheme, left: f32, x: usize) -> f32 {
    left + x as f32 * theme.cell.size + theme.cell.size / 2.0
}

fn cell_y(theme: &LoadedTheme, y: usize) -> f32 {
    theme.layout.board.top - y as f32 * theme.cell.size - theme.cell.size / 2.0
}

const fn opponent_player(player: PlayerId) -> PlayerId {
    match player {
        PlayerId::One => PlayerId::Two,
        PlayerId::Two => PlayerId::One,
    }
}

const fn client_player_index(player: PlayerId) -> usize {
    match player {
        PlayerId::One => 0,
        PlayerId::Two => 1,
    }
}

fn slot_keys() -> [(u8, KeyCode); 10] {
    [
        (1, KeyCode::Digit1),
        (2, KeyCode::Digit2),
        (3, KeyCode::Digit3),
        (4, KeyCode::Digit4),
        (5, KeyCode::Digit5),
        (6, KeyCode::Digit6),
        (7, KeyCode::Digit7),
        (8, KeyCode::Digit8),
        (9, KeyCode::Digit9),
        (0, KeyCode::Digit0),
    ]
}

fn bazaar_catalog_keys() -> [(WeaponToken, KeyCode); 10] {
    [
        (WeaponToken::FlipOut, KeyCode::Digit1),
        (WeaponToken::Gimp, KeyCode::Digit2),
        (WeaponToken::Missing, KeyCode::Digit3),
        (WeaponToken::NiceDay, KeyCode::Digit4),
        (WeaponToken::RiseUp, KeyCode::Digit5),
        (WeaponToken::PieceIt, KeyCode::Digit6),
        (WeaponToken::Ace, KeyCode::Digit7),
        (WeaponToken::SoLong, KeyCode::Digit8),
        (WeaponToken::Upbyside, KeyCode::Digit9),
        (WeaponToken::NoSlide, KeyCode::Digit0),
    ]
}

fn handle_bazaar_click(
    world: Vec2,
    theme: &LoadedTheme,
    game: &mut TwoPlayerGame,
    ui: &mut BazaarUiState,
    content_mode: ContentMode,
) {
    let player = PlayerId::One;
    if let Some(token) = bazaar_catalog_token_at(world, theme) {
        ui.selected = token;
        ui.last_message = format!("Selected {}.", weapon_spec(token).name);
        return;
    }
    if bazaar_add_rect(theme).contains(world) {
        buy_selected_bazaar_weapon(game, ui, player);
        return;
    }
    if bazaar_remove_rect(theme).contains(world) {
        remove_selected_bazaar_weapon(game, ui, player);
        return;
    }
    if bazaar_done_rect(theme).contains(world) {
        match game.bazaar_done(player) {
            events if events.is_empty() => {
                ui.last_message =
                    UiTextTone::bazaar_waiting_copy(content_mode, BazaarWaitingText::LocalRepeated)
            }
            _ => {
                ui.last_message =
                    UiTextTone::bazaar_waiting_copy(content_mode, BazaarWaitingText::LocalWaiting)
            }
        }
        return;
    }
    if let Some(token) = bazaar_arsenal_token_at(world, theme, game, player) {
        ui.selected = token;
        remove_selected_bazaar_weapon(game, ui, player);
    }
}

fn buy_selected_bazaar_weapon(game: &mut TwoPlayerGame, ui: &mut BazaarUiState, player: PlayerId) {
    buy_bazaar_weapon(game, ui, player, ui.selected);
}

fn buy_bazaar_weapon(
    game: &mut TwoPlayerGame,
    ui: &mut BazaarUiState,
    player: PlayerId,
    token: WeaponToken,
) {
    match game.bazaar_buy(player, token) {
        Ok(index) => {
            ui.last_message = format!(
                "Added {} to slot {}.",
                weapon_spec(token).name,
                arsenal_slot_label(index),
            );
        }
        Err(error) => {
            ui.last_message = format!("Could not add {}: {error:?}.", weapon_spec(token).name);
        }
    }
}

fn remove_selected_bazaar_weapon(
    game: &mut TwoPlayerGame,
    ui: &mut BazaarUiState,
    player: PlayerId,
) {
    let token = ui.selected;
    match game.bazaar_remove_staged(player, token) {
        Ok(()) => {
            ui.last_message = format!(
                "Removed staged {} and refunded its entry price.",
                weapon_spec(token).name
            );
        }
        Err(error) => {
            ui.last_message = format!(
                "Could not remove {}: only newly staged purchases can be refunded ({error:?}).",
                weapon_spec(token).name,
            );
        }
    }
}

fn adjacent_catalog_token(current: WeaponToken, step: isize) -> WeaponToken {
    let rows = sorted_weapon_catalog();
    let index = rows
        .iter()
        .position(|spec| spec.token == current)
        .unwrap_or_default() as isize;
    let next = (index + step).rem_euclid(rows.len() as isize) as usize;
    rows[next].token
}

fn bazaar_catalog_token_at(world: Vec2, theme: &LoadedTheme) -> Option<WeaponToken> {
    let rows = sorted_weapon_catalog();
    let rect = theme.layout.rects.bazaar_catalog.rect();
    if !rect.contains(world) {
        return None;
    }
    let row_height = rect.height() / rows.len() as f32;
    let row = ((rect.max.y - world.y) / row_height).floor() as usize;
    rows.get(row).map(|spec| spec.token)
}

fn bazaar_arsenal_token_at(
    world: Vec2,
    theme: &LoadedTheme,
    game: &TwoPlayerGame,
    player: PlayerId,
) -> Option<WeaponToken> {
    let rect = theme.layout.rects.bazaar_arsenal.rect();
    if !rect.contains(world) {
        return None;
    }
    let slot_width = rect.width() / 10.0;
    let index = ((world.x - rect.min.x) / slot_width).floor() as usize;
    game.bazaar_session(player)
        .and_then(|bazaar| bazaar.staged_arsenal().slots().get(index))
        .copied()
        .flatten()
        .map(|slot| slot.token)
}

fn bazaar_add_rect(theme: &LoadedTheme) -> Rect {
    theme.layout.rects.bazaar_add.rect()
}

fn bazaar_remove_rect(theme: &LoadedTheme) -> Rect {
    theme.layout.rects.bazaar_remove.rect()
}

fn bazaar_done_rect(theme: &LoadedTheme) -> Rect {
    theme.layout.rects.bazaar_done.rect()
}

fn arsenal_slot_label(index: usize) -> String {
    if index == 9 {
        "0".to_string()
    } else {
        (index + 1).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_piece_preview_does_not_advance_core_state() {
        let game = TwoPlayerGame::new(GameSeed::from_u64(1), GameSeed::from_u64(2));
        let first = game.player(PlayerId::One).next_piece_kind_preview();
        let second = game.player(PlayerId::One).next_piece_kind_preview();

        assert_eq!(first, second);
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
        let long = ClientRunConfig::parse(vec![OsString::from("--rated")], None)
            .expect("rated CLI parses");
        let short = ClientRunConfig::parse(vec![OsString::from("-r")], None)
            .expect("legacy rated CLI parses");

        assert_eq!(long.content_mode, ContentMode::Rated);
        assert_eq!(short.content_mode, ContentMode::Rated);
        assert!(!long.deterministic_capture);
        assert!(!short.deterministic_capture);
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
    fn controls_can_switch_between_modern_and_legacy_layouts() {
        assert_eq!(
            controls_for(PlayerId::One, ControlScheme::ModernSplit).left,
            KeyCode::ArrowLeft
        );
        assert_eq!(
            controls_for(PlayerId::One, ControlScheme::LegacyInspired).left,
            KeyCode::KeyJ
        );
        assert_eq!(
            controls_for(PlayerId::One, ControlScheme::LegacyInspired).fast_drop,
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
            theme: ThemeChoice::HighContrast,
            sound_pack: SoundPackChoice::Muted,
            controls: ControlScheme::LegacyInspired,
            pixel_scale: 1.5,
            ernie_level: 12,
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
        assert!(encoded.contains("legacy-inspired"));
        assert!(encoded.contains("Ada"));
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
}
