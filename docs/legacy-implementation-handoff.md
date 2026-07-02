# Legacy Implementation Handoff

This is the source-oriented companion to `plan-impl.md` and
`docs/external-research.md`. It records high-risk C++ behavior that implementation
agents are likely to miss.

Use it before implementing any phase that touches gameplay, protocol, AI, or
records.

## Agent Rules

- Read the phase in `plan-impl.md`, domain terms in `CONTEXT.md`, source facts in `docs/rewrite-spec.md`, fixture conventions in `docs/core-fixtures.md`, and this handoff.
- Treat C++ implementation as authoritative when comments, manpages, and weapon text disagree.
- Keep game rules in `battletris-core`; Bevy, sockets, persistence, assets, and platform APIs stay in adapter crates.
- Add deterministic tests or fixtures in the same phase that introduces compatibility-sensitive behavior.
- Update `docs/rewrite-spec.md` and `docs/traceability-checklist.md` when implementation resolves or changes a source fact.
- Run `./scripts/full-check.sh` before handing off changes.

## Current Phase State

Phase 1 is implemented in the current workspace: Cargo workspace, baseline crates,
crate-level docs, ADR 0001, GitHub Actions CI, and `./scripts/full-check.sh`
exist. The next gameplay agent should start at Phase 2 unless `plan-impl.md` says
otherwise.

## Phases 2 And 5: Boards, Cells, Lines, Funds

Primary sources: `usr/src/game/BTConstants.H`, `BTBox.*`, `BTBoard.*`,
`BTBoardManager.*`, `BTScoreManager.*`, and `BTGame.C`.

### Board Coordinates

| Fact | Source |
| --- | --- |
| Board is `10 x 28`. | `usr/src/game/BTConstants.H:89-90` |
| Board storage is `map_[x][y]`. | `usr/src/game/BTBoardManager.C:33-43` |
| Origin is top-left. Normal gravity increases `y`. | `usr/src/game/BTBoardManager.C:85-117` |
| Default occupancy treats out-of-bounds as occupied. | `usr/src/game/BTBoardManager.H:71-75` |
| Fallout later relaxes vertical bounds in middle columns. | `usr/src/game/BTBoardManager.H:77-85` |
| Default spawn constants are `x=5`, `y=0`; actual post-lock spawn subtracts `rot_/2` from x. | `usr/src/game/BTConstants.H:98-99`, `usr/src/game/BTGame.C:799-803` |

### Cell Compatibility

Use typed cells internally. Legacy IDs are a compatibility view, not the core data
model.

| Cell | Legacy behavior | Source |
| --- | --- | --- |
| Empty | Snapshot value `0`. | `usr/src/game/BTBoard.C:24-29` |
| Visible box | Removable, value `0`, `id()` reports color unless hidden. | `usr/src/game/BTBox.H:69-75` |
| Structure | ID `20`, not removable. | `usr/src/game/BTBox.H:121-134` |
| Happy | ID `21`, value `150` before missed. | `usr/src/game/BTConstants.H:57-58`, `BTBox.H:91-103` |
| Frown | ID `22`, value `0` after `landed()`. | `usr/src/game/BTBox.C:301-308` |
| Gimp | ID `23`, removable, preserves existing value. | `usr/src/game/BTBox.H:138-145`, `BTBoardManager.C:373-383` |
| Die | IDs `24..29`, values `1..6`. | `usr/src/game/BTConstants.H:61-66`, `BTBox.H:78-88` |
| Bug invisible cell | Occupied invisible cell whose legacy `id()` is `0`, so legacy snapshots lose it as empty. | `usr/src/game/BTBox.C:188-203` |
| Twilight hidden cell | Existing cell hidden with `hide()`, so legacy `id()` reports `-1`. | `usr/src/game/BTBox.H:72-74`, `BTBoardManager.C:390-400` |

### Board Snapshots

