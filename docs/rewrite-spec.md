# BattleTris Rewrite Spec

Discovery and spec extraction from the legacy C++/Motif codebase.

This document is intentionally source-oriented. It records what the rewrite must preserve, which legacy files own each behavior today, and where those behaviors should land in the Rust/Bevy workspace.

## Source Baseline

- Primary legacy source root: `usr/src/`.
- Existing orientation docs: `README.md`, `PORTING.md`, `usr/src/man/BattleTris.1`, `usr/src/man/btref.1`.
- Core gameplay identity from `README.md:6-14` and `README.md:50-77`: two-player networked Tetris, funds from dice and smiley clears, weapon bazaar every 20 combined lines, arsenal launch keys, first death loses, optional unranked computer play.
- Porting inventory in `PORTING.md:16-32` is accurate at a directory level and is expanded below for rewrite planning.

## Rewrite Modules

Use these module names as planning targets. Exact crate names can change in Phase 1, but the boundaries should stay explicit.

| Module                | Responsibility                                                                                                                               |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `battletris-core`     | Deterministic board, pieces, line clears, funds, scoring events, weapons, bazaar rules, arsenal, AI primitives, game clock/tick model.       |
| `battletris-protocol` | Fixed-width network framing, protocol messages, versioning, challenge/start/game/bazaar/disconnect flows, serialization compatibility tests. |
| `battletris-client`   | Bevy app, rendering, input mapping, menus, local settings, audio playback, asset/theme loading, direct-connect UI.                           |
| `battletris-server`   | Optional lobby/relay/dedicated service, presence, challenge routing, ranked result intake if selected.                                       |
| `battletris-db`       | Player profiles, stats, rank math, persistence, migration/import/export from legacy concepts.                                                |
| `battletris-tools`    | Asset conversion/generation, audio generation, replay/protocol inspection, legacy data extraction, admin/referee tools.                      |

## Compatibility Stance

Default decision for the first Rust implementation: preserve gameplay semantics first, modernize infrastructure and presentation behind clean boundaries.

Resolved product decisions:

| Area               | Decision                                                                                                                                                                                                |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Weapon scope       | V1 ships every original weapon in the legacy catalog. Balancing changes can happen later only after compatibility tests exist.                                                                          |
| Presentation       | The default look should be as faithful as practical to the original 1994/Motif presentation, while keeping themes, scaling, filtering, and sound packs swappable through data/configuration boundaries. |
| Persistence        | Start with a new schema and migrations. No known legacy records are required for v1, but keep an import/export path available if old DB records surface.                                                |
| Platforms          | V1 targets Linux, macOS, and Windows. Web is out of v1 unless a later ADR adds it.                                                                                                                      |
| Networking MVP     | Start with local networking: LAN/direct-IP or local-host flows before hosted infrastructure.                                                                                                            |
| Networking MVP+    | ADR 0007 selects a self-hosted lobby plus ranked-result authority foundation. Relay transport, NAT traversal, reconnect tokens, and fully authoritative per-tick simulation remain later scope.          |
| Ranked trust       | Local/direct-connect play can assume friendly participants. Hosted or self-hosted ranked play is server-verified by matching result claims from both players before records mutate.                     |
| Spectators/replays | Defer spectators and replays to MVP++ or later.                                                                                                                                                         |
| Identity           | V1 has no account requirement. MVP+ can add server-issued identity for hosted/self-hosted servers.                                                                                                      |
| Rankings           | Rankings are scoped to a server/community, not globally hard-coded. A "main" network can emerge operationally if one deployment becomes canonical.                                                      |
| Legacy behavior    | Preserve original functionality as faithfully as possible because we do not yet have hands-on play experience to justify intentional balance changes.                                                   |
| Weapon text        | Keep original weapon names and descriptions unchanged for compatibility/faithfulness unless a later content ADR says otherwise.                                                                         |
| Settings           | V1 settings should expose scale/upscaling, pixel filtering, theme, sound pack, controls, and networking defaults where implemented.                                                                     |
| Distribution       | Source builds are the baseline; GitHub Releases are the first likely packaged distribution path. Storefront/package-manager distribution is later.                                                      |
| Repository layout  | Keep the legacy C++ tree in place. Add the Rust workspace in the canonical Rust location for the selected layout, preferably `crates/` for multiple workspace crates.                                   |

Compatibility requirements unless an ADR deliberately changes them:

| Area                | Preserve                                                                                                                                                                                                                                                                                                              |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Board and pieces    | 10x28 board, 8x8 piece maps, default spawn constants `x=5/y=0` with actual placement offset by half the piece rotation width, no wall kicks, original piece shapes, weird pieces, 4x4 piece, long-dong piece. Source: `usr/src/game/BTConstants.H:89-126`, `usr/src/game/BTPiece.C`, `usr/src/game/BTGame.C:795-803`. |
| Controls and timers | Default drop 512ms, fast drop 10ms, slide 150ms, movement keys `j/l/k`, space fast drop, `p` pause, `c` Condor spy toggle in computer mode, number keys for arsenal. Source: `usr/src/game/BTConstants.H:92-94`, `usr/src/game/BattleTris.C:70-117`, `usr/src/game/BTGame.C:704-740`.                                 |
| Funds               | Dice values 1-6 and happy value 150; funds from a clear are sum of box values multiplied by lines cleared. Source: `usr/src/game/BTBox.H:78-103`, `usr/src/game/BTBoardManager.C:577-616`.                                                                                                                            |
| Bazaar cadence      | Bazaar after 20 combined player lines, using the original combined-line wrap behavior. Source: `usr/src/game/BTScoreManager.C:14-15`, `usr/src/game/BTScoreManager.C:170-194`.                                                                                                                                        |
| Arsenal             | Ten slots, numbered launch controls, identical weapon quantities stack. Source: `usr/src/game/BTConstants.H:133-135`, `usr/src/game/BTArsenal.C:26-56`, `usr/src/game/BTWeaponManager.C:184-224`.                                                                                                                     |
| Weapons             | Preserve and ship the full original weapon catalog and line-based durations for v1 compatibility. Source: `usr/src/share/btweapons.db`, `usr/src/share/btweaponsp.db`, `usr/src/game/BTProtocol.H:79-115`.                                                                                                            |
| AI mode             | Computer play exists and remains unranked. It should be ported as deterministic core logic, not as Bevy UI behavior. Source: `usr/src/game/BTComputer.C`, `usr/src/game/BTCBoard.C`.                                                                                                                                  |
| Stats concept       | Preserve wins, losses, rank value, current streak value/type, best scores, line/funds records, fastest kill/death, longest game, and head-to-head records. Source: `usr/src/db/BTPlayer.H:36-79`.                                                                                                                     |

Modernization decisions for the rewrite:

| Area                | Modernize                                                                                                                                                                                                                    |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rendering/UI        | Replace Motif/X11 with Bevy while preserving screens and flows. Current external recommendation is Bevy `0.19.0`, sprite/atlas board rendering, Bevy UI for player screens, and a local action map. Motif internals should not leak into core rules or data models. |
| Assets              | Treat art, fonts, themes, and sounds as data packs. Use generated/source-controlled audio first; recovered original sounds are optional. See `docs/external-research.md` for the theme and sound-pack layout.              |
| Network format      | Do not preserve C++ ABI details like `unsigned long` packet sizes. ADR 0003 selects a fixed-width frame envelope with postcard payloads and compatibility fixtures.                                                            |
| Networking topology | Preserve challenge/start/play/bazaar/game-over flow, but do not require the old server/slave direct-peer architecture. ADR 0004 selects direct TCP connect plus best-effort LAN discovery for Phase 15; ADR 0007 adds self-hosted lobby discovery and ranked-result authority for Phase 17. |
| Identity            | Do not bind new identity to Unix login/GECOS/plan files. V1 should not require accounts; MVP+ hosted/self-hosted servers can issue identities.                                                                               |
| Persistence         | Do not port the hash-file DB implementation unless needed for import. ADR 0005 selects SQLite, migrations, and cross-platform project paths for the fresh schema.                                                              |
| Build               | Replace Make/autoconf and custom containers with Cargo workspace, CI, tests, clippy, formatting, and idiomatic Rust collections.                                                                                             |
| Platform support    | Keep platform-specific code behind adapters so the v1 target set can include Linux, macOS, and Windows.                                                                                                                      |

