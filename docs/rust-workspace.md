# Rust Workspace

The Rust rewrite lives alongside the legacy C++ tree. The legacy source remains
under `usr/src/`; new Rust code lives in a Cargo workspace rooted at the
repository root with crates under `crates/`.

## Crate Boundaries

| Crate | Purpose |
| --- | --- |
| `battletris-core` | Deterministic gameplay rules, board and piece state, line clears, funds, weapons, bazaar flow, scoring events, and AI primitives. |
| `battletris-protocol` | Versioned network messages, fixed-width framing, challenge/start/play/bazaar/game-over flows, and protocol compatibility fixtures. |
| `battletris-client` | Bevy desktop client, rendering, input mapping, menus, settings, assets, and audio. |
| `battletris-server` | Optional lobby, relay, presence, challenge routing, or dedicated server service selected by later networking ADRs. |
| `battletris-db` | Player profiles, ranking, stats, head-to-head records, migrations, and optional legacy import/export. |
| `battletris-tools` | Asset conversion, generated audio, replay inspection, protocol fixtures, legacy data extraction, and admin utilities. |

`battletris-core` must stay free of Bevy, rendering, input, sockets, database,
and platform UI dependencies. Client, server, protocol, database, and tooling
crates adapt core state and events to their respective external systems.

## Documentation Expectations

Each crate must have crate-level documentation explaining its purpose. Public
modules and public items should be documented when introduced. Module docs should
state ownership boundaries when code touches gameplay, protocol, persistence,
rendering, networking, or tools.

## Local Quality Gate

Run these commands before handing off a phase:

```sh
./scripts/full-check.sh
```

Linux CI runs the same gate for every push and pull request.
