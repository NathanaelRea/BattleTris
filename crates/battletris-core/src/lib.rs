//! Deterministic BattleTris rules and state.
//!
//! This crate owns gameplay behavior that must be replayable in tests, network
//! simulations, and future headless servers: boards, pieces, line clears, funds,
//! weapons, bazaar flow, scoring events, and AI primitives.
//!
//! `battletris-core` must not depend on Bevy, rendering, input devices, sockets,
//! databases, or platform UI APIs. Those concerns belong to adapter crates.

pub mod fixtures;