External implementation research packets are collected in
`docs/external-research.md`. Use that document for crate versions, feature flags,
Bevy API names, packaging notes, and rejected external alternatives. Keep this
spec focused on legacy behavior and compatibility facts.

High-risk implementation handoff notes are collected in
`docs/legacy-implementation-handoff.md`. Fixture conventions are in
`docs/core-fixtures.md`. Use those documents when implementing phase behavior so
source quirks and fixture formats stay consistent across agents.

Known quirks that need explicit tests or ADRs before changing:

| Quirk                                                                                                                            | Initial decision                                                                                                     |
| -------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Zero-duration weapons can set active flags without natural `BT_WPN_OFF`.                                                         | Implemented as explicit one-shot effects in `battletris-core`; timed active flags remain Phase 9 scope.              |
| Timed weapon stacking may not restore symmetrically, especially speed changes.                                                   | Preserve visible legacy stacking until a balancing ADR changes it.                                                   |
| `The Blind Cleric` description says it bombs a region, but code removes about half of all removable blocks.                      | Preserved as a one-shot randomized full-board removable-cell pass with deterministic core RNG tests.                 |
| `Twilight` and `Gimp` affect existing blocks only and have no undo path.                                                         | Preserved as one-shot board mutations; Twilight keeps value/removability in typed core state but remains lossy in legacy snapshots. |
| Incoming weapons are queued and flushed after piece placement, after scoring/recon updates and before the next piece is created. | Preserve timing in deterministic simulations unless networking prototype proves a better tick model.                 |
| Piece-probability weapons mutate probabilities directly rather than recomputing from the active weapon set.                      | Implemented as timed activation/expiration hooks with additive line-duration stacking in `battletris-core`.          |
| Some exotic pieces override standard rotation with custom state machines.                                                        | Extract per-piece rotation fixtures instead of assuming all shapes use the base square-matrix rotation.              |
| Bazaar start is a local/ring event derived from score wrap, not a peer wire packet.                                              | Protocol design should not require a `BT_START_BAZ` wire message unless an ADR intentionally changes the flow.       |
| Randomness mixes `rand`, `drand48`, and `lrand48`.                                                                               | Do not preserve RNG implementation; preserve distributions and outcomes through seedable Rust RNG fixtures.          |
| Protocol comments are stale in places.                                                                                           | Treat implementation as authoritative over comments.                                                                 |
| Legacy player records depend on Unix login/GECOS/plan files and contain named-player high-funds caps.                           | ADR 0006 intentionally fixes these fresh-schema bugs while preserving rank/stat concepts and tests.                  |

## Legacy Source Inventory By Domain

| Domain                   | Legacy files                                                                                                                                                                           | Notes                                                                                                            | Rewrite target                                                                              |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Board/grid/cells         | `usr/src/game/BTBoardManager.*`, `usr/src/game/BTBoard.*`, `usr/src/game/BTBox.*`, `usr/src/game/BTConstants.H`                                                                        | Board size, cell occupancy, line detection/removal, box values, invisible/gimp/structure cells, board snapshots. | `battletris-core::board`, `battletris-core::cell`, `battletris-protocol` board serializers. |
| Pieces and rotations     | `usr/src/game/BTPiece.*`, `usr/src/game/BTPieceManager.*`, `usr/src/game/BTConstants.H`                                                                                                | Normal pieces, die, happy/frown, weird pieces, 4x4, long-dong, piece probabilities, rotation logic.              | `battletris-core::piece`, `battletris-core::rng`.                                           |
| Game loop and controls   | `usr/src/game/BTGame.*`, `usr/src/game/BTTimeOut.H`, `usr/src/game/BTStopwatch.H`, `usr/src/game/BattleTris.C`                                                                         | Drop/slide/slick/hatter/jeopardy timeouts, placement, pause, death, key dispatch.                                | Core tick/game state plus `battletris-client` input adapter.                                |
| Funds/scoring/lines      | `usr/src/game/BTScore.*`, `usr/src/game/BTScoreManager.*`, `usr/src/game/BTLine.H`, `usr/src/db/BTGameStats.*`                                                                         | Local/opponent score snapshots, funds, line counts, bazaar trigger, game stats.                                  | `battletris-core::board`, `battletris-core::score`, `battletris-db::stats`.                 |
| Weapons runtime          | `usr/src/game/BTWeapon.*`, `usr/src/game/BTWeaponManager.*`, `usr/src/game/BTArsenal.*`, `usr/src/game/BTPimp.*`, `usr/src/share/btweapons.db`, `usr/src/share/btweaponsp.db`          | Catalog, prices, durations, active effects, launch/reflection/nullification, inventory.                          | `battletris-core::weapons`, `battletris-tools::catalog`.                                    |
| Bazaar                   | `usr/src/game/BTBazaar.*`, `usr/src/game/BTGame.*`, `usr/src/game/BTComputer.*`                                                                                                        | Shopping UI, add/remove/done, Carter price multiplier, computer shopping.                                        | Core bazaar/economy plus `battletris-client` UI.                                            |
| Recon/spy                | `usr/src/game/BTRecon.*`, `usr/src/game/BTCommManager.*`                                                                                                                               | Opponent board/funds visibility for Ames/Ace/Condor.                                                             | `battletris-core::recon`, client panel, protocol events.                                    |
| AI/computer              | `usr/src/game/BTComputer.*`, `usr/src/game/BTCBoard.*`, `usr/src/game/BTMove.*`, `usr/src/game/BTMovePath.*`                                                                           | Difficulty levels, board evaluation, placement search, shopping/weapon orders.                                   | `battletris-core::ai`.                                                                      |
| Peer gameplay networking | `usr/src/game/BTCommManager.*`, `usr/src/game/BTNetManager.*`, `usr/src/game/BTProtocol.H`, `usr/src/sockets/*`                                                                        | Challenge sockets, game packets, score/weapon/board/arsenal serialization, pause/error handling.                 | `battletris-protocol`, `battletris-client::net`.                                            |
| Server/presence          | `usr/src/daemons/*`, `usr/src/game/BTNetManager.*`, `usr/src/daemons/btserver.cf.in`                                                                                                   | Master daemon, slave daemons, DB gateway, presence/status, verify/update/result requests.                        | `battletris-server`.                                                                        |
| Player database          | `usr/src/db/*`, `usr/src/btref/*`, `usr/src/man/btref.1`                                                                                                                               | Persistent hash DB, locks, player/network records, rank/stats, referee CLI.                                      | `battletris-db`, `battletris-tools::admin`, optional import.                                |
| UI/screens               | `usr/src/game/BTStartup.*`, `BTChallenge.*`, `BTChallengeDialog.*`, `BTRoster.*`, `BTAbout.*`, `BTBiff.*`, `BTBazaar.*`, `BTGame.*`, `usr/src/widget/*`, `usr/src/share/BattleTris.ad` | Motif form/button/list/text/drawing wrappers, screens, resources, dialogs, sleep Biff.                           | `battletris-client`.                                                                        |
| Sound/audio              | `usr/src/game/BTSoundManager.*`, `usr/src/audio/DevAudio.*`, `usr/src/share/sounds/`                                                                                                   | Solaris `/dev/audio`, event-to-sound folders, currently missing original sounds.                                 | `battletris-client::audio`, `battletris-tools::audio-gen`.                                  |
| Assets/fonts             | `usr/src/art/*`, `usr/src/game/PPMReader.*`, `usr/src/game/bt_fonts.txt`, `usr/src/share/BattleTris.ad`                                                                                | PPM/XPM/XBM assets, app-default resources, font list, PPM loader.                                                | Asset packs, theme data, conversion tools.                                                  |
| Utility/container layers | `usr/src/stdlib/*`, `usr/src/signals/*`, `usr/src/widget/*`, `usr/src/sockets/*`                                                                                                       | Custom lists, blocks, signal handling, socket wrappers, Motif wrappers.                                          | Replace with Rust std/crates; only behavior-level tests survive.                            |

