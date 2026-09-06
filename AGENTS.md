# Repository instructions: strict architecture

These instructions apply to every file and every future feature in this repository.

Before designing or changing a feature, read:

1. `docs/architecture-review.md`
2. `docs/architecture-rollback.md`
3. `docs/architecture-rules.md`
4. `docs/ui-control-rules.md` for any UI work
5. The relevant parser/integration specification under `docs/`

The rules in `docs/architecture-rules.md` are mandatory. A feature is not complete merely because it works in the UI. It must preserve feature boundaries, typed contracts, scoped reads and refreshes, canonical item identity, transactional local persistence, non-blocking durable network work, additive migrations, rollback compatibility, and the repository's verification requirements.

Do not weaken an architectural rule to make a change easier. If a requirement genuinely conflicts with the architecture, stop and document the conflict, risk, proposed exception, and rollback plan before implementation. Destructive migrations, data retention/deletion, removal of a legacy rollback path, or an unbounded hot-path query require explicit user approval.

Create a Git checkpoint before any high-risk migration or cross-cutting refactor. Keep existing behavior and storage readable until the replacement has parity tests and a verified rollback route.