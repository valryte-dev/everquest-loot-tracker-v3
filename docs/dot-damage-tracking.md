# Damage-over-time tracking

## Purpose

EverQuest logs report direct damage, but many classic damage-over-time effects only report the spell landing. The tracker enriches those landing messages with the separately cached Project 1999 spell catalog, infers six-second ticks, and writes the inferred damage through the same encounter summaries used by ordinary melee and spell damage.

## Spell catalog fields

The separate `spell-info.db` catalog stores the original wiki fields plus:

- `cast_on_you`, `cast_on_other`, `wears_off`, and catalog `casting_time` used for local-cast resolution
- `damage_kind`: `dot`, `direct`, `hybrid`, or `non_damage`
- `direct_damage` for fixed or ranged direct-hitpoint effects
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

A glow message is never treated as evidence that damage occurred. Glows may represent direct damage, buffs, or other effects. The tracker creates a DoT application only when a separate landing line matches a cached spell-database profile categorized as `dot` or `hybrid`, with valid per-tick damage and duration fields.

Caster attribution is conservative:

1. An item-glow message within three seconds attributes the landing only when the glow owner is the active log character.
2. Otherwise, an exact `You begin casting <Spell Name>.` creates a pending local cast whose expected resolution timestamp is the log timestamp plus that spell's catalog casting time. A matching landing within two seconds of that expected time attributes the landing to the active log character; a matching resist closes the pending cast with no damage. The conservative 15-second window is used only when the cached spell record has no parseable casting time.
3. Otherwise, the landing remains memory-only for exactly the next log line. If a same-target generic non-melee line immediately preceded the landing, the next same-target combat action confirms an active-character proc using that logged amount. Without preceding non-melee evidence, the attacker on the next same-target combat action becomes the probable proccer, including an observed player.
4. An intervening line, a different target, or a non-melee event cancels proc attribution for that landing.
5. If none of these clues produces a confirmation, the landing is discarded without creating spell activity, a DoT application, proc damage, or an encounter.

New attribution is persisted as `item_glow`, `direct_cast`, or `proc` and shown in the live data contract. The schema retains `unknown` compatibility for older records. `Your spell is interrupted.` cancels both the pending direct-cast clue and any pending item-glow clue, so an interrupted attempt cannot be credited as a landing, proc, DoT application, or damage. Item glow retains priority because it explicitly identifies the owner; direct casting requires an exact spell-name match. Proc inference is intentionally adjacency-based and requires the same normalized target, so unrelated lines and simultaneous fights do not cross-attribute the effect.

## Proc tracking

A spell landing is counted as a weapon proc only when it has no valid glow or direct-cast clue and the immediately following line is a same-target melee hit, observed melee hit, or riposte attempt. Immediately preceding same-target generic non-melee damage forces attribution to the active character and supplies the proc amount. Otherwise the confirming action's attacker is the probable proccer. Direct casts and glow-attributed effects never increment proc counts.

Both direct-damage and DoT procs are stored in `proc_occurrences` using the landing file/offset/spell as an idempotency key. Direct proc damage uses the immediately preceding logged non-melee amount when present; otherwise it falls back to fixed catalog damage. It is written once to `damage_events` with a deterministic `proc://<occurrence-id>` source. The following weapon hit remains an independent melee event. DoT proc ticks retain `dot://<application-id>` identities and are included in the proc's cumulative damage without duplicating the normal DoT total.

`damage_spell_summaries` is the incremental read model by encounter, caster, and spell. It stores proc count, inferred direct-proc damage, all calculated DoT damage, tick count, and the subset of DoT damage caused by procs. Page reads are bounded by the requested encounter IDs and never scan the full event history.

## Damage-source explanations

Combat displays describe the most specific source justified by the available evidence. Known weapon-to-proc associations produce labels such as Essence Mace / proc DD / Essence Tap and Great Spear of Dawn / proc DoT / Dawncall. Unknown proc weapons remain labeled as unknown or, for the active character, only as stale-capable candidates drawn from the most recent primary/secondary inventory snapshot. Direct casts, item-glow attribution, calculated DoT ticks, explicitly named spell damage, confirmed-local generic non-melee damage, and melee hits each retain their distinct source explanation. Unattributed generic non-melee damage is discarded.

Every explanation carries a confidence level and evidence tooltip. The UI must not claim that one of two equipped weapons caused a melee hit when an older inventory snapshot merely recorded them, and it must not invent a spell name for generic non-melee damage. Source explanations are intentionally limited to recorded encounter details, the Proc and DoT Training Lab, and battle replay. Live and docked colored fighter bars remain compact and show only combat metrics.

The live page, detached/docked meter, recorded-encounter grid, and encounter detail show proc and DoT totals. Direct-cast and item-click categorization is limited to the active character. Proc attribution can include observed players when their same-target combat action immediately follows a recognized landing; ordinary explicit melee and damage tracking remains unchanged.
## Tick and refresh behavior