## Core Gameplay Facts

| Fact                                                                                                                                                                                                                 | Source                                                                                              | Rewrite note                                                                    |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| Board is 10 columns by 28 rows.                                                                                                                                                                                      | `usr/src/game/BTConstants.H:89-90`                                                                  | Core board dimensions should be constants and fixture metadata.                 |
| Piece maps are 8x8.                                                                                                                                                                                                  | `usr/src/game/BTConstants.H:101-102`                                                                | Useful for legacy shape extraction; Rust model can store compact coordinates.   |
| Default spawn constants are x=5, y=0; actual placement subtracts half the piece rotation width from x after construction.                                                                                            | `usr/src/game/BTConstants.H:98-99`, `usr/src/game/BTGame.C:795-803`                                 | Preserve spawn constants and final placement in core tests.                     |
| Default drop/fast-drop/slide are 512ms/10ms/150ms.                                                                                                                                                                   | `usr/src/game/BTConstants.H:92-94`                                                                  | Client can expose speed settings later, but default must match.                 |
| Normal piece IDs are 1-7; die is 8; happy is 9; weird/exotic are 10-18.                                                                                                                                              | `usr/src/game/BTConstants.H:104-126`                                                                | Extract shape fixtures.                                                         |
| Standard pieces use square-matrix rotation with no wall kicks; some weird pieces override rotation behavior.                                                                                                         | `usr/src/game/BTPiece.C:85-147`, `usr/src/game/BTPiece.C:338-426`, `usr/src/game/BTPiece.C:454-632` | Preserve feel before considering modern kicks; add per-piece rotation fixtures. |
| Piece generation uses weighted rejection probabilities: normal pieces keep at `.21`, dice at `1`, happy and long-dong at `.02`; Broken Record repeats the old piece 90 percent of the time unless a happy is queued. | `usr/src/game/BTPieceManager.C:16-60`, `usr/src/game/BTPieceManager.C:179-217`                      | Preserve distributions with deterministic RNG.                                  |
| Dice are one-cell pieces with random pip values 1-6.                                                                                                                                                                 | `usr/src/game/BTPiece.C:266-275`                                                                    | Pip value contributes to funds only when cleared.                               |
| Happy pieces are worth 150 only if cleared before becoming frowns.                                                                                                                                                   | `usr/src/game/BTPiece.C:277-286`, `usr/src/game/BTBoardManager.C:588-595`                           | Missed happy becomes unhappy/value 0 and can trigger idiot sound.               |
| Core RNG uses explicit seeds and named deterministic streams instead of legacy C RNG state.                                                                                                                          | ADR 0002                                                                                            | Implemented in `battletris-core::rng`; preserving distributions, not exact legacy random sequences. |
| Core piece-loop events are emitted from command/tick handling instead of rendering or networking code.                                                                                                                | `usr/src/game/BTGame.C:704-827`                                                                     | Implemented in `battletris-core::game` through movement, lock, line scoring, and spawn failure. |
| Session-level two-player events are logged deterministically for rendering, audio, networking, replay, and tests.                                                                                                    | `usr/src/game/BTGame.C:360-386`, `usr/src/game/BTGame.C:427-476`, `usr/src/game/BTGame.C:577-608`, `usr/src/game/BTGame.C:799-806` | Implemented in `battletris-core::game` with `TwoPlayerGame`, `BattleEvent`, and sequence-numbered `LoggedEvent` entries. |
| Full-line clearing computes funds as sum of cell values times number of cleared lines.                                                                                                                               | `usr/src/game/BTBoardManager.C:551-617`                                                             | Add scenario tests for single/double/triple/tetris with dice/happy.             |
| Score increases on fast-drop start by board height minus current y.                                                                                                                                                  | `usr/src/game/BTGame.C:716-732`                                                                     | Lines primarily drive funds and bazaar, not score.                              |
| New piece spawn failure sends game over.                                                                                                                                                                             | `usr/src/game/BTGame.C:799-806`                                                                     | Core should emit a loss event.                                                  |
| Bazaar trigger is every 20 combined lines.                                                                                                                                                                           | `usr/src/game/BTScoreManager.C:170-194`                                                             | Trigger should be derived from both player line totals.                         |
| Weapon durations decrement by target player line clears.                                                                                                                                                             | `usr/src/game/BTWeaponManager.C:137-149`                                                            | Do not model these as wall-clock durations.                                     |
| Weapon catalog order, names, descriptions, prices, and line durations are legacy data loaded from token/pricing/text files.                                                                                          | `usr/src/game/BTProtocol.H:79-115`, `usr/src/share/btweapons.db`, `usr/src/share/btweaponsp.db`      | Implemented in `battletris-core::weapons` as a stable catalog table.            |
| Arsenal shopping uses ten numbered slots, stacks identical weapons before the first hole, leaves holes after use, and stages bazaar purchases until both players are done.                                            | `usr/src/game/BTArsenal.C:26-56`, `usr/src/game/BTBazaar.C:305-367`                                  | Implemented in `battletris-core::weapons` and integrated into `TwoPlayerGame`.  |
| Zero-duration one-shot weapons apply immediately rather than living as expiring active flags. Swap exchanges boards; Rise up inserts one garbage line; Flip out mirrors horizontally; Missing removes one removable cell; Piece It/Bug add middle-half cells; Blind removes about half of removable cells; Keating transfers target funds; Reagan permits negative funds; Nice Day queues a happy; Susan swaps arsenals; Twilight hides existing cells; Gimp replaces removable cells preserving value. | `usr/src/game/BTBoardManager.C:158-236`, `usr/src/game/BTBoardManager.C:246-251`, `usr/src/game/BTBoardManager.C:298-400`, `usr/src/game/BTScoreManager.C:110-127`, `usr/src/game/BTWeaponManager.C:102-110` | Implemented in `battletris-core::board` and `battletris-core::game`; Mirror/incoming queue semantics remain Phase 10. |
| Carter Years doubles bazaar prices based on state captured when the bazaar opens.                                                                                                                                     | `usr/src/game/BTGame.C:577-593`, `usr/src/game/BTBazaar.C:393-431`                                   | `Bazaar` captures Carter pricing at session creation; activation is Phase 9.    |
| Timed weapons activate from arsenal launch, stack remaining line duration, apply through the final line clear before expiration, and restore probability/board hooks where legacy has an off path.                                                                                   | `usr/src/game/BTWeaponManager.C:114-149`, `usr/src/game/BTGame.C:547-570`, `usr/src/game/BTGame.C:654-661`, `usr/src/game/BTBoardManager.C:410-487` | Implemented in `battletris-core::weapons`, `battletris-core::game`, and `battletris-core::board`; Phase 10 adds Mirror, spy, and incoming FIFO core semantics. |
| Mirror reflects supported launches back onto a mirrored launcher and nullifies the legacy exception matrix after consuming the arsenal quantity.                                                                                                                                    | `usr/src/game/BTWeaponManager.C:191-219`                                                                                 | Implemented in `battletris-core::game` with explicit reflected/nullified launch events. |
| Spy weapons live on the launcher, produce deterministic recon board/funds snapshots after target placements, and use the William Ames/Ace/Condor visibility tiers.                                                                                                                   | `usr/src/game/BTRecon.C:54-118`, `usr/src/game/BTRecon.C:171-210`, `usr/src/game/BTGame.C:781-793`                       | Implemented in `battletris-core::recon` and emitted by `battletris-core::game`; client display and protocol payloads remain adapter work. |

## Client Presentation And Settings

Phase 13 adds the first Bevy client shell beyond the playfield. The client owns
screen navigation, local settings, theme selection, control layouts, and semantic
sound-event mapping while gameplay decisions remain in `battletris-core`.

