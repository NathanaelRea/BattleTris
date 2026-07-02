//! Board cell identity, values, removability, and legacy ID mappings.
//!
//! Cells are typed in the core model. Legacy numeric IDs are compatibility views
//! used by board snapshots and protocol adapters.

/// Funds value awarded by a happy cell before it becomes a frown.
pub const HAPPY_VALUE: i32 = 150;

/// A one-cell die pip value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pip(u8);

impl Pip {
    /// Creates a pip value in the inclusive legacy range `1..=6`.
    #[must_use]
    pub const fn new(value: u8) -> Option<Self> {
        if value >= 1 && value <= 6 {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Returns the numeric pip value.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// A legacy visible-cell color ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleColor(u8);

impl VisibleColor {
    /// Creates a legacy visible color ID in the range used for normal boxes.
    #[must_use]
    pub const fn new(value: u8) -> Option<Self> {
        if value >= 1 && value <= 19 {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Returns the numeric legacy color ID.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// The typed state of an occupied board cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// A normal visible, removable cell with no funds value.
    Visible {
        /// Legacy color ID reported by compatibility snapshots.
        color: VisibleColor,
    },
    /// A non-removable structure cell, used by later weapons such as Bottle neck.
    Structure,
    /// A happy cell worth funds until missed.
    Happy,
    /// A missed happy cell with no funds value.
    Frown,
    /// A removable gimp cell that preserves the replaced cell's funds value.
    Gimp {
        /// Funds value preserved from the cell replaced by The Gimp.
        value: i32,
    },
    /// A die cell whose pip value contributes funds when cleared.
    Die {
        /// Die pip value in the legacy range `1..=6`.
        pip: Pip,
    },
    /// An occupied Bug Report cell that legacy snapshots serialize as empty.
    Invisible,
    /// A Twilight-hidden cell whose legacy ID is `-1`.
    Hidden {
        /// Funds value preserved from the hidden cell.
        value: i32,
        /// Whether the hidden cell can be removed by line and weapon effects.
        removable: bool,
    },
}

impl Cell {
    /// Creates the default visible fixture cell.
    #[must_use]
    pub const fn visible() -> Self {
        Self::Visible {
            color: VisibleColor(1),
        }
    }

    /// Creates a die cell for a valid pip value.
    #[must_use]
    pub const fn die(pip: Pip) -> Self {
        Self::Die { pip }
    }

    /// Returns whether line and weapon effects may remove this occupied cell.
    #[must_use]
    pub const fn is_removable(self) -> bool {
        match self {
            Self::Structure => false,
            Self::Hidden { removable, .. } => removable,
            Self::Visible { .. }
            | Self::Happy
            | Self::Frown
            | Self::Gimp { .. }
            | Self::Die { .. }
            | Self::Invisible => true,
        }
    }

    /// Returns the funds value carried by this cell.
    #[must_use]
    pub const fn value(self) -> i32 {
        match self {
            Self::Happy => HAPPY_VALUE,
            Self::Gimp { value } | Self::Hidden { value, .. } => value,
            Self::Die { pip } => pip.get() as i32,
            Self::Visible { .. } | Self::Structure | Self::Frown | Self::Invisible => 0,
        }
    }

    /// Returns the legacy `BTBox::id()` compatibility value for this cell.
    #[must_use]
    pub const fn legacy_id(self) -> i16 {
        match self {
            Self::Visible { color } => color.get() as i16,
            Self::Structure => 20,
            Self::Happy => 21,
            Self::Frown => 22,
            Self::Gimp { .. } => 23,
            Self::Die { pip } => 23 + pip.get() as i16,
            Self::Invisible => 0,
            Self::Hidden { .. } => -1,
        }
    }

    /// Recreates a cell from a nonzero legacy snapshot ID.
    #[must_use]
    pub const fn from_legacy_id(id: i16) -> Option<Self> {
        match id {
            1..=19 => Some(Self::Visible {
                color: VisibleColor(id as u8),
            }),
            20 => Some(Self::Structure),
            21 => Some(Self::Happy),
            22 => Some(Self::Frown),
            23 => Some(Self::Gimp { value: 0 }),
            24..=29 => Some(Self::Die {
                pip: Pip((id - 23) as u8),
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Cell, Pip, HAPPY_VALUE};

    #[test]
    fn pip_accepts_only_legacy_range() {
        assert!(Pip::new(0).is_none());
        assert_eq!(Pip::new(1).expect("valid pip").get(), 1);
        assert_eq!(Pip::new(6).expect("valid pip").get(), 6);
        assert!(Pip::new(7).is_none());
    }

    #[test]
    fn cell_values_match_legacy_funds_behavior() {
        assert_eq!(Cell::visible().value(), 0);
        assert_eq!(Cell::Structure.value(), 0);
        assert_eq!(Cell::Happy.value(), HAPPY_VALUE);
        assert_eq!(Cell::Frown.value(), 0);
        assert_eq!(Cell::Gimp { value: 6 }.value(), 6);
        assert_eq!(Cell::die(Pip::new(4).expect("valid pip")).value(), 4);
        assert_eq!(Cell::Invisible.value(), 0);
        assert_eq!(
            Cell::Hidden {
                value: 5,
                removable: true
            }
            .value(),
            5
        );
    }

    #[test]
    fn removability_preserves_structure_exception() {
        assert!(Cell::visible().is_removable());
        assert!(!Cell::Structure.is_removable());
        assert!(Cell::Gimp { value: 0 }.is_removable());
        assert!(Cell::Hidden {
            value: 0,
            removable: true
        }
        .is_removable());
        assert!(!Cell::Hidden {
            value: 0,
            removable: false
        }
        .is_removable());
    }

    #[test]
    fn legacy_ids_cover_typed_cell_view() {
        assert_eq!(Cell::visible().legacy_id(), 1);
        assert_eq!(Cell::Structure.legacy_id(), 20);
        assert_eq!(Cell::Happy.legacy_id(), 21);
        assert_eq!(Cell::Frown.legacy_id(), 22);
        assert_eq!(Cell::Gimp { value: 6 }.legacy_id(), 23);
        assert_eq!(Cell::die(Pip::new(1).expect("valid pip")).legacy_id(), 24);
        assert_eq!(Cell::die(Pip::new(6).expect("valid pip")).legacy_id(), 29);
        assert_eq!(Cell::Invisible.legacy_id(), 0);
        assert_eq!(
            Cell::Hidden {
                value: 0,
                removable: true
            }
            .legacy_id(),
            -1
        );
    }

    #[test]
    fn legacy_id_recreation_is_lossy_where_legacy_is_lossy() {
        assert_eq!(Cell::from_legacy_id(0), None);
        assert_eq!(Cell::from_legacy_id(1), Some(Cell::visible()));
        assert_eq!(Cell::from_legacy_id(20), Some(Cell::Structure));
        assert_eq!(Cell::from_legacy_id(21), Some(Cell::Happy));
        assert_eq!(Cell::from_legacy_id(22), Some(Cell::Frown));
        assert_eq!(Cell::from_legacy_id(23), Some(Cell::Gimp { value: 0 }));
        assert_eq!(
            Cell::from_legacy_id(24),
            Some(Cell::die(Pip::new(1).unwrap()))
        );
        assert_eq!(
            Cell::from_legacy_id(29),
            Some(Cell::die(Pip::new(6).unwrap()))
        );
        assert_eq!(Cell::from_legacy_id(-1), None);
        assert_eq!(Cell::from_legacy_id(30), None);
    }
}
