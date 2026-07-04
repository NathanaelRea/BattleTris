# Manual Network Tests

Use this checklist for release-candidate multiplayer testing. Record client
version, protocol version, OS, network type, host bind/share address, lobby
address, and any visible error text.

## Direct IP

1. On the host, set Host Bind to `0.0.0.0:4405` and Share Address to the host LAN
   IP, for example `192.168.1.23:4405`.
2. Open Challenge, choose Host Direct, and start hosting.
3. On the joiner, set Join Address to the host Share Address, choose Join Direct,
   and start the challenge.
4. Confirm the host sees an incoming challenge and can accept.
5. Confirm both clients enter Game, show Direct-Connect status, and play basic
   movement without checksum/desync errors.
6. Repeat with the host denying the challenge. The joiner should see
   `Challenge denied: <reason>.` and return to a recoverable Challenge state.

## Hosted Lobby

1. Start a lobby server: `battletris-server --listen 0.0.0.0:4404 --community garage`.
2. On both clients, set Lobby Address to the server LAN address, not `0.0.0.0`.
3. On the host, set Host Bind and Share Address as in the Direct IP test.
4. Choose Host Via Lobby or Sleep and confirm the host becomes available.
5. On the joiner, choose Browse Lobby, refresh if needed, select the host, and
   send the challenge.
6. Confirm the server issues hosted start metadata, the joiner connects directly
   to the host share address, the host accepts, and both clients enter the same
   hosted session with the same seed.

## Sleep/Biff

1. From Startup, enter Sleep with valid identity, community, lobby, bind, and
   share settings.
2. Confirm Sleep shows identity, community, server, bind, share, protocol, ranked
   preference, and availability status.
3. From another client, challenge the sleeping player through Direct IP or Browse
   Lobby.
4. Confirm the sleeping client wakes to an incoming challenge prompt and Enter/C
   accepts while D denies.
5. Wake/cancel Sleep and confirm the same port can be reused immediately.

## Ranked Results

1. Run a hosted game with Hosted Ranked enabled and no desyncs.
2. Finish the game on both clients.
3. Confirm the first submitted claim shows `Ranked result pending: waiting for the
   peer claim.`
4. Confirm the second matching claim records the result and the hosted Community
   roster can fetch the server record.
5. Repeat a mismatch/desync case and confirm the client shows
   `Ranked result rejected: <reason>.` or a desync stop message.

## Failure Cases

1. Start two hosts on the same bind address. The second should show
   `Host bind failed: address already in use. Try another port or cancel the old host.`
2. Join an unreachable host address. The joiner should show
   `Join timed out. Check the host share address and firewall.`
3. Try to use `0.0.0.0` or `127.0.0.1` from another machine. The Settings guide
   should warn to use the host LAN IP.
4. Stop the lobby server and browse/register. The client should show
   `Lobby server unavailable. Direct IP can still be used.`
5. Disconnect one peer during an online game. The other client should show
   `Peer disconnected. The online game has ended.`
6. Force or simulate a checksum mismatch. The online game should stop with
   `Desync detected. The online game stopped to avoid showing different results.`