| Area | Implementation note |
| --- | --- |
| Startup/menu flow | `battletris-client` exposes startup, challenge placeholder, settings, about placeholder, roster placeholder, sleep placeholder, and game screens through keyboard shortcuts. Challenge remains a placeholder until protocol/networking phases. |
| Local modes | Startup can launch local human-vs-human or unranked human-vs-computer. Computer play uses the deterministic core `ComputerOpponent` for placement commands and the core `HumanVsComputer` mode for unranked status. |
| Settings | Runtime settings cover theme, sound-pack selection, control layout, and pixel scale. Phase 18 persists these settings as TOML at `ProjectDirs::config_dir()/settings.toml` using the ADR 0005 app id, while gameplay state remains core-owned. |
| Themes/assets | The default theme is original scalable sprites; a high-contrast theme validates the swappable theme boundary without blocking on final art packs. Phase 18 packages source-controlled asset manifests under `assets/` next to the release binaries. |
| Audio | The client maps core `BattleEvent`/`CoreEvent` values to semantic `SoundEvent` categories such as menu action, line clear, bazaar, weapon launch, warning, and game over. The generated-default versus muted pack setting validates a swappable sound-pack boundary before recovered audio exists, and Phase 18 bundles a generated-default sound-pack manifest. |

## Distribution And Release

Phase 18 ships source-built GitHub Release archives for the v1 target platforms:
Linux, macOS, and Windows. `scripts/package-release.sh` builds release binaries
for the selected target, creates a `battletris-<version>-<target>.tar.gz`
archive under `dist/`, bundles `assets/`, documentation snapshots, `Cargo.lock`,
and a `release-manifest.toml`, then writes a SHA-256 sidecar when the host has a
hashing tool. `scripts/smoke-package.sh` unpacks an archive and checks the
minimum shippable layout: client/server/tools binaries, asset manifests, release
metadata, and docs.

Packaged assets live at `assets/` next to the archive README, and the client can
also use `BATTLETRIS_ASSETS_DIR` for development overrides. User settings follow
ADR 0005: `directories::ProjectDirs::from("org", "BattleTris", "BattleTris")`
with TOML settings at `config_dir()/settings.toml`; local player DB, user theme
packs, sound packs, and logs keep the paths selected by that ADR. Storefront and
package-manager distribution remains later scope after GitHub Releases are
validated.

## Protocol And Networking

### Legacy Protocol Facts

| Fact                                                                                                                | Source                                                                                                          | Rewrite note                                                 |
| ------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| Default server host/port are `poptart.eng.sun.com:4404`.                                                            | `usr/src/game/BTProtocol.H:11-12`                                                                               | Historical only; do not keep as default.                     |
| Packet framing is type plus length plus payload, both network byte order.                                           | `usr/src/sockets/PacketBuffer.H:39-45`, `usr/src/sockets/PacketBuffer.C:14-52`                                  | Use fixed-width `u32` framing and explicit message versions. |
| `BTToken` includes gameplay/ring, challenge/start, pause, and server/client DB tokens.                              | `usr/src/game/BTProtocol.H:14-77`                                                                               | Split local events from wire messages in Rust.               |
| `BTWeaponToken` order is the weapon catalog ABI.                                                                    | `usr/src/game/BTProtocol.H:79-115`                                                                              | Preserve token IDs for catalog compatibility.                |
| `BT_SCORE` comments are stale; implementation sends full `BTScore`, not a short delta.                              | `usr/src/game/BTProtocol.H:18`, `usr/src/game/BTCommManager.C:307-318`, `usr/src/game/BTScore.C:12-40`          | Implementation wins over comments.                           |
| `BT_CHALL` comments are stale; implementation sends `BTNetworkEntry`.                                               | `usr/src/game/BTProtocol.H:36`, `usr/src/game/BTNetManager.C:238-290`                                           | Capture real payloads in protocol tests.                     |
| Board snapshots carry motivation, height, width, and cell IDs.                                                      | `usr/src/game/BTCommManager.C:332-364`, `usr/src/game/BTBoard.C:14-36`                                          | Replace negative/invisible casts with typed cells.           |
| Arsenal snapshots carry length plus `(weaponToken, quantity)` pairs.                                                | `usr/src/game/BTCommManager.C:366-409`                                                                          | Use for Lazy Susan and protocol fixtures.                    |
| Remote weapons are queued, then flushed after current piece placement/scoring/recon and before next-piece creation. | `usr/src/game/BTCommManager.C:419-424`, `usr/src/game/BTCommManager.C:573-589`, `usr/src/game/BTGame.C:776-803` | Core FIFO and event ordering are implemented in `battletris-core::game`; Phase 14 adds protocol-owned launch/active/expired wire payloads without exposing local event enums. |

### Rust Protocol Boundary

Phase 14 implements `battletris-protocol` as a standalone wire boundary. Frames use the ADR 0003 16-byte big-endian envelope with magic `BTRS`, protocol version `1.0`, message kind, flags, and payload length. Payloads are postcard-encoded serde structs, and decoders validate magic, major version, maximum payload length, and exact frame length before deserializing.

The first public message set covers direct-connect foundations: hello/version advertisement, challenge, accept, deny, deterministic start, player input, board/score/arsenal snapshots, weapon launch/active/expired messages, bazaar done/state, game-over, pause/resume state, and graceful disconnect. These are intentionally protocol-owned types rather than `battletris-core::BattleEvent` values so local ring events such as bazaar start remain derived locally unless a later protocol ADR changes that boundary.

Phase 15 adds the first transport adapter in `battletris-protocol`: Tokio direct TCP frame I/O plus host/join helpers for hello, challenge, accept, deterministic start, scripted inputs, score snapshots, bazaar completion, game-over, and disconnect. LAN discovery remains best-effort metadata using `_battletris._tcp.local.` with TXT fields for protocol version, display name, port, and availability; manual direct connect is the required path. Integration tests run two headless clients over loopback and treat mismatched snapshots as desync evidence without introducing hosted authority or server-verified ranked play.

Phase 17 adds hosted/self-hosted protocol messages for lobby registration, lobby listing, server-issued game starts, ranked result claims, and result accept/reject responses. ADR 0007 selects a self-hosted lobby plus ranked-result authority: direct gameplay transport can remain peer-to-peer, while the server owns session ids, deterministic seeds, protocol-major admission, stale/completed session rejection, and ranked record writes. Ranked writes require both players to submit matching result claims for the same live session before `battletris-server` adapts the result into `battletris-db`.

### Legacy Message Groups

| Group                  | Tokens                                                                                                                                                                                                                       | Notes                                                                                                  |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Core/gameplay          | `BT_SCORE`, `BT_OP_SCORE`, `BT_LINE`, `BT_BOARD`, `BT_ARSENAL`, `BT_FUNDS`, `BT_WPN_ON`, `BT_WPN_LAUNCH`, `BT_WPN_OFF`, `BT_DEAD`, `BT_START_BAZ`, `BT_END_BAZ`, `BT_GAME_OVER`, `BT_AIRSLIDE`, `BT_LAWYER`                  | Some are local ring events, not peer wire messages; `BT_START_BAZ` is locally derived from score wrap. |
| Challenge/start        | `BT_PING`, `BT_BUSY`, `BT_CHALL`, `BT_ACCPT`, `BT_DENY`, `BT_START`                                                                                                                                                          | Used by direct peer challenge flow.                                                                    |
| Pause/misc             | `BT_PAUSE`, `BT_IDIOT`, `BT_CONDOR_OFF`                                                                                                                                                                                      | `BT_PAUSE` toggles; no separate unpause token.                                                         |
| Master/slave/client DB | `BT_LOCAL`, `BT_REMOTE`, `BT_COOKIE_GOOD`, `BT_COOKIE_BAD`, `BT_ACCEPTED`, `BT_REJECTED`, `BT_OBEY_ME`, `BT_I_OBEY`, `BT_NEWCLIENT`, `BT_CLIENTOK`, `BT_CLIENTBAD`, `BT_DISCONNECT`, `BT_HARIKARI`, `BT_QUER_*`, `BT_RESP_*` | Useful as functional flow reference, not a required topology.                                          |

## Player Records And Ranking

