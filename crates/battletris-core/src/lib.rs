//! Deterministic BattleTris rules and state.
//!
//! This crate owns gameplay behavior that must be replayable in tests, network
//! simulations, and future headless servers: boards, pieces, line clears, funds,
//! weapons, bazaar flow, scoring events, and AI primitives.
//!
//! `battletris-core` must not depend on Bevy, rendering, input devices, sockets,
//! databases, or platform UI APIs. Those concerns belong to adapter crates.

pub mod ai;
pub mod board;
pub mod cell;
pub mod fixtures;
pub mod game;
pub mod piece;
pub mod piece_generator;
pub mod recon;
pub mod rng;
pub mod score;
pub mod weapons;
