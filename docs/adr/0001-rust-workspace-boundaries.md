# ADR 0001: Rust Workspace Boundaries

- Status: Accepted
- Date: 2026-07-01
- Phase: 1

## Context

The rewrite needs deterministic gameplay logic, a Bevy client, modern networking,
player persistence, optional server infrastructure, and tooling without coupling
those concerns together. The legacy C++ tree must remain in place while the Rust
rewrite grows beside it.

Phase 1 also needs a quality gate and documentation expectations before later
agents add behavior in parallel.

## Decision

Use a Cargo workspace at the repository root with baseline crates under
`crates/`:

| Crate | Boundary |
| --- | --- |
| `battletris-core` | Deterministic game rules, state, events, weapons, economy, bazaar, scoring, and AI primitives. |
| `battletris-protocol` | Wire messages, framing, versioning, and protocol compatibility fixtures. |
| `battletris-client` | Bevy application, rendering, input, menus, settings, assets, and audio adapters. |
| `battletris-server` | Optional lobby, relay, presence, challenge routing, or dedicated server service. |
| `battletris-db` | Player profiles, ranking, stats, head-to-head records, migrations, and persistence. |
| `battletris-tools` | Asset/audio generation, replay/protocol inspection, legacy extraction, and admin tools. |

`battletris-core` must not depend on Bevy, rendering, input adapters, socket
transports, database drivers, or platform UI APIs. It should expose deterministic
state transitions and events that other crates consume.

Every crate must have crate-level documentation. Public modules and public items
introduced by implementation phases should document their ownership boundary and
compatibility-sensitive behavior. The workspace enables the `missing_docs` lint
as a warning so the CI clippy gate can fail undocumented public API additions.

The Phase 1 Linux quality gate is:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace --all-targets
```

## Consequences

- The rewrite can add deterministic tests below Bevy from Phase 2 onward.
- Client, networking, persistence, and tools can evolve without owning core game
  rules.
- Later phases may add dependencies inside adapter crates, but adding Bevy or
  platform dependencies to `battletris-core` requires a new ADR.
- The workspace starts dependency-free to keep Phase 1 CI fast and avoid making
  rendering or networking decisions before the relevant phases.