| Legacy source                            | Responsibility                                                                        | Rewrite target                                               |
| ---------------------------------------- | ------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| `usr/src/db/BTPlayer.*`                  | Player stats, display strings, serialization, rank updates, head-to-head aggregation. | `battletris-db::player`, `battletris-db::ranking`.           |
| `usr/src/db/BTPlayerRecord.*`            | Head-to-head record.                                                                  | `battletris-db::head_to_head`.                               |
| `usr/src/db/BTGameStats.*`               | Per-game result payload.                                                              | `battletris-core::GameSummary`, `battletris-db::GameResult`. |
| `usr/src/db/BTNetworkEntry.*`            | Online presence identity and challenge endpoint.                                      | `battletris-protocol::presence`, `battletris-server`.        |
| `usr/src/db/BTDB.*`, lock classes        | Persistent hash table and file locking.                                               | Replace; implement importer only if needed.                  |
| `usr/src/btref/*`, `usr/src/man/btref.1` | Referee/admin CLI for DB inspection, delete, clean, compress, stats.                  | `battletris-tools::admin` if ranked server is kept.          |

Stats concepts to preserve:

| Concept            | Notes                                                                                                                                                                                                            |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ranked wins/losses | Human network games update records; computer games are unranked.                                                                                                                                                 |
| Rank value         | Legacy starts at `BT_ELO_START` 1200 in `usr/src/game/BTConstants.H:26-28`; formula is integer arithmetic with `average_game_value = 5` in `usr/src/db/BTPlayer.C:56-73`.                                        |
| Streaks            | Legacy stores current streak count plus streak type, not separate longest/current win and loss streaks. Source: `usr/src/db/BTPlayer.H:43-44`, `usr/src/db/BTPlayer.C:604-611`, `usr/src/db/BTPlayer.C:715-722`. |
| Bests/records      | Best scores, line counts, funds, fastest kill/death, longest game.                                                                                                                                               |
| Head-to-head       | Per-opponent wins/losses and display in roster flow.                                                                                                                                                             |

Legacy DB shape inputs for the modern schema:

| Legacy record           | Fields to model                                                                                                                                                       | Source                                                                                                         |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Player identity/profile | Stable player key, display/gecos name, rank, wins, losses, high score, high lines, high funds, current streak count/type, fastest kill, quickest death, longest game. | `usr/src/db/BTDBRecord.H:15-23`, `usr/src/db/BTPlayer.H:35-47`, `usr/src/db/BTPlayer.C:467-488`                |
| Head-to-head record     | Player key, opponent key, wins against opponent, losses against opponent.                                                                                             | `usr/src/db/BTPlayerRecord.H:25-40`, `usr/src/db/BTPlayer.C:490-501`                                           |
| Game result summary     | Winner key/name, winner score/lines/funds, loser key/name, loser score/lines/funds, duration.                                                                         | `usr/src/db/BTGameStats.H:14-38`, `usr/src/db/BTGameStats.C:24-67`                                             |
| Network presence        | User name, host name, timestamp, process id, address network/local parts, port, max weapon count, protocol major/minor, status unknown/waiting/playing.               | `usr/src/db/BTNetworkEntry.H:21-60`, `usr/src/db/BTNetworkEntry.C:42-60`, `usr/src/db/BTNetworkEntry.C:99-120` |
| Legacy storage          | Hash database split across `.idx` and `.dat` files with binary record payloads. Use only as an optional import source, not as the new persistence model.              | `usr/src/db/BTDB.H:21-24`, `usr/src/db/BTDB.H:32-90`                                                           |

There are no known legacy DB files to migrate. The modern schema should be designed from these record shapes and migrations should start fresh. If old `.idx`/`.dat` files appear, build a one-time importer unless repeated import becomes a real requirement.

### Rust Persistence Boundary

Phase 16 implements `battletris-db` as a SQLite-backed fresh schema with embedded
`refinery` migrations. Player identity uses explicit BattleTris player ids plus
display names and community labels rather than Unix login, GECOS, or plan-file
lookups. The default community label is `local`; future hosted/self-hosted
deployments can select their own labels without creating a hard-coded global
ranking service.

Ranked human-vs-human results update both players in one transaction: legacy
starting rank `1200`, the legacy integer rank formula with average game value
`5`, wins/losses, current streak, best score/lines/funds, fastest kill,
quickest death, longest game, result history, and reciprocal head-to-head rows.
Computer games and explicitly unranked games do not mutate player records. ADR
0006 documents the compatibility choice to preserve ranking concepts while
fixing fresh-schema bugs such as named-player high-funds caps.

## Weapon Catalog

Catalog order is significant: `usr/src/game/BTProtocol.H:79-115` defines token IDs, `usr/src/share/btweapons.db` supplies names/descriptions, and `usr/src/share/btweaponsp.db` supplies price/duration pairs. The loader skips `#` lines and ignores a third pricing field in `usr/src/game/BTPimp.C:44-87`.

V1 ships every original weapon in this catalog.

Durations are measured in lines cleared by the affected player, not time.

