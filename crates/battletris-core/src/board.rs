//! Board dimensions, coordinates, occupancy, and snapshots.
//!
//! The board uses top-left origin coordinates: `x` increases to the right and
//! normal gravity increases `y`. Legacy storage was column-major internally, but
//! snapshots are row-major and this module exposes row-major snapshot data.

use rand::Rng;

use crate::cell::Cell;

/// Legacy board width in cells.
pub const BOARD_WIDTH: usize = 10;

/// Legacy board height in cells.
pub const BOARD_HEIGHT: usize = 28;

/// How a completed row is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineClearMode {
    /// Legacy default: remove the row, drop all rows above it, and recheck the same row index.
    Normal,
    /// The Force weapon behavior: erase the row without dropping rows above it.
    Force,
}

/// Result of a completed-line scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineClearOutcome {
    /// Number of completed rows removed.
    pub lines_cleared: u32,
    /// Funds awarded by this scan.
    pub funds: i32,
    /// Happy cells converted to frowns because they were scanned in non-full rows.
    pub happy_missed: u32,
    /// Original row indexes removed, in legacy bottom-up scan order.
    pub cleared_rows: Vec<usize>,
}

/// A board coordinate with top-left origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coord {
    /// Horizontal position, where `0` is the left edge.
    pub x: usize,
    /// Vertical position, where `0` is the top row.
    pub y: usize,
}

impl Coord {
    /// Creates an in-bounds coordinate.
    #[must_use]
    pub const fn new(x: usize, y: usize) -> Option<Self> {
        if x < BOARD_WIDTH && y < BOARD_HEIGHT {
            Some(Self { x, y })
        } else {
            None
        }
    }
}

/// A typed, row-major board snapshot that preserves all core cell state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardSnapshot {
    /// Snapshot width in cells.
    pub width: usize,
    /// Snapshot height in cells.
    pub height: usize,
    /// Row-major cells indexed by `y * width + x`.
    pub cells: Vec<Option<Cell>>,
}

impl BoardSnapshot {
    /// Validates and creates a typed board snapshot.
    pub fn new(width: usize, height: usize, cells: Vec<Option<Cell>>) -> Result<Self, BoardError> {
        if width != BOARD_WIDTH || height != BOARD_HEIGHT {
            return Err(BoardError::InvalidDimensions { width, height });
        }
        if cells.len() != width * height {
            return Err(BoardError::InvalidCellCount {
                expected: width * height,
                actual: cells.len(),
            });
        }

        Ok(Self {
            width,
            height,
            cells,
        })
    }
}

/// A legacy-compatible board snapshot with row-major numeric IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBoardSnapshot {
    /// Legacy motivation field preserved for future protocol adapters.
    pub motivation: i32,
    /// Snapshot width in cells.
    pub width: usize,
    /// Snapshot height in cells.
    pub height: usize,
    /// Row-major legacy IDs indexed by `y * width + x`.
    pub ids: Vec<i16>,
}

impl LegacyBoardSnapshot {
    /// Validates and creates a legacy board snapshot.
    pub fn new(
        motivation: i32,
        width: usize,
        height: usize,
        ids: Vec<i16>,
    ) -> Result<Self, BoardError> {
        if width != BOARD_WIDTH || height != BOARD_HEIGHT {
            return Err(BoardError::InvalidDimensions { width, height });
        }
        if ids.len() != width * height {
            return Err(BoardError::InvalidCellCount {
                expected: width * height,
                actual: ids.len(),
            });
        }

        Ok(Self {
            motivation,
            width,
            height,
            ids,
        })
    }
}

/// Errors returned when constructing or mutating boards and snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardError {
    /// Width or height differs from the legacy board dimensions.
    InvalidDimensions {
        /// Snapshot or input width.
        width: usize,
        /// Snapshot or input height.
        height: usize,
    },
    /// Snapshot cell count does not match its dimensions.
    InvalidCellCount {
        /// Required number of row-major cells.
        expected: usize,
        /// Provided number of row-major cells.
        actual: usize,
    },
    /// A coordinate was outside the board.
    OutOfBounds {
        /// Horizontal coordinate.
        x: usize,
        /// Vertical coordinate.
        y: usize,
    },
}

/// A player's rectangular BattleTris board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    cells: Vec<Option<Cell>>,
}

impl Default for Board {
    fn default() -> Self {
        Self::empty()
    }
}

