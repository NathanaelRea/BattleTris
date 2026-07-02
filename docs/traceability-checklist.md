# Traceability Checklist

Use this checklist to connect compatibility-sensitive source facts to rewrite docs, Rust owners, and tests. The source facts live in `docs/rewrite-spec.md`; remaining upfront research is tracked in `plan-research.md`.

## Completion Rules

- Every preserved behavior has a legacy source citation in `docs/rewrite-spec.md`.
- Every implemented behavior has an owning crate or module named in docs or code.
- Every compatibility-sensitive behavior has deterministic tests, fixtures, or protocol examples.
- Every deliberate behavior change has fixture coverage proving the new behavior or an ADR explaining the trade-off.
- Every new domain term or resolved ambiguity is reflected in `CONTEXT.md`.

## Core Gameplay

| Area | Trace Needed | Owner | Test Or Fixture Evidence | Status |
| --- | --- | --- | --- | --- |
| Board dimensions | Legacy width, height, coordinate bounds | `battletris-core::board` | Constant tests and board fixture metadata | Pending implementation |
| Board coordinates | Origin, row order, x/y semantics, upside-down snapshot behavior | `battletris-core::board` | Snapshot and text fixture round trips | Pending research |
| Cell identity | Visible, invisible, structure, gimp, die, happy, frown IDs and typed equivalents | `battletris-core::cell` | Cell construction and legacy-ID mapping tests | Pending implementation |
| Cell value | Die values, happy value, frown value, gimp value preservation | `battletris-core::cell` | Value/removability table tests | Pending implementation |
| Occupancy | In-bounds, out-of-bounds, empty, occupied, and later Fallout exceptions | `battletris-core::board` | Occupancy invariant tests | Pending implementation |
| Board snapshots | Typed core snapshot plus legacy-compatible ID view | `battletris-core`, `battletris-protocol` | Snapshot round trips and representative legacy payload fixtures | External research complete; pending implementation |
| Fixture grammar | Compact text format for boards, cells, metadata, expected output | `battletris-core::fixtures` | Parser tests and golden examples | ADR 0002 accepted; pending implementation |
| Pieces | Legacy IDs, shapes, spawn placement, rotation width, custom rotations | `battletris-core::piece` | Per-piece shape and rotation fixtures | Pending research |
| Piece generation | Weighted piece distribution, dice pips, happy queue, weapon probability hooks | `battletris-core::piece_generator` | Seeded generation fixtures | ADR 0002 accepted; pending implementation |
| Timers and inputs | Default drop, fast drop, slide, movement, rotation, lock, spawn failure | `battletris-core::game`, `battletris-client` | Command/tick simulation tests | External Bevy adapter research complete; pending implementation |
| Line clears | Detection, removal, row drop behavior, Force exception | `battletris-core::board`, `battletris-core::score` | Single/double/triple/tetris fixtures | Pending research |
| Funds and score | Dice/happy funds, fast-drop score, economy events | `battletris-core::score`, `battletris-core::economy` | Funds and score scenario fixtures | Pending research |
| Bazaar trigger | Combined-line threshold and wrap behavior | `battletris-core::bazaar` | Two-player line-count scenarios | Pending research |
| Game flow | Start, pause, resume, bazaar enter/leave, death, game over | `battletris-core::game` | Headless scripted full-game tests | Pending research |

## Weapons And AI

| Area | Trace Needed | Owner | Test Or Fixture Evidence | Status |
| --- | --- | --- | --- | --- |
| Weapon catalog | Token order, names, descriptions, prices, line durations | `battletris-core::weapons` | Catalog consistency tests against extracted data | Pending research |
| Arsenal | Ten slots, number-key semantics, stacking, purchase/remove-before-commit | `battletris-core::arsenal`, `battletris-core::bazaar` | Arsenal and bazaar scenario tests | Pending research |
| One-shot weapons | Board, economy, queue, and arsenal effects with legacy quirks | `battletris-core::weapons` | One scenario per weapon | Pending research |
| Timed weapons | Activation, active behavior, line expiration, stacking/restoration | `battletris-core::weapons` | Activation/effect/expiration fixtures | Pending research |
| Launch pipeline | Local launch, incoming queue flush timing, Mirror behavior, nullification exceptions | `battletris-core::weapons`, `battletris-protocol` | Cross-player scripted simulations | Pending research |
| Recon | William Ames, Ace of Spies, Condor visibility and funds behavior | `battletris-core::recon` | Deterministic visibility scenarios | Pending research |
| Computer opponent | Placement search, board evaluation, difficulty, shopping, weapon strategy | `battletris-core::ai` | Seeded board-state decision tests | Pending research |

## Adapters, Persistence, And Release

| Area | Trace Needed | Owner | Test Or Fixture Evidence | Status |
| --- | --- | --- | --- | --- |
| Protocol | Framing, message groups, challenge/start/play/bazaar/game-over/disconnect flows | `battletris-protocol` | Serialization and scripted protocol tests | ADR 0003 accepted; pending implementation |
| Networking mode | Direct IP, LAN discovery, hosted relay/lobby, or authority responsibilities | `battletris-client`, `battletris-server` | ADR plus integration tests | ADR 0004 accepted for Phase 15; hosted authority pending Phase 17 |
| Player records | Wins, losses, rank value, streaks, records, head-to-head data | `battletris-db` | Ranking and migration tests | ADR 0005 persistence backend accepted; pending implementation |
| Identity scope | Local identity, server/community labeling, ranked trust model | `battletris-db`, `battletris-server` | ADR and persistence tests | Pending decision |
| Client screens | Startup, challenge, sleep/about, roster, game, bazaar, game over | `battletris-client` | Smoke tests and manual scenario checklist | External Bevy/UI research complete; pending implementation |
| Assets and themes | Original-inspired default theme, scalable sprites, optional recovered assets | `battletris-client`, `battletris-tools` | Asset loading smoke tests | External rendering/assets research complete; pending implementation |
| Audio | Semantic sound events, generated default sounds, optional recovered pack | `battletris-client::audio`, `battletris-tools` | Sound-event mapping tests | External audio research complete; pending implementation |
| Distribution | Target platforms, packaging, asset bundling, save/config locations | workspace, release tooling | Packaged build smoke tests | External packaging research complete; pending implementation |