|  Id | Token             | Name              | Price | Duration | Behavior notes                                                                                                                                                                                                                                          |
| --: | ----------------- | ----------------- | ----: | -------: | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
|   0 | `BT_FEARED_WEIRD` | The Feared Weird  |   400 |        3 | Enables weird disjointed pieces; restores normal probabilities on off. Source: `BTPieceManager.C:89-94`, `BTPiece.C:288-632`.                                                                                                                           |
|   1 | `BT_FOUR_BY_FOUR` | Four-by-Four      |   425 |       10 | Replaces normal square with hollow 4x4 piece. Source: `BTPieceManager.C:96-99`, `BTPiece.C:635-653`.                                                                                                                                                    |
|   2 | `BT_HATTER`       | The Mad Hatter    |   375 |        5 | 20ms timeout repeatedly rotates current piece. Computer ignores launch. Source: `BTGame.C:311-322`, `BTGame.C:554-556`, `BTWeaponManager.C:194-198`.                                                                                                    |
|   3 | `BT_UPBYSIDE`     | Upbyside-down     |   125 |       10 | Flips board vertically, reverses gravity and left/right movement, changes spawn to the bottom, flips back on off. Human rotation is not reversed. Source: `BTBoardManager.C:284-295`, `BTGame.C:547-552`, `BTGame.C:696-700`.                           |
|   4 | `BT_FALL_OUT`     | Fallout           |   250 |       10 | Middle columns fall out; pieces dropped into black hole disappear. Source: `BTBoardManager.H:71-85`, `BTBoardManager.C:410-420`.                                                                                                                        |
|   5 | `BT_SWAP`         | Swap meet         |  1200 |        0 | Swaps board snapshots; cancels Bottle/Upbyside; Mirror nullifies. Source: `BTGame.C:485-533`, `BTCommManager.C:448-453`.                                                                                                                                |
|   6 | `BT_LAWYERS`      | Lawyer's delite   |   350 |        5 | Each line cleared by launcher raises opponent. Source: `BTGame.C:477-482`, `BTGame.C:830-868`, `BTBoardManager.C:274-278`.                                                                                                                              |
|   7 | `BT_RISE_UP`      | Rise up           |    75 |        0 | Inserts one garbage line with one random hole. Source: `BTBoardManager.C:158-236`, `BTBoardManager.C:444-448`.                                                                                                                                          |
|   8 | `BT_FLIP_OUT`     | Flip out          |    15 |        0 | One-shot horizontal mirror of board contents. Computer ignores launch. Source: `BTBoardManager.C:246-251`, `BTWeaponManager.C:194-198`.                                                                                                                 |
|   9 | `BT_SPEEDY`       | Speedy Gonzales   |   275 |       10 | Halves target drop timeout; restored on off. Computer ignores launch. Source: `BTGame.C:563-566`, `BTGame.C:654-656`, `BTWeaponManager.C:194-198`.                                                                                                      |
|  10 | `BT_MISSING`      | Missing Pieces    |    50 |        0 | Picks a random start coordinate, then scans/wraps to remove the next removable occupied cell. Source: `BTBoardManager.C:326-355`.                                                                                                                       |
|  11 | `BT_PIECE_IT`     | Piece It Together |   100 |        0 | Randomly adds a visible block with x across the full board and y in the middle half; legacy code retries until it finds an empty cell. Source: `BTBoardManager.C:298-321`.                                                                              |
|  12 | `BT_BLIND`        | The Blind Cleric  |   400 |        0 | Code removes about half of removable blocks despite text saying region bomb. Source: `BTBoardManager.C:357-370`.                                                                                                                                        |
|  13 | `BT_MONDALE`      | Mondale '96       |   150 |       50 | Target keeps 70 percent of new funds; launcher receives 30 percent. Source: `BTScoreManager.C:106-108`, `BTScoreManager.C:154-163`.                                                                                                                     |
|  14 | `BT_KEATING`      | Keating Five      |   425 |        0 | Zeroes target funds and credits launcher from the launcher's cached opponent-funds value; Mirror nullifies. Source: `BTScoreManager.C:110-123`, `BTScoreManager.C:151-153`.                                                                             |
|  15 | `BT_CARTER`       | Carter Years      |   250 |       20 | Doubles target bazaar prices while active. Source: `BTGame.C:590-593`, `BTBazaar.C:393-395`, `BTBazaar.C:415-431`.                                                                                                                                      |
|  16 | `BT_REAGAN`       | Reagan Era        |   425 |        0 | One-shot multiplies target funds by -1. Source: `BTScoreManager.C:125-127`.                                                                                                                                                                             |
|  17 | `BT_AMES`         | William Ames      |    50 |       20 | Cheap spy: recon panel, 50 percent board-cell reporting, noisy funds. Source: `BTRecon.C:58-63`, `BTRecon.C:94-118`.                                                                                                                                    |
|  18 | `BT_ACE`          | Ace of Spies      |   100 |       30 | Better spy: 85 percent board reporting; funds usually exact, with a flake after opponent tetris clears. Source: `BTRecon.C:106-111`, `BTRecon.C:201-210`.                                                                                               |
|  19 | `BT_CONDOR`       | The Condor        |   225 |       40 | Accurate spy: 100 percent board/funds. `BT_CONDOR_OFF` is the shared recon cleanup token for spy expiration. Source: `BTRecon.C:112-114`, `BTRecon.C:188-210`, `BTGame.C:887-895`.                                                                      |
|  20 | `BT_NICE_DAY`     | Have a Nice Day   |    50 |        0 | Queues one happy piece worth 150 if cleared immediately; otherwise frowns. Source: `BTPieceManager.C:113-115`, `BTPiece.C:277-286`.                                                                                                                     |
|  21 | `BT_SO_LONG`      | So Long           |   100 |       10 | Deprives target of long pieces. Source: `BTPieceManager.C:109-111`, `BTPieceManager.C:142-144`.                                                                                                                                                         |
|  22 | `BT_NO_DICE`      | No Dice           |   600 |       35 | Deprives target of dice pieces. Source: `BTPieceManager.C:105-107`, `BTPieceManager.C:138-140`.                                                                                                                                                         |
|  23 | `BT_BUG`          | Bug Report        |   320 |        0 | Like Piece It Together, but invisible block. Source: `BTBoardManager.C:298-321`, `BTBox.C:188-203`.                                                                                                                                                     |
|  24 | `BT_BOTTLE`       | Bottle neck       |   150 |       10 | Builds non-removable 3-wide side walls over 8 rows, destroying overwritten side cells; expiration removes structures and does not restore overwritten blocks. Source: `BTBoardManager.H:12-13`, `BTBoardManager.C:423-441`, `BTBoardManager.C:471-487`. |
|  25 | `BT_NO_SLIDE`     | Slide Denied      |   125 |       10 | Removes BattleTris slide by reducing slide timeout to zero. Source: `BTGame.C:742-749`, `BTGame.C:297-309`.                                                                                                                                             |
|  26 | `BT_SUSAN`        | Lazy Susan        |   600 |        0 | Swaps arsenals via serialized arsenal exchange. Source: `BTWeaponManager.C:102-110`, `BTCommManager.C:366-407`.                                                                                                                                         |
|  27 | `BT_MEADOW`       | Meadow            |   475 |       10 | Doubles base and fast drop timeouts, effectively halving drop speed. Source: `BTGame.C:567-570`, `BTGame.C:658-661`.                                                                                                                                    |
|  28 | `BT_MIRROR`       | Mirror Mirror     |   500 |       10 | If active on the launching player, consumes the launched weapon, reflects most launches locally, and nullifies exceptions instead of sending them. Source: `BTWeaponManager.C:191-219`.                                                                 |
|  29 | `BT_TWILIGHT`     | The Twilight Zone |   450 |        0 | Makes existing bricks invisible; new bricks are not automatically hidden. Source: `BTBoardManager.C:390-400`, `BTBox.C:281-288`.                                                                                                                        |
|  30 | `BT_SLICK`        | Slick Willy       |   650 |        3 | Moves current piece endlessly left/right and reverses at blockage. Source: `BTGame.C:325-346`, `BTGame.C:558-560`.                                                                                                                                      |
|  31 | `BT_BROKEN`       | Broken Record     |   325 |        5 | Repeats previous piece with high probability unless happy is queued. Source: `BTPieceManager.C:101-103`, `BTPieceManager.C:184-209`.                                                                                                                    |
|  32 | `BT_FORCE`        | The Force         |   325 |        5 | Cleared line erases row without dropping rows above. Source: `BTBoardManager.C:94-101`, `BTBoardManager.C:584-586`.                                                                                                                                     |
|  33 | `BT_GIMP`         | The Gimp          |    25 |        0 | Replaces removable existing blocks with gimp pixmap boxes, preserving values; no restore. Source: `BTBoardManager.C:373-386`, `BTBox.C:237-247`.                                                                                                        |

Mirror nullification list from `usr/src/game/BTWeaponManager.C:204-219`: `BT_SWAP`, `BT_MONDALE`, `BT_KEATING`, `BT_AMES`, `BT_ACE`, `BT_CONDOR`, `BT_NICE_DAY`, `BT_SUSAN`, and `BT_MIRROR` are nullified rather than reflected.

## User Flows

### Startup

| Step | Legacy behavior                                                                                                                | Source                                                                                                        |
| ---: | ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------- |
|    1 | `BattleTris` initializes Xt/Motif, parses resources/options, installs fallback app-defaults, creates top-level shell.          | `usr/src/game/BattleTris.C:119-259`, `usr/src/game/BattleTris.C:528-545`, `usr/src/game/BattleTris.C:620-707` |
|    2 | Startup loads weapon catalog, creates communication/network managers, loads images, initializes sound manager, builds screens. | `usr/src/game/BTStartup.C:91-178`                                                                             |
|    3 | Startup screen shows Sleep, Challenge, About, Roster, Quit buttons.                                                            | `usr/src/game/BTStartup.C:223-273`                                                                            |
|    4 | `-X`/no-server computer mode can run without network.                                                                          | `README.md:47-48`, `usr/src/game/BattleTris.C:251-259`                                                        |

### Sleep Mode

| Step | Legacy behavior                                                                | Source                                                                                                     |
| ---: | ------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- |
|    1 | `-s` or resource sleep starts hidden/asleep.                                   | `usr/src/game/BattleTris.C:251-258`, `usr/src/man/BattleTris.1:100-103`                                    |
|    2 | Sleep hides startup and shows shaped Biff dialog while listener remains alive. | `usr/src/game/BTStartup.C:550-556`, `usr/src/game/BTBiff.C:55-86`                                          |
|    3 | Incoming challenge changes Biff/dialog state; ignored challenges time out.     | `usr/src/game/BTStartup.C:317-328`, `usr/src/game/BTStartup.C:349-370`, `usr/src/game/BTStartup.C:715-740` |

### Challenge, Accept, Deny, Start