- `BTBoard` stores motivation, height, width, and row-major `rep_[y * width + x]`: `usr/src/game/BTBoard.H:23-36`, `usr/src/game/BTBoard.C:14-36`.
- Empty cells serialize as `0`; occupied cells serialize as `box->id()`: `usr/src/game/BTBoard.C:24-29`.
- Upside-down snapshots reverse row order only: `usr/src/game/BTBoard.C:18-23`.
- Applying a snapshot recreates nonzero IDs with `createByID`; `0` stays empty: `usr/src/game/BTBoardManager.C:627-638`, `usr/src/game/BTBox.C:249-264`.
- Modern protocol snapshots should validate dimensions and avoid legacy `unsigned long` ABI details.

### Line Clears And Funds

- `checkLines()` scans from bottom row upward: `usr/src/game/BTBoardManager.C:551-598`.
- A line is full when every column is occupied. Removability is not checked: `usr/src/game/BTBoardManager.C:574-584`.
- Normal clear shifts rows above down and clears row `0`: `usr/src/game/BTBoardManager.C:85-117`.
- Adjacent full rows rely on `j++` after removal so the same row index is rechecked after dropping: `usr/src/game/BTBoardManager.C:584-586`.
- Force erases the row without dropping rows above and skips the `j++` recheck: `usr/src/game/BTBoardManager.C:94-101`, `usr/src/game/BTBoardManager.C:584-586`.
- Funds are `sum(cleared cell values) * number_of_lines_cleared`: `usr/src/game/BTBoardManager.C:577-616`.
- `BT_FUNDS` is emitted before `BT_LINE`: `usr/src/game/BTBoardManager.C:613-615`.
- Happy cells in non-full rows become frowns while lines are scanned. Happy cells in cleared rows pay `150` and are removed: `usr/src/game/BTBoardManager.C:588-595`.

### Bazaar Trigger

- Threshold is `20` combined lines: `usr/src/game/BTScoreManager.C:14-15`.
- `lines_til_baz_` resets to `20`: `usr/src/game/BTScoreManager.C:25`, `usr/src/game/BTScoreManager.C:79-83`.
- Trigger is modulo wrap: recompute `20 - (local + opponent) % 20`; if the new value is greater than the old value, emit `BT_START_BAZ`: `usr/src/game/BTScoreManager.C:170-194`.
- `BT_START_BAZ` is a local ring event derived from scores. Do not require a peer wire message for it unless a later ADR changes the flow.

High-value tests: board bounds, snapshot row order, cell ID/value/removability,
Bug/Twilight lossy legacy views, single/double/triple/tetris funds, happy clear
versus frown conversion, Force no-drop clears, and bazaar wrap at `19+1`, `18+2`,
`19+4`, and `39+2`.

## Phases 3 And 4: Pieces, Rotation, Generation, Timers

Primary sources: `BTConstants.H`, `BTPiece.*`, `BTPieceManager.*`, `BTGame.C`,
and `BattleTris.C`.

### Piece Shape Table

`rot_` is both the rotation-square width and the spawn-centering width. `rot_ = 0`
means no rotation and no spawn-centering.

