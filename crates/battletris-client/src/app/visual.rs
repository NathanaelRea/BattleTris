//! Deterministic visual fixtures and screenshot capture.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VisualFixture {
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
    pub(super) const ALL: [Self; 11] = [
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

    pub(super) const fn id(self) -> &'static str {
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

    pub(super) fn from_id(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|fixture| fixture.id() == value)
    }

    pub(super) const fn screen(self) -> ClientScreen {
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
pub(super) enum VisualCaptureSpec {
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
    pub(super) fn to_capture(
        &self,
        themes: &ThemePacks,
        default_theme: ThemeChoice,
    ) -> VisualCapture {
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
pub(super) struct VisualCapture {
    pub(super) jobs: Vec<VisualCaptureJob>,
    pub(super) current: usize,
    pub(super) applied: Option<usize>,
    pub(super) frames_until_capture: u16,
    pub(super) frames_since_request: u16,
    pub(super) requested: bool,
}

impl VisualCapture {
    pub(super) fn new(jobs: Vec<VisualCaptureJob>) -> Self {
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
pub(super) struct VisualCaptureJob {
    pub(super) fixture: VisualFixture,
    pub(super) theme: ThemeChoice,
    pub(super) path: PathBuf,
    pub(super) expected_width: u32,
    pub(super) expected_height: u32,
}

pub(super) fn visual_capture_job(
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

pub(super) fn apply_visual_fixture_state(
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
    settings.lobby_addr = DEFAULT_LOBBY_ADDR.to_string();

    if fixture == VisualFixture::Challenge {
        settings.ernie_level = 0;
    }

    *local = visual_local_game(fixture, settings.ernie_level);
    *recon = visual_recon_panel(fixture, local);
    *bazaar_ui = visual_bazaar_ui(fixture);
    *roster = visual_roster_records();
}

pub(super) fn visual_local_game(fixture: VisualFixture, ernie_level: usize) -> LocalGame {
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

pub(super) fn visual_computer_game(
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
        mode: LocalGameMode::ComputerOpponent,
        network_session: None,
        network_lockstep: None,
        network_failed_closed: false,
        network_game_over_sent: false,
        network_result_claim_submitted: false,
        status_message: Some(status_message.to_string()),
    }
}

pub(super) fn visual_bazaar_game() -> LocalGame {
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
        mode: LocalGameMode::LocalHumanVsHuman,
        network_session: None,
        network_lockstep: None,
        network_failed_closed: false,
        network_game_over_sent: false,
        network_result_claim_submitted: false,
        status_message: Some("Visual fixture: bazaar shopping".to_string()),
    }
}

pub(super) fn visual_game_over_game() -> LocalGame {
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
        mode: LocalGameMode::LocalHumanVsHuman,
        network_session: None,
        network_lockstep: None,
        network_failed_closed: false,
        network_game_over_sent: false,
        network_result_claim_submitted: false,
        status_message: None,
    }
}

pub(super) fn visual_recon_panel(fixture: VisualFixture, local: &LocalGame) -> ReconPanel {
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

pub(super) fn visual_bazaar_ui(fixture: VisualFixture) -> BazaarUiState {
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

pub(super) fn visual_roster_records() -> RosterRecords {
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

pub(super) fn visual_playing_board() -> Board {
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

pub(super) fn visual_opponent_board() -> Board {
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

pub(super) fn visual_board_cells_board() -> Board {
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

pub(super) fn visible_cell(color: u8) -> Cell {
    Cell::visible_with_color(VisibleColor::new(color).expect("fixture color in legacy range"))
}

pub(super) fn apply_visual_capture_fixture(
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

pub(super) fn request_visual_capture(
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

pub(super) fn save_visual_capture(
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

pub(super) fn validate_visual_capture_pixels(
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

pub(super) fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))
}
