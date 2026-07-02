//! Deterministic spy/recon snapshots for William Ames, Ace of Spies, and Condor.

use rand::Rng;

use crate::{
    board::{Board, BoardSnapshot, Coord, BOARD_HEIGHT, BOARD_WIDTH},
    cell::Cell,
    weapons::WeaponToken,
};

/// Active recon quality for a spy weapon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconLevel {
    /// William Ames: cheap, noisy spy.
    Ames,
    /// Ace of Spies: mostly accurate spy.
    Ace,
    /// The Condor: exact spy satellite.
    Condor,
}

impl ReconLevel {
    /// Converts a spy weapon token to its recon level.
    #[must_use]
    pub const fn from_token(token: WeaponToken) -> Option<Self> {
        match token {
            WeaponToken::Ames => Some(Self::Ames),
            WeaponToken::Ace => Some(Self::Ace),
            WeaponToken::Condor => Some(Self::Condor),
            _ => None,
        }
    }

    const fn occupied_report_percent(self) -> u32 {
        match self {
            Self::Ames => 50,
            Self::Ace => 85,
            Self::Condor => 100,
        }
    }
}

/// One recon panel update from a target board to a viewing player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconSnapshot {
    /// Recon quality that produced this sample.
    pub level: ReconLevel,
    /// Sampled target board. Unreported occupied cells appear empty.
    pub board: BoardSnapshot,
    /// Sampled target funds.
    pub funds: i32,
}

/// Samples a target board/funds pair using the active recon level.
#[must_use]
pub fn sample_recon(
    level: ReconLevel,
    board: &Board,
    funds: i32,
    target_cleared_lines: u32,
    rng: &mut impl Rng,
) -> ReconSnapshot {
    let mut cells = Vec::with_capacity(BOARD_WIDTH * BOARD_HEIGHT);
    let threshold = level.occupied_report_percent();
    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            let cell = board.get(Coord { x, y });
            cells.push(report_cell(cell, threshold, rng));
        }
    }

    ReconSnapshot {
        level,
        board: BoardSnapshot {
            width: BOARD_WIDTH,
            height: BOARD_HEIGHT,
            cells,
        },
        funds: report_funds(level, funds, target_cleared_lines, rng),
    }
}

fn report_cell(cell: Option<Cell>, threshold: u32, rng: &mut impl Rng) -> Option<Cell> {
    let cell = cell?;
    if threshold == 100 || (rng.next_u64() % 100) < u64::from(threshold) {
        Some(cell)
    } else {
        None
    }
}

fn report_funds(
    level: ReconLevel,
    funds: i32,
    target_cleared_lines: u32,
    rng: &mut impl Rng,
) -> i32 {
    match level {
        ReconLevel::Condor => funds,
        ReconLevel::Ace if target_cleared_lines == 4 => funds + (rng.next_u64() % 100) as i32,
        ReconLevel::Ace => funds,
        ReconLevel::Ames if funds > 0 => (rng.next_u64() % funds as u64) as i32,
        ReconLevel::Ames => 0,
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand_chacha::ChaCha12Rng;

    use super::{sample_recon, ReconLevel};
    use crate::{
        board::{Board, Coord},
        cell::Cell,
    };

    #[test]
    fn condor_reports_exact_board_and_funds() {
        let mut board = Board::empty();
        board.set(Coord::new(3, 4).unwrap(), Some(Cell::visible()));
        let mut rng = ChaCha12Rng::seed_from_u64(1);

        let snapshot = sample_recon(ReconLevel::Condor, &board, 225, 0, &mut rng);

        assert_eq!(snapshot.funds, 225);
        assert_eq!(snapshot.board.cells, board.snapshot().cells);
    }

    #[test]
    fn ames_and_ace_sample_occupied_cells_deterministically() {
        let mut board = Board::empty();
        for y in 0..10 {
            for x in 0..10 {
                board.set(Coord::new(x, y).unwrap(), Some(Cell::visible()));
            }
        }
        let mut ames_rng = ChaCha12Rng::seed_from_u64(7);
        let mut ace_rng = ChaCha12Rng::seed_from_u64(7);

        let ames = sample_recon(ReconLevel::Ames, &board, 100, 0, &mut ames_rng);
        let ace = sample_recon(ReconLevel::Ace, &board, 100, 0, &mut ace_rng);

        let ames_seen = ames
            .board
            .cells
            .iter()
            .filter(|cell| cell.is_some())
            .count();
        let ace_seen = ace.board.cells.iter().filter(|cell| cell.is_some()).count();
        assert!(ames_seen < ace_seen);
        assert_eq!(ace.funds, 100);
    }
}
