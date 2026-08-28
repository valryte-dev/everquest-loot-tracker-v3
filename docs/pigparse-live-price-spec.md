# PigParse Market Data and Live Loot Price Association

This document specifies how EverQuest Loot Tracker V3 imports PigParse market data and associates a 30-day WTS value with parsed live loot. It is intended to accompany [`live-loot-parser-spec.md`](./live-loot-parser-spec.md) when implementing a live-loot plugin in another system.

## 1. Primary value definition

The application's primary item value is:

```text
PigParse Green server, transaction type 0, 30-day average WTS price, in platinum pieces
```

In the PigParse payload this is `a30` from a row whose `t` is `0`.

Do not substitute the current, 60-day, 90-day, six-month, or all-time average when exact V3 compatibility is required. If the 30-day WTS average is absent or zero, V3 reports no live price.

## 2. Data source

V3 reads the JSON API directly; it does not parse the HTML table at `/ServerIndex/Green`.

```http
GET https://www.pigparse.org/api/item/getall/Green
User-Agent: EverQuestLootTracker/3
Accept: application/json
```

Current implementation details:

- Request timeout: 45 seconds.
- Redirects and normal HTTPS behavior are delegated to the HTTP client.
- Any non-success HTTP status is an error.
- The response root must be a non-empty JSON array.
- An HTTP, JSON, empty-response, or database error must leave the previously stored usable dataset intact.
- Refresh is user-triggered by `market.refresh`; V3 does not automatically refresh prices at startup or on a timer.

The public server-index page is:

```text
https://www.pigparse.org/ServerIndex/Green
```

## 3. PigParse row format

Example API row:

```json
{
  "i": 17454,
  "t": 0,
  "n": "10 Dose Ant's Potion",
  "l": "2026-08-15T23:53:40.365+00:00",
  "tc": 0,
  "ta": 0,
  "t30": 476,
  "a30": 219,
  "t60": 711,
  "a60": 217,
  "t90": 1016,
  "a90": 218,
  "t6m": 3392,
  "a6m": 222,
  "ty": 8244,
  "ay": 219
}
```

Field mapping used by V3:

| API field | Stored meaning | Notes |
|---|---|---|
| `i` | `source_item_id` | PigParse/EverQuest item ID |
| `t` | `transaction_type` | `0` is the WTS row used for primary values |
| `n` | `item_name` | Trim surrounding whitespace; skip the row if empty |
| `l` | `last_seen` | PigParse timestamp/text |
| `tc` | `current_count` | Current-period observations |
| `ta` | `current_average_pp` | Current-period average platinum |
| `t30` | `count_30d` | 30-day sample count |
| `a30` | `average_30d_pp` | Primary V3 value |
| `t60` | `count_60d` | 60-day sample count |
| `a60` | `average_60d_pp` | 60-day average platinum |
| `t90` | `count_90d` | 90-day sample count |
| `a90` | `average_90d_pp` | 90-day average platinum |
| `t6m` | `count_6m` | Six-month sample count |
| `a6m` | `average_6m_pp` | Six-month average platinum |
| `ty` | `count_all` | Long-range/all-history sample count as mapped by V3 |
| `ay` | `average_all_pp` | Long-range/all-history average as mapped by V3 |

Numeric fields may be JSON integers or strings containing base-10 integers. Missing, malformed, non-integer, or null numeric fields become `0`. Missing or non-string text fields become an empty string.

Prices are integer platinum values. Do not interpret them as copper, floating-point currency, or formatted strings.

## 4. Local market table

Equivalent storage schema:

```sql
CREATE TABLE item_market_values (
    server TEXT NOT NULL COLLATE NOCASE,
    source_item_id INTEGER NOT NULL,
    transaction_type INTEGER NOT NULL,
    item_name TEXT NOT NULL COLLATE NOCASE,
    last_seen TEXT NOT NULL,
    current_count INTEGER NOT NULL DEFAULT 0,
    current_average_pp INTEGER NOT NULL DEFAULT 0,
    count_30d INTEGER NOT NULL DEFAULT 0,
    average_30d_pp INTEGER NOT NULL DEFAULT 0,
    count_60d INTEGER NOT NULL DEFAULT 0,
    average_60d_pp INTEGER NOT NULL DEFAULT 0,
    count_90d INTEGER NOT NULL DEFAULT 0,
    average_90d_pp INTEGER NOT NULL DEFAULT 0,
    count_6m INTEGER NOT NULL DEFAULT 0,
    average_6m_pp INTEGER NOT NULL DEFAULT 0,
    count_all INTEGER NOT NULL DEFAULT 0,
    average_all_pp INTEGER NOT NULL DEFAULT 0,
    fetched_at TEXT NOT NULL,
    is_manual INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (server, source_item_id, transaction_type)
);
```

