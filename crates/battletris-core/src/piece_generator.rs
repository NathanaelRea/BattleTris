//! Deterministic piece generation, dice pips, and probability hooks.

use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::{
    cell::Pip,
    piece::{Piece, PieceKind},
    rng::{GameSeed, RngStream},
};

/// Integer weight scale for legacy `.01` probability slots.
pub const PROBABILITY_SCALE: u16 = 100;

/// Mutable piece probability table used by weapon hooks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceProbabilities {
    weights: [u16; PieceKind::COUNT],
}

impl Default for PieceProbabilities {
    fn default() -> Self {
        Self::legacy_default()
    }
}

impl PieceProbabilities {
    /// Returns the default legacy keep probabilities.
    #[must_use]
    pub const fn legacy_default() -> Self {
        let mut weights = [0; PieceKind::COUNT];
        weights[PieceKind::El.index()] = 21;
        weights[PieceKind::ReverseEl.index()] = 21;
        weights[PieceKind::SlantRight.index()] = 21;
        weights[PieceKind::SlantLeft.index()] = 21;
        weights[PieceKind::Long.index()] = 21;
        weights[PieceKind::Plug.index()] = 21;
        weights[PieceKind::Box.index()] = 21;
        weights[PieceKind::Die.index()] = PROBABILITY_SCALE;
        weights[PieceKind::Happy.index()] = 2;
        weights[PieceKind::LongDong.index()] = 2;
        Self { weights }
    }

    /// Returns the current weight for a piece kind.
    #[must_use]
    pub const fn weight(&self, kind: PieceKind) -> u16 {
        self.weights[kind.index()]
    }

    /// Mutates one probability slot, matching legacy weapon hook behavior.
    pub fn set_weight(&mut self, kind: PieceKind, weight: u16) {
        self.weights[kind.index()] = weight;
    }

    /// Enables or disables weird pieces as The Feared Weird does.
    pub fn set_weird_pieces_enabled(&mut self, enabled: bool) {
        let weight = if enabled { 21 } else { 0 };
        for kind in [
            PieceKind::Dog,
            PieceKind::ReverseDog,
            PieceKind::Cap,
            PieceKind::Wall,
            PieceKind::Tower,
            PieceKind::Star,
            PieceKind::WeirdLong,
        ] {
            self.set_weight(kind, weight);
        }
    }

    /// Enables or disables Four-by-Four replacement probability.
    pub fn set_four_by_four_enabled(&mut self, enabled: bool) {
        self.set_weight(PieceKind::Box, if enabled { 0 } else { 21 });
        self.set_weight(PieceKind::FourByFour, if enabled { 21 } else { 0 });
    }

    /// Enables or disables long pieces as So Long does.
    pub fn set_long_pieces_enabled(&mut self, enabled: bool) {
        self.set_weight(PieceKind::Long, if enabled { 21 } else { 0 });
    }

    /// Enables or disables dice as No Dice does.
    pub fn set_dice_enabled(&mut self, enabled: bool) {
        self.set_weight(PieceKind::Die, if enabled { PROBABILITY_SCALE } else { 0 });
    }

    fn total_weight(&self) -> u32 {
        self.weights.iter().map(|weight| u32::from(*weight)).sum()
    }
}

/// Deterministic falling-piece generator.
#[derive(Debug, Clone)]
pub struct PieceGenerator {
    probabilities: PieceProbabilities,
    piece_rng: ChaCha12Rng,
    dice_rng: ChaCha12Rng,
    happy_rng: ChaCha12Rng,
    queued_happy: u32,
    old_piece: Option<PieceKind>,
    broken_record_enabled: bool,
}

impl PieceGenerator {
    /// Creates a generator from an explicit game seed.
    #[must_use]
    pub fn new(seed: GameSeed) -> Self {
        Self {
            probabilities: PieceProbabilities::legacy_default(),
            piece_rng: seed.stream(RngStream::PieceSelection),
            dice_rng: seed.stream(RngStream::DicePips),
            happy_rng: seed.stream(RngStream::HappyQueue),
            queued_happy: 0,
            old_piece: None,
            broken_record_enabled: false,
        }
    }

    /// Returns immutable access to the mutable legacy probability table.
    #[must_use]
    pub const fn probabilities(&self) -> &PieceProbabilities {
        &self.probabilities
    }

    /// Returns mutable access to the legacy probability table.
    pub fn probabilities_mut(&mut self) -> &mut PieceProbabilities {
        &mut self.probabilities
    }

    /// Queues happy pieces, as Have a Nice Day does.
    pub fn queue_happy(&mut self, count: u32) {
        self.queued_happy = self.queued_happy.saturating_add(count);
        // Keep the happy stream observable and isolated for fixtures/future hooks.
        let _ = self.happy_rng.next_u64();
    }

    /// Enables or disables Broken Record repeat behavior.
    pub fn set_broken_record_enabled(&mut self, enabled: bool) {
        self.broken_record_enabled = enabled;
    }

    /// Creates the next piece.
    pub fn next_piece(&mut self) -> Piece {
        let kind = self.next_kind();
        self.old_piece = Some(kind);
        self.create_piece(kind)
    }

    /// Returns the next piece kind without constructing cells.
    pub fn next_kind(&mut self) -> PieceKind {
        if self.queued_happy > 0 {
            self.queued_happy -= 1;
            return PieceKind::Happy;
        }

        if self.broken_record_enabled
            && self.old_piece.is_some()
            && self.roll_below(PieceKind::COUNT as u64 * 10, 9)
        {
            return self.old_piece.expect("checked above");
        }

        self.weighted_kind()
    }

