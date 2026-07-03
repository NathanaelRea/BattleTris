//! Legacy piece IDs, shapes, spawn anchors, rotation, and collision.
//!
//! Pieces use the legacy 8x8 local map coordinate space. Rotation operates only
//! on occupied local cells, so empty margins may hang outside the board exactly
//! as they did in the C++ implementation.

use crate::{
    board::{Board, BOARD_WIDTH},
    cell::{Cell, Pip, VisibleColor},
};

/// Width and height of the legacy piece map.
pub const PIECE_MAP_SIZE: usize = 8;

/// Legacy spawn x before centering by the piece rotation width.
pub const SPAWN_X: isize = 5;

/// Legacy spawn y.
pub const SPAWN_Y: isize = 0;

/// Local coordinate in the legacy 8x8 piece map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalCoord {
    /// Horizontal local coordinate.
    pub x: u8,
    /// Vertical local coordinate.
    pub y: u8,
}

impl LocalCoord {
    const fn new(x: u8, y: u8) -> Self {
        Self { x, y }
    }
}

/// Legacy piece identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PieceKind {
    /// `BT_EL_PIECE`.
    El = 1,
    /// `BT_REL_PIECE`.
    ReverseEl = 2,
    /// `BT_SL_RT_PIECE`.
    SlantRight = 3,
    /// `BT_SL_LF_PIECE`.
    SlantLeft = 4,
    /// `BT_LONG_PIECE`.
    Long = 5,
    /// `BT_PLUG_PIECE`.
    Plug = 6,
    /// `BT_BOX_PIECE`.
    Box = 7,
    /// `BT_DIE_PIECE`.
    Die = 8,
    /// `BT_HAP_PIECE`.
    Happy = 9,
    /// `BT_DOG_PIECE`.
    Dog = 10,
    /// `BT_RDOG_PIECE`.
    ReverseDog = 11,
    /// `BT_CAP_PIECE`.
    Cap = 12,
    /// `BT_WALL_PIECE`.
    Wall = 13,
    /// `BT_TOWER_PIECE`.
    Tower = 14,
    /// `BT_STAR_PIECE`.
    Star = 15,
    /// `BT_WLONG_PIECE`.
    WeirdLong = 16,
    /// `BT_4x4_PIECE`.
    FourByFour = 17,
    /// `BT_LONG_DONG_PIECE`.
    LongDong = 18,
}

impl PieceKind {
    /// Number of legacy piece kinds.
    pub const COUNT: usize = 18;

    /// Returns the legacy numeric piece ID.
    #[must_use]
    pub const fn legacy_id(self) -> u8 {
        self as u8
    }

    /// Returns a zero-based index in legacy numeric ID order.
    #[must_use]
    pub const fn index(self) -> usize {
        self.legacy_id() as usize - 1
    }

