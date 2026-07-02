//! Headless single-board falling-piece tick loop for deterministic simulations.
//!
//! It owns command/tick timing, line clears, score/funds updates, and events that
//! adapters can render, send, or record without coupling core logic to those adapters.

use crate::{
    ai::ComputerDifficulty,
    board::{Board, LineClearMode, BOARD_HEIGHT},
    cell::Cell,
    piece::{Piece, PieceKind},
    piece_generator::PieceGenerator,
    recon::{sample_recon, ReconLevel, ReconSnapshot},
    rng::{GameSeed, RngStream},
    score::{BazaarTracker, PlayerScore},
    weapons::{
        is_phase_10_timed_weapon, is_timed_weapon, mirror_nullifies, ActiveEffects, Arsenal,
        ArsenalError, Bazaar, BazaarError, WeaponToken,
    },
};
use rand::Rng;
use rand_chacha::ChaCha12Rng;
use std::collections::VecDeque;

/// Legacy default fast-drop interval in milliseconds.
pub const DEFAULT_FAST_DROP_MS: u64 = 10;
/// Legacy default automatic drop interval in milliseconds.
pub const DEFAULT_DROP_MS: u64 = 512;
/// Legacy default slide grace interval in milliseconds.
pub const DEFAULT_SLIDE_MS: u64 = 150;

/// Input commands accepted by the headless core loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Move one cell left immediately if possible.
    MoveLeft,
    /// Move one cell right immediately if possible.
    MoveRight,
    /// Rotate forward immediately if possible.
    RotateClockwise,
    /// Rotate backward immediately if possible.
    RotateCounterClockwise,
    /// Start fast drop and award the one-time fast-drop score bump.
    StartFastDrop,
    /// Return to the default drop interval.
    StopFastDrop,
}

/// Core events emitted by command/tick handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreEvent {
    /// A piece became active.
    PieceSpawned {
        /// Spawned piece kind.
        kind: PieceKind,
    },
    /// The active piece moved to a new anchor.
    PieceMoved {
        /// New board anchor.
        anchor: (isize, isize),
    },
    /// The active piece rotated.
    PieceRotated {
        /// New rotation state.
        orientation: u8,
    },
    /// A downward drop was blocked and slide grace began.
    PieceLanded,
    /// The active piece locked into the board.
    PieceLocked {
        /// Locked piece kind.
        kind: PieceKind,
    },
    /// A piece could not spawn because its spawn cells were occupied.
    SpawnFailed {
        /// Blocked piece kind.
        kind: PieceKind,
    },
    /// Fast drop started and awarded score.
    FastDropStarted {
        /// Score awarded by fast-drop start.
        score_delta: i32,
    },
    /// One or more lines cleared after a piece lock.
    LinesCleared {
        /// Number of lines cleared.
        lines: u32,
        /// Funds awarded by dice/happy/gimp/hidden values.
        funds: i32,
    },
    /// Happy cells became frowns because they were missed in non-full rows.
    HappyMissed {
        /// Number of happy cells converted.
        count: u32,
    },
}

/// Failure from bazaar shopping through the two-player session API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShoppingError {
    /// Shopping is only allowed while the bazaar is open.
    BazaarClosed,
    /// This player has already clicked Done and is waiting for the opponent.
    PlayerDone,
    /// The staged bazaar operation failed.
    Bazaar(BazaarError),
}

/// Failure while launching a weapon from an arsenal slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchError {
    /// Weapons can only launch while actively playing.
    NotPlaying,
    /// The numbered arsenal slot could not be consumed.
    Arsenal(ArsenalError),
    /// The selected weapon is neither a Phase 8 one-shot nor a Phase 9 timed weapon.
    UnsupportedWeapon(WeaponToken),
}

impl From<ArsenalError> for LaunchError {
    fn from(value: ArsenalError) -> Self {
        Self::Arsenal(value)
    }
}

impl From<BazaarError> for ShoppingError {
    fn from(value: BazaarError) -> Self {
        Self::Bazaar(value)
    }
}

/// Stable player slot used by local, network, replay, and test adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerId {
    /// First player slot.
    One,
    /// Second player slot.
    Two,
}

impl PlayerId {
    const fn opponent(self) -> Self {
        match self {
            Self::One => Self::Two,
            Self::Two => Self::One,
        }
    }
}

/// Session-level state for the two-player headless game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    /// Both piece loops are accepting commands and ticks.
    Playing,
    /// Time is stopped until resumed.
    Paused,
    /// Bazaar is open; both players must leave before play resumes.
    Bazaar,
    /// One player has died and the match is complete.
    GameOver,
}

/// Whether a BattleTris Game result is eligible for ranked persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    /// Both players are human-controlled; ranked adapters may submit the result.
    HumanVsHuman,
    /// One player is a deterministic computer opponent; legacy mode is unranked.
    HumanVsComputer {
        /// Computer-controlled player slot.
        computer: PlayerId,
        /// Computer opponent difficulty row.
        difficulty: ComputerDifficulty,
    },
}

impl GameMode {
    /// Returns whether this game mode may write ranked results.
    #[must_use]
    pub const fn is_ranked(self) -> bool {
        matches!(self, Self::HumanVsHuman)
    }
}

/// Deterministic event boundary consumed by renderers, audio, networking, replays, and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BattleEvent {
    /// A new two-player match started.
    GameStarted,
    /// The match was paused.
    Paused,
    /// A paused match resumed.
    Resumed,
    /// A player entered or changed their single-board piece loop.
    PlayerEvent {
        /// Player that owns the event.
        player: PlayerId,
        /// Single-board event.
        event: CoreEvent,
    },
    /// The shared bazaar threshold was reached.
    BazaarEntered,
    /// A player committed and is waiting for the other player to finish bazaar.
    BazaarPlayerDone {
        /// Player that finished shopping.
        player: PlayerId,
    },
    /// Both players finished bazaar and play resumed.
    BazaarLeft,
    /// A spawn failure killed one player and ended the game.
    PlayerDied {
        /// Player that died.
        player: PlayerId,
    },
    /// Final game-over result.
    GameOver {
        /// Winning player.
        winner: PlayerId,
        /// Losing player.
        loser: PlayerId,
    },
    /// A player consumed an arsenal slot to launch a weapon.
    WeaponLaunched {
        /// Launching player.
        launcher: PlayerId,
        /// Target player.
        target: PlayerId,
        /// Weapon token.
        token: WeaponToken,
    },
    /// A zero-duration weapon effect was applied immediately.
    OneShotWeaponApplied {
        /// Launching player.
        launcher: PlayerId,
        /// Target player.
        target: PlayerId,
        /// Weapon token.
        token: WeaponToken,
    },
    /// A line-duration weapon became active on a player.
    TimedWeaponActivated {
        /// Launching player.
        launcher: PlayerId,
        /// Affected player.
        target: PlayerId,
        /// Weapon token.
        token: WeaponToken,
        /// Remaining target-player lines after stacking.
        remaining_lines: u32,
    },
    /// A line-duration weapon expired after target-player line clears.
    TimedWeaponExpired {
        /// Affected player.
        player: PlayerId,
        /// Weapon token.
        token: WeaponToken,
    },
    /// A launched weapon was queued for the target's post-placement flush.
    IncomingWeaponQueued {
        /// Original launching player.
        launcher: PlayerId,
        /// Player whose incoming FIFO received the launch.
        target: PlayerId,
        /// Weapon token.
        token: WeaponToken,
    },
    /// Mirror reflected a launch back onto the launching player.
    WeaponReflected {
        /// Player whose active Mirror reflected the launch.
        player: PlayerId,
        /// Weapon token.
        token: WeaponToken,
    },
    /// Mirror consumed and nullified a launch without applying it.
    WeaponNullified {
        /// Player whose active Mirror nullified the launch.
        player: PlayerId,
        /// Weapon token.
        token: WeaponToken,
    },
    /// A recon/spy panel received a deterministic target snapshot.
    ReconUpdated {
        /// Player viewing the recon panel.
        viewer: PlayerId,
        /// Player whose board/funds were sampled.
        target: PlayerId,
        /// Sampled recon data.
        snapshot: ReconSnapshot,
    },
    /// A spy/recon panel was shut down after its line duration expired.
    ReconDisabled {
        /// Player whose recon panel closed.
        viewer: PlayerId,
        /// Player who had been observed.
        target: PlayerId,
        /// Expired spy weapon token.
        token: WeaponToken,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QueuedWeapon {
    launcher: PlayerId,
    token: WeaponToken,
}

/// One entry in the deterministic session event log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedEvent {
    /// Zero-based event index in emission order.
    pub sequence: u64,
    /// Session event payload.
    pub event: BattleEvent,
}

/// Configurable tick timings for one board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimingConfig {
    /// Fast-drop interval in milliseconds.
    pub fast_drop_ms: u64,
    /// Default automatic drop interval in milliseconds.
    pub drop_ms: u64,
    /// Slide grace interval in milliseconds.
    pub slide_ms: u64,
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            fast_drop_ms: DEFAULT_FAST_DROP_MS,
            drop_ms: DEFAULT_DROP_MS,
            slide_ms: DEFAULT_SLIDE_MS,
        }
    }
}

/// Headless single-board state through piece movement and locking.
#[derive(Debug, Clone)]
pub struct PieceLoop {
    board: Board,
    generator: PieceGenerator,
    weapon_rng: ChaCha12Rng,
    active: Option<Piece>,
    timing: TimingConfig,
    drop_elapsed_ms: u64,
    slide_elapsed_ms: Option<u64>,
    fast_drop: bool,
    fast_drop_scored: bool,
    score: PlayerScore,
    arsenal: Arsenal,
    active_effects: ActiveEffects,
    game_over: bool,
    pending_spawn: bool,
    slick_direction: isize,
}

impl PieceLoop {
    /// Creates a loop with an empty board and immediately spawns the first piece.
    #[must_use]
    pub fn new(seed: GameSeed) -> (Self, Vec<CoreEvent>) {
        Self::with_board(seed, Board::empty())
    }

    /// Creates a loop with an explicit board and immediately spawns the first piece.
    #[must_use]
    pub fn with_board(seed: GameSeed, board: Board) -> (Self, Vec<CoreEvent>) {
        let mut game = Self {
            board,
            generator: PieceGenerator::new(seed),
            weapon_rng: seed.stream(RngStream::WeaponEffects),
            active: None,
            timing: TimingConfig::default(),
            drop_elapsed_ms: 0,
            slide_elapsed_ms: None,
            fast_drop: false,
            fast_drop_scored: false,
            score: PlayerScore::default(),
            arsenal: Arsenal::new(),
            active_effects: ActiveEffects::new(),
            game_over: false,
            pending_spawn: false,
            slick_direction: 1,
        };
        let events = game.spawn_next_piece();
        (game, events)
    }

    /// Returns the board.
    #[must_use]
    pub const fn board(&self) -> &Board {
        &self.board
    }

    /// Returns the active falling piece, if any.
    #[must_use]
    pub const fn active_piece(&self) -> Option<&Piece> {
        self.active.as_ref()
    }