| ID | Token | `rot_` | Initial local cells | Source |
| ---: | --- | ---: | --- | --- |
| 1 | `BT_EL_PIECE` | 3 | `(1,0),(1,1),(1,2),(2,2)` | `usr/src/game/BTPiece.C:185-194` |
| 2 | `BT_REL_PIECE` | 3 | `(2,0),(2,1),(2,2),(1,2)` | `usr/src/game/BTPiece.C:196-205` |
| 3 | `BT_SL_RT_PIECE` | 3 | `(0,2),(1,2),(1,1),(2,1)` | `usr/src/game/BTPiece.C:218-227` |
| 4 | `BT_SL_LF_PIECE` | 3 | `(0,1),(1,1),(1,2),(2,2)` | `usr/src/game/BTPiece.C:207-216` |
| 5 | `BT_LONG_PIECE` | 4 | `(0,1),(1,1),(2,1),(3,1)` | `usr/src/game/BTPiece.C:229-240` |
| 6 | `BT_PLUG_PIECE` | 3 | `(0,2),(1,2),(1,1),(2,2)` | `usr/src/game/BTPiece.C:242-251` |
| 7 | `BT_BOX_PIECE` | 0 | `(1,1),(1,2),(2,1),(2,2)` | `usr/src/game/BTPiece.C:253-264` |
| 8 | `BT_DIE_PIECE` | 0 | `(1,1)`, pip `1..6` | `usr/src/game/BTPiece.C:266-275` |
| 9 | `BT_HAP_PIECE` | 0 | `(1,1)`, happy cell | `usr/src/game/BTPiece.C:277-286` |
| 10 | `BT_DOG_PIECE` | 3 | `(0,0),(1,1),(2,1),(2,2)` | `usr/src/game/BTPiece.C:288-297` |
| 11 | `BT_RDOG_PIECE` | 3 | `(0,1),(0,2),(1,1),(2,2)` | `usr/src/game/BTPiece.C:299-308` |
| 12 | `BT_CAP_PIECE` | 4 | `(0,2),(1,1),(2,1),(3,2)` | `usr/src/game/BTPiece.C:310-321` |
| 13 | `BT_WALL_PIECE` | 4 | `(0,1),(0,2),(3,1),(3,2)`, custom | `usr/src/game/BTPiece.C:323-426` |
| 14 | `BT_TOWER_PIECE` | 3 | `(2,0),(1,1),(0,1),(2,2)` | `usr/src/game/BTPiece.C:428-437` |
| 15 | `BT_STAR_PIECE` | 3 | `(1,0),(0,1),(1,2),(2,1)`, custom | `usr/src/game/BTPiece.C:439-485` |
| 16 | `BT_WLONG_PIECE` | 4 | `(1,0),(1,1),(2,2),(2,3)`, custom | `usr/src/game/BTPiece.C:487-632` |
| 17 | `BT_4x4_PIECE` | 0 | Hollow 4x4 border | `usr/src/game/BTPiece.C:635-653` |
| 18 | `BT_LONG_DONG_PIECE` | 8 | `(0..7,0)` | `usr/src/game/BTPiece.C:655-664` |

Post-lock spawn anchor is `x = 5 - rot_/2`, `y = 0`: `usr/src/game/BTGame.C:799-803`.
That means rot 3 spawns at x 4, rot 4 at x 3, rot 8 at x 1, and rot 0 at x 5.

### Rotation And Collision

- Standard forward rotation maps old `(ox, oy)` to new `(oy, rot - 1 - ox)`. Reverse maps old `(ox, oy)` to new `(rot - 1 - oy, ox)`: `usr/src/game/BTPiece.C:85-147`.
- There are no wall kicks. Any occupied or out-of-bounds destination aborts unchanged.
- `moveTo` and `canMoveTo` iterate mapped cells only, so empty margins of an 8x8 map can hang outside the board: `usr/src/game/BTPiece.C:45-80`.
- Wall, Star, and Weird Long have custom rotation state machines. Extract exact orientation fixtures before implementing behavior that depends on them.
- Star toggles between two states and ignores the reverse argument: `usr/src/game/BTPiece.C:439-485`.

### Piece Generation

- Default keep probabilities are normal IDs `1..7` at `.21`, die at `1`, happy at `.02`, long-dong at `.02`, and weird IDs otherwise `0`: `usr/src/game/BTPieceManager.C:16-37`.
- Happy queue has priority over Broken Record and decrements one queued happy per piece: `usr/src/game/BTPieceManager.C:179-217`.
- Broken Record repeats `old_piece_` 90 percent of the time unless a happy is queued: `usr/src/game/BTPieceManager.C:179-217`.
- Repeated die pieces reroll pips because construction creates a new die cell: `usr/src/game/BTPiece.C:266-275`.
- Weapon hooks mutate probability slots directly rather than recomputing from active effects: `usr/src/game/BTPieceManager.C:86-149`.
- Use ADR 0002 RNG streams. Do not preserve legacy `rand`, `drand48`, or `lrand48` sequences.

