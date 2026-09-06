# Architecture refactor checkpoints and rollback

The architecture work was delivered as additive, independently tagged checkpoints. No legacy loot, split, compound, history, inventory, combat, or market tables were removed.

## Checkpoints

| Tag | Purpose |
| --- | --- |
| `pre-dot-damage-tracking-2026-09-06` | Clean v3.16.0 baseline before spell classification and inferred DoT combat tracking |
| `dot-damage-tracking-2026-09-06` | Verified implementation checkpoint after additive schema, parser, runtime, UI, and tests |
| `pre-architecture-refactor-2026-09-06` | Known-good baseline before the refactor |
| `architecture-phase-1-scoped-refresh` | True page-scoped database reads and coalesced frontend refreshes |
| `architecture-phase-2-ingestion` | Durable Planner upload queue and transactional log batches |
| `architecture-phase-3-damage` | Incremental damage summaries, resumable backfill, and stale encounter repair |
| `architecture-phase-4-items` | Canonical item IDs and shared market-value resolution |
| `architecture-phase-5-6-models` | Additive compound snapshots and unified split lifecycle snapshots |
| `architecture-phase-7-contracts` | Typed frontend contracts and safe online database backup/restore |
| database-management-preview | Read-only database statistics and cleanup previews |
| pre-folder-reconcile-lock-fix-2026-09-06 | Rollback point before startup writer sequencing and runtime transaction coordination |

## Safest rollback workflow

Do not rewrite the current branch or delete the current database. Create a recovery branch at the desired checkpoint:

```powershell
git switch -c recovery/architecture-phase-3 architecture-phase-3-damage
```

Build and test that branch against a copy of the database first. The migrations are additive, so older checkpoints ignore the newer shadow tables. Compound and split legacy storage remains intact for this purpose.

To return to current work:

```powershell
git switch main
```

## Data recovery

The System page database backup now uses SQLite's online backup API. Restore validates the selected database, creates a timestamped `loot-tracker-pre-restore-*.db` recovery backup of the current database, and only then restores the selected file.

Keep the recovery backup until the restored app has been verified. Application logs are not automatically deleted; retention remains opt-in so diagnostic history is never removed unexpectedly.

## Verification before changing checkpoints

1. Close the running app.
2. Back up the database from the System page.
3. Create a recovery branch from the desired tag.
4. Build the branch and launch it against a copied database.
5. Verify live loot, character switching, imports, splits, compounds, and live damage before using it with the primary database.