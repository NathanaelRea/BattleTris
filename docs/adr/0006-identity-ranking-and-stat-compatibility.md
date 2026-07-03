# ADR 0006: Identity, Ranking, And Stat Compatibility

- Status: Accepted
- Date: 2026-07-02
- Phase: 16

## Context

The legacy player DB tied player identity to Unix login names and derived display
names from GECOS and optional `.battletris` plan files. Legacy stat updates also
contained named-player high-funds caps for three Brown-era accounts. The Rust
rewrite needs persistent local records without requiring Unix accounts, and Phase
17 may add hosted or self-hosted communities with their own ranking scopes.

## Decision

Use explicit BattleTris player ids as the stable key in the fresh schema. A
record also carries a display name and a community label. V1 local play uses the
`local` community label by default; hosted/self-hosted deployments can choose
their own label later without hard-coding a global ranking service.

Preserve the legacy rank concept and integer rank formula using the original
starting value `1200` and average game value `5`. Preserve the record categories:
wins, losses, current streak, best score, best lines, best funds, fastest kill,
quickest death, longest game, and head-to-head wins/losses.

Intentionally do not preserve the Unix/GECOS/plan-file identity coupling or the
three named-player high-funds caps. Those were environment-specific bugs/jokes in
the original deployment, not product rules. Fresh-schema ranked writes use normal
max-record behavior for every player.

Computer games remain unranked. Explicitly unranked human games also do not
mutate player records.

## Consequences

- Local records work on Linux, macOS, and Windows without accounts or passwd DB
  lookups.
- Rankings are inspectable by community/server label but remain local-trust until
  Phase 17 chooses a hosted authority model.
- Compatibility keeps the player-record concepts and rank feel while fixing
  legacy stat bugs before any new ranked writes ship.