### Timers, Input, Lock Order

- Defaults are fast drop `10ms`, drop `512ms`, slide `150ms`: `usr/src/game/BTConstants.H:92-94`.
- Legacy keyboard mapping is `j/l/k`, space, `p`, `c`, and arsenal number keys: `usr/src/game/BattleTris.C:70-117`.
- Number key `0` launches arsenal slot 10: `usr/src/game/BTGame.C:734-740`.
- Failed drop starts the slide timer, not immediate lock: `usr/src/game/BTGame.C:752-763`.
- Slide expiry can move the piece down if it became possible before locking: `usr/src/game/BTGame.C:765-827`.
- Fast-drop score bump is `BT_BOARD_HGT - y_` and should happen once per fast-drop start: `usr/src/game/BTGame.C:716-732`.
- Lock order is dispose piece, check lines/funds, update score, flush idiot, send recon board, flush queued weapons, spawn next piece, then detect spawn failure: `usr/src/game/BTGame.C:776-806`.

High-value tests: piece IDs and shapes, spawn anchors, no-wall-kick rotation,
custom rotation cycles, collision with empty margins, generation probability hooks,
happy queue priority, Broken Record repeat behavior, die pip reroll, slide-to-lock,
fast-drop scoring, spawn failure, and queued weapon timing around next spawn.

## Phases 7 To 10: Weapons, Bazaar, Arsenal, Launch, Recon

Primary sources: `BTProtocol.H`, `BTWeaponManager.*`, `BTArsenal.*`, `BTBazaar.*`,
`BTPimp.*`, `BTBoardManager.*`, `BTScoreManager.*`, `BTCommManager.*`, and
`BTRecon.*`.

### Catalog And Bazaar

- Token order in `BTProtocol.H:79-115` is the stable catalog ABI. Bazaar display sorting by price is not ABI order.
- Names and descriptions are in `usr/src/share/btweapons.db`; prices and durations are in `usr/src/share/btweaponsp.db`.
- The catalog loader skips `#` lines and ignores a third pricing field: `usr/src/game/BTPimp.C:44-87`.
- Arsenal has ten slots. Slot labels are `1..9,0`, and `0` means slot 10: `usr/src/game/BTConstants.H:133-135`, `usr/src/game/BTWeaponManager.C:184-191`.
- Launch consumes one quantity before Mirror handling, including nullified Mirror exceptions: `usr/src/game/BTWeaponManager.C:193-222`.
- `useWeapon` leaves holes. It does not compact slots: `usr/src/game/BTArsenal.C:45-49`.
- `BTArsenal::buyWeapon` stops at the first empty slot before checking later slots, so a hole before an existing same weapon can duplicate a stack: `usr/src/game/BTArsenal.C:26-42`.
- Bazaar purchases are staged. Existing weapons cannot be removed; only newly added quantities can be refunded before commit: `usr/src/game/BTBazaar.C:305-318`, `usr/src/game/BTBazaar.C:451-476`.
- Funds and arsenal changes commit when the bazaar hides after both players are done: `usr/src/game/BTBazaar.C:346-367`.
- Carter price doubling is captured at bazaar entry: `usr/src/game/BTGame.C:577-593`, `usr/src/game/BTBazaar.C:393-431`.

### Weapon Effect Pitfalls

