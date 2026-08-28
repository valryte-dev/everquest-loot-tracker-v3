# Splits Functionality and Live Loot Integration

This document specifies the EverQuest Loot Tracker V3 split workflow and its relationship with Live Loot. It is intended for an agent implementing compatible functionality in another system.

Read this alongside:

- [`live-loot-parser-spec.md`](./live-loot-parser-spec.md) for log parsing, group snapshots, and mob correlation.
- [`pigparse-live-price-spec.md`](./pigparse-live-price-spec.md) for dynamic 30-day WTS values and item association.

## 1. Core domain boundary

Live Loot and Splits have different lifecycles:

- **Live Loot** is a temporary inbox of parsed drops. Users may clear it routinely.
- **Active Splits** is a durable working list of items that matter for later sale, consumption, or payout.
- **Sold & Consumed History** is the durable final record after an active split is completed.

Adding a Live Loot row to Splits is a **snapshot-copy operation**, not a flag stored only on the temporary loot row and not a dependent child that is cascade-deleted with that row.

The most important invariant is:

> Once a Live Loot drop has been saved to Splits, deleting that drop from Live Loot—individually, through a selection, or through “delete all”—must not delete or alter the saved split record.

## 2. Required Live Loot table behavior

Every Live Loot table row must provide an action button:

```text
Add to split
```

After the split snapshot has been created, the row should visibly indicate that state, for example:

```text
On split
```

Required behavior:

1. Clicking **Add to split** copies the current Live Loot values and group snapshot into split-owned storage.
2. The operation is idempotent. Repeated add requests for the same source loot ID must not create duplicate active splits or duplicate participants.
3. The UI should update only after the split write succeeds.
4. A normal Live Loot **Delete** action deletes only the temporary Live Loot record.
5. Live Loot bulk deletion follows the same rule and leaves all saved split records intact.
6. Removing an item from Splits must be a separate, explicit split action.

V3 currently makes the `On split` row action a toggle: clicking it again removes the active split while the source loot still exists. A new plugin may instead make removal available only on the Splits page, or require confirmation, to reduce accidental loss. In either design, the ordinary Live Loot Delete button must never remove the split.

## 3. Data copied from Live Loot

When the user adds a parsed drop to Splits, copy:

| Live Loot field | Active Split field | Behavior |
|---|---|---|
| `id` | `sourceLootId` | Origin/idempotency reference only |
| `itemName` | `itemName` | Independent editable copy |
| `mobName` | `mobName` | Independent editable copy; may be null |
| `looterName` | `looterName` | Also treated as the current holder in V3 |
| `attendees` | `participants` | Independent many-to-many snapshot |
| current time | `addedAt` | Time added to Splits |

The current implementation does not copy the original loot timestamp, source filename, raw log line, source offset, or PigParse price into the active split table.

The split's displayed market value is resolved dynamically by exact item-name association using the current cached PigParse 30-day WTS value. A user-entered payout override is stored separately.

Recommended normalized active-split model:

```json
{
  "id": 840,
  "origin": "live-loot",
  "sourceLootId": 3812,
  "itemName": "Tears of Prexus",
  "mobName": "a mortiferous golem",
  "looterName": "Youngman",
  "heldByName": "Youngman",
  "participants": ["Youngman", "Posed", "Skriz"],
  "payoutValuePp": null,
  "addedAt": "2026-08-25T10:30:00-06:00"
}
```

## 4. Persistence design

### Live Loot source tables

```sql
CREATE TABLE loot_drops (
    id INTEGER PRIMARY KEY,
    happened_at TEXT NOT NULL,
    item_name TEXT NOT NULL,
    mob_name TEXT,
    looter_name TEXT,
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    UNIQUE(source_file, source_offset, raw_line)
);

CREATE TABLE loot_drop_members (
    loot_drop_id INTEGER NOT NULL REFERENCES loot_drops(id) ON DELETE CASCADE,
    member_name TEXT NOT NULL,
    PRIMARY KEY (loot_drop_id, member_name)
);
```

### Split snapshots created from Live Loot

Equivalent V3 schema:

```sql
CREATE TABLE split_loot_items (
    id INTEGER PRIMARY KEY,
    loot_drop_id INTEGER NOT NULL UNIQUE,
    item_name TEXT NOT NULL COLLATE NOCASE,
    mob_name TEXT,
    looter_name TEXT,
    payout_value_pp INTEGER,
    added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE split_loot_members (
    split_loot_item_id INTEGER NOT NULL
        REFERENCES split_loot_items(id) ON DELETE CASCADE,
    member_name TEXT NOT NULL,
    PRIMARY KEY(split_loot_item_id, member_name)
);
```

### Critical foreign-key rule

`split_loot_items.loot_drop_id` is deliberately **not** a foreign key to `loot_drops.id`. It is a stable origin reference and uniqueness key.

Do not add either of these constraints:

