# External Research Packets

Access date for all sources in this document: 2026-07-01.

This document records external crate, Bevy, platform, persistence, networking,
packaging, and test research for future implementation agents. It does not add
production dependencies or implement systems.

## Source Summary

| Area | Sources checked |
| --- | --- |
| Bevy baseline | `cargo info bevy` for `bevy 0.19.0`; docs.rs `bevy 0.19.0` feature page; docs.rs pages for `FixedUpdate`, `Fixed`, `States`, `MessageReader`, `ImagePlugin`, `Sprite`, `TextureAtlas`, `AudioPlayer`, and `PlaybackSettings`. |
| Serialization | `cargo info serde`, `postcard`, `bincode`, `wincode`, `bytes`; docs.rs `postcard 1.1.3`; docs.rs `bincode 3.0.0` unmaintained notice. |
| RNG and tests | `cargo info rand`, `rand_chacha`, `insta`, `proptest`, `toml`. |
| UI and input | `cargo info leafwing-input-manager`, `bevy_egui`; docs.rs feature pages for both crates. |
| Networking | `cargo info tokio`, `tokio-util`, `mdns-sd`; docs.rs `mdns-sd 0.20.1` feature page. |
| Persistence and paths | `cargo info rusqlite`, `refinery@0.9.2`, `directories`; docs.rs feature page for `rusqlite 0.40.1`; docs.rs `ProjectDirs` paths. |
| Audio and packaging | `cargo info hound`, `cargo-dist`; Bevy audio docs for `AudioPlayer` and `PlaybackSettings`. |

## Bevy Version And Feature Baseline

Question answered and phase unblocked: which Bevy version and feature baseline
should Phase 12 use for the desktop 2D client.

Recommendation: use `bevy = "0.19.0"` for Phase 12 if the workspace remains on
Rust `1.95.0` or newer. `bevy 0.19.0` was published 2026-06-19 and declares
`rust-version = 1.95.0`. Start `battletris-client` with `default-features =
false` and explicit desktop features rather than the full default set:

```toml
bevy = { version = "0.19.0", default-features = false, features = [
    "2d",
    "audio",
    "ui",
    "png",
    "wav",
    "vorbis",
    "x11",
    "wayland",
    "multi_threaded",
] }
```

Use `dynamic_linking` only behind a local developer feature, not for release
artifacts. It improves iterative compile/link cycles but complicates packaged
distribution and shared-library lookup.

Platform notes:

| Platform | Notes |
| --- | --- |
| Linux | Enable both `x11` and `wayland` for v1. Bevy's winit path should select the available session backend at runtime. Source builds may need normal graphics/windowing development packages. |
| macOS | Use the same Bevy feature set without platform-specific code in core. Verify Metal-backed renderer startup in packaged smoke tests. |
| Windows | Keep feature choices compatible with standard winit/wgpu desktop builds. Verify asset path and SQLite bundled behavior on Windows before release. |

Minimal smoke app checklist for Phase 12:

| Check | Expected result |
| --- | --- |
| Startup | `battletris-client` opens a resizable desktop window and exits cleanly. |
| Renderer | A `Camera2d` scene renders two test boards, text labels, and atlas sprites. |
| Filtering | `DefaultPlugins.set(ImagePlugin::default_nearest())` keeps pixel art crisp. |
| Input | Keyboard actions reach a Bevy adapter without mutating core state outside fixed ticks. |
| Audio | A generated `.wav` sound plays through `AudioPlayer` and `PlaybackSettings::DESPAWN`. |
| Packaging dry run | The app can locate assets from a configured asset root and from the development workspace. |

Rejected alternatives:

| Alternative | Reason not first choice |
| --- | --- |
| Bevy default features | Pulls unnecessary 3D/PBR/gltf scope for a 2D desktop client. |
| Older Bevy | Avoids a newer MSRV but increases migration work before implementation starts. |
| `dynamic_linking` in releases | Makes packaged artifacts fragile and platform-dependent. |

Risks and follow-up triggers: if CI or developer machines cannot move to Rust
`1.95.0`, pinning Bevy below `0.19.0` needs a new research pass. Re-check Bevy
release notes immediately before Phase 12 because Bevy APIs move quickly.

