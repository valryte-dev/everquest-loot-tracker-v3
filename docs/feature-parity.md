# V2 to V3 feature parity

| Area | Required V3 capability | Phase |
|---|---|---:|
| Live logs | Active `eqlog_*` switching, shared tailing, character swaps, loot/mob/group parsing | 1 |
| Group | Active character inclusion, join/leave/tell inference, remembered roster, manual correction | 1 |
| Loot grid | Value/mob/looter/shared-by editing, filters, all-column sorting, bulk selection/deletion | 1 |
| Diagnostics | Rolling logs, levels, filter with clear, pause/copy/export | 1 |
| Exports/import | `*-Inventory.txt` and `*-Spellbook.txt` watcher, API POST/PUT claim lifecycle, returned planner URL | 2 |
| Characters | Equipment, carried/banked, cards, spells, roster summaries, estimates, recipes | 2 |
| Master items | IDs, protected manual corrections, PigParse 30-day WTS, exact matching | 2 |
| Splits | Manual/loot entries, mob/holder/looter/shared-by, aliases, payout summaries | 2 |
| History | Sold/consumed state, value, note, filter/sort/select/bulk delete | 2 |
| Compounds | Multi-template projects, master-linked output/components, ownership, saved templates | 3 |
| WTS | Character groups, inventory scopes, compact editing, clipboard text, clickable INI links | 3 |
| Web | Read-only local dashboard for loot, splits, history, and compounds | 3 |
| UX | Five themes, consistent shell, no mock-data flash, every grid filter/sort/clear rule | 4 |
| Product | Help, changelog, migration UI, backup/restore, signed native installers | 4 |
