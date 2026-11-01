# Wardrobe item catalog

The Wardrobe page uses an embedded, read-only Project 1999 item catalog. It is intentionally separate from the user's writable loot database: catalog searches cannot lock or enlarge the operational database, while market prices still resolve through the application's shared master-item list.

## Source and snapshot

- Source: the public P99 Planner item database, whose item content is parsed from the community-maintained Project 1999 Wiki.
- Embedded item asset: `src-tauri/assets/p99-item-catalog.sqlite`
- Embedded set asset: `src-tauri/assets/p99-item-sets.tsv`
- Snapshot identifier: `20260907221018487`
- Item count at import: 12,122
- Equipment sets at import: 308 sets containing 2,303 item/slot mappings

The desktop backend materializes the embedded bytes into a versioned temporary read-only SQLite file. Wardrobe queries are scoped by slot and, by default, the selected class and race. The frontend can then combine multiple equation-based stat filters (`>=`, `<=`, `=`, `>`, and `<`) without mutating either database.

The set catalog preserves P99 Planner's named item-to-slot groups rather than inferring sets from similar item names. This includes armor and equipment collections such as Haze Panther Armor, Nathsar Armor, and Netted Kelp Armor. Applying a set resolves every piece through the same catalog, excludes pieces incompatible with the selected race/class, and uses the normal item rendering path.

## Identity and rendering

`peqId` is treated as the canonical game item ID when available. It is used for master-market-value association and passed to the existing character model renderer. The catalog's icon, material, `idfile`, color, and item type fields feed the same renderer used by Character Details.

## Quick Compare

The Wardrobe draft keeps a local Quick Compare shortlist alongside the equipped outfit. Candidates are chosen through the same slot-, race-, class-, set-, tradeability-, handedness-, stat-filter-, and sort-aware catalog picker used for direct equipment changes.

Each candidate can be enabled or disabled, assigned High, Normal, or Low conflict priority, annotated with a note, and removed without changing the equipped model. The comparison resolves all enabled candidates into one simulated loadout, shows per-item changes against the displaced slot, and reports combined combat, attribute, resist, weapon, weight, and market-value totals with signed deltas. Market values use the shared master-item snapshot by canonical `peqId` first and normalized name fallback; the estimated upgrade cost includes only enabled winning candidates and reports missing-price coverage. A winning two-handed primary candidate clears the simulated secondary slot. **Apply to wardrobe** is the only action that replaces the equipped draft; browsing and editing the shortlist remain non-destructive.

The shortlist is stored in the existing local Wardrobe draft. Older drafts are normalized with an empty shortlist on read, and Character Details transfers start with an empty shortlist so copied equipment remains backward compatible.
## Refreshing the catalog

Replace the embedded SQLite and set assets only from a verified P99 Planner data snapshot, update the snapshot identifier and counts above, then run:

1. `cargo test wardrobe_catalog --lib`
2. `npm test`
3. `npm run build`
4. `npm run build:standalone`

Do not merge Wardrobe rows into `master_items`; the master list remains the canonical local identity/value layer, while this catalog is reference metadata for equipment discovery.
