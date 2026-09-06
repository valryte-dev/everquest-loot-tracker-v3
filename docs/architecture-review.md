# Architecture and Performance Review

## Executive summary

The current stack remains a strong fit for the application:

- **Tauri** provides a lightweight, cross-platform desktop shell.
- **Rust** suits file watching, parsing, SQLite access, and background work.
- **React with TypeScript** supports the increasingly rich interface.
- **SQLite** remains appropriate for a local-first desktop application.

A ground-up rewrite is not recommended. The safest approach is an incremental refactor that preserves every feature while improving query scope, background processing, database consistency, and module boundaries.

The highest-value change is to stop rebuilding the entire application snapshot whenever one page needs fresh data. True page queries, scoped refresh notifications, and single-flight refresh handling should improve startup, navigation, window resizing, and live-update responsiveness.

## Review requirements

- Preserve every existing feature.
- Do not regress live log or output-file detection.
- Improve responsiveness during startup, navigation, resizing, and refreshes.
- Use consistent item identity and market-value access everywhere.
- Normalize data where it improves correctness and maintainability.
- Establish readable Rust and React feature boundaries.
- Make each migration independently reversible and measurable.

## Current architecture

```text
EverQuest log and output files
             |
             v
   Rust watchers and parsers
             |
             v
      Application services
             |
             v
          SQLite DB
             |
             v
   Tauri command boundary
             |
             v
 React application and pages
```

This overall separation is sound. The main concerns are broad refresh paths, duplicated domain representations, oversized modules, and expensive operations inside latency-sensitive watcher flows.

## Measured database state

The inspected local database was approximately **1.97 GB**.

| Data | Approximate rows |
| --- | ---: |
| Damage events | 4,403,592 |
| Incoming damage events | 700,896 |
| Damage encounters | 58,591 |
| Encounters marked active | 435 |
| Files represented by scan cursors | 96 |

Additional observations:

- `PRAGMA foreign_key_check` found no violations.
- `freelist_count` was zero, so the size is live data rather than reclaimable free pages.
- Repeated `raw_line` and `source_file` values account for roughly 689 MB across outgoing and incoming damage before indexes.
- Unresolved item associations remain in live loot (71), history loot (737), offers (75), and inventory (1,701 missing IDs and one ID/name mismatch).
- Linked loot, tracked loot, splits, and completed splits had no unresolved item associations.

## Measured query behavior

| Query | Approximate duration |
| --- | ---: |
| Recent 5,000 encounter rows | 8.8 ms |
| Participant aggregates for 5,000 encounters | 205.7 ms |
| Incoming damage aggregates | 45.4 ms |
| Weapon aggregates | 259.2 ms |
| Global combat query | 18 ms per call |
| Inventory value query | 5 ms |

The individual timings are not catastrophic. The problem is that several expensive queries are bundled together and repeatedly triggered by pages that do not need them.

## Highest-priority findings

### 1. Global snapshots load too much

`App.tsx` requests a broad snapshot during initial load, data-change events, and revision checks. The backend snapshot assembles unrelated feature data, including thousands of damage summaries.

The current page-snapshot function still builds the full snapshot before pruning fields. This reduces the response payload but does not reduce database work.

Use three query levels:

1. **Shell snapshot** for active character, group, watcher status, global system messages, mini live-fight state, and lightweight revisions.
2. **Page snapshot** containing only the current page's paginated or bounded data and summaries.
3. **Focused mutation response** returning the changed entity or affected page revision without forcing a global reload.

### 2. Refreshes can overlap

Multiple sources can request refreshes at nearly the same time, allowing redundant calls and stale responses.

Recommended behavior:

- Allow only one refresh per scope at a time.
- Coalesce requests received while a refresh is running.
- Run one final refresh if data changed during the active request.
- Reject results older than the last applied revision.
- Track scoped revisions such as `shell`, `loot`, `characters`, `splits`, and `damage`.

### 3. Network uploads block watcher work

Planner uploads currently happen synchronously during export processing. A slow request, certificate failure, HTTP error, or timeout can delay file processing and live updates.

The watcher should import locally and enqueue work:

