# ADR 0004: Networking MVP Direct Connect And LAN Discovery

- Status: Accepted
- Date: 2026-07-01
- Phase: 14, 15

## Context

The rewrite should prove deterministic local network play before adding hosted
infrastructure, relay services, global identity, or server-verified ranking.
Phase 14 needs to answer whether Phase 15 ships direct IP only, LAN discovery,
or both.

LAN discovery is useful for preserving the original challenge/discovery feel,
but local network policy and firewalls can block discovery. A manual direct
connect path is still required for reliable testing and play.

## Decision

Phase 15 ships direct TCP connect as the required local network path and LAN
discovery as best-effort convenience. Discovery failure must not prevent play.

Use Tokio for TCP networking and Bevy integration through adapter resources or
channels. Use mDNS/DNS-SD only for discovery metadata.

Initial dependencies:

```toml
tokio = { version = "1.52.3", default-features = false, features = [
    "net",
    "io-util",
    "rt",
    "rt-multi-thread",
    "macros",
    "time",
    "sync",
] }
tokio-util = { version = "0.7.18", default-features = false, features = ["codec"] }
mdns-sd = { version = "0.20.1", default-features = false, features = ["async"] }
```

Advertise LAN games as `_battletris._tcp.local.` with TXT metadata for protocol
major/minor, display name, port, and availability state. The actual gameplay
transport is the same direct TCP protocol used by manual direct connect.

Do not introduce hosted authority, relay, NAT traversal, or server-verified
ranked play before Phase 17 unless a later ADR moves that scope earlier.

## Consequences

- Local network play is testable on loopback and works without discovery.
- LAN discovery can preserve a friendly challenge flow without becoming a hard
  dependency.
- NAT traversal and internet matchmaking remain out of the networking MVP.
- Friendly direct-connect games are not a trustworthy ranked authority; hosted
  or self-hosted ranked verification remains a later decision.
