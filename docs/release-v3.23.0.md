# EverQuest Loot Tracker v3.23.0

This release adds creative character portrait tooling, stronger crash diagnostics, and another broad performance pass for large databases and high-traffic encounters.

## Highlights

- Added Portrait Studio for composing saved characters, equipment, and exported group portraits.
- Added saved portrait projects, reliable project loading, character removal, and PNG export.
- Added optional AI portrait generation with user-managed provider credentials.
- Added persistent application diagnostics and a Discord-friendly crash report workflow.
- Improved character model, equipment, map, quest, proc, and research catalog behavior.
- Coalesced file-watcher notifications so log spam cannot create an unbounded event backlog.
- Reduced offscreen live-meter rendering work during encounters with many fighters.
- Added slow live-batch diagnostics to make future performance reports actionable.
- Added targeted schema indexes for large combat databases and hot live-attribution queries.

## Upgrade notes

- Database schema 55 is applied automatically on first launch.
- Existing data and saved settings are preserved.
- The release includes native Windows, macOS, and Linux packages plus a Flatpak bundle.
