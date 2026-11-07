# Mob spawn timers

The **Combat → Spawn Timers** workspace estimates when a slain NPC will respawn.

## Runtime flow

1. The existing shared log parser recognizes `You have slain …!` and `… has been slain by …!` messages.
2. The ingestion coordinator stores the immutable kill in `activity_mob_history`.
3. The timer query correlates that kill with the same character's most recent `character_zone_history` record at or before the kill.
4. The bundled catalog resolves timing in this order:
   - exact NPC override;
   - case-insensitive NPC-name containment override;
   - zone default;
   - unknown-zone fallback of 6 minutes 40 seconds.
5. The React page counts down locally once per second. It refreshes from the focused backend command after watcher changes and every 30 seconds. Recently due timers remain visible for four minutes.

An experience message is retained for experience tracking but cannot create a spawn timer by itself. Quest turn-ins and other rewards use the same `You gain experience` messages as kills, so timer inference requires independent mob evidence: a manually corrected XP context, a matching slain combat record, or one unambiguous catalog mob from recent combat. Bare and ambiguous XP events produce no timer. Confirmed kills continue to suppress XP inference to prevent duplicates.

Remote slain messages accept multi-word killers so pet finishing blows remain confirmed kills. Server-wide Friends and `/who all` summaries such as `There are 2 players in EverQuest.` are not zone evidence. Timer reads also ignore previously stored `EverQuest` summary rows and fall back to the most recent real zone, allowing caster XP events to retain named catalog correlation without rewriting history.

For caster XP inference, a single catalog mob with combat in the preceding 30 seconds takes priority over stale active catalog encounters. If multiple catalog mobs are fresh, no timer is created. When none are fresh, the existing 10-minute window is used only when it contains one unambiguous catalog mob, preserving long-range caster and pet kills without treating evidence-free XP as a kill.

Pet attack reports provide an additional named-mob path for caster kills that produce no visible damage or slain line. On XP, the parser retains the most recent non-self `Pet tells you, 'Attacking <target> Master.'` target from the preceding five minutes; repeated reports where the pet names itself are ignored. This evidence creates a timer only when the target resolves to a named catalog override in the active zone, so an arbitrary pet target cannot turn quest XP into a zone-default timer. A short read-time compatibility lookup lets newly captured evidence repair recent XP events from an older build, while future events persist the evidence directly in `experience_gain_contexts`.

The timer query is bounded to fourteen days and 10,000 kill rows so multi-day raid respawns remain active without creating an unbounded history scan. It does not reparse logs, mutate kill history, or add work to the watcher hot path. Same-named mobs remain separate timers because each confirmed kill retains its own history ID.

## Server-wide timers

The snapshot also imports EqTool's public PigParse roll-timer feed for the server inferred from the active log filename (`Green`, `Blue`, or `Red`). Responses are cached for 60 seconds and network failures fall back to the most recent in-process result, so local kill timers continue to work offline.

- **Scout roll** is sourced from the latest definitive Scout Charisa or `a Kromzek Captain` observation and is due 10 hours after that observation. If EqTool marks the observation as a guess, the UI reports the timing as unknown rather than presenting false precision.
- **Ring 8 roll** starts from the reported server quake timestamp and advances in 24-hour cycles. The displayed roll is 30 minutes before the next cycle boundary, matching EqTool's client logic.

Global timer cards display their source and observation time so the calculation remains auditable.

## Surfaces

- The dedicated page shows complete local timer details and a separate server-wide section.
- The workspace strip uses fixed-width compact cards so close controls never drift across the page. Clearing a strip card is session-only.
- The resizable, always-on-top compact window is opened with **Undock** and follows both local and server-wide timers independently of the main page.
- The catalog lookup accepts a mob name and optional zone. Without a zone it searches named exact/pattern overrides; with a zone it resolves the same exact → pattern → zone-default → unknown fallback chain used by live timers.
- The override manager stores nickname, exact mob name, optional zone, respawn minutes, window minutes, future window JSON, an optional last-known-spawn anchor, and provenance in normalized SQLite rows. JSON imports can merge or deliberately replace the managed catalog; rows can also be added, edited, and deleted individually.
- Entering a last known spawn time immediately creates the same timer schedule used by logged kills, including variance and cumulative skipped windows. A newer confirmed kill supersedes the manual anchor. The source timestamp remains catalog metadata and is not inserted into kill history.