Use an index equivalent to:

```sql
CREATE INDEX idx_market_item_match
ON item_market_values(server, item_name, transaction_type);
```

## 5. Refresh algorithm

Perform the full refresh in one database transaction:

1. Fetch and validate a non-empty JSON array before changing storage.
2. Record one `fetchedAt` timestamp for the whole refresh. V3 uses a local RFC 3339 timestamp.
3. Delete all non-manual rows for the Green server.
4. Preserve rows where `is_manual = 1`.
5. For each payload row:
   - Trim `n`.
   - Skip an empty name.
   - Convert fields using the rules above.
   - Insert it as `server = Green` and `is_manual = 0`.
   - Ignore duplicate primary keys within the response.
6. For every WTS row where `t = 0` and `i > 0`, insert the ID/name into the master item catalog if it does not conflict with an existing authoritative item.
7. Commit only after all rows succeed.

Manual rows with the same `(server, source_item_id, transaction_type)` take precedence because they survive deletion and the incoming PigParse insert is ignored by the primary-key conflict.

The live-price query does not explicitly sort `is_manual` first. If two different item IDs have the same item name, selection still follows `count_30d DESC, last_seen DESC`; a manual row is guaranteed to win only when it occupies the same primary key as the incoming PigParse row or otherwise wins that ordering. A new plugin may choose a clearer explicit manual-override rule, but that is a deliberate improvement over exact V3 behavior.

Reference pseudocode:

```text
payload = GET PigParse Green API
require payload is a non-empty array

transaction:
  delete market rows where server == Green and isManual == false

  for row in payload:
    name = trim(string(row.n))
    if name is empty:
      continue

    market = map abbreviated fields; invalid numbers become zero
    insert market with conflict-ignore

    if market.transactionType == 0 and market.itemId > 0:
      insert master item (market.itemId, name, source=market)
      on conflict: keep existing master item

  commit
```

## 6. Two distinct item associations

V3 deliberately has two related but different association paths.

### A. Live loot to price: exact name association

A parsed loot line contains an item name but no item ID. Live Loot therefore looks directly in `item_market_values` by name.

Compatibility query:

```sql
SELECT average_30d_pp
FROM item_market_values
WHERE server = 'Green' COLLATE NOCASE
  AND transaction_type = 0
  AND item_name = :parsed_item_name COLLATE NOCASE
  AND average_30d_pp > 0
ORDER BY count_30d DESC, last_seen DESC
LIMIT 1;
```

Rules:

- Match the loot parser's final `itemName` exactly, ignoring case.
- Do not use substring, token, fuzzy, plural, or punctuation-insensitive matching.
- SQLite `NOCASE` primarily provides ASCII case folding; do not assume full Unicode normalization.
- Do not trim or rewrite one side only. Normalize consistently before storage and lookup if the plugin chooses to normalize.
- Require `transaction_type = 0`.
- Require `average_30d_pp > 0`.
- If duplicate market rows share a name, prefer the greatest 30-day sample count, then the most recent `last_seen`.
- No match means `null`/unknown, not zero platinum.

The loot parser already removes the optional log-language article `a ` or `an `, which normally leaves the canonical PigParse item name.

### B. Master item association: authoritative ID/name catalog

The separate `master_items` table provides stable item IDs for clickable EverQuest links, inventory reconciliation, recipes, and administration:

