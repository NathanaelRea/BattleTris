# Legacy Server Interop

This document captures the Phase 5 verification path for the Rust legacy roster listener. The Rust server is intended to replace original `btserverd` roster duties while gameplay remains direct peer-to-peer between clients.

## Automated Rust Smoke

Run the server on explicit loopback ports for local testing:

```sh
cargo run --bin server -- --modern-listen 127.0.0.1:4405 --legacy-listen 127.0.0.1:4404
```

In another shell, verify the Rust legacy client path against the Rust listener:

```sh
cargo run --bin tools -- net-smoke legacy-roster --server 127.0.0.1:4404 --share 127.0.0.1:4405
cargo run --bin tools -- net-smoke legacy-rust-interop --server 127.0.0.1:4404
```

`legacy-rust-interop` keeps two roster connections registered, confirms discovery and `BT_QUER_VERIFY`, performs a direct legacy challenge/start handshake between the two Rust peers, sends ignored result traffic to the roster listener, confirms the roster remains healthy, and disconnects both entries.

## Original C++ Manual Checks

Use a Rust server reachable by the original client:

```sh
cargo run --bin server -- --legacy-listen 0.0.0.0:4404
BattleTris -S SERVER_HOST -P 4404
```

Single-client roster check:

1. Start the Rust server.
2. Start one original C++ client with `BattleTris -S SERVER_HOST -P 4404`.
3. Confirm the server logs a legacy registration and roster query without malformed-packet errors.
4. Confirm the original client can refresh its network roster and see its own advertised entry.

Two-client direct-play check:

1. Start the Rust server on `0.0.0.0:4404`.
2. Start two original C++ clients pointed at the Rust server.
3. Confirm both clients appear in the original roster UI.
4. Challenge one client from the other.
5. Confirm the challenge is verified, accepted, and starts direct peer gameplay.
6. Finish a game and confirm the server logs ignored `BT_QUER_RESULT` traffic without dropping either roster connection.
7. Disconnect both clients and confirm the server removes both roster entries.

## Known Gaps

- Original C++ manual verification is not automated in this repository because the original binary, display server, and LAN topology are operator-provided.
- The Rust server intentionally does not persist legacy ranked results or legacy player DB records.
- The Rust server does not relay gameplay, bridge legacy to modern matchmaking, or add NAT traversal.
- Native `unsigned long` ABI compatibility still depends on the existing legacy codec tests matching the locally built original C++ client environment.
