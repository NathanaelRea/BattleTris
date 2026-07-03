//! Desktop client entry point.
//!
//! This crate hosts the Bevy application, rendering, menus, settings, audio
//! event mapping, and local keyboard input. It consumes deterministic core
//! state and events instead of owning gameplay rules.

use battletris_core::{
    ai::{computer_difficulty, ComputerOpponent, BAZAAR_LEAVE_DELAY_MS},
    board::{Board, Coord, BOARD_HEIGHT, BOARD_WIDTH},
    cell::Cell,
    game::{BattleEvent, Command, CoreEvent, GameMode, GamePhase, PlayerId, TwoPlayerGame},
    piece::PieceKind,
    rng::GameSeed,
    weapons::{WeaponToken, WEAPON_CATALOG},
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{ffi::OsStr, fmt::Write as _, fs, path::PathBuf};

const CELL_SIZE: f32 = 18.0;
const CELL_GAP: f32 = 1.5;
const BOARD_TOP: f32 = 255.0;
const PLAYER_ONE_LEFT: f32 = -360.0;
const PLAYER_TWO_LEFT: f32 = 120.0;
const SMOKE_SCREENSHOT_CAPTURE_DELAY_FRAMES: u16 = 5;
const SMOKE_SCREENSHOT_TIMEOUT_FRAMES: u16 = 300;
const SETTINGS_FILE_NAME: &str = "settings.toml";

fn main() {
    let smoke_screenshot = smoke_screenshot_path().map(SmokeScreenshot::new);
    let settings = ClientSettings::load_or_default();
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.045, 0.05, 0.065)))
        .insert_resource(LocalGame::new_human_vs_human())
        .insert_resource(settings)
        .insert_resource(SoundEventState::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BattleTris".into(),
                resolution: (1040, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                handle_keyboard_input,
                drive_computer_opponent,
                tick_game,
                collect_sound_events,
                render_game,
            ),
        );

    if let Some(smoke_screenshot) = smoke_screenshot {
        app.insert_resource(smoke_screenshot)
            .add_systems(Update, request_smoke_screenshot.after(render_game));
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
        }
    }

    fn apply_persisted(&mut self, persisted: PersistedClientSettings) {
        self.theme = persisted.theme;
        self.sound_pack = persisted.sound_pack;
        self.controls = persisted.controls;
        self.pixel_scale = sanitize_pixel_scale(persisted.pixel_scale);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct PersistedClientSettings {
    theme: ThemeChoice,
    sound_pack: SoundPackChoice,
    controls: ControlScheme,
    pixel_scale: f32,
}

impl Default for PersistedClientSettings {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::OriginalInspired,
            sound_pack: SoundPackChoice::GeneratedDefault,
            controls: ControlScheme::ModernSplit,
            pixel_scale: 1.0,
        }
    }
}

#[derive(Resource)]
struct LocalGame {
    game: TwoPlayerGame,
    computer: Option<ComputerController>,
}

impl LocalGame {
    fn new_human_vs_human() -> Self {
        Self {
            game: TwoPlayerGame::new(GameSeed::from_u64(1), GameSeed::from_u64(2)),
            computer: None,
        }
    }

    fn new_human_vs_computer() -> Self {
        let difficulty = computer_difficulty(7).expect("legacy AI difficulty exists");
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
        }
    }

    fn restart(&mut self) {
        *self = match self.game.mode() {
            GameMode::HumanVsHuman => Self::new_human_vs_human(),
            GameMode::HumanVsComputer { .. } => Self::new_human_vs_computer(),
        };
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
}

#[derive(Resource, Debug)]
struct SmokeScreenshot {
    path: PathBuf,
    frames_until_capture: u16,
    frames_since_request: u16,
    requested: bool,
}