```sql
-- Incorrect for this workflow:
FOREIGN KEY (loot_drop_id) REFERENCES loot_drops(id) ON DELETE CASCADE

-- Also incorrect because it prevents clearing Live Loot:
FOREIGN KEY (loot_drop_id) REFERENCES loot_drops(id) ON DELETE RESTRICT
```

The saved split must remain valid after its source loot row no longer exists.

For a new schema, naming the field `source_loot_id` rather than `loot_drop_id` makes this ownership boundary clearer.

## 5. Add-to-split operation

Exact V3-compatible behavior:

```sql
INSERT OR IGNORE INTO split_loot_items(
    loot_drop_id,
    item_name,
    mob_name,
    looter_name
)
SELECT id, item_name, mob_name, looter_name
FROM loot_drops
WHERE id = :loot_id;

INSERT OR IGNORE INTO split_loot_members(
    split_loot_item_id,
    member_name
)
SELECT split.id, member.member_name
FROM split_loot_items AS split
JOIN loot_drop_members AS member
  ON member.loot_drop_id = split.loot_drop_id
WHERE split.loot_drop_id = :loot_id;
```

A stronger plugin implementation should wrap both statements in one transaction and return an error when the source loot ID does not exist.

Recommended command contract:

```json
{
  "action": "loot.addToSplit",
  "payload": {
    "lootId": 3812
  }
}
```

Recommended response:

```json
{
  "ok": true,
  "created": true,
  "splitId": 840,
  "sourceLootId": 3812
}
```

For an idempotent retry, return success with `created: false` and the existing split ID.

## 6. Live Loot deletion

Individual and bulk Live Loot deletion should execute only against Live Loot storage:

```sql
DELETE FROM loot_drops WHERE id = :loot_id;
```

The `loot_drop_members` rows may cascade because they belong to Live Loot. The independently copied `split_loot_items` and `split_loot_members` rows must remain.

Deletion matrix:

| User action | Live Loot | Active Split | Split participants | Completed history |
|---|---:|---:|---:|---:|
| Delete one Live Loot row | Delete | Preserve | Preserve | Preserve |
| Delete selected Live Loot rows | Delete selected | Preserve all | Preserve all | Preserve |
| Delete all Live Loot rows | Delete all | Preserve all | Preserve all | Preserve |
| Remove active split | Preserve source if present | Delete | Cascade delete | Preserve |
| Complete active split | Preserve source if present | Delete after copy | Delete after copy | Create |
| Delete history record | No change | No change | No change | Delete selected history |

Do not implement a generic “delete loot everywhere” repository method for the Live Loot UI. Use separate commands for each lifecycle.

## 7. Independent editing after the copy

Once added, the active split owns independent editable fields:

- Item name
- Dropped-by mob
- Looted/held-by character
- Participants/shared-by names
- Payout value override

Editing Live Loot after saving the split does not update the split. Editing the split does not update Live Loot.

This is intentional snapshot behavior. It prevents later cleanup or correction of the temporary inbox from unexpectedly changing payout records.

If a plugin wants a “refresh from source” feature, make it a separate explicit action that previews overwritten fields. Never synchronize silently.

## 8. Manually added splits

The Splits page also supports entries with no Live Loot source. A manual split accepts:

```json
{
  "itemName": "Tears of Prexus",
  "mobName": "a mortiferous golem",
  "looterName": "Youngman",
  "attendees": ["Youngman", "Posed", "Skriz"],
  "payoutValuePp": null
}
```

Manual entries use their own item and participant tables. V3 exposes a unified active list by assigning string keys:

```text
loot:<source-loot-id>
manual:<manual-split-id>
```

This makes command routing explicit while allowing both types to appear in one sortable/filterable grid.

Manually entered participant names are also added to the remembered character-name catalog for reuse.

## 9. Value behavior

An active split exposes two value concepts:

```text
payoutValuePp   = user-entered override, nullable
marketValuePp   = current cached PigParse 30-day WTS by exact item name, nullable
```

Effective value:

```text
effectiveValuePp = payoutValuePp ?? marketValuePp ?? 0
```

An explicit override of `0` is valid and must not fall through to the market value.

Because `marketValuePp` is dynamically resolved, a PigParse refresh may change an open split's displayed value. Completing a split copies the chosen final value into history, where it becomes independent of later market refreshes.

## 10. Aliases and payout calculation

Some characters are alternate characters belonging to one person. V3 stores mappings:

```text
alias character -> canonical person
```

Before calculating a split:

1. Replace each participant name with its canonical alias target when one exists, matching case-insensitively.
2. Deduplicate the resulting canonical people.
3. Divide the effective split value by the number of unique canonical people.
4. Apply integer floor division to each person's share.

Formula:

```text
people = unique(participants.map(resolveAlias))
sharePp = floor(effectiveValuePp / max(1, people.length))
```

The current summary may leave a remainder undistributed because every share is floored. Example: `100 pp / 3 people` displays `33 pp` each and does not allocate the remaining `1 pp`.