    /// Returns the legacy token name.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::El => "BT_EL_PIECE",
            Self::ReverseEl => "BT_REL_PIECE",
            Self::SlantRight => "BT_SL_RT_PIECE",
            Self::SlantLeft => "BT_SL_LF_PIECE",
            Self::Long => "BT_LONG_PIECE",
            Self::Plug => "BT_PLUG_PIECE",
            Self::Box => "BT_BOX_PIECE",
            Self::Die => "BT_DIE_PIECE",
            Self::Happy => "BT_HAP_PIECE",
            Self::Dog => "BT_DOG_PIECE",
            Self::ReverseDog => "BT_RDOG_PIECE",
            Self::Cap => "BT_CAP_PIECE",
            Self::Wall => "BT_WALL_PIECE",
            Self::Tower => "BT_TOWER_PIECE",
            Self::Star => "BT_STAR_PIECE",
            Self::WeirdLong => "BT_WLONG_PIECE",
            Self::FourByFour => "BT_4x4_PIECE",
            Self::LongDong => "BT_LONG_DONG_PIECE",
        }
    }

    /// Returns the legacy rotation-square width. `0` means not rotatable.
    #[must_use]
    pub const fn rotation_width(self) -> u8 {
        match self {
            Self::Box | Self::Die | Self::Happy | Self::FourByFour => 0,
            Self::Long | Self::Cap | Self::Wall | Self::WeirdLong => 4,
            Self::LongDong => 8,
            Self::El
            | Self::ReverseEl
            | Self::SlantRight
            | Self::SlantLeft
            | Self::Plug
            | Self::Dog
            | Self::ReverseDog
            | Self::Tower
            | Self::Star => 3,
        }
    }

    /// Returns the post-lock spawn anchor for this piece kind.
    #[must_use]
    pub const fn spawn_anchor(self) -> (isize, isize) {
        (SPAWN_X - self.rotation_width() as isize / 2, SPAWN_Y)
    }

    /// Returns all legacy piece kinds in numeric ID order.
    #[must_use]
    pub const fn all() -> [Self; 18] {
        [
            Self::El,
            Self::ReverseEl,
            Self::SlantRight,
            Self::SlantLeft,
            Self::Long,
            Self::Plug,
            Self::Box,
            Self::Die,
            Self::Happy,
            Self::Dog,
            Self::ReverseDog,
            Self::Cap,
            Self::Wall,
            Self::Tower,
            Self::Star,
            Self::WeirdLong,
            Self::FourByFour,
            Self::LongDong,
        ]
    }
}

/// A falling piece before it locks into a board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    kind: PieceKind,
    anchor: (isize, isize),
    orientation: u8,
    cells: Vec<PieceCell>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PieceCell {
    coord: LocalCoord,
    cell: Cell,
}

impl Piece {
    /// Creates a piece at its legacy post-lock spawn anchor.
    #[must_use]
    pub fn spawn(kind: PieceKind) -> Self {
        let (x, y) = kind.spawn_anchor();
        Self::new(kind, x, y)
    }

    /// Creates a die piece at its legacy spawn anchor with an explicit pip.
    #[must_use]
    pub fn spawn_die(pip: Pip) -> Self {
        let (x, y) = PieceKind::Die.spawn_anchor();
        Self::new_with_cell(PieceKind::Die, x, y, Cell::die(pip))
    }

    /// Creates a piece at an explicit board anchor.
    #[must_use]
    pub fn new(kind: PieceKind, x: isize, y: isize) -> Self {
        Self::new_with_cell(kind, x, y, cell_for_piece(kind))
    }

    fn new_with_cell(kind: PieceKind, x: isize, y: isize, cell: Cell) -> Self {
        let cells = initial_coords(kind)
            .iter()
            .map(|&coord| PieceCell { coord, cell })
            .collect();

        Self {
            kind,
            anchor: (x, y),
            orientation: 0,
            cells,
        }
    }

    /// Returns this piece's kind.
    #[must_use]
    pub const fn kind(&self) -> PieceKind {
        self.kind
    }

    /// Returns this piece's board anchor.
    #[must_use]
    pub const fn anchor(&self) -> (isize, isize) {
        self.anchor
    }

    /// Returns this piece's rotation state.
    #[must_use]
    pub const fn orientation(&self) -> u8 {
        self.orientation
    }

    /// Returns occupied local coordinates in deterministic order.
    #[must_use]
    pub fn local_coords(&self) -> Vec<LocalCoord> {
        let mut coords = self.cells.iter().map(|cell| cell.coord).collect::<Vec<_>>();
        coords.sort_unstable();
        coords
    }

    /// Returns occupied board coordinates in deterministic order.
    #[must_use]
    pub fn board_coords(&self) -> Vec<(isize, isize)> {
        let mut coords = self
            .cells
            .iter()
            .map(|cell| {
                (
                    self.anchor.0 + isize::from(cell.coord.x),
                    self.anchor.1 + isize::from(cell.coord.y),
                )
            })
            .collect::<Vec<_>>();
        coords.sort_unstable();
        coords
    }

