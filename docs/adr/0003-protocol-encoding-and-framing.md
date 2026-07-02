# ADR 0003: Protocol Encoding And Framing

- Status: Accepted
- Date: 2026-07-01
- Phase: 14

## Context

The legacy protocol uses C++ structs and stale comments in places. The rewrite
needs a portable protocol for direct-connect play, deterministic sync, scripted
protocol tests, and later hosted/self-hosted play. The frame shape and payload
encoding become a long-term compatibility contract once network play ships.

External research found that `bincode 3.0.0` is an unmaintained final release
whose docs.rs page says the crate contains only a compiler error. Older
`bincode 2.0.1` still exists, but it should not anchor a new wire contract.
`postcard 1.1.3` documents a stable wire format as of postcard v1.0.0.

## Decision

`battletris-protocol` owns a hand-written fixed-width frame envelope using
big-endian integer fields. Payloads are encoded with `postcard = "1.1.3"` and
`serde = "1.0.228"` derives. Use `bytes = "1.12.0"` for frame buffers and codec
implementation.

Initial dependencies:

```toml
serde = { version = "1.0.228", features = ["derive"] }
postcard = { version = "1.1.3", default-features = false, features = ["use-std"] }
bytes = "1.12.0"
```

The initial frame header is 16 bytes: `magic: [u8; 4]`, `major: u16`,
`minor: u16`, `kind: u16`, `flags: u16`, and `payload_len: u32`. Decoders must
validate the magic, reject unsupported major versions during handshake, enforce a
maximum payload length before allocation, and either skip or reject unknown
message kinds based on capability negotiation.

Every public message needs a golden byte fixture. Protocol tests should cover
round trips, unknown-message handling, length-limit rejection, version mismatch,
and representative challenge/start/input/bazaar/game-over flows.

## Consequences

- The frame envelope remains inspectable and independent of any serializer.
- Postcard payloads reduce hand-written encoding work while keeping a stable
  documented format.
- Non-Rust clients will need the frame spec plus the postcard payload schema.
- Changing the frame envelope, payload format, byte order, or message
  discriminants after release requires a protocol ADR and compatibility tests.