- The first inferred tick occurs six seconds after the landing.
- Reapplying the same spell to the same target marks the previous application `refreshed`; it does not stack.
- Future ticks belong to the newly attributed caster after a refresh.
- A mob death or player death ends relevant active applications.
- If the client emits explicit named DoT damage, inference for that application is disabled before due ticks are calculated, preventing double-counting.
- Inferred ticks use deterministic application/tick keys, so rescans and repeated processing are idempotent.

## Persistence and read model

The main database migration `027_dot_damage_tracking.sql` adds `dot_applications`. Migration `028_repair_dot_event_timestamps.sql` repairs the timestamp-column defect present in the first local preview build and rebuilds only affected participant summaries. It is additive and references existing `damage_encounters`. Inferred ticks are written into `damage_events`; existing participant summaries, totals, charts, and DPS calculations therefore include them without parallel aggregation logic.

Damage page, global combat, and encounter-detail snapshots attach only active DoTs as `activeDots`. The UI shows spell, caster, target, damage per tick, ticks remaining, next-tick countdown, and remaining-duration progress.

Schema 30 adds the append-only `combat_spell_activity` table for confirmed catalog landings. It retains the encounter, caster, target, spell, time, and normalized source (`direct`, `proc`, or `item_click`); `unknown` remains schema-compatible for legacy rows, and item-click rows retain the item named by the preceding `begins to glow` message. Damage snapshots query at most the 24 newest activity rows per requested encounter.

A healer may not produce an outgoing attack that identifies the primary target. The live Damage Tracker therefore provides a character-scoped target selector across its currently active encounters. A locked target is validated by the backend, ordered first on the Damage page, detached/docked meter, and cross-page combat strip, and can be returned to automatic selection in one click. The preference changes presentation priority only; damage remains associated with the explicit target or attacker named by each log event, preserving simultaneous encounters. Target locks clear at application startup and character switches, and cease to affect ordering as soon as the selected encounter is no longer live.

Live fight cards and the detached/docked meter reuse the same spell activity controls. Player ranking bars sort by known persisted proc/DoT spell damage, then landing count, while source counters remain separate. The recent activity feed shows each confirmed spell, its target and attributed caster, and whether attribution came from a direct cast, attack-confirmed proc, or item click.

## Complete Heal chain sharing

The live guild Complete Heal chain includes an icon-only copy action for Discord. It exports the current target mob plus anonymous operational metrics: call count, number of distinct healers, elapsed chain duration, average and median gaps, minimum-to-maximum gap range, standard deviation, count of gaps over twelve seconds, and the recent numbered cadence. The export deliberately excludes cleric names, active-character names, heal targets, timestamps, channels, messages, and source-file paths.
## Training lab

The Combat navigation includes a **Proc & DoT Training Lab**. It accepts pasted, complete EverQuest log lines plus the character represented by `You`, then runs the production envelope parser, combat event parser, spell landing matcher, caster attribution, encounter writer, and tick inference against a disposable temporary database.

The lab never writes to the real loot-tracker database. It displays line-by-line parser decisions, ignored and invalid lines, matched spell profiles, target and caster attribution, refresh state, attack-confirmed proc records, inferred direct proc damage, inferred DoT ticks versus explicit events, cumulative damage, and damage grouped by caster. Optional projection advances recognized active DoTs through their scheduled expiration solely for visualization. Review markers and notes remain local to the page.

After analysis, the lab builds a deterministic battle replay from the returned timestamped damage events. Play, pause, restart, previous-time, next-time, speed, and scrub controls reveal explicit damage, proc damage, and inferred DoT ticks cumulatively. Both live encounters and replay encounters render the same typed `DamageFighterBars` control, so future layout and accessibility changes apply to both surfaces. Each encounter is shown with themed fighter bars containing: player identity, prominent combat time, incoming damage, contribution, DPS, total damage, and separated proc/direct-cast damage totals. Fighter combat duration uses compact unit labels (`10s`, `5m 10s`, `1h 5m 10s`) and omits only leading zero units. Source evidence stays in the replay interpretation strip and recorded fight detail instead of consuming space inside the colored bars. Replay is frontend-only and never mutates either the disposable simulation or the user's database. The shared docked and detached live meter places cumulative damage beside a 30-second rolling DPS chart, preserves fighter-bar colors across both charts, reports current values, and limits the visible trend to the latest two minutes for stable rendering during long encounters.
## Pet and summoned-fighter damage

A pet command such as `Treasure Chest tells you, 'Attacking Grink Master.'` records typed, character-scoped pet evidence. Multi-word pet names are supported. Subsequent melee by that pet is attributed to a separate fighter in the same encounter; no owner is guessed.

When no pet command is available, an otherwise-unknown attacker hitting a target that is already an active encounter is also retained as a separate outgoing fighter. Active-target identity has priority over stale or simultaneous encounters named after the attacker, so every recognized hit against X appears on the DPS side of the active X encounter. This fallback covers other players' pets. When an active encounter mob attacks either a pet identified by its command or any fighter already contributing against that mob, the hit is retained as incoming damage against that fighter; pet ownership is never guessed. Schema 31 stores only the derived, rebuildable `combat_pet_evidence`; Damage Tracker rescans clear and reconstruct it from the logs.
## Rescans

