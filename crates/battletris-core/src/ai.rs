//! Deterministic computer opponent placement, shopping, and weapon strategy.

use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::{
    board::{Board, Coord, LineClearMode, BOARD_HEIGHT, BOARD_WIDTH},
    game::{Command, PieceLoop},
    piece::{Piece, PieceKind},
    rng::{GameSeed, RngStream},
    weapons::{Arsenal, Bazaar, WeaponToken},
};

/// Legacy computer opponent bazaar leave delay in milliseconds.
pub const BAZAAR_LEAVE_DELAY_MS: u64 = 3000;

const OPEN_HOLE_PENALTY: i32 = 7000;
const CLOSED_HOLE_PENALTY: i32 = 10000;
const COVERED_HOLE_PENALTY: i32 = 3000;
const HEIGHT_PENALTY: i32 = 30000;
const LINE_BONUS: i32 = 5000;
const HAPPY_BONUS: i32 = 20000;
const VARIANCE_PENALTY: i32 = 50;
const MIN_COMBO_COST: i32 = 750;
const MAX_COMBO_COST: i32 = 1250;
const SWAP_TOP_LINE: usize = 5;
const SUSAN_ENABLE_OPPONENT_LINES: u32 = 50;

/// One legacy computer opponent difficulty row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComputerDifficulty {
    /// Difficulty index used by legacy setup screens.
    pub level: usize,
    /// Delay between AI actions in milliseconds.
    pub delay_ms: u64,
    /// Legacy display name prefix for Ernie.
    pub name: &'static str,
}

/// Fixed legacy difficulty table from `BTComputer.C`.
pub const COMPUTER_DIFFICULTIES: [ComputerDifficulty; 15] = [
    difficulty(0, 4000, "Comatose"),
    difficulty(1, 3000, "Somnambulant"),
    difficulty(2, 2000, "Lethargic"),
    difficulty(3, 1500, "Pensive"),
    difficulty(4, 1250, "Able"),
    difficulty(5, 1000, "Willing"),
    difficulty(6, 750, "Focused"),
    difficulty(7, 550, "Lively"),
    difficulty(8, 400, "Energetic"),
    difficulty(9, 350, "Pepped-up"),
    difficulty(10, 300, "Caffeinated"),
    difficulty(11, 225, "Bug-eyed"),
    difficulty(12, 100, "Supercharged"),
    difficulty(13, 10, "Hell-Bent"),
    difficulty(14, 0, "Bionic"),
];

const fn difficulty(level: usize, delay_ms: u64, name: &'static str) -> ComputerDifficulty {
    ComputerDifficulty {
        level,
        delay_ms,
        name,
    }
}

/// Returns one difficulty row by level.
#[must_use]
pub fn computer_difficulty(level: usize) -> Option<ComputerDifficulty> {
    COMPUTER_DIFFICULTIES.get(level).copied()
}

/// Board evaluation knobs matching the major legacy penalty categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationWeights {
    /// Per open hole penalty.
    pub open_hole: i32,
    /// Per closed hole penalty.
    pub closed_hole: i32,
    /// Penalty for occupied cells above holes.
    pub covered_hole: i32,
    /// Squared-height penalty scale.
    pub height: i32,
    /// Per-line reward.
    pub line_bonus: i32,
    /// Immediate happy clear reward.
    pub happy_bonus: i32,
    /// Column height variance penalty.
    pub variance: i32,
}

impl Default for EvaluationWeights {
    fn default() -> Self {
        Self {
            open_hole: OPEN_HOLE_PENALTY,
            closed_hole: CLOSED_HOLE_PENALTY,
            covered_hole: COVERED_HOLE_PENALTY,
            height: HEIGHT_PENALTY,
            line_bonus: LINE_BONUS,
            happy_bonus: HAPPY_BONUS,
            variance: VARIANCE_PENALTY,
        }
    }
}

/// Result of evaluating one final piece placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputerPlacement {
    /// Final anchor x.
    pub x: isize,
    /// Final anchor y.
    pub y: isize,
    /// Rotation orientation after placement search.
    pub orientation: u8,
    /// Lower scores are better.
    pub penalty: i32,
    /// Lines cleared by this placement in the simulation.
    pub lines_cleared: u32,
}