Documentation updates made: this packet, `docs/rewrite-spec.md`, and
`docs/traceability-checklist.md`.

## Bevy App Architecture Over Deterministic Core

Question answered and phase unblocked: how the Bevy client should consume core
state and events without moving rules into ECS systems.

Recommendation: keep `battletris-core` as the only owner of rules, tick
progression, deterministic event logs, and replay inputs. Bevy owns presentation,
input collection, asset handles, audio playback, UI state, and networking
adapters. The client should treat core events as ordered data returned from a
core tick, then mirror those events into Bevy messages only for rendering and
audio fan-out.

Use these Bevy APIs in Phase 12:

| Concern | API names |
| --- | --- |
| Fixed tick | `FixedUpdate`, `Time<Fixed>`, `Time::<Fixed>::from_hz`, `Update` |
| State machine | `#[derive(States)]`, `app.init_state::<ClientState>()`, `State<T>`, `NextState<T>`, `OnEnter`, `OnExit`, `in_state` |
| Presentation events | `#[derive(Message)]`, `MessageReader<T>`, `MessageWriter<T>` |
| Input | `ButtonInput<KeyCode>`, local action-map resource, per-tick command buffer |

Recommended schedule order:

| Schedule | Responsibility |
| --- | --- |
| `PreUpdate` | Poll keyboard/window/UI/network adapters and append timestamp-free actions to a pending input buffer. |
| `FixedUpdate` | Convert pending actions into tick commands, call `battletris-core`, append replay inputs, store the latest snapshot, and publish ordered presentation messages. |
| `Update` | Render from latest snapshot, play audio from presentation messages, and animate UI. No gameplay decisions here. |
| `OnEnter`/`OnExit` | Spawn/despawn screen entities and reset adapter-only resources. |

Event ordering notes: core returns `Vec<CoreEvent>` in deterministic order for a
single tick. The Bevy adapter must not merge events from different ticks before
audio, rendering, replay, or networking sees them. Network outbound messages
should be derived from the same tick result after the local core commit, not
from rendering components.

Rejected alternatives:

| Alternative | Reason not first choice |
| --- | --- |
| ECS owns rules | Makes replay, headless tests, protocol checks, and future server authority much harder. |
| Bevy messages as the replay log | Message retention and reader cursors are presentation mechanics, not a stable compatibility log. |
| Variable-frame game ticks in `Update` | Reintroduces frame-rate-dependent gameplay and desync risk. |

Risks and follow-up triggers: if Bevy `FixedUpdate` catch-up runs multiple ticks
in one rendered frame, the adapter must preserve tick boundaries in messages and
snapshots. If a future hosted server becomes authoritative, this schedule still
works because network commands already enter before fixed ticks.

Documentation updates made: this packet and `docs/traceability-checklist.md`.

## Bevy 2D Rendering, Scaling, And Theme Packs

Question answered and phase unblocked: how to render faithful scalable boards,
pieces, arsenal, funds, scores, and effects.

Recommendation: use Bevy sprite rendering with texture atlases for board cells,
pieces, weapon icons, and UI accents. Use Bevy UI/text for labels, menus, and
settings where it preserves the original-inspired visual language. Keep the core
board snapshot typed and render it through `battletris-client` adapter entities.

Use these Bevy APIs:

| Concern | API names |
| --- | --- |
| Pixel filtering | `ImagePlugin::default_nearest()` |
| Board/piece sprites | `Sprite`, `Sprite::from_atlas_image`, `TextureAtlas`, `TextureAtlasLayout` |
| Camera | `Camera2d`, orthographic scaling controlled by a client setting |
| Text | `Text`, `TextFont`, `TextColor`, `Text2d` when world-space labels are useful |

Theme pack proposal:

```text
assets/
  themes/
    original-inspired/
      theme.toml
      sprites/blocks.png
      sprites/ui.png
      fonts/default.ttf
  sounds/
    generated-default/
      sound-pack.toml
      line-clear.wav
```

