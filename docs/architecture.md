# V3 architecture

## Principles

1. Local-first: SQLite remains authoritative and network imports are optional integrations.
2. Compatible migration: V3 opens the established `EverQuestLootTracker/loot-tracker.db` and applies only additive, versioned migrations.
3. One domain core: log parsing, roster state, splits, compounds, values, inventories, WTS, and exports live in Rust—not in UI components.
4. Typed boundary: React talks to narrow Tauri commands and event channels using shared TypeScript contracts.
5. Platform adapters: paths, file sharing, notifications, startup, and packaging are isolated under infrastructure/platform.
6. Responsive performance: filesystem work, HTTP, parsing, and SQLite run away from the UI thread; event bursts are debounced and coalesced.
7. Safe writes: EverQuest INI modification remains byte-preserving and is covered by golden-file tests.

## Layers

```text
React features -> Tauri commands/events -> application use cases -> domain
                                               |
                                      infrastructure adapters
                                      SQLite · watcher · HTTP · INI
```

The loopback webpage becomes a read-only adapter over the same application queries instead of a second source of business logic.

## Platform data path

The core intentionally retains the V2 product directory name so the same database is discovered automatically:

- Windows: `%LOCALAPPDATA%/EverQuestLootTracker/loot-tracker.db`
- macOS: `~/Library/Application Support/EverQuestLootTracker/loot-tracker.db`
- Linux: `$XDG_DATA_HOME/EverQuestLootTracker/loot-tracker.db`, falling back to `~/.local/share/...`

## Build policy

Windows artifacts build on Windows, `.app`/`.dmg` on macOS, and AppImage/deb/rpm on Linux. The application code is shared; operating-system signing and bundling remain native.