Aliases affect summaries; they do not rewrite the participant snapshots stored on each split.

## 11. Holder and looter summaries

V3 currently uses `looterName` for both:

- **Looted By** — who originally looted the item.
- **Held By** — who is presumed to be holding it now.

The Splits summary groups active item names by the canonical form of `looterName` for both lists.

A new plugin may model `heldByName` separately so custody can change without losing provenance. For exact V3 compatibility, initialize:

```text
heldByName = looterName
```

and keep the two equal unless the target system intentionally extends the model.

## 12. Completing a split

An active split can be finalized with disposition:

```text
sold
consumed
```

Completion requires:

- Final integer value in platinum
- Disposition
- Optional note

Create a completed-history snapshot containing:

```json
{
  "itemName": "Tears of Prexus",
  "mobName": "a mortiferous golem",
  "looterName": "Youngman",
  "valuePp": 1250,
  "disposition": "sold",
  "note": "Sold in East Commons",
  "participants": ["Youngman", "Posed", "Skriz"],
  "completedAt": "2026-08-25T18:40:00-06:00"
}
```

Then remove the active split and its active participant rows. The completed record and completed participants are independently stored.

A plugin should perform history creation, participant copying, and active-split deletion in one transaction. Retrying the command should not create duplicate history records.

Completed history supports:

- Sorting and filtering
- Editing disposition, final value, and note
- Individual or bulk deletion
- Completed-value summary

## 13. UI requirements

### Live Loot page

Each row should show actions comparable to:

```text
[Track] [Edit] [Add to split] [Delete]
```

After saving:

```text
[Track] [Edit] [On split] [Delete]
```

The **Delete** action must remain scoped to Live Loot even when the row is on a split.

### Splits page

Provide three views:

1. **Active splits**
   - Add manual split
   - Filter and sort
   - Edit
   - Mark sold
   - Mark consumed
   - Delete active split
2. **Sold & consumed**
   - Filter and sort
   - Edit final value, disposition, and note
   - Select multiple/all and delete
3. **Payout summary**
   - Open total value
   - Completed total value
   - Amounts owed by canonical person
   - Items held by person
   - Items looted by person

The participant editor should place current group members first, support filtering with a one-click clear control, and allow rapid checkbox add/remove.

## 14. Reference state transitions

```text
LiveLootOnly
  -- Add to split --> LiveLootAndActiveSplit

LiveLootAndActiveSplit
  -- Delete Live Loot --> ActiveSplitOnly
  -- Remove split --> LiveLootOnly
  -- Complete split --> LiveLootAndHistory

ActiveSplitOnly
  -- Edit split --> ActiveSplitOnly
  -- Remove split --> Removed
  -- Complete split --> HistoryOnly

LiveLootAndHistory
  -- Delete Live Loot --> HistoryOnly

HistoryOnly
  -- Edit history --> HistoryOnly
  -- Delete history --> Removed
```

There is intentionally no transition where `Delete Live Loot` implicitly removes an active split or history record.

## 15. Minimum compatibility tests

An implementation should test at least:

1. Live Loot rows expose an Add to split action.
2. Adding copies item, mob, looter, and every loot-time participant.
3. Adding the same source loot twice creates one active split.
4. Adding fails cleanly when the source loot does not exist.
5. The Live Loot row reports `splitListed = true` after a successful add.
6. Deleting the source Live Loot row preserves the active split.
7. Bulk-deleting Live Loot preserves every saved split.
8. Clearing all Live Loot preserves every saved split.
9. Split participants survive deletion of the source loot participants.
10. Editing Live Loot after the copy does not rewrite the split.
11. Editing the split does not rewrite Live Loot.
12. Removing a split deletes only the active split and its participants.
13. Manual splits work without any Live Loot source.
14. A payout override of zero takes precedence over market value.
15. Without an override, the current PigParse 30-day WTS value is used.
16. Alias resolution deduplicates multiple characters belonging to one person.
17. Equal shares use integer floor division.
18. Completing a sold split copies final fields and participants to history.
19. Completing a consumed split copies final fields and participants to history.
20. Completing removes the active split but preserves completed history.
21. Later PigParse refreshes do not rewrite completed values.
22. History editing does not recreate an active split.
23. History deletion does not affect Live Loot or other active splits.

## 16. Source-of-truth implementation

This specification reflects these V3 files at release `v3.3.0`:

- `src/app/App.tsx` — Live Loot row actions, split editor, history, and summaries.
- `src-tauri/src/application/data.rs` — split copy, editing, removal, completion, alias persistence, and Live Loot deletion.
- `src-tauri/src/migrations/000_v2_compatibility.sql` — independent Live Loot, active split, manual split, and completed-history tables.
- `src/shared/contracts.ts` — frontend record contracts.

If deployed code and this document disagree, the deployed version's code is authoritative. Preserve the deletion invariant even if the target system uses different table names or storage technology.
