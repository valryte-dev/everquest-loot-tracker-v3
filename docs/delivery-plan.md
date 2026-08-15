# Delivery plan

The rewrite is organized by complete vertical workflows rather than page mockups. A slice is complete only when its Rust use cases, SQLite migration, Tauri contracts, React interface, tests, diagnostics, and migration behavior are all present.

## Slice 1 — Runtime and live loot

- Cross-platform data, config, log, and EverQuest folder selection.
- Shared-access `eqlog_*` tailing with active-character switching.
- Loot, mob, group join/leave/tell parsing and group snapshots.
- Current group editing, remembered names, aliases, loot edits, filtering, sorting, selection, and deletion.
- Structured rolling diagnostics and watcher status.

## Slice 2 — Items, inventories, and characters

- PigParse Green master item values and protected manual corrections.
- Inventory and spellbook watcher/import API lifecycle with returned review URL.
- Equipment, carried, banked, cards, spells, values, and roster-wide summaries.
- Recipe readiness at character and roster scope.

## Slice 3 — Splits and lifecycle

- Active split list from loot or manual entry.
- Holder, looter, mob, participants, aliases, values, and payout summaries.
- Sold/consumed history with notes and complete grid controls.

## Slice 4 — Compounds and WTS

- Master-linked compound outputs and components.
- Multi-template projects, saved recipes, contribution ownership, missing items, and estimated values.
- Character WTS groups, inventory scopes, direct compound handoff, and safe Page 10 clickable-link export.

## Slice 5 — Web, migration, and release

- Read-only loopback dashboard backed by application queries.
- V2 database/settings/workspace migration and backup/restore.
- Help, changelog, accessibility review, performance budgets, and signed update delivery.