    /// Returns the next generated piece kind without advancing this loop.
    #[must_use]
    pub fn next_piece_kind_preview(&self) -> PieceKind {
        let mut generator = self.generator.clone();
        generator.next_piece().kind()
    }

    /// Returns the current score.
    #[must_use]
    pub const fn score(&self) -> i32 {
        self.score.score()
    }

    /// Returns the current funds.
    #[must_use]
    pub const fn funds(&self) -> i32 {
        self.score.funds()
    }

    /// Returns the player's arsenal.
    #[must_use]
    pub const fn arsenal(&self) -> &Arsenal {
        &self.arsenal
    }

    /// Returns the active timed weapon state for this player.
    #[must_use]
    pub const fn active_effects(&self) -> &ActiveEffects {
        &self.active_effects
    }

    /// Returns total lines cleared.
    #[must_use]
    pub const fn lines(&self) -> u32 {
        self.score.lines()
    }

    /// Returns whether spawn failure has ended this loop.
    #[must_use]
    pub const fn is_game_over(&self) -> bool {
        self.game_over
    }

    /// Returns mutable access to the piece generator for deterministic scenarios.
    pub fn generator_mut(&mut self) -> &mut PieceGenerator {
        &mut self.generator
    }

    fn set_board(&mut self, board: Board) {
        self.board = board;
    }

    fn set_funds(&mut self, funds: i32) {
        self.score.set_funds(funds);
    }

    fn add_funds(&mut self, funds: i32) {
        self.score.add_funds(funds);
    }

    /// Applies one command and returns emitted events.
    pub fn command(&mut self, command: Command) -> Vec<CoreEvent> {
        if self.game_over {
            return Vec::new();
        }

        match command {
            Command::MoveLeft => self.move_active(self.horizontal_step(-1), 0),
            Command::MoveRight => self.move_active(self.horizontal_step(1), 0),
            Command::RotateClockwise => self.rotate_active(false),
            Command::RotateCounterClockwise => self.rotate_active(true),
            Command::StartFastDrop => self.start_fast_drop(),
            Command::StopFastDrop => {
                self.fast_drop = false;
                self.fast_drop_scored = false;
                Vec::new()
            }
        }
    }

    /// Advances time by a deterministic number of milliseconds.
    pub fn tick(&mut self, elapsed_ms: u64) -> Vec<CoreEvent> {
        self.tick_with_spawn_mode(elapsed_ms, true)
    }

    fn tick_deferred_spawn(&mut self, elapsed_ms: u64) -> Vec<CoreEvent> {
        self.tick_with_spawn_mode(elapsed_ms, false)
    }

    fn tick_with_spawn_mode(
        &mut self,
        elapsed_ms: u64,
        auto_spawn_after_lock: bool,
    ) -> Vec<CoreEvent> {
        if self.game_over {
            return Vec::new();
        }

        let mut events = Vec::new();
        let slide_was_active = self.slide_elapsed_ms.is_some();
        let mut interval = if self.fast_drop {
            self.timing.fast_drop_ms
        } else {
            self.timing.drop_ms
        };
        if self.active_effects.is_active(WeaponToken::Speedy) {
            interval = (interval / 2).max(1);
        }
        if self.active_effects.is_active(WeaponToken::Meadow) {
            interval *= 2;
        }

        self.drop_elapsed_ms += elapsed_ms;
        while self.drop_elapsed_ms >= interval && !self.game_over && !self.pending_spawn {
            self.drop_elapsed_ms -= interval;
            events.extend(self.drop_or_start_slide());
        }

        if self.active_effects.is_active(WeaponToken::Hatter) {
            events.extend(self.rotate_active(false));
        }
        if self.active_effects.is_active(WeaponToken::Slick) {
            events.extend(self.slick_step());
        }

        if let Some(slide_elapsed) = self.slide_elapsed_ms.as_mut().filter(|_| slide_was_active) {
            *slide_elapsed += elapsed_ms;
            let slide_ms = if self.active_effects.is_active(WeaponToken::NoSlide) {
                0
            } else {
                self.timing.slide_ms
            };
            if *slide_elapsed >= slide_ms {
                events.extend(self.expire_slide(auto_spawn_after_lock));
            }
        }

        events
    }

    fn spawn_deferred_piece(&mut self) -> Vec<CoreEvent> {
        if !self.pending_spawn || self.game_over {
            return Vec::new();
        }

        self.pending_spawn = false;
        self.spawn_next_piece()
    }

    fn move_active(&mut self, dx: isize, dy: isize) -> Vec<CoreEvent> {
        let Some(piece) = self.active.as_mut() else {
            return Vec::new();
        };
        let (x, y) = piece.anchor();
        let fallout = self.active_effects.is_active(WeaponToken::FallOut);
        if !piece.move_to_with_fallout(&self.board, x + dx, y + dy, fallout) {
            return Vec::new();
        }

        if dy != 0 {
            self.slide_elapsed_ms = None;
        }

        vec![CoreEvent::PieceMoved {
            anchor: piece.anchor(),
        }]
    }

    fn rotate_active(&mut self, reverse: bool) -> Vec<CoreEvent> {
        let Some(piece) = self.active.as_mut() else {
            return Vec::new();
        };
        if !piece.rotate(&self.board, reverse) {
            return Vec::new();
        }

        vec![CoreEvent::PieceRotated {
            orientation: piece.orientation(),
        }]
    }

    fn start_fast_drop(&mut self) -> Vec<CoreEvent> {
        self.fast_drop = true;
        if self.fast_drop_scored {
            return Vec::new();
        }

        let Some(piece) = self.active.as_ref() else {
            return Vec::new();
        };
        let score_delta = (BOARD_HEIGHT as isize - piece.anchor().1).max(0) as i32;
        self.score.add_score(score_delta);
        self.fast_drop_scored = true;
        vec![CoreEvent::FastDropStarted { score_delta }]
    }

    fn drop_or_start_slide(&mut self) -> Vec<CoreEvent> {
        if let Some(piece) = self.active.as_ref() {
            let (x, y) = piece.anchor();
            let dy = self.gravity_step();
            if piece.can_move_to_with_fallout(&self.board, x, y + dy, self.fallout_active()) {
                return self.move_active(0, dy);
            }
        }

        if self.slide_elapsed_ms.is_none() {
            self.slide_elapsed_ms = Some(0);
            vec![CoreEvent::PieceLanded]
        } else {
            Vec::new()
        }
    }

    fn expire_slide(&mut self, auto_spawn_after_lock: bool) -> Vec<CoreEvent> {
        self.slide_elapsed_ms = None;
        if let Some(piece) = self.active.as_ref() {
            let (x, y) = piece.anchor();
            let dy = self.gravity_step();
            if piece.can_move_to_with_fallout(&self.board, x, y + dy, self.fallout_active()) {
                return self.move_active(0, dy);
            }
        }

        self.lock_active(auto_spawn_after_lock)
    }

    fn lock_active(&mut self, auto_spawn_after_lock: bool) -> Vec<CoreEvent> {
        let Some(piece) = self.active.take() else {
            return Vec::new();
        };
        let kind = piece.kind();
        if piece
            .board_coords()
            .iter()
            .all(|&(_, y)| y < BOARD_HEIGHT as isize)
        {
            piece.lock_into(&mut self.board);
        }
        self.fast_drop = false;
        self.fast_drop_scored = false;
        self.drop_elapsed_ms = 0;

        let mut events = vec![CoreEvent::PieceLocked { kind }];
        let clear_mode = if self.active_effects.is_active(WeaponToken::Force) {
            LineClearMode::Force
        } else {
            LineClearMode::Normal
        };
        let mut outcome = self.board.clear_completed_lines(clear_mode);
        if self.active_effects.is_active(WeaponToken::Mondale) {
            outcome.funds = outcome.funds * 70 / 100;
        }
        self.score.apply_line_clear(&outcome);
        if outcome.funds != 0 || outcome.lines_cleared != 0 {
            events.push(CoreEvent::LinesCleared {
                lines: outcome.lines_cleared,
                funds: outcome.funds,
            });
        }
        if outcome.happy_missed != 0 {
            events.push(CoreEvent::HappyMissed {
                count: outcome.happy_missed,
            });
        }
        if auto_spawn_after_lock {
            events.extend(self.spawn_next_piece());
        } else {
            self.pending_spawn = true;
        }
        events
    }

    fn spawn_next_piece(&mut self) -> Vec<CoreEvent> {
        let mut piece = self.generator.next_piece();
        if self.active_effects.is_active(WeaponToken::Upbyside) {
            let (x, _) = piece.anchor();
            let y = BOARD_HEIGHT as isize - 1 - isize::from(piece.max_local_y());
            piece.set_anchor(x, y);
        }
        let kind = piece.kind();
        if !piece.can_move_to(&self.board, piece.anchor().0, piece.anchor().1) {
            self.game_over = true;
            self.active = None;
            return vec![CoreEvent::SpawnFailed { kind }];
        }

        self.active = Some(piece);
        vec![CoreEvent::PieceSpawned { kind }]
    }

    fn activate_timed_effect(&mut self, token: WeaponToken) -> u32 {
        let remaining = self.active_effects.activate(token);
        match token {
            WeaponToken::FearedWeird => self
                .generator
                .probabilities_mut()
                .set_weird_pieces_enabled(true),
            WeaponToken::FourByFour => self
                .generator
                .probabilities_mut()
                .set_four_by_four_enabled(true),
            WeaponToken::SoLong => self
                .generator
                .probabilities_mut()
                .set_long_pieces_enabled(false),
            WeaponToken::NoDice => self.generator.probabilities_mut().set_dice_enabled(false),
            WeaponToken::Broken => self.generator.set_broken_record_enabled(true),
            WeaponToken::Bottle => self.board.add_bottle_neck(),
            WeaponToken::FallOut => self.board.clear_fallout_hole(),
            WeaponToken::Upbyside => self.board.flip_on_horizontal_axis(),
            _ => {}
        }
        remaining
    }

    fn expire_timed_effect(&mut self, token: WeaponToken) {
        match token {
            WeaponToken::FearedWeird => self
                .generator
                .probabilities_mut()
                .set_weird_pieces_enabled(false),
            WeaponToken::FourByFour => self
                .generator
                .probabilities_mut()
                .set_four_by_four_enabled(false),
            WeaponToken::SoLong => self
                .generator
                .probabilities_mut()
                .set_long_pieces_enabled(true),
            WeaponToken::NoDice => self.generator.probabilities_mut().set_dice_enabled(true),
            WeaponToken::Broken => self.generator.set_broken_record_enabled(false),
            WeaponToken::Bottle => self.board.remove_bottle_neck(),
            WeaponToken::Upbyside => self.board.flip_on_horizontal_axis(),
            _ => {}
        }
    }

    fn observe_effect_line_clear(&mut self, lines: u32) -> Vec<WeaponToken> {
        let expired = self.active_effects.observe_line_clear(lines);
        for token in &expired {
            self.expire_timed_effect(*token);
        }
        expired
    }

    const fn fallout_active(&self) -> bool {
        self.active_effects.is_active(WeaponToken::FallOut)
    }

    fn gravity_step(&self) -> isize {
        if self.active_effects.is_active(WeaponToken::Upbyside) {
            -1
        } else {
            1
        }
    }