    /// Returns occupied board cells in deterministic coordinate order.
    #[must_use]
    pub fn cells(&self) -> Vec<((isize, isize), Cell)> {
        let mut cells = self
            .cells
            .iter()
            .map(|cell| {
                (
                    (
                        self.anchor.0 + isize::from(cell.coord.x),
                        self.anchor.1 + isize::from(cell.coord.y),
                    ),
                    cell.cell,
                )
            })
            .collect::<Vec<_>>();
        cells.sort_unstable_by_key(|(coord, _)| *coord);
        cells
    }

    /// Moves the piece anchor without collision checks for spawn transformations.
    pub fn set_anchor(&mut self, x: isize, y: isize) {
        self.anchor = (x, y);
    }

    /// Returns the lowest occupied local y coordinate.
    #[must_use]
    pub fn max_local_y(&self) -> u8 {
        self.cells
            .iter()
            .map(|cell| cell.coord.y)
            .max()
            .unwrap_or(0)
    }

    /// Locks this piece into a board. Callers must ensure it fits first.
    pub fn lock_into(self, board: &mut Board) {
        for ((x, y), cell) in self.cells() {
            let coord = crate::board::Coord::new(x as usize, y as usize)
                .expect("locked piece cells must be in bounds");
            board.set(coord, Some(cell));
        }
    }

    /// Returns true when every occupied cell can move to the destination anchor.
    #[must_use]
    pub fn can_move_to(&self, board: &Board, x: isize, y: isize) -> bool {
        self.can_move_to_with_fallout(board, x, y, false)
    }

    /// Returns true when occupied cells can move, with optional Fallout black-hole bounds.
    #[must_use]
    pub fn can_move_to_with_fallout(
        &self,
        board: &Board,
        x: isize,
        y: isize,
        fallout: bool,
    ) -> bool {
        self.cells.iter().all(|cell| {
            !board.is_occupied_with_fallout(
                x + isize::from(cell.coord.x),
                y + isize::from(cell.coord.y),
                fallout,
            )
        })
    }

    /// Moves the piece to the destination anchor if there is no collision.
    pub fn move_to(&mut self, board: &Board, x: isize, y: isize) -> bool {
        self.move_to_with_fallout(board, x, y, false)
    }

    /// Moves the piece with optional Fallout black-hole bounds.
    pub fn move_to_with_fallout(
        &mut self,
        board: &Board,
        x: isize,
        y: isize,
        fallout: bool,
    ) -> bool {
        if !self.can_move_to_with_fallout(board, x, y, fallout) {
            return false;
        }

        self.anchor = (x, y);
        true
    }

    /// Returns true when this piece can rotate at its current anchor.
    #[must_use]
    pub fn can_rotate(&self, board: &Board, reverse: bool) -> bool {
        self.rotated_cells(reverse).is_some_and(|cells| {
            cells.iter().all(|cell| {
                !board.is_occupied(
                    self.anchor.0 + isize::from(cell.coord.x),
                    self.anchor.1 + isize::from(cell.coord.y),
                )
            })
        })
    }

    /// Rotates this piece in place if there is no collision. No wall kicks apply.
    pub fn rotate(&mut self, board: &Board, reverse: bool) -> bool {
        let Some(cells) = self.rotated_cells(reverse) else {
            return false;
        };
        if cells.iter().any(|cell| {
            board.is_occupied(
                self.anchor.0 + isize::from(cell.coord.x),
                self.anchor.1 + isize::from(cell.coord.y),
            )
        }) {
            return false;
        }

        self.cells = cells;
        self.orientation = rotated_orientation(self.kind, self.orientation, reverse);
        true
    }