impl SmokeScreenshot {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            frames_until_capture: SMOKE_SCREENSHOT_CAPTURE_DELAY_FRAMES,
            frames_since_request: 0,
            requested: false,
        }
    }
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

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    spawn_player_view(&mut commands, PlayerId::One, PLAYER_ONE_LEFT, "Player 1");
    spawn_player_view(
        &mut commands,
        PlayerId::Two,
        PLAYER_TWO_LEFT,
        "Player 2 / Computer",
    );

    commands.spawn((
        Text2d::new("BattleTris"),
        TextFont::from_font_size(22.0),
        TextColor(Color::srgb(0.86, 0.88, 0.82)),
        Transform::from_xyz(0.0, -300.0, 5.0),
        PhaseText,
    ));

    commands.spawn((
        Text2d::new(""),
        TextFont::from_font_size(18.0),
        TextColor(Color::srgb(0.9, 0.88, 0.74)),
        Transform::from_xyz(0.0, 245.0, 10.0),
        MenuText,
    ));
}

fn spawn_player_view(commands: &mut Commands, player: PlayerId, left: f32, label: &str) {
    let width = BOARD_WIDTH as f32 * CELL_SIZE;
    let height = BOARD_HEIGHT as f32 * CELL_SIZE;
    let center_x = left + width / 2.0;
    let center_y = BOARD_TOP - height / 2.0;

    commands.spawn((
        Sprite::from_color(
            Color::srgb(0.11, 0.13, 0.15),
            Vec2::new(width + 12.0, height + 12.0),
        ),
        Transform::from_xyz(center_x, center_y, -1.0),
    ));

    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            commands.spawn((
                Sprite::from_color(
                    empty_cell_color(ThemeChoice::OriginalInspired),
                    Vec2::splat((CELL_SIZE - CELL_GAP).max(1.0)),
                ),
                Transform::from_xyz(cell_x(left, x), cell_y(y), 0.0),
                BoardCell { player, x, y },
            ));
        }
    }

    commands.spawn((
        Text2d::new(label),
        TextFont::from_font_size(24.0),
        TextColor(Color::srgb(0.88, 0.9, 0.86)),
        Transform::from_xyz(center_x, BOARD_TOP + 34.0, 5.0),
    ));

    commands.spawn((
        Text2d::new(""),
        TextFont::from_font_size(15.0),
        TextColor(Color::srgb(0.74, 0.78, 0.72)),
        Transform::from_xyz(center_x, BOARD_TOP - height - 44.0, 5.0),
        HudText { player },
    ));
}

