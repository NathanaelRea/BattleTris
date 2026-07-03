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
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::PrimaryWindow;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    ffi::{OsStr, OsString},
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

const SMOKE_SCREENSHOT_CAPTURE_DELAY_FRAMES: u16 = 30;
const SMOKE_SCREENSHOT_TIMEOUT_FRAMES: u16 = 300;
const SETTINGS_FILE_NAME: &str = "settings.toml";
const CLIENT_FIXED_TICK_MS: u64 = 10;
const INPUT_REPEAT_INITIAL_MS: u64 = 150;
const INPUT_REPEAT_MS: u64 = 50;
const DEFAULT_ERNIE_LEVEL: usize = 7;

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
    let window = themes.get(settings.theme).layout.window;
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
    app.insert_resource(ClearColor(themes.get(settings.theme).palette.background))
        .insert_resource(local_game)
        .insert_resource(ClientTickClock::default())
        .insert_resource(InputRepeatState::default())
        .insert_resource(recon_panel)
        .insert_resource(bazaar_ui)
        .insert_resource(themes)
        .insert_resource(sound_packs)
        .insert_resource(settings)
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
        .add_systems(Startup, setup)
        .add_systems(Update, apply_visual_capture_fixture.before(render_game))
        .add_systems(
            Update,
            (
                handle_keyboard_input,
                handle_mouse_buttons,
                drive_computer_opponent,
                tick_game,
                update_recon_panel,
                collect_sound_events,
                play_sound_events,
                render_game,
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
    OriginalInspired,
    HighContrast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SoundPackChoice {
    GeneratedDefault,
    Muted,
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
            Self::OriginalInspired => "original-inspired",
            Self::HighContrast => "high-contrast",
        }
    }

    fn from_id(value: &str) -> Option<Self> {
        [Self::OriginalInspired, Self::HighContrast]
            .into_iter()
            .find(|choice| choice.directory() == value)
    }
}

#[derive(Debug, Clone)]
struct ClientRunConfig {
    capture: Option<VisualCaptureSpec>,
    deterministic_capture: bool,
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
        if args.is_empty() {
            return Ok(Self {
                capture: smoke_env.map(|path| VisualCaptureSpec::Smoke { path: path.into() }),
                deterministic_capture: false,
            });
        }

        if args
            .first()
            .is_some_and(|arg| arg == OsStr::new("headless"))
        {
            return parse_headless_args(&args[1..]);
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
        })
    }
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
    GameRecon,
    BoardCells,
}

impl VisualFixture {
    const ALL: [Self; 10] = [
        Self::Startup,
        Self::Challenge,
        Self::Sleep,
        Self::About,
        Self::Roster,
        Self::Settings,
        Self::GamePlaying,
        Self::GameBazaar,
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
            Self::GamePlaying | Self::GameBazaar | Self::GameRecon | Self::BoardCells => {
                ClientScreen::Game
            }
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
    let window = themes.get(theme).layout.window;
    VisualCaptureJob {
        fixture,
        theme,
        path,
        expected_width: window.width.round() as u32,
        expected_height: window.height.round() as u32,
    }
}

fn parse_headless_args(args: &[OsString]) -> Result<ClientRunConfig, String> {
    let Some(command) = args.first() else {
        return Err("headless requires a command: capture or capture-all".to_string());
    };
    if is_help_arg(command) {
        return Err(client_usage());
    }

    match command.to_str() {
        Some("capture") => parse_headless_capture_args(&args[1..]),
        Some("capture-all") => parse_headless_capture_all_args(&args[1..]),
        Some(other) => Err(format!("unrecognized headless command: {other}")),
        None => Err(format!(
            "headless command is not valid UTF-8: {}",
            display_arg(command)
        )),
    }
}

fn parse_headless_capture_args(args: &[OsString]) -> Result<ClientRunConfig, String> {
    let mut fixture = None;
    let mut theme = ThemeChoice::OriginalInspired;
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
    })
}

fn parse_headless_capture_all_args(args: &[OsString]) -> Result<ClientRunConfig, String> {
    let mut theme = ThemeChoice::OriginalInspired;
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
    ThemeChoice::from_id(value).ok_or_else(|| {
        format!("unknown theme '{value}'; expected original-inspired or high-contrast")
    })
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
        "Usage:\n  client\n  client --smoke-screenshot <path>\n  client headless capture --fixture <fixture> --theme <theme> --output <path>\n  client headless capture-all --theme <theme> --out-dir <dir>\n\nFixtures: {}\nThemes: original-inspired, high-contrast",
        visual_fixture_list()
    )
}

#[derive(Resource, Debug, Clone)]
struct SoundPacks {
    generated_default: LoadedSoundPack,
}

impl SoundPacks {
    fn load(assets_dir: &std::path::Path) -> Self {
        Self {
            generated_default: LoadedSoundPack::load(assets_dir, SoundPackChoice::GeneratedDefault),
        }
    }

    fn sound_for(&self, choice: SoundPackChoice, event: SoundEvent) -> Option<&LoadedSoundEvent> {
        match choice {
            SoundPackChoice::GeneratedDefault => self.generated_default.event(event),
            SoundPackChoice::Muted => None,
        }
    }
}

#[derive(Debug, Clone)]
struct LoadedSoundPack {
    events: Vec<LoadedSoundEvent>,
}