    fn rotated_cells(&self, reverse: bool) -> Option<Vec<PieceCell>> {
        let width = self.kind.rotation_width();
        if width == 0 {
            return None;
        }

        if self.kind == PieceKind::Star {
            let next_orientation = (self.orientation + 1) % 2;
            return Some(cells_for_coords(
                custom_orientation(self.kind, next_orientation),
                self.cells[0].cell,
            ));
        }

        if matches!(self.kind, PieceKind::Wall | PieceKind::WeirdLong) {
            let next_orientation = rotated_orientation(self.kind, self.orientation, reverse);
            return Some(cells_for_coords(
                custom_orientation(self.kind, next_orientation),
                self.cells[0].cell,
            ));
        }

        let width = width - 1;
        Some(
            self.cells
                .iter()
                .map(|cell| {
                    let coord = if reverse {
                        LocalCoord::new(width - cell.coord.y, cell.coord.x)
                    } else {
                        LocalCoord::new(cell.coord.y, width - cell.coord.x)
                    };
                    PieceCell {
                        coord,
                        cell: cell.cell,
                    }
                })
                .collect(),
        )
    }
}

fn rotated_orientation(kind: PieceKind, orientation: u8, reverse: bool) -> u8 {
    let orientations = match kind {
        PieceKind::Star => 2,
        PieceKind::WeirdLong => 6,
        _ => 4,
    };

    if reverse {
        (orientation + orientations - 1) % orientations
    } else {
        (orientation + 1) % orientations
    }
}

fn cell_for_piece(kind: PieceKind) -> Cell {
    match kind {
        PieceKind::Die => Cell::die(Pip::new(1).expect("literal pip is valid")),
        PieceKind::Happy => Cell::Happy,
        _ => Cell::visible_with_color(piece_color(kind)),
    }
}

const fn piece_color(kind: PieceKind) -> VisibleColor {
    let color = match kind {
        PieceKind::El | PieceKind::Dog => 2,
        PieceKind::ReverseEl | PieceKind::ReverseDog => 3,
        PieceKind::SlantRight | PieceKind::Cap => 4,
        PieceKind::SlantLeft | PieceKind::Wall => 5,
        PieceKind::Long | PieceKind::Tower | PieceKind::LongDong => 6,
        PieceKind::Plug | PieceKind::Star => 7,
        PieceKind::Box | PieceKind::WeirdLong | PieceKind::FourByFour => 8,
        PieceKind::Die | PieceKind::Happy => 1,
    };
    VisibleColor::new(color).expect("piece color id is in the legacy visible range")
}

fn cells_for_coords(coords: &[LocalCoord], cell: Cell) -> Vec<PieceCell> {
    coords
        .iter()
        .map(|&coord| PieceCell { coord, cell })
        .collect()
}

fn initial_coords(kind: PieceKind) -> &'static [LocalCoord] {
    match kind {
        PieceKind::El => &EL,
        PieceKind::ReverseEl => &REVERSE_EL,
        PieceKind::SlantRight => &SLANT_RIGHT,
        PieceKind::SlantLeft => &SLANT_LEFT,
        PieceKind::Long => &LONG,
        PieceKind::Plug => &PLUG,
        PieceKind::Box => &BOX,
        PieceKind::Die | PieceKind::Happy => &ONE_CELL,
        PieceKind::Dog => &DOG,
        PieceKind::ReverseDog => &REVERSE_DOG,
        PieceKind::Cap => &CAP,
        PieceKind::Wall => custom_orientation(kind, 0),
        PieceKind::Tower => &TOWER,
        PieceKind::Star => custom_orientation(kind, 0),
        PieceKind::WeirdLong => custom_orientation(kind, 0),
        PieceKind::FourByFour => &FOUR_BY_FOUR,
        PieceKind::LongDong => &LONG_DONG,
    }
}

fn custom_orientation(kind: PieceKind, orientation: u8) -> &'static [LocalCoord] {
    match (kind, orientation) {
        (PieceKind::Wall, 0) => &WALL_0,
        (PieceKind::Wall, 1) => &WALL_1,
        (PieceKind::Wall, 2) => &WALL_2,
        (PieceKind::Wall, 3) => &WALL_3,
        (PieceKind::Star, 0) => &STAR_0,
        (PieceKind::Star, 1) => &STAR_1,
        (PieceKind::WeirdLong, 0) => &WEIRD_LONG_0,
        (PieceKind::WeirdLong, 1) => &WEIRD_LONG_1,
        (PieceKind::WeirdLong, 2) => &WEIRD_LONG_2,
        (PieceKind::WeirdLong, 3) => &WEIRD_LONG_3,
        (PieceKind::WeirdLong, 4) => &WEIRD_LONG_4,
        (PieceKind::WeirdLong, 5) => &WEIRD_LONG_5,
        _ => unreachable!("custom orientation requested for non-custom piece"),
    }
}