`theme.toml` should describe atlas cell sizes, board dimensions, padding, font
paths, colors, and semantic sprite names. Settings should store the selected
theme pack id, integer scale, and pixel filtering preference. The original-style
default should use nearest filtering and integer board scaling; modern themes may
opt into linear filtering later.

Manual smoke checklist for Phase 12 and Phase 13:

| Check | Expected result |
| --- | --- |
| Integer scale | Board cells remain crisp at 1x, 2x, 3x, and resized windows. |
| High DPI | Text and sprites remain aligned on scaled monitors. |
| Theme reload | Missing optional theme assets fail with a clear error and do not panic mid-game. |
| Layout | Two boards, next piece, score, funds, line count, arsenal, and active effects remain visible at minimum window size. |

Rejected alternatives:

| Alternative | Reason not first choice |
| --- | --- |
| One mesh per board | Efficient later, but sprites and atlases are simpler to validate against legacy visuals first. |
| Hard-coded assets | Blocks theme packs, optional recovered assets, and release asset checks. |
| UI-only board rendering | Board cells are a game visualization, not ordinary UI layout. Sprites make atlas/pixel control clearer. |

Risks and follow-up triggers: if per-cell sprite counts become a performance
issue, replace board rendering internals with a mesh or chunked sprite strategy
without changing core snapshots or theme metadata.

Documentation updates made: this packet and `docs/traceability-checklist.md`.

## Bevy UI, Input Mapping, And Settings

Question answered and phase unblocked: which UI and input abstraction should the
client use for startup, challenge, bazaar, roster, settings, and game-over flows.

Recommendation: build the player-facing UI with Bevy UI plus custom sprites and
theme data. Do not use `bevy_egui` for v1 player screens because it adds a
different immediate-mode visual language and many default features. Reserve
`bevy_egui = "0.41.0"` for a future dev/debug overlay if needed.

Use a small local action map for v1 keyboard controls instead of adding
`leafwing-input-manager` immediately. BattleTris controls are simple,
compatibility-sensitive, and need deterministic per-tick capture. If future
gamepad/chord/remapping complexity grows, `leafwing-input-manager = "0.21.0"`
matches Bevy `0.19` and should be added with `default-features = false` and
only the needed features, starting with `keyboard`.

Settings persistence shape:

```toml
[video]
scale = 2
pixel_filter = "nearest"
theme = "original-inspired"

[audio]
sound_pack = "generated-default"
master_volume = 0.8

[controls.player1]
left = "KeyJ"
right = "KeyL"
rotate = "KeyK"
fast_drop = "Space"
pause = "KeyP"
arsenal_1 = "Digit1"

[network]
last_host = "127.0.0.1:4404"
lan_discovery = true
```

Use `directories = "6.0.0"` and `toml = "1.1.2+spec-1.1.0"` for settings when
Phase 13 adds persistence. Keep key names as Bevy `KeyCode` strings at the
adapter boundary, not in core commands.

Rejected alternatives:

| Alternative | Reason not first choice |
| --- | --- |
| `bevy_egui` for player screens | Fast for tools but visually mismatched for a themed game UI; defaults include clipboard, URL, picking, and render features. |
| `leafwing-input-manager` by default | Strong crate, but more abstraction than needed for keyboard-only compatibility controls. |
| Store raw platform scancodes | Less portable and harder for users to edit than semantic `KeyCode` strings. |

Risks and follow-up triggers: if Bevy UI focus handling consumes gameplay keys
while a text field or remapping dialog is active, the input adapter must expose
an explicit `GameplayInputEnabled` gate. Reconsider Leafwing when gamepad support
becomes a real v1 requirement.

Documentation updates made: this packet and `docs/traceability-checklist.md`.

## Bevy Audio And Generated Sound Packs

Question answered and phase unblocked: how to play and generate short sound
effects while keeping sound packs replaceable.

Recommendation: use Bevy audio for playback and generate default short `.wav`
effects from source-controlled Rust/tool configuration. Enable Bevy `wav` for
generated audio and keep `vorbis` for optional compressed/recovered packs.

Use these APIs and crates:

