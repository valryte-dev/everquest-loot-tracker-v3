# Contributing

- Keep business rules out of React components and Tauri command handlers.
- Add parser fixtures from real EverQuest lines before changing a parser.
- Use additive, versioned SQLite migrations; never rewrite a user's database in place without a backup.
- Every grid must support filtering, one-click filter clearing, visible sortable columns, keyboard operation, loading state, empty state, and error state.
- Never render mock inventory, loot, or split data while real data is loading.
- Filesystem watchers must coalesce duplicate events and must not refresh unrelated UI state.
- Run frontend build/tests and Rust formatting/tests before opening a pull request.