const fn c(x: u8, y: u8) -> LocalCoord {
    LocalCoord::new(x, y)
}

const EL: [LocalCoord; 4] = [c(1, 0), c(1, 1), c(1, 2), c(2, 2)];
const REVERSE_EL: [LocalCoord; 4] = [c(2, 0), c(2, 1), c(2, 2), c(1, 2)];
const SLANT_RIGHT: [LocalCoord; 4] = [c(0, 2), c(1, 2), c(1, 1), c(2, 1)];
const SLANT_LEFT: [LocalCoord; 4] = [c(0, 1), c(1, 1), c(1, 2), c(2, 2)];
const LONG: [LocalCoord; 4] = [c(0, 1), c(1, 1), c(2, 1), c(3, 1)];
const PLUG: [LocalCoord; 4] = [c(0, 2), c(1, 2), c(1, 1), c(2, 2)];
const BOX: [LocalCoord; 4] = [c(1, 1), c(1, 2), c(2, 1), c(2, 2)];
const ONE_CELL: [LocalCoord; 1] = [c(1, 1)];
const DOG: [LocalCoord; 4] = [c(0, 0), c(1, 1), c(2, 1), c(2, 2)];
const REVERSE_DOG: [LocalCoord; 4] = [c(0, 1), c(0, 2), c(1, 1), c(2, 2)];
const CAP: [LocalCoord; 4] = [c(0, 2), c(1, 1), c(2, 1), c(3, 2)];
const TOWER: [LocalCoord; 4] = [c(2, 0), c(1, 1), c(0, 1), c(2, 2)];
const WALL_0: [LocalCoord; 4] = [c(0, 1), c(0, 2), c(3, 1), c(3, 2)];
const WALL_1: [LocalCoord; 4] = [c(0, 2), c(1, 3), c(2, 0), c(3, 1)];
const WALL_2: [LocalCoord; 4] = [c(1, 0), c(1, 3), c(2, 0), c(2, 3)];
const WALL_3: [LocalCoord; 4] = [c(0, 1), c(1, 0), c(2, 3), c(3, 2)];
const STAR_0: [LocalCoord; 4] = [c(1, 0), c(0, 1), c(1, 2), c(2, 1)];
const STAR_1: [LocalCoord; 4] = [c(0, 0), c(2, 0), c(0, 2), c(2, 2)];
const WEIRD_LONG_0: [LocalCoord; 4] = [c(1, 0), c(1, 1), c(2, 2), c(2, 3)];
const WEIRD_LONG_1: [LocalCoord; 4] = [c(0, 0), c(1, 1), c(2, 2), c(3, 3)];
const WEIRD_LONG_2: [LocalCoord; 4] = [c(0, 1), c(1, 1), c(2, 2), c(3, 2)];
const WEIRD_LONG_3: [LocalCoord; 4] = [c(0, 2), c(1, 2), c(2, 1), c(3, 1)];
const WEIRD_LONG_4: [LocalCoord; 4] = [c(0, 3), c(1, 2), c(2, 1), c(3, 0)];
const WEIRD_LONG_5: [LocalCoord; 4] = [c(1, 2), c(1, 3), c(2, 0), c(2, 1)];
const FOUR_BY_FOUR: [LocalCoord; 12] = [
    c(0, 0),
    c(1, 0),
    c(2, 0),
    c(3, 0),
    c(0, 1),
    c(3, 1),
    c(0, 2),
    c(3, 2),
    c(0, 3),
    c(1, 3),
    c(2, 3),
    c(3, 3),
];
const LONG_DONG: [LocalCoord; 8] = [
    c(0, 0),
    c(1, 0),
    c(2, 0),
    c(3, 0),
    c(4, 0),
    c(5, 0),
    c(6, 0),
    c(7, 0),
];