| Concern | Recommendation |
| --- | --- |
| Playback | Spawn `AudioPlayer::new(handle)` with `PlaybackSettings::DESPAWN` for one-shot effects. |
| Volume | Use `PlaybackSettings::with_volume` for initial volume and `AudioSink` for already-playing sounds. |
| Loops | Use `PlaybackSettings::LOOP` only for music/ambience if later added; gameplay effects are one-shot. |
| Generation | Use `hound = "3.5.1"` in `battletris-tools` to write deterministic PCM WAV files. |
| Mapping | Map semantic `SoundEvent` values to sound-pack file ids in `sound-pack.toml`. |

Initial semantic sound-event list: line clear, tetris, die funds, happy clear,
happy missed, bazaar enter, purchase, purchase denied, weapon launch, weapon
incoming, mirror, pause, unpause, death, game over, menu confirm, menu cancel.

Rejected alternatives:

| Alternative | Reason not first choice |
| --- | --- |
| Recovered original sounds as prerequisite | Original assets may be missing or unclear to redistribute; optional packs keep v1 unblocked. |
| Runtime sound synthesis | Adds latency and platform risk; deterministic generated files are simpler. |
| External audio engine | Bevy audio is sufficient for short effects and keeps client dependencies smaller. |

Risks and follow-up triggers: Bevy audio starts after asset load; short effects
must preload handles for gameplay-critical sounds. If latency is unacceptable in
manual smoke tests, run a small audio-specific spike before Phase 13 polish.

Documentation updates made: this packet and `docs/traceability-checklist.md`.

## Deterministic RNG And Test Fixture Dependencies

Question answered and phase unblocked: which RNG and test crates should support
deterministic piece generation, dice pips, fixtures, snapshots, and properties.

Recommendation: see ADR 0002. Use `rand = "0.10.1"` and `rand_chacha =
"0.10.0"` in `battletris-core` with OS/thread RNG features disabled. Use a
serializable `GameSeed` and deterministic named streams for piece selection,
dice pips, happy-piece queues, and weapon random effects. Do not serialize raw
RNG state for replay compatibility; serialize the seed, protocol/core version,
and tick-indexed player inputs.

Fixture and test stack:

| Need | Recommendation |
| --- | --- |
| Compact fixtures | Hand-written text fixtures under the owning core module. |
| Metadata | `toml = "1.1.2+spec-1.1.0"` as a dev/test dependency where structured metadata is useful. |
| Golden outputs | `insta = "1.48.0"` with reviewed snapshots for board/piece/event outputs. |
| Invariants | `proptest = "1.11.0"` for board invariants, serialization round trips, and protocol compatibility. |

Rejected alternatives:

| Alternative | Reason not first choice |
| --- | --- |
| Preserve legacy `rand`/`drand48` internals | Platform/library behavior is not the user-visible contract; distributions and outcomes are. |
| `thread_rng` or OS RNG | Breaks replayability and cross-platform deterministic tests. |
| Serialize `ChaCha` internal state as public data | Couples saves/replays to crate internals; seed plus inputs is the stable contract. |

Risks and follow-up triggers: changing RNG crate, stream partitioning, or seed
encoding after release breaks replay and fixture compatibility and requires an
ADR. If exact legacy random sequences become a product requirement later, add a
separate compatibility RNG rather than changing the default seed model silently.

Documentation updates made: this packet, ADR 0002, `docs/rust-workspace.md`, and
`docs/traceability-checklist.md`.

## Serialization And Protocol Framing

Question answered and phase unblocked: which encoding/framing strategy should
`battletris-protocol` use for a versioned BattleTris protocol.

Recommendation: see ADR 0003. Use a hand-written fixed-width big-endian frame
envelope and `postcard = "1.1.3"` for stable `serde` payloads. `postcard 1.1.3`
documents a stable wire format as of v1.0.0. Avoid `bincode`: `bincode 3.0.0`
is an unmaintained final release whose docs.rs page says the crate contains a
compiler error, and older `bincode 2.0.1` should not become a new long-term
wire contract.

Initial dependency recommendations:

```toml
serde = { version = "1.0.228", features = ["derive"] }
postcard = { version = "1.1.3", default-features = false, features = ["use-std"] }
bytes = "1.12.0"
```

Frame notes:

