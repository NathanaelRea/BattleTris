//! Deterministic RNG seeds and named streams for replayable core logic.
//!
//! Core code must construct RNGs from explicit seeds only. Named streams keep
//! piece choice, dice pips, happy queues, and future weapon randomness isolated
//! so adding one random draw does not perturb unrelated simulations.

use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;

/// Stable 32-byte game seed used by replays, fixtures, and protocol starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameSeed(pub [u8; 32]);

impl GameSeed {
    /// Returns a seed from a little-endian integer for compact tests and tools.
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        let bytes = value.to_le_bytes();
        let mut seed = [0; 32];
        seed[0] = bytes[0];
        seed[1] = bytes[1];
        seed[2] = bytes[2];
        seed[3] = bytes[3];
        seed[4] = bytes[4];
        seed[5] = bytes[5];
        seed[6] = bytes[6];
        seed[7] = bytes[7];
        Self(seed)
    }

    /// Builds a deterministic RNG for a named stream.
    #[must_use]
    pub fn stream(self, stream: RngStream) -> ChaCha12Rng {
        ChaCha12Rng::from_seed(self.stream_seed(stream.name()))
    }

    fn stream_seed(self, name: &[u8]) -> [u8; 32] {
        let mut seed = self.0;
        let hash = fnv1a64(name).to_le_bytes();
        for (index, byte) in seed.iter_mut().enumerate() {
            *byte ^= hash[index % hash.len()].wrapping_add(index as u8);
        }
        seed
    }
}

/// Named deterministic RNG streams owned by core logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RngStream {
    /// Weighted falling-piece selection.
    PieceSelection,
    /// Die pip selection.
    DicePips,
    /// Happy-piece queue effects.
    HappyQueue,
    /// Future weapon random effects.
    WeaponEffects,
    /// Computer opponent shopping and tie-breaking.
    ComputerOpponent,
}

impl RngStream {
    const fn name(self) -> &'static [u8] {
        match self {
            Self::PieceSelection => b"piece-selection",
            Self::DicePips => b"dice-pips",
            Self::HappyQueue => b"happy-queue",
            Self::WeaponEffects => b"weapon-effects",
            Self::ComputerOpponent => b"computer-opponent",
        }
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{GameSeed, RngStream};
    use rand::Rng;

    #[test]
    fn named_rng_streams_are_stable_and_independent() {
        let seed = GameSeed::from_u64(7);
        let piece_a = seed.stream(RngStream::PieceSelection).next_u64();
        let piece_b = seed.stream(RngStream::PieceSelection).next_u64();
        let dice = seed.stream(RngStream::DicePips).next_u64();

        assert_eq!(piece_a, piece_b);
        assert_ne!(piece_a, dice);
    }
}
