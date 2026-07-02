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
| Board dimensions | Legacy width, height, coordinate bounds | `battletris-core::board` | Constant tests and board fixture metadata | Implemented in Phase 2 |
| Board coordinates | Origin, row order, x/y semantics, upside-down snapshot behavior | `battletris-core::board` | Snapshot and text fixture round trips | Implemented in Phase 2 |
| Cell identity | Visible, invisible, structure, gimp, die, happy, frown IDs and typed equivalents | `battletris-core::cell` | Cell construction and legacy-ID mapping tests | Implemented in Phase 2 |
| Cell value | Die values, happy value, frown value, gimp value preservation | `battletris-core::cell` | Value/removability table tests | Implemented in Phase 2 |
| Occupancy | In-bounds, out-of-bounds, empty, occupied, and later Fallout exceptions | `battletris-core::board` | Occupancy invariant tests | Implemented for default occupancy in Phase 2; Fallout pending Phase 9 |
| Board snapshots | Typed core snapshot plus legacy-compatible ID view | `battletris-core`, `battletris-protocol` | Snapshot round trips and representative legacy payload fixtures | Core typed and legacy-ID snapshots implemented in Phase 2; protocol adapter pending Phase 14 |
| Fixture grammar | Compact text format for boards, cells, metadata, expected output | `battletris-core::fixtures` | Parser tests and golden examples | Board fixture parser implemented in Phase 2 |
| Pieces | Legacy IDs, shapes, spawn placement, rotation width, custom rotations | `battletris-core::piece` | Per-piece shape tests plus standard/custom rotation and collision fixtures in `battletris-core::piece` tests | Implemented in Phase 3 |
| Piece generation | Weighted piece distribution, dice pips, happy queue, weapon probability hooks | `battletris-core::piece_generator` | Seeded generator tests for stable sequences, happy queue priority, die rerolls, and probability hook slots | Implemented in Phase 4 for core generation; weapon activation plumbing pending weapon phases |
| Timers and inputs | Default drop, fast drop, slide, movement, rotation, lock, spawn failure | `battletris-core::game`, `battletris-client` | Headless command/tick tests for movement, rotation, drop timing, fast-drop scoring, slide-to-lock, and spawn failure; client HUD/input helper tests cover adapter previews and key-mapped bazaar purchases | Implemented in Phase 4 for core piece loop; client adapter implemented in Phase 12 |
| Line clears | Detection, removal, row drop behavior, Force exception | `battletris-core::board`, `battletris-core::score` | Board line-clear tests cover single/double/triple/tetris clears, row drop, and Force no-drop behavior | Implemented in Phase 5 |
| Funds and score | Dice/happy funds, fast-drop score, economy events | `battletris-core::score`, `battletris-core::game` | Board and piece-loop tests cover die/happy funds, missed-happy frowns, line count, and fast-drop score preservation | Implemented in Phase 5 |
| Bazaar trigger | Combined-line threshold and wrap behavior | `battletris-core::score` | `BazaarTracker` wrap tests cover `19+1`, `18+2`, `19+4`, and `39+2` | Implemented in Phase 5 |
| Game flow | Start, pause, resume, bazaar enter/leave, death, game over | `battletris-core::game` | `TwoPlayerGame` unit tests cover start log, pause/resume gating, bazaar enter/leave, deterministic sequence numbers, death, and game over | Implemented in Phase 6 |

## Weapons And AI