| Concern | Recommendation |
| --- | --- |
| Versioning | Use an initial 16-byte header: `magic: [u8; 4]`, `major: u16`, `minor: u16`, `kind: u16`, `flags: u16`, `payload_len: u32`. Major mismatch rejects during handshake; minor mismatch negotiates common capabilities. |
| Endian | Header uses big-endian integer fields. Payload encoding is delegated to postcard's documented format. |
| Length | Header includes `payload_len: u32`; enforce a small max payload before allocation. |
| Unknown messages | Unknown kind with compatible major can be skipped if length is valid and message is marked optional; otherwise close with protocol error. |
| Compatibility tests | Golden byte fixtures for every public message, cross-version decode tests, fuzz/property tests for length and unknown-message handling. |

Rejected alternatives:

| Alternative | Reason not first choice |
| --- | --- |
| Fully hand-written payload encoding | Maximum control but slower to evolve and more error-prone for many message shapes. |
| `bincode` | Latest release is unmaintained/unusable for new work; older versions should not anchor the contract. |
| `wincode = "0.5.5"` | Bincode-compatible fork but young and not needed when postcard has a stable spec. |
| `rkyv` | Zero-copy benefits do not matter for small protocol messages and add compatibility complexity. |

Risks and follow-up triggers: any change to frame fields, byte order, payload
format, or message discriminants requires a protocol ADR and compatibility
fixtures. If message bytes need to interoperate with non-Rust clients, add a
human-readable protocol spec before implementation ships.

Documentation updates made: this packet, ADR 0003, `docs/rust-workspace.md`,
`plan-impl.md`, and `docs/traceability-checklist.md`.

## Local Networking And LAN Discovery

Question answered and phase unblocked: what Phase 15 should ship for local
network play.

Recommendation: see ADR 0004. Phase 15 should support direct TCP connect as the
required path and LAN discovery as best-effort convenience. Discovery must not
be required to play. Hosted relay, lobby, NAT traversal, global identity, and
server-verified ranked play stay in Phase 17 or later.

Initial dependency recommendations:

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

Implementation notes:

| Concern | Recommendation |
| --- | --- |
| Transport | TCP for ordered reliable challenge/start/input/bazaar/game-over messages. |
| Framing | `tokio_util::codec` over the ADR 0003 frame envelope. |
| Bevy integration | Run networking in async tasks and exchange adapter messages with Bevy resources/channels before `FixedUpdate`. |
| Discovery | Advertise `_battletris._tcp.local.` with TXT protocol major/minor, display name, port, and state. |
| Desync | Exchange deterministic seed, tick inputs, periodic state hashes, and explicit desync reports. |
| Disconnect | Surface remote disconnect as a core/client event; do not silently continue ranked games. |

Headless two-client integration test strategy: run two protocol endpoints on
loopback, complete challenge/accept/start, exchange a fixed seed and scripted
inputs, force bazaar done messages, assert matching state hashes, then test
disconnect and major-version rejection.

Rejected alternatives:

| Alternative | Reason not first choice |
| --- | --- |
| UDP first | BattleTris messages are small and ordered; reliability work would distract from deterministic sync. |
| Internet NAT traversal in Phase 15 | Product scope says hosted/relay/lobby is MVP+. |
| LAN discovery only | mDNS can be blocked by network policy; direct IP must always work. |
| Server authority before hosted play | More architecture and operations work before deterministic peer sync is proven. |

Risks and follow-up triggers: mDNS behavior varies by OS/firewall. If discovery
is flaky, keep it optional and do not delay direct-connect MVP. If ranked trust
becomes a v1 requirement, Phase 17 authority decisions must move earlier via
ADR.

Documentation updates made: this packet, ADR 0004, `plan-impl.md`,
`docs/rust-workspace.md`, and `docs/traceability-checklist.md`.

## Persistence Backend And Cross-Platform Paths

Question answered and phase unblocked: which backend and paths should support
player records, rankings, settings, migrations, and future server use.

Recommendation: see ADR 0005. Use SQLite through `rusqlite = "0.40.1"` with
the `bundled` feature for consistent desktop builds, `refinery = "0.9.2"` for
schema migrations, `directories = "6.0.0"` for OS paths, and TOML settings files
for user-editable client settings.