```text
Watcher -> local import -> upload_jobs row -> background uploader
```

The uploader should provide persistent status, bounded retry with backoff, manual retry, shutdown cancellation, and no coupling to live log processing.

### 4. Log writes are too granular

Lines are read in batches, but event application repeatedly opens connections or uses small implicit transactions.

Recommended behavior:

- Use one connection for a processed file batch.
- Apply parsed events in a transaction.
- Commit in bounded chunks for very large files.
- Emit one scoped notification after each committed batch.

### 5. The same log ranges are parsed repeatedly

History, death reports, damage, and live features independently inspect overlapping log data. This duplicates reads and creates cursor-consistency risk.

The target is one ingestion coordinator that reads each byte range once and distributes typed events:

```text
File range
   |
   v
Shared envelope parser
   |
   +--> loot and linked loot
   +--> group state
   +--> history and deaths
   +--> damage and healing
   +--> merchant messages
```

Existing per-feature cursors should remain during migration until the unified pipeline proves exact parity.

## Damage subsystem

Damage is the largest and most expensive data domain.

Recommended changes:

- Separate live encounter state from historical browsing.
- Maintain encounter and participant summary rows incrementally.
- Paginate historical encounters instead of loading a large fixed set.
- Close encounters on death, inactivity, character switch, startup reconciliation, and watcher restart.
- Verify query plans and add indexes around encounter ID, timestamp, attacker, defender, and active state.
- After validating diagnostics requirements, stop storing full raw lines for new high-volume damage records or move them into a retention-controlled diagnostics table.
- Preserve user-facing encounter history even if diagnostic event retention is introduced.

## Database normalization

### Master item identity

Many feature tables still store only an item name. Name-only matching is vulnerable to punctuation, apostrophes, articles, spelling differences, and normalization changes.

Every item-bearing record should keep both:

- `item_id`: nullable foreign key to the canonical master item.
- `item_name`: original captured display text for historical fidelity.

Safe migration path:

1. Add nullable `item_id` columns.
2. Backfill confident matches.
3. Resolve all new records during ingestion.
4. Prefer the ID on reads and fall back to normalized names.
5. Show unresolved or conflicting matches on the Items admin page.
6. Add stronger constraints only after unresolved records are controlled.

### Split lifecycle

Active, manual, and completed split tables mirror UI phases but fragment one business lifecycle.

Recommended target:

```text
split_items
  id
  item_id
  captured_item_name
  phase: looted | sold_pending | paid
  looted_by_person_id
  held_by_person_id
  dropped_by_mob_id
  sale_value
  sold_at
  note

split_participants
  split_item_id
  person_id
  share_weight
  payout_amount
  paid_at
```

This directly models:

1. Item looted and held.
2. Item sold with payouts pending.
3. Each person's payout completed independently.

Use dual-write and shadow comparison. Retain old tables for at least one release to support rollback.

### Compound projects and recipes

Core compound data is stored in settings JSON while normalized compound and recipe tables also exist. This creates competing models.

Recommended changes:

- Store templates, projects, selected templates, components, contributors, and split links relationally.
- Link compounds and recipe components to master item IDs.
- Keep only UI preferences in settings JSON.
- Dual-write JSON and relational data during migration.
- Compare calculated summaries before changing the read path.

### People and aliases

People, characters, aliases, split participants, holders, looters, and senders should eventually share one identity model:

```text
people
  id
  display_name

characters
  id
  character_name
  person_id
  server
```

Captured historical names should still be retained when needed. This work should follow refresh, item, and split improvements because it affects many features.

### Sources and application logs

- A future `log_sources` table can replace repeated source paths with source IDs while retaining file metadata, character, server, and rotation lineage.
- The rolling application log needs actual retention by age and row count, with errors retained longer than routine entries.

## Migrations and startup

Schema migration and derived-data refresh currently overlap. Some SQL runs unconditionally, including item-resolver reconstruction.

Separate these responsibilities:

- **Schema migrations:** versioned, idempotent, one-time operations.
- **Backfills:** resumable jobs with visible progress and errors.
- **Derived refreshes:** explicit, transactional application jobs.
- **Maintenance:** scheduled or user-triggered tasks represented by global system status.

