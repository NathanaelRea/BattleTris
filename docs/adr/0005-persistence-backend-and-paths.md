# ADR 0005: Persistence Backend And Paths

- Status: Accepted
- Date: 2026-07-01
- Phase: 16

## Context

BattleTris needs persistent player records, rankings, head-to-head stats,
settings, migrations, and a path toward optional future server storage. There
are no known legacy database files to migrate for v1, so the rewrite can start
with a fresh schema while preserving the legacy record concepts.

The persistence backend and path conventions affect `battletris-db`,
`battletris-client`, `battletris-server`, packaging, and test setup.

## Decision

Use SQLite through `rusqlite = "0.39.0"` with bundled SQLite for consistent
desktop builds. Use `refinery = "0.9.2"` for schema migrations and
`directories = "6.0.0"` for cross-platform project directories. Store
user-editable client settings as TOML.

Initial dependencies:

```toml
rusqlite = { version = "0.39.0", default-features = false, features = ["bundled"] }
refinery = { version = "0.9.2", default-features = false, features = ["rusqlite-bundled"] }
directories = "6.0.0"
toml = "1.1.2+spec-1.1.0"
```

Use `directories::ProjectDirs::from("org", "BattleTris", "BattleTris")` unless
a later product decision selects a different app id.

Default locations:

| Data | Location |
| --- | --- |
| Settings | `ProjectDirs::config_dir()/settings.toml` |
| Local player DB | `ProjectDirs::data_dir()/battletris.sqlite3` |
| User theme packs | `ProjectDirs::data_dir()/themes/` |
| User sound packs | `ProjectDirs::data_dir()/sounds/` |
| Logs | `ProjectDirs::state_dir()/logs/` on Linux, otherwise `data_local_dir()/logs/` |
| Packaged assets | `assets/` next to the executable, with CLI/environment override for development. |

Do not port the legacy hash database as the new backend. Build an importer only
if real legacy `.idx`/`.dat` files appear.

## Consequences

- SQLite supports ranking and head-to-head queries better than flat files while
  remaining simple for local play and future server use.
- Bundled SQLite reduces platform variance in packaged desktop builds at the
  cost of extra build time.
- Migrations are testable from an empty database and previous schema versions.
- Identity model and ranked trust scope still need the Phase 16 decision before
  implementation writes ranked records.
- Phase 16 implementation uses `rusqlite 0.39.0` because `refinery 0.9.2`'s
  rusqlite integration rejects `rusqlite 0.40.1` through the `libsqlite3-sys`
  links constraint.
