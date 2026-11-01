# Character Spell Collection

The Character Details **Spells** tab compares a selected character's imported spellbook and inventory against the local Project 1999 spell catalog.

## Data ownership

- Scribed state comes from the latest imported `*-Spellbook.txt` snapshot.
- Held scroll counts come from the latest imported character inventory snapshot.
- Spell metadata, including the authoritative P99 `spellicon` letter, comes from the separate local `spell-info.db` catalog.
- If Character Details opens while that catalog is refreshing, the spell list watches synchronization and reloads automatically when it completes; it does not retain the incomplete startup snapshot.
- Castable eligibility and class level come from the eleven P99 caster class pages (including the Bard song table), not merely from each spell page. Catalog refresh atomically replaces class assignments, so proc effects, recourses, NPC spells, and other internal records remain available to combat parsing without appearing as learnable character spells.
- Character class and level come from the normalized character profile.

The feature does not duplicate spell metadata in the main application database and does not add a migration. The catalog is requested only when the Spells tab is mounted and is cached for later visits during the session.

## Collection states

Each class spell has exactly one state:

- **Scribed**: the normalized spell name appears in the character spellbook.
- **Scroll held**: it is not scribed, but one or more matching `Spell:` inventory items are held.
- **Missing**: neither a spellbook entry nor an inventory scroll is present.

Names are matched case-insensitively after removing the `Spell:` prefix, a terminal period, extra whitespace, and normalizing common apostrophe variants. There is no fuzzy matching, which prevents similarly named spells from being combined.

## UI behavior

The summary shows catalog coverage, scribed count, ready scrolls, and missing count. Users can filter by collection state, switch between spells usable at the current level and all class levels, use the shared grid text filter, and sort every column. Spell names reuse the shared spell detail hover card.

Each spell name includes its locally bundled P99 spell-gem icon. Displayed fields include level, type, target, mana, school, casting time, duration, resist, a compact wiki lookup action, and a one-line description with the complete text available on hover. If class is unknown, the tab directs the user to set it in the Inventory tab. If level is unknown, the view defaults to all class levels.
