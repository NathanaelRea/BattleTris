# Manual Network Tests

Use this checklist for release-candidate multiplayer validation that cannot be
fully covered by the headless package smoke tests.

## Direct IP

1. Host a direct game bound to `0.0.0.0:4405` or another local address.
2. Join from a second client using the host's reachable LAN address, not
   `0.0.0.0` or `127.0.0.1`.
3. Confirm the challenge can be accepted and the game starts.
4. Confirm a denied challenge returns both clients to Challenge/Sleep cleanly.
5. Confirm disconnecting one client returns the peer to Challenge/Sleep cleanly.

## Self-Hosted Lobby

1. Start `battletris-server --modern-listen 0.0.0.0:4405 --community <name>`.
2. Point both clients at the server's reachable address.
3. Register a hosted game and confirm the joiner can list and join it.
4. Confirm gameplay still uses the host's direct share address.
5. Submit matching ranked results from both clients and confirm the server accepts
   the completed result.

## Legacy Interop

1. Host from the Rust client and join from the original client.
2. Host from the original client and join from the Rust client.
3. Confirm accept, deny, busy or timeout, and disconnect behavior.
4. Confirm board, score, funds, arsenal, weapon launch, Bazaar done, and
   death/game-over updates are visible on the peer.