The manual Damage Tracker rescan rebuilds derived damage, incoming damage, CH calls, encounters, and DoT applications from configured logs. Source loot, history, splits, inventory, spell catalog, and market data are not changed.

## Known limits

- Landing messages without the wiki `Someone` placeholder cannot reliably identify a target and are not inferred.
- Ambiguous duplicate landing messages are skipped.
- Variable or level-scaled values use the parsed upper damage value when the wiki presents a supported `X ... to Y ... per tick` effect.
- A spell with no parseable numeric tick count or damage remains cataloged but is not inferred in combat.

## Saved fight replay library

Any recorded Damage Tracker encounter can be saved from its row action into the training lab's external replay library. Replay files use the versioned, portable `.eqfight.json` format and are stored outside SQLite in the application's `combat-replays` data directory. The file retains the active character, source metadata, summary totals, optional notes, and—most importantly—the original raw log window used as parser input. The capture includes twelve lines before and after the encounter offsets so proc landings, item clicks, misses, blocks, and simultaneous-fight noise remain available for future parser training.

The lab lists replay files in a filterable, sortable grid with character, mob, time, damage, participants, events, proc and DoT totals, capture quality, and saved time. Loading a replay replaces the editor contents and immediately runs the current production parser, rather than trusting old derived totals. Users can import shared replay files with the native file picker. Imported files are validated and copied into the library with collision-safe names.

If the original EverQuest log no longer exists, saving falls back to raw outgoing and incoming combat lines retained in SQLite. The replay is labeled `Reduced context` because landing and attribution clues may be absent. The fallback does not invent missing lines or alter the source database.
## Proc Coach

CH encounter history is grouped into one row per fifteen-second chain session and log character. Opening an encounter shows its individual calls and supports an interactive play, pause, step, reset, speed, and scrub replay. Live and historical encounters can be saved as portable `.eqch.json` files in the application data directory under `ch-chain-replays`; the saved replay library is filterable and sortable, and loading a file does not mutate SQLite. After an encounter is concluded by mob death, a later chain, or fifteen seconds without another call, its detail and replay views calculate each cleric's population standard deviation from that cleric's own consecutive-call rotation intervals. Active encounters show the analysis as pending so partial data is not presented as final. The live panel also renders a compact Chain Pulse chart over the most recent eighteen gaps. It uses the observed median as a robust learned cadence, shades a twenty-percent tolerance band, classifies early/stable/late calls, projects the currently open gap, reports pace drift, and derives a coefficient-of-variation stability score without imposing a hard-coded raid cadence.

The separate Complete Heal Chain Lab reuses the production guild-CH parser and the exact live `ClericChainPulse` component. It provides editable pasted input, a realistic sample, line-by-line parser interpretation, and a controllable open-gap clock for previewing learned, on-pace, due, and overdue visual states without writing to SQLite.

The training lab includes an optional, supervised Proc Coach for reviewing difficult proc attribution sequences.

- The deterministic Rust parser remains authoritative for live encounters. Agent findings never modify parser rules or combat records automatically.
- Analysis runs only after the user clicks **Analyze with Agent**. It is not part of the watcher, backlog scanner, startup path, or live damage hot path.
- Exact pasted or replay log lines and the deterministic parser report are sent to the OpenAI Responses API for that requested review. Response storage is disabled (`store: false`).
- The user's OpenAI API key is stored by the operating-system credential vault and is never written to SQLite, application settings, replay files, or logs.
- Results use a strict typed schema: classification, likely caster, spell, target, evidence, confidence, parser agreement, damage, and a proposed correction.
- Every result is suggest-only. The user can edit the classification, caster, spell, and note, then explicitly accept or reject it.
- Reviewed decisions can be appended to a loaded `.eqfight.json` file. Older replay files remain readable because `coachReviews` defaults to an empty list.
- Replay updates use a temporary file and recoverable replacement so the prior replay can be restored if writing the updated file fails.

Saved reviews are training evidence for later parser work; the production parser does not learn from them automatically. Parser changes must still be implemented as deterministic rules with fixture tests and normal verification.

CH encounter history is connected directly to the Complete Heal Chain Lab. Any in-database history row can be handed to the lab without first creating a duplicate replay file, while the lab also lists the canonical external `.eqch.json` replay library. Loaded encounters retain their original call timing and support play, pause, previous/next call stepping, speed selection, and timeline scrubbing through the shared live `ClericChainPulse` visualization.

The shared CH cadence graph defaults to a 0–15 second vertical scale. Its compact numeric control accepts a manual upper bound from 1 to 600 seconds for edge cases; because the control lives inside `ClericChainPulse`, the same behavior is available in live CH tracking and both lab/history replay surfaces.