Resolver refreshes should be atomic so readers see either the old complete result or the new complete result, never a partially rebuilt table.

## Backup and restore

Prefer SQLite's online backup API over copying a live database after a WAL checkpoint.

A safe restore should:

- Preserve a recovery copy of the current database.
- Pause writers and close active connections.
- Validate the selected backup before replacement.
- Restore and run integrity/schema checks.
- Restart the application after success.

## Code organization

Current concentration points include:

- `runtime.rs`: more than 3,000 lines.
- `data.rs`: more than 2,500 lines.
- `App.tsx`: dense global application orchestration.
- Old versioned or unused UI components.
- Broad `any` and `serde_json::Value` usage at the command boundary.

Recommended feature-oriented organization:

```text
src-tauri/src/
  features/
    loot/
      commands.rs
      models.rs
      parser.rs
      repository.rs
      service.rs
    characters/
    splits/
    compounds/
    merchant/
    history/
    damage/
    spells/
    items/
  platform/
    file_watchers/
    jobs/
    sqlite/
    logging/
  shared/
    identities/
    parsing/
    revisions/

src/
  app/
    shell/
    routing/
    notifications/
  features/
    loot/
    characters/
    splits/
    compounds/
    merchant/
    history/
    damage/
    spells/
    items/
  shared/
    components/
    hooks/
    contracts/
    formatting/
```

The exact names matter less than clear ownership. Parsing, persistence, commands, UI, and tests for a feature should be easy to locate together.

Replace generic command payloads and `any`-heavy frontend state with typed request and response contracts. Generated TypeScript bindings from Rust types are ideal if compatible with the build; otherwise use explicitly mirrored interfaces plus contract tests.

## UI consistency

The shared grid already handles much of the filter, clear, sort, and pagination standard. Custom grids should adopt it or deliberately provide equivalent behavior.

Centralize these reusable patterns:

- Item cells with spell tooltips, gem highlighting, and clickable-link support.
- Market value and market age as separate columns.
- Icon-only row actions with accessible labels and tooltips.
- Filters with a visible clear button.
- Sortable headers.
- Empty, loading, stale, and error states.
- File and folder controls with Browse buttons.
- Collapsible panels with persisted preferences where useful.

## Target architecture

```text
File watchers
    |
    v
Ingestion coordinator -----> Persistent background jobs
    |                              |
    v                              v
Typed domain events          Planner/wiki/market network work
    |
    +--------+---------+---------+---------+
    v        v         v         v         v
  Loot    History    Damage    Merchant   Groups
    |        |         |          |         |
    +--------+---------+----------+---------+
                     |
                     v
              Repositories / SQLite
                     |
                     v
        Scoped Tauri queries and mutations
                     |
                     v
          React feature stores and pages
```

Target properties:

- Read each new log range once.
- Persist locally before optional network work.
- Share typed domain events across features.
- Use page-specific, paginated reads.
- Publish scoped revision notifications.
- Resolve items through one canonical service.
- Expose durable background tasks through global system messages.

## Safe implementation plan

### Phase 0: Baseline and safeguards

- Capture startup, navigation, refresh, resize, watcher-latency, and query timings.
- Add parity tests for loot, links, groups, characters, imports, splits, compounds, merchant parsing, history, deaths, damage, CH chains, spells, gems, WTS, and updates.
- Add database migration fixtures from older releases.
- Document backup and rollback procedures.

### Phase 1: Scoped reads and refreshes

- Add a lightweight shell snapshot.
- Implement true page-specific backend queries.
- Add scoped revision counters and notifications.
- Add single-flight refresh handling.
- Keep the global snapshot behind a rollback flag until parity is proven.

This should deliver the largest immediate improvement to startup, navigation, resizing, and live updates.

### Phase 2: Watcher isolation and batched writes

- Move Planner and other HTTP work to persistent background jobs.
- Process log and output batches through one connection and bounded transactions.
- Notify the UI after commits instead of per event.
- Measure event-to-UI latency during backlogs and network failures.

