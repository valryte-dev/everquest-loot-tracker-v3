# Database management and retention

## Current scope

The Database Management page provides read-only analysis and safe maintenance controls:

- Database, WAL, reusable-page, schema, journal, and integrity statistics.
- Sortable and filterable storage categories.
- Estimated logical payload by category, clearly separated from physical file size.
- Data ranges and record counts.
- SQLite online backup.
- Preview-only combat-detail retention calculations.

No cleanup action is enabled until a retention policy is explicitly approved.

## Current database finding

The database audited on 2026-09-06 was 1,967,370,240 bytes (approximately 1.83 GiB) with no reusable freelist pages. Its dominant growth source was combat detail:

- 4,403,592 outgoing damage-event rows.
- 700,896 incoming damage-event rows.
- 58,591 encounter summaries.
- Detail ranges from May 2025 through September 2026.

Loot, activity history, application logs, inventory, merchant data, and market data are small relative to combat hit detail.

## Retention options measured on 2026-09-06

| Detail retained | Rows eligible for cleanup | Outgoing | Incoming |
| --- | ---: | ---: | ---: |
| 30 days | 4,370,685 | 3,759,470 | 611,215 |
| 90 days | 2,714,671 | 2,298,307 | 416,364 |
| 180 days | 1,506,481 | 1,251,791 | 254,690 |
| 365 days | 587,301 | 463,948 | 123,353 |

These values are previews and change as new combat is recorded.

## Recommended first policy

Keep encounter summaries and analytics indefinitely, and retain individual hit detail for 180 days. This currently makes about 1.5 million detail rows eligible while preserving:

- Encounter totals and outcomes.
- Per-player damage totals and hit counts.
- Incoming damage summaries.
- Historical DPS and comparison charts.

Old per-hit timelines, raw combat messages, and their per-hit weapon references would no longer be available before the cutoff.

## Required cleanup workflow

A future cleanup action must:

1. Calculate and show an exact preview.
2. Require an online database backup.
3. Require explicit confirmation containing the cutoff and eligible row count.
4. Run outside the watcher hot path as a visible background system task.
5. Delete in bounded transactions with resumable progress.
6. Preserve encounter, participant, and target summaries.
7. Report reusable internal pages after deletion.
8. Offer optimize-and-shrink as a separate operation only when enough temporary disk space exists.
9. Keep a recovery backup until the optimized database passes integrity and feature checks.

Deletion alone does not immediately reduce the database file on disk. It makes pages reusable by future writes. Shrinking the physical file is a separate, more expensive operation.