/// Stateful deterministic computer opponent strategy.
#[derive(Debug, Clone)]
pub struct ComputerOpponent {
    difficulty: ComputerDifficulty,
    rng: ChaCha12Rng,
    orders: Vec<WeaponOrder>,
    next_launch_opponent_lines: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WeaponOrder {
    token: WeaponToken,
    gate: LaunchGate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaunchGate {
    Opponent(u32),
    Mine(u32),
    Bazaar(u32),
}

impl ComputerOpponent {
    /// Creates a deterministic computer opponent at a legacy difficulty level.
    #[must_use]
    pub fn new(seed: GameSeed, level: usize) -> Self {
        let difficulty = computer_difficulty(level).unwrap_or(COMPUTER_DIFFICULTIES[0]);
        let mut strategy = Self {
            difficulty,
            rng: seed.stream(RngStream::ComputerOpponent),
            orders: Vec::new(),
            next_launch_opponent_lines: 0,
        };
        strategy.reset_orders();
        strategy
    }

    /// Returns this opponent's difficulty row.
    #[must_use]
    pub const fn difficulty(&self) -> ComputerDifficulty {
        self.difficulty
    }

    /// Chooses the minimum-penalty final placement for the loop's active piece.
    #[must_use]
    pub fn choose_placement(&self, piece_loop: &PieceLoop) -> Option<ComputerPlacement> {
        let piece = piece_loop.active_piece()?;
        choose_placement(piece_loop.board(), piece, EvaluationWeights::default())
    }

    /// Converts the chosen placement into deterministic core commands.
    #[must_use]
    pub fn commands_for_placement(
        &self,
        piece_loop: &PieceLoop,
        placement: &ComputerPlacement,
    ) -> Vec<Command> {
        let Some(piece) = piece_loop.active_piece() else {
            return Vec::new();
        };
        commands_for_placement(piece, placement)
    }

    /// Buys a deterministic legacy-inspired weapon combo during bazaar.
    pub fn shop(
        &mut self,
        bazaar: &mut Bazaar,
        my_lines: u32,
        opponent_lines: u32,
        board: &Board,
    ) -> Vec<WeaponToken> {
        if self.orders.is_empty() {
            self.next_launch_opponent_lines = opponent_lines;
            self.reset_orders();
        }

        let mut bought = Vec::new();
        let mut combo_cost = 0;
        while let Some(token) = self.next_purchase(bazaar, board, opponent_lines, combo_cost) {
            let price = bazaar.price(token);
            if bazaar.buy(token).is_err() {
                break;
            }
            bought.push(token);
            combo_cost += price;
            let gate = launch_gate_for_purchase(token, my_lines, self.next_launch_opponent_lines);
            self.orders.push(WeaponOrder { token, gate });
            if token == WeaponToken::Susan
                || combo_cost >= MIN_COMBO_COST
                || combo_cost >= MAX_COMBO_COST
            {
                break;
            }
        }
        bought
    }

    /// Returns launch slot labels whose order gates are satisfied.
    #[must_use]
    pub fn launch_slots(
        &mut self,
        arsenal: &Arsenal,
        my_lines: u32,
        opponent_lines: u32,
        bazaar_lines: u32,
    ) -> Vec<u8> {
        let mut slots = Vec::new();
        let mut remaining = Vec::new();
        for order in self.orders.drain(..) {
            if gate_satisfied(order.gate, my_lines, opponent_lines, bazaar_lines) {
                if let Some(label) = slot_label_for_token(arsenal, order.token) {
                    slots.push(label);
                }
            } else {
                remaining.push(order);
            }
        }
        self.orders = remaining;
        slots
    }

    fn reset_orders(&mut self) {
        self.orders.clear();
        self.orders.push(WeaponOrder {
            token: WeaponToken::Susan,
            gate: LaunchGate::Opponent(SUSAN_ENABLE_OPPONENT_LINES),
        });
    }

    fn next_purchase(
        &mut self,
        bazaar: &Bazaar,
        board: &Board,
        opponent_lines: u32,
        combo_cost: i32,
    ) -> Option<WeaponToken> {
        let top = board_top(board);
        let swap_allowed = top <= SWAP_TOP_LINE;
        let susan_allowed = opponent_lines >= SUSAN_ENABLE_OPPONENT_LINES;
        let mut candidates = [
            WeaponToken::NiceDay,
            WeaponToken::Reagan,
            WeaponToken::Speedy,
            WeaponToken::Mondale,
            WeaponToken::Carter,
            WeaponToken::SoLong,
            WeaponToken::Swap,
            WeaponToken::Susan,
            WeaponToken::Mirror,
            WeaponToken::Gimp,
        ];
        let offset = (self.rng.next_u64() as usize) % candidates.len();
        candidates.rotate_left(offset);
        candidates.into_iter().find(|token| {
            if *token == WeaponToken::Swap && !swap_allowed {
                return false;
            }
            if *token == WeaponToken::Susan && !susan_allowed {
                return false;
            }
            let price = bazaar.price(*token);
            price <= bazaar.staged_funds() && combo_cost + price <= MAX_COMBO_COST
        })
    }
}

/// Finds the best deterministic final placement for one piece on one board.
#[must_use]
pub fn choose_placement(
    board: &Board,
    piece: &Piece,
    weights: EvaluationWeights,
) -> Option<ComputerPlacement> {
    reachable_placements(board, piece, weights)
        .into_iter()
        .min_by_key(|placement| {
            (
                placement.penalty,
                placement.y,
                placement.x,
                placement.orientation,
            )
        })
}

/// Evaluates a board after a hypothetical placement. Lower scores are better.
#[must_use]
pub fn evaluate_board(
    board: &Board,
    piece_kind: PieceKind,
    lines_cleared: u32,
    weights: EvaluationWeights,
) -> i32 {
    let heights = column_heights(board);
    let holes = hole_counts(board, &heights);
    let max_height = heights.iter().copied().max().unwrap_or(0);
    let variance = height_variance(&heights);
    let happy_bonus = if piece_kind == PieceKind::Happy && lines_cleared > 0 {
        weights.happy_bonus
    } else {
        0
    };

    holes.open * weights.open_hole
        + holes.closed * weights.closed_hole
        + holes.covered * weights.covered_hole
        + max_height * max_height * weights.height / BOARD_HEIGHT as i32
        + variance * weights.variance
        - lines_cleared as i32 * weights.line_bonus
        - happy_bonus
}

fn reachable_placements(
    board: &Board,
    piece: &Piece,
    weights: EvaluationWeights,
) -> Vec<ComputerPlacement> {
    let mut placements = Vec::new();
    let mut oriented = piece.clone();
    let rotations = if piece.kind().rotation_width() == 0 {
        1
    } else {
        4
    };
    for _ in 0..rotations {
        let orientation = oriented.orientation();
        for x in -8..BOARD_WIDTH as isize + 8 {
            let mut candidate = oriented.clone();
            if !candidate.move_to(board, x, candidate.anchor().1) {
                continue;
            }
            while candidate.move_to(board, x, candidate.anchor().1 + 1) {}
            if !candidate.board_coords().iter().all(|&(cx, cy)| {
                cx >= 0 && cx < BOARD_WIDTH as isize && cy >= 0 && cy < BOARD_HEIGHT as isize
            }) {
                continue;
            }

            let mut simulated = board.clone();
            let kind = candidate.kind();
            let anchor = candidate.anchor();
            candidate.lock_into(&mut simulated);
            let outcome = simulated.clear_completed_lines(LineClearMode::Normal);
            let penalty = evaluate_board(&simulated, kind, outcome.lines_cleared, weights);
            placements.push(ComputerPlacement {
                x: anchor.0,
                y: anchor.1,
                orientation,
                penalty,
                lines_cleared: outcome.lines_cleared,
            });
        }

        if !oriented.rotate(board, false) {
            break;
        }
    }
    placements
}

fn commands_for_placement(piece: &Piece, placement: &ComputerPlacement) -> Vec<Command> {
    let mut commands = Vec::new();
    let rotations = (placement.orientation + 4 - piece.orientation()) % 4;
    for _ in 0..rotations {
        commands.push(Command::RotateClockwise);
    }

    let dx = placement.x - piece.anchor().0;
    let command = if dx < 0 {
        Command::MoveLeft
    } else {
        Command::MoveRight
    };
    for _ in 0..dx.unsigned_abs() {
        commands.push(command);
    }
    commands.push(Command::StartFastDrop);
    commands
}

fn launch_gate_for_purchase(token: WeaponToken, my_lines: u32, opponent_lines: u32) -> LaunchGate {
    match token {
        WeaponToken::SoLong | WeaponToken::Mondale | WeaponToken::Carter => {
            LaunchGate::Opponent(opponent_lines)
        }
        WeaponToken::Lawyers => LaunchGate::Mine(my_lines.saturating_add(1)),
        _ => LaunchGate::Bazaar(opponent_lines),
    }
}

const fn gate_satisfied(
    gate: LaunchGate,
    my_lines: u32,
    opponent_lines: u32,
    bazaar_lines: u32,
) -> bool {
    match gate {
        LaunchGate::Opponent(lines) => opponent_lines >= lines,
        LaunchGate::Mine(lines) => my_lines >= lines,
        LaunchGate::Bazaar(lines) => bazaar_lines >= lines,
    }
}

fn slot_label_for_token(arsenal: &Arsenal, token: WeaponToken) -> Option<u8> {
    arsenal
        .slots()
        .iter()
        .position(|slot| slot.is_some_and(|slot| slot.token == token))
        .map(|index| if index == 9 { 0 } else { index as u8 + 1 })
}

fn board_top(board: &Board) -> usize {
    (0..BOARD_HEIGHT)
        .find(|&y| (0..BOARD_WIDTH).any(|x| board.get(Coord { x, y }).is_some()))
        .unwrap_or(BOARD_HEIGHT)
}

fn column_heights(board: &Board) -> [i32; BOARD_WIDTH] {
    let mut heights = [0; BOARD_WIDTH];
    for (x, height) in heights.iter_mut().enumerate() {
        if let Some(y) = (0..BOARD_HEIGHT).find(|&y| board.get(Coord { x, y }).is_some()) {
            *height = (BOARD_HEIGHT - y) as i32;
        }
    }
    heights
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HoleCounts {
    open: i32,
    closed: i32,
    covered: i32,
}

fn hole_counts(board: &Board, heights: &[i32; BOARD_WIDTH]) -> HoleCounts {
    let mut counts = HoleCounts {
        open: 0,
        closed: 0,
        covered: 0,
    };
    for x in 0..BOARD_WIDTH {
        let mut seen_occupied = false;
        let mut covered = 0;
        for y in 0..BOARD_HEIGHT {
            match board.get(Coord { x, y }) {
                Some(_) => {
                    seen_occupied = true;
                    covered += 1;
                }
                None if seen_occupied => {
                    if has_side_escape(board, x, y) {
                        counts.open += 1;
                    } else {
                        counts.closed += 1;
                    }
                    counts.covered += covered;
                }
                None => {}
            }
        }
    }
    if heights.iter().all(|height| *height == 0) {
        counts.covered = 0;
    }
    counts
}

fn has_side_escape(board: &Board, x: usize, y: usize) -> bool {
    (0..x).all(|left| board.get(Coord { x: left, y }).is_none())
        || (x + 1..BOARD_WIDTH).all(|right| board.get(Coord { x: right, y }).is_none())
}

fn height_variance(heights: &[i32; BOARD_WIDTH]) -> i32 {
    let mean = heights.iter().sum::<i32>() / BOARD_WIDTH as i32;
    heights
        .iter()
        .map(|height| {
            let delta = *height - mean;
            delta * delta
        })
        .sum::<i32>()
        / BOARD_WIDTH as i32
}

#[cfg(test)]
mod tests {
    use super::{
        choose_placement, computer_difficulty, ComputerOpponent, EvaluationWeights,
        BAZAAR_LEAVE_DELAY_MS, COMPUTER_DIFFICULTIES,
    };
    use crate::{
        board::{Board, Coord},
        cell::Cell,
        game::Command,
        piece::{Piece, PieceKind},
        rng::GameSeed,
        weapons::{Arsenal, Bazaar, WeaponToken},
    };

    #[test]
    fn difficulty_table_preserves_legacy_delays_and_names() {
        assert_eq!(COMPUTER_DIFFICULTIES.len(), 15);
        assert_eq!(computer_difficulty(0).unwrap().name, "Comatose");
        assert_eq!(computer_difficulty(0).unwrap().delay_ms, 4000);
        assert_eq!(computer_difficulty(14).unwrap().name, "Bionic");
        assert_eq!(computer_difficulty(14).unwrap().delay_ms, 0);
        assert_eq!(BAZAAR_LEAVE_DELAY_MS, 3000);
    }

    #[test]
    fn placement_choice_is_reproducible_for_fixed_board() {
        let mut board = Board::empty();
        for x in 0..10 {
            if !(3..=6).contains(&x) {
                board.set(Coord { x, y: 27 }, Some(Cell::visible()));
            }
        }
        let piece = Piece::new(PieceKind::Long, 3, 0);

        let first = choose_placement(&board, &piece, EvaluationWeights::default()).unwrap();
        let second = choose_placement(&board, &piece, EvaluationWeights::default()).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.lines_cleared, 1);
    }

    #[test]
    fn evaluation_penalizes_holes_and_rewards_line_clears() {
        let weights = EvaluationWeights::default();
        let mut clean = Board::empty();
        let mut holey = Board::empty();
        for x in 0..10 {
            clean.set(Coord { x, y: 27 }, Some(Cell::visible()));
            if x != 5 {
                holey.set(Coord { x, y: 27 }, Some(Cell::visible()));
            }
        }
        holey.set(Coord { x: 5, y: 26 }, Some(Cell::visible()));

        assert!(
            super::evaluate_board(&holey, PieceKind::Plug, 0, weights)
                > super::evaluate_board(&clean, PieceKind::Plug, 0, weights)
        );
        assert!(
            super::evaluate_board(&clean, PieceKind::Plug, 1, weights)
                < super::evaluate_board(&clean, PieceKind::Plug, 0, weights)
        );
        assert!(
            super::evaluate_board(&clean, PieceKind::Happy, 1, weights)
                < super::evaluate_board(&clean, PieceKind::Plug, 1, weights)
        );
    }

    #[test]
    fn commands_drive_toward_chosen_placement() {
        let piece = Piece::new(PieceKind::Plug, 4, 0);
        let placement = super::ComputerPlacement {
            x: 2,
            y: 25,
            orientation: 1,
            penalty: 0,
            lines_cleared: 0,
        };

        assert_eq!(
            super::commands_for_placement(&piece, &placement),
            vec![
                Command::RotateClockwise,
                Command::MoveLeft,
                Command::MoveLeft,
                Command::StartFastDrop,
            ]
        );
    }

    #[test]
    fn shopping_and_launch_orders_are_deterministic() {
        let mut first = ComputerOpponent::new(GameSeed::from_u64(8), 5);
        let mut second = ComputerOpponent::new(GameSeed::from_u64(8), 5);
        let mut first_bazaar = Bazaar::new(Arsenal::new(), 1000, false);
        let mut second_bazaar = Bazaar::new(Arsenal::new(), 1000, false);
        let board = Board::empty();

        let first_bought = first.shop(&mut first_bazaar, 3, 50, &board);
        let second_bought = second.shop(&mut second_bazaar, 3, 50, &board);

        assert_eq!(first_bought, second_bought);
        assert!(!first_bought.is_empty());

        let mut susan_orders = ComputerOpponent::new(GameSeed::from_u64(8), 5);
        let mut arsenal = Arsenal::new();
        arsenal.buy_weapon(WeaponToken::Susan).unwrap();
        assert_eq!(
            susan_orders.launch_slots(&arsenal, 3, 49, 0),
            Vec::<u8>::new()
        );
        assert!(!susan_orders.launch_slots(&arsenal, 3, 50, 0).is_empty());
    }
}