```sql
CREATE TABLE master_items (
    item_id INTEGER PRIMARY KEY,
    item_name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    source TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

Sources and precedence in normal use:

1. Inventory exports are authoritative because the item ID appears in the game-generated file.
2. Manual corrections are explicitly maintained by the user.
3. PigParse WTS rows seed missing catalog entries with source `market`.

An inventory import removes conflicting ID/name pairs, installs the exported pair with source `inventory`, and propagates that ID to matching WTS and recipe records. PigParse refresh uses conflict-ignore and therefore does not overwrite that established catalog pair.

Important: the current Live Loot price lookup does **not** require or consult `master_items`. A plugin may return the associated master ID as extra enrichment, but it must still support name-only pricing for exact V3 behavior.

## 7. Enriching a live loot event

Given the normalized event from the live-loot parser:

```json
{
  "eventType": "loot",
  "itemName": "Tears of Prexus",
  "looterName": "Youngman",
  "mobName": "a mortiferous golem",
  "groupMembers": ["Youngman", "Posed"]
}
```

Enrich it without changing the original parsed fields:

```json
{
  "eventType": "loot",
  "itemName": "Tears of Prexus",
  "looterName": "Youngman",
  "mobName": "a mortiferous golem",
  "groupMembers": ["Youngman", "Posed"],
  "market": {
    "server": "Green",
    "transactionType": "WTS",
    "itemId": 13047,
    "average30dPp": 1250,
    "count30d": 8,
    "lastSeen": "2026-08-20T03:21:00.000+00:00",
    "fetchedAt": "2026-08-24T09:00:00-06:00",
    "matchedBy": "exact-name"
  }
}
```

The values above illustrate the contract and are not a current quote for that item.

If no qualifying row exists:

```json
{
  "market": null
}
```

Recommended UI output:

- Display `average30dPp` as `1,250 pp`.
- Display unknown as `—` or `No 30-day WTS value`, never `0 pp`.
- Optionally expose `count30d` as a confidence/liquidity hint.
- Surface `fetchedAt` so stale local data is distinguishable from a live network quote.
- Do not block or delay the loot notification on a PigParse HTTP request; enrich from the local cache.

## 8. Live versus snapshot pricing

V3 handles the price differently depending on record type:

- Recent Live Loot resolves the current cached 30-day WTS value when the application snapshot is queried. Refreshing PigParse can therefore change the displayed value for an existing recent drop.
- Adding loot to the split list normally continues to expose the current market value unless the user enters a payout override.
- Tracking a loot item copies the current `average_30d_pp` into `tracked_loot_items.value_pp`. That tracked value is a historical snapshot and does not change after later PigParse refreshes.
- Sold/consumed history stores its own finalized value.

A plugin should decide explicitly whether it wants a dynamic current value, a drop-time snapshot, or both. A useful model is:

```json
{
  "marketValueAtDropPp": 1250,
  "currentMarketValuePp": 1400,
  "marketValueFetchedAt": "2026-08-24T09:00:00-06:00"
}
```

## 9. Failure and staleness behavior

- Keep the last successful local dataset when the request fails.
- Never clear prices merely because PigParse is temporarily unavailable.
- Roll back the replacement transaction if any database operation fails.
- Record/log the HTTP status and error without including secrets or unrelated response data.
- A successful refresh with a non-empty array may legitimately contain items whose `a30` is zero; those items have no primary live price.
- The current V3 UI does not enforce a freshness cutoff. A plugin may warn on age, but should continue showing the cached value with its timestamp.

## 10. Minimum compatibility tests

An implementation should test at least:

1. A Green `t = 0` exact-name row with positive `a30` enriches loot.
2. A `t = 1` row is not used as the primary WTS value.
3. Matching is case-insensitive.
4. Partial and fuzzy names do not match.
5. `a30 = 0` produces unknown rather than `0 pp`.
6. Duplicate names prefer greater `t30`, then newer `l`.
7. Numeric strings parse as integers.
8. Invalid or missing numeric fields become zero.
9. Empty item names are skipped.
10. An empty or non-array payload does not replace cached data.
11. An HTTP failure does not replace cached data.
12. A database failure rolls back the refresh.
13. Manual market rows survive refresh.
14. Inventory-authoritative master IDs are not overwritten by PigParse.
15. A loot event can receive a price without a master-item record.
16. A tracked-loot action snapshots the current value.
17. Later refreshes alter dynamic Live Loot pricing but not an existing tracked snapshot.

## 11. Source-of-truth implementation

This specification reflects these V3 files at release `v3.3.0`:

- `src-tauri/src/application/services.rs` — PigParse HTTP request, payload conversion, and transactional refresh.
- `src-tauri/src/application/data.rs` — live-loot price association, item editing, inventory authority, and tracked-value snapshots.
- `src-tauri/src/migrations/000_v2_compatibility.sql` — market table and name-match index.
- `src-tauri/src/migrations/003_master_items.sql` — master item catalog and association backfill.

If the API changes, preserve the normalized contract described here and update the field adapter. If deployed code and this document disagree, the deployed version's code is authoritative.