### Phase 3: Damage read model

- Separate current encounters from historical browsing.
- Add incremental encounter and participant summaries.
- Paginate history.
- Repair stale active-encounter lifecycle behavior.
- Review raw-line retention after confirming diagnostic needs.

### Phase 4: Canonical item IDs

- Add nullable item IDs to item-bearing records.
- Backfill known matches.
- Resolve at ingestion time.
- Add an unresolved-items administration queue.
- Switch pages to one item-access service with name fallback.

### Phase 5: Compound normalization

- Move projects from settings JSON to relational tables.
- Dual-write while validating.
- Shadow-compare project summaries and contributions.
- Retain JSON rollback data for at least one public release.

### Phase 6: Split lifecycle unification

- Add unified split and participant tables.
- Dual-write every split mutation.
- Compare phase totals, balances, pending payouts, and completed history.
- Switch reads only after exact parity.

### Phase 7: Module and contract cleanup

- Split large Rust and React files into feature-owned modules.
- Remove dead versioned components only after confirming no use.
- Replace generic values with typed contracts.
- Add architecture decision records for parsing, identity, refreshes, and jobs.

## Rollback strategy

Every material change should be independently reversible:

- Put new read paths behind feature flags.
- Use additive migrations before destructive cleanup.
- Dual-write old and new models during structural changes.
- Shadow-read and compare before switching the UI.
- Do not remove legacy tables or columns in the release that first introduces replacements.
- Back up the database before backfills.
- Make backfills resumable.
- Keep verification queries with each migration.

## Verification checklist

Before completing any phase:

- All Rust tests pass.
- All frontend tests pass.
- Strict Clippy and linting pass.
- Production frontend and Tauri builds pass.
- A local standalone Windows executable is produced for testing.
- Existing databases migrate successfully.
- New installations create databases successfully.
- Watchers are tested with rotation, character switching, backlogs, and simultaneous output changes.
- Network failures do not delay live log processing.
- Refreshes do not reset in-progress form input.
- Old and new data paths produce matching results.
- Performance is compared with the Phase 0 baseline.

## Implementation status — 2026-09-06

The non-destructive refactor phases are implemented and checkpointed:

- Page requests now execute selective database queries instead of building and pruning a global snapshot.
- Frontend refreshes are single-flight and coalesce overlapping requests; shell task/combat status uses a lightweight global status query.
- Log append batches and cursor advancement commit atomically on one connection.
- Inventory exports persist locally first, then enter a durable Planner upload queue with bounded retry and visible task progress.
- Damage encounter, participant, and target summaries are maintained incrementally; historical reads prefer summaries and fall back to raw events during backfill.
- Canonical item IDs were added additively to item-bearing records and all value reads share the same resolver with a name fallback.
- Compound projects/templates and split lifecycle phases are shadowed into normalized relational snapshots while legacy storage remains the active rollback path.
- Shared TypeScript contracts now define compound models and mutation runners; untrusted legacy JSON is normalized from `unknown` values.
- Database backup/restore uses SQLite's online backup API, integrity checks, and an automatic pre-restore recovery copy.
- The System page displays the actual runtime schema version.
- Startup database writers are ordered (live folder reconciliation, backlog scan, then damage-summary backfill), and long runtime transactions share a writer coordinator to prevent SQLite lock failures.

Deliberately deferred destructive work:

- Raw combat/log retention is unchanged because deleting diagnostic history requires an explicit opt-in policy.
- Legacy compound and split tables remain in place until at least one public release validates the normalized shadow models.
- Source-path deduplication is not applied to existing high-volume history because rewriting the approximately 2 GB database would add migration and startup risk without improving the live hot path.

See [architecture-rollback.md](architecture-rollback.md) for checkpoint tags and the recovery procedure.
## Immediate next step

The first implementation slice should include:

1. True page-scoped queries.
2. A lightweight shell snapshot.
3. Scoped revision notifications.
4. Single-flight frontend refreshes.
5. Batched watcher transactions.
6. Background Planner upload jobs.

This addresses the most visible performance risks without changing the user-facing data model and creates the foundation for safe item, compound, and split normalization.