- Model zero-duration weapons as one-shot effects even though legacy active flags can remain set forever: `usr/src/game/BTWeaponManager.C:114-149`.
- Reagan multiplies target funds by `-1`; it does not clamp to zero: `usr/src/game/BTScoreManager.C:125-127`.
- Keating transfers the launcher's cached opponent-funds value later through score handling: `usr/src/game/BTScoreManager.C:92-123`, `usr/src/game/BTScoreManager.C:148-163`.
- Blind Cleric removes about half of all removable cells, despite weapon text describing a region bomb: `usr/src/game/BTBoardManager.C:357-370`.
- Twilight and Gimp affect existing cells only and have no undo path: `usr/src/game/BTBoardManager.C:373-400`.
- Timed durations decrement by target-player line clears, not wall-clock time: `usr/src/game/BTWeaponManager.C:137-149`.
- Same-weapon launches add remaining line duration: `usr/src/game/BTWeaponManager.C:114-119`.
- Final-line effects apply to the line clear that expires the weapon because score/funds processing precedes expiration.
- Speedy and Meadow restoration is not symmetric under stacked launches. Preserve visible behavior unless a balancing ADR changes it.

### Launch, Mirror, Incoming Queue

- Local launch path is number key, consume arsenal quantity, Mirror branch, then `BT_WPN_LAUNCH`: `usr/src/game/BTGame.C:734-740`, `usr/src/game/BTWeaponManager.C:184-224`.
- Peer receives launches as `BT_WPN_ON`, queues them, and flushes after current placement/scoring/recon and before next piece creation: `usr/src/game/BTCommManager.C:321-329`, `usr/src/game/BTCommManager.C:419-424`, `usr/src/game/BTCommManager.C:573-589`, `usr/src/game/BTGame.C:776-803`.
- Mirror active on the launching player consumes the weapon, reflects most launches locally, and nullifies these exceptions: `BT_SWAP`, `BT_MONDALE`, `BT_KEATING`, `BT_AMES`, `BT_ACE`, `BT_CONDOR`, `BT_NICE_DAY`, `BT_SUSAN`, and `BT_MIRROR`: `usr/src/game/BTWeaponManager.C:204-219`.
- Network Swap board snapshots are buffered and flushed after queued weapons: `usr/src/game/BTCommManager.C:448-453`, `usr/src/game/BTCommManager.C:573-588`.

### Recon

- Spy launch side effects live on the launcher: `usr/src/game/BTRecon.C:171-182`.
- Target sends recon board snapshots after each placement if active: `usr/src/game/BTGame.C:781-793`.
- Ames reports occupied cells with 50 percent probability, Ace with 85 percent, Condor with 100 percent: `usr/src/game/BTRecon.C:54-91`.
- Ames funds are noisy, Ace is usually exact with a tetris flake, and Condor is exact: `usr/src/game/BTRecon.C:94-118`, `usr/src/game/BTRecon.C:201-210`.
- `BT_CONDOR_OFF` is shared spy cleanup, not a Condor-only weapon event: `usr/src/game/BTProtocol.H:41-44`, `usr/src/game/BTRecon.C:188-199`.

High-value tests: exact catalog rows, arsenal holes and stacking, staged bazaar
commit/refund, Carter prices, every one-shot weapon, duration expiration on line
clears, final-line expiration, stacked/restoration quirks, incoming FIFO timing,
Mirror exception matrix, Swap buffering, and deterministic recon samples.

## Phase 11: Computer Opponent

Primary sources: `BTComputer.*`, `BTCBoard.*`, `BTMove.*`, and `BTMovePath.*`.

- Difficulty levels are a fixed delay/name table; level `0` is `Comatose` at `4000ms`, and the final level is `Bionic` with delay `0`: `usr/src/game/BTComputer.C:83-102`.
- Computer search evaluates reachable final placements from spawn and chooses the minimum penalty score: `usr/src/game/BTComputer.C:906-1122`, `usr/src/game/BTComputer.C:1202-1249`.
- Evaluation penalizes holes, covered holes, height, variance, and special active effects while rewarding line clears and immediate happy clears: `usr/src/game/BTCBoard.C:193-443`.
- Shopping buys toward combos, schedules launches by opponent/my/bazaar line gates, and auto-leaves bazaar after `3000ms`: `usr/src/game/BTComputer.C:549-677`.
- Susan is enabled by a scheduled order at opponent line `50` even though a nearby comment says `40`: `usr/src/game/BTComputer.C:177-200`.
- Computer mode remains unranked. The human challenge path is the only stats submission path: `usr/src/game/BTStartup.C:542-547`, `usr/src/game/BTStartup.C:642-650`.