## Managed override catalog

The initial managed catalog is generated from `spawntimers.json` by `tools/import_spawn_timer_overrides.py`. Its 50 nickname-keyed entries are normalized into migration 43 and preserved in `src-tauri/assets/spawn-timer-overrides.json` for review. `Respawn` values are interpreted as minutes. Managed overrides take precedence over EqTool NPC and zone rules when the logged mob name matches the actual name, nickname, or short name.

Schema 44 enriches the imported rows with explicit Project 1999 wiki evidence. A catalog respawn is now the nominal time and `window_minutes` is the variance on either side. The first supported set includes the common 7-day ±8-hour and 3-day ±8-hour raid windows, Statue (5 days ±8 hours), Vindicator (8 hours ±1 hour), Magi P`Tasa (3 days ±12 hours), Zlandicar (4.5 days ±8 hours), Lodizal, and Stormfeather. Manual rows are never overwritten by this enrichment.

Schema 45 adds `last_known_spawn_at` as a distinct manual timer anchor. It deliberately does not reinterpret imported `last_known_tod` values, whose provenance and meaning are not reliable enough to create timers automatically.

Schema 46 fills the zone for every bundled override using the NPC pages and consolidated encounter listings on the Project 1999 wiki. The enrichment is provenance-limited: it only fills blank zones on bundled or previously wiki-enriched rows, so manually saved and JSON-imported zone choices are not overwritten. The less obvious mappings include the Alice-area card mobs in Plane of Mischief, `an angry goblin` in Temple of Droga, Sentry Badain in Great Divide, and the Scout timer (Scout Charisa) in Western Wastes.

## EverQuest-time global timers

Geb is always present in the global timer collection. The timer projects the next 7:00 AM from the latest parsed `/time` synchronization using the established conversion of one EQ minute to three real seconds, so one EQ day is 72 real minutes. At exactly 7:00 AM it briefly reads due, then rolls to the next EQ day. Before the first time synchronization it remains visible as `TIMING UNKNOWN` with a prompt to use `/time`; later time messages automatically correct the schedule through the shared EQ clock.

Lodizal and Stormfeather use `windows_json.kind = cumulativeSkip`. After a skipped cycle both the center and uncertainty multiply from the original death time: 18h ±1h, 36h ±2h, then 54h ±3h. Lodizal permits two skips (three displayed windows). The Stormfeather wiki says it may skip up to three times, so its catalog permits a fourth 72h ±4h window. The snapshot automatically advances to the first window that has not closed, and all three UI surfaces distinguish **opens in**, **in window / closes in**, and **window passed** states.

The wiki research helper at `tools/research_spawn_timer_wiki.py` is intentionally report-only: external wiki prose must be reviewed before it changes user timers.

## Catalog provenance

`src-tauri/assets/eqtool-spawn-timers.tsv` is mechanically generated from `EQToolShared/Zones.cs` in [EqTool](https://github.com/smasherprog/EqTool), pinned to revision `200bf339ae438e635ef50b1e071a654f801a44e0`.

The imported snapshot contains:

- 121 zone defaults;
- 200 exact or contains-style NPC overrides;
- 269 zone aliases.

EqTool is MIT licensed. The required license notice is retained in `THIRD_PARTY_NOTICES.md`. To deliberately refresh the snapshot, review the upstream diff and run:

```powershell
python tools/import_eqtool_spawn_catalog.py <path-to-EqTool>/EQToolShared/Zones.cs src-tauri/assets/eqtool-spawn-timers.tsv --revision <commit>
```

Always review changed timings and run the catalog and full application tests before shipping.
