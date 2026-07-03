# ADR 0007: Self-Hosted Lobby And Ranked Authority

- Status: Accepted
- Date: 2026-07-02
- Phase: 17

## Context

Phase 15 proved friendly direct TCP play. Phase 16 added community-scoped
rankings, but direct peers are not a trustworthy authority for ranked writes.
Phase 17 needs a hosted or self-hosted path for discovery, session ownership,
disconnect/error handling, version skew, stale sessions, and result tampering.

Full NAT traversal, global matchmaking, and fully authoritative per-tick server
simulation are larger operational commitments than the MVP+ rewrite needs. The
deterministic core already makes it possible for peers to play a server-issued
session and later submit matching result claims.

## Decision

Phase 17 selects a self-hosted lobby plus server-verified ranked result model.
The server is authoritative for lobby sessions, protocol version admission,
player identities within its community label, deterministic match seeds, and
ranked record writes. The game transport can remain direct TCP for MVP+ play;
the server does not relay every gameplay frame or simulate every tick yet.

Ranked writes require both participants to submit matching result claims for the
same live session. The server rejects unknown sessions, stale/completed sessions,
protocol-major mismatches, wrong participants, non-matching claims, and unranked
or computer-style results. Once a session is recorded it cannot be recorded
again.

The protocol exposes hosted lobby/session/result messages as protocol-owned wire
types, separate from core gameplay events. The server crate owns session state
and adapts verified results into `battletris-db` records.

## Consequences

- Self-hosted communities can discover games and maintain their own rankings
  without a global account service.
- Direct gameplay remains usable while ranked authority moves to the server.
- Result tampering is reduced by dual-claim verification, but not eliminated
  against colluding clients or modified deterministic simulations.
- A later ADR can add relay transport, reconnect tokens, cryptographic identity,
  or fully authoritative tick validation without changing the core game rules.