    fn slick_step(&mut self) -> Vec<CoreEvent> {
        let first = self.move_active(self.slick_direction, 0);
        if !first.is_empty() {
            return first;
        }

        self.slick_direction = -self.slick_direction;
        self.move_active(self.slick_direction, 0)
    }

    fn horizontal_step(&self, step: isize) -> isize {
        if self.active_effects.is_active(WeaponToken::Upbyside) {
            -step
        } else {
            step
        }
    }
}

/// Headless two-player BattleTris session without weapon effects.
#[derive(Debug, Clone)]
pub struct TwoPlayerGame {
    player_one: PieceLoop,
    player_two: PieceLoop,
    phase: GamePhase,
    before_pause: Option<GamePhase>,
    bazaar: BazaarTracker,
    bazaar_done: [bool; 2],
    bazaar_sessions: [Option<Bazaar>; 2],
    incoming_weapons: [VecDeque<QueuedWeapon>; 2],
    cached_opponent_funds: [i32; 2],
    mode: GameMode,
    log: Vec<LoggedEvent>,
}

impl TwoPlayerGame {
    /// Starts a two-player game with empty boards.
    #[must_use]
    pub fn new(player_one_seed: GameSeed, player_two_seed: GameSeed) -> Self {
        Self::with_boards(
            player_one_seed,
            Board::empty(),
            player_two_seed,
            Board::empty(),
        )
    }

    /// Starts a two-player game with explicit boards for scripted simulations.
    #[must_use]
    pub fn with_boards(
        player_one_seed: GameSeed,
        player_one_board: Board,
        player_two_seed: GameSeed,
        player_two_board: Board,
    ) -> Self {
        Self::with_boards_and_mode(
            player_one_seed,
            player_one_board,
            player_two_seed,
            player_two_board,
            GameMode::HumanVsHuman,
        )
    }

    /// Starts an unranked human-vs-computer game with explicit boards.
    #[must_use]
    pub fn human_vs_computer(
        human_seed: GameSeed,
        human_board: Board,
        computer_seed: GameSeed,
        computer_board: Board,
        computer: PlayerId,
        difficulty: ComputerDifficulty,
    ) -> Self {
        let (player_one_seed, player_one_board, player_two_seed, player_two_board) = match computer
        {
            PlayerId::One => (computer_seed, computer_board, human_seed, human_board),
            PlayerId::Two => (human_seed, human_board, computer_seed, computer_board),
        };
        Self::with_boards_and_mode(
            player_one_seed,
            player_one_board,
            player_two_seed,
            player_two_board,
            GameMode::HumanVsComputer {
                computer,
                difficulty,
            },
        )
    }

    fn with_boards_and_mode(
        player_one_seed: GameSeed,
        player_one_board: Board,
        player_two_seed: GameSeed,
        player_two_board: Board,
        mode: GameMode,
    ) -> Self {
        let (player_one, player_one_events) =
            PieceLoop::with_board(player_one_seed, player_one_board);
        let (player_two, player_two_events) =
            PieceLoop::with_board(player_two_seed, player_two_board);
        let mut game = Self {
            player_one,
            player_two,
            phase: GamePhase::Playing,
            before_pause: None,
            bazaar: BazaarTracker::new(),
            bazaar_done: [false, false],
            bazaar_sessions: [None, None],
            incoming_weapons: [VecDeque::new(), VecDeque::new()],
            cached_opponent_funds: [0, 0],
            mode,
            log: Vec::new(),
        };

        game.record(BattleEvent::GameStarted);
        game.record_player_events(PlayerId::One, player_one_events);
        game.record_player_events(PlayerId::Two, player_two_events);
        game.detect_startup_deaths();
        game
    }

    /// Returns the control/ranking mode for this BattleTris Game.
    #[must_use]
    pub const fn mode(&self) -> GameMode {
        self.mode
    }

    /// Returns whether adapters may submit this result to ranked persistence.
    #[must_use]
    pub const fn is_ranked_game(&self) -> bool {
        self.mode.is_ranked()
    }

    /// Returns the current session phase.
    #[must_use]
    pub const fn phase(&self) -> GamePhase {
        self.phase
    }

    /// Returns the complete deterministic event log.
    #[must_use]
    pub fn event_log(&self) -> &[LoggedEvent] {
        &self.log
    }

    /// Returns one player's board loop.
    #[must_use]
    pub const fn player(&self, player: PlayerId) -> &PieceLoop {
        match player {
            PlayerId::One => &self.player_one,
            PlayerId::Two => &self.player_two,
        }
    }

    /// Pauses playing or bazaar state. Repeated pauses are ignored.
    pub fn pause(&mut self) -> Vec<LoggedEvent> {
        if matches!(self.phase, GamePhase::Paused | GamePhase::GameOver) {
            return Vec::new();
        }

        self.before_pause = Some(self.phase);
        self.phase = GamePhase::Paused;
        self.record_one(BattleEvent::Paused)
    }

    /// Resumes the phase that was active before pause.
    pub fn resume(&mut self) -> Vec<LoggedEvent> {
        if self.phase != GamePhase::Paused {
            return Vec::new();
        }

        self.phase = self.before_pause.take().unwrap_or(GamePhase::Playing);
        self.record_one(BattleEvent::Resumed)
    }

    /// Applies a command to one player if the game is actively playing.
    pub fn command(&mut self, player: PlayerId, command: Command) -> Vec<LoggedEvent> {
        if self.phase != GamePhase::Playing {
            return Vec::new();
        }

        let events = self.player_mut(player).command(command);
        self.record_and_process_player_events(player, events)
    }

    /// Advances one player's clock if the game is actively playing.
    pub fn tick_player(&mut self, player: PlayerId, elapsed_ms: u64) -> Vec<LoggedEvent> {
        if self.phase != GamePhase::Playing {
            return Vec::new();
        }

        let events = self.player_mut(player).tick_deferred_spawn(elapsed_ms);
        self.record_and_process_player_events(player, events)
    }

    /// Marks a player done in bazaar. Play resumes when both players are done.
    pub fn bazaar_done(&mut self, player: PlayerId) -> Vec<LoggedEvent> {
        if self.phase != GamePhase::Bazaar {
            return Vec::new();
        }

        let index = player_index(player);
        if self.bazaar_done[index] {
            return Vec::new();
        }

        self.bazaar_done[index] = true;
        let mut emitted = self.record_one(BattleEvent::BazaarPlayerDone { player });
        if self.bazaar_done == [true, true] {
            self.commit_bazaar_sessions();
            self.bazaar_done = [false, false];
            self.bazaar_sessions = [None, None];
            self.phase = GamePhase::Playing;
            emitted.extend(self.record_one(BattleEvent::BazaarLeft));
        }
        emitted
    }

    /// Stages one bazaar purchase for a player.
    pub fn bazaar_buy(
        &mut self,
        player: PlayerId,
        token: WeaponToken,
    ) -> Result<usize, ShoppingError> {
        let bazaar = self.bazaar_session_mut(player)?;
        Ok(bazaar.buy(token)?)
    }

    /// Removes one newly staged bazaar purchase for a player.
    pub fn bazaar_remove_staged(
        &mut self,
        player: PlayerId,
        token: WeaponToken,
    ) -> Result<(), ShoppingError> {
        let bazaar = self.bazaar_session_mut(player)?;
        Ok(bazaar.remove_staged(token)?)
    }

    /// Returns a player's staged bazaar session while the bazaar is open.
    #[must_use]
    pub fn bazaar_session(&self, player: PlayerId) -> Option<&Bazaar> {
        self.bazaar_sessions[player_index(player)].as_ref()
    }

    /// Launches one arsenal slot and applies supported weapon effects.
    pub fn launch_weapon_slot(
        &mut self,
        launcher: PlayerId,
        slot_label: u8,
    ) -> Result<Vec<LoggedEvent>, LaunchError> {
        if self.phase != GamePhase::Playing {
            return Err(LaunchError::NotPlaying);
        }

        let token = self
            .player(launcher)
            .arsenal
            .token_for_slot_label(slot_label)?;
        if !is_supported_launch_weapon(token) {
            return Err(LaunchError::UnsupportedWeapon(token));
        }

        self.player_mut(launcher)
            .arsenal
            .consume_slot_label(slot_label)?;

        let start = self.log.len();
        let target = launcher.opponent();
        self.record(BattleEvent::WeaponLaunched {
            launcher,
            target,
            token,
        });
        self.resolve_launched_weapon(launcher, target, token);
        Ok(self.log[start..].to_vec())
    }

    /// Queues a weapon received from a peer for the target's next post-placement flush.
    pub fn queue_incoming_weapon(
        &mut self,
        launcher: PlayerId,
        target: PlayerId,
        token: WeaponToken,
    ) -> Vec<LoggedEvent> {
        if !is_supported_launch_weapon(token) {
            return Vec::new();
        }

        let start = self.log.len();
        self.incoming_weapons[player_index(target)].push_back(QueuedWeapon { launcher, token });
        self.record(BattleEvent::IncomingWeaponQueued {
            launcher,
            target,
            token,
        });
        self.log[start..].to_vec()
    }

    fn player_mut(&mut self, player: PlayerId) -> &mut PieceLoop {
        match player {
            PlayerId::One => &mut self.player_one,
            PlayerId::Two => &mut self.player_two,
        }
    }

    fn players_mut(
        &mut self,
        first: PlayerId,
        second: PlayerId,
    ) -> (&mut PieceLoop, &mut PieceLoop) {
        assert_ne!(first, second);
        match first {
            PlayerId::One => (&mut self.player_one, &mut self.player_two),
            PlayerId::Two => (&mut self.player_two, &mut self.player_one),
        }
    }

