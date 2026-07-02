//! Network protocol types and serialization boundaries.
//!
//! This crate will define versioned wire messages, fixed-width framing,
//! challenge/start/play/bazaar/game-over flows, and compatibility tests derived
//! from the legacy protocol. It must keep wire messages separate from local core
//! events so transports can change without changing gameplay rules.
