# BattleTris Release Layout

Release archives are generated under `dist/` and are not source artifacts. This
directory is kept so local and CI packaging commands share a stable output root.

Use `./scripts/package-check.sh` for the local package gate. It builds an archive
and runs headless package smoke: server help, `battletris-tools net-smoke --help`,
direct loopback smoke, and loopback self-hosted lobby register/list/join/status.

Packaged multiplayer setup instructions are emitted into each archive README and
mirrored in `docs/distribution.md`.