/// Renders occupied local cells as an 8x8 fixture map.
#[must_use]
pub fn piece_to_text(piece: &Piece) -> String {
    let mut text = String::with_capacity(PIECE_MAP_SIZE * (PIECE_MAP_SIZE + 1));
    for y in 0..PIECE_MAP_SIZE {
        for x in 0..PIECE_MAP_SIZE {
            let occupied = piece
                .cells
                .iter()
                .any(|cell| cell.coord.x == x as u8 && cell.coord.y == y as u8);
            text.push(if occupied { 'X' } else { '.' });
        }
        text.push('\n');
    }
    text
}

/// Returns whether the piece's occupied cells can be inside the board at spawn.
#[must_use]
pub fn spawn_is_horizontally_centered(kind: PieceKind) -> bool {
    let piece = Piece::spawn(kind);
    piece
        .board_coords()
        .iter()
        .all(|&(x, _)| x >= 0 && x < BOARD_WIDTH as isize)
}

#[cfg(test)]
mod tests {
    use super::{piece_to_text, spawn_is_horizontally_centered, Piece, PieceKind};
    use crate::{
        board::{Board, Coord},
        cell::Cell,
    };

    #[test]
    fn legacy_piece_ids_tokens_rotation_widths_and_spawn_anchors_match_sources() {
        let expected = [
            (PieceKind::El, 1, "BT_EL_PIECE", 3, (4, 0)),
            (PieceKind::ReverseEl, 2, "BT_REL_PIECE", 3, (4, 0)),
            (PieceKind::SlantRight, 3, "BT_SL_RT_PIECE", 3, (4, 0)),
            (PieceKind::SlantLeft, 4, "BT_SL_LF_PIECE", 3, (4, 0)),
            (PieceKind::Long, 5, "BT_LONG_PIECE", 4, (3, 0)),
            (PieceKind::Plug, 6, "BT_PLUG_PIECE", 3, (4, 0)),
            (PieceKind::Box, 7, "BT_BOX_PIECE", 0, (5, 0)),
            (PieceKind::Die, 8, "BT_DIE_PIECE", 0, (5, 0)),
            (PieceKind::Happy, 9, "BT_HAP_PIECE", 0, (5, 0)),
            (PieceKind::Dog, 10, "BT_DOG_PIECE", 3, (4, 0)),
            (PieceKind::ReverseDog, 11, "BT_RDOG_PIECE", 3, (4, 0)),
            (PieceKind::Cap, 12, "BT_CAP_PIECE", 4, (3, 0)),
            (PieceKind::Wall, 13, "BT_WALL_PIECE", 4, (3, 0)),
            (PieceKind::Tower, 14, "BT_TOWER_PIECE", 3, (4, 0)),
            (PieceKind::Star, 15, "BT_STAR_PIECE", 3, (4, 0)),
            (PieceKind::WeirdLong, 16, "BT_WLONG_PIECE", 4, (3, 0)),
            (PieceKind::FourByFour, 17, "BT_4x4_PIECE", 0, (5, 0)),
            (PieceKind::LongDong, 18, "BT_LONG_DONG_PIECE", 8, (1, 0)),
        ];

        assert_eq!(PieceKind::all().len(), expected.len());
        for (kind, id, token, rot, spawn) in expected {
            assert_eq!(kind.legacy_id(), id);
            assert_eq!(kind.token(), token);
            assert_eq!(kind.rotation_width(), rot);
            assert_eq!(kind.spawn_anchor(), spawn);
            assert!(spawn_is_horizontally_centered(kind));
        }
    }

