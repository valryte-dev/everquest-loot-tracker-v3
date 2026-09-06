## Summary

Describe the user-facing change and the feature module that owns it.

## Architecture gate

- [ ] I read `AGENTS.md` and `docs/architecture-rules.md`.
- [ ] Business rules are outside React pages and thin Tauri command handlers.
- [ ] Requests, responses, domain events, and persisted models are typed and validated.
- [ ] Reads are page-scoped/bounded and refreshes are coalesced and scoped.
- [ ] Item references and prices use the canonical shared resolver.
- [ ] Watcher writes are transactional; optional network work is durable and non-blocking.
- [ ] Database changes are additive/idempotent and preserve a tested rollback path.
- [ ] The feature follows grid, theme, accessibility, loading, error, and Browse-control rules.
- [ ] I added or updated realistic regression tests.
- [ ] I updated architecture/rollback documentation where applicable.

## Verification

- [ ] `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] `npm run build`
- [ ] `npm test`
- [ ] Local standalone Windows build produced for application-code changes

## Rollback

Name the checkpoint, feature/shadow switch, or additive compatibility path used to reverse this change safely.