| Area | Trace Needed | Owner | Test Or Fixture Evidence | Status |
| --- | --- | --- | --- | --- |
| Weapon catalog | Token order, names, descriptions, prices, line durations | `battletris-core::weapons` | Catalog consistency tests in `battletris-core::weapons` cover stable token order, names, prices, and durations | Implemented in Phase 7 |
| Arsenal | Ten slots, number-key semantics, stacking, purchase/remove-before-commit | `battletris-core::weapons`, `battletris-core::game` | Arsenal, bazaar staging/refund, Carter price, and two-player commit tests in `battletris-core` | Implemented in Phase 7 |
| One-shot weapons | Board, economy, queue, and arsenal effects with legacy quirks | `battletris-core::board`, `battletris-core::game`, `battletris-core::weapons` | `phase_8_one_shot_weapons_apply_deterministic_scenarios` covers Swap, Rise up, Flip out, Missing Pieces, Piece It Together, Blind Cleric, Keating, Reagan, Nice Day, Bug, Susan, Twilight, and Gimp | Implemented in Phase 8 |
| Timed weapons | Activation, active behavior, line expiration, stacking/restoration | `battletris-core::weapons`, `battletris-core::game`, `battletris-core::board` | `phase_9_timed_weapons_apply_activation_effects`, `timed_weapon_launch_activates_and_stacks_line_duration`, `timed_weapons_expire_after_target_line_clears_and_restore_hooks`, and `timed_effects_change_core_behaviors_while_active` cover activation, active effects, expiration, and representative restoration behavior | Implemented in Phase 9 |
| Launch pipeline | Local launch, incoming queue flush timing, Mirror behavior, nullification exceptions | `battletris-core::weapons`, `battletris-core::game`, `battletris-protocol` | `mirror_reflects_supported_launches_and_nullifies_exception_tokens` and `queued_incoming_weapons_flush_fifo_after_target_placement` cover core launch behavior; protocol adapter pending Phase 14 | Implemented in Phase 10 for core; protocol wire mapping pending Phase 14 |
| Recon | William Ames, Ace of Spies, Condor visibility and funds behavior | `battletris-core::recon`, `battletris-core::game` | `condor_reports_exact_board_and_funds`, `ames_and_ace_sample_occupied_cells_deterministically`, and `phase_10_spy_weapons_activate_on_launcher_and_emit_recon_after_placement` | Implemented in Phase 10 for core; client panel/protocol adapter pending later phases |
| Computer opponent | Placement search, board evaluation, difficulty, shopping, weapon strategy | `battletris-core::ai` | Seeded board-state decision tests | Handoff researched; pending implementation |

## Adapters, Persistence, And Release

| Area | Trace Needed | Owner | Test Or Fixture Evidence | Status |
| --- | --- | --- | --- | --- |
| Protocol | Framing, message groups, challenge/start/play/bazaar/game-over/disconnect flows | `battletris-protocol` | Serialization and scripted protocol tests | ADR 0003 and handoff accepted; pending implementation |
| Networking mode | Direct IP, LAN discovery, hosted relay/lobby, or authority responsibilities | `battletris-client`, `battletris-server` | ADR plus integration tests | ADR 0004 accepted for Phase 15; hosted authority pending Phase 17 |
| Player records | Wins, losses, rank value, streaks, records, head-to-head data | `battletris-db` | Ranking and migration tests | ADR 0005 accepted; rank/stat bug decision pending Phase 16 |
| Identity scope | Local identity, server/community labeling, ranked trust model | `battletris-db`, `battletris-server` | ADR and persistence tests | Pending Phase 16 decision |
| Client screens | Startup, challenge, sleep/about, roster, game, bazaar, game over | `battletris-client` | Client unit tests cover core-state HUD rendering, next-piece previews, control layouts, local computer mode, semantic sound mapping, and bazaar key affordability; workspace build verifies Bevy shell wiring | Game, bazaar, game-over, startup, settings, challenge placeholder, sleep placeholder, about placeholder, and roster placeholder implemented in Phase 13 |
| Assets and themes | Original-inspired default theme, scalable sprites, optional recovered assets | `battletris-client`, `battletris-tools` | Theme switching and scale settings are covered by client adapter tests/build checks | Original-inspired and high-contrast scalable sprite themes implemented in Phase 13; packaged external asset packs remain later polish |
| Audio | Semantic sound events, generated default sounds, optional recovered pack | `battletris-client::audio`, `battletris-tools` | Client sound-event mapping tests cover line clear and bazaar semantic events | Semantic generated-default/muted sound-pack boundary implemented in Phase 13; actual recovered or packaged audio files remain optional later polish |
| Distribution | Target platforms, packaging, asset bundling, save/config locations | workspace, release tooling | Packaged build smoke tests | External packaging research complete; pending implementation |
