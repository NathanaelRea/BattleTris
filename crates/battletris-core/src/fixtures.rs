//! Test fixture helpers for deterministic core scenarios.
//!
//! Board fixtures use the compact text shape documented in
//! `docs/core-fixtures.md`: TOML-like front matter delimited by `+++` followed
//! by named text sections such as `@board`.

use crate::{
    board::{Board, BoardError, Coord, BOARD_HEIGHT, BOARD_WIDTH},
    cell::{Cell, Pip},
};

/// A named text fixture used by deterministic core tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextFixture<'a> {
    /// Human-readable fixture name used in test diagnostics.
    pub name: &'a str,
    /// Fixture contents in the compact text format owned by the calling module.
    pub contents: &'a str,
}

impl<'a> TextFixture<'a> {
    /// Creates a fixture wrapper without interpreting its contents.
    #[must_use]
    pub const fn new(name: &'a str, contents: &'a str) -> Self {
        Self { name, contents }
    }
}

/// Errors returned when parsing compact core fixtures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixtureError {
    /// The fixture did not contain delimited front matter.
    MissingFrontMatter,
    /// Required metadata was absent.
    MissingMetadata(&'static str),
    /// Metadata did not have the expected value or shape.
    InvalidMetadata {
        /// Metadata key that failed validation.
        key: &'static str,
        /// Parsed metadata value.
        value: String,
    },
    /// The fixture did not contain an expected text section.
    MissingSection(&'static str),
    /// A board row used a glyph not supported by the board fixture parser.
    UnknownBoardGlyph {
        /// Unknown glyph from the board text section.
        glyph: char,
        /// Horizontal board coordinate of the glyph.
        x: usize,
        /// Vertical board coordinate of the glyph.
        y: usize,
    },
    /// The board text dimensions did not match the metadata.
    InvalidBoardShape {
        /// Width declared by fixture metadata.
        expected_width: usize,
        /// Height declared by fixture metadata.
        expected_height: usize,
        /// Width found in the offending text row.
        actual_width: usize,
        /// Number of board rows found in the text section.
        actual_height: usize,
    },
    /// The parsed board could not be constructed.
    Board(BoardError),
}

impl From<BoardError> for FixtureError {
    fn from(value: BoardError) -> Self {
        Self::Board(value)
    }
}

/// Parses a compact board fixture into a typed board.
pub fn parse_board_fixture(fixture: TextFixture<'_>) -> Result<Board, FixtureError> {
    let (metadata, body) = split_front_matter(fixture.contents)?;
    let kind = metadata_value(metadata, "kind").ok_or(FixtureError::MissingMetadata("kind"))?;
    if kind != "board" {
        return Err(FixtureError::InvalidMetadata {
            key: "kind",
            value: kind.to_owned(),
        });
    }

    let width = parse_usize_metadata(metadata, "width")?;
    let height = parse_usize_metadata(metadata, "height")?;
    if width != BOARD_WIDTH || height != BOARD_HEIGHT {
        return Err(FixtureError::InvalidMetadata {
            key: "dimensions",
            value: format!("{width}x{height}"),
        });
    }

    let rows = section_lines(body, "@board")?;
    if rows.len() != height {
        return Err(FixtureError::InvalidBoardShape {
            expected_width: width,
            expected_height: height,
            actual_width: rows.first().map_or(0, |row| row.chars().count()),
            actual_height: rows.len(),
        });
    }

    let mut board = Board::empty();
    for (y, row) in rows.iter().enumerate() {
        let actual_width = row.chars().count();
        if actual_width != width {
            return Err(FixtureError::InvalidBoardShape {
                expected_width: width,
                expected_height: height,
                actual_width,
                actual_height: rows.len(),
            });
        }

        for (x, glyph) in row.chars().enumerate() {
            let cell =
                cell_from_glyph(glyph).ok_or(FixtureError::UnknownBoardGlyph { glyph, x, y })?;
            board.set(
                Coord::new(x, y).expect("fixture dimensions were validated"),
                cell,
            );
        }
    }

    Ok(board)
}

/// Renders a board using the compact board fixture glyphs.
#[must_use]
pub fn board_to_text(board: &Board) -> String {
    let mut text = String::with_capacity(BOARD_HEIGHT * (BOARD_WIDTH + 1));
    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            let coord = Coord::new(x, y).expect("loop stays in bounds");
            text.push(glyph_from_cell(board.get(coord)));
        }
        text.push('\n');
    }
    text
}

fn split_front_matter(contents: &str) -> Result<(&str, &str), FixtureError> {
    let contents = contents
        .strip_prefix("+++\r\n")
        .or_else(|| contents.strip_prefix("+++\n"))
        .ok_or(FixtureError::MissingFrontMatter)?;

    let (delimiter_start, delimiter_len) = contents
        .find("\r\n+++\r\n")
        .map(|index| (index, "\r\n+++\r\n".len()))
        .or_else(|| {
            contents
                .find("\n+++\n")
                .map(|index| (index, "\n+++\n".len()))
        })
        .ok_or(FixtureError::MissingFrontMatter)?;

    let metadata = &contents[..delimiter_start];
    let body = &contents[delimiter_start + delimiter_len..];
    Ok((metadata, body))
}

fn metadata_value<'a>(metadata: &'a str, key: &str) -> Option<&'a str> {
    metadata.lines().find_map(|line| {
        let (line_key, value) = line.split_once('=')?;
        if line_key.trim() == key {
            Some(value.trim().trim_matches('"'))
        } else {
            None
        }
    })
}

