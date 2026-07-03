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

## Dependency Placement From External Research

External research packets live in `docs/external-research.md`. These dependency
recommendations should be applied by the phase that first needs each crate, not
added preemptively.

| Crate | Recommended dependencies |
| --- | --- |
| `battletris-core` | `rand = "0.10.1"` and `rand_chacha = "0.10.0"` with OS/thread RNG features disabled, as accepted in ADR 0002. Keep Bevy, sockets, database drivers, and platform APIs out. |
| `battletris-protocol` | `serde = "1.0.228"` with `derive`, `postcard = "1.1.3"` with `use-std`, and `bytes = "1.12.0"`, as accepted in ADR 0003. |
| `battletris-client` | `bevy = "0.19.0"` once the workspace is on Rust `1.95.0` or newer. Use explicit 2D/audio/UI/png/wav/vorbis/x11 features rather than Bevy defaults, with Wayland available through the client `wayland` Cargo feature for environments with Wayland development packages. Use Bevy UI plus a local action map before considering `bevy_egui` or `leafwing-input-manager`. |
| `battletris-client` settings | `directories = "6.0.0"` and `toml = "1.1.2+spec-1.1.0"` when settings persistence is implemented. |
| `battletris-client` networking | `tokio = "1.52.3"`, `tokio-util = "0.7.18"`, and `mdns-sd = "0.20.1"` for direct TCP connect plus optional LAN discovery, as accepted in ADR 0004. |
| `battletris-db` | `rusqlite = "0.40.1"` with `bundled`, `refinery = "0.9.2"` with `rusqlite-bundled`, and `directories = "6.0.0"`, as accepted in ADR 0005. |
| `battletris-tools` | `hound = "3.5.1"` for generated WAV sound packs. Packaging may evaluate `cargo-dist = "0.32.0"` in Phase 18. |
| Tests and fixtures | `insta = "1.48.0"`, `proptest = "1.11.0"`, and `toml = "1.1.2+spec-1.1.0"` where the owning phase needs snapshots, property tests, or fixture metadata. |

## Documentation Expectations

Each crate must have crate-level documentation explaining its purpose. Public
modules and public items should be documented when introduced. Module docs should
state ownership boundaries when code touches gameplay, protocol, persistence,
rendering, networking, or tools.

## Local Quality Gate

On Linux, the Bevy client audio stack needs `pkg-config` and ALSA development
files before Cargo can build the workspace. Run the native dependency preflight
when setting up a workstation or when CI reports a missing system library:

```sh
./scripts/check-native-deps.sh
```

Run the full gate before handing off a phase:

```sh
./scripts/full-check.sh
```

Run the package CI job locally when release packaging or bundled assets change:

```sh
./scripts/package-check.sh
```

CI installs the same Linux native packages, runs the same full gate for every
push and pull request, and uses `./scripts/package-check.sh` for package smoke.