Initial dependency recommendations:

```toml
rusqlite = { version = "0.40.1", default-features = false, features = ["bundled"] }
refinery = { version = "0.9.2", default-features = false, features = ["rusqlite-bundled"] }
directories = "6.0.0"
toml = "1.1.2+spec-1.1.0"
```

Use `directories::ProjectDirs::from("org", "BattleTris", "BattleTris")` unless
a later product decision chooses a different organization/app id.

Path shape:

| Data | Location |
| --- | --- |
| User settings | `ProjectDirs::config_dir()/settings.toml` |
| Local player DB | `ProjectDirs::data_dir()/battletris.sqlite3` |
| User theme packs | `ProjectDirs::data_dir()/themes/` |
| User sound packs | `ProjectDirs::data_dir()/sounds/` |
| Logs | `ProjectDirs::state_dir()/logs/` on Linux, otherwise `data_local_dir()/logs/` |
| Packaged assets | Next to executable under `assets/`, with environment/CLI override for development. |
| Server data | Explicit configured directory, defaulting to the same project data root for local self-hosting. |

Migration test strategy: every migration runs against an empty temp DB and a DB
at the previous schema version; ranking/stat update tests use transactions and
in-memory SQLite where possible; file-path tests assert the project path is
requested through `ProjectDirs` and do not bake user home directories.

Rejected alternatives:

| Alternative | Reason not first choice |
| --- | --- |
| Legacy hash DB | Useful only for optional import if real data appears; not a maintainable v1 backend. |
| JSON/TOML player records | Easy initially but weak for head-to-head queries, migrations, and future server use. |
| `sled`/`redb` | Embedded KV stores are less natural for ranking queries and schema migrations. |
| System SQLite only | Smaller builds but more platform variance for packaged releases. |

Risks and follow-up triggers: `rusqlite` bundled builds compile SQLite and can
increase build time. If distribution policy later requires system SQLite, record
that as a packaging ADR. Identity and ranked trust scope still need the Phase 16
ADR before implementation updates records.

Documentation updates made: this packet, ADR 0005, `docs/rust-workspace.md`,
and `docs/traceability-checklist.md`.

## Packaging, Assets, And Release Tooling

Question answered and phase unblocked: what first release artifacts and asset
lookup rules should Phase 18 target.

Recommendation: keep source builds as the baseline and use GitHub Releases as
the first packaged distribution channel. When packaging begins, evaluate
`cargo-dist = "0.32.0"` for producing per-platform archives and installers. Do
not add packaging tooling before the client has real assets and a runnable binary.

First release artifact shape:

```text
battletris-<version>-<target>/
  battletris-client(.exe)
  assets/
    themes/original-inspired/...
    sounds/generated-default/...
  README.md
  LICENSES/
```

Runtime asset lookup order:

| Priority | Source |
| --- | --- |
| 1 | Explicit CLI flag or `BATTLETRIS_ASSET_DIR` for development and tests. |
| 2 | `assets/` adjacent to the executable for release archives. |
| 3 | `ProjectDirs::data_dir()` user asset packs. |
| 4 | Workspace `assets/` path in debug builds only. |

CI/release notes for Phase 18: build Linux, macOS, and Windows release artifacts;
run `./scripts/full-check.sh`; run packaged smoke tests that launch the client,
load default assets, load default sound pack metadata, create/read settings, and
exit cleanly. Keep Bevy `dynamic_linking` off in packaged builds.

Rejected alternatives:

| Alternative | Reason not first choice |
| --- | --- |
| Package-manager/storefront distribution first | Premature before GitHub Releases prove artifact layout and smoke tests. |
| Embed all assets into the binary | Makes theme/sound replacement and recovered-original packs harder. |
| Rely only on current working directory | Fragile for desktop launchers and packaged archives. |

Risks and follow-up triggers: macOS signing/notarization and Windows installer
polish can become separate Phase 18 tasks. If assets contain recovered original
material, licensing must be resolved before release packaging.

Documentation updates made: this packet, `docs/rewrite-spec.md`, and
`docs/traceability-checklist.md`.