High-value tests: deterministic move choice for fixed boards/seeds, evaluation
fixtures for holes and line clears, level-delay table, bazaar shopping choices,
scheduled launch gates, and exclusion from ranked result writes.

## Phases 14 And 15: Protocol And Direct-Connect Networking

Primary sources: `BTProtocol.H`, `BTCommManager.*`, `BTNetManager.*`, `BTRingNode.*`,
and ADRs 0003 and 0004.

- Legacy `BTToken` mixes local ring events, peer wire messages, challenge tokens, and server DB tokens. Do not make one Rust enum mean all of these: `usr/src/game/BTProtocol.H:14-77`.
- Local ring packets are in-process `{origin, token, data}` routed until they return to origin: `usr/src/game/BTRingNode.H:17-21`, `usr/src/game/BTRingNode.C:4-25`.
- Local `BT_SCORE` sends a full score snapshot; the remote receives it as local `BT_OP_SCORE`: `usr/src/game/BTCommManager.C:307-318`, `usr/src/game/BTCommManager.C:411-416`.
- Local `BT_WPN_LAUNCH` becomes peer wire `BT_WPN_ON`; receiver queues until flush.
- `BT_START_BAZ` is locally derived from score wrap and should not be sent as a required peer message.
- Local death sends peer `BT_DEAD`; peer maps that to a win: `usr/src/game/BTCommManager.C:74-80`, `usr/src/game/BTCommManager.C:277-282`.
- ADR 0003 selects fixed-width frame envelopes with postcard payloads.
- ADR 0004 selects required direct TCP connect plus optional LAN discovery; no hosted authority or ranked verification before Phase 17 unless a later ADR moves scope.

Phase 14 should classify every protocol type as one of: core event, presentation
event, peer wire message, challenge/discovery message, server/DB message, or
legacy-only reference.

## Phase 16: Persistence, Ranking, Identity

Primary sources: `usr/src/db/*`, `BTNetManager.*`, `BTStartup.*`, `BTGame.C`, and
ADR 0005.

- Model player key, display name, rank, wins/losses, high score/lines/funds, current streak count/type, fastest kill, quickest death, longest game, and head-to-head records: `usr/src/db/BTPlayer.H:35-51`, `usr/src/db/BTPlayerRecord.H:25-40`.
- Rank starts at `1200`; legacy math is integer/truncating with average game value `5`: `usr/src/game/BTConstants.H:26-28`, `usr/src/db/BTPlayer.C:56-73`.
- Legacy server processes winner rank before fetching loser, so winner rank is computed against default loser rank `1200`; loser rank then uses the updated winner rank: `usr/src/daemons/BTDBServer.C:157-190`.
- Legacy result stats can swap score/line/funds values when local challengee wins because `BTGame::cleanUp()` initially writes opponent as winner and local as loser: `usr/src/game/BTGame.C:887-902`, `usr/src/game/BTNetManager.C:384-405`.
- Same username results are suppressed: `usr/src/game/BTNetManager.C:384-390`.
- Legacy identity is Unix-login and host based. The rewrite should not bind identity to Unix login or GECOS: `usr/src/game/BTNetManager.C:72-127`, `usr/src/db/BTPlayer.C:82-103`.
- Direct-connect friendly games are not trustworthy ranked authority. Phase 16/17 must decide how local display identity, community identity, ranked writes, and hosted verification relate.

Recommendation for Phase 16: preserve ranking concepts but intentionally fix the
legacy rank/stat bugs in the fresh schema, with tests or an ADR documenting the
behavior change before ranked writes ship.