    #[test]
    fn visible_piece_color_ids_match_legacy_constructor_offsets() {
        let expected = [
            (PieceKind::El, 2),
            (PieceKind::ReverseEl, 3),
            (PieceKind::SlantRight, 4),
            (PieceKind::SlantLeft, 5),
            (PieceKind::Long, 6),
            (PieceKind::Plug, 7),
            (PieceKind::Box, 8),
            (PieceKind::Dog, 2),
            (PieceKind::ReverseDog, 3),
            (PieceKind::Cap, 4),
            (PieceKind::Wall, 5),
            (PieceKind::Tower, 6),
            (PieceKind::Star, 7),
            (PieceKind::WeirdLong, 8),
            (PieceKind::FourByFour, 8),
            (PieceKind::LongDong, 6),
        ];

        for (kind, id) in expected {
            for (_, cell) in Piece::spawn(kind).cells() {
                assert_eq!(cell.legacy_id(), id, "{kind:?}");
            }
        }
    }

    #[test]
    fn fixture_manifest_covers_every_piece_rotation_state() {
        let manifest = include_str!("../fixtures/piece/legacy-rotation-coverage.btfix");
        let rows = manifest
            .lines()
            .skip_while(|line| line.trim() != "@pieces")
            .skip(1)
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>();

        assert_eq!(rows.len(), PieceKind::COUNT);
        for (row, kind) in rows.iter().zip(PieceKind::all()) {
            let fields = row.split('|').collect::<Vec<_>>();
            assert_eq!(fields.len(), 5, "{row}");
            assert_eq!(fields[0], kind.token());
            assert_eq!(fields[1].parse::<u8>().unwrap(), kind.legacy_id());
            assert_eq!(fields[2].parse::<u8>().unwrap(), kind.rotation_width());
            assert_eq!(
                fields[3],
                format!("{},{}", kind.spawn_anchor().0, kind.spawn_anchor().1)
            );
            assert_eq!(fields[4].parse::<u8>().unwrap(), orientation_count(kind));
        }
    }

    #[test]
    fn initial_shapes_match_legacy_fixtures() {
        let expected = [
            (
                PieceKind::El,
                ".X......\n.X......\n.XX.....\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::ReverseEl,
                "..X.....\n..X.....\n.XX.....\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::SlantRight,
                "........\n.XX.....\nXX......\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::SlantLeft,
                "........\nXX......\n.XX.....\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::Long,
                "........\nXXXX....\n........\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::Plug,
                "........\n.X......\nXXX.....\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::Box,
                "........\n.XX.....\n.XX.....\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::Die,
                "........\n.X......\n........\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::Happy,
                "........\n.X......\n........\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::Dog,
                "X.......\n.XX.....\n..X.....\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::ReverseDog,
                "........\nXX......\nX.X.....\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::Cap,
                "........\n.XX.....\nX..X....\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::Wall,
                "........\nX..X....\nX..X....\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::Tower,
                "..X.....\nXX......\n..X.....\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::Star,
                ".X......\nX.X.....\n.X......\n........\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::WeirdLong,
                ".X......\n.X......\n..X.....\n..X.....\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::FourByFour,
                "XXXX....\nX..X....\nX..X....\nXXXX....\n........\n........\n........\n........\n",
            ),
            (
                PieceKind::LongDong,
                "XXXXXXXX\n........\n........\n........\n........\n........\n........\n........\n",
            ),
        ];

        for (kind, text) in expected {
            assert_eq!(piece_to_text(&Piece::spawn(kind)), text, "{kind:?}");
        }
    }

    #[test]
    fn standard_rotation_and_reverse_rotation_use_legacy_square_mapping() {
        let board = Board::empty();
        let mut piece = Piece::spawn(PieceKind::El);

        assert!(piece.rotate(&board, false));
        assert_eq!(piece.orientation(), 1);
        assert_eq!(
            piece_to_text(&piece),
            "..X.....\nXXX.....\n........\n........\n........\n........\n........\n........\n"
        );

        assert!(piece.rotate(&board, true));
        assert_eq!(piece.orientation(), 0);
        assert_eq!(
            piece_to_text(&piece),
            ".X......\n.X......\n.XX.....\n........\n........\n........\n........\n........\n"
        );
    }