    fn create_piece(&mut self, kind: PieceKind) -> Piece {
        if kind == PieceKind::Die {
            let pip = Pip::new(self.dice_uniform_inclusive(1, 6) as u8).expect("range is valid");
            Piece::spawn_die(pip)
        } else {
            Piece::spawn(kind)
        }
    }

    fn weighted_kind(&mut self) -> PieceKind {
        let total = self.probabilities.total_weight();
        assert!(
            total > 0,
            "piece probabilities must contain at least one nonzero weight"
        );

        let mut draw = self.uniform_below(u64::from(total)) as u32;
        for kind in PieceKind::all() {
            let weight = u32::from(self.probabilities.weight(kind));
            if draw < weight {
                return kind;
            }
            draw -= weight;
        }

        unreachable!("draw is bounded by total weight");
    }

    fn roll_below(&mut self, denominator: u64, numerator: u64) -> bool {
        self.uniform_below(denominator) < numerator
    }

    fn dice_uniform_inclusive(&mut self, min: u64, max: u64) -> u64 {
        min + self.dice_uniform_below(max - min + 1)
    }

    fn uniform_below(&mut self, upper: u64) -> u64 {
        self.piece_rng.next_u64() % upper
    }

    fn dice_uniform_below(&mut self, upper: u64) -> u64 {
        self.dice_rng.next_u64() % upper
    }
}

#[cfg(test)]
mod tests {
    use super::PieceGenerator;
    use crate::{
        cell::{Cell, Pip},
        piece::PieceKind,
        rng::GameSeed,
    };

    #[test]
    fn default_probabilities_match_legacy_keep_slots() {
        let generator = PieceGenerator::new(GameSeed::from_u64(1));
        let probabilities = generator.probabilities();

        for kind in [
            PieceKind::El,
            PieceKind::ReverseEl,
            PieceKind::SlantRight,
            PieceKind::SlantLeft,
            PieceKind::Long,
            PieceKind::Plug,
            PieceKind::Box,
        ] {
            assert_eq!(probabilities.weight(kind), 21);
        }
        assert_eq!(probabilities.weight(PieceKind::Die), 100);
        assert_eq!(probabilities.weight(PieceKind::Happy), 2);
        assert_eq!(probabilities.weight(PieceKind::LongDong), 2);
        assert_eq!(probabilities.weight(PieceKind::Dog), 0);
    }

    #[test]
    fn seeded_piece_sequences_are_stable() {
        let mut generator = PieceGenerator::new(GameSeed::from_u64(42));

        let sequence = (0..12)
            .map(|_| generator.next_piece().kind())
            .collect::<Vec<_>>();

        assert_eq!(
            sequence,
            vec![
                PieceKind::Plug,
                PieceKind::SlantLeft,
                PieceKind::Box,
                PieceKind::Die,
                PieceKind::El,
                PieceKind::Die,
                PieceKind::SlantLeft,
                PieceKind::Die,
                PieceKind::SlantRight,
                PieceKind::SlantLeft,
                PieceKind::Long,
                PieceKind::Box,
            ]
        );
    }

    #[test]
    fn queued_happy_has_priority_over_broken_record() {
        let mut generator = PieceGenerator::new(GameSeed::from_u64(9));
        generator.set_broken_record_enabled(true);
        assert_ne!(generator.next_kind(), PieceKind::Happy);

        generator.queue_happy(2);

        assert_eq!(generator.next_kind(), PieceKind::Happy);
        assert_eq!(generator.next_kind(), PieceKind::Happy);
    }

    #[test]
    fn repeated_die_pieces_reroll_pips() {
        let mut generator = PieceGenerator::new(GameSeed::from_u64(5));
        for kind in PieceKind::all() {
            generator.probabilities_mut().set_weight(kind, 0);
        }
        generator.probabilities_mut().set_weight(PieceKind::Die, 1);

        let pips = (0..6)
            .map(|_| match generator.next_piece().cells()[0].1 {
                Cell::Die { pip } => pip,
                cell => panic!("expected die cell, got {cell:?}"),
            })
            .collect::<Vec<_>>();

        assert_eq!(
            pips,
            vec![
                Pip::new(2).unwrap(),
                Pip::new(3).unwrap(),
                Pip::new(1).unwrap(),
                Pip::new(1).unwrap(),
                Pip::new(2).unwrap(),
                Pip::new(5).unwrap(),
            ]
        );
    }

    #[test]
    fn probability_hooks_mutate_slots_directly() {
        let mut generator = PieceGenerator::new(GameSeed::from_u64(1));
        let probabilities = generator.probabilities_mut();

        probabilities.set_weird_pieces_enabled(true);
        probabilities.set_four_by_four_enabled(true);
        probabilities.set_long_pieces_enabled(false);
        probabilities.set_dice_enabled(false);

        assert_eq!(probabilities.weight(PieceKind::Dog), 21);
        assert_eq!(probabilities.weight(PieceKind::Box), 0);
        assert_eq!(probabilities.weight(PieceKind::FourByFour), 21);
        assert_eq!(probabilities.weight(PieceKind::Long), 0);
        assert_eq!(probabilities.weight(PieceKind::Die), 0);
    }
}
