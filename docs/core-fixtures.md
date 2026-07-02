# Core Fixture Conventions

These conventions keep deterministic `battletris-core` fixtures readable and
consistent across implementation phases. They are intentionally small. Add parser
features only when a phase needs them.

## Fixture Rules

- Keep fixtures ASCII and deterministic.
- Store fixtures under the owning crate, usually `crates/battletris-core/fixtures/`.
- Use top-to-bottom board rows. The top-left cell is `(x=0, y=0)`.
- Put legacy source references in fixture metadata when the fixture preserves a C++ behavior.
- Make all randomness explicit through a `GameSeed`, named RNG stream, or scripted random output.
- Prefer several tiny fixtures over one broad scenario.
- Do not encode Bevy, rendering, key events, sockets, or wall-clock behavior in core fixtures.

## Text Fixture Shape

Use TOML front matter delimited by `+++`, followed by one or more named text
sections. The existing `TextFixture` wrapper can hold these strings before a full
parser exists.

```text
+++
kind = "board"
name = "empty-legacy-board"
width = 10
height = 28
source = ["usr/src/game/BTConstants.H:89-90"]

[legend]
"." = "empty"
"X" = "visible"
+++
@board
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
..........
```

## Board Glyphs

Only `.` and `X` should be assumed by default. Define every special cell in the
fixture `legend` so future tests can represent values and lossy legacy IDs
without guessing.

Recommended meanings:

| Glyph | Meaning |
| --- | --- |
| `.` | Empty cell |
| `X` | Removable visible cell with value `0` |
| `S` | Non-removable structure cell |
| `1`..`6` | Die cell with that pip value |
| `H` | Happy cell worth `150` funds until missed |
| `F` | Frown cell worth `0` funds |
| `G` | Gimp cell. Include its preserved value in metadata if nonzero |
| `I` | Invisible Bug Report cell. Legacy board snapshots lose this as `0` |
| `T` | Hidden/Twilight cell. Legacy `id()` reports `-1` |

Board fixtures must state `width = 10` and `height = 28` unless they are parser
unit tests for invalid dimensions.

## Piece Fixtures

Piece fixtures should record local 8x8 maps, the legacy piece ID, `rot` width,
and expected spawn anchor. Use one `@orientation N` section per rotation state.

```text
+++
kind = "piece"
name = "legacy-el-piece"
id = 1
token = "BT_EL_PIECE"
rot = 3
spawn_anchor = [4, 0]
source = ["usr/src/game/BTPiece.C:185-194"]

[legend]
"." = "empty"
"X" = "visible"
+++
@orientation 0
........
.X......
.X......
.XX.....
........
........
........
........
```

Phase 3 should add exact fixtures for every legacy piece ID before depending on
rotation behavior in later gameplay tests.

## Scenario Fixtures

Use scenario fixtures for behavior that crosses modules, such as line clears,
funds, bazaar entry, incoming weapon timing, or AI decisions.

Recommended section names:

| Section | Purpose |
| --- | --- |
| `@player A board` | Initial board for player A |
| `@player B board` | Initial board for player B |
| `@commands` | Tick-indexed core commands or scripted placements |
| `@expected board A` | Expected final board for player A |
| `@expected board B` | Expected final board for player B |
| `@expected events` | Ordered deterministic core events |
| `@expected funds` | Player funds assertions when event text is too noisy |

Core event expectations should preserve tick order. Do not sort events for
snapshot convenience.

## Snapshot Fixtures

Board snapshot tests should include both a typed core snapshot and a legacy-ID
view. The legacy-ID view is intentionally lossy for at least one cell type:
Bug Report invisible cells serialize as `0`, the same value as empty cells.

Use separate fixtures for:

- Typed round trips that must preserve all core cell state.
- Legacy compatibility views that prove known lossy mappings and row order.

## RNG Fixtures

Use ADR 0002 for the seed model. Fixtures should name the deterministic stream
that supplies random choices, such as `piece_selection`, `dice_pip`, or
`weapon_effect`.

Do not serialize `rand_chacha` internal state in fixtures. Use seeds, scripted
inputs, and expected outputs.
