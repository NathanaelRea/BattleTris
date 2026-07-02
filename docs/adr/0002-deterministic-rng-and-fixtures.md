# ADR 0002: Deterministic RNG And Fixtures

- Status: Accepted
- Date: 2026-07-01
- Phase: 4

## Context

The legacy implementation mixes C library random functions and preserves no
portable RNG contract. The rewrite needs deterministic piece generation, dice
pips, weapon random effects, replay inputs, and fixture tests across Linux,
macOS, and Windows.

Changing RNG behavior after release would break seeded simulations, snapshots,
and replay files, so the seed model must be explicit before Phase 4 implements
piece generation.

## Decision

Use `rand = "0.10.1"` and `rand_chacha = "0.10.0"` in `battletris-core` with
OS/thread RNG features disabled:

```toml
rand = { version = "0.10.1", default-features = false, features = ["std"] }
rand_chacha = { version = "0.10.0", default-features = false, features = ["std"] }
```

Core code should create RNGs only from an explicit `GameSeed`, not from OS
entropy or `thread_rng`. Use a stable seed representation, preferably
`GameSeed([u8; 32])`, and partition named deterministic streams for piece
selection, dice pips, happy-piece queues, and weapon random effects. Replays and
protocol flows should serialize the seed, core/protocol version, and tick-indexed
player inputs rather than serializing `rand_chacha` internal state.

Use compact text fixtures for core scenarios, `insta = "1.48.0"` for reviewed
golden snapshots, `proptest = "1.11.0"` for invariants and round trips, and
`toml = "1.1.2+spec-1.1.0"` for structured fixture metadata where needed.

## Consequences

- `battletris-core` remains deterministic and Bevy-free.
- Seeded simulations can be reproduced across platforms and CI runs.
- Changing RNG crate, stream partitioning, or seed encoding after shipped replay
  files requires a new ADR and compatibility fixtures.
- The rewrite preserves legacy distributions and visible outcomes, not the exact
  legacy RNG implementation.
