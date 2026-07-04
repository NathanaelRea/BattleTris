# Distribution

BattleTris releases start as source-built GitHub Release archives. Each archive
contains the desktop client, optional server/tools binaries, bundled asset
manifests, release metadata, and documentation snapshots for one target platform.

## Local Packaging

Run `./scripts/package-release.sh` to build the host release package. Pass a Rust
different installed target.

Run `./scripts/smoke-package.sh dist/<archive>.tar.gz` to verify the archive
layout before publishing. The package smoke is headless: it checks server help,
`battletris-tools net-smoke --help`, a direct loopback peer script, and a
loopback self-hosted lobby register/list/join/status script without opening the
Bevy client.

Run `./scripts/package-check.sh` to match the CI package job locally: it builds
the host release archive, then runs package smoke against that archive.

## Direct IP Multiplayer

Manual Direct IP is the required multiplayer path. Gameplay uses direct TCP
between the two players; the lobby server is not a gameplay relay.

LAN example:

1. The host opens the Rust client and chooses a direct host bind address such as
   `0.0.0.0:4405`.
2. The host shares a reachable address for the same listener, such as
   `192.168.1.23:4405`.
3. The host allows inbound TCP on the direct gameplay port in the local firewall.
4. The joiner chooses Direct Join and enters the host share address.

`0.0.0.0` is only a bind address. Do not give it to another player as a join or
share address. If the client suggests the wrong LAN address, replace it with the
address that the joiner can actually reach.

Direct IP troubleshooting checklist:

1. Confirm both players are using the same BattleTris protocol version.
2. Confirm the host is still listening and has not already accepted a peer.
3. Confirm the joiner entered the share address, not `0.0.0.0` or `localhost`.
4. Confirm the host firewall allows inbound TCP on the chosen direct port.
5. Confirm the joiner can route to the host address. Home-router NAT, guest Wi-Fi
   isolation, VPN split routing, and carrier NAT can all block direct reachability.

Support checklist for common failures:

1. `Host bind failed: address already in use` means another host is using the
   port. Cancel the old host or choose a different port.
2. `Join timed out` means the joiner could not complete the direct TCP/handshake
   path. Recheck the host share address, firewall, and Wi-Fi/VPN routing.
3. Do not join `0.0.0.0` or `127.0.0.1` from another machine. Use the host LAN
   IP, for example `192.168.1.23:4405`.
4. `Challenge denied` is a host decision, not a transport failure. The joiner can
   retry after the host listens again.
5. `Peer disconnected`, `peer idle timeout`, or `Desync detected` end the online
   game intentionally. Return to Challenge/Sleep and start a new session.

## Self-Hosted Lobby

The optional lobby provides community presence, server-issued hosted session
metadata, and ranked-result authority. It still does not relay gameplay frames.
After the lobby starts a hosted session, the joiner connects directly to the
host's advertised gameplay address.

Operator example:

```sh
battletris-server --listen 0.0.0.0:4404 --community garage
```

Client setup:

1. Set Lobby Address to the server's reachable address, for example
   `192.168.1.10:4404`.
2. The host still needs a direct gameplay bind/share address, for example bind
   `0.0.0.0:4405` and share `192.168.1.23:4405`.
3. The joiner starts the hosted lobby session, receives server-owned start
   metadata, then connects to the host's direct gameplay address.
4. The host accepts only hosted direct challenges that match the server-issued
   session metadata.

Ranked trust limits:

1. Ranked writes require a server-issued hosted session and matching Result
   Claims from both participants.
2. The server verifies that both claims match before recording the result.
3. The model prevents accidental single-client writes, but it does not provide
   anti-cheat, cryptographic identity, or protection against colluding modified
   clients.

Hosted lobby troubleshooting checklist:

1. `Lobby server unavailable` does not block Direct IP. Verify the server is
   running and that clients use the server's reachable address, not `0.0.0.0`.
2. A hosted game still needs the host direct share address to be reachable by the
   joiner. The lobby is presence and authority, not a gameplay relay.
3. `Ranked result pending` means the server accepted the first claim and is
   waiting for the peer. `Ranked result rejected` means the server rejected the
   claim; record the reason from the client status text.

## Networking Limitations

Current release expectations:

1. No NAT traversal.
2. No internet relay or gameplay proxy.
3. No authoritative per-tick gameplay server.
4. No anti-cheat.
5. LAN discovery is best effort; manual Direct IP remains supported when
   discovery fails or is unavailable.

## User Paths

The client follows ADR 0005 and uses
`directories::ProjectDirs::from("org", "BattleTris", "BattleTris")`:

| Data | Location |
| --- | --- |
| Settings | `config_dir()/settings.toml` |
| Local player DB | `data_dir()/battletris.sqlite3` |
| User theme packs | `data_dir()/themes/` |
| User sound packs | `data_dir()/sounds/` |
| Logs | `state_dir()/logs/` on Linux, otherwise `data_local_dir()/logs/` |
| Packaged assets | `assets/` next to the packaged binaries; `BATTLETRIS_ASSETS_DIR` overrides this for development |

Package-manager and storefront distribution remain later scope until GitHub
Release archives are validated on Linux, macOS, and Windows.
