# GitHub release setup

The workflows build unsigned artifacts without repository secrets. Public distribution should configure:

- `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` for updater signatures.
- `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, and `APPLE_SIGNING_IDENTITY` for macOS code signing.
- `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID` for Apple notarization.
- A Windows code-signing provider or certificate workflow before publishing broadly.

Protect `main`, require the `frontend`, `native-core`, and `desktop-smoke-build` jobs, require pull requests, and prevent force pushes.

Release packages are intentionally created on their native hosted runners. Cross-compiling all desktop packages from one developer machine is not part of the supported release process.