fn handle_keyboard_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut local: ResMut<LocalGame>,
    mut settings: ResMut<ClientSettings>,
    mut sound: ResMut<SoundEventState>,
) {
    handle_screen_shortcuts(&keys, &mut settings, &mut sound);

    match settings.screen {
        ClientScreen::Startup => handle_startup_input(&keys, &mut local, &mut settings, &mut sound),
        ClientScreen::Settings => handle_settings_input(&keys, &mut settings, &mut sound),
        ClientScreen::Game => handle_game_input(&keys, &mut local, &settings),
        ClientScreen::Challenge
        | ClientScreen::Sleep
        | ClientScreen::About
        | ClientScreen::Roster => {}
    }
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
        sound.last_event = Some(SoundEvent::MenuAction);
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
        sound.last_event = Some(SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::KeyC) {
        *local = LocalGame::new_human_vs_computer();
        settings.screen = ClientScreen::Game;
        sound.next_log_index = 0;
        sound.last_event = Some(SoundEvent::MenuAction);
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
        sound.last_event = Some(SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::KeyO) {
        settings.sound_pack = match settings.sound_pack {
            SoundPackChoice::GeneratedDefault => SoundPackChoice::Muted,
            SoundPackChoice::Muted => SoundPackChoice::GeneratedDefault,
        };
        sound.last_event = Some(SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::KeyM) {
        settings.controls = match settings.controls {
            ControlScheme::ModernSplit => ControlScheme::LegacyInspired,
            ControlScheme::LegacyInspired => ControlScheme::ModernSplit,
        };
        sound.last_event = Some(SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Equal) {
        settings.pixel_scale = sanitize_pixel_scale(settings.pixel_scale + 0.25).min(2.0);
        sound.last_event = Some(SoundEvent::MenuAction);
    }
    if keys.just_pressed(KeyCode::Minus) {
        settings.pixel_scale = sanitize_pixel_scale(settings.pixel_scale - 0.25).max(0.75);
        sound.last_event = Some(SoundEvent::MenuAction);
    }

    if settings.persisted() != previous {
        settings.save();
    }
}

fn handle_game_input(
    keys: &ButtonInput<KeyCode>,
    local: &mut LocalGame,
    settings: &ClientSettings,
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

    if local.game.phase() == GamePhase::Bazaar {
        handle_bazaar_input(keys, &mut local.game);
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
        send_player_controls(keys, &mut local.game, player, settings.controls);
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
) {
    let controls = controls_for(player, scheme);
    send_press_command(keys, game, player, controls.left, Command::MoveLeft);
    send_press_command(keys, game, player, controls.right, Command::MoveRight);
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

fn handle_bazaar_input(keys: &ButtonInput<KeyCode>, game: &mut TwoPlayerGame) {
    if keys.just_pressed(KeyCode::Enter) {
        let _ = game.bazaar_done(PlayerId::One);
    }
    if keys.just_pressed(KeyCode::Space) {
        let _ = game.bazaar_done(PlayerId::Two);
    }

    for (token, key) in bazaar_buy_keys() {
        if keys.just_pressed(key) {
            let player = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
                PlayerId::Two
            } else {
                PlayerId::One
            };
            let _ = game.bazaar_buy(player, token);
        }
    }
}

fn drive_computer_opponent(time: Res<Time>, mut local: ResMut<LocalGame>) {
    let elapsed_ms = time.delta().as_millis().min(u128::from(u64::MAX)) as u64;
    let Some(mut computer) = local.computer.take() else {
        return;
    };

    match local.game.phase() {
        GamePhase::Playing => {
            computer.reset_for_play();
            drive_computer_play(elapsed_ms, &mut local.game, &mut computer);
        }
        GamePhase::Bazaar => drive_computer_bazaar(elapsed_ms, &mut local.game, &mut computer),
        GamePhase::Paused | GamePhase::GameOver => {}
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
        game.player(opponent_player(computer.player)).lines(),
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
        for token in computer_bazaar_tokens() {
            let _ = game.bazaar_buy(computer.player, token);
        }
        computer.shopped_this_bazaar = true;
    }

    computer.bazaar_elapsed_ms = computer.bazaar_elapsed_ms.saturating_add(elapsed_ms);
    if computer.bazaar_elapsed_ms >= BAZAAR_LEAVE_DELAY_MS {
        let _ = game.bazaar_done(computer.player);
    }
}

fn tick_game(time: Res<Time>, mut local: ResMut<LocalGame>) {
    let elapsed_ms = time.delta().as_millis().min(u128::from(u64::MAX)) as u64;
    if elapsed_ms == 0 || local.game.phase() != GamePhase::Playing {
        return;
    }

    let _ = local.game.tick_player(PlayerId::One, elapsed_ms);
    let _ = local.game.tick_player(PlayerId::Two, elapsed_ms);
}

fn collect_sound_events(
    local: Res<LocalGame>,
    settings: Res<ClientSettings>,
    mut sound: ResMut<SoundEventState>,
) {
    if settings.sound_pack == SoundPackChoice::Muted {
        sound.next_log_index = local.game.event_log().len();
        sound.last_event = None;
        return;
    }

    for logged in &local.game.event_log()[sound.next_log_index..] {
        if let Some(event) = sound_event_for(&logged.event) {
            sound.last_event = Some(event);
        }
    }
    sound.next_log_index = local.game.event_log().len();
}

type HudTextQuery<'w, 's> =
    Query<'w, 's, (&'static HudText, &'static mut Text2d), (Without<PhaseText>, Without<MenuText>)>;

type PhaseTextSingle<'w, 's> =
    Single<'w, 's, &'static mut Text2d, (With<PhaseText>, Without<HudText>, Without<MenuText>)>;

type MenuTextSingle<'w, 's> =
    Single<'w, 's, &'static mut Text2d, (With<MenuText>, Without<HudText>, Without<PhaseText>)>;

#[derive(SystemParam)]
struct RenderGameParams<'w, 's> {
    local: Res<'w, LocalGame>,
    settings: Res<'w, ClientSettings>,
    sound: Res<'w, SoundEventState>,
    clear_color: ResMut<'w, ClearColor>,
    cells: Query<'w, 's, (&'static BoardCell, &'static mut Sprite)>,
    hud: HudTextQuery<'w, 's>,
    phase_text: PhaseTextSingle<'w, 's>,
    menu_text: MenuTextSingle<'w, 's>,
    reported_startup_render: Local<'s, bool>,
}

fn render_game(mut render: RenderGameParams) {
    for (cell, mut sprite) in &mut render.cells {
        sprite.color = render_cell_color(
            &render.local.game,
            cell.player,
            cell.x,
            cell.y,
            render.settings.theme,
        );
        sprite.custom_size = Some(Vec2::splat(
            ((CELL_SIZE - CELL_GAP) * render.settings.pixel_scale).max(1.0),
        ));
    }

    for (hud, mut text) in &mut render.hud {
        text.0 = player_hud(&render.local.game, hud.player);
    }

    render.phase_text.0 = phase_label(&render.local.game, &render.settings, &render.sound);
    render.menu_text.0 = menu_label(&render.local.game, &render.settings);
    let menu_label_chars = render.menu_text.0.chars().count();
    let menu_is_unhealthy = render.settings.screen != ClientScreen::Game && menu_label_chars == 0;
    render.clear_color.0 = if menu_is_unhealthy {
        Color::srgb(0.5, 0.0, 0.28)
    } else {
        Color::srgb(0.045, 0.05, 0.065)
    };

    if !*render.reported_startup_render {
        report_startup_render_health(render.settings.screen, menu_label_chars);
        *render.reported_startup_render = true;
    }
}

fn report_startup_render_health(screen: ClientScreen, menu_label_chars: usize) {
    info!("BattleTris render health: screen={screen:?} menu_label_chars={menu_label_chars}");
    if screen != ClientScreen::Game && menu_label_chars == 0 {
        error!("BattleTris render health: non-game screen has empty menu text");
    }
}

fn smoke_screenshot_path() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == OsStr::new("--smoke-screenshot") {
            return args.next().map(PathBuf::from);
        }
        if let Some(path) = arg
            .to_str()
            .and_then(|arg| arg.strip_prefix("--smoke-screenshot="))
        {
            return Some(PathBuf::from(path));
        }
    }

    std::env::var_os("BATTLETRIS_SMOKE_SCREENSHOT").map(PathBuf::from)
}

fn request_smoke_screenshot(
    mut commands: Commands,
    mut smoke: ResMut<SmokeScreenshot>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if smoke.requested {
        smoke.frames_since_request = smoke.frames_since_request.saturating_add(1);
        if smoke.frames_since_request > SMOKE_SCREENSHOT_TIMEOUT_FRAMES {
            error!(
                "BattleTris smoke screenshot timed out before capture: {}",
                smoke.path.display()
            );
            app_exit.write(AppExit::error());
        }
        return;
    }

    if smoke.frames_until_capture > 0 {
        smoke.frames_until_capture -= 1;
        return;
    }

    let path = smoke.path.clone();
    info!("BattleTris smoke screenshot requested: {}", path.display());
    commands.spawn(Screenshot::primary_window()).observe(
        move |screenshot: On<ScreenshotCaptured>, mut app_exit: MessageWriter<AppExit>| {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    if let Err(error) = std::fs::create_dir_all(parent) {
                        error!(
                            "BattleTris smoke screenshot could not create {}: {error}",
                            parent.display()
                        );
                        app_exit.write(AppExit::error());
                        return;
                    }
                }
            }

            match screenshot.image.clone().try_into_dynamic() {
                Ok(image) => match image.to_rgb8().save(&path) {
                    Ok(()) => {
                        info!("BattleTris smoke screenshot saved: {}", path.display());
                        app_exit.write(AppExit::Success);
                    }
                    Err(error) => {
                        error!(
                            "BattleTris smoke screenshot could not save {}: {error}",
                            path.display()
                        );
                        app_exit.write(AppExit::error());
                    }
                },
                Err(error) => {
                    error!("BattleTris smoke screenshot could not decode captured image: {error}");
                    app_exit.write(AppExit::error());
                }
            }
        },
    );
    smoke.requested = true;
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
    game: &TwoPlayerGame,
    player: PlayerId,
    x: usize,
    y: usize,
    theme: ThemeChoice,
) -> Color {
    let piece_cell = game
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
    game.player(player).board().get(coord).map_or_else(
        || empty_cell_color(theme),
        |cell| cell_color(cell, false, theme),
    )
}

