# Mandatory architecture rules for future features

This document is the repository's architecture gate. It applies to all new features, fixes, refactors, migrations, and UI changes. Pull requests and agent-produced changes must satisfy every applicable rule.

## 1. Feature ownership and boundaries

- Put feature behavior in a feature-owned module. React pages must not accumulate business rules, and Tauri command handlers must remain thin orchestration boundaries.
- Shared code belongs in `shared` only when at least two features genuinely use the same concept.
- Extend the existing domain parser and typed event flow instead of parsing the same log line independently in multiple features.
- Do not add another broad responsibility to `App.tsx`, `application/data.rs`, or `application/runtime.rs` when a focused module can own it.
- Remove or replace versioned implementations only after proving they are unused and the active implementation has parity coverage.

## 2. Typed contracts

- Define request, response, persisted model, and domain-event shapes explicitly.
- TypeScript `any` and unstructured Rust `serde_json::Value` are allowed only at a documented compatibility or normalization boundary. Convert them immediately into a validated typed model.
- Shared frontend/backend concepts must have one canonical contract location. Do not redefine compound, item, split, damage, or identity models in individual pages.
- Unknown external input must be validated and normalized before business logic uses it.

## 3. Database and identity

- SQLite changes must use numbered, additive, idempotent migrations.
- Every item-bearing record stores the canonical nullable `item_id` and the captured display name. Resolve by ID first and use normalized-name matching only as a fallback.
- All prices must come through the shared item/value resolver. A feature must not create its own item-price association logic.
- Split participants and payouts must preserve per-person state. Never collapse independently paid participants into a single destructive status update.
- Replacing a persisted model requires dual-write or append-only shadow storage, parity tests, and retention of the legacy read path for at least one public release.
- Never delete, rewrite, compact, or apply retention to user data without explicit approval, a verified backup, visible progress, and a recovery plan.

## 4. Ingestion, watchers, and network work

- Read each new file byte range through the shared parsing pipeline. Cursor advancement and the events produced from that range must commit atomically.
- File watchers must handle file rotation, split log files, truncation, application downtime, character switching, duplicated notifications, and partially written lines.
- Persist local imports before optional external work.
- Network calls must not block file watching, live loot, combat, or UI refresh. Use a durable job with bounded retry/backoff, visible task state, and safe restart behavior.
- Character switches clear transient group state. Historical records remain associated with the character/source that produced them.
- Add real log fixtures and parser regression tests for every new message pattern or ambiguity.

## 5. Query and refresh performance

- Use shell/global-status queries for lightweight cross-page state and page-scoped queries for feature data. Do not reintroduce a full application snapshot into normal page refreshes.
- Queries over growing history must be bounded, indexed, summarized, or paginated. New unbounded history queries require measured justification.
- Refresh requests must be single-flight/coalesced and must ignore stale responses.
- Emit one scoped notification after a committed batch. Do not refresh unrelated pages or reset in-progress forms because background state changed.
- Expensive backfills and maintenance operations must be resumable and expose progress through system tasks.
- Performance-sensitive changes must include a regression test or recorded before/after measurement appropriate to the risk.

## 6. UI consistency and accessibility

- Follow `docs/ui-control-rules.md`.
- Every data grid provides filtering, one-click clear, sortable visible columns, a loading state, an empty state, an error state, and keyboard-accessible controls.
- Grid row actions use compact icons with accessible labels and tooltips.
- Price and market-window age/basis remain separate columns wherever prices are displayed.
- File and folder fields use the shared native `PathPicker` Browse control; drag-and-drop also provides a Browse fallback.
- Pages render real loading states and never flash mock/sample data before persisted data arrives.
- Themes, top alignment, global system messages, and the global live-fight strip must work consistently on every new page.

## 7. Safety and rollback

- Create a named Git checkpoint before a high-risk migration, ingestion rewrite, or cross-cutting refactor.
- Prefer additive changes and feature/shadow switches over destructive replacement.
- Database backup uses SQLite's online backup mechanism. Restore must validate integrity and preserve an automatic pre-restore recovery copy.
- Do not remove a compatibility table, column, parser, or fallback in the same release that introduces its replacement.
- Update `docs/architecture-rollback.md` when a new architectural checkpoint or data-model transition is added.

## 8. Definition of done

A change is complete only when all applicable items pass:

- Rust formatting check.
- Strict Clippy with warnings denied.
- Full Rust tests, including migration replay and realistic parser fixtures.
- Full frontend tests.
- Strict TypeScript and production frontend build.
- Cross-platform CI remains green on Windows, macOS, and Linux.
- A local standalone Windows executable is built for application-code changes unless the user explicitly says otherwise. Do not build Linux locally unless requested.
- Build distributable standalone executables through `npm run build:standalone` (Tauri `build --no-bundle`) so production web assets use the embedded custom protocol. Plain `cargo build --release` creates a development-protocol binary that may try to load `localhost` and must not be distributed.
- Existing database migration and clean database creation are verified for schema changes.
- Watcher, character-switch, backlog, network-failure, and form-stability regressions are tested when affected.
- Documentation and rollback instructions are updated.

## Exception process

An exception must be explicit, narrow, and documented before implementation. Record:

1. The rule that cannot be met.
2. Why the feature cannot be implemented within it.
3. User-visible and data-integrity risks.
4. The temporary containment and rollback plan.
5. The follow-up that removes the exception.

Silence, schedule pressure, or convenience is not approval for an exception.