    fn apply_one_shot_weapon(
        &mut self,
        launcher_id: PlayerId,
        target_id: PlayerId,
        token: WeaponToken,
    ) {
        if token == WeaponToken::Keating {
            let transfer = self.cached_opponent_funds[player_index(launcher_id)];
            let (launcher, target) = self.players_mut(launcher_id, target_id);
            target.set_funds(0);
            launcher.add_funds(transfer);
            return;
        }

        if launcher_id == target_id {
            let target = self.player_mut(target_id);
            match token {
                WeaponToken::RiseUp => {
                    let hole = target.weapon_rng.next_u64() as usize;
                    target.board.insert_garbage_line(hole);
                }
                WeaponToken::FlipOut => target.board.flip_on_vertical_axis(),
                WeaponToken::Missing => {
                    let x = target.weapon_rng.next_u64() as usize;
                    let y = target.weapon_rng.next_u64() as usize;
                    let _ = target.board.remove_next_removable_from(x, y);
                }
                WeaponToken::PieceIt => {
                    let _ = target
                        .board
                        .add_random_middle_cell(&mut target.weapon_rng, Cell::visible());
                }
                WeaponToken::Blind => {
                    target
                        .board
                        .remove_random_half_removable(&mut target.weapon_rng);
                }
                WeaponToken::Reagan => target.set_funds(-target.funds()),
                WeaponToken::Bug => {
                    let _ = target
                        .board
                        .add_random_middle_cell(&mut target.weapon_rng, Cell::Invisible);
                }
                WeaponToken::Twilight => {
                    target.board.hide_existing_cells();
                }
                WeaponToken::Gimp => {
                    target.board.gimp_removable_cells();
                }
                WeaponToken::Swap
                | WeaponToken::Keating
                | WeaponToken::NiceDay
                | WeaponToken::Susan => unreachable!("mirror nullifies this one-shot"),
                _ => unreachable!("checked by is_phase_8_one_shot"),
            }
            return;
        }

        let (launcher, target) = self.players_mut(launcher_id, target_id);
        match token {
            WeaponToken::Swap => {
                let launcher_board = launcher.board().clone();
                launcher.set_board(target.board().clone());
                target.set_board(launcher_board);
            }
            WeaponToken::RiseUp => {
                let hole = target.weapon_rng.next_u64() as usize;
                target.board.insert_garbage_line(hole);
            }
            WeaponToken::FlipOut => target.board.flip_on_vertical_axis(),
            WeaponToken::Missing => {
                let x = target.weapon_rng.next_u64() as usize;
                let y = target.weapon_rng.next_u64() as usize;
                let _ = target.board.remove_next_removable_from(x, y);
            }
            WeaponToken::PieceIt => {
                let _ = target
                    .board
                    .add_random_middle_cell(&mut target.weapon_rng, Cell::visible());
            }
            WeaponToken::Blind => {
                target
                    .board
                    .remove_random_half_removable(&mut target.weapon_rng);
            }
            WeaponToken::Keating => unreachable!("handled before mutable player split"),
            WeaponToken::Reagan => target.set_funds(-target.funds()),
            WeaponToken::NiceDay => target.generator.queue_happy(1),
            WeaponToken::Bug => {
                let _ = target
                    .board
                    .add_random_middle_cell(&mut target.weapon_rng, Cell::Invisible);
            }
            WeaponToken::Susan => std::mem::swap(&mut launcher.arsenal, &mut target.arsenal),
            WeaponToken::Twilight => {
                target.board.hide_existing_cells();
            }
            WeaponToken::Gimp => {
                target.board.gimp_removable_cells();
            }
            _ => unreachable!("checked by is_phase_8_one_shot"),
        }
    }

    fn resolve_launched_weapon(
        &mut self,
        launcher: PlayerId,
        target: PlayerId,
        token: WeaponToken,
    ) {
        if self
            .player(launcher)
            .active_effects
            .is_active(WeaponToken::Mirror)
        {
            if mirror_nullifies(token) {
                self.record(BattleEvent::WeaponNullified {
                    player: launcher,
                    token,
                });
            } else {
                self.record(BattleEvent::WeaponReflected {
                    player: launcher,
                    token,
                });
                self.apply_weapon_effect(launcher, launcher, token);
            }
        } else {
            self.apply_weapon_effect(launcher, target, token);
        }
    }

    fn apply_weapon_effect(&mut self, launcher: PlayerId, target: PlayerId, token: WeaponToken) {
        if is_phase_8_one_shot(token) {
            self.apply_one_shot_weapon(launcher, target, token);
            self.record(BattleEvent::OneShotWeaponApplied {
                launcher,
                target,
                token,
            });
        } else {
            let affected = timed_effect_target(launcher, target, token);
            let remaining_lines = self.player_mut(affected).activate_timed_effect(token);
            self.record(BattleEvent::TimedWeaponActivated {
                launcher,
                target: affected,
                token,
                remaining_lines,
            });
        }
    }

    fn record_and_process_player_events(
        &mut self,
        player: PlayerId,
        events: Vec<CoreEvent>,
    ) -> Vec<LoggedEvent> {
        let start = self.log.len();
        let mut saw_spawn_failed = false;
        let mut saw_piece_lock = false;
        let mut lines_cleared = 0;
        for event in events {
            saw_spawn_failed |= matches!(event, CoreEvent::SpawnFailed { .. });
            saw_piece_lock |= matches!(event, CoreEvent::PieceLocked { .. });
            if let CoreEvent::LinesCleared { lines, .. } = event {
                lines_cleared = lines;
            }
            self.record(BattleEvent::PlayerEvent { player, event });
        }

        if lines_cleared > 0 {
            let lawyer_active = self
                .player(player)
                .active_effects
                .is_active(WeaponToken::Lawyers);
            let mondale_tax = self
                .last_lines_cleared_funds(player)
                .map_or(0, |funds| funds * 30 / 70);
            let expired = self
                .player_mut(player)
                .observe_effect_line_clear(lines_cleared);
            for token in expired {
                self.record(BattleEvent::TimedWeaponExpired { player, token });
            }
            self.observe_recon_line_clear(player, lines_cleared);
            if mondale_tax != 0 {
                self.player_mut(player.opponent()).add_funds(mondale_tax);
            }
            if lawyer_active {
                for _ in 0..lines_cleared {
                    let opponent = player.opponent();
                    let hole = self.player_mut(opponent).weapon_rng.next_u64() as usize;
                    self.player_mut(opponent).board.insert_garbage_line(hole);
                }
            }
        }

        if lines_cleared > 0 || saw_piece_lock {
            self.emit_recon_updates(player, lines_cleared);
            self.flush_incoming_weapons(player);
            let spawn_events = self.player_mut(player).spawn_deferred_piece();
            for event in spawn_events {
                saw_spawn_failed |= matches!(event, CoreEvent::SpawnFailed { .. });
                self.record(BattleEvent::PlayerEvent { player, event });
            }
        }

        if saw_spawn_failed {
            self.finish_game(player);
        } else if lines_cleared > 0 {
            self.observe_bazaar();
        }

        self.log[start..].to_vec()
    }

    fn observe_recon_line_clear(&mut self, target: PlayerId, lines: u32) {
        let viewer = target.opponent();
        let expired = self
            .player_mut(viewer)
            .active_effects
            .observe_line_clear_for(
                lines,
                [WeaponToken::Ames, WeaponToken::Ace, WeaponToken::Condor],
            );
        for token in expired {
            self.record(BattleEvent::TimedWeaponExpired {
                player: viewer,
                token,
            });
            if ReconLevel::from_token(token).is_some() {
                self.record(BattleEvent::ReconDisabled {
                    viewer,
                    target,
                    token,
                });
            }
        }
    }

    fn flush_incoming_weapons(&mut self, target: PlayerId) {
        while let Some(queued) = self.incoming_weapons[player_index(target)].pop_front() {
            self.apply_weapon_effect(queued.launcher, target, queued.token);
        }
    }

    fn emit_recon_updates(&mut self, target: PlayerId, target_cleared_lines: u32) {
        let viewer = target.opponent();
        let Some(level) = self.active_recon_level(viewer) else {
            return;
        };
        let board = self.player(target).board().clone();
        let funds = self.player(target).funds();
        let snapshot = {
            let rng = &mut self.player_mut(viewer).weapon_rng;
            sample_recon(level, &board, funds, target_cleared_lines, rng)
        };
        self.cached_opponent_funds[player_index(viewer)] = snapshot.funds;
        self.record(BattleEvent::ReconUpdated {
            viewer,
            target,
            snapshot,
        });
    }

    fn active_recon_level(&self, viewer: PlayerId) -> Option<ReconLevel> {
        [WeaponToken::Condor, WeaponToken::Ace, WeaponToken::Ames]
            .into_iter()
            .find(|token| self.player(viewer).active_effects.is_active(*token))
            .and_then(ReconLevel::from_token)
    }

    fn record_player_events(&mut self, player: PlayerId, events: Vec<CoreEvent>) {
        for event in events {
            self.record(BattleEvent::PlayerEvent { player, event });
        }
    }

    fn last_lines_cleared_funds(&self, player: PlayerId) -> Option<i32> {
        self.log.iter().rev().find_map(|logged| match logged.event {
            BattleEvent::PlayerEvent {
                player: event_player,
                event: CoreEvent::LinesCleared { funds, .. },
            } if event_player == player => Some(funds),
            _ => None,
        })
    }

    fn observe_bazaar(&mut self) {
        if self.phase != GamePhase::Playing {
            return;
        }

        if self
            .bazaar
            .observe(self.player_one.lines(), self.player_two.lines())
        {
            self.phase = GamePhase::Bazaar;
            self.bazaar_done = [false, false];
            self.open_bazaar_sessions();
            self.record(BattleEvent::BazaarEntered);
        }
    }

    fn open_bazaar_sessions(&mut self) {
        self.bazaar_sessions = [
            Some(Bazaar::new(
                self.player_one.arsenal.clone(),
                self.player_one.funds(),
                self.player_one
                    .active_effects
                    .is_active(WeaponToken::Carter),
            )),
            Some(Bazaar::new(
                self.player_two.arsenal.clone(),
                self.player_two.funds(),
                self.player_two
                    .active_effects
                    .is_active(WeaponToken::Carter),
            )),
        ];
    }

    fn bazaar_session_mut(&mut self, player: PlayerId) -> Result<&mut Bazaar, ShoppingError> {
        if self.phase != GamePhase::Bazaar {
            return Err(ShoppingError::BazaarClosed);
        }

        let index = player_index(player);
        if self.bazaar_done[index] {
            return Err(ShoppingError::PlayerDone);
        }

        self.bazaar_sessions[index]
            .as_mut()
            .ok_or(ShoppingError::BazaarClosed)
    }

    fn commit_bazaar_sessions(&mut self) {
        if let Some(session) = self.bazaar_sessions[0].take() {
            let commit = session.commit();
            self.player_one.arsenal = commit.arsenal;
            self.player_one.score.set_funds(commit.funds);
        }
        if let Some(session) = self.bazaar_sessions[1].take() {
            let commit = session.commit();
            self.player_two.arsenal = commit.arsenal;
            self.player_two.score.set_funds(commit.funds);
        }
    }

    fn detect_startup_deaths(&mut self) {
        match (
            self.player_one.is_game_over(),
            self.player_two.is_game_over(),
        ) {
            (true, false) => self.finish_game(PlayerId::One),
            (false, true) => self.finish_game(PlayerId::Two),
            _ => {}
        }
    }

    fn finish_game(&mut self, loser: PlayerId) {
        if self.phase == GamePhase::GameOver {
            return;
        }

        let winner = loser.opponent();
        self.phase = GamePhase::GameOver;
        self.record(BattleEvent::PlayerDied { player: loser });
        self.record(BattleEvent::GameOver { winner, loser });
    }

    fn record_one(&mut self, event: BattleEvent) -> Vec<LoggedEvent> {
        let logged = self.record(event);
        vec![logged]
    }

    fn record(&mut self, event: BattleEvent) -> LoggedEvent {
        let logged = LoggedEvent {
            sequence: self.log.len() as u64,
            event,
        };
        self.log.push(logged.clone());
        logged
    }
}

const fn player_index(player: PlayerId) -> usize {
    match player {
        PlayerId::One => 0,
        PlayerId::Two => 1,
    }
}

const fn is_phase_8_one_shot(token: WeaponToken) -> bool {
    matches!(
        token,
        WeaponToken::Swap
            | WeaponToken::RiseUp
            | WeaponToken::FlipOut
            | WeaponToken::Missing
            | WeaponToken::PieceIt
            | WeaponToken::Blind
            | WeaponToken::Keating
            | WeaponToken::Reagan
            | WeaponToken::NiceDay
            | WeaponToken::Bug
            | WeaponToken::Susan
            | WeaponToken::Twilight
            | WeaponToken::Gimp
    )
}

