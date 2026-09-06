# Damage-over-time tracking

## Purpose

EverQuest logs report direct damage, but many classic damage-over-time effects only report the spell landing. The tracker enriches those landing messages with the separately cached Project 1999 spell catalog, infers six-second ticks, and writes the inferred damage through the same encounter summaries used by ordinary melee and spell damage.

## Spell catalog fields

The separate `spell-info.db` catalog stores the original wiki fields plus:

- `cast_on_you`, `cast_on_other`, and `wears_off`
- `damage_kind`: `dot`, `direct`, `hybrid`, or `non_damage`
- `damage_per_tick`, `tick_count`, and `tick_interval_seconds`
- `total_dot_damage`

A spell is classified as a DoT when an effect contains `Decrease Hitpoints ... per tick`. A separate `Decrease Hitpoints` effect without `per tick` classifies direct damage. Spells with both are hybrid. The tick interval is six seconds.

Existing spell databases are migrated in place. The migration clears only the catalog refresh timestamp, causing one background wiki refresh to populate the new fields. It does not delete cached spell records.

## Landing recognition

`Cast on Other` is converted to an anchored, case-insensitive matcher. The wiki placeholder `Someone` captures the target name. For example:

```text
Someone staggers as the light of dawn washes over it.
```

matches:

```text
Hexbone skeleton staggers as the light of dawn washes over it.
```

If no profile matches, or more than one profile matches the same line, no DoT is inferred. This deliberately avoids false damage.

## Caster attribution

Caster attribution is conservative:

1. An item-glow message within three seconds attributes the landing to the glow owner. A local or ownerless glow uses the active log character.
2. Otherwise, the first player attack or riposte attempt against the same target within five seconds supplies the caster.
3. If neither clue exists, the caster remains `Unknown`.

Attribution method is persisted as `item_glow`, `next_attack`, or `unknown` and shown in the live data contract.

## Tick and refresh behavior

- The first inferred tick occurs six seconds after the landing.
- Reapplying the same spell to the same target marks the previous application `refreshed`; it does not stack.
- Future ticks belong to the newly attributed caster after a refresh.
- A mob death or player death ends relevant active applications.
- If the client emits explicit named DoT damage, inference for that application is disabled before due ticks are calculated, preventing double-counting.
- Inferred ticks use deterministic application/tick keys, so rescans and repeated processing are idempotent.

## Persistence and read model

The main database migration `027_dot_damage_tracking.sql` adds `dot_applications`. It is additive and references existing `damage_encounters`. Inferred ticks are written into `damage_events`; existing participant summaries, totals, charts, and DPS calculations therefore include them without parallel aggregation logic.

Damage page, global combat, and encounter-detail snapshots attach only active DoTs as `activeDots`. The UI shows spell, caster, target, damage per tick, ticks remaining, next-tick countdown, and remaining-duration progress.

## Rescans

The manual Damage Tracker rescan rebuilds derived damage, incoming damage, CH calls, encounters, and DoT applications from configured logs. Source loot, history, splits, inventory, spell catalog, and market data are not changed.

## Known limits

- Landing messages without the wiki `Someone` placeholder cannot reliably identify a target and are not inferred.
- Ambiguous duplicate landing messages are skipped.
- Variable or level-scaled values use the parsed upper damage value when the wiki presents a supported `X ... to Y ... per tick` effect.
- A spell with no parseable numeric tick count or damage remains cataloged but is not inferred in combat.