| Step | Legacy behavior                                                                                                         | Source                                                                        |
| ---: | ----------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
|    1 | Client listens locally for peer challenges, registers a `BTNetworkEntry` with server, and requests connection DB.       | `usr/src/game/BTNetManager.C:121-170`                                         |
|    2 | Challenge screen lists players and can update server DB views.                                                          | `usr/src/game/BTChallenge.C:176-182`, `usr/src/game/BTChallenge.C:253-267`    |
|    3 | Challenger selects player, verifies server entry, connects directly to peer, sends `BT_CHALL` with local network entry. | `usr/src/game/BTChallenge.C:218-248`, `usr/src/game/BTNetManager.C:238-290`   |
|    4 | If peer accepts, challenger sends `BT_START`, shows game, and waits for peer `BT_START`.                                | `usr/src/game/BTNetManager.C:301-324`, `usr/src/game/BTCommManager.C:490-529` |
|    5 | Incoming challenge sends `BT_BUSY` if already busy; otherwise shows accept dialog and replies `BT_ACCPT` or `BT_DENY`.  | `usr/src/game/BTNetManager.C:438-572`                                         |
|    6 | Game starts on ring `BT_START`.                                                                                         | `usr/src/game/BTGame.C:454-476`, `usr/src/game/BTGame.C:870-879`              |

### Play Loop

| Step | Legacy behavior                                                                                                        | Source                                                                                                                |
| ---: | ---------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
|    1 | Drop timeout moves piece down/up depending on active effects; slide timeout handles BattleTris slide after landing.    | `usr/src/game/BTGame.C:284-309`                                                                                       |
|    2 | Player input moves, rotates, fast-drops, pauses, or launches weapons.                                                  | `usr/src/game/BattleTris.C:70-117`, `usr/src/game/BTGame.C:734-740`                                                   |
|    3 | Piece placement disposes piece into board, checks lines/funds, emits line/score/weapon events, then spawns next piece. | `usr/src/game/BTGame.C:765-828`, `usr/src/game/BTBoardManager.C:551-617`                                              |
|    4 | Score manager sends local score snapshot; comm manager serializes it as `BT_SCORE`; peer converts it to `BT_OP_SCORE`. | `usr/src/game/BTScoreManager.C:215-218`, `usr/src/game/BTCommManager.C:67-72`, `usr/src/game/BTCommManager.C:253-256` |
|    5 | Spawn failure sends `BT_GAME_OVER`.                                                                                    | `usr/src/game/BTGame.C:799-806`                                                                                       |

### Bazaar

| Step | Legacy behavior                                                                                                  | Source                                                                   |
| ---: | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
|    1 | Every 20 combined lines, score manager emits `BT_START_BAZ`.                                                     | `usr/src/game/BTScoreManager.C:170-194`                                  |
|    2 | Game pauses timeouts, stops stopwatch, resizes/shows bazaar, and applies Carter multiplier if active.            | `usr/src/game/BTGame.C:577-593`                                          |
|    3 | Player selects weapon, sees description/price/duration, buys/removes newly added weapons, and commits with Done. | `usr/src/game/BTBazaar.C:272-327`, `usr/src/game/BTBazaar.C:386-476`     |
|    4 | Done sends `BT_END_BAZ`; if opponent is not done, UI waits/dims.                                                 | `usr/src/game/BTGame.C:184-200`, `usr/src/game/BTGame.C:595-608`         |
|    5 | Both done calls `leaveBazaar` and resumes play.                                                                  | `usr/src/game/BTGame.C:427-446`                                          |
|    6 | Computer auto-shops and sends `BT_END_BAZ` after its timeout.                                                    | `usr/src/game/BTComputer.C:243-307`, `usr/src/game/BTComputer.C:549-677` |

### Arsenal Launch

| Step | Legacy behavior                                                             | Source                                                                                                      |
| ---: | --------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
|    1 | Number keys launch arsenal slot; `0` maps to slot 10.                       | `usr/src/game/BTGame.C:734-740`, `usr/src/game/BTWeaponManager.C:184-191`                                   |
|    2 | Local `BT_WPN_LAUNCH` becomes peer `BT_WPN_ON`.                             | `usr/src/game/BTCommManager.C:58-64`, `usr/src/game/BTCommManager.C:321-329`                                |
|    3 | Mirror may reflect or nullify before launch reaches opponent.               | `usr/src/game/BTWeaponManager.C:204-219`                                                                    |
|    4 | Receiver queues weapon and flushes after piece placement.                   | `usr/src/game/BTCommManager.C:419-424`, `usr/src/game/BTCommManager.C:573-581`, `usr/src/game/BTGame.C:795` |
|    5 | Timed weapons count down on line clear and emit `BT_WPN_OFF` at expiration. | `usr/src/game/BTWeaponManager.C:137-149`                                                                    |

### Pause And Disconnect

| Step | Legacy behavior                                                                    | Source                                                                                                                 |
| ---: | ---------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
|    1 | `p` toggles pause locally and sends `BT_PAUSE`.                                    | `usr/src/game/BattleTris.C:94-117`, `usr/src/game/BTGame.C:360-386`, `usr/src/game/BTCommManager.C:90-95`              |
|    2 | Receiving `BT_PAUSE` calls pause without echo. There is no separate unpause token. | `usr/src/game/BTCommManager.C:291-293`                                                                                 |
|    3 | Client shutdown sends server `BT_DISCONNECT`; slave revokes DB presence.           | `usr/src/game/BTNetManager.C:229-233`, `usr/src/daemons/BTSlave.C:341-362`                                             |
|    4 | Active peer error sends/receives `BT_ERR`, shows abort/crash state, and ends game. | `usr/src/game/BTCommManager.C:479-483`, `usr/src/game/BTCommManager.C:591-599`, `usr/src/game/BTCommManager.C:296-302` |

### Death, Game Over, Stats

| Step | Legacy behavior                                                                 | Source                                                                                                   |
| ---: | ------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
|    1 | Local death sends `BT_GAME_OVER`; comm manager converts loss to peer `BT_DEAD`. | `usr/src/game/BTGame.C:803-805`, `usr/src/game/BTCommManager.C:74-80`                                    |
|    2 | Peer receives `BT_DEAD`, marks win, and cleans up.                              | `usr/src/game/BTCommManager.C:277-282`, `usr/src/game/BTGame.C:616-631`, `usr/src/game/BTGame.C:887-911` |
|    3 | Startup timeout hides/resets game, updates network status, and records stats.   | `usr/src/game/BTStartup.C:530-548`, `usr/src/game/BTNetManager.C:384-417`                                |

### Roster And Player Records

| Step | Legacy behavior                                                                               | Source                                                                     |
| ---: | --------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
|    1 | Roster fetches player DB with `BT_QUER_PLYDB`; server responds with count and player records. | `usr/src/game/BTNetManager.C:758-831`, `usr/src/daemons/BTSlave.C:309-317` |
|    2 | Roster shows sortable player list and info panes.                                             | `usr/src/game/BTRoster.C:201-225`, `usr/src/game/BTRoster.C:234-270`       |
|    3 | Selecting players shows head-to-head records.                                                 | `usr/src/game/BTRoster.C:273-301`, `usr/src/man/BattleTris.1:142-152`      |

### Referee/Admin

| Step | Legacy behavior                                                     | Source                                                        |
| ---: | ------------------------------------------------------------------- | ------------------------------------------------------------- |
|    1 | `btref` opens network and player DBs directly.                      | `usr/src/btref/btref.C:53-91`                                 |
|    2 | Commands cover network list/data/delete/flush/cruft/clean/compress. | `usr/src/btref/btcmds.C:34-384`, `usr/src/man/btref.1:79-154` |
|    3 | Commands cover player data/list/delete/flush/compress/stats.        | `usr/src/btref/btcmds.C:386-719`                              |

## UI, Assets, And Audio