impl Board {
    /// Creates an empty legacy-sized board.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            cells: vec![None; BOARD_WIDTH * BOARD_HEIGHT],
        }
    }

    /// Recreates a board from a typed snapshot.
    pub fn from_snapshot(snapshot: BoardSnapshot) -> Result<Self, BoardError> {
        if snapshot.width != BOARD_WIDTH || snapshot.height != BOARD_HEIGHT {
            return Err(BoardError::InvalidDimensions {
                width: snapshot.width,
                height: snapshot.height,
            });
        }
        if snapshot.cells.len() != BOARD_WIDTH * BOARD_HEIGHT {
            return Err(BoardError::InvalidCellCount {
                expected: BOARD_WIDTH * BOARD_HEIGHT,
                actual: snapshot.cells.len(),
            });
        }

        Ok(Self {
            cells: snapshot.cells,
        })
    }

    /// Recreates a board from a legacy ID snapshot.
    ///
    /// This intentionally loses legacy-invisible cells because ID `0` means both
    /// empty and Bug Report invisible in the C++ snapshot representation.
    pub fn from_legacy_snapshot(snapshot: LegacyBoardSnapshot) -> Result<Self, BoardError> {
        if snapshot.width != BOARD_WIDTH || snapshot.height != BOARD_HEIGHT {
            return Err(BoardError::InvalidDimensions {
                width: snapshot.width,
                height: snapshot.height,
            });
        }
        if snapshot.ids.len() != BOARD_WIDTH * BOARD_HEIGHT {
            return Err(BoardError::InvalidCellCount {
                expected: BOARD_WIDTH * BOARD_HEIGHT,
                actual: snapshot.ids.len(),
            });
        }

        let cells = snapshot.ids.into_iter().map(Cell::from_legacy_id).collect();

        Ok(Self { cells })
    }

    /// Returns the legacy board width.
    #[must_use]
    pub const fn width(&self) -> usize {
        BOARD_WIDTH
    }

    /// Returns the legacy board height.
    #[must_use]
    pub const fn height(&self) -> usize {
        BOARD_HEIGHT
    }

    /// Returns the cell at an in-bounds coordinate.
    #[must_use]
    pub fn get(&self, coord: Coord) -> Option<Cell> {
        self.cells[Self::index(coord)]
    }

    /// Sets a cell at an in-bounds coordinate.
    pub fn set(&mut self, coord: Coord, cell: Option<Cell>) {
        let index = Self::index(coord);
        self.cells[index] = cell;
    }

    /// Sets a cell by raw coordinates, returning an error when out of bounds.
    pub fn try_set(&mut self, x: usize, y: usize, cell: Option<Cell>) -> Result<(), BoardError> {
        let coord = Coord::new(x, y).ok_or(BoardError::OutOfBounds { x, y })?;
        self.set(coord, cell);
        Ok(())
    }

    /// Returns whether a signed coordinate is outside the board.
    #[must_use]
    pub const fn is_out_of_bounds(x: isize, y: isize) -> bool {
        x < 0 || y < 0 || x >= BOARD_WIDTH as isize || y >= BOARD_HEIGHT as isize
    }

    /// Returns true when a coordinate is out of bounds or contains a cell.
    #[must_use]
    pub fn is_occupied(&self, x: isize, y: isize) -> bool {
        if Self::is_out_of_bounds(x, y) {
            return true;
        }

        self.cells[y as usize * BOARD_WIDTH + x as usize].is_some()
    }

    /// Fallout occupancy lets the middle six columns fall below the board.
    #[must_use]
    pub fn is_occupied_with_fallout(&self, x: isize, y: isize, fallout: bool) -> bool {
        if fallout && (2..=7).contains(&x) && y >= BOARD_HEIGHT as isize {
            return false;
        }

        self.is_occupied(x, y)
    }

    /// Scans for completed rows, applies legacy row removal, and converts missed happy cells.
    pub fn clear_completed_lines(&mut self, mode: LineClearMode) -> LineClearOutcome {
        let mut y = BOARD_HEIGHT as isize - 1;
        let mut cleared_rows = Vec::new();
        let mut cleared_value = 0;
        let mut happy_missed = 0;

        while y >= 0 {
            let row = y as usize;
            if self.row_is_full(row) {
                cleared_value += self.row_value(row);
                cleared_rows.push(row);
                match mode {
                    LineClearMode::Normal => self.drop_rows_above(row),
                    LineClearMode::Force => {
                        self.clear_row(row);
                        y -= 1;
                    }
                }
            } else {
                happy_missed += self.convert_happy_row(row);
                y -= 1;
            }
        }

        let lines_cleared = cleared_rows.len() as u32;
        LineClearOutcome {
            lines_cleared,
            funds: cleared_value * lines_cleared as i32,
            happy_missed,
            cleared_rows,
        }
    }

    /// Inserts one legacy garbage line at the bottom, pushing existing rows up.
    pub fn insert_garbage_line(&mut self, hole: usize) {
        let hole = hole % BOARD_WIDTH;
        for y in 0..BOARD_HEIGHT - 1 {
            for x in 0..BOARD_WIDTH {
                self.cells[y * BOARD_WIDTH + x] = self.cells[(y + 1) * BOARD_WIDTH + x];
            }
        }

        for x in 0..BOARD_WIDTH {
            self.cells[(BOARD_HEIGHT - 1) * BOARD_WIDTH + x] = if x == hole {
                None
            } else {
                Some(Cell::visible())
            };
        }
    }

    /// Mirrors board contents on the vertical axis, matching Flip out.
    pub fn flip_on_vertical_axis(&mut self) {
        for y in 0..BOARD_HEIGHT {
            for x in 0..BOARD_WIDTH / 2 {
                let left = y * BOARD_WIDTH + x;
                let right = y * BOARD_WIDTH + (BOARD_WIDTH - 1 - x);
                self.cells.swap(left, right);
            }
        }
    }

    /// Mirrors board contents on the horizontal axis, matching Upbyside-down.
    pub fn flip_on_horizontal_axis(&mut self) {
        for y in 0..BOARD_HEIGHT / 2 {
            for x in 0..BOARD_WIDTH {
                let top = y * BOARD_WIDTH + x;
                let bottom = (BOARD_HEIGHT - 1 - y) * BOARD_WIDTH + x;
                self.cells.swap(top, bottom);
            }
        }
    }

    /// Removes cells in the Fallout black-hole columns.
    pub fn clear_fallout_hole(&mut self) {
        for y in 0..BOARD_HEIGHT {
            for x in 2..=7 {
                self.set(Coord { x, y }, None);
            }
        }
    }

    /// Removes the next removable occupied cell from a legacy row-major wrapped scan.
    pub fn remove_next_removable_from(&mut self, start_x: usize, start_y: usize) -> Option<Coord> {
        let start_x = start_x % BOARD_WIDTH;
        let start_y = start_y % BOARD_HEIGHT;

        for y_offset in 0..BOARD_HEIGHT {
            let y = (start_y + y_offset) % BOARD_HEIGHT;
            for x_offset in 0..BOARD_WIDTH {
                let x = (start_x + x_offset) % BOARD_WIDTH;
                let index = y * BOARD_WIDTH + x;
                if self.cells[index].is_some_and(Cell::is_removable) {
                    self.cells[index] = None;
                    return Coord::new(x, y);
                }
            }
        }

        None
    }

    /// Adds a random occupied cell in the middle half of the board.
    pub fn add_random_middle_cell(&mut self, rng: &mut impl Rng, cell: Cell) -> Option<Coord> {
        let candidates = (BOARD_HEIGHT / 4..BOARD_HEIGHT * 3 / 4)
            .flat_map(|y| (0..BOARD_WIDTH).map(move |x| Coord { x, y }))
            .filter(|coord| self.get(*coord).is_none())
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return None;
        }

        let index = (rng.next_u64() as usize) % candidates.len();
        let coord = candidates[index];
        self.set(coord, Some(cell));
        Some(coord)
    }

    /// Randomly removes about half of all removable cells, matching Blind Cleric.
    pub fn remove_random_half_removable(&mut self, rng: &mut impl Rng) -> u32 {
        let mut removed = 0;
        for cell in &mut self.cells {
            if cell.is_some_and(Cell::is_removable) && rng.next_u64().is_multiple_of(2) {
                *cell = None;
                removed += 1;
            }
        }
        removed
    }

    /// Hides every existing cell with no undo path, preserving value and removability.
    pub fn hide_existing_cells(&mut self) -> u32 {
        let mut hidden = 0;
        for cell in &mut self.cells {
            if let Some(existing) = *cell {
                *cell = Some(Cell::Hidden {
                    value: existing.value(),
                    removable: existing.is_removable(),
                });
                hidden += 1;
            }
        }
        hidden
    }

    /// Replaces removable existing cells with Gimp cells, preserving funds value.
    pub fn gimp_removable_cells(&mut self) -> u32 {
        let mut gimped = 0;
        for cell in &mut self.cells {
            if let Some(existing) = *cell {
                if existing.is_removable() {
                    *cell = Some(Cell::Gimp {
                        value: existing.value(),
                    });
                    gimped += 1;
                }
            }
        }
        gimped
    }

    /// Builds Bottle neck side walls. Overwritten side cells are intentionally lost.
    pub fn add_bottle_neck(&mut self) {
        for y in BOARD_HEIGHT - 18..BOARD_HEIGHT - 10 {
            for x in [0, 1, 2, 7, 8, 9] {
                self.set(Coord { x, y }, Some(Cell::Structure));
            }
        }
    }

    /// Removes Bottle neck side wall structures without restoring overwritten cells.
    pub fn remove_bottle_neck(&mut self) {
        for y in BOARD_HEIGHT - 18..BOARD_HEIGHT - 10 {
            for x in [0, 1, 2, 7, 8, 9] {
                if self.get(Coord { x, y }) == Some(Cell::Structure) {
                    self.set(Coord { x, y }, None);
                }
            }
        }
    }

    /// Creates a typed row-major snapshot preserving all core cell state.
    #[must_use]
    pub fn snapshot(&self) -> BoardSnapshot {
        BoardSnapshot {
            width: BOARD_WIDTH,
            height: BOARD_HEIGHT,
            cells: self.cells.clone(),
        }
    }

    /// Creates a legacy row-major ID snapshot.
    ///
    /// When `upside_down` is true, row order is reversed to match the legacy
    /// `BTBoard` constructor behavior; columns within each row are unchanged.
    #[must_use]
    pub fn legacy_snapshot(&self, motivation: i32, upside_down: bool) -> LegacyBoardSnapshot {
        let mut ids = Vec::with_capacity(self.cells.len());
        let rows: Box<dyn Iterator<Item = usize>> = if upside_down {
            Box::new((0..BOARD_HEIGHT).rev())
        } else {
            Box::new(0..BOARD_HEIGHT)
        };

        for y in rows {
            for x in 0..BOARD_WIDTH {
                let id = self.cells[y * BOARD_WIDTH + x].map_or(0, Cell::legacy_id);
                ids.push(id);
            }
        }

        LegacyBoardSnapshot {
            motivation,
            width: BOARD_WIDTH,
            height: BOARD_HEIGHT,
            ids,
        }
    }

    const fn index(coord: Coord) -> usize {
        coord.y * BOARD_WIDTH + coord.x
    }

    fn row_is_full(&self, row: usize) -> bool {
        (0..BOARD_WIDTH).all(|x| self.cells[row * BOARD_WIDTH + x].is_some())
    }

    fn row_value(&self, row: usize) -> i32 {
        (0..BOARD_WIDTH)
            .filter_map(|x| self.cells[row * BOARD_WIDTH + x])
            .map(Cell::value)
            .sum()
    }

    fn clear_row(&mut self, row: usize) {
        for x in 0..BOARD_WIDTH {
            self.cells[row * BOARD_WIDTH + x] = None;
        }
    }

    fn drop_rows_above(&mut self, row: usize) {
        for y in (1..=row).rev() {
            for x in 0..BOARD_WIDTH {
                self.cells[y * BOARD_WIDTH + x] = self.cells[(y - 1) * BOARD_WIDTH + x];
            }
        }
        self.clear_row(0);
    }

    fn convert_happy_row(&mut self, row: usize) -> u32 {
        let mut converted = 0;
        for x in 0..BOARD_WIDTH {
            let index = row * BOARD_WIDTH + x;
            if self.cells[index] == Some(Cell::Happy) {
                self.cells[index] = Some(Cell::Frown);
                converted += 1;
            }
        }
        converted
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Board, BoardSnapshot, Coord, LegacyBoardSnapshot, LineClearMode, BOARD_HEIGHT, BOARD_WIDTH,
    };
    use crate::cell::{Cell, Pip};

    #[test]
    fn board_dimensions_match_legacy_constants() {
        let board = Board::empty();

        assert_eq!(BOARD_WIDTH, 10);
        assert_eq!(BOARD_HEIGHT, 28);
        assert_eq!(board.width(), 10);
        assert_eq!(board.height(), 28);
    }

    #[test]
    fn coordinate_constructor_rejects_out_of_bounds() {
        assert_eq!(Coord::new(0, 0), Some(Coord { x: 0, y: 0 }));
        assert_eq!(Coord::new(9, 27), Some(Coord { x: 9, y: 27 }));
        assert_eq!(Coord::new(10, 0), None);
        assert_eq!(Coord::new(0, 28), None);
    }

    #[test]
    fn occupancy_treats_out_of_bounds_as_occupied() {
        let mut board = Board::empty();
        board.set(Coord::new(3, 4).expect("in bounds"), Some(Cell::visible()));

        assert!(!board.is_occupied(0, 0));
        assert!(board.is_occupied(3, 4));
        assert!(board.is_occupied(-1, 0));
        assert!(board.is_occupied(0, -1));
        assert!(board.is_occupied(10, 0));
        assert!(board.is_occupied(0, 28));
    }

    #[test]
    fn typed_snapshot_round_trips_all_cell_state() {
        let mut board = Board::empty();
        board.set(Coord::new(0, 0).unwrap(), Some(Cell::Invisible));
        board.set(
            Coord::new(1, 0).unwrap(),
            Some(Cell::Hidden {
                value: 5,
                removable: false,
            }),
        );
        board.set(Coord::new(2, 0).unwrap(), Some(Cell::Gimp { value: 6 }));

        let snapshot = board.snapshot();
        let restored = Board::from_snapshot(snapshot).expect("valid snapshot");

        assert_eq!(restored, board);
    }

    #[test]
    fn legacy_snapshot_maps_ids_row_major() {
        let mut board = Board::empty();
        board.set(Coord::new(0, 0).unwrap(), Some(Cell::visible()));
        board.set(Coord::new(1, 0).unwrap(), Some(Cell::Structure));
        board.set(Coord::new(2, 0).unwrap(), Some(Cell::Happy));
        board.set(Coord::new(3, 0).unwrap(), Some(Cell::Frown));
        board.set(Coord::new(4, 0).unwrap(), Some(Cell::Gimp { value: 3 }));
        board.set(
            Coord::new(5, 0).unwrap(),
            Some(Cell::die(Pip::new(6).unwrap())),
        );
        board.set(Coord::new(6, 0).unwrap(), Some(Cell::Invisible));
        board.set(
            Coord::new(7, 0).unwrap(),
            Some(Cell::Hidden {
                value: 0,
                removable: true,
            }),
        );

        let snapshot = board.legacy_snapshot(42, false);

        assert_eq!(snapshot.motivation, 42);
        assert_eq!(&snapshot.ids[0..10], &[1, 20, 21, 22, 23, 29, 0, -1, 0, 0]);
    }

    #[test]
    fn legacy_upside_down_snapshot_reverses_rows_only() {
        let mut board = Board::empty();
        board.set(Coord::new(0, 0).unwrap(), Some(Cell::visible()));
        board.set(Coord::new(9, 27).unwrap(), Some(Cell::Structure));

        let normal = board.legacy_snapshot(0, false);
        let upside_down = board.legacy_snapshot(0, true);

        assert_eq!(normal.ids[0], 1);
        assert_eq!(normal.ids[BOARD_WIDTH * BOARD_HEIGHT - 1], 20);
        assert_eq!(upside_down.ids[9], 20);
        assert_eq!(upside_down.ids[BOARD_WIDTH * (BOARD_HEIGHT - 1)], 1);
    }

    #[test]
    fn legacy_snapshot_round_trip_loses_invisible_and_hidden_cells() {
        let mut board = Board::empty();
        board.set(Coord::new(0, 0).unwrap(), Some(Cell::Invisible));
        board.set(
            Coord::new(1, 0).unwrap(),
            Some(Cell::Hidden {
                value: 150,
                removable: true,
            }),
        );
        board.set(
            Coord::new(2, 0).unwrap(),
            Some(Cell::die(Pip::new(3).unwrap())),
        );

        let restored = Board::from_legacy_snapshot(board.legacy_snapshot(0, false))
            .expect("valid legacy snapshot");

        assert_eq!(restored.get(Coord::new(0, 0).unwrap()), None);
        assert_eq!(restored.get(Coord::new(1, 0).unwrap()), None);
        assert_eq!(
            restored.get(Coord::new(2, 0).unwrap()),
            Some(Cell::die(Pip::new(3).unwrap()))
        );
    }

    #[test]
    fn snapshots_validate_dimensions_and_cell_count() {
        assert!(BoardSnapshot::new(9, 28, vec![None; 9 * 28]).is_err());
        assert!(LegacyBoardSnapshot::new(0, 10, 27, vec![0; 10 * 27]).is_err());
        assert!(BoardSnapshot::new(10, 28, vec![None; 3]).is_err());
        assert!(LegacyBoardSnapshot::new(0, 10, 28, vec![0; 3]).is_err());
        assert!(Board::from_snapshot(BoardSnapshot {
            width: 10,
            height: 28,
            cells: vec![None; 3],
        })
        .is_err());
        assert!(Board::from_legacy_snapshot(LegacyBoardSnapshot {
            motivation: 0,
            width: 10,
            height: 28,
            ids: vec![0; 3],
        })
        .is_err());
    }

    #[test]
    fn single_line_clear_drops_rows_and_awards_die_funds() {
        let mut board = Board::empty();
        board.set(Coord::new(0, 25).unwrap(), Some(Cell::visible()));
        for x in 0..BOARD_WIDTH {
            let cell = if x == 3 {
                Cell::die(Pip::new(4).unwrap())
            } else {
                Cell::visible()
            };
            board.set(Coord::new(x, 27).unwrap(), Some(cell));
        }

        let outcome = board.clear_completed_lines(LineClearMode::Normal);

        assert_eq!(outcome.lines_cleared, 1);
        assert_eq!(outcome.funds, 4);
        assert_eq!(outcome.cleared_rows, vec![27]);
        assert_eq!(board.get(Coord::new(0, 26).unwrap()), Some(Cell::visible()));
        assert_eq!(board.get(Coord::new(0, 25).unwrap()), None);
    }

    #[test]
    fn multi_line_funds_are_value_sum_times_lines_cleared() {
        for (lines, pips, expected_funds) in [
            (2, &[2, 4][..], 12),
            (3, &[1, 2, 3][..], 18),
            (4, &[1, 2, 3, 4][..], 40),
        ] {
            let mut board = Board::empty();
            for (offset, pip) in pips.iter().enumerate() {
                let y = BOARD_HEIGHT - 1 - offset;
                for x in 0..BOARD_WIDTH {
                    let cell = if x == 0 {
                        Cell::die(Pip::new(*pip).unwrap())
                    } else {
                        Cell::visible()
                    };
                    board.set(Coord::new(x, y).unwrap(), Some(cell));
                }
            }

            let outcome = board.clear_completed_lines(LineClearMode::Normal);

            assert_eq!(outcome.lines_cleared, lines);
            assert_eq!(outcome.funds, expected_funds);
        }
    }

    #[test]
    fn happy_cells_pay_when_cleared_and_frown_when_missed() {
        let mut board = Board::empty();
        board.set(Coord::new(0, 26).unwrap(), Some(Cell::Happy));
        for x in 0..BOARD_WIDTH {
            let cell = if x == 5 { Cell::Happy } else { Cell::visible() };
            board.set(Coord::new(x, 27).unwrap(), Some(cell));
        }

        let outcome = board.clear_completed_lines(LineClearMode::Normal);

        assert_eq!(outcome.lines_cleared, 1);
        assert_eq!(outcome.funds, 150);
        assert_eq!(outcome.happy_missed, 1);
        assert_eq!(board.get(Coord::new(0, 27).unwrap()), Some(Cell::Frown));
    }

    #[test]
    fn force_clear_erases_rows_without_dropping_or_rechecking() {
        let mut board = Board::empty();
        board.set(Coord::new(0, 25).unwrap(), Some(Cell::visible()));
        for y in [26, 27] {
            for x in 0..BOARD_WIDTH {
                board.set(Coord::new(x, y).unwrap(), Some(Cell::visible()));
            }
        }

        let outcome = board.clear_completed_lines(LineClearMode::Force);

        assert_eq!(outcome.lines_cleared, 2);
        assert_eq!(board.get(Coord::new(0, 25).unwrap()), Some(Cell::visible()));
        assert_eq!(board.get(Coord::new(0, 26).unwrap()), None);
        assert_eq!(board.get(Coord::new(0, 27).unwrap()), None);
    }
}
