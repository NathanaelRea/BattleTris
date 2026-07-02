# Traceability Checklist

Use this checklist to connect compatibility-sensitive source facts to rewrite docs, Rust owners, and tests. Source facts live in `docs/rewrite-spec.md` and high-risk handoff notes live in `docs/legacy-implementation-handoff.md`; external research is tracked in `plan-research.md` and `docs/external-research.md`.

## Completion Rules

- Every preserved behavior has a legacy source citation in `docs/rewrite-spec.md`.
- Every implemented behavior has an owning crate or module named in docs or code.
- Every compatibility-sensitive behavior has deterministic tests, fixtures, or protocol examples.
- Every deliberate behavior change has fixture coverage proving the new behavior or an ADR explaining the trade-off.
- Every new domain term or resolved ambiguity is reflected in `CONTEXT.md`.

## Core Gameplay

| Area | Trace Needed | Owner | Test Or Fixture Evidence | Status |
| --- | --- | --- | --- | --- |
| Board dimensions | Legacy width, height, coordinate bounds | `battletris-core::board` | Constant tests and board fixture metadata | Handoff researched; pending implementation |
| Board coordinates | Origin, row order, x/y semantics, upside-down snapshot behavior | `battletris-core::board` | Snapshot and text fixture round trips | Handoff researched; pending implementation |
| Cell identity | Visible, invisible, structure, gimp, die, happy, frown IDs and typed equivalents | `battletris-core::cell` | Cell construction and legacy-ID mapping tests | Handoff researched; pending implementation |
| Cell value | Die values, happy value, frown value, gimp value preservation | `battletris-core::cell` | Value/removability table tests | Handoff researched; pending implementation |
| Occupancy | In-bounds, out-of-bounds, empty, occupied, and later Fallout exceptions | `battletris-core::board` | Occupancy invariant tests | Handoff researched; pending implementation |
| Board snapshots | Typed core snapshot plus legacy-compatible ID view | `battletris-core`, `battletris-protocol` | Snapshot round trips and representative legacy payload fixtures | Handoff researched; pending implementation |
| Fixture grammar | Compact text format for boards, cells, metadata, expected output | `battletris-core::fixtures` | Parser tests and golden examples | Fixture conventions documented; pending implementation |
| Pieces | Legacy IDs, shapes, spawn placement, rotation width, custom rotations | `battletris-core::piece` | Per-piece shape and rotation fixtures | Handoff researched; pending implementation |
| Piece generation | Weighted piece distribution, dice pips, happy queue, weapon probability hooks | `battletris-core::piece_generator` | Seeded generation fixtures | ADR 0002 and handoff accepted; pending implementation |
| Timers and inputs | Default drop, fast drop, slide, movement, rotation, lock, spawn failure | `battletris-core::game`, `battletris-client` | Command/tick simulation tests | External adapter and handoff researched; pending implementation |
| Line clears | Detection, removal, row drop behavior, Force exception | `battletris-core::board`, `battletris-core::score` | Single/double/triple/tetris fixtures | Handoff researched; pending implementation |
| Funds and score | Dice/happy funds, fast-drop score, economy events | `battletris-core::score`, `battletris-core::economy` | Funds and score scenario fixtures | Handoff researched; pending implementation |
| Bazaar trigger | Combined-line threshold and wrap behavior | `battletris-core::bazaar` | Two-player line-count scenarios | Handoff researched; pending implementation |
| Game flow | Start, pause, resume, bazaar enter/leave, death, game over | `battletris-core::game` | Headless scripted full-game tests | Handoff researched; pending implementation |

## Weapons And AI

| Area | Trace Needed | Owner | Test Or Fixture Evidence | Status |
| --- | --- | --- | --- | --- |
| Weapon catalog | Token order, names, descriptions, prices, line durations | `battletris-core::weapons` | Catalog consistency tests against extracted data | Handoff researched; pending implementation |
| Arsenal | Ten slots, number-key semantics, stacking, purchase/remove-before-commit | `battletris-core::arsenal`, `battletris-core::bazaar` | Arsenal and bazaar scenario tests | Handoff researched; pending implementation |
| One-shot weapons | Board, economy, queue, and arsenal effects with legacy quirks | `battletris-core::weapons` | One scenario per weapon | Handoff researched; pending implementation |
| Timed weapons | Activation, active behavior, line expiration, stacking/restoration | `battletris-core::weapons` | Activation/effect/expiration fixtures | Handoff researched; pending implementation |
| Launch pipeline | Local launch, incoming queue flush timing, Mirror behavior, nullification exceptions | `battletris-core::weapons`, `battletris-protocol` | Cross-player scripted simulations | Handoff researched; pending implementation |
| Recon | William Ames, Ace of Spies, Condor visibility and funds behavior | `battletris-core::recon` | Deterministic visibility scenarios | Handoff researched; pending implementation |
| Computer opponent | Placement search, board evaluation, difficulty, shopping, weapon strategy | `battletris-core::ai` | Seeded board-state decision tests | Handoff researched; pending implementation |

## Adapters, Persistence, And Release

| Area | Trace Needed | Owner | Test Or Fixture Evidence | Status |
| --- | --- | --- | --- | --- |
| Protocol | Framing, message groups, challenge/start/play/bazaar/game-over/disconnect flows | `battletris-protocol` | Serialization and scripted protocol tests | ADR 0003 and handoff accepted; pending implementation |
| Networking mode | Direct IP, LAN discovery, hosted relay/lobby, or authority responsibilities | `battletris-client`, `battletris-server` | ADR plus integration tests | ADR 0004 accepted for Phase 15; hosted authority pending Phase 17 |
| Player records | Wins, losses, rank value, streaks, records, head-to-head data | `battletris-db` | Ranking and migration tests | ADR 0005 accepted; rank/stat bug decision pending Phase 16 |
| Identity scope | Local identity, server/community labeling, ranked trust model | `battletris-db`, `battletris-server` | ADR and persistence tests | Pending Phase 16 decision |
| Client screens | Startup, challenge, sleep/about, roster, game, bazaar, game over | `battletris-client` | Smoke tests and manual scenario checklist | External Bevy/UI research complete; pending implementation |
| Assets and themes | Original-inspired default theme, scalable sprites, optional recovered assets | `battletris-client`, `battletris-tools` | Asset loading smoke tests | External rendering/assets research complete; pending implementation |
| Audio | Semantic sound events, generated default sounds, optional recovered pack | `battletris-client::audio`, `battletris-tools` | Sound-event mapping tests | External audio research complete; pending implementation |
| Distribution | Target platforms, packaging, asset bundling, save/config locations | workspace, release tooling | Packaged build smoke tests | External packaging research complete; pending implementation |
