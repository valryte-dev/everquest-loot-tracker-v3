# Architecture refactor checkpoints and rollback

The architecture work was delivered as additive, independently tagged checkpoints. No legacy loot, split, compound, history, inventory, combat, or market tables were removed. Schema 30 only adds `combat_spell_activity`; older builds ignore this table, so code rollback remains non-destructive.

## Checkpoints

| Tag | Purpose |
| --- | --- |
| `pre-proc-metrics-2026-09-06` | Verified v3.16.0 DoT-training baseline before proc persistence, direct-damage catalog enrichment, and combat-meter changes |
| `pre-dot-damage-tracking-2026-09-06` | Clean v3.16.0 baseline before spell classification and inferred DoT combat tracking |
| `dot-damage-tracking-2026-09-06` | Verified implementation checkpoint after additive schema, parser, runtime, UI, and tests |
| `pre-dot-training-page-2026-09-06` | Verified DoT implementation immediately before the isolated parser training workspace |
| `dot-training-page-2026-09-06` | Verified isolated DoT Training Lab implementation and production-parser simulation tests |
| `pre-architecture-refactor-2026-09-06` | Known-good baseline before the refactor |
| `architecture-phase-1-scoped-refresh` | True page-scoped database reads and coalesced frontend refreshes |
| `architecture-phase-2-ingestion` | Durable Planner upload queue and transactional log batches |
| `architecture-phase-3-damage` | Incremental damage summaries, resumable backfill, and stale encounter repair |
| `architecture-phase-4-items` | Canonical item IDs and shared market-value resolution |
| `architecture-phase-5-6-models` | Additive compound snapshots and unified split lifecycle snapshots |
| `architecture-phase-7-contracts` | Typed frontend contracts and safe online database backup/restore |
| database-management-preview | Read-only database statistics and cleanup previews |
| pre-folder-reconcile-lock-fix-2026-09-06 | Rollback point before startup writer sequencing and runtime transaction coordination |
| `pre-normalized-proc-catalog-2026-09-11` | Verified baseline immediately before the canonical 340-row item-to-proc catalog and broader direct-damage classification |

## Combat schema compatibility

Schema 31 adds the derived `combat_pet_evidence` table used to classify multi-word and otherwise-unowned pets as separate outgoing fighters. It contains no user-authored data and is rebuilt by a Damage Tracker rescan. Builds before schema 31 ignore this additive table.

Schema 32 adds `item_proc_spells`. It is reference data bundled from the Project 1999 Weapon Procs table and reconciled to nullable `master_items.item_id` values. It does not replace or mutate loot, inventory, split, encounter, or damage records. Builds before schema 32 ignore it. The separate `spell-info.db` cache advances its derived combat classifier to version 3 and can be safely rebuilt from the wiki catalog if needed.
## CH replay compatibility

CH encounter replay is additive and does not change the SQLite schema. Portable `.eqch.json` files live in the `ch-chain-replays` application-data directory. Rolling back the application leaves those files untouched; older builds ignore them. Copy that directory before removing or editing replay files manually.

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
