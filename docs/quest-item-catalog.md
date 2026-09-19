# Quest Item Catalog

## Purpose

The Quest Item Readiness workspace compares imported inventory and bank data across the whole roster with three reviewed Project 1999 wiki catalogs:

- Plane of Sky class test requirements
- Thurgadin, Kael, and Skyshrine armor turn-in pieces (armor only; gems remain in the Armor Gems workspace)
- Class epic checklist items

The page loads the quest catalog and inventory only when its route is active. No wiki request runs during application startup, log parsing, inventory watching, combat, or page rendering.

## Bundled snapshot

The maintained source snapshot is `src-tauri/assets/p99-quest-items.tsv`. It currently contains:

- 272 Plane of Sky component rows across all 14 classes
- 294 Velious armor class/faction/slot rows across all 14 classes
- 351 epic checklist component rows across all 14 classes
- 296 validated, locally bundled P99 item icons

Every candidate wiki link is fetched and accepted only when its page contains the P99 `Itempage` template. This excludes NPCs, zones, spells, and walkthrough-only links. Legacy wiki titles are checked with apostrophe/backtick spelling variants before they are rejected. `Token of Mastery` is a reviewed exception because the Magician epic checklist grants it but the wiki has no item page. Captured item names are always retained; nullable `master_items.item_id` values are resolved through the shared item-name resolver.

## Maintainer refresh

Run from the project root:

```powershell
python tools/scrape_quest_catalog.py
```

The script uses the MediaWiki API, validates linked item pages, writes a deterministic TSV, and downloads missing item icons. It keeps a resumable cache in the operating-system temporary directory. Review the generated diff and run the complete test suite before shipping a refreshed snapshot.

## UI behavior

The workspace provides category tabs, a class selector, text filtering with a one-click clear control, readiness/name/class sorting, owned-only and ready-only filters, and expand/collapse-all controls. Compact cards show reward imagery, completion progress, and component previews. Expanded cards show stored values plus character, quantity, and carried/banked location details.

Repeated requirements shared by multiple quests are counted once in the page-level owned-value summary. Quest readiness remains scoped to each quest.