| Area           | Legacy facts                                                                                                                                    | Rewrite notes                                                                                   |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| Startup        | `BTStartup.C` loads images `btbazaar.ppm`, `btbiff1.ppm`, `btstartup2.ppm`, `btchalbiff.ppm`, `btsleepbiff.ppm`, `btgimp.ppm`, `btgimp2.ppm`.   | Convert or recreate as source asset pack; do not depend on PPM loader at runtime.               |
| App-defaults   | `usr/src/share/BattleTris.ad` defines fonts, colors, and layout resources for screens/dialogs.                                                  | Use as retro theme reference.                                                                   |
| Fonts          | `usr/src/game/bt_fonts.txt` lists legacy XLFD fonts.                                                                                            | Use as reference for default theme, not runtime dependency.                                     |
| Drawing        | `PPMReader.C` loads P3/P6 PPM to XImage; widgets use X pixmaps and shape masks.                                                                 | Bevy textures replace XImage/pixmap logic.                                                      |
| Sound          | `BTSoundManager.C` maps events to folder groups like welcome/start/near_death/tetris/won/lost/launched. `DevAudio.C` talks to Sun `/dev/audio`. | Define semantic sound events and generate default sounds. Original sounds can be optional pack. |
| Missing sounds | `usr/src/share/sounds/` is empty; README says original sounds have not been recovered.                                                          | Do not block rewrite on recovered audio.                                                        |

## Feature Matrix

This matrix maps original files or file groups to the rewrite module that should own their behavior.

| Legacy file/group                                                                    | Feature responsibility                                                                       | Rewrite module                                                                         |
| ------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `usr/src/game/BTConstants.H`                                                         | Version constants, board size, piece IDs, color/box IDs, timing defaults, arsenal size.      | `battletris-core`; selected constants mirrored in `battletris-protocol`.               |
| `usr/src/game/BTBox.*`                                                               | Cell identity, drawing metadata, removable/hidden/value state, happy/frown/gimp/dice values. | `battletris-core::cell`; rendering IDs mapped in `battletris-client`.                  |
| `usr/src/game/BTBoardManager.*`                                                      | Board mutation, collision, line clear, funds, board weapon effects.                          | `battletris-core::board`, `battletris-core::weapons::board_effects`.                   |
| `usr/src/game/BTBoard.*`                                                             | Serializable board snapshot.                                                                 | `battletris-protocol`; `battletris-core` snapshot type.                                |
| `usr/src/game/BTPiece.*`                                                             | Piece shapes, rotation, single-cell dice/happy, exotic pieces.                               | `battletris-core::piece`.                                                              |
| `usr/src/game/BTPieceManager.*`                                                      | Piece probabilities, weapon-modified generation, next piece state.                           | `battletris-core::piece_generator`.                                                    |
| `usr/src/game/BTGame.*`                                                              | Main game state machine, timers, placement, pause, bazaar transitions, weapon side effects.  | Split between `battletris-core::game` and `battletris-client`.                         |
| `usr/src/game/BTScore.*`, `BTScoreManager.*`                                         | Funds, lines, score text, bazaar trigger, economy effects.                                   | `battletris-core::score`, `battletris-core::economy`.                                  |
| `usr/src/game/BTWeapon.*`, `BTWeaponManager.*`                                       | Weapon model, arsenal display, launch/reflection/expiration.                                 | `battletris-core::weapons`; UI labels in `battletris-client`.                          |
| `usr/src/game/BTArsenal.*`                                                           | Ten-slot arsenal, quantities, serialization support.                                         | `battletris-core::arsenal`; `battletris-protocol` serialization.                       |
| `usr/src/game/BTPimp.*`, `usr/src/share/btweapons.db`, `usr/src/share/btweaponsp.db` | Weapon catalog load, name, description, price, duration.                                     | Static data in `battletris-core` or assets; extraction in `battletris-tools`.          |
| `usr/src/game/BTBazaar.*`                                                            | Bazaar UI and purchase/remove/done rules.                                                    | Rules in `battletris-core::bazaar`; UI in `battletris-client`.                         |
| `usr/src/game/BTRecon.*`                                                             | Spy board/funds panel and visibility behavior.                                               | Visibility rules in `battletris-core::recon`; panel in `battletris-client`.            |
| `usr/src/game/BTComputer.*`, `BTCBoard.*`, `BTMove.*`, `BTMovePath.*`                | Computer opponent, board evaluation, move search, shopping and launch strategy.              | `battletris-core::ai`.                                                                 |
| `usr/src/game/BTCommManager.*`                                                       | Peer gameplay packet send/receive, local ring bridge, queued weapons, errors.                | `battletris-protocol`; `battletris-client::net`.                                       |
| `usr/src/game/BTNetManager.*`, `BTChallenge.*`, `BTChallengeDialog.*`                | Server registration, challenge list, direct peer connect, accept/deny/start.                 | `battletris-protocol`, `battletris-client::matchmaking`, optional `battletris-server`. |
| `usr/src/game/BTRoster.*`                                                            | Player roster, sort/display, head-to-head UI.                                                | `battletris-client::roster`, `battletris-db`.                                          |
| `usr/src/game/BTStartup.*`, `BTAbout.*`, `BTBiff.*`, `BattleTris.C`, `BTFallbacks.C` | App lifecycle, startup/sleep/about screens, resources, key translations.                     | `battletris-client`.                                                                   |
| `usr/src/game/BTSoundManager.*`, `usr/src/audio/DevAudio.*`                          | Event sound lookup and Solaris audio playback.                                               | `battletris-client::audio`, `battletris-tools::audio_gen`.                             |
| `usr/src/game/PPMReader.*`, `usr/src/art/*`, `usr/src/share/BattleTris.ad`           | Legacy art/resource loading.                                                                 | `battletris-tools::assets`, client asset packs.                                        |
| `usr/src/game/BTProtocol.H`, `usr/src/sockets/*`                                     | Token IDs, packet framing, socket abstraction, Xt socket callbacks.                          | `battletris-protocol`; networking adapters.                                            |
| `usr/src/db/*`                                                                       | Persistent DB, records, locks, serialization.                                                | `battletris-db`; import tools if needed.                                               |
| `usr/src/daemons/*`                                                                  | Master/slave daemons, presence, DB gateway.                                                  | `battletris-server` if retained.                                                       |
| `usr/src/btref/*`, `usr/src/man/btref.1`                                             | Referee/admin CLI.                                                                           | `battletris-tools::admin`.                                                             |
| `usr/src/widget/*`                                                                   | Motif wrapper library.                                                                       | No direct port; client UI patterns only.                                               |
| `usr/src/stdlib/*`, `usr/src/signals/*`                                              | Custom containers and signal handling.                                                       | Replace with Rust std/crates.                                                          |

## Phase 1 Inputs

Phase 1 can start from these concrete contracts:

| Contract                                                | Needed artifact                                                                                                                           |
| ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Core crate must not depend on Bevy.                     | Workspace with `battletris-core` pure Rust tests.                                                                                         |
| Board and piece behavior must be deterministic.         | Fixtures for board dimensions, piece shapes, rotations, line clears, dice/happy funds.                                                    |
| Weapon catalog order is stable.                         | Static catalog generated from this spec or source DB extraction tests.                                                                    |
| Protocol must separate local events from wire messages. | `battletris-protocol` message enums with fixed-width serialization tests.                                                                 |
| Client renders emitted core events.                     | Bevy adapter subscribes to core state/events rather than owning rules.                                                                    |
| Player persistence is independent from Unix login.      | `battletris-db` identity ADR before ranking implementation.                                                                               |
| Platform-specific code stays isolated.                  | CI and adapter design should support Linux, macOS, and Windows as v1 targets.                                                             |
| Legacy source stays in place.                           | Add the Rust workspace alongside the existing tree, with workspace crates under `crates/` unless Phase 1 chooses a more idiomatic layout. |

## Remaining Open Questions For Later Phases

| Question                                                                                                                              | Proposed phase                |
| ------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------- |
| Should the fresh schema intentionally fix known legacy rank/stat update bugs, or preserve them for compatibility?                      | Phase 16 ADR or test decision. |
| How should server-local rankings discover or label a community's "main" server without hard-coding a global ranking service?          | Phase 16 decision.            |
| For MVP+ hosted/self-hosted play, should the server be authoritative, a relay with verification, or a lobby plus peer game transport? | Phase 17 ADR.                 |
