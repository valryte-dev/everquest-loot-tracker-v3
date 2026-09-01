# System Messages and Application Updates

## User experience

- The application shell renders system-message banners above every page.
- Each message can include a primary action and a dismiss button.
- Update dismissals are stored against the specific release version. A later release therefore appears automatically.
- The System page shows the installed version, latest public release, last check time, check status, and update actions.

## Release checks

The Rust backend queries the repository's latest public GitHub release in a background thread. Automatic checks are cached for 12 hours so startup is not blocked and GitHub API limits are respected. Users can bypass the cache with **Check for updates** on the System page.

Successful checks store the latest version and verified GitHub release URL in `app_settings`. Failures are stored as update status and written to `application_logs` under the `updates` area.

Versions are compared using Semantic Versioning. Draft and prerelease builds are excluded because GitHub's latest-release endpoint returns the latest published stable release.

## Update action

**View update** and **Download update** open the public GitHub release page. Automatic binary replacement is not enabled until Tauri updater signing and platform code-signing secrets are configured. Never silently install an unsigned package.

## Future rule

New global notices should use the shared system-message banner pattern, include a stable message identifier, provide an accessible dismissal label, and persist dismissal only for the exact notice or release version.
