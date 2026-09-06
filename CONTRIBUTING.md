# Contributing

All changes must follow the mandatory repository architecture gate in [`docs/architecture-rules.md`](docs/architecture-rules.md). Repository agents also receive the same requirements from [`AGENTS.md`](AGENTS.md).

Before contributing:

- Read the architecture rules and the applicable feature/parser specifications.
- Create a checkpoint before high-risk migrations or cross-cutting refactors.
- Keep business rules out of React components and Tauri command handlers.
- Add parser fixtures from real EverQuest lines before changing a parser.
- Use additive, versioned SQLite migrations; never rewrite user data without explicit approval, a verified backup, and a recovery path.
- Use page-scoped reads, coalesced refreshes, canonical item identity, and durable background jobs.
- Follow the grid, theme, accessibility, and Browse-control standards.
- Never render mock inventory, loot, split, or combat data while real data is loading.
- Ensure file watchers coalesce duplicate events and do not refresh unrelated UI state.
- Complete the architecture definition-of-done checklist before opening a pull request.