impl LoadedSoundPack {
    fn load(assets_dir: &std::path::Path, choice: SoundPackChoice) -> Self {
        let sound_dir = assets_dir.join("sounds").join(choice.directory());
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
        raw.validate(&sound_dir, &manifest_path);
        let prefix = format!("sounds/{}/", choice.directory());
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
    fn validate(&self, sound_dir: &std::path::Path, manifest_path: &std::path::Path) {
        if self.kind != "sound-pack" || self.format_version != 1 {
            panic!(
                "BattleTris sound-pack manifest {} has unsupported kind/version: kind={} format_version={}",
                manifest_path.display(),
                self.kind,
                self.format_version
            );
        }
        for expected in SoundEvent::ALL {
            if !self.event.iter().any(|event| event.id == expected.id()) {
                panic!(
                    "BattleTris sound-pack manifest {} is missing event {}",
                    manifest_path.display(),
                    expected.id()
                );
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
            }
        }
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
    original_inspired: LoadedTheme,
    high_contrast: LoadedTheme,
}

impl ThemePacks {
    fn load(assets_dir: &std::path::Path) -> Self {
        Self {
            original_inspired: LoadedTheme::load(assets_dir, ThemeChoice::OriginalInspired),
            high_contrast: LoadedTheme::load(assets_dir, ThemeChoice::HighContrast),
        }
    }

    const fn get(&self, choice: ThemeChoice) -> &LoadedTheme {
        match choice {
            ThemeChoice::OriginalInspired => &self.original_inspired,
            ThemeChoice::HighContrast => &self.high_contrast,
        }
    }
}

#[derive(Debug, Clone)]
struct LoadedTheme {
    sprites: LoadedThemeSprites,
    cell: ThemeCell,
    layout: ThemeLayout,
    palette: ThemePalette,
    button: ThemeButtonStyle,
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
            cell: raw.cell,
            layout: raw.layout,
            palette: raw.palette.into_palette(&manifest_path),
            button: raw.button.into_style(&manifest_path),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawTheme {
    name: String,
    kind: String,
    format_version: u32,
    sprites: ThemeSprites,
    fonts: ThemeFonts,
    cell: ThemeCell,
    layout: ThemeLayout,
    palette: RawThemePalette,
    button: RawThemeButtonStyle,
}

impl RawTheme {
    fn validate(&self, theme_dir: &std::path::Path, manifest_path: &std::path::Path) {
        if self.kind != "theme" || self.format_version != 1 {
            panic!(
                "BattleTris theme manifest {} has unsupported kind/version: kind={} format_version={}",
                manifest_path.display(),
                self.kind,
                self.format_version
            );
        }
        if self.name.is_empty()
            || self.cell.size <= 0.0
            || self.cell.gap < 0.0
            || self.cell.shadow < 0.0
            || self.layout.window.width <= 0.0
            || self.layout.window.height <= 0.0
            || self.layout.board.spacing <= 0.0
        {
            panic!(
                "BattleTris theme manifest {} has invalid name or layout values",
                manifest_path.display()
            );
        }
        for relative in [
            &self.sprites.atlas,
            &self.sprites.startup,
            &self.sprites.bazaar,
            &self.sprites.biff,
            &self.sprites.gimp,
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
        if !self.fonts.default.is_empty() {
            let path = theme_dir.join(&self.fonts.default);
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

#[derive(Debug, Deserialize)]
struct ThemeSprites {
    atlas: String,
    startup: String,
    bazaar: String,
    biff: String,
    gimp: String,
}

impl ThemeSprites {
    fn loaded(&self, choice: ThemeChoice) -> LoadedThemeSprites {
        let prefix = format!("themes/{}/", choice.directory());
        LoadedThemeSprites {
            startup: format!("{prefix}{}", self.startup),
            bazaar: format!("{prefix}{}", self.bazaar),
            biff: format!("{prefix}{}", self.biff),
        }
    }
}

#[derive(Debug, Clone)]
struct LoadedThemeSprites {
    startup: String,
    bazaar: String,
    biff: String,
}

#[derive(Debug, Deserialize)]
struct ThemeFonts {
    default: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ThemeCell {
    size: f32,
    gap: f32,
    shadow: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ThemeLayout {
    window: ThemeWindowLayout,
    board: ThemeBoardLayout,
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

#[derive(Debug, Clone)]
struct ThemePalette {
    background: Color,
    board_background: Color,
    empty: Color,
    structure: Color,
    happy: Color,
    frown: Color,
    gimp: Color,
    die: Color,
    invisible: Color,
    hidden: Color,
    text_primary: Color,
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

#[derive(Debug, Deserialize)]
struct RawThemePalette {
    background: String,
    board_background: String,
    empty: String,
    structure: String,
    happy: String,
    frown: String,
    gimp: String,
    die: String,
    invisible: String,
    hidden: String,
    text_primary: String,
    text_secondary: String,
    text_accent: String,
    visible_colors: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawThemeButtonStyle {
    normal: String,
    hover: String,
    pressed: String,
    text: String,
}

impl RawThemeButtonStyle {
    fn into_style(self, manifest_path: &std::path::Path) -> ThemeButtonStyle {
        ThemeButtonStyle {
            normal: parse_hex_color(&self.normal, manifest_path),
            hover: parse_hex_color(&self.hover, manifest_path),
            pressed: parse_hex_color(&self.pressed, manifest_path),
            text: parse_hex_color(&self.text, manifest_path),
        }
    }
}

impl RawThemePalette {
    fn into_palette(self, manifest_path: &std::path::Path) -> ThemePalette {
        ThemePalette {
            background: parse_hex_color(&self.background, manifest_path),
            board_background: parse_hex_color(&self.board_background, manifest_path),
            empty: parse_hex_color(&self.empty, manifest_path),
            structure: parse_hex_color(&self.structure, manifest_path),
            happy: parse_hex_color(&self.happy, manifest_path),
            frown: parse_hex_color(&self.frown, manifest_path),
            gimp: parse_hex_color(&self.gimp, manifest_path),
            die: parse_hex_color(&self.die, manifest_path),
            invisible: parse_hex_color(&self.invisible, manifest_path),
            hidden: parse_hex_color(&self.hidden, manifest_path),
            text_primary: parse_hex_color(&self.text_primary, manifest_path),
            text_secondary: parse_hex_color(&self.text_secondary, manifest_path),
            text_accent: parse_hex_color(&self.text_accent, manifest_path),
            visible_colors: self
                .visible_colors
                .iter()
                .map(|color| parse_hex_color(color, manifest_path))
                .collect(),
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
    theme: ThemeChoice,
    sound_pack: SoundPackChoice,
    controls: ControlScheme,
    pixel_scale: f32,
    ernie_level: usize,
    display_name: String,
    community_label: String,
    direct_listen_addr: String,
    lobby_addr: String,
    settings_path: Option<PathBuf>,
    assets_dir: PathBuf,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            screen: ClientScreen::Startup,
            theme: ThemeChoice::OriginalInspired,
            sound_pack: SoundPackChoice::GeneratedDefault,
            controls: ControlScheme::ModernSplit,
            pixel_scale: 1.0,
            ernie_level: DEFAULT_ERNIE_LEVEL,
            display_name: default_display_name(),
            community_label: CommunityLabel::local().as_str().to_string(),
            direct_listen_addr: "127.0.0.1:4405".to_string(),
            lobby_addr: "127.0.0.1:4404".to_string(),
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
            lobby_addr: self.lobby_addr.clone(),
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
            sanitize_nonempty_setting(persisted.direct_listen_addr, "127.0.0.1:4405".to_string());
        self.lobby_addr =
            sanitize_nonempty_setting(persisted.lobby_addr, "127.0.0.1:4404".to_string());
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
    lobby_addr: String,
}

impl Default for PersistedClientSettings {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::OriginalInspired,
            sound_pack: SoundPackChoice::GeneratedDefault,
            controls: ControlScheme::ModernSplit,
            pixel_scale: 1.0,
            ernie_level: DEFAULT_ERNIE_LEVEL,
            display_name: default_display_name(),
            community_label: "local".to_string(),
            direct_listen_addr: "127.0.0.1:4405".to_string(),
            lobby_addr: "127.0.0.1:4404".to_string(),
        }
    }
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
    rank: u64,
    display_name: String,
    wins: u64,
    losses: u64,
    high_score: u64,
    high_lines: u64,
    high_funds: u64,
    streak: String,
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
                        rank: profile.rank,
                        display_name: profile.display_name,
                        wins: profile.wins,
                        losses: profile.losses,
                        high_score: profile.high_score,
                        high_lines: profile.high_lines,
                        high_funds: profile.high_funds,
                        streak: streak_label(profile.streak_kind, profile.streak_count),
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
    let mut game = TwoPlayerGame::bazaar_fixture(
        GameSeed::from_u64(111),
        visual_playing_board(),
        650,
        GameSeed::from_u64(222),
        visual_opponent_board(),
        425,
    );
    let _ = game.bazaar_buy(PlayerId::One, WeaponToken::Gimp);
    let _ = game.bazaar_buy(PlayerId::One, WeaponToken::FlipOut);
    let _ = game.bazaar_buy(PlayerId::Two, WeaponToken::RiseUp);
    LocalGame {
        game,
        computer: None,
        local_player: PlayerId::One,
        status_message: Some("Visual fixture: bazaar shopping".to_string()),
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
            selected: WeaponToken::Gimp,
            last_message: "Visual fixture has staged Gimp and Flip out for Player 1.".to_string(),
        }
    } else {
        BazaarUiState::default()
    }
}

fn visual_roster_records() -> RosterRecords {
    RosterRecords {
        rows: vec![
            RosterRow {
                rank: 1,
                display_name: "Ada".to_string(),
                wins: 12,
                losses: 3,
                high_score: 48_250,
                high_lines: 82,
                high_funds: 1_450,
                streak: "W5".to_string(),
            },
            RosterRow {
                rank: 2,
                display_name: "Grace".to_string(),
                wins: 9,
                losses: 4,
                high_score: 37_600,
                high_lines: 69,
                high_funds: 1_100,
                streak: "W2".to_string(),
            },
            RosterRow {
                rank: 3,
                display_name: "Katherine".to_string(),
                wins: 7,
                losses: 5,
                high_score: 31_900,
                high_lines: 58,
                high_funds: 980,
                streak: "L1".to_string(),
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
            Coord::new(index % BOARD_WIDTH, 5 + index / BOARD_WIDTH)
                .expect("fixture coordinate in bounds"),
            Some(cell),
        );
    }
    for y in BOARD_HEIGHT - 5..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            if !(x == 4 && y == BOARD_HEIGHT - 1) {
                board.set(
                    Coord::new(x, y).expect("fixture coordinate in bounds"),
                    Some(visible_cell(((x + y) % 9 + 1) as u8)),
                );
            }
        }
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
}

impl Default for BazaarUiState {
    fn default() -> Self {
        Self {
            selected: WeaponToken::Gimp,
            last_message: "Select a weapon, then Add. Click staged arsenal slots to remove."
                .to_string(),
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
    Warning,
    GameOver,
}

impl SoundEvent {
    const ALL: [Self; 8] = [
        Self::MenuAction,
        Self::PieceLocked,
        Self::LineClear,
        Self::BazaarEntered,
        Self::Purchase,
        Self::WeaponLaunch,
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

#[derive(Component)]
struct HudText {
    player: PlayerId,
}

#[derive(Component)]
struct PhaseText;

#[derive(Component)]
struct MenuText;

#[derive(Component)]
struct GameEntity;

#[derive(Component)]
struct BazaarEntity;

#[derive(Component)]
struct BazaarText;

#[derive(Component)]
struct PlayerViewEntity {
    player: PlayerId,
}

#[derive(Component)]
struct ScreenShell;

#[derive(Component)]
struct ScreenText;

#[derive(Component)]
struct ButtonFace;

#[derive(Component)]
struct MenuButton {
    screen: ClientScreen,
    rect: Rect,
    action: MenuAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    StartHumanVsHuman,
    StartHumanVsComputer,
    AdjustErnieDifficulty(isize),
    GoTo(ClientScreen),
    Quit,
}

fn setup(
    mut commands: Commands,
    settings: Res<ClientSettings>,
    themes: Res<ThemePacks>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn(Camera2d);
    let theme = themes.get(settings.theme);

    spawn_screen_shell(&mut commands, theme, &asset_server);

    spawn_player_view(
        &mut commands,
        theme,
        PlayerId::One,
        theme.layout.board.player_one_left,
        "Player 1",
    );
    spawn_player_view(
        &mut commands,
        theme,
        PlayerId::Two,
        theme.layout.board.player_two_left,
        "Player 2 / Computer",
    );
    spawn_bazaar_overlay(&mut commands, theme, &asset_server);

    commands.spawn((
        Text2d::new("BattleTris"),
        TextFont::from_font_size(22.0),
        TextColor(theme.palette.text_primary),
        Transform::from_xyz(0.0, -300.0, 5.0),
        PhaseText,
        GameEntity,
    ));

    commands.spawn((
        Text2d::new(""),
        TextFont::from_font_size(18.0),
        TextColor(theme.palette.text_accent),
        Transform::from_xyz(0.0, 245.0, 10.0),
        MenuText,
        ScreenShell,
    ));
}

fn spawn_bazaar_overlay(commands: &mut Commands, theme: &LoadedTheme, asset_server: &AssetServer) {
    commands.spawn((
        Sprite::from_image(asset_server.load(theme.sprites.bazaar.clone())),
        Transform::from_xyz(0.0, 0.0, 20.0),
        Visibility::Hidden,
        BazaarEntity,
        GameEntity,
    ));
    commands.spawn((
        Text2d::new(""),
        TextFont::from_font_size(12.0),
        TextColor(theme.palette.text_primary),
        Transform::from_xyz(-370.0, 350.0, 21.0),
        Visibility::Hidden,
        BazaarEntity,
        BazaarText,
        GameEntity,
    ));
}

fn spawn_screen_shell(commands: &mut Commands, theme: &LoadedTheme, asset_server: &AssetServer) {
    commands.spawn((
        Sprite::from_image(asset_server.load(theme.sprites.startup.clone())),
        Transform::from_xyz(0.0, 34.0, -2.0),
        ScreenShell,
    ));

    commands.spawn((
        Sprite::from_image(asset_server.load(theme.sprites.biff.clone())),
        Transform::from_xyz(-220.0, -155.0, 1.0),
        ScreenShell,
    ));

    commands.spawn((
        Text2d::new(""),
        TextFont::from_font_size(18.0),
        TextColor(theme.palette.text_primary),
        Transform::from_xyz(55.0, 70.0, 4.0),
        ScreenText,
        ScreenShell,
    ));

    for spec in startup_buttons() {
        spawn_menu_button(commands, theme, spec);
    }
    for spec in secondary_screen_buttons() {
        spawn_menu_button(commands, theme, spec);
    }
}

#[derive(Debug, Clone, Copy)]
struct MenuButtonSpec {
    screen: ClientScreen,
    label: &'static str,
    center: Vec2,
    size: Vec2,
    action: MenuAction,
}

fn spawn_menu_button(commands: &mut Commands, theme: &LoadedTheme, spec: MenuButtonSpec) {
    commands.spawn((
        Sprite::from_color(theme.button.normal, spec.size),
        Transform::from_xyz(spec.center.x, spec.center.y, 3.0),
        ButtonFace,
        MenuButton {
            screen: spec.screen,
            rect: Rect::from_center_size(spec.center, spec.size),
            action: spec.action,
        },
        ScreenShell,
    ));
    commands.spawn((
        Text2d::new(spec.label),
        TextFont::from_font_size(17.0),
        TextColor(theme.button.text),
        Transform::from_xyz(spec.center.x, spec.center.y - 5.0, 4.0),
        MenuButton {
            screen: spec.screen,
            rect: spec.rect(),
            action: spec.action,
        },
        ScreenShell,
    ));
}

impl MenuButtonSpec {
    fn rect(self) -> Rect {
        Rect::from_center_size(self.center, self.size)
    }
}

fn startup_buttons() -> [MenuButtonSpec; 7] {
    let size = Vec2::new(150.0, 34.0);
    [
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "Challenge",
            center: Vec2::new(210.0, 120.0),
            size,
            action: MenuAction::GoTo(ClientScreen::Challenge),
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "Sleep",
            center: Vec2::new(210.0, 75.0),
            size,
            action: MenuAction::GoTo(ClientScreen::Sleep),
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "About",
            center: Vec2::new(210.0, 30.0),
            size,
            action: MenuAction::GoTo(ClientScreen::About),
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "Roster",
            center: Vec2::new(210.0, -15.0),
            size,
            action: MenuAction::GoTo(ClientScreen::Roster),
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "Quit",
            center: Vec2::new(210.0, -60.0),
            size,
            action: MenuAction::Quit,
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "Local Game",
            center: Vec2::new(210.0, -125.0),
            size,
            action: MenuAction::StartHumanVsHuman,
        },
        MenuButtonSpec {
            screen: ClientScreen::Startup,
            label: "Play Ernie",
            center: Vec2::new(210.0, -170.0),
            size,
            action: MenuAction::StartHumanVsComputer,
        },
    ]
}

fn secondary_screen_buttons() -> [MenuButtonSpec; 8] {
    let size = Vec2::new(132.0, 32.0);
    [
        MenuButtonSpec {
            screen: ClientScreen::Challenge,
            label: "Level -",
            center: Vec2::new(-85.0, -118.0),
            size,
            action: MenuAction::AdjustErnieDifficulty(-1),
        },
        MenuButtonSpec {
            screen: ClientScreen::Challenge,
            label: "Level +",
            center: Vec2::new(52.0, -118.0),
            size,
            action: MenuAction::AdjustErnieDifficulty(1),
        },
        MenuButtonSpec {
            screen: ClientScreen::Challenge,
            label: "Play Ernie",
            center: Vec2::new(200.0, -168.0),
            size,
            action: MenuAction::StartHumanVsComputer,
        },
        MenuButtonSpec {
            screen: ClientScreen::Challenge,
            label: "Back",
            center: Vec2::new(52.0, -168.0),
            size,
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
        MenuButtonSpec {
            screen: ClientScreen::Sleep,
            label: "Wake",
            center: Vec2::new(52.0, -168.0),
            size,
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
        MenuButtonSpec {
            screen: ClientScreen::About,
            label: "OK",
            center: Vec2::new(52.0, -168.0),
            size,
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
        MenuButtonSpec {
            screen: ClientScreen::Roster,
            label: "Back",
            center: Vec2::new(52.0, -168.0),
            size,
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
        MenuButtonSpec {
            screen: ClientScreen::Settings,
            label: "Back",
            center: Vec2::new(52.0, -168.0),
            size,
            action: MenuAction::GoTo(ClientScreen::Startup),
        },
    ]
}

fn spawn_player_view(
    commands: &mut Commands,
    theme: &LoadedTheme,
    player: PlayerId,
    left: f32,
    label: &str,
) {
    let width = BOARD_WIDTH as f32 * theme.cell.size;
    let height = BOARD_HEIGHT as f32 * theme.cell.size;
    let center_x = left + width / 2.0;
    let center_y = theme.layout.board.top - height / 2.0;

    commands.spawn((
        Sprite::from_color(
            theme.palette.board_background,
            Vec2::new(
                width + theme.cell.shadow * 4.0,
                height + theme.cell.shadow * 4.0,
            ),
        ),
        Transform::from_xyz(center_x, center_y, -1.0),
        PlayerViewEntity { player },
        GameEntity,
    ));

    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            commands.spawn((
                Sprite::from_color(
                    theme.palette.empty,
                    Vec2::splat((theme.cell.size - theme.cell.gap).max(1.0)),
                ),
                Transform::from_xyz(cell_x(theme, left, x), cell_y(theme, y), 0.0),
                BoardCell { player, x, y },
                PlayerViewEntity { player },
                GameEntity,
            ));
        }
    }

    commands.spawn((
        Text2d::new(label),
        TextFont::from_font_size(24.0),
        TextColor(theme.palette.text_primary),
        Transform::from_xyz(center_x, theme.layout.board.top + 34.0, 5.0),
        PlayerViewEntity { player },
        GameEntity,
    ));

    commands.spawn((
        Text2d::new(""),
        TextFont::from_font_size(15.0),
        TextColor(theme.palette.text_secondary),
        Transform::from_xyz(center_x, theme.layout.board.top - height - 44.0, 5.0),
        HudText { player },
        PlayerViewEntity { player },
        GameEntity,
    ));
}

#[derive(SystemParam)]
struct KeyboardInputParams<'w> {
    time: Res<'w, Time>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    local: ResMut<'w, LocalGame>,
    settings: ResMut<'w, ClientSettings>,
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
            handle_settings_input(&input.keys, &mut input.settings, &mut input.sound);
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
        handle_bazaar_click(world, &mut input.local.game, &mut input.bazaar_ui);
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
        MenuAction::StartHumanVsHuman => {
            *local = LocalGame::new_human_vs_human();
            settings.screen = ClientScreen::Game;
            sound.next_log_index = 0;
        }
        MenuAction::StartHumanVsComputer => {
            *local = LocalGame::new_human_vs_computer(settings.ernie_level);
            settings.screen = ClientScreen::Game;
            sound.next_log_index = 0;
        }
        MenuAction::AdjustErnieDifficulty(step) => {
            adjust_ernie_level(settings, step);
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
}

fn handle_challenge_input(
    keys: &ButtonInput<KeyCode>,
    local: &mut LocalGame,
    settings: &mut ClientSettings,
    sound: &mut SoundEventState,
) {
    if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyJ) {
        adjust_ernie_level(settings, -1);
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyL) {
        adjust_ernie_level(settings, 1);
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::KeyC) {
        *local = LocalGame::new_human_vs_computer(settings.ernie_level);
        settings.screen = ClientScreen::Game;
        sound.next_log_index = 0;
        queue_sound(sound, SoundEvent::MenuAction);
    }
}

fn handle_settings_input(
    keys: &ButtonInput<KeyCode>,
    settings: &mut ClientSettings,
    sound: &mut SoundEventState,
) {
    let previous = settings.persisted();

    if keys.just_pressed(KeyCode::KeyT) {
        settings.theme = match settings.theme {
            ThemeChoice::OriginalInspired => ThemeChoice::HighContrast,
            ThemeChoice::HighContrast => ThemeChoice::OriginalInspired,
        };
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::KeyO) {
        settings.sound_pack = match settings.sound_pack {
            SoundPackChoice::GeneratedDefault => SoundPackChoice::Muted,
            SoundPackChoice::Muted => SoundPackChoice::GeneratedDefault,
        };
        queue_sound(sound, SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::KeyM) {
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

    if settings.persisted() != previous {
        settings.save();
    }
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
        handle_bazaar_input(keys, &mut local.game, bazaar_ui);
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
) {
    if keys.just_pressed(KeyCode::Enter) {
        match game.bazaar_done(PlayerId::One) {
            events if events.is_empty() => {
                bazaar_ui.last_message = "Player 1 is already waiting.".to_string()
            }
            _ => bazaar_ui.last_message = "Player 1 done. Waiting for opponent.".to_string(),
        }
    }
    if keys.just_pressed(KeyCode::Space) {
        match game.bazaar_done(PlayerId::Two) {
            events if events.is_empty() => {
                bazaar_ui.last_message = "Player 2 is already waiting.".to_string()
            }
            _ => bazaar_ui.last_message = "Player 2 done. Waiting for opponent.".to_string(),
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
        let Some(sound_event) = sound_packs.sound_for(settings.sound_pack, event) else {
            continue;
        };
        commands.spawn((
            AudioPlayer::new(asset_server.load(sound_event.file.clone())),
            PlaybackSettings::DESPAWN,
        ));
    }
}

type HudTextQuery<'w, 's> =
    Query<'w, 's, (&'static HudText, &'static mut Text2d), (Without<PhaseText>, Without<MenuText>)>;

type PhaseTextSingle<'w, 's> =
    Single<'w, 's, &'static mut Text2d, (With<PhaseText>, Without<HudText>, Without<MenuText>)>;

type MenuTextSingle<'w, 's> =
    Single<'w, 's, &'static mut Text2d, (With<MenuText>, Without<HudText>, Without<PhaseText>)>;

type ScreenTextSingle<'w, 's> = Single<
    'w,
    's,
    &'static mut Text2d,
    (
        With<ScreenText>,
        Without<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
    ),
>;

type BazaarTextSingle<'w, 's> = Single<
    'w,
    's,
    &'static mut Text2d,
    (
        With<BazaarText>,
        Without<MenuText>,
        Without<HudText>,
        Without<PhaseText>,
        Without<ScreenText>,
    ),
>;

type ShellVisibilityQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Visibility, Option<&'static MenuButton>),
    (With<ScreenShell>, Without<GameEntity>),
>;

type GameVisibilityQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Visibility,
        Option<&'static PlayerViewEntity>,
        Option<&'static BazaarEntity>,
    ),
    With<GameEntity>,
>;

#[derive(SystemParam)]
struct RenderGameParams<'w, 's> {
    local: Res<'w, LocalGame>,
    settings: Res<'w, ClientSettings>,
    roster: Res<'w, RosterRecords>,
    themes: Res<'w, ThemePacks>,
    sound: Res<'w, SoundEventState>,
    bazaar_ui: Res<'w, BazaarUiState>,
    clear_color: ResMut<'w, ClearColor>,
    recon: Res<'w, ReconPanel>,
    cells: Query<'w, 's, (&'static BoardCell, &'static mut Sprite)>,
    hud: HudTextQuery<'w, 's>,
    phase_text: PhaseTextSingle<'w, 's>,
    menu_text: MenuTextSingle<'w, 's>,
    screen_text: ScreenTextSingle<'w, 's>,
    bazaar_text: BazaarTextSingle<'w, 's>,
    reported_startup_render: Local<'s, bool>,
}

fn render_game(mut render: RenderGameParams) {
    let theme = render.themes.get(render.settings.theme);
    for (cell, mut sprite) in &mut render.cells {
        sprite.color = render_cell_color(
            &render.local,
            &render.recon,
            cell.player,
            cell.x,
            cell.y,
            theme,
        );
        sprite.custom_size = Some(Vec2::splat(
            ((theme.cell.size - theme.cell.gap) * render.settings.pixel_scale).max(1.0),
        ));
    }

    for (hud, mut text) in &mut render.hud {
        text.0 = player_hud(&render.local, &render.recon, hud.player);
    }

    render.phase_text.0 = phase_label(&render.local, &render.settings, &render.sound);
    render.menu_text.0 = menu_label(&render.local.game, &render.settings);
    render.screen_text.0 = screen_body_label(&render.local.game, &render.settings, &render.roster);
    render.bazaar_text.0 = bazaar_overlay_label(&render.local, &render.bazaar_ui);
    let menu_label_chars =
        render.menu_text.0.chars().count() + render.screen_text.0.chars().count();
    let menu_is_unhealthy = render.settings.screen != ClientScreen::Game && menu_label_chars == 0;
    render.clear_color.0 = if menu_is_unhealthy {
        Color::srgb(0.5, 0.0, 0.28)
    } else {
        theme.palette.background
    };

    if !*render.reported_startup_render {
        report_startup_render_health(render.settings.screen, menu_label_chars);
        *render.reported_startup_render = true;
    }
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
    for (mut visibility, player_view, bazaar_entity) in &mut game_entities {
        let entity_visible = if bazaar_entity.is_some() {
            bazaar_visible
        } else {
            player_view.is_none_or(|view| player_view_visible(&local, &recon, view.player))
        };
        *visibility = if game_visible && entity_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for (mut visibility, button) in &mut shell_entities {
        let visible = !game_visible && button.is_none_or(|button| button.screen == settings.screen);
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
        sprite.color = if hovered && mouse.pressed(MouseButton::Left) {
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
    if screen != ClientScreen::Game && menu_label_chars == 0 {
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

fn render_cell_color(
    local: &LocalGame,
    recon: &ReconPanel,
    player: PlayerId,
    x: usize,
    y: usize,
    theme: &LoadedTheme,
) -> Color {
    if player != local.local_player && local.computer.is_some() {
        if recon.manual_condor {
            return local
                .game
                .player(player)
                .board()
                .get(Coord { x, y })
                .map_or_else(
                    || empty_cell_color(theme),
                    |cell| cell_color(cell, false, theme),
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
                    || empty_cell_color(theme),
                    |cell| cell_color(cell, false, theme),
                );
        }
        return empty_cell_color(theme);
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
        return cell_color(cell, true, theme);
    }

    let Some(coord) = Coord::new(x, y) else {
        return empty_cell_color(theme);
    };
    local.game.player(player).board().get(coord).map_or_else(
        || empty_cell_color(theme),
        |cell| cell_color(cell, false, theme),
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

fn phase_label(local: &LocalGame, settings: &ClientSettings, sound: &SoundEventState) -> String {
    if settings.screen != ClientScreen::Game {
        return String::new();
    }
    let game = &local.game;

    let sound_label = sound
        .last_event
        .map(|event| format!("  sound {:?}", event))
        .unwrap_or_default();
    let mode = match game.mode() {
        GameMode::HumanVsHuman => "human vs human",
        GameMode::HumanVsComputer { .. } => "human vs computer (unranked)",
    };
    let status = local
        .status_message
        .as_ref()
        .map(|message| format!("  {message}"))
        .unwrap_or_default();
    let weapon_status = latest_weapon_feedback(game)
        .map(|message| format!("  {message}"))
        .unwrap_or_default();
    let common = format!(
        "F1 menu  F2 challenge  F3 settings  Esc game  {mode}{sound_label}{status}{weapon_status}"
    );

    match game.phase() {
        GamePhase::Playing => format!(
            "{}\nP pause  R restart  controls {}  1-0 launch P1, Shift+1-0 launch P2",
            common,
            controls_label(settings.controls)
        ),
        GamePhase::Paused => format!("{common}\npaused  P resume  R restart"),
        GamePhase::Bazaar => format!(
            "{}\nbazaar  click rows/Add/Remove/DONE  Up/Down select  A add  X remove  Enter done",
            common
        ),
        GamePhase::GameOver => game
            .event_log()
            .iter()
            .rev()
            .find_map(|logged| match logged.event {
                BattleEvent::GameOver { winner, loser } => Some(format!(
                    "{common}\n{winner:?} wins, {loser:?} loses  R restart"
                )),
                _ => None,
            })
            .unwrap_or_else(|| format!("{common}\ngame over  R restart")),
    }
}

fn menu_label(_game: &TwoPlayerGame, settings: &ClientSettings) -> String {
    match settings.screen {
        ClientScreen::Startup => "BattleTris\nChallenge, Sleep, About, Roster, Quit".to_string(),
        ClientScreen::Game => String::new(),
        ClientScreen::Challenge => "Challenge".to_string(),
        ClientScreen::Sleep => "Sleep".to_string(),
        ClientScreen::About => "About BattleTris".to_string(),
        ClientScreen::Roster => "Roster".to_string(),
        ClientScreen::Settings => format!(
            "Settings\nT theme: {:?}  O sound: {:?}  M controls\n-/= scale: {:.2}x",
            settings.theme, settings.sound_pack, settings.pixel_scale,
        ),
    }
}

fn screen_body_label(
    game: &TwoPlayerGame,
    settings: &ClientSettings,
    roster: &RosterRecords,
) -> String {
    match settings.screen {
        ClientScreen::Startup => {
            "Click Challenge for human/computer setup, Sleep to wait as available, About for credits, Roster for records, or Quit. Keyboard: H local game, C Play Ernie, F2/F3/F4/F5/F6 screens.".to_string()
        }
        ClientScreen::Challenge => {
            let difficulty = selected_ernie_difficulty(settings);
            let lobby_preview = lobby_registration_preview(settings);
            format!(
                "Challenge\nIdentity: {} ({}) on community {}\nDirect TCP protocol v{}.{} ({}, {})\nAdvertised direct address: {}  Lobby: {}  Ranked: {}\nErnie difficulty: level {} of {}  {}  {}ms action delay\nUse J/Left and L/Right or Level -/+ as the slider.\nClick Play {} Ernie or press Enter/C for unranked computer play.",
                lobby_preview.player.display_name,
                lobby_preview.player.player_id,
                settings.community_label,
                PROTOCOL_MAJOR,
                PROTOCOL_MINOR,
                CAPABILITY_DIRECT_TCP,
                CAPABILITY_SELF_HOSTED_LOBBY,
                lobby_preview.direct_addr,
                settings.lobby_addr,
                lobby_preview.ranked,
                difficulty.level,
                COMPUTER_DIFFICULTIES.len() - 1,
                difficulty.name,
                difficulty.delay_ms,
                difficulty.name,
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
            "BattleTris\nRust/Bevy rewrite of the legacy X11/Motif game.\nOriginal authors: Bryan Cantrill, Charlie Hoecker, and Mike Shapiro, Brown CS32 spring 1994.\nRevived and exhumed by the BattleTris community in 2026.\nThis build preserves deterministic rules, the weapon economy, Biff, Ernie, ranked records, and source-backed art/audio packs.\nClick OK to return.".to_string()
        }
        ClientScreen::Roster => roster_label(game, settings, roster),
        ClientScreen::Settings => format!(
            "Theme: {:?}\nSound pack: {:?}\nControls: {}\nScale: {:.2}x\nDisplay name: {}\nCommunity: {}\nDirect listen: {}\nLobby: {}\nProtocol: v{}.{}\nAssets: {}\nSettings: {}",
            settings.theme,
            settings.sound_pack,
            controls_label(settings.controls),
            settings.pixel_scale,
            settings.display_name,
            settings.community_label,
            settings.direct_listen_addr,
            settings.lobby_addr,
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

fn sanitize_ernie_level(level: usize) -> usize {
    level.min(COMPUTER_DIFFICULTIES.len() - 1)
}

fn adjust_ernie_level(settings: &mut ClientSettings, step: isize) {
    let max = COMPUTER_DIFFICULTIES.len() as isize - 1;
    settings.ernie_level = (settings.ernie_level as isize + step).clamp(0, max) as usize;
    settings.save();
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
        direct_addr: settings.direct_listen_addr.clone(),
        ranked: true,
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

fn roster_label(game: &TwoPlayerGame, settings: &ClientSettings, roster: &RosterRecords) -> String {
    let ranked = if game.is_ranked_game() {
        "ranked"
    } else {
        "unranked"
    };
    let mut label = format!(
        "Roster\nPlayer 1: {}\nPlayer 2: {}\nCommunity: {}\nCurrent game is {ranked}. Local records sorted by rank.\n",
        settings.display_name,
        match game.mode() {
            GameMode::HumanVsHuman => "local human",
            GameMode::HumanVsComputer { .. } => "computer Ernie",
        },
        settings.community_label,
    );
    if let Some(error) = &roster.error {
        let _ = writeln!(label, "Records unavailable: {error}");
    } else if roster.rows.is_empty() {
        label.push_str("No ranked human-vs-human results have been recorded yet.\n");
    } else {
        for row in roster.rows.iter().take(8) {
            let _ = writeln!(
                label,
                "#{:<4} {:<16} {:>3}-{:>3} score {} lines {} funds {} streak {}",
                row.rank,
                truncate_label(&row.display_name, 16),
                row.wins,
                row.losses,
                row.high_score,
                row.high_lines,
                row.high_funds,
                row.streak,
            );
        }
        if roster.rows.len() > 8 {
            let _ = writeln!(label, "... and {} more", roster.rows.len() - 8);
        }
    }
    label.push_str("Esc returns to game.");
    label
}

fn streak_label(kind: StreakKind, count: u64) -> String {
    match kind {
        StreakKind::None => "none".to_string(),
        StreakKind::Wins => format!("W{count}"),
        StreakKind::Losses => format!("L{count}"),
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

fn bazaar_overlay_label(local: &LocalGame, ui: &BazaarUiState) -> String {
    if local.game.phase() != GamePhase::Bazaar {
        return String::new();
    }

    let Some(bazaar) = local.game.bazaar_session(local.local_player) else {
        return "Bazaar closed".to_string();
    };
    let selected = weapon_spec(ui.selected);
    let done = local.game.bazaar_player_done(local.local_player);
    let mut text = format!(
        "BATTLETRIS BAZAAR\nFunds: {}{}  Arsenal: {}\n{}\n\n",
        bazaar.staged_funds(),
        if bazaar.carter_prices() {
            "  CARTER PRICES"
        } else {
            ""
        },
        arsenal_slots_label(bazaar.staged_arsenal()),
        if done {
            "Done selected. Waiting for opponent; shopping controls are dimmed."
        } else {
            "Click a row to inspect. Click Add/Remove/DONE. Number slots launch in game, remove staged here."
        }
    );

    for (row, spec) in sorted_weapon_catalog().into_iter().enumerate() {
        let marker = if spec.token == ui.selected { '>' } else { ' ' };
        let duration = if spec.line_duration == 0 {
            "one-shot".to_string()
        } else {
            format!("{} lines", spec.line_duration)
        };
        let _ = writeln!(
            text,
            "{marker}{:02}. {:<21} ${:<4} {:<8}",
            row + 1,
            spec.name,
            bazaar.price(spec.token),
            duration,
        );
    }

    let _ = write!(
        text,
        "\nSelected: {}  ${}  {}\n{}\n\n[Add] [Remove staged] [DONE]\n{}",
        selected.name,
        bazaar.price(selected.token),
        if selected.line_duration == 0 {
            "one-shot".to_string()
        } else {
            format!("{} target lines", selected.line_duration)
        },
        selected.description,
        ui.last_message,
    );
    text
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

fn cell_color(cell: Cell, _active: bool, theme: &LoadedTheme) -> Color {
    match cell {
        Cell::Visible { color } => {
            let index = usize::from(color.get().saturating_sub(1))
                % theme.palette.visible_colors.len().max(1);
            theme
                .palette
                .visible_colors
                .get(index)
                .copied()
                .unwrap_or(theme.palette.text_accent)
        }
        Cell::Structure => theme.palette.structure,
        Cell::Happy => theme.palette.happy,
        Cell::Frown => theme.palette.frown,
        Cell::Gimp { .. } => theme.palette.gimp,
        Cell::Die { .. } => theme.palette.die,
        Cell::Invisible => theme.palette.invisible,
        Cell::Hidden { .. } => theme.palette.hidden,
    }
}

fn empty_cell_color(theme: &LoadedTheme) -> Color {
    theme.palette.empty
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
        BattleEvent::WeaponLaunched { .. }
        | BattleEvent::OneShotWeaponApplied { .. }
        | BattleEvent::TimedWeaponActivated { .. }
        | BattleEvent::WeaponReflected { .. }
        | BattleEvent::WeaponNullified { .. } => Some(SoundEvent::WeaponLaunch),
        BattleEvent::TimedWeaponExpired { .. } => Some(SoundEvent::Purchase),
        BattleEvent::PlayerDied { .. } | BattleEvent::GameOver { .. } => Some(SoundEvent::GameOver),
        BattleEvent::Paused | BattleEvent::Resumed => Some(SoundEvent::MenuAction),
        _ => None,
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

fn handle_bazaar_click(world: Vec2, game: &mut TwoPlayerGame, ui: &mut BazaarUiState) {
    let player = PlayerId::One;
    if let Some(token) = bazaar_catalog_token_at(world) {
        ui.selected = token;
        ui.last_message = format!("Selected {}.", weapon_spec(token).name);
        return;
    }
    if bazaar_add_rect().contains(world) {
        buy_selected_bazaar_weapon(game, ui, player);
        return;
    }
    if bazaar_remove_rect().contains(world) {
        remove_selected_bazaar_weapon(game, ui, player);
        return;
    }
    if bazaar_done_rect().contains(world) {
        match game.bazaar_done(player) {
            events if events.is_empty() => {
                ui.last_message = "Already waiting for opponent.".to_string()
            }
            _ => ui.last_message = "Done. Waiting for opponent.".to_string(),
        }
        return;
    }
    if let Some(token) = bazaar_arsenal_token_at(world, game, player) {
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

fn bazaar_catalog_token_at(world: Vec2) -> Option<WeaponToken> {
    const LEFT: f32 = -390.0;
    const RIGHT: f32 = 95.0;
    const TOP: f32 = 284.0;
    const ROW_HEIGHT: f32 = 13.5;
    if world.x < LEFT || world.x > RIGHT || world.y > TOP || world.y < TOP - ROW_HEIGHT * 34.0 {
        return None;
    }
    let row = ((TOP - world.y) / ROW_HEIGHT).floor() as usize;
    sorted_weapon_catalog().get(row).map(|spec| spec.token)
}

fn bazaar_arsenal_token_at(
    world: Vec2,
    game: &TwoPlayerGame,
    player: PlayerId,
) -> Option<WeaponToken> {
    const LEFT: f32 = -75.0;
    const TOP: f32 = 338.0;
    const SLOT_WIDTH: f32 = 44.0;
    const SLOT_HEIGHT: f32 = 24.0;
    if world.x < LEFT
        || world.x > LEFT + SLOT_WIDTH * 10.0
        || world.y > TOP
        || world.y < TOP - SLOT_HEIGHT
    {
        return None;
    }
    let index = ((world.x - LEFT) / SLOT_WIDTH).floor() as usize;
    game.bazaar_session(player)
        .and_then(|bazaar| bazaar.staged_arsenal().slots().get(index))
        .copied()
        .flatten()
        .map(|slot| slot.token)
}

fn bazaar_add_rect() -> Rect {
    Rect::from_center_size(Vec2::new(170.0, -310.0), Vec2::new(82.0, 32.0))
}

fn bazaar_remove_rect() -> Rect {
    Rect::from_center_size(Vec2::new(275.0, -310.0), Vec2::new(118.0, 32.0))
}

fn bazaar_done_rect() -> Rect {
    Rect::from_center_size(Vec2::new(360.0, -310.0), Vec2::new(72.0, 32.0))
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
    fn capture_all_jobs_cover_every_fixture_for_one_theme() {
        let config = ClientRunConfig::parse(
            vec![
                OsString::from("headless"),
                OsString::from("capture-all"),
                OsString::from("--theme"),
                OsString::from("original-inspired"),
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
        assert_eq!(capture.jobs[0].theme, ThemeChoice::OriginalInspired);
        assert_eq!(
            capture.jobs[0].path,
            PathBuf::from("target/visual/current/startup.png")
        );
        assert_eq!(capture.jobs[0].expected_width, 1040);
        assert_eq!(capture.jobs[0].expected_height, 720);
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
        let first = bazaar_catalog_token_at(Vec2::new(-380.0, 283.0));
        let last = bazaar_catalog_token_at(Vec2::new(-380.0, 284.0 - 13.5 * 33.0));

        assert_eq!(first, Some(WeaponToken::FlipOut));
        assert_eq!(last, Some(WeaponToken::Swap));
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
    fn computer_opponent_view_is_hidden_until_recon() {
        let local = LocalGame::new_human_vs_computer(DEFAULT_ERNIE_LEVEL);
        let mut recon = ReconPanel::default();

        assert!(player_view_visible(&local, &recon, PlayerId::One));
        assert!(!player_view_visible(&local, &recon, PlayerId::Two));

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
            direct_listen_addr: "127.0.0.1:4405".to_string(),
            lobby_addr: "127.0.0.1:4404".to_string(),
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
                .sound_for(SoundPackChoice::GeneratedDefault, event)
                .expect("generated-default maps every semantic event");
            assert!(loaded.file.ends_with(".wav"));
        }
        assert!(packs
            .sound_for(SoundPackChoice::Muted, SoundEvent::LineClear)
            .is_none());
    }

    #[test]
    fn lobby_registration_preview_uses_protocol_identity() {
        let settings = ClientSettings {
            display_name: "Ada Lovelace".to_string(),
            direct_listen_addr: "127.0.0.1:4405".to_string(),
            ..Default::default()
        };

        let preview = lobby_registration_preview(&settings);

        assert_eq!(preview.player.player_id, "ada-lovelace");
        assert_eq!(preview.player.display_name, "Ada Lovelace");
        assert_eq!(preview.direct_addr, "127.0.0.1:4405");
        assert!(preview.ranked);
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
