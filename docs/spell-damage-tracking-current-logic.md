# Spell damage tracking: current implemented logic

This describes what V3 does today. It is an audit reference, not a claim that every rule is correct. **Audit flag** marks behavior likely worth reviewing.

## Processing pipeline

Spell damage combines:

1. EverQuest events parsed in `domain/log_events.rs`.
2. Project 1999 metadata cached separately in `spell-info.db` by `infrastructure/spell_catalog.rs`.
3. A bundled, versioned item-to-proc snapshot from the [Project 1999 Weapon Procs table](https://wiki.project1999.com/Weapon_Procs), reconciled to master item IDs by `infrastructure/proc_catalog.rs`.
4. Stateful attribution and inference in `application/dot_tracking.rs`.
5. Canonical damage writes and spell summaries in `application/runtime.rs` and `application/combat_metrics.rs`.

For every complete timestamped line, the ordinary event is persisted first. The spell tracker then consumes the same parsed event, resolves prior clues, flushes inferred ticks due at that timestamp, and evaluates spell landing text.

## Catalog classification

The wiki import retains spell name, effects, duration, casting time, `Cast on You`, `Cast on Other`, and `Wears Off`.

Names are normalized by removing leading `Spell:`, converting backticks and known malformed apostrophe encodings to `'`, removing a trailing period, and trimming whitespace.

| `damage_kind` | Current rule |
| --- | --- |
| `dot` | An effect contains `Decrease Hitpoints ... by N ... per tick`, with no separate direct effect. |
| `direct` | An effect contains `Decrease Hitpoints ... by N` or `Decrease HP when cast by N` without `per tick`, with no DoT effect. |
| `hybrid` | Both forms were parsed. |
| `non_damage` | Neither supported form was parsed. |

Numeric rules:

- Only positive integer damage is parsed.
- A supported `X (...) to Y` expression captures `Y`.
- Duration must contain literal `N tick` or `N ticks` text.
- Tick interval is always six seconds.
- Total DoT estimate is damage per tick multiplied by tick count.

Older wiki records with empty structured slots receive a deliberately narrow description fallback when they explicitly state a numeric amount of damage and do not describe tick cadence, reactive damage, or absorption. The derived spell cache is reclassified locally at combat schema version 3; no network refresh is required. Only `direct`, `dot`, and `hybrid` entries with non-empty `Cast on Other` and usable damage data become combat profiles, plus explicit safety rules such as an already-known proc-only effect.

**Audit flags:** formula-based, level-scaled, negative, or unusually worded effects can fail classification; non-literal durations produce no inferred DoT; only the first supported direct and per-tick effects are used.

## Recognized spell log events

### Local cast

```text
You begin casting <Spell Name>.
```

This exact, case-sensitive shape creates one pending local cast. A new cast replaces the prior pending cast.

### Local interruption

```text
Your spell is interrupted.
```

This exact, case-sensitive shape clears both pending local-cast and item-glow clues. It creates no damage.

### Local resist

```text
Your target resisted the <Spell Name> spell.
<Target> resisted your <Spell Name> spell.
```

A resist clears the pending cast only if spell name and expected resolution time match. The parsed target is not used by the current attribution state machine.

### Item glow

```text
<Item> begins to glow.
Your <Item> begins to glow.
<SingleTokenOwner>'s <Item> begins to glow.
```

`Your` and ownerless glows parse as the active character. Named owners are limited to a single token, but only a glow owned by the active character is eligible for spell attribution. A glow is only a clue and never creates damage by itself.

### Explicit outgoing damage

```text
<Target> was hit by non-melee for N points of damage.
<Target> has taken N damage from your <Spell Name>.
```

A named `has taken ... from your <Spell>` line is canonical active-character damage and retains its spell name. A generic non-melee line immediately before a unique catalog landing is retained in tracker memory as active-character proc evidence; after the next same-target combat action confirms the proc, its logged amount becomes the proc damage. Separately, a generic non-melee line after a confirmed direct-cast or item-click landing during the previous six seconds is persisted under that spell name. Other unattributed generic lines are discarded.

**Audit flag:** the six-second direct-cast/item-click association window can select the wrong spell if the active character lands multiple effects on the same target close together. The three-line proc rule requires consecutive parsed log entries and the same normalized target.

### Explicit incoming damage

```text
You have taken N damage from <Spell Name> by <Attacker>.
```

This becomes incoming damage against the active character. There is no equivalent current pattern for spell damage against another player or pet.

## Landing recognition

Each combat profile's wiki `Cast on Other` is converted to a regex:

1. Escape the whole message.
2. Replace literal `Someone` case-insensitively with a non-greedy target capture.
3. Anchor the expression to the whole log body and match case-insensitively.

```text
Wiki: Someone staggers as the light of dawn washes over it.
Log:  Hexbone skeleton staggers as the light of dawn washes over it.
```

The target becomes `Hexbone skeleton`. Profiles reload at most once every 30 seconds per tracker instance.

When several spells share the same landing message, selection is deterministic:

1. An explicit matching `You begin casting` spell wins.
2. A matching active-character item clue wins when the item-to-proc catalog identifies one candidate.
3. For a local proc with immediately preceding non-melee damage, the newest primary/secondary weapon snapshot may select one candidate when exactly one equipped weapon maps to it.
4. A single remaining profile is accepted normally.
5. A same-target collision involving known proc spells is retained as `Unidentified direct proc` rather than discarded. Logged non-melee damage is preserved, but no catalog damage or item ownership is invented.

**Audit flags:** messages lacking literal `Someone` are excluded; punctuation or wording differences fail; a last-known weapon snapshot can be stale and is therefore only used when it uniquely disambiguates candidates.

## Caster/source priority

For one recognized landing, priority is:

1. Recent item glow.
2. Matching local direct cast.
3. Kept memory-only while eligible for one-line proc confirmation, with immediately preceding generic non-melee evidence retained when present.

### Item click

A glow qualifies zero through three whole seconds after it. When multiple profiles match, the canonical item-to-proc relationship must select the spell; a unique landing continues to work for click items not present in the weapon-proc catalog.

- caster: active character only
- attribution: `item_glow`
- activity source: `item_click`
- source name: glowing item
- proc count: no

**Audit flag:** any combat landing inside the glow window can be assigned to that item, even if item and spell are unrelated.

### Direct cast

Spell names must match ignoring case. With catalog cast time:

```text
expected landing = cast timestamp + casting time
```

The landing must be within plus or minus two seconds. Without catalog cast time, it may land zero through 15 whole seconds after cast start.

- caster: active character
- attribution: `direct_cast`
- activity source: `direct`
- proc count: no

### Unattributed observed item clicks

Some item effects do not emit another player's `begins to glow` line. The spell catalog therefore has a small, extensible `spell_source_rules` table for effects known to be item-click-only. A matching landing is written immediately to `combat_spell_activity` with:

- caster: `Unattributed`
- activity source: `unknown`
- source name: the known item when available
- proc count: no
- inferred damage: no

This keeps the landing visible in the live fight without assigning it to the next attacker or corrupting damage totals. The catalog maps `Curse of the Spirits` (`Someone is consumed by the raging spirits of the land.`) to the `Spear of Fate` Shaman epic item click. When another player's click message is not visible, the source is known but the caster remains `Unattributed`.

### Proc

Without a qualifying active-character glow, direct cast, or cataloged item-click-only rule, the landing remains memory-only for exactly the next parsed line. The tracker also remembers whether the immediately preceding line was generic non-melee damage against the same target. The next line confirms a proc when it is a same-target:

- failed same-target combat attempt ending in `miss!`, `misses!`, `ripostes!`, `blocks!`, `parries!`, or `dodges!`;
- local melee damage event; or
- observed melee damage event.

Caster selection is deliberately conservative. An immediately following same-target combat action explicitly belonging to the active character attributes the proc to that character. Matching generic non-melee damage immediately before the landing additionally supplies its logged direct proc damage. A following same-target combat action from any other player can confirm that the landing was a proc, but it does not prove that player caused it; the occurrence and any inferred proc damage are assigned to `Unidentified player`. The last-known weapon loadout is checked against the bundled weapon-proc relationships only as possible source evidence. DoT attribution and activity become `proc`, and a `proc_occurrences` row is created. Ambiguous known-proc landings are counted under `Unidentified direct proc`; they only add direct damage when the log supplied an actual non-melee amount. Any intervening line, different target/source, or unsupported event consumes and discards the candidate.

**Audit flags:** inserted log chatter can make valid procs fail; only one pending proc exists across simultaneous fights; the next attacker confirms adjacency but is never treated as proof of caster identity; an item-click-only spell needs a catalog source rule to avoid being misclassified as a proc; no rule proves which equipped weapon caused the proc.
Observed-player example:

```text
[Mon Sep 07 08:55:39 2026] Lizzyflop says 'Ahhh, I feel much better now\...'
[Mon Sep 07 08:55:39 2026] Grink staggers.
[Mon Sep 07 08:55:39 2026] Lizzyflop crushes Grink for 30 points of damage.
```

For Essence Tap, the immediately preceding named `says 'Ahhh, I feel much better now...'` message is a spell-specific caster candidate. The landing must then be immediately followed by that same named player attacking or attempting to attack the same target. Only the full three-line sequence attributes the proc to that player. The melee hit remains separate, while Essence Tap contributes its catalog direct-damage estimate to the player's proc total. Without matching combat confirmation, a standalone `Grink staggers.` landing remains `Unidentified player`.

For a spell explicitly classified as `proc_only` in the spell catalog, an immediately following same-target melee hit or attempted hit identifies the proccer. This includes blocked, dodged, parried, riposted, and missed attempts. A matching non-melee damage line immediately before the landing is a stronger local signature: when its amount equals the proc spell's catalog damage, the active character owns the proc even if another player's melee line follows. Without that prefix, the next same-target attacker owns the proc and receives the spell catalog's damage estimate. This covers One Hundred Blows (`120 non-melee -> spinning`) and Divine Might Effect (`65 non-melee -> struck by a surge`) using one rule. Item-click-only spells are excluded from this inference and remain unattributed unless the active character's cast or glow evidence identifies them.

Root landings ending in `'s feet become entwined.` are ambiguous because several direct-cast root spells and weapon procs share that message. A matching active-character `You begin casting ... Roots.` clue wins and is recorded as a direct spell, never a proc. Without a direct-cast or item-click clue, the immediately following same-target attack or combat attempt identifies the likely proccer. When the exact root spell cannot be disambiguated, it is retained as `Unidentified root proc` with no guessed spell damage.
Observed-player DoT-proc example:

```text
[Sun Sep 06 20:46:11 2026] Drusella Sathir staggers as the light of dawn washes over it.
[Sun Sep 06 20:46:11 2026] Balbazak pierces Drusella Sathir for 271 points of damage.
```

The landing uniquely matches Dawncall. Balbazak's immediately following same-target pierce confirms proc adjacency but not caster identity. The 271-point pierce remains Balbazak's melee damage. Dawncall is stored as a proc-attributed DoT application under `Unidentified player`, and its calculated ticks accrue to that unidentified bucket for the catalog duration unless refreshed, explicitly superseded, or ended with the encounter.

## Direct damage accounting

Explicit damage enters `damage_events` and updates encounter totals, spell damage, hit count, max hit, participant summaries, and DPS.

An attack-confirmed proc creates one synthetic event when it has either an immediately preceding logged non-melee amount or catalog `direct_damage > 0`:

```text
source_file = proc://<occurrence id>
source_offset = 1
attack_kind = <Spell Name>
damage_type = spell
attacker = confirmed caster
```

The confirming melee hit remains separate. Pure direct casts and item clicks do not infer catalog direct damage; current logic expects their explicit damage line.

**Audit flags:** duplicate suppression recognizes an explicit attack name equal to a known spell; hybrid direct damage is inferred only for confirmed procs, not direct casts or item clicks. For the three-line active proc sequence, the logged non-melee amount overrides the catalog fallback.

## DoT accounting

A matched profile with damage per tick and tick count creates an application after attribution by active-character direct cast, active-character item click, or an attack-confirmed proc. The next-combat rule can confirm an observed proc but cannot identify an observed player as its caster; those applications use `Unidentified player`. Unconfirmed landings create no application or inferred ticks.

Before insertion, every active same-encounter, same-target, same-spell application is marked `refreshed`, regardless of caster. The replacement resets duration:

```text
expires_at = landed_at + tick interval * total ticks
```

Tick behavior:

- Tick 1 occurs one interval after landing.
- Due count is floor of elapsed time divided by interval, clamped to total ticks.
- Each due tick becomes `dot://<application id>` with tick number as source offset.
- Ticks update ordinary encounter and participant totals.
- Final tick changes status to `expired`.
- Live mode uses local current time; historical scans advance with later log timestamps.

When explicit outgoing damage names a known spell, the newest active same-source/target/spell application has future inference disabled.

**Audit flags:** already-inferred ticks are not removed when explicit damage later appears; generic non-melee cannot disable a named DoT; fixed catalog values ignore scaling/modifiers/resists; duplicate reprocessing can mark the existing application refreshed before its replacement insert is ignored.

## Encounter association

A confirmed landing reuses an `active` encounter matching source file, log character, and target. Otherwise it creates one. Unconfirmed landings do not create encounters. Applications finish through expiration, matching mob death, active-character death, or refresh.

**Audit flags:** landing lookup does not apply the normal 120-second encounter-gap rule; one tracker holds only one cast/glow/proc clue across all fights; rescans create a tracker per file so clues cannot cross a rotated file boundary; same-name simultaneous mobs cannot be distinguished.

## Persistence

| Table | Purpose |
| --- | --- |
| `damage_events` | Canonical explicit and inferred outgoing damage. |
| `damage_encounters` | Fight totals and lifecycle. |
| `dot_applications` | Landing attribution, tick schedule, refresh, and inference state. |
| `proc_occurrences` | Confirmed proc and inferred direct-damage identity. |
| `damage_spell_summaries` | Incremental per-encounter/caster/spell metrics. |
| `combat_spell_activity` | Recognized landing feed with source classification. |
| `item_proc_spells` | Bundled item-to-proc relationships with nullable canonical `item_id`, captured item name, spell, proc level, and source provenance. |

Idempotency uses original source/offset for landings and procs, `proc://<id>` for direct proc damage, and `dot://<application id>` plus tick number for DoT ticks.

## Displayed metrics

The app exposes proc count, inferred direct proc damage, inferred DoT damage/tick count, proc-attributed DoT damage, and total proc damage. Activity views show at most the newest 24 landings per encounter. Live, docked/detached, saved detail, and training replay share fighter bars.

A last-known weapon snapshot is only a stale-capable candidate; it is not proof of proc source.

## Current decision flow

```text
Complete timestamped line
  +-- ordinary parser -> persist explicit outgoing/incoming damage
  +-- spell tracker
       +-- consume prior one-line proc candidate
       |    +-- same-target melee/riposte -> confirm
       |    +-- otherwise -> discard
       +-- named explicit spell damage -> disable future DoT inference
       +-- flush inferred ticks due at this timestamp
       +-- update glow/cast/interruption/resist state
       +-- exactly one Cast-on-Other profile matches?
            +-- glow <= 3s -> item click
            +-- else matching cast time -> direct
            +-- else -> keep memory-only and wait one line for proc evidence
            +-- preceding generic non-melee + confirmation -> active-character proc, logged damage
            +-- otherwise confirmation -> confirming attacker proc, catalog damage
            +-- confirmed attribution -> persist activity/application/encounter
```

## Correction worksheet

For each wrong result, provide the smallest contiguous excerpt containing the cast/glow, landing or resist, explicit damage, immediately following attack/riposte, and encounter ending when relevant.

| Field | Expected answer |
| --- | --- |
| Spell | Which catalog spell landed? |
| Target | Which mob received it? |
| Caster | Who gets credit? |
| Source | Direct cast, item click, active proc, observed proc, or discard? |
| Direct damage | Logged, inferred, or zero? |
| DoT damage | Amount per tick and expected ticks? |
| Proc count | Should it increment? |
| Refresh | Stack, reset, ignore, resist, or interrupt? |
| Encounter | Which fight owns it? |

Each correction should become a focused production-parser/state-machine test before behavior changes.
