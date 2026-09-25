# Spell Research Catalog

The **Roster → Spell Research** workspace is an offline inventory tracker and recipe-readiness view for classic Project 1999 spell research.

## Catalog coverage

The maintained snapshot is `src-tauri/assets/p99-spell-research.tsv`. It contains 117 spell recipes and 275 normalized component requirements from the Project 1999 [Skill Research](https://wiki.project1999.com/Research) reference:

- 28 Enchanter recipes using pages and half-pages
- 35 Magician recipes using words, spell-scroll chains, and supporting items
- 30 Necromancer recipes using words
- 24 Wizard recipes using runes

The snapshot includes vendor-available combines as well as research-only spells so every useful dropped word, page, and rune can be evaluated. Practice runes are not spell recipes and are intentionally outside this catalog.

## Inventory behavior

The page reads the same canonical inventory snapshots and master item identities used elsewhere in the app. Holdings are combined across characters while preserving holder and location details. Recipes account for quantities—for example, identically named left/right page halves become a requirement for two copies rather than two indistinguishable rows.

The page offers:

- recipe readiness grouped by research class;
- class, spell-level, component-family, held, ready, and research-only filters;
- the number of combines currently possible;
- a component inventory showing every recipe that uses each item;
- shared item hover cards and stored market values.

## Maintenance

Runtime behavior never scrapes the wiki. To deliberately refresh the reviewed snapshot, run:

```powershell
python tools/scrape_spell_research.py
```

Review the TSV diff and run the catalog tests before shipping. The scraper preserves wiki provenance and normalizes spell ingredients to inventory names such as `Spell: Minor Summoning: Air`.