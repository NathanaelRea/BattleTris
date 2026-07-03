# BattleTris Context

BattleTris is a competitive two-player Tetris variant where cleared special pieces produce funds that buy weapons used to disrupt the opponent. This context defines the domain language used by planning docs, tests, and rewrite code.

## Language

### Gameplay

**BattleTris Game**:
A contest between exactly two players that ends when one player dies.
_Avoid_: Match, round

**Player**:
A participant who owns a board, controls pieces, earns funds, buys weapons, and can win or lose a BattleTris Game.
_Avoid_: User when referring to in-game behavior

**Opponent**:
The other player in the current BattleTris Game.
_Avoid_: Enemy, remote player when locality is irrelevant

**Computer Opponent**:
A deterministic non-human opponent that plays by the same core game rules and does not create ranked results.
_Avoid_: Bot when discussing preserved legacy mode

**Board**:
The rectangular playfield owned by one player.
_Avoid_: Grid when referring to the domain object

**Cell**:
One board square that may be empty or occupied by visible, invisible, gimp, structure, die, happy, or frown state.
_Avoid_: Box except when citing legacy `BTBox` code

**Piece**:
A falling arrangement of cells controlled by a player before it locks into the board.
_Avoid_: Brick, block when referring to the whole falling shape

**Die Piece**:
A one-cell special piece with a pip value from 1 to 6 that contributes funds when cleared in a line.
_Avoid_: Dice block

**Happy Piece**:
A one-cell special piece worth 150 funds only if it is cleared immediately after landing.
_Avoid_: Smiley when precision matters

**Frown**:
A missed happy cell that remains on the board without the happy-piece funds value.
_Avoid_: Unhappy unless citing legacy IDs

**Line Clear**:
The removal of one or more full board rows after a piece locks.
_Avoid_: Row clear unless discussing board coordinates

### Economy And Weapons

**Funds**:
The spendable in-game currency earned from clearing valued cells.
_Avoid_: Money, score, points

**Bazaar**:
The shopping state entered after the two players collectively clear the configured line threshold.
_Avoid_: Shop, store

**Weapon**:
A purchasable effect that a player launches to alter the opponent, the opponent's board, or related game state.
_Avoid_: Power-up, attack

**One-Shot Weapon**:
A weapon whose visible effect happens at launch time rather than expiring after future line clears.
_Avoid_: Instant weapon

**Timed Weapon**:
A weapon whose effect remains active until its line-duration expires.
_Avoid_: Time-based weapon

**Line Duration**:
The number of target-player line clears that a timed weapon lasts.
_Avoid_: Seconds, timer duration

**Active Effect**:
The current game-state consequence of a timed weapon that has not expired.
_Avoid_: Buff, debuff

**Arsenal**:
The player's ordered weapon inventory with numbered launch slots and stacked quantities.
_Avoid_: Inventory when referring to launch slots

**Weapon Launch**:
The act of consuming one arsenal quantity to apply or send a weapon effect.
_Avoid_: Fire, cast

**Recon**:
Weapon-driven visibility into the opponent's board or funds.
_Avoid_: Spy UI when referring to the game rule

### Records And Network Play

**Ranked Game**:
A human-vs-human game whose result updates player records under the selected ranking scope.
_Avoid_: Rated match

**Player Record**:
Persistent stats for one player, including wins, losses, rank value, streak, records, and head-to-head results.
_Avoid_: Account, profile when discussing stats only

**Community**:
A server or deployment scope that owns its own player records and rankings.
_Avoid_: Global ranking service

**Direct-Connect Game**:
A network game where clients connect without hosted lobby or relay infrastructure.
_Avoid_: P2P when the exact transport has not been chosen

**Hosted Play**:
Network play that depends on a lobby, relay, dedicated server, or server-verified ranking service.
_Avoid_: Internet play when authority and relay responsibilities matter

**Self-Hosted Lobby**:
A community-owned hosted play service that lists available games, issues session ids and seeds, and verifies ranked results without requiring a global service.
_Avoid_: Matchmaking when the service only lists joinable games

**Result Claim**:
A participant's server-submitted summary of a completed ranked game. Hosted ranked writes require matching claims from both players.
_Avoid_: Score report when the payload includes the full winner/loser result

## Relationships

- A **BattleTris Game** has exactly two **Players**.
- A **Player** owns exactly one **Board** and one **Arsenal** during a **BattleTris Game**.
- A **Board** contains **Cells**, and a **Piece** locks into occupied **Cells**.
- A **Line Clear** may award **Funds** from valued **Cells**.
- Combined **Line Clears** from both **Players** trigger the **Bazaar**.
- The **Bazaar** adds **Weapons** to each player's **Arsenal**.
- A **Weapon Launch** targets the **Opponent** unless a weapon-specific rule says otherwise.
- A **Timed Weapon** creates an **Active Effect** that expires by **Line Duration**.
- A **Computer Opponent** participates in a **BattleTris Game** but never creates a **Ranked Game**.
- A **Community** owns the scope for **Player Records** and rankings.
- A **Self-Hosted Lobby** owns hosted session discovery and server-verified **Result Claims** for its **Community**.

## Example Dialogue

> **Dev:** "When a player clears a line with a die cell, should we add points to score or funds?"
> **Domain expert:** "Funds. Score exists separately, but dice and happy cells are about buying weapons in the bazaar."
>
> **Dev:** "Does a timed weapon expire after seconds or after the opponent clears lines?"
> **Domain expert:** "Line duration. Weapon durations are measured in the target player's line clears."

## Flagged Ambiguities

- Legacy code uses "box" for what the rewrite should call a **Cell**; keep `BTBox` only in source citations.
- "Money" and "points" have both appeared informally; use **Funds** for spendable currency and score only for scoring.
- "Duration" is ambiguous; use **Line Duration** for weapon expiration and wall-clock timing only for input/drop behavior.
- "Server" can mean lobby, relay, dedicated authority, or community ranking host; name the exact responsibility when making networking decisions.
- "Rankings" are **Community** scoped by default, not global.
