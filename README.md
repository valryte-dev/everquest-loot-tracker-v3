# EverQuest Loot Tracker

A local-first, cross-platform EverQuest loot, split, inventory, recipe, and sale tracker for Windows, macOS, and Linux.

## Architecture

- **Rust** owns log tailing, parsing, SQLite, aliases, imports, market data, compound calculations, WTS links, INI writes, and the read-only local web service.
- **Tauri 2** provides the small native desktop shell and a typed command/event boundary.
- **React + TypeScript** provides the shared modern interface and reusable grid, filter, dialog, theme, and form components.
- **SQLite** remains local and compatible with existing Loot Tracker data through additive migrations.
- **GitHub Actions** produces official Windows, macOS, and Linux packages on native hosted runners. A local Windows installer is also built after feature changes for immediate testing.

See [architecture](docs/architecture.md), [feature requirements](docs/feature-parity.md), and [delivery plan](docs/delivery-plan.md).

## Local development

Local development runs only the platform being used by the contributor:

```bash
npm ci
npm run tauri dev
```

Core verification:

```bash
npm run build
npm test
cargo test --manifest-path src-tauri/Cargo.toml
```

For a local Windows test installer, run `npm run tauri build`. Do not build Linux locally; push a version tag after CI passes and the Release workflow will create the cross-platform draft release.

## Release process

1. Merge a reviewed pull request to `main`.
2. Confirm the three-platform CI workflow is green.
3. Update the application version and changelog.
4. Push a signed `v*` tag.
5. Review the draft GitHub release and publish it.

macOS signing/notarization and Tauri updater signing require the repository secrets listed in [release setup](docs/release-setup.md).