fn player_hud(game: &TwoPlayerGame, player: PlayerId) -> String {
    let loop_state = game.player(player);
    let mut text = format!(
        "score {}  funds {}  lines {}\nnext {}\narsenal {}\neffects {}",
        loop_state.score(),
        loop_state.funds(),
        loop_state.lines(),
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

fn phase_label(game: &TwoPlayerGame, settings: &ClientSettings, sound: &SoundEventState) -> String {
    let sound_label = sound
        .last_event
        .map(|event| format!("  sound {:?}", event))
        .unwrap_or_default();
    let mode = match game.mode() {
        GameMode::HumanVsHuman => "human vs human",
        GameMode::HumanVsComputer { .. } => "human vs computer (unranked)",
    };
    let common = format!("F1 menu  F2 challenge  F3 settings  Esc game  {mode}{sound_label}");

    match game.phase() {
        GamePhase::Playing => format!(
            "{}\nP pause  R restart  controls {}  1-0 launch P1, Shift+1-0 launch P2",
            common,
            controls_label(settings.controls)
        ),
        GamePhase::Paused => format!("{common}\npaused  P resume  R restart"),
        GamePhase::Bazaar => format!(
            "{}\nbazaar  A/S/D/F/G buy, Shift+key P2  Enter P1 done  Space P2 done",
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

fn menu_label(game: &TwoPlayerGame, settings: &ClientSettings) -> String {
    match settings.screen {
        ClientScreen::Startup => "BattleTris\nH local human-vs-human\nC unranked human-vs-computer\nF2 challenge placeholder  F3 settings  F4 about  F5 roster  F6 sleep".to_string(),
        ClientScreen::Game => String::new(),
        ClientScreen::Challenge => "Challenge\nDirect-connect challenge setup is not wired into this client yet.\nEsc returns to the active local game.".to_string(),
        ClientScreen::Sleep => "Sleep\nPlaceholder for legacy sleep/biff presence behavior.\nEsc returns to the active local game.".to_string(),
        ClientScreen::About => "About\nBattleTris Rust/Bevy rewrite. Core rules are deterministic Rust; this client is an adapter.\nEsc returns to the active local game.".to_string(),
        ClientScreen::Roster => roster_label(game),
        ClientScreen::Settings => format!(
            "Settings\nT theme: {:?}\nO sound pack: {:?}\nM controls: {}\n-/= scale: {:.2}x\nassets: {}\nsettings: {}\nEsc returns to game",
            settings.theme,
            settings.sound_pack,
            controls_label(settings.controls),
            settings.pixel_scale,
            settings.assets_dir.display(),
            settings
                .settings_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "unavailable".to_string()),
        ),
    }
}

fn sanitize_pixel_scale(pixel_scale: f32) -> f32 {
    if pixel_scale.is_finite() {
        pixel_scale.clamp(0.75, 2.0)
    } else {
        1.0
    }
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

fn roster_label(game: &TwoPlayerGame) -> String {
    let ranked = if game.is_ranked_game() {
        "ranked"
    } else {
        "unranked"
    };
    format!(
        "Roster\nPlayer 1: local human\nPlayer 2: {}\nCurrent game is {ranked}. Persistent records are not shown here.\nEsc returns to game.",
        match game.mode() {
            GameMode::HumanVsHuman => "local human",
            GameMode::HumanVsComputer { .. } => "computer Ernie",
        }
    )
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
    bazaar_buy_keys()
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

fn cell_color(cell: Cell, active: bool, theme: ThemeChoice) -> Color {
    if theme == ThemeChoice::HighContrast {
        return match cell {
            Cell::Visible { color } => {
                let hue = (color.get() as f32 % 7.0) / 7.0;
                Color::hsl(360.0 * hue, 0.92, if active { 0.74 } else { 0.56 })
            }
            Cell::Structure => Color::srgb(0.8, 0.82, 0.86),
            Cell::Happy => Color::srgb(1.0, 0.92, 0.0),
            Cell::Frown => Color::srgb(0.82, 0.47, 0.16),
            Cell::Gimp { .. } => Color::srgb(1.0, 0.1, 0.9),
            Cell::Die { .. } => Color::srgb(0.78, 0.84, 1.0),
            Cell::Invisible => Color::srgba(0.2, 0.22, 0.26, 0.35),
            Cell::Hidden { .. } => Color::srgb(0.0, 0.0, 0.0),
        };
    }

    match cell {
        Cell::Visible { color } => {
            let hue = (color.get() as f32 % 7.0) / 7.0;
            Color::hsl(360.0 * hue, 0.62, if active { 0.66 } else { 0.48 })
        }
        Cell::Structure => Color::srgb(0.46, 0.47, 0.49),
        Cell::Happy => Color::srgb(0.97, 0.79, 0.22),
        Cell::Frown => Color::srgb(0.5, 0.42, 0.34),
        Cell::Gimp { .. } => Color::srgb(0.78, 0.18, 0.73),
        Cell::Die { pip } => {
            let lightness = 0.34 + f32::from(pip.get()) * 0.055;
            Color::srgb(lightness, lightness, 0.93)
        }
        Cell::Invisible => Color::srgba(0.09, 0.1, 0.11, 0.22),
        Cell::Hidden { .. } => Color::srgb(0.025, 0.028, 0.032),
    }
}

fn empty_cell_color(theme: ThemeChoice) -> Color {
    match theme {
        ThemeChoice::OriginalInspired => Color::srgb(0.075, 0.085, 0.095),
        ThemeChoice::HighContrast => Color::srgb(0.015, 0.018, 0.022),
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
        BattleEvent::WeaponLaunched { .. }
        | BattleEvent::OneShotWeaponApplied { .. }
        | BattleEvent::TimedWeaponActivated { .. } => Some(SoundEvent::WeaponLaunch),
        BattleEvent::PlayerDied { .. } | BattleEvent::GameOver { .. } => Some(SoundEvent::GameOver),
        BattleEvent::Paused | BattleEvent::Resumed => Some(SoundEvent::MenuAction),
        _ => None,
    }
}

fn cell_x(left: f32, x: usize) -> f32 {
    left + x as f32 * CELL_SIZE + CELL_SIZE / 2.0
}

fn cell_y(y: usize) -> f32 {
    BOARD_TOP - y as f32 * CELL_SIZE - CELL_SIZE / 2.0
}

const fn opponent_player(player: PlayerId) -> PlayerId {
    match player {
        PlayerId::One => PlayerId::Two,
        PlayerId::Two => PlayerId::One,
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

fn bazaar_buy_keys() -> [(WeaponToken, KeyCode); 5] {
    [
        (WeaponToken::FlipOut, KeyCode::KeyA),
        (WeaponToken::Gimp, KeyCode::KeyS),
        (WeaponToken::Missing, KeyCode::KeyD),
        (WeaponToken::RiseUp, KeyCode::KeyF),
        (WeaponToken::NiceDay, KeyCode::KeyG),
    ]
}

fn computer_bazaar_tokens() -> [WeaponToken; 5] {
    [
        WeaponToken::NiceDay,
        WeaponToken::Missing,
        WeaponToken::RiseUp,
        WeaponToken::FlipOut,
        WeaponToken::Gimp,
    ]
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
    fn hud_mentions_core_state_and_preview() {
        let game = TwoPlayerGame::new(GameSeed::from_u64(1), GameSeed::from_u64(2));
        let hud = player_hud(&game, PlayerId::One);

        assert!(hud.contains("score 0"));
        assert!(hud.contains("funds 0"));
        assert!(hud.contains("next "));
        assert!(hud.contains("arsenal empty"));
    }

    #[test]
    fn bazaar_keys_are_affordable_intro_weapons() {
        for (token, _) in bazaar_buy_keys() {
            assert!(battletris_core::weapons::weapon_spec(token).price <= 100);
        }
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
        let local = LocalGame::new_human_vs_computer();

        assert!(!local.game.is_ranked_game());
        assert!(local.computer.is_some());
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
        };

        let encoded = toml::to_string_pretty(&settings).expect("settings encode");
        let decoded: PersistedClientSettings = toml::from_str(&encoded).expect("settings decode");

        assert_eq!(decoded, settings);
        assert!(encoded.contains("high-contrast"));
        assert!(encoded.contains("legacy-inspired"));
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
    }
}