    #[test]
    fn custom_rotation_cycles_match_legacy_state_machines() {
        let board = Board::empty();
        let cases = [
            (
                PieceKind::Wall,
                vec![
                    "........\nX..X....\nX..X....\n........\n........\n........\n........\n........\n",
                    "..X.....\n...X....\nX.......\n.X......\n........\n........\n........\n........\n",
                    ".XX.....\n........\n........\n.XX.....\n........\n........\n........\n........\n",
                    ".X......\nX.......\n...X....\n..X.....\n........\n........\n........\n........\n",
                ],
            ),
            (
                PieceKind::Star,
                vec![
                    ".X......\nX.X.....\n.X......\n........\n........\n........\n........\n........\n",
                    "X.X.....\n........\nX.X.....\n........\n........\n........\n........\n........\n",
                ],
            ),
            (
                PieceKind::WeirdLong,
                vec![
                    ".X......\n.X......\n..X.....\n..X.....\n........\n........\n........\n........\n",
                    "X.......\n.X......\n..X.....\n...X....\n........\n........\n........\n........\n",
                    "........\nXX......\n..XX....\n........\n........\n........\n........\n........\n",
                    "........\n..XX....\nXX......\n........\n........\n........\n........\n........\n",
                    "...X....\n..X.....\n.X......\nX.......\n........\n........\n........\n........\n",
                    "..X.....\n..X.....\n.X......\n.X......\n........\n........\n........\n........\n",
                ],
            ),
        ];

        for (kind, states) in cases {
            let mut piece = Piece::spawn(kind);
            for expected in &states {
                assert_eq!(piece_to_text(&piece), *expected, "{kind:?}");
                assert!(piece.rotate(&board, false));
            }
            assert_eq!(piece.orientation(), 0);
            assert_eq!(piece_to_text(&piece), states[0]);
        }
    }

    #[test]
    fn star_rotation_ignores_reverse_argument() {
        let board = Board::empty();
        let mut piece = Piece::spawn(PieceKind::Star);

        assert!(piece.rotate(&board, true));

        assert_eq!(piece.orientation(), 1);
        assert_eq!(
            piece_to_text(&piece),
            "X.X.....\n........\nX.X.....\n........\n........\n........\n........\n........\n"
        );
    }

    const fn orientation_count(kind: PieceKind) -> u8 {
        match kind {
            PieceKind::Box | PieceKind::Die | PieceKind::Happy | PieceKind::FourByFour => 1,
            PieceKind::Star => 2,
            PieceKind::WeirdLong => 6,
            _ => 4,
        }
    }

    #[test]
    fn non_rotating_pieces_reject_rotation() {
        let board = Board::empty();
        for kind in [
            PieceKind::Box,
            PieceKind::Die,
            PieceKind::Happy,
            PieceKind::FourByFour,
        ] {
            let mut piece = Piece::spawn(kind);
            assert!(!piece.can_rotate(&board, false));
            assert!(!piece.rotate(&board, false));
            assert_eq!(piece.orientation(), 0);
        }
    }

    #[test]
    fn no_wall_kick_rotation_aborts_unchanged_on_collision() {
        let mut board = Board::empty();
        board.set(Coord::new(3, 1).unwrap(), Some(Cell::visible()));
        let mut piece = Piece::new(PieceKind::El, 2, 0);
        let before = piece.clone();

        assert!(!piece.rotate(&board, false));

        assert_eq!(piece, before);
    }

    #[test]
    fn movement_and_rotation_collide_only_on_mapped_cells() {
        let board = Board::empty();
        let long = Piece::new(PieceKind::Long, 0, 0);

        assert!(long.can_move_to(&board, 0, -1));
        assert!(!long.can_move_to(&board, 0, -2));

        let mut long_dong = Piece::spawn(PieceKind::LongDong);
        assert!(long_dong.rotate(&board, false));
        assert_eq!(
            long_dong.board_coords(),
            vec![
                (1, 0),
                (1, 1),
                (1, 2),
                (1, 3),
                (1, 4),
                (1, 5),
                (1, 6),
                (1, 7)
            ]
        );
    }
}
