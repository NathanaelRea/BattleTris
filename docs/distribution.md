# Distribution

BattleTris releases start as source-built GitHub Release archives. Each archive
contains the desktop client, optional server/tools binaries, bundled asset
manifests, release metadata, and documentation snapshots for one target platform.

## Local Packaging

Run `./scripts/package-release.sh` to build the host release package. Pass a Rust
different installed target.

Run `./scripts/smoke-package.sh dist/<archive>.tar.gz` to verify the archive
layout before publishing.

Run `./scripts/package-check.sh` to match the CI package job locally: it builds
the host release archive, then runs package smoke against that archive.

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