fn parse_usize_metadata(metadata: &str, key: &'static str) -> Result<usize, FixtureError> {
    let value = metadata_value(metadata, key).ok_or(FixtureError::MissingMetadata(key))?;
    value.parse().map_err(|_| FixtureError::InvalidMetadata {
        key,
        value: value.to_owned(),
    })
}

fn section_lines<'a>(body: &'a str, name: &'static str) -> Result<Vec<&'a str>, FixtureError> {
    let mut lines = body.lines();
    for line in lines.by_ref() {
        if line.trim() == name {
            let mut section = Vec::new();
            for section_line in lines {
                if section_line.starts_with('@') {
                    break;
                }
                if !section_line.trim().is_empty() {
                    section.push(section_line);
                }
            }
            return Ok(section);
        }
    }

    Err(FixtureError::MissingSection(name))
}

fn cell_from_glyph(glyph: char) -> Option<Option<Cell>> {
    match glyph {
        '.' => Some(None),
        'X' => Some(Some(Cell::visible())),
        'S' => Some(Some(Cell::Structure)),
        '1'..='6' => Some(Some(Cell::die(Pip::new(glyph as u8 - b'0')?))),
        'H' => Some(Some(Cell::Happy)),
        'F' => Some(Some(Cell::Frown)),
        'G' => Some(Some(Cell::Gimp { value: 0 })),
        'I' => Some(Some(Cell::Invisible)),
        'T' => Some(Some(Cell::Hidden {
            value: 0,
            removable: true,
        })),
        _ => None,
    }
}

fn glyph_from_cell(cell: Option<Cell>) -> char {
    match cell {
        None => '.',
        Some(Cell::Visible { .. }) => 'X',
        Some(Cell::Structure) => 'S',
        Some(Cell::Happy) => 'H',
        Some(Cell::Frown) => 'F',
        Some(Cell::Gimp { .. }) => 'G',
        Some(Cell::Die { pip }) => char::from(b'0' + pip.get()),
        Some(Cell::Invisible) => 'I',
        Some(Cell::Hidden { .. }) => 'T',
    }
}

#[cfg(test)]
mod tests {
    use super::{board_to_text, parse_board_fixture, FixtureError, TextFixture};
    use crate::{
        board::{Coord, BOARD_HEIGHT, BOARD_WIDTH},
        cell::{Cell, Pip},
    };

    #[test]
    fn fixture_wrapper_preserves_name_and_contents() {
        let fixture = TextFixture::new("empty-board", "..........\n");

        assert_eq!(fixture.name, "empty-board");
        assert_eq!(fixture.contents, "..........\n");
    }

    #[test]
    fn board_fixture_parses_legacy_sized_board() {
        let fixture = TextFixture::new(
            "mixed-cells",
            include_str!("../fixtures/board/mixed-cells.btfix"),
        );

        let board = parse_board_fixture(fixture).expect("fixture should parse");

        assert_eq!(board.width(), BOARD_WIDTH);
        assert_eq!(board.height(), BOARD_HEIGHT);
        assert_eq!(board.get(Coord::new(0, 0).unwrap()), Some(Cell::visible()));
        assert_eq!(board.get(Coord::new(1, 0).unwrap()), Some(Cell::Structure));
        assert_eq!(
            board.get(Coord::new(2, 0).unwrap()),
            Some(Cell::die(Pip::new(1).unwrap()))
        );
        assert_eq!(
            board.get(Coord::new(7, 0).unwrap()),
            Some(Cell::die(Pip::new(6).unwrap()))
        );
        assert_eq!(board.get(Coord::new(8, 0).unwrap()), Some(Cell::Happy));
        assert_eq!(board.get(Coord::new(9, 0).unwrap()), Some(Cell::Frown));
        assert_eq!(
            board.get(Coord::new(0, 1).unwrap()),
            Some(Cell::Gimp { value: 0 })
        );
        assert_eq!(board.get(Coord::new(1, 1).unwrap()), Some(Cell::Invisible));
        assert_eq!(
            board.get(Coord::new(2, 1).unwrap()),
            Some(Cell::Hidden {
                value: 0,
                removable: true,
            })
        );
    }

    #[test]
    fn board_fixture_text_round_trips_glyphs() {
        let fixture = TextFixture::new(
            "mixed-cells",
            include_str!("../fixtures/board/mixed-cells.btfix"),
        );
        let board = parse_board_fixture(fixture).expect("fixture should parse");
        let text = board_to_text(&board);
        let mut rows = text.lines();

        assert_eq!(rows.next(), Some("XS123456HF"));
        assert_eq!(rows.next(), Some("GIT......."));
        assert_eq!(text.lines().count(), BOARD_HEIGHT);
    }

    #[test]
    fn board_fixture_accepts_crlf_line_endings() {
        let contents = include_str!("../fixtures/board/mixed-cells.btfix")
            .replace("\r\n", "\n")
            .replace('\n', "\r\n");
        let fixture = TextFixture::new("mixed-cells-crlf", &contents);

        let board = parse_board_fixture(fixture).expect("CRLF fixture should parse");

        assert_eq!(board.width(), BOARD_WIDTH);
        assert_eq!(board.height(), BOARD_HEIGHT);
        assert_eq!(board.get(Coord::new(0, 0).unwrap()), Some(Cell::visible()));
    }

    #[test]
    fn board_fixture_rejects_invalid_dimensions() {
        let fixture = TextFixture::new(
            "invalid-dimensions",
            include_str!("../fixtures/board/invalid-dimensions.btfix"),
        );

        let error = parse_board_fixture(fixture).expect_err("dimensions should be rejected");

        assert_eq!(
            error,
            FixtureError::InvalidMetadata {
                key: "dimensions",
                value: "9x28".to_owned(),
            }
        );
    }
}