const fn is_supported_launch_weapon(token: WeaponToken) -> bool {
    is_phase_8_one_shot(token) || is_timed_weapon(token) || is_phase_10_timed_weapon(token)
}

const fn timed_effect_target(launcher: PlayerId, target: PlayerId, token: WeaponToken) -> PlayerId {
    match token {
        WeaponToken::Lawyers | WeaponToken::Ames | WeaponToken::Ace | WeaponToken::Condor => {
            launcher
        }
        _ => target,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BattleEvent, Command, CoreEvent, GameMode, GamePhase, PieceLoop, PlayerId, ShoppingError,
        TwoPlayerGame, DEFAULT_DROP_MS, DEFAULT_SLIDE_MS,
    };
    use crate::{
        ai::computer_difficulty,
        board::{Board, Coord, LineClearMode, LineClearOutcome, BOARD_HEIGHT, BOARD_WIDTH},
        cell::{Cell, Pip},
        piece::PieceKind,
        recon::ReconLevel,
        rng::GameSeed,
        weapons::WeaponToken,
    };

    #[test]
    fn seeded_loop_spawns_stable_first_piece() {
        let (game, events) = PieceLoop::new(GameSeed::from_u64(42));

        assert_eq!(
            events,
            vec![CoreEvent::PieceSpawned {
                kind: PieceKind::Plug
            }]
        );
        assert_eq!(game.active_piece().unwrap().kind(), PieceKind::Plug);
    }

    #[test]
    fn commands_move_and_rotate_active_piece() {
        let (mut game, _) = PieceLoop::new(GameSeed::from_u64(42));

        assert_eq!(
            game.command(Command::MoveLeft),
            vec![CoreEvent::PieceMoved { anchor: (3, 0) }]
        );
        assert_eq!(
            game.command(Command::RotateClockwise),
            vec![CoreEvent::PieceRotated { orientation: 1 }]
        );
        assert_eq!(game.active_piece().unwrap().orientation(), 1);
    }

    #[test]
    fn default_drop_tick_moves_piece_down() {
        let (mut game, _) = PieceLoop::new(GameSeed::from_u64(42));

        assert!(game.tick(DEFAULT_DROP_MS - 1).is_empty());
        assert_eq!(game.tick(1), vec![CoreEvent::PieceMoved { anchor: (4, 1) }]);
    }

    #[test]
    fn fast_drop_scores_once_per_start() {
        let (mut game, _) = PieceLoop::new(GameSeed::from_u64(42));

        assert_eq!(
            game.command(Command::StartFastDrop),
            vec![CoreEvent::FastDropStarted { score_delta: 28 }]
        );
        assert!(game.command(Command::StartFastDrop).is_empty());
        assert_eq!(game.score(), BOARD_HEIGHT as i32);
    }

    #[test]
    fn failed_drop_starts_slide_then_locks_and_spawns() {
        let mut board = Board::empty();
        for x in 0..10 {
            board.set(Coord::new(x, 3).unwrap(), Some(Cell::visible()));
        }
        let (mut game, _) = PieceLoop::with_board(GameSeed::from_u64(42), board);

        assert_eq!(game.tick(DEFAULT_DROP_MS), vec![CoreEvent::PieceLanded]);
        assert_eq!(
            game.tick(DEFAULT_SLIDE_MS),
            vec![
                CoreEvent::PieceLocked {
                    kind: PieceKind::Plug
                },
                CoreEvent::LinesCleared { lines: 1, funds: 0 },
                CoreEvent::SpawnFailed {
                    kind: PieceKind::SlantLeft
                },
            ]
        );
    }

    #[test]
    fn slide_expiry_moves_down_if_space_becomes_available() {
        let mut board = Board::empty();
        for x in 0..10 {
            board.set(Coord::new(x, 3).unwrap(), Some(Cell::visible()));
        }
        let (mut game, _) = PieceLoop::with_board(GameSeed::from_u64(42), board);

        assert_eq!(game.tick(DEFAULT_DROP_MS), vec![CoreEvent::PieceLanded]);
        for x in 0..10 {
            game.board.set(Coord::new(x, 3).unwrap(), None);
        }

        assert_eq!(
            game.tick(DEFAULT_SLIDE_MS),
            vec![CoreEvent::PieceMoved { anchor: (4, 1) }]
        );
    }

    #[test]
    fn spawn_failure_emits_game_over_event() {
        let mut board = Board::empty();
        board.set(Coord::new(6, 1).unwrap(), Some(Cell::visible()));

        let (game, events) = PieceLoop::with_board(GameSeed::from_u64(1), board);

        assert_eq!(
            events,
            vec![CoreEvent::SpawnFailed {
                kind: PieceKind::Die
            }]
        );
        assert!(game.is_game_over());
    }

    #[test]
    fn lock_applies_line_clear_funds_and_line_count_before_next_spawn() {
        let mut board = Board::empty();
        for x in 0..10 {
            let cell = if x == 0 {
                crate::cell::Cell::die(crate::cell::Pip::new(6).unwrap())
            } else {
                Cell::visible()
            };
            board.set(Coord::new(x, 3).unwrap(), Some(cell));
        }
        let (mut game, _) = PieceLoop::with_board(GameSeed::from_u64(42), board);

        assert_eq!(game.tick(DEFAULT_DROP_MS), vec![CoreEvent::PieceLanded]);
        let events = game.tick(DEFAULT_SLIDE_MS);

        assert_eq!(game.lines(), 1);
        assert_eq!(game.funds(), 6);
        assert!(events.contains(&CoreEvent::LinesCleared { lines: 1, funds: 6 }));
        assert!(matches!(events.last(), Some(CoreEvent::SpawnFailed { .. })));
    }

    #[test]
    fn two_player_start_logs_both_initial_spawns() {
        let game = TwoPlayerGame::new(GameSeed::from_u64(42), GameSeed::from_u64(7));

        assert_eq!(game.phase(), GamePhase::Playing);
        assert_eq!(game.event_log()[0].event, BattleEvent::GameStarted);
        assert_eq!(game.event_log()[0].sequence, 0);
        assert!(matches!(
            game.event_log()[1].event,
            BattleEvent::PlayerEvent {
                player: PlayerId::One,
                event: CoreEvent::PieceSpawned { .. }
            }
        ));
        assert!(matches!(
            game.event_log()[2].event,
            BattleEvent::PlayerEvent {
                player: PlayerId::Two,
                event: CoreEvent::PieceSpawned { .. }
            }
        ));
    }

    #[test]
    fn human_vs_computer_games_are_unranked() {
        let difficulty = computer_difficulty(14).unwrap();
        let game = TwoPlayerGame::human_vs_computer(
            GameSeed::from_u64(1),
            Board::empty(),
            GameSeed::from_u64(2),
            Board::empty(),
            PlayerId::Two,
            difficulty,
        );

        assert_eq!(
            game.mode(),
            GameMode::HumanVsComputer {
                computer: PlayerId::Two,
                difficulty,
            }
        );
        assert!(!game.is_ranked_game());
        assert!(GameMode::HumanVsHuman.is_ranked());
    }

    #[test]
    fn pause_stops_commands_and_ticks_until_resume() {
        let mut game = TwoPlayerGame::new(GameSeed::from_u64(42), GameSeed::from_u64(7));
        let before = game.event_log().len();

        assert_eq!(game.pause()[0].event, BattleEvent::Paused);
        assert_eq!(game.phase(), GamePhase::Paused);
        assert!(game.command(PlayerId::One, Command::MoveLeft).is_empty());
        assert!(game.tick_player(PlayerId::One, DEFAULT_DROP_MS).is_empty());
        assert_eq!(game.event_log().len(), before + 1);

        assert_eq!(game.resume()[0].event, BattleEvent::Resumed);
        assert_eq!(game.phase(), GamePhase::Playing);
        assert!(!game.command(PlayerId::One, Command::MoveLeft).is_empty());
    }

    #[test]
    fn scripted_full_game_enters_leaves_bazaar_and_finishes() {
        let player_one_board = board_with_gap_line_and_blocking_ceiling();
        let player_two_board = board_with_lock_then_blocked_spawn();
        let mut game = TwoPlayerGame::with_boards(
            GameSeed::from_u64(1),
            player_one_board,
            GameSeed::from_u64(42),
            player_two_board,
        );
        game.player_two.score.apply_line_clear(&LineClearOutcome {
            lines_cleared: 19,
            funds: 0,
            happy_missed: 0,
            cleared_rows: Vec::new(),
        });
        assert!(!game.bazaar.observe(0, 19));

        let mut saw_landed = false;
        for _ in 0..BOARD_HEIGHT {
            let events = game.tick_player(PlayerId::One, DEFAULT_DROP_MS);
            if events.iter().any(|logged| {
                logged.event
                    == BattleEvent::PlayerEvent {
                        player: PlayerId::One,
                        event: CoreEvent::PieceLanded,
                    }
            }) {
                saw_landed = true;
                break;
            }
        }
        assert!(saw_landed);

        let lock_events = game.tick_player(PlayerId::One, DEFAULT_SLIDE_MS);
        assert!(lock_events.iter().any(|logged| {
            logged.event
                == BattleEvent::PlayerEvent {
                    player: PlayerId::One,
                    event: CoreEvent::LinesCleared { lines: 1, funds: 3 },
                }
        }));
        assert!(lock_events
            .iter()
            .any(|logged| logged.event == BattleEvent::BazaarEntered));
        assert_eq!(game.phase(), GamePhase::Bazaar);
        assert!(game.tick_player(PlayerId::Two, DEFAULT_DROP_MS).is_empty());

        assert_eq!(
            game.bazaar_done(PlayerId::One)[0].event,
            BattleEvent::BazaarPlayerDone {
                player: PlayerId::One
            }
        );
        assert_eq!(game.phase(), GamePhase::Bazaar);
        let left_events = game.bazaar_done(PlayerId::Two);
        assert_eq!(left_events.last().unwrap().event, BattleEvent::BazaarLeft);
        assert_eq!(game.phase(), GamePhase::Playing);

        assert_eq!(game.pause()[0].event, BattleEvent::Paused);
        assert_eq!(game.resume()[0].event, BattleEvent::Resumed);

        assert!(game
            .tick_player(PlayerId::Two, DEFAULT_DROP_MS)
            .iter()
            .any(|logged| logged.event
                == BattleEvent::PlayerEvent {
                    player: PlayerId::Two,
                    event: CoreEvent::PieceLanded,
                }));
        let game_over_events = game.tick_player(PlayerId::Two, DEFAULT_SLIDE_MS);
        assert!(game_over_events.iter().any(|logged| {
            matches!(
                logged.event,
                BattleEvent::PlayerEvent {
                    player: PlayerId::Two,
                    event: CoreEvent::SpawnFailed { .. }
                }
            )
        }));
        assert_eq!(
            game_over_events.last().unwrap().event,
            BattleEvent::GameOver {
                winner: PlayerId::One,
                loser: PlayerId::Two,
            }
        );
        assert_eq!(game.phase(), GamePhase::GameOver);

        for (index, logged) in game.event_log().iter().enumerate() {
            assert_eq!(logged.sequence, index as u64);
        }
    }

    #[test]
    fn bazaar_shopping_stages_until_both_players_are_done() {
        let mut game = TwoPlayerGame::new(GameSeed::from_u64(42), GameSeed::from_u64(7));
        game.player_one.score.add_funds(100);
        game.player_two.score.add_funds(100);
        game.phase = GamePhase::Bazaar;
        game.open_bazaar_sessions();

        assert_eq!(game.bazaar_buy(PlayerId::One, WeaponToken::Gimp), Ok(0));
        assert_eq!(game.player(PlayerId::One).funds(), 100);
        assert_eq!(game.player(PlayerId::One).arsenal().slots()[0], None);
        assert_eq!(
            game.bazaar_session(PlayerId::One).unwrap().staged_funds(),
            75
        );

        assert_eq!(
            game.bazaar_done(PlayerId::One)[0].event,
            BattleEvent::BazaarPlayerDone {
                player: PlayerId::One
            }
        );
        assert_eq!(
            game.bazaar_buy(PlayerId::One, WeaponToken::FlipOut),
            Err(ShoppingError::PlayerDone)
        );
        assert_eq!(game.player(PlayerId::One).funds(), 100);

        game.bazaar_buy(PlayerId::Two, WeaponToken::FlipOut)
            .unwrap();
        let events = game.bazaar_done(PlayerId::Two);
        assert_eq!(events.last().unwrap().event, BattleEvent::BazaarLeft);
        assert_eq!(game.phase(), GamePhase::Playing);
        assert_eq!(game.player(PlayerId::One).funds(), 75);
        assert_eq!(
            game.player(PlayerId::One).arsenal().slots()[0]
                .unwrap()
                .token,
            WeaponToken::Gimp
        );
        assert_eq!(game.player(PlayerId::Two).funds(), 85);
        assert_eq!(
            game.player(PlayerId::Two).arsenal().slots()[0]
                .unwrap()
                .token,
            WeaponToken::FlipOut
        );
        assert!(game.bazaar_session(PlayerId::One).is_none());
    }

    #[test]
    fn bazaar_remove_staged_refunds_before_commit() {
        let mut game = TwoPlayerGame::new(GameSeed::from_u64(42), GameSeed::from_u64(7));
        game.player_one.score.add_funds(100);
        game.phase = GamePhase::Bazaar;
        game.open_bazaar_sessions();

        game.bazaar_buy(PlayerId::One, WeaponToken::Gimp).unwrap();
        assert_eq!(
            game.bazaar_remove_staged(PlayerId::One, WeaponToken::Gimp),
            Ok(())
        );
        assert_eq!(
            game.bazaar_session(PlayerId::One).unwrap().staged_funds(),
            100
        );
        assert_eq!(
            game.bazaar_session(PlayerId::One)
                .unwrap()
                .staged_arsenal()
                .slots()[0],
            None
        );
    }

    #[test]
    fn phase_8_one_shot_weapons_apply_deterministic_scenarios() {
        assert_swap_meet_swaps_boards();
        assert_rise_up_inserts_garbage_line();
        assert_flip_out_mirrors_board();
        assert_missing_pieces_removes_one_removable_cell();
        assert_piece_it_together_adds_visible_middle_cell();
        assert_blind_cleric_removes_seeded_half();
        assert_keating_uses_cached_opponent_funds();
        assert_reagan_allows_negative_funds();
        assert_have_a_nice_day_queues_happy_piece();
        assert_bug_report_adds_invisible_middle_cell();
        assert_lazy_susan_swaps_arsenals();
        assert_twilight_hides_existing_cells_lossily();
        assert_gimp_replaces_removable_cells_preserving_value();
    }

    #[test]
    fn timed_weapon_launch_activates_and_stacks_line_duration() {
        let mut game = TwoPlayerGame::new(GameSeed::from_u64(1), GameSeed::from_u64(2));
        game.player_one
            .arsenal
            .buy_weapon(WeaponToken::Speedy)
            .unwrap();
        game.player_one
            .arsenal
            .buy_weapon(WeaponToken::Speedy)
            .unwrap();

        let events = game.launch_weapon_slot(PlayerId::One, 1).unwrap();
        assert_eq!(
            events[1].event,
            BattleEvent::TimedWeaponActivated {
                launcher: PlayerId::One,
                target: PlayerId::Two,
                token: WeaponToken::Speedy,
                remaining_lines: 10,
            }
        );
        game.launch_weapon_slot(PlayerId::One, 1).unwrap();
        assert_eq!(
            game.player(PlayerId::Two)
                .active_effects()
                .remaining_lines(WeaponToken::Speedy),
            20
        );
        assert_eq!(game.player(PlayerId::One).arsenal().slots()[0], None);
    }

    #[test]
    fn phase_9_timed_weapons_apply_activation_effects() {
        for token in [
            WeaponToken::FearedWeird,
            WeaponToken::FourByFour,
            WeaponToken::Hatter,
            WeaponToken::Upbyside,
            WeaponToken::FallOut,
            WeaponToken::Lawyers,
            WeaponToken::Speedy,
            WeaponToken::Mondale,
            WeaponToken::Carter,
            WeaponToken::SoLong,
            WeaponToken::NoDice,
            WeaponToken::Bottle,
            WeaponToken::NoSlide,
            WeaponToken::Meadow,
            WeaponToken::Slick,
            WeaponToken::Broken,
            WeaponToken::Force,
        ] {
            let game = launch_timed(token);
            let affected = if token == WeaponToken::Lawyers {
                PlayerId::One
            } else {
                PlayerId::Two
            };
            assert!(
                game.player(affected).active_effects().is_active(token),
                "{token:?} should be active"
            );
        }
    }

    #[test]
    fn timed_weapons_expire_after_target_line_clears_and_restore_hooks() {
        let mut game = launch_timed(WeaponToken::NoDice);
        assert_eq!(
            game.player(PlayerId::Two)
                .generator
                .probabilities()
                .weight(PieceKind::Die),
            0
        );

        game.player_two.score.apply_line_clear(&LineClearOutcome {
            lines_cleared: 34,
            funds: 0,
            happy_missed: 0,
            cleared_rows: Vec::new(),
        });
        assert!(game
            .player_two
            .active_effects
            .observe_line_clear(34)
            .is_empty());
        assert!(game
            .player(PlayerId::Two)
            .active_effects()
            .is_active(WeaponToken::NoDice));

        let expired = game.player_two.observe_effect_line_clear(1);
        assert_eq!(expired, vec![WeaponToken::NoDice]);
        assert_eq!(
            game.player(PlayerId::Two)
                .generator
                .probabilities()
                .weight(PieceKind::Die),
            100
        );
    }

    #[test]
    fn timed_effects_change_core_behaviors_while_active() {
        assert_speedy_and_meadow_change_drop_cadence();
        assert_upbyside_reverses_horizontal_movement();
        assert_no_slide_locks_without_grace();
        assert_force_erases_without_dropping();
        assert_bottle_structures_are_removed_without_restoring_overwrites();
        assert_lawyers_raises_opponent_on_final_line_before_expiring();
        assert_mondale_tax_splits_new_funds();
        assert_carter_prices_are_captured_at_bazaar_entry();
        assert_upbyside_flips_board_and_reverses_gravity();
        assert_fallout_clears_middle_columns_on_activation();
        assert_bottle_uses_legacy_wall_rows();
        assert_slick_bounces_between_blockage();
    }

    #[test]
    fn phase_10_spy_weapons_activate_on_launcher_and_emit_recon_after_placement() {
        let mut game = TwoPlayerGame::with_boards(
            GameSeed::from_u64(100),
            Board::empty(),
            GameSeed::from_u64(42),
            board_with_lock_then_blocked_spawn(),
        );
        game.player_one
            .arsenal
            .buy_weapon(WeaponToken::Condor)
            .unwrap();
        let launch = game.launch_weapon_slot(PlayerId::One, 1).unwrap();
        assert!(launch.iter().any(|logged| {
            logged.event
                == BattleEvent::TimedWeaponActivated {
                    launcher: PlayerId::One,
                    target: PlayerId::One,
                    token: WeaponToken::Condor,
                    remaining_lines: 40,
                }
        }));

        game.tick_player(PlayerId::Two, DEFAULT_DROP_MS);
        let events = game.tick_player(PlayerId::Two, DEFAULT_SLIDE_MS);

        assert!(events.iter().any(|logged| {
            matches!(
                &logged.event,
                BattleEvent::ReconUpdated {
                    viewer: PlayerId::One,
                    target: PlayerId::Two,
                    snapshot,
                } if snapshot.level == ReconLevel::Condor
            )
        }));
    }

    #[test]
    fn mirror_reflects_supported_launches_and_nullifies_exception_tokens() {
        let mut reflected = TwoPlayerGame::new(GameSeed::from_u64(100), GameSeed::from_u64(200));
        reflected
            .player_one
            .active_effects
            .activate(WeaponToken::Mirror);
        reflected
            .player_one
            .arsenal
            .buy_weapon(WeaponToken::Speedy)
            .unwrap();

        let events = reflected.launch_weapon_slot(PlayerId::One, 1).unwrap();

        assert!(events.iter().any(|logged| {
            logged.event
                == BattleEvent::WeaponReflected {
                    player: PlayerId::One,
                    token: WeaponToken::Speedy,
                }
        }));
        assert!(reflected
            .player(PlayerId::One)
            .active_effects()
            .is_active(WeaponToken::Speedy));
        assert!(!reflected
            .player(PlayerId::Two)
            .active_effects()
            .is_active(WeaponToken::Speedy));

        let mut nullified = TwoPlayerGame::new(GameSeed::from_u64(100), GameSeed::from_u64(200));
        nullified
            .player_one
            .active_effects
            .activate(WeaponToken::Mirror);
        nullified
            .player_one
            .arsenal
            .buy_weapon(WeaponToken::Keating)
            .unwrap();
        nullified.player_two.score.add_funds(425);

        let events = nullified.launch_weapon_slot(PlayerId::One, 1).unwrap();

        assert!(events.iter().any(|logged| {
            logged.event
                == BattleEvent::WeaponNullified {
                    player: PlayerId::One,
                    token: WeaponToken::Keating,
                }
        }));
        assert_eq!(nullified.player(PlayerId::One).funds(), 0);
        assert_eq!(nullified.player(PlayerId::Two).funds(), 425);

        for token in all_launch_tokens() {
            let mut game = TwoPlayerGame::new(GameSeed::from_u64(100), GameSeed::from_u64(200));
            game.player_one.active_effects.activate(WeaponToken::Mirror);
            game.player_one.arsenal.buy_weapon(token).unwrap();

            let events = game.launch_weapon_slot(PlayerId::One, 1).unwrap();
            let expected = if crate::weapons::mirror_nullifies(token) {
                BattleEvent::WeaponNullified {
                    player: PlayerId::One,
                    token,
                }
            } else {
                BattleEvent::WeaponReflected {
                    player: PlayerId::One,
                    token,
                }
            };

            assert!(
                events.iter().any(|logged| logged.event == expected),
                "missing mirror result for {token:?}"
            );
        }
    }

    #[test]
    fn queued_incoming_weapons_flush_fifo_after_target_placement() {
        let mut game = TwoPlayerGame::with_boards(
            GameSeed::from_u64(100),
            Board::empty(),
            GameSeed::from_u64(42),
            board_with_lock_then_blocked_spawn(),
        );
        game.queue_incoming_weapon(PlayerId::One, PlayerId::Two, WeaponToken::RiseUp);
        game.queue_incoming_weapon(PlayerId::One, PlayerId::Two, WeaponToken::Speedy);

        game.tick_player(PlayerId::Two, DEFAULT_DROP_MS);
        let events = game.tick_player(PlayerId::Two, DEFAULT_SLIDE_MS);

        let applied = events
            .iter()
            .filter_map(|logged| match logged.event {
                BattleEvent::OneShotWeaponApplied { token, .. }
                | BattleEvent::TimedWeaponActivated { token, .. } => Some(token),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(applied, vec![WeaponToken::RiseUp, WeaponToken::Speedy]);
        assert!(game
            .player(PlayerId::Two)
            .active_effects()
            .is_active(WeaponToken::Speedy));
    }

    #[test]
    fn queued_incoming_weapon_flushes_before_next_piece_spawn() {
        let mut game = TwoPlayerGame::new(GameSeed::from_u64(100), GameSeed::from_u64(42));
        game.queue_incoming_weapon(PlayerId::One, PlayerId::Two, WeaponToken::NiceDay);

        game.tick_player(PlayerId::Two, DEFAULT_DROP_MS * BOARD_HEIGHT as u64);
        let events = game.tick_player(PlayerId::Two, DEFAULT_SLIDE_MS);

        let applied_index = events
            .iter()
            .position(|logged| {
                logged.event
                    == BattleEvent::OneShotWeaponApplied {
                        launcher: PlayerId::One,
                        target: PlayerId::Two,
                        token: WeaponToken::NiceDay,
                    }
            })
            .expect("queued weapon should apply");
        let spawn_index = events
            .iter()
            .position(|logged| {
                logged.event
                    == BattleEvent::PlayerEvent {
                        player: PlayerId::Two,
                        event: CoreEvent::PieceSpawned {
                            kind: PieceKind::Happy,
                        },
                    }
            })
            .expect("queued happy should become the next spawn");

        assert!(applied_index < spawn_index);
    }

    #[test]
    fn spy_effects_expire_from_target_line_clears_and_emit_cleanup() {
        let mut game = TwoPlayerGame::new(GameSeed::from_u64(100), GameSeed::from_u64(200));
        game.player_one.active_effects.activate(WeaponToken::Condor);
        assert_eq!(
            game.player_one
                .active_effects
                .observe_line_clear_for(39, [WeaponToken::Condor]),
            Vec::new()
        );

        let events = game.record_and_process_player_events(
            PlayerId::Two,
            vec![CoreEvent::LinesCleared { lines: 1, funds: 0 }],
        );

        assert!(events.iter().any(|logged| {
            logged.event
                == BattleEvent::TimedWeaponExpired {
                    player: PlayerId::One,
                    token: WeaponToken::Condor,
                }
        }));
        assert!(events.iter().any(|logged| {
            logged.event
                == BattleEvent::ReconDisabled {
                    viewer: PlayerId::One,
                    target: PlayerId::Two,
                    token: WeaponToken::Condor,
                }
        }));
    }

    fn launch_one_shot(token: WeaponToken, target_board: Board) -> TwoPlayerGame {
        let mut game = TwoPlayerGame::with_boards(
            GameSeed::from_u64(100),
            Board::empty(),
            GameSeed::from_u64(200),
            target_board,
        );
        game.player_one.arsenal.buy_weapon(token).unwrap();
        let events = game.launch_weapon_slot(PlayerId::One, 1).unwrap();
        assert_eq!(
            events[0].event,
            BattleEvent::WeaponLaunched {
                launcher: PlayerId::One,
                target: PlayerId::Two,
                token,
            }
        );
        assert_eq!(
            events[1].event,
            BattleEvent::OneShotWeaponApplied {
                launcher: PlayerId::One,
                target: PlayerId::Two,
                token,
            }
        );
        game
    }

    fn assert_swap_meet_swaps_boards() {
        let mut player_one_board = Board::empty();
        player_one_board.set(Coord::new(0, 4).unwrap(), Some(Cell::visible()));
        let mut player_two_board = Board::empty();
        player_two_board.set(Coord::new(9, 4).unwrap(), Some(Cell::Structure));
        let mut game = TwoPlayerGame::with_boards(
            GameSeed::from_u64(100),
            player_one_board,
            GameSeed::from_u64(200),
            player_two_board,
        );
        game.player_one
            .arsenal
            .buy_weapon(WeaponToken::Swap)
            .unwrap();

        game.launch_weapon_slot(PlayerId::One, 1).unwrap();

        assert_eq!(
            game.player(PlayerId::One)
                .board()
                .get(Coord::new(9, 4).unwrap()),
            Some(Cell::Structure)
        );
        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(0, 4).unwrap()),
            Some(Cell::visible())
        );
    }

    fn assert_rise_up_inserts_garbage_line() {
        let game = launch_one_shot(WeaponToken::RiseUp, Board::empty());

        assert_eq!(occupied_count(game.player(PlayerId::Two).board()), 9);
        assert_eq!(
            row_occupied_count(game.player(PlayerId::Two).board(), BOARD_HEIGHT - 1),
            9
        );
    }

    fn assert_flip_out_mirrors_board() {
        let mut board = Board::empty();
        board.set(Coord::new(0, 10).unwrap(), Some(Cell::Structure));

        let game = launch_one_shot(WeaponToken::FlipOut, board);

        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(9, 10).unwrap()),
            Some(Cell::Structure)
        );
    }

    fn assert_missing_pieces_removes_one_removable_cell() {
        let mut board = Board::empty();
        board.set(Coord::new(0, 0).unwrap(), Some(Cell::Structure));
        board.set(Coord::new(5, 10).unwrap(), Some(Cell::visible()));

        let game = launch_one_shot(WeaponToken::Missing, board);

        assert_eq!(occupied_count(game.player(PlayerId::Two).board()), 1);
        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(0, 0).unwrap()),
            Some(Cell::Structure)
        );
    }

    fn assert_piece_it_together_adds_visible_middle_cell() {
        let game = launch_one_shot(WeaponToken::PieceIt, Board::empty());

        let cells = occupied_coords(game.player(PlayerId::Two).board());
        assert_eq!(cells.len(), 1);
        assert!((BOARD_HEIGHT / 4..BOARD_HEIGHT * 3 / 4).contains(&cells[0].y));
        assert_eq!(
            game.player(PlayerId::Two).board().get(cells[0]),
            Some(Cell::visible())
        );
    }

    fn assert_blind_cleric_removes_seeded_half() {
        let mut board = Board::empty();
        for x in 0..BOARD_WIDTH {
            board.set(Coord::new(x, 12).unwrap(), Some(Cell::visible()));
        }

        let game = launch_one_shot(WeaponToken::Blind, board);

        assert_eq!(occupied_count(game.player(PlayerId::Two).board()), 7);
    }

    fn assert_keating_uses_cached_opponent_funds() {
        let mut game = TwoPlayerGame::new(GameSeed::from_u64(100), GameSeed::from_u64(200));
        game.player_one
            .arsenal
            .buy_weapon(WeaponToken::Keating)
            .unwrap();
        game.player_two.score.add_funds(425);
        game.cached_opponent_funds[0] = 300;

        game.launch_weapon_slot(PlayerId::One, 1).unwrap();

        assert_eq!(game.player(PlayerId::One).funds(), 300);
        assert_eq!(game.player(PlayerId::Two).funds(), 0);
    }

    fn assert_reagan_allows_negative_funds() {
        let mut game = TwoPlayerGame::new(GameSeed::from_u64(100), GameSeed::from_u64(200));
        game.player_one
            .arsenal
            .buy_weapon(WeaponToken::Reagan)
            .unwrap();
        game.player_two.score.add_funds(125);

        game.launch_weapon_slot(PlayerId::One, 1).unwrap();

        assert_eq!(game.player(PlayerId::Two).funds(), -125);
    }

    fn assert_have_a_nice_day_queues_happy_piece() {
        let mut game = launch_one_shot(WeaponToken::NiceDay, Board::empty());

        assert_eq!(game.player_two.generator.next_kind(), PieceKind::Happy);
    }

    fn assert_bug_report_adds_invisible_middle_cell() {
        let game = launch_one_shot(WeaponToken::Bug, Board::empty());
        let cells = occupied_coords(game.player(PlayerId::Two).board());

        assert_eq!(cells.len(), 1);
        assert!((BOARD_HEIGHT / 4..BOARD_HEIGHT * 3 / 4).contains(&cells[0].y));
        assert_eq!(
            game.player(PlayerId::Two).board().get(cells[0]),
            Some(Cell::Invisible)
        );
        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .legacy_snapshot(0, false)
                .ids[cells[0].y * BOARD_WIDTH + cells[0].x],
            0
        );
    }

    fn assert_lazy_susan_swaps_arsenals() {
        let mut game = TwoPlayerGame::new(GameSeed::from_u64(100), GameSeed::from_u64(200));
        game.player_one
            .arsenal
            .buy_weapon(WeaponToken::Susan)
            .unwrap();
        game.player_one
            .arsenal
            .buy_weapon(WeaponToken::Gimp)
            .unwrap();
        game.player_two
            .arsenal
            .buy_weapon(WeaponToken::FlipOut)
            .unwrap();

        game.launch_weapon_slot(PlayerId::One, 1).unwrap();

        assert_eq!(
            game.player(PlayerId::One).arsenal().slots()[0]
                .unwrap()
                .token,
            WeaponToken::FlipOut
        );
        assert_eq!(
            game.player(PlayerId::Two).arsenal().slots()[1]
                .unwrap()
                .token,
            WeaponToken::Gimp
        );
    }

    fn assert_twilight_hides_existing_cells_lossily() {
        let mut board = Board::empty();
        board.set(Coord::new(0, 0).unwrap(), Some(Cell::Structure));
        board.set(
            Coord::new(1, 0).unwrap(),
            Some(Cell::die(Pip::new(6).unwrap())),
        );

        let game = launch_one_shot(WeaponToken::Twilight, board);

        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(0, 0).unwrap()),
            Some(Cell::Hidden {
                value: 0,
                removable: false,
            })
        );
        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(1, 0).unwrap()),
            Some(Cell::Hidden {
                value: 6,
                removable: true,
            })
        );
    }

    fn assert_gimp_replaces_removable_cells_preserving_value() {
        let mut board = Board::empty();
        board.set(Coord::new(0, 0).unwrap(), Some(Cell::Structure));
        board.set(
            Coord::new(1, 0).unwrap(),
            Some(Cell::die(Pip::new(5).unwrap())),
        );

        let game = launch_one_shot(WeaponToken::Gimp, board);

        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(0, 0).unwrap()),
            Some(Cell::Structure)
        );
        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(1, 0).unwrap()),
            Some(Cell::Gimp { value: 5 })
        );
    }

    fn launch_timed(token: WeaponToken) -> TwoPlayerGame {
        launch_timed_with_board(token, Board::empty())
    }

    fn launch_timed_with_board(token: WeaponToken, target_board: Board) -> TwoPlayerGame {
        let mut game = TwoPlayerGame::with_boards(
            GameSeed::from_u64(100),
            Board::empty(),
            GameSeed::from_u64(200),
            target_board,
        );
        game.player_one.arsenal.buy_weapon(token).unwrap();
        let events = game.launch_weapon_slot(PlayerId::One, 1).unwrap();
        assert_eq!(
            events[0].event,
            BattleEvent::WeaponLaunched {
                launcher: PlayerId::One,
                target: PlayerId::Two,
                token,
            }
        );
        game
    }

    fn assert_speedy_and_meadow_change_drop_cadence() {
        let mut speedy = launch_timed(WeaponToken::Speedy);
        assert!(!speedy
            .tick_player(PlayerId::Two, DEFAULT_DROP_MS / 2)
            .is_empty());

        let mut meadow = launch_timed(WeaponToken::Meadow);
        assert!(meadow
            .tick_player(PlayerId::Two, DEFAULT_DROP_MS)
            .is_empty());
        assert!(!meadow
            .tick_player(PlayerId::Two, DEFAULT_DROP_MS)
            .is_empty());
    }

    fn assert_upbyside_reverses_horizontal_movement() {
        let mut game = launch_timed(WeaponToken::Upbyside);
        let before = game.player(PlayerId::Two).active_piece().unwrap().anchor();

        game.command(PlayerId::Two, Command::MoveLeft);

        assert_eq!(
            game.player(PlayerId::Two).active_piece().unwrap().anchor(),
            (before.0 + 1, before.1)
        );
    }

    fn assert_upbyside_flips_board_and_reverses_gravity() {
        let mut board = Board::empty();
        board.set(Coord::new(3, 0).unwrap(), Some(Cell::visible()));
        let mut game = launch_timed_with_board(WeaponToken::Upbyside, board);

        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(3, BOARD_HEIGHT - 1).unwrap()),
            Some(Cell::visible())
        );
        let before = game.player(PlayerId::Two).active_piece().unwrap().anchor();
        game.tick_player(PlayerId::Two, DEFAULT_DROP_MS);
        assert_eq!(
            game.player(PlayerId::Two).active_piece().unwrap().anchor(),
            (before.0, before.1 - 1)
        );
    }

    fn assert_fallout_clears_middle_columns_on_activation() {
        let mut board = Board::empty();
        board.set(Coord::new(1, 10).unwrap(), Some(Cell::visible()));
        board.set(Coord::new(2, 10).unwrap(), Some(Cell::visible()));
        board.set(Coord::new(7, 10).unwrap(), Some(Cell::visible()));
        board.set(Coord::new(8, 10).unwrap(), Some(Cell::visible()));
        let game = launch_timed_with_board(WeaponToken::FallOut, board);

        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(1, 10).unwrap()),
            Some(Cell::visible())
        );
        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(2, 10).unwrap()),
            None
        );
        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(7, 10).unwrap()),
            None
        );
        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(8, 10).unwrap()),
            Some(Cell::visible())
        );
    }

    fn assert_bottle_uses_legacy_wall_rows() {
        let game = launch_timed_with_board(WeaponToken::Bottle, Board::empty());

        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(0, 10).unwrap()),
            Some(Cell::Structure)
        );
        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(0, 17).unwrap()),
            Some(Cell::Structure)
        );
        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(0, 18).unwrap()),
            None
        );
    }

    fn assert_slick_bounces_between_blockage() {
        let mut game = launch_timed(WeaponToken::Slick);
        game.player_two.active.as_mut().unwrap().set_anchor(6, 0);
        let mut board = Board::empty();
        let blocker = game
            .player(PlayerId::Two)
            .active_piece()
            .unwrap()
            .board_coords()
            .into_iter()
            .find(|(x, _)| *x + 1 < BOARD_WIDTH as isize)
            .expect("anchored test piece should have a right-adjacent blocker cell");
        board.set(
            Coord::new((blocker.0 + 1) as usize, blocker.1 as usize).unwrap(),
            Some(Cell::Structure),
        );
        game.player_two.set_board(board);

        let before = game.player(PlayerId::Two).active_piece().unwrap().anchor();
        game.tick_player(PlayerId::Two, 1);

        assert_eq!(
            game.player(PlayerId::Two).active_piece().unwrap().anchor(),
            (before.0 - 1, before.1)
        );
    }

    fn assert_no_slide_locks_without_grace() {
        let mut game = TwoPlayerGame::with_boards(
            GameSeed::from_u64(100),
            Board::empty(),
            GameSeed::from_u64(42),
            board_with_lock_then_blocked_spawn(),
        );
        game.player_one
            .arsenal
            .buy_weapon(WeaponToken::NoSlide)
            .unwrap();
        game.launch_weapon_slot(PlayerId::One, 1).unwrap();

        assert!(game
            .tick_player(PlayerId::Two, DEFAULT_DROP_MS)
            .iter()
            .any(|logged| logged.event
                == BattleEvent::PlayerEvent {
                    player: PlayerId::Two,
                    event: CoreEvent::PieceLanded,
                }));
        let events = game.tick_player(PlayerId::Two, 0);

        assert!(events.iter().any(|logged| {
            matches!(
                logged.event,
                BattleEvent::PlayerEvent {
                    player: PlayerId::Two,
                    event: CoreEvent::PieceLocked { .. }
                }
            )
        }));
    }

    fn assert_force_erases_without_dropping() {
        let mut board = Board::empty();
        board.set(Coord::new(0, 25).unwrap(), Some(Cell::visible()));
        for x in 0..BOARD_WIDTH {
            board.set(Coord::new(x, 26).unwrap(), Some(Cell::visible()));
        }
        let mut game = TwoPlayerGame::with_boards(
            GameSeed::from_u64(100),
            Board::empty(),
            GameSeed::from_u64(42),
            board,
        );
        game.player_one
            .arsenal
            .buy_weapon(WeaponToken::Force)
            .unwrap();
        game.launch_weapon_slot(PlayerId::One, 1).unwrap();
        game.player_two
            .board
            .clear_completed_lines(LineClearMode::Force);

        assert_eq!(
            game.player(PlayerId::Two)
                .board()
                .get(Coord::new(0, 25).unwrap()),
            Some(Cell::visible())
        );
    }

    fn assert_bottle_structures_are_removed_without_restoring_overwrites() {
        let mut board = Board::empty();
        let overwritten = Coord::new(0, 10).unwrap();
        board.set(overwritten, Some(Cell::visible()));
        let mut game = TwoPlayerGame::with_boards(
            GameSeed::from_u64(100),
            Board::empty(),
            GameSeed::from_u64(200),
            board,
        );
        game.player_one
            .arsenal
            .buy_weapon(WeaponToken::Bottle)
            .unwrap();
        game.launch_weapon_slot(PlayerId::One, 1).unwrap();
        assert_eq!(
            game.player(PlayerId::Two).board().get(overwritten),
            Some(Cell::Structure)
        );

        game.player_two.expire_timed_effect(WeaponToken::Bottle);

        assert_eq!(game.player(PlayerId::Two).board().get(overwritten), None);
    }

    fn assert_lawyers_raises_opponent_on_final_line_before_expiring() {
        let mut game = launch_timed(WeaponToken::Lawyers);
        game.player_one.active_effects.observe_line_clear(4);

        let expired = game.player_one.observe_effect_line_clear(1);
        game.player_two.board.insert_garbage_line(0);

        assert_eq!(expired, vec![WeaponToken::Lawyers]);
        assert_eq!(
            row_occupied_count(game.player(PlayerId::Two).board(), BOARD_HEIGHT - 1),
            9
        );
    }

    fn assert_mondale_tax_splits_new_funds() {
        let mut game = launch_timed(WeaponToken::Mondale);
        game.player_two.score.add_funds(70);
        game.player_one.score.add_funds(30);

        assert_eq!(game.player(PlayerId::Two).funds(), 70);
        assert_eq!(game.player(PlayerId::One).funds(), 30);
    }

    fn assert_carter_prices_are_captured_at_bazaar_entry() {
        let mut game = launch_timed(WeaponToken::Carter);
        game.player_two.score.add_funds(100);
        game.phase = GamePhase::Bazaar;
        game.open_bazaar_sessions();

        assert_eq!(
            game.bazaar_session(PlayerId::Two)
                .unwrap()
                .price(WeaponToken::Gimp),
            50
        );
    }

    fn occupied_count(board: &Board) -> usize {
        occupied_coords(board).len()
    }

    fn row_occupied_count(board: &Board, y: usize) -> usize {
        (0..BOARD_WIDTH)
            .filter(|x| board.get(Coord::new(*x, y).unwrap()).is_some())
            .count()
    }

    fn occupied_coords(board: &Board) -> Vec<Coord> {
        (0..BOARD_HEIGHT)
            .flat_map(|y| (0..BOARD_WIDTH).map(move |x| Coord { x, y }))
            .filter(|coord| board.get(*coord).is_some())
            .collect()
    }

    const fn all_launch_tokens() -> [WeaponToken; 34] {
        [
            WeaponToken::FearedWeird,
            WeaponToken::FourByFour,
            WeaponToken::Hatter,
            WeaponToken::Upbyside,
            WeaponToken::FallOut,
            WeaponToken::Swap,
            WeaponToken::Lawyers,
            WeaponToken::RiseUp,
            WeaponToken::FlipOut,
            WeaponToken::Speedy,
            WeaponToken::Missing,
            WeaponToken::PieceIt,
            WeaponToken::Blind,
            WeaponToken::Mondale,
            WeaponToken::Keating,
            WeaponToken::Carter,
            WeaponToken::Reagan,
            WeaponToken::Ames,
            WeaponToken::Ace,
            WeaponToken::Condor,
            WeaponToken::NiceDay,
            WeaponToken::SoLong,
            WeaponToken::NoDice,
            WeaponToken::Bug,
            WeaponToken::Bottle,
            WeaponToken::NoSlide,
            WeaponToken::Susan,
            WeaponToken::Meadow,
            WeaponToken::Mirror,
            WeaponToken::Twilight,
            WeaponToken::Slick,
            WeaponToken::Broken,
            WeaponToken::Force,
            WeaponToken::Gimp,
        ]
    }

    fn board_with_gap_line_and_blocking_ceiling() -> Board {
        let mut board = Board::empty();
        for x in 0..BOARD_WIDTH {
            if x != 6 {
                board.set(
                    Coord::new(x, BOARD_HEIGHT - 1).unwrap(),
                    Some(Cell::visible()),
                );
            }
        }
        board
    }

    fn board_with_lock_then_blocked_spawn() -> Board {
        let mut board = Board::empty();
        for x in 0..BOARD_WIDTH {
            board.set(Coord::new(x, 3).unwrap(), Some(Cell::visible()));
        }
        board
    }
}
