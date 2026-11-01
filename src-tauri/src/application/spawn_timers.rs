use std::{collections::HashMap, sync::OnceLock};

#[cfg(not(test))]
use std::{
    sync::Mutex,
    time::{Duration as StdDuration, Instant},
};

#[cfg(test)]
use chrono::Timelike;
use chrono::{DateTime, Duration, Local, NaiveDateTime, TimeZone};
#[cfg(not(test))]
use reqwest::blocking::Client;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::eq_time::{self, EqTimeSync};
use crate::infrastructure::database::Database;

const CATALOG_TSV: &str = include_str!("../../assets/eqtool-spawn-timers.tsv");
const UNKNOWN_ZONE_SECONDS: i64 = 6 * 60 + 40;
const DEFAULT_LINGER_SECONDS: i64 = 4 * 60;
const MAX_LINGER_SECONDS: i64 = 7 * 24 * 60 * 60;
const CASTER_ACTIVE_ENCOUNTER_GAP_SECONDS: i64 = 10 * 60;
const FRESH_CATALOG_ENCOUNTER_SECONDS: i64 = 30;
const REAL_SECONDS_PER_EQ_MINUTE: i64 = 3;
const EQ_DAY_MINUTES: i64 = 24 * 60;
const GEB_SPAWN_MINUTES: i64 = 7 * 60;
#[cfg(not(test))]
const GLOBAL_TIMER_ENDPOINT: &str = "https://pigparse.azurewebsites.net/api/rolltimer/timers";
#[cfg(not(test))]
const GLOBAL_TIMER_CACHE_SECONDS: u64 = 60;

#[derive(Debug, Clone)]
struct MobRule {
    pattern: String,
    normalized_pattern: String,
    seconds: i64,
}

#[derive(Debug, Default)]
struct ZoneRules {
    default_seconds: i64,
    exact: Vec<MobRule>,
    contains: Vec<MobRule>,
}

#[derive(Debug)]
struct SpawnCatalog {
    zones: HashMap<String, ZoneRules>,
    aliases: HashMap<String, String>,
    revision: String,
    override_count: usize,
}

#[derive(Debug)]
struct SpawnMatch {
    seconds: i64,
    window_seconds: i64,
    window_after: bool,
    max_skips: u32,
    match_kind: &'static str,
    matched_rule: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowRules {
    #[serde(default)]
    max_skips: u32,
    #[serde(default)]
    shape: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnCatalogSummary {
    zone_count: usize,
    override_count: usize,
    alias_count: usize,
    upstream_revision: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnTimer {
    id: i64,
    experience_event_id: Option<i64>,
    encounter_id: Option<i64>,
    character: String,
    mob_name: String,
    killer_name: Option<String>,
    zone_name: Option<String>,
    killed_at: String,
    origin_kind: String,
    respawn_at: String,
    respawn_seconds: i64,
    window_start_at: Option<String>,
    window_end_at: Option<String>,
    window_kind: String,
    window_number: u32,
    total_windows: u32,
    match_kind: String,
    matched_rule: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnTimerSnapshot {
    generated_at: String,
    linger_seconds: i64,
    timers: Vec<SpawnTimer>,
    global_timers: Vec<GlobalSpawnTimer>,
    catalog: SpawnCatalogSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnTimerLookupResult {
    zone_name: String,
    mob_name: String,
    respawn_seconds: i64,
    window_seconds: i64,
    window_kind: String,
    total_windows: u32,
    match_kind: String,
    matched_rule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnTimerOverride {
    nickname: String,
    mob_name: String,
    #[serde(default)]
    short_name: Option<String>,
    #[serde(default)]
    zone_name: Option<String>,
    #[serde(default)]
    continent: Option<String>,
    respawn_minutes: i64,
    #[serde(default)]
    window_minutes: i64,
    #[serde(default)]
    windows_json: Option<String>,
    #[serde(default)]
    last_known_tod: Option<String>,
    #[serde(default)]
    last_known_spawn_at: Option<String>,
    #[serde(default)]
    source: String,
    #[serde(default)]
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnTimerCatalogImportResult {
    imported: usize,
    total: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnKillScanMob {
    nickname: String,
    mob_name: String,
    zone_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnKillScanOptions {
    active_character: Option<String>,
    characters: Vec<String>,
    mobs: Vec<SpawnKillScanMob>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnKillScanResult {
    nickname: String,
    mob_name: String,
    character: String,
    killed_at: String,
    zone_name: Option<String>,
    source_kind: String,
    encounter_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSpawnTimer {
    id: String,
    name: String,
    server: String,
    due_at: Option<String>,
    duration_seconds: i64,
    uncertain: bool,
    origin: String,
    origin_at: String,
    description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EqToolRollTimer {
    roll_timer_type: i64,
    guess: bool,
    name: String,
    date_time: String,
}

#[cfg(not(test))]
#[derive(Debug, Default)]
struct GlobalTimerCache {
    server: String,
    fetched_at: Option<Instant>,
    timers: Vec<GlobalSpawnTimer>,
}

fn normalize(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn catalog() -> &'static SpawnCatalog {
    static CATALOG: OnceLock<SpawnCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut zones = HashMap::<String, ZoneRules>::new();
        let mut aliases = HashMap::new();
        let mut revision = String::new();
        let mut override_count = 0;
        for line in CATALOG_TSV.lines() {
            if let Some(value) = line.strip_prefix("# upstream_revision\t") {
                revision = value.trim().to_owned();
                continue;
            }
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let columns = line.split('\t').collect::<Vec<_>>();
            if columns.len() < 3 {
                continue;
            }
            let record_type = columns[0];
            let zone_key = normalize(columns[1]);
            let name = columns[2].trim();
            match record_type {
                "default" => {
                    zones.entry(zone_key).or_default().default_seconds = columns
                        .get(3)
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(UNKNOWN_ZONE_SECONDS);
                }
                "alias" => {
                    aliases.insert(normalize(name), zone_key);
                }
                "exact" | "contains" => {
                    let rule = MobRule {
                        pattern: name.to_owned(),
                        normalized_pattern: normalize(name),
                        seconds: columns
                            .get(3)
                            .and_then(|value| value.parse().ok())
                            .unwrap_or(UNKNOWN_ZONE_SECONDS),
                    };
                    let zone = zones.entry(zone_key).or_default();
                    if record_type == "exact" {
                        zone.exact.push(rule);
                    } else {
                        zone.contains.push(rule);
                    }
                    override_count += 1;
                }
                _ => {}
            }
        }
        SpawnCatalog {
            zones,
            aliases,
            revision,
            override_count,
        }
    })
}

fn spawn_match(zone_name: Option<&str>, mob_name: &str) -> SpawnMatch {
    let catalog = catalog();
    let Some(zone_key) = zone_name
        .map(normalize)
        .and_then(|zone| catalog.aliases.get(&zone).cloned().or(Some(zone)))
        .filter(|zone| catalog.zones.contains_key(zone))
    else {
        return SpawnMatch {
            seconds: UNKNOWN_ZONE_SECONDS,
            window_seconds: 0,
            window_after: false,
            max_skips: 0,
            match_kind: "unknownDefault",
            matched_rule: None,
        };
    };
    let zone = &catalog.zones[&zone_key];
    let mob = normalize(mob_name);
    if let Some(rule) = zone
        .exact
        .iter()
        .find(|rule| rule.normalized_pattern == mob)
    {
        return SpawnMatch {
            seconds: rule.seconds,
            window_seconds: 0,
            window_after: false,
            max_skips: 0,
            match_kind: "exact",
            matched_rule: Some(rule.pattern.clone()),
        };
    }
    if let Some(rule) = zone
        .contains
        .iter()
        .find(|rule| mob.contains(&rule.normalized_pattern))
    {
        return SpawnMatch {
            seconds: rule.seconds,
            window_seconds: 0,
            window_after: false,
            max_skips: 0,
            match_kind: "contains",
            matched_rule: Some(rule.pattern.clone()),
        };
    }
    SpawnMatch {
        seconds: zone.default_seconds,
        window_seconds: 0,
        window_after: false,
        max_skips: 0,
        match_kind: "zoneDefault",
        matched_rule: None,
    }
}

fn experience_fallback_match(zone_name: Option<&str>) -> SpawnMatch {
    let catalog = catalog();
    let seconds = zone_name
        .map(normalize)
        .and_then(|zone| catalog.aliases.get(&zone).cloned().or(Some(zone)))
        .and_then(|zone| catalog.zones.get(&zone).map(|rules| rules.default_seconds))
        .unwrap_or(UNKNOWN_ZONE_SECONDS);
    SpawnMatch {
        seconds,
        window_seconds: 0,
        window_after: false,
        max_skips: 0,
        match_kind: "experienceFallback",
        matched_rule: None,
    }
}

fn load_overrides(connection: &rusqlite::Connection) -> Result<Vec<SpawnTimerOverride>, String> {
    let mut statement = connection
        .prepare(
            "SELECT nickname,mob_name,short_name,zone_name,continent,respawn_minutes,
                    window_minutes,windows_json,last_known_tod,last_known_spawn_at,source,updated_at
             FROM spawn_timer_overrides ORDER BY mob_name COLLATE NOCASE,nickname COLLATE NOCASE",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(SpawnTimerOverride {
                nickname: row.get(0)?,
                mob_name: row.get(1)?,
                short_name: row.get(2)?,
                zone_name: row.get(3)?,
                continent: row.get(4)?,
                respawn_minutes: row.get(5)?,
                window_minutes: row.get(6)?,
                windows_json: row.get(7)?,
                last_known_tod: row.get(8)?,
                last_known_spawn_at: row.get(9)?,
                source: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(rows)
}

fn matching_override<'a>(
    overrides: &'a [SpawnTimerOverride],
    zone_name: Option<&str>,
    mob_name: &str,
) -> Option<&'a SpawnTimerOverride> {
    let mob = normalize(mob_name);
    let zone = zone_name.map(normalize);
    overrides.iter().find(|entry| {
        let name_matches = normalize(&entry.mob_name) == mob
            || normalize(&entry.nickname) == mob
            || entry
                .short_name
                .as_deref()
                .is_some_and(|name| normalize(name) == mob);
        let zone_matches = entry.zone_name.as_deref().is_none_or(|required| {
            zone.as_deref()
                .is_some_and(|actual| normalize(required) == actual)
        });
        name_matches && zone_matches
    })
}

fn spawn_match_with_overrides(
    overrides: &[SpawnTimerOverride],
    zone_name: Option<&str>,
    mob_name: &str,
) -> SpawnMatch {
    if let Some(entry) = matching_override(overrides, zone_name, mob_name) {
        let rules = entry
            .windows_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<WindowRules>(value).ok())
            .unwrap_or_default();
        return SpawnMatch {
            seconds: entry.respawn_minutes * 60,
            window_seconds: entry.window_minutes * 60,
            window_after: rules.shape.eq_ignore_ascii_case("after"),
            max_skips: rules.max_skips,
            match_kind: "catalogOverride",
            matched_rule: Some(entry.mob_name.clone()),
        };
    }
    spawn_match(zone_name, mob_name)
}

#[derive(Debug)]
struct ActiveEncounterCandidate {
    id: i64,
    mob_name: String,
    started_at: DateTime<Local>,
    last_damage_at: DateTime<Local>,
}

fn unique_catalog_encounter(
    overrides: &[SpawnTimerOverride],
    candidates: Option<&Vec<ActiveEncounterCandidate>>,
    zone_name: Option<&str>,
    happened_at: DateTime<Local>,
) -> Option<(i64, String)> {
    let mut matched = HashMap::<String, (i64, String, DateTime<Local>)>::new();
    for candidate in candidates.into_iter().flatten() {
        if candidate.started_at > happened_at + Duration::seconds(5)
            || candidate.last_damage_at > happened_at + Duration::seconds(5)
            || candidate.last_damage_at
                < happened_at - Duration::seconds(CASTER_ACTIVE_ENCOUNTER_GAP_SECONDS)
        {
            continue;
        }
        let Some(entry) = matching_override(overrides, zone_name, &candidate.mob_name) else {
            continue;
        };
        let key = normalize(&entry.nickname);
        let replace = matched
            .get(&key)
            .is_none_or(|existing| candidate.last_damage_at > existing.2);
        if replace {
            matched.insert(
                key,
                (
                    candidate.id,
                    entry.mob_name.clone(),
                    candidate.last_damage_at,
                ),
            );
        }
    }
    let fresh = matched
        .values()
        .filter(|candidate| {
            candidate.2 >= happened_at - Duration::seconds(FRESH_CATALOG_ENCOUNTER_SECONDS)
        })
        .collect::<Vec<_>>();
    if fresh.len() == 1 {
        return fresh
            .first()
            .map(|candidate| (candidate.0, candidate.1.clone()));
    }
    if fresh.len() > 1 {
        return None;
    }
    if matched.len() != 1 {
        return None;
    }
    matched
        .into_values()
        .next()
        .map(|(id, mob_name, _)| (id, mob_name))
}

pub fn lookup(
    database: &Database,
    mob_name: &str,
    zone_name: Option<&str>,
) -> Result<Vec<SpawnTimerLookupResult>, String> {
    let mob_name = mob_name.trim();
    if mob_name.is_empty() {
        return Ok(Vec::new());
    }
    let connection = database.connect().map_err(|error| error.to_string())?;
    let overrides = load_overrides(&connection)?;
    if let Some(zone_name) = zone_name.map(str::trim).filter(|value| !value.is_empty()) {
        let matched = spawn_match_with_overrides(&overrides, Some(zone_name), mob_name);
        return Ok(vec![SpawnTimerLookupResult {
            zone_name: zone_name.to_owned(),
            mob_name: mob_name.to_owned(),
            respawn_seconds: matched.seconds,
            window_seconds: matched.window_seconds,
            window_kind: if matched.window_seconds == 0 {
                "fixed"
            } else if matched.window_after {
                "after"
            } else {
                "symmetric"
            }
            .to_owned(),
            total_windows: matched.max_skips + 1,
            match_kind: matched.match_kind.to_owned(),
            matched_rule: matched.matched_rule,
        }]);
    }
    let query = normalize(mob_name);
    let catalog = catalog();
    let mut results = overrides
        .iter()
        .filter(|entry| {
            normalize(&entry.mob_name).contains(&query)
                || normalize(&entry.nickname).contains(&query)
                || entry
                    .short_name
                    .as_deref()
                    .is_some_and(|name| normalize(name).contains(&query))
        })
        .map(|entry| SpawnTimerLookupResult {
            zone_name: entry.zone_name.clone().unwrap_or_else(|| "Any zone".into()),
            mob_name: entry.mob_name.clone(),
            respawn_seconds: entry.respawn_minutes * 60,
            window_seconds: entry.window_minutes * 60,
            window_kind: entry
                .windows_json
                .as_deref()
                .and_then(|value| serde_json::from_str::<WindowRules>(value).ok())
                .filter(|rules| rules.shape.eq_ignore_ascii_case("after"))
                .map_or("symmetric", |_| "after")
                .to_owned(),
            total_windows: entry
                .windows_json
                .as_deref()
                .and_then(|value| serde_json::from_str::<WindowRules>(value).ok())
                .map_or(1, |rules| rules.max_skips + 1),
            match_kind: "catalogOverride".into(),
            matched_rule: Some(entry.mob_name.clone()),
        })
        .chain(catalog.zones.iter().flat_map(|(zone_name, zone)| {
            zone.exact
                .iter()
                .map(|rule| ("exact", rule))
                .chain(zone.contains.iter().map(|rule| ("contains", rule)))
                .filter(|(_, rule)| {
                    rule.normalized_pattern.contains(&query)
                        || query.contains(&rule.normalized_pattern)
                })
                .map(|(match_kind, rule)| SpawnTimerLookupResult {
                    zone_name: zone_name.clone(),
                    mob_name: mob_name.to_owned(),
                    respawn_seconds: rule.seconds,
                    window_seconds: 0,
                    window_kind: "fixed".to_owned(),
                    total_windows: 1,
                    match_kind: match_kind.to_owned(),
                    matched_rule: Some(rule.pattern.clone()),
                })
                .collect::<Vec<_>>()
        }))
        .collect::<Vec<_>>();
    results.sort_by(|left, right| {
        left.zone_name
            .cmp(&right.zone_name)
            .then_with(|| left.matched_rule.cmp(&right.matched_rule))
    });
    results.truncate(100);
    Ok(results)
}

pub fn catalog_snapshot(database: &Database) -> Result<Vec<SpawnTimerOverride>, String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    load_overrides(&connection)
}

pub fn kill_scan_options(database: &Database) -> Result<SpawnKillScanOptions, String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    let active_character = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='active_character'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .filter(|value| !value.trim().is_empty());
    let mut statement = connection
        .prepare(
            "SELECT character_name FROM (
                SELECT character_name FROM activity_mob_history
                UNION SELECT character_name FROM spawn_timer_trigger_history
                UNION SELECT character_name FROM damage_encounters
                UNION SELECT value AS character_name FROM app_settings WHERE key='active_character'
             ) WHERE TRIM(character_name)<>'' ORDER BY character_name COLLATE NOCASE",
        )
        .map_err(|error| error.to_string())?;
    let characters = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let mobs = load_overrides(&connection)?
        .into_iter()
        .map(|row| SpawnKillScanMob {
            nickname: row.nickname,
            mob_name: row.mob_name,
            zone_name: row.zone_name,
        })
        .collect();
    Ok(SpawnKillScanOptions {
        active_character,
        characters,
        mobs,
    })
}

pub fn anchor_from_last_kill(
    database: &Database,
    nickname: &str,
    character: &str,
) -> Result<SpawnKillScanResult, String> {
    let nickname = nickname.trim();
    let character = character.trim();
    if nickname.is_empty() || character.is_empty() {
        return Err("Choose both a mob and a character before scanning.".into());
    }
    let _guard = database.writer_guard();
    let connection = database.connect().map_err(|error| error.to_string())?;
    let row = load_overrides(&connection)?
        .into_iter()
        .find(|row| row.nickname.eq_ignore_ascii_case(nickname))
        .ok_or_else(|| "The selected mob is no longer in the spawn timer catalog.".to_string())?;
    let kill = connection
        .query_row(
            "SELECT happened_at,source_kind,encounter_id FROM (
                SELECT happened_at,'timerTrigger' AS source_kind,NULL AS encounter_id,0 AS priority
                FROM spawn_timer_trigger_history
                WHERE character_name=?1 COLLATE NOCASE AND mob_name=?2 COLLATE NOCASE
                UNION ALL
                SELECT happened_at,'confirmedKill' AS source_kind,NULL AS encounter_id,1 AS priority
                FROM activity_mob_history
                WHERE character_name=?1 COLLATE NOCASE AND mob_name=?2 COLLATE NOCASE
                UNION ALL
                SELECT COALESCE(ended_at,last_damage_at),'combatRecord',id,2
                FROM damage_encounters
                WHERE character_name=?1 COLLATE NOCASE AND mob_name=?2 COLLATE NOCASE
                  AND outcome='slain'
             ) ORDER BY happened_at DESC,priority ASC LIMIT 1",
            params![character, row.mob_name],
            |result| {
                Ok((
                    result.get::<_, String>(0)?,
                    result.get::<_, String>(1)?,
                    result.get::<_, Option<i64>>(2)?,
                ))
            },
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => format!(
                "No confirmed kill of {} was found for {}.",
                row.mob_name, character
            ),
            other => other.to_string(),
        })?;
    let killed_at = local_timestamp(&kill.0)
        .ok_or_else(|| format!("The stored kill time could not be read: {}", kill.0))?;
    let zone_name = connection
        .query_row(
            "SELECT zone_name FROM character_zone_history
             WHERE character_name=? COLLATE NOCASE AND happened_at<=?
             ORDER BY happened_at DESC,id DESC LIMIT 1",
            params![character, kill.0],
            |result| result.get::<_, String>(0),
        )
        .ok()
        .or_else(|| row.zone_name.clone());
    connection
        .execute(
            "UPDATE spawn_timer_overrides
             SET last_known_spawn_at=?,updated_at=CURRENT_TIMESTAMP
             WHERE nickname=? COLLATE NOCASE",
            params![killed_at.to_rfc3339(), nickname],
        )
        .map_err(|error| error.to_string())?;
    Ok(SpawnKillScanResult {
        nickname: row.nickname,
        mob_name: row.mob_name,
        character: character.to_owned(),
        killed_at: killed_at.to_rfc3339(),
        zone_name,
        source_kind: kill.1,
        encounter_id: kill.2,
    })
}

fn save_override(
    connection: &rusqlite::Connection,
    row: &SpawnTimerOverride,
    source: &str,
) -> Result<(), String> {
    if row.nickname.trim().is_empty()
        || row.mob_name.trim().is_empty()
        || row.respawn_minutes < 0
        || row.window_minutes < 0
    {
        return Err(
            "Nickname, mob name, and non-negative respawn/window minutes are required".into(),
        );
    }
    connection.execute(
        "INSERT INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,
            respawn_minutes,window_minutes,windows_json,last_known_tod,last_known_spawn_at,source,updated_at)
         VALUES(?,?,?,?,?,?,?,?,?,?,?,CURRENT_TIMESTAMP)
         ON CONFLICT(nickname) DO UPDATE SET mob_name=excluded.mob_name,short_name=excluded.short_name,
            zone_name=excluded.zone_name,continent=excluded.continent,respawn_minutes=excluded.respawn_minutes,
            window_minutes=excluded.window_minutes,windows_json=excluded.windows_json,
            last_known_tod=excluded.last_known_tod,last_known_spawn_at=excluded.last_known_spawn_at,
            source=excluded.source,updated_at=CURRENT_TIMESTAMP",
        params![row.nickname.trim(),row.mob_name.trim(),row.short_name.as_deref(),row.zone_name.as_deref(),row.continent.as_deref(),row.respawn_minutes,row.window_minutes,row.windows_json.as_deref(),row.last_known_tod.as_deref(),row.last_known_spawn_at.as_deref(),source],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

pub fn catalog_save(database: &Database, row: SpawnTimerOverride) -> Result<(), String> {
    let _guard = database.writer_guard();
    let connection = database.connect().map_err(|error| error.to_string())?;
    save_override(&connection, &row, "manual")
}

pub fn catalog_delete(database: &Database, nickname: &str) -> Result<(), String> {
    let _guard = database.writer_guard();
    let connection = database.connect().map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM spawn_timer_overrides WHERE nickname=? COLLATE NOCASE",
            [nickname],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn value_string(value: &serde_json::Value, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| value.get(*name))
        .and_then(|value| value.as_str())
        .map(str::to_owned)
}

fn value_i64(value: &serde_json::Value, names: &[&str]) -> Option<i64> {
    names
        .iter()
        .find_map(|name| value.get(*name))
        .and_then(|value| value.as_i64())
}

fn parse_import(json: &str) -> Result<Vec<SpawnTimerOverride>, String> {
    let root: serde_json::Value =
        serde_json::from_str(json).map_err(|error| format!("Invalid spawn timer JSON: {error}"))?;
    let object = root
        .as_object()
        .ok_or("Spawn timer JSON must be an object keyed by mob nickname")?;
    object
        .iter()
        .map(|(nickname, value)| {
            let mob_name = value_string(value, &["Name", "name", "mobName"])
                .ok_or_else(|| format!("{nickname} is missing Name"))?;
            let respawn_minutes = value_i64(value, &["Respawn", "respawnMinutes"])
                .ok_or_else(|| format!("{nickname} is missing Respawn"))?;
            let windows = ["Windows", "windows", "windowsJson"]
                .iter()
                .find_map(|name| value.get(*name))
                .filter(|value| !value.is_null())
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string())
                });
            Ok(SpawnTimerOverride {
                nickname: nickname.clone(),
                mob_name,
                short_name: value_string(value, &["ShortName", "shortName"]),
                zone_name: value_string(value, &["Zone", "zone", "zoneName"]),
                continent: value_string(value, &["Continent", "continent"]),
                respawn_minutes,
                window_minutes: value_i64(value, &["WindowLength", "windowMinutes"]).unwrap_or(0),
                windows_json: windows,
                last_known_tod: value_string(value, &["LastKnownTOD", "lastKnownTod"]),
                last_known_spawn_at: value_string(value, &["LastKnownSpawnAt", "lastKnownSpawnAt"]),
                source: "json-import".into(),
                updated_at: String::new(),
            })
        })
        .collect()
}

pub fn catalog_import(
    database: &Database,
    json: &str,
    replace: bool,
) -> Result<SpawnTimerCatalogImportResult, String> {
    let rows = parse_import(json)?;
    let _guard = database.writer_guard();
    let mut connection = database.connect().map_err(|error| error.to_string())?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    if replace {
        transaction
            .execute("DELETE FROM spawn_timer_overrides", [])
            .map_err(|error| error.to_string())?;
    }
    for row in &rows {
        save_override(&transaction, row, "json-import")?;
    }
    let total = transaction
        .query_row("SELECT COUNT(*) FROM spawn_timer_overrides", [], |row| {
            row.get::<_, usize>(0)
        })
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(SpawnTimerCatalogImportResult {
        imported: rows.len(),
        total,
    })
}

fn local_timestamp(value: &str) -> Option<DateTime<Local>> {
    if let Ok(timestamp) = DateTime::parse_from_rfc3339(value) {
        return Some(timestamp.with_timezone(&Local));
    }
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
        .ok()
        .and_then(|value| Local.from_local_datetime(&value).earliest())
}

fn active_server(connection: &rusqlite::Connection) -> String {
    let path = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='active_log_path'",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default()
        .to_lowercase();
    if path.contains("p1999pvp") {
        "Red".to_owned()
    } else if path.contains("project1999") {
        "Blue".to_owned()
    } else {
        "Green".to_owned()
    }
}

fn derive_global_timers(
    server: &str,
    source: &[EqToolRollTimer],
    now: DateTime<Local>,
) -> Vec<GlobalSpawnTimer> {
    let mut result = Vec::new();
    if let Some(quake) = source
        .iter()
        .filter(|timer| timer.roll_timer_type == 2)
        .filter_map(|timer| DateTime::parse_from_rfc3339(&timer.date_time).ok())
        .max()
    {
        let quake = quake.with_timezone(&Local);
        let mut roll = quake - Duration::minutes(30);
        while roll <= now {
            roll += Duration::hours(24);
        }
        result.push(GlobalSpawnTimer {
            id: format!("{server}-ring-8-roll"),
            name: "Ring 8 roll".to_owned(),
            server: server.to_owned(),
            due_at: Some(roll.to_rfc3339()),
            duration_seconds: 24 * 60 * 60,
            uncertain: false,
            origin: "Server quake cycle".to_owned(),
            origin_at: quake.to_rfc3339(),
            description: "24-hour quake cycle, 30 minutes before the boundary.".to_owned(),
        });
    }
    let scout_source = source
        .iter()
        .filter(|timer| timer.roll_timer_type == 1)
        .filter(|timer| !timer.guess)
        .max_by_key(|timer| &timer.date_time)
        .or_else(|| {
            source
                .iter()
                .filter(|timer| timer.roll_timer_type == 1)
                .max_by_key(|timer| &timer.date_time)
        });
    if let Some(timer) = scout_source {
        if let Ok(due) = DateTime::parse_from_rfc3339(&timer.date_time) {
            let due = due.with_timezone(&Local);
            result.push(GlobalSpawnTimer {
                id: format!("{server}-scout-roll"),
                name: "Scout roll".to_owned(),
                server: server.to_owned(),
                due_at: (!timer.guess).then(|| due.to_rfc3339()),
                duration_seconds: 10 * 60 * 60,
                uncertain: timer.guess,
                origin: timer.name.clone(),
                origin_at: (due - Duration::hours(10)).to_rfc3339(),
                description:
                    "10 hours after the latest Scout Charisa or Kromzek Captain observation."
                        .to_owned(),
            });
        }
    }
    result
}

fn geb_global_timer(
    server: &str,
    sync: Option<&EqTimeSync>,
    now: DateTime<Local>,
) -> GlobalSpawnTimer {
    let schedule = sync.and_then(|sync| {
        let synchronized_at = local_timestamp(&sync.synchronized_at)?;
        let elapsed_seconds = (now - synchronized_at).num_seconds().max(0);
        let elapsed_eq_minutes = elapsed_seconds / REAL_SECONDS_PER_EQ_MINUTE;
        let current_eq_minutes =
            (i64::from(sync.game_minutes) + elapsed_eq_minutes).rem_euclid(EQ_DAY_MINUTES);
        let until_spawn = (GEB_SPAWN_MINUTES - current_eq_minutes).rem_euclid(EQ_DAY_MINUTES);
        let due = synchronized_at
            + Duration::seconds((elapsed_eq_minutes + until_spawn) * REAL_SECONDS_PER_EQ_MINUTE);
        Some((synchronized_at, due))
    });
    GlobalSpawnTimer {
        id: format!("{server}-geb-daily"),
        name: "Geb spawn".to_owned(),
        server: server.to_owned(),
        due_at: schedule.as_ref().map(|(_, due)| due.to_rfc3339()),
        duration_seconds: EQ_DAY_MINUTES * REAL_SECONDS_PER_EQ_MINUTE,
        uncertain: schedule.is_none(),
        origin: if sync.is_some() {
            "Synchronized EQ clock".to_owned()
        } else {
            "Use /time to synchronize".to_owned()
        },
        origin_at: schedule
            .as_ref()
            .map(|(origin, _)| origin.to_rfc3339())
            .unwrap_or_else(|| now.to_rfc3339()),
        description: "Spawns every EverQuest day at 7:00 AM game time.".to_owned(),
    }
}

#[cfg(not(test))]
fn global_timers(server: &str, now: DateTime<Local>) -> Vec<GlobalSpawnTimer> {
    static CACHE: OnceLock<Mutex<GlobalTimerCache>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(GlobalTimerCache::default()));
    if let Ok(cached) = cache.lock() {
        if cached.server == server
            && cached
                .fetched_at
                .is_some_and(|time| time.elapsed().as_secs() < GLOBAL_TIMER_CACHE_SECONDS)
        {
            return cached.timers.clone();
        }
    }
    let fetched = Client::builder()
        .connect_timeout(StdDuration::from_secs(2))
        .timeout(StdDuration::from_secs(5))
        .user_agent("EverQuestLootTracker/3 spawn-timers")
        .build()
        .and_then(|client| {
            client
                .get(format!("{GLOBAL_TIMER_ENDPOINT}/{server}"))
                .send()?
                .error_for_status()?
                .json::<Vec<EqToolRollTimer>>()
        })
        .map(|source| derive_global_timers(server, &source, now));
    match fetched {
        Ok(timers) => {
            if let Ok(mut cached) = cache.lock() {
                cached.server = server.to_owned();
                cached.fetched_at = Some(Instant::now());
                cached.timers = timers.clone();
            }
            timers
        }
        Err(_) => cache
            .lock()
            .ok()
            .filter(|cached| cached.server == server)
            .map(|cached| cached.timers.clone())
            .unwrap_or_default(),
    }
}

#[cfg(test)]
fn global_timers(_server: &str, _now: DateTime<Local>) -> Vec<GlobalSpawnTimer> {
    Vec::new()
}

#[derive(Debug)]
struct TimerSchedule {
    center: DateTime<Local>,
    start: DateTime<Local>,
    end: DateTime<Local>,
    number: u32,
    total: u32,
}

fn timer_schedule(
    killed_at: DateTime<Local>,
    matched: &SpawnMatch,
    now: DateTime<Local>,
    linger_seconds: i64,
) -> Option<TimerSchedule> {
    let total = matched.max_skips + 1;
    (1..=total).find_map(|number| {
        let multiplier = i64::from(number);
        let center = killed_at + Duration::seconds(matched.seconds * multiplier);
        let variance = Duration::seconds(matched.window_seconds * multiplier);
        let start = if matched.window_after {
            center
        } else {
            center - variance
        };
        let end = center + variance;
        (end + Duration::seconds(linger_seconds) >= now).then_some(TimerSchedule {
            center,
            start,
            end,
            number,
            total,
        })
    })
}

fn is_explicit_local_kill(raw_line: &str) -> bool {
    let body = raw_line
        .split_once("] ")
        .map_or(raw_line, |(_, body)| body)
        .trim();
    body.starts_with("You have slain ")
}

pub fn snapshot(database: &Database) -> Result<SpawnTimerSnapshot, String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    let linger_seconds = configured_linger_seconds(&connection);
    let mut statement = connection
        .prepare(
            "SELECT m.id,m.happened_at,m.character_name,m.mob_name,m.killer_name,
                    (SELECT z.zone_name FROM character_zone_history z
                     WHERE z.character_name=m.character_name COLLATE NOCASE
                       AND z.happened_at<=m.happened_at
                       AND z.zone_name<>'EverQuest' COLLATE NOCASE
                     ORDER BY z.happened_at DESC,z.id DESC LIMIT 1),
                    'confirmedKill' AS origin_kind,m.raw_line,
                    EXISTS(
                        SELECT 1 FROM damage_encounters d
                        WHERE d.character_name=m.character_name COLLATE NOCASE
                          AND d.mob_name=m.mob_name COLLATE NOCASE
                          AND d.outcome='slain'
                          AND datetime(d.started_at)<=datetime(m.happened_at,'+5 seconds')
                          AND datetime(COALESCE(d.ended_at,d.last_damage_at))
                              BETWEEN datetime(m.happened_at,'-20 seconds')
                                  AND datetime(m.happened_at,'+5 seconds')
                    ) AS has_encounter_evidence
             FROM activity_mob_history m
             WHERE m.happened_at>=datetime('now','localtime','-14 days')
               AND m.mob_name NOT LIKE 'Eye of %' COLLATE NOCASE
               AND NOT EXISTS(SELECT 1 FROM known_members k WHERE k.name=m.mob_name COLLATE NOCASE)
             UNION ALL
             SELECT -t.id,t.happened_at,t.character_name,t.mob_name,NULL,
                    (SELECT z.zone_name FROM character_zone_history z
                     WHERE z.character_name=t.character_name COLLATE NOCASE
                       AND z.happened_at<=t.happened_at
                       AND z.zone_name<>'EverQuest' COLLATE NOCASE
                     ORDER BY z.happened_at DESC,z.id DESC LIMIT 1),
                    'timerTrigger' AS origin_kind,t.raw_line,1
             FROM spawn_timer_trigger_history t
             WHERE t.happened_at>=datetime('now','localtime','-14 days')
             ORDER BY happened_at DESC,id DESC LIMIT 10000",
        )
        .map_err(|error| error.to_string())?;
    let now = Local::now();
    let server = active_server(&connection);
    let overrides = load_overrides(&connection)?;
    let mut timers = statement
        .query_map(params![], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, bool>(8)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter_map(
            |(
                id,
                happened_at,
                character,
                mob_name,
                killer_name,
                zone_name,
                origin_kind,
                raw_line,
                has_encounter_evidence,
            )| {
                let killed_at = local_timestamp(&happened_at)?;
                let matched =
                    spawn_match_with_overrides(&overrides, zone_name.as_deref(), &mob_name);
                if matched.seconds <= 0 {
                    return None;
                }
                // A remote "X has been slain by Y" line is syntactically identical for NPCs
                // and players. EqTool resolves that ambiguity against its NPC catalogue before
                // starting a timer. Keep local kills and dedicated triggers authoritative, but
                // require either catalogue or encounter evidence for a remote slain message so
                // an unknown raid player's death cannot become a zone-default spawn timer.
                if origin_kind == "confirmedKill"
                    && !is_explicit_local_kill(&raw_line)
                    && matched.matched_rule.is_none()
                    && !has_encounter_evidence
                {
                    return None;
                }
                let schedule = timer_schedule(killed_at, &matched, now, linger_seconds)?;
                Some(SpawnTimer {
                    id,
                    experience_event_id: None,
                    encounter_id: None,
                    character,
                    mob_name,
                    killer_name,
                    zone_name,
                    killed_at: killed_at.to_rfc3339(),
                    origin_kind,
                    respawn_at: schedule.center.to_rfc3339(),
                    respawn_seconds: matched.seconds,
                    window_start_at: (matched.window_seconds > 0)
                        .then(|| schedule.start.to_rfc3339()),
                    window_end_at: (matched.window_seconds > 0).then(|| schedule.end.to_rfc3339()),
                    window_kind: if matched.window_seconds == 0 {
                        "fixed"
                    } else if matched.window_after {
                        "after"
                    } else {
                        "symmetric"
                    }
                    .to_owned(),
                    window_number: schedule.number,
                    total_windows: schedule.total,
                    match_kind: matched.match_kind.to_owned(),
                    matched_rule: matched.matched_rule,
                })
            },
        )
        .collect::<Vec<_>>();
    for (index, entry) in overrides.iter().enumerate() {
        let Some(anchor_at) = entry
            .last_known_spawn_at
            .as_deref()
            .and_then(local_timestamp)
        else {
            continue;
        };
        let same_mob = |timer: &SpawnTimer| {
            normalize(&timer.mob_name) == normalize(&entry.mob_name)
                && entry.zone_name.as_deref().is_none_or(|zone| {
                    timer
                        .zone_name
                        .as_deref()
                        .is_some_and(|actual| normalize(actual) == normalize(zone))
                })
        };
        let newer_confirmed_exists = timers.iter().any(|timer| {
            matches!(timer.origin_kind.as_str(), "confirmedKill" | "timerTrigger")
                && same_mob(timer)
                && local_timestamp(&timer.killed_at).is_some_and(|value| value >= anchor_at)
        });
        if newer_confirmed_exists {
            continue;
        }
        let matched =
            spawn_match_with_overrides(&overrides, entry.zone_name.as_deref(), &entry.mob_name);
        if matched.seconds <= 0 {
            continue;
        }
        let Some(schedule) = timer_schedule(anchor_at, &matched, now, linger_seconds) else {
            continue;
        };
        timers.retain(|timer| {
            !(matches!(timer.origin_kind.as_str(), "confirmedKill" | "timerTrigger")
                && same_mob(timer)
                && local_timestamp(&timer.killed_at).is_some_and(|value| value < anchor_at))
        });
        timers.push(SpawnTimer {
            id: -1_000_000_000 - index as i64,
            experience_event_id: None,
            encounter_id: None,
            character: "Manual catalog entry".to_owned(),
            mob_name: entry.mob_name.clone(),
            killer_name: None,
            zone_name: entry.zone_name.clone(),
            killed_at: anchor_at.to_rfc3339(),
            origin_kind: "manualSpawn".to_owned(),
            respawn_at: schedule.center.to_rfc3339(),
            respawn_seconds: matched.seconds,
            window_start_at: (matched.window_seconds > 0).then(|| schedule.start.to_rfc3339()),
            window_end_at: (matched.window_seconds > 0).then(|| schedule.end.to_rfc3339()),
            window_kind: if matched.window_seconds == 0 {
                "fixed"
            } else if matched.window_after {
                "after"
            } else {
                "symmetric"
            }
            .to_owned(),
            window_number: schedule.number,
            total_windows: schedule.total,
            match_kind: matched.match_kind.to_owned(),
            matched_rule: matched.matched_rule,
        });
    }
    // Catalog inference is intentionally independent of the encounter's current lifecycle state.
    // A caster's encounter is commonly closed as `disengaged` shortly after its XP line, and
    // limiting this lookup to rows that are still `active` made a valid timer disappear on the
    // next refresh. Player-death encounters remain excluded because they are not kill evidence.
    let mut catalog_encounter_statement = connection
        .prepare(
            "SELECT id,character_name,mob_name,started_at,last_damage_at
             FROM damage_encounters
             WHERE outcome IN ('active','disengaged','slain')
               AND last_damage_at>=datetime('now','localtime','-14 days')
             ORDER BY last_damage_at DESC,id DESC",
        )
        .map_err(|error| error.to_string())?;
    let mut catalog_encounters = HashMap::<String, Vec<ActiveEncounterCandidate>>::new();
    for row in catalog_encounter_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
    {
        let (id, character, mob_name, started_at, last_damage_at) = row;
        let (Some(started_at), Some(last_damage_at)) = (
            local_timestamp(&started_at),
            local_timestamp(&last_damage_at),
        ) else {
            continue;
        };
        catalog_encounters
            .entry(normalize(&character))
            .or_default()
            .push(ActiveEncounterCandidate {
                id,
                mob_name,
                started_at,
                last_damage_at,
            });
    }
    let mut experience_statement = connection
        .prepare(
            "SELECT e.id,e.happened_at,e.character_name,
                    COALESCE(NULLIF(c.zone_name COLLATE NOCASE,'EverQuest'),
                        (SELECT z.zone_name FROM character_zone_history z
                         WHERE z.character_name=e.character_name COLLATE NOCASE
                           AND z.happened_at<=e.happened_at
                           AND z.zone_name<>'EverQuest' COLLATE NOCASE
                         ORDER BY z.happened_at DESC,z.id DESC LIMIT 1)),
                    c.mob_name,d.id,d.mob_name
             FROM experience_gain_events e
             LEFT JOIN experience_gain_contexts c ON c.experience_gain_event_id=e.id
             LEFT JOIN damage_encounters d ON d.id=(
                 SELECT candidate.id FROM damage_encounters candidate
                 WHERE candidate.character_name=e.character_name COLLATE NOCASE
                   AND candidate.outcome='slain'
                   AND datetime(candidate.started_at)<=datetime(e.happened_at,'+5 seconds')
                   AND datetime(COALESCE(candidate.ended_at,candidate.last_damage_at))>=datetime(e.happened_at,'-20 seconds')
                 ORDER BY candidate.last_damage_at DESC,candidate.id DESC
                 LIMIT 1
             )
             WHERE e.happened_at>=datetime('now','localtime','-14 days')
               AND NOT EXISTS(
                   SELECT 1 FROM activity_mob_history m
                   WHERE m.character_name=e.character_name COLLATE NOCASE
                     AND m.happened_at BETWEEN datetime(e.happened_at,'-15 seconds')
                                           AND datetime(e.happened_at,'+15 seconds')
               )
             ORDER BY e.happened_at DESC,e.id DESC LIMIT 10000",
        )
        .map_err(|error| error.to_string())?;
    let inferred = experience_statement
        .query_map(params![], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter_map(
            |(id, happened_at, character, zone_name, context_mob, encounter_id, encounter_mob)| {
                let killed_at = local_timestamp(&happened_at)?;
                let active_candidate = (context_mob.is_none() && encounter_mob.is_none())
                    .then(|| {
                        unique_catalog_encounter(
                            &overrides,
                            catalog_encounters.get(&normalize(&character)),
                            zone_name.as_deref(),
                            killed_at,
                        )
                    })
                    .flatten();
                let resolved_encounter_id =
                    encounter_id.or(active_candidate.as_ref().map(|candidate| candidate.0));
                let inferred_mob = context_mob
                    .clone()
                    .or(encounter_mob.clone())
                    .or_else(|| active_candidate.map(|candidate| candidate.1));
                let matched = inferred_mob
                    .as_deref()
                    .map(|mob| spawn_match_with_overrides(&overrides, zone_name.as_deref(), mob))
                    .unwrap_or_else(|| experience_fallback_match(zone_name.as_deref()));
                if matched.seconds <= 0 {
                    return None;
                }
                let schedule = timer_schedule(killed_at, &matched, now, linger_seconds)?;
                let origin_kind = if context_mob.is_some() {
                    "correctedMob"
                } else if resolved_encounter_id.is_some() {
                    "combatRecord"
                } else {
                    "experienceFallback"
                };
                Some(SpawnTimer {
                    id: -id,
                    experience_event_id: Some(id),
                    encounter_id: resolved_encounter_id,
                    character,
                    mob_name: inferred_mob.unwrap_or_else(|| "XP kill fallback".to_owned()),
                    killer_name: None,
                    zone_name,
                    killed_at: killed_at.to_rfc3339(),
                    origin_kind: origin_kind.to_owned(),
                    respawn_at: schedule.center.to_rfc3339(),
                    respawn_seconds: matched.seconds,
                    window_start_at: (matched.window_seconds > 0)
                        .then(|| schedule.start.to_rfc3339()),
                    window_end_at: (matched.window_seconds > 0).then(|| schedule.end.to_rfc3339()),
                    window_kind: if matched.window_seconds == 0 {
                        "fixed"
                    } else if matched.window_after {
                        "after"
                    } else {
                        "symmetric"
                    }
                    .to_owned(),
                    window_number: schedule.number,
                    total_windows: schedule.total,
                    match_kind: matched.match_kind.to_owned(),
                    matched_rule: matched.matched_rule,
                })
            },
        );
    timers.extend(inferred);
    timers.sort_by(|left, right| left.respawn_at.cmp(&right.respawn_at));
    let eq_time = eq_time::snapshot(&connection)?;
    let mut global = global_timers(&server, now);
    global.push(geb_global_timer(&server, eq_time.as_ref(), now));
    global.sort_by(|left, right| left.due_at.cmp(&right.due_at));
    let catalog = catalog();
    Ok(SpawnTimerSnapshot {
        generated_at: now.to_rfc3339(),
        linger_seconds,
        timers,
        global_timers: global,
        catalog: SpawnCatalogSummary {
            zone_count: catalog.zones.len(),
            override_count: catalog.override_count,
            alias_count: catalog.aliases.len(),
            upstream_revision: catalog.revision.clone(),
        },
    })
}

fn configured_linger_seconds(connection: &rusqlite::Connection) -> i64 {
    connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='spawn_timer_linger_seconds'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_LINGER_SECONDS)
        .clamp(0, MAX_LINGER_SECONDS)
}

pub fn set_linger_seconds(database: &Database, linger_seconds: i64) -> Result<(), String> {
    if !(0..=MAX_LINGER_SECONDS).contains(&linger_seconds) {
        return Err("Timer linger must be between 0 minutes and 7 days.".into());
    }
    let _guard = database.writer_guard();
    let connection = database.connect().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO app_settings(key,value) VALUES('spawn_timer_linger_seconds',?)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [linger_seconds.to_string()],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn correct_mob(
    database: &Database,
    experience_event_id: i64,
    encounter_id: Option<i64>,
    mob_name: &str,
) -> Result<(), String> {
    let corrected = mob_name.trim();
    if corrected.is_empty() || corrected.len() > 120 || corrected.chars().any(char::is_control) {
        return Err("Enter a mob name between 1 and 120 characters.".into());
    }
    let mut connection = database.connect().map_err(|error| error.to_string())?;
    let event_exists = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM experience_gain_events WHERE id=?)",
            [experience_event_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| error.to_string())?;
    if !event_exists {
        return Err("The XP event for this timer no longer exists.".into());
    }
    if let Some(id) = encounter_id {
        super::combat_metrics::correct_recorded_encounter_target(&mut connection, id, corrected)?;
    }
    connection
        .execute(
            "INSERT INTO experience_gain_contexts(experience_gain_event_id,mob_name,mob_evidence)
             VALUES(?,?,'manual-spawn-timer')
             ON CONFLICT(experience_gain_event_id) DO UPDATE SET
                mob_name=excluded.mob_name,mob_evidence=excluded.mob_evidence",
            params![experience_event_id, corrected],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn loads_the_pinned_eqtool_catalog() {
        let catalog = catalog();
        assert_eq!(catalog.zones.len(), 121);
        assert_eq!(catalog.override_count, 200);
        assert!(catalog.aliases.len() >= 260);
        assert_eq!(catalog.revision, "200bf339ae438e635ef50b1e071a654f801a44e0");
    }

    #[test]
    fn resolves_zone_defaults_and_npc_overrides() {
        let ordinary = spawn_match(Some("Estate of Unrest"), "a death beetle");
        assert_eq!(ordinary.seconds, 22 * 60);
        assert_eq!(ordinary.match_kind, "zoneDefault");
        let guard = spawn_match(Some("Butcherblock Mountains"), "Guard Armandin");
        assert_eq!(guard.seconds, 24 * 60);
        assert_eq!(guard.match_kind, "contains");
    }

    #[test]
    fn lookup_finds_overrides_and_zone_defaults() {
        let directory = tempdir().unwrap();
        let database = Database::open(directory.path().join("lookup.sqlite3")).unwrap();
        database.migrate().unwrap();
        let overrides = lookup(&database, "Guard Armandin", None).unwrap();
        assert!(overrides.iter().any(|result| {
            result.zone_name == "butcher"
                && result.respawn_seconds == 24 * 60
                && result.match_kind == "contains"
        }));
        let zone_default = lookup(&database, "a death beetle", Some("Estate of Unrest")).unwrap();
        assert_eq!(zone_default.len(), 1);
        assert_eq!(zone_default[0].respawn_seconds, 22 * 60);
        assert_eq!(zone_default[0].match_kind, "zoneDefault");
        let bundled = lookup(&database, "vindi", None).unwrap();
        assert!(bundled
            .iter()
            .any(|result| result.mob_name == "Derakor the Vindicator"
                && result.respawn_seconds == 480 * 60
                && result.window_seconds == 60 * 60));
    }

    #[test]
    fn cumulative_skip_windows_widen_after_each_missed_cycle() {
        let killed_at = local_timestamp("2026-10-20 18:00:00").unwrap();
        let matched = SpawnMatch {
            seconds: 18 * 60 * 60,
            window_seconds: 60 * 60,
            window_after: false,
            max_skips: 2,
            match_kind: "catalogOverride",
            matched_rule: Some("Lodizal (lodi)".into()),
        };
        let during_second = local_timestamp("2026-10-22 05:00:00").unwrap();
        let schedule =
            timer_schedule(killed_at, &matched, during_second, DEFAULT_LINGER_SECONDS).unwrap();
        assert_eq!(schedule.number, 2);
        assert_eq!(schedule.total, 3);
        assert_eq!(schedule.start.hour(), 4);
        assert_eq!(schedule.end.hour(), 8);
    }

    #[test]
    fn after_only_window_never_opens_before_nominal_respawn() {
        let killed_at = local_timestamp("2026-10-20 18:00:00").unwrap();
        let matched = SpawnMatch {
            seconds: 72 * 60 * 60,
            window_seconds: 8 * 60 * 60,
            window_after: true,
            max_skips: 0,
            match_kind: "catalogOverride",
            matched_rule: Some("an angry goblin (angry)".into()),
        };
        let schedule =
            timer_schedule(killed_at, &matched, killed_at, DEFAULT_LINGER_SECONDS).unwrap();
        assert_eq!(schedule.start, schedule.center);
        assert_eq!(schedule.end, schedule.center + Duration::hours(8));
    }

    #[test]
    fn configured_linger_controls_how_long_completed_timers_remain() {
        let directory = tempdir().unwrap();
        let database = Database::open(directory.path().join("timer-linger.sqlite3")).unwrap();
        database.migrate().unwrap();
        set_linger_seconds(&database, 15 * 60).unwrap();
        let connection = database.connect().unwrap();
        assert_eq!(configured_linger_seconds(&connection), 15 * 60);
        drop(connection);

        let killed_at = local_timestamp("2026-10-20 18:00:00").unwrap();
        let matched = SpawnMatch {
            seconds: 10 * 60,
            window_seconds: 0,
            window_after: false,
            max_skips: 0,
            match_kind: "catalogOverride",
            matched_rule: Some("a test mob".into()),
        };
        let after_spawn = killed_at + Duration::minutes(20);
        assert!(timer_schedule(killed_at, &matched, after_spawn, 15 * 60).is_some());
        assert!(timer_schedule(killed_at, &matched, after_spawn, 0).is_none());
        assert!(set_linger_seconds(&database, MAX_LINGER_SECONDS + 1).is_err());
    }

    #[test]
    fn imports_and_updates_nickname_keyed_override_json() {
        let directory = tempdir().unwrap();
        let database = Database::open(directory.path().join("import.sqlite3")).unwrap();
        database.migrate().unwrap();
        let result = catalog_import(
            &database,
            r#"{"test":{"Name":"Test Mob","Respawn":90,"WindowLength":12}}"#,
            false,
        )
        .unwrap();
        assert_eq!(result.imported, 1);
        let rows = catalog_snapshot(&database).unwrap();
        let row = rows.iter().find(|row| row.nickname == "test").unwrap();
        assert_eq!(row.mob_name, "Test Mob");
        assert_eq!(row.respawn_minutes, 90);
        assert_eq!(row.window_minutes, 12);
    }

    #[test]
    fn bundled_override_catalog_has_wiki_backed_zones() {
        let directory = tempdir().unwrap();
        let database = Database::open(directory.path().join("spawn-zones.sqlite3")).unwrap();
        database.migrate().unwrap();
        let rows = catalog_snapshot(&database).unwrap();
        assert_eq!(rows.len(), 52);
        assert!(rows.iter().all(|row| row.zone_name.is_some()));
        assert_eq!(
            rows.iter()
                .find(|row| row.nickname == "angry")
                .and_then(|row| row.zone_name.as_deref()),
            Some("Temple of Droga")
        );
        assert_eq!(
            rows.iter()
                .find(|row| row.nickname == "grink")
                .and_then(|row| row.zone_name.as_deref()),
            Some("Plane of Mischief")
        );
        assert_eq!(
            rows.iter()
                .find(|row| row.nickname == "scout")
                .and_then(|row| row.zone_name.as_deref()),
            Some("Western Wastes")
        );
        let tormax = rows.iter().find(|row| row.nickname == "tormax").unwrap();
        assert_eq!(tormax.mob_name, "King Tormax");
        assert_eq!(tormax.zone_name.as_deref(), Some("Kael Drakkel"));
        assert_eq!(tormax.respawn_minutes, 7 * 24 * 60);
        assert_eq!(tormax.window_minutes, 8 * 60);
    }

    #[test]
    fn unknown_zones_use_eqtools_safe_default() {
        let matched = spawn_match(Some("A Future Zone"), "a test mob");
        assert_eq!(matched.seconds, 400);
        assert_eq!(matched.match_kind, "unknownDefault");
    }

    #[test]
    fn experience_fallback_uses_the_zone_default_without_a_slain_message() {
        let directory = tempdir().unwrap();
        let database = Database::open(directory.path().join("xp-spawn-timers.sqlite3")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        let happened_at = (Local::now() - Duration::seconds(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        connection.execute("INSERT INTO experience_gain_events(happened_at,character_name,raw_line,source_file,source_offset) VALUES(?1,'Tester','You gain experience!!','test.log',1)",[&happened_at]).unwrap();
        let event_id = connection.last_insert_rowid();
        connection.execute("INSERT INTO experience_gain_contexts(experience_gain_event_id,zone_name,zone_evidence) VALUES(?1,'Estate of Unrest','entered')",[event_id]).unwrap();
        drop(connection);

        let result = snapshot(&database).unwrap();
        assert_eq!(result.timers.len(), 1);
        assert_eq!(result.timers[0].mob_name, "XP kill fallback");
        assert_eq!(result.timers[0].respawn_seconds, 22 * 60);
        assert_eq!(result.timers[0].match_kind, "experienceFallback");
    }

    #[test]
    fn experience_timer_prefers_the_matching_combat_record() {
        let directory = tempdir().unwrap();
        let database =
            Database::open(directory.path().join("xp-combat-spawn-timers.sqlite3")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        let happened_at = (Local::now() - Duration::seconds(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let started_at = (Local::now() - Duration::seconds(20))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        connection.execute("INSERT INTO experience_gain_events(happened_at,character_name,raw_line,source_file,source_offset) VALUES(?1,'Tester','You gain experience!!','test.log',20)",[&happened_at]).unwrap();
        let event_id = connection.last_insert_rowid();
        connection.execute("INSERT INTO experience_gain_contexts(experience_gain_event_id,zone_name,zone_evidence) VALUES(?1,'Estate of Unrest','entered')",[event_id]).unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,ended_at,last_damage_at,outcome,source_file,first_source_offset,last_source_offset)
             VALUES('Tester','a death beetle',?1,?2,?2,'slain','test.log',10,19)",
            params![started_at, happened_at],
        ).unwrap();
        let encounter_id = connection.last_insert_rowid();
        drop(connection);

        let result = snapshot(&database).unwrap();
        assert_eq!(result.timers.len(), 1);
        assert_eq!(result.timers[0].mob_name, "a death beetle");
        assert_eq!(result.timers[0].origin_kind, "combatRecord");
        assert_eq!(result.timers[0].experience_event_id, Some(event_id));
        assert_eq!(result.timers[0].encounter_id, Some(encounter_id));
        assert_eq!(result.timers[0].match_kind, "zoneDefault");
    }

    #[test]
    fn experience_timer_does_not_promote_a_disengaged_fight_to_a_kill() {
        let directory = tempdir().unwrap();
        let database =
            Database::open(directory.path().join("xp-disengaged-spawn-timers.sqlite3")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        let happened_at = (Local::now() - Duration::seconds(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let started_at = (Local::now() - Duration::seconds(20))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let ended_at = (Local::now() - Duration::seconds(10))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        connection.execute("INSERT INTO experience_gain_events(happened_at,character_name,raw_line,source_file,source_offset) VALUES(?1,'Tester','You gain experience!!','test.log',20)",[&happened_at]).unwrap();
        connection.execute("INSERT INTO experience_gain_contexts(experience_gain_event_id,zone_name,zone_evidence) VALUES(?1,'Estate of Unrest','entered')",[connection.last_insert_rowid()]).unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,ended_at,last_damage_at,outcome,source_file,first_source_offset,last_source_offset)
             VALUES('Tester','Guard McStinkles',?1,?2,?2,'disengaged','test.log',10,19)",
            params![started_at, ended_at],
        ).unwrap();
        drop(connection);

        let result = snapshot(&database).unwrap();
        assert_eq!(result.timers.len(), 1);
        assert_eq!(result.timers[0].mob_name, "XP kill fallback");
        assert_eq!(result.timers[0].origin_kind, "experienceFallback");
        assert_eq!(result.timers[0].encounter_id, None);
    }

    #[test]
    fn caster_fallback_uses_the_only_active_catalog_mob_despite_a_damage_gap() {
        let directory = tempdir().unwrap();
        let database = Database::open(directory.path().join("xp-caster-fallback.sqlite3")).unwrap();
        database.migrate().unwrap();
        let happened_at = (Local::now() - Duration::seconds(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let started_at = (Local::now() - Duration::minutes(3))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let last_damage_at = (Local::now() - Duration::minutes(2))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let connection = database.connect().unwrap();
        connection.execute("INSERT INTO experience_gain_events(happened_at,character_name,raw_line,source_file,source_offset) VALUES(?1,'Caster','You gain experience!!','test.log',20)",[&happened_at]).unwrap();
        let event_id = connection.last_insert_rowid();
        connection.execute("INSERT INTO experience_gain_contexts(experience_gain_event_id,zone_name,zone_evidence) VALUES(?1,'Plane of Mischief','entered')",[event_id]).unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,outcome,source_file,first_source_offset,last_source_offset)
             VALUES('Caster','TwentyTwo',?1,?2,'active','test.log',10,19)",
            params![started_at, last_damage_at],
        ).unwrap();
        let encounter_id = connection.last_insert_rowid();
        drop(connection);

        let result = snapshot(&database).unwrap();
        assert_eq!(result.timers.len(), 1);
        assert_eq!(result.timers[0].mob_name, "Twenty Two");
        assert_eq!(result.timers[0].origin_kind, "combatRecord");
        assert_eq!(result.timers[0].encounter_id, Some(encounter_id));
        assert_eq!(result.timers[0].respawn_seconds, 70 * 60);
        assert_eq!(result.timers[0].match_kind, "catalogOverride");
    }

    #[test]
    fn caster_fallback_ignores_a_saved_server_wide_who_zone() {
        let directory = tempdir().unwrap();
        let database = Database::open(directory.path().join("xp-false-who-zone.sqlite3")).unwrap();
        database.migrate().unwrap();
        let happened_at = (Local::now() - Duration::seconds(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let real_zone_at = (Local::now() - Duration::minutes(10))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let false_zone_at = (Local::now() - Duration::minutes(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let started_at = (Local::now() - Duration::minutes(3))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let last_damage_at = (Local::now() - Duration::minutes(2))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let connection = database.connect().unwrap();
        connection.execute("INSERT INTO character_zone_history(happened_at,character_name,zone_name,evidence,source_file,source_offset) VALUES(?1,'Caster','Plane of Mischief','entered','test.log',1)",[&real_zone_at]).unwrap();
        connection.execute("INSERT INTO character_zone_history(happened_at,character_name,zone_name,evidence,source_file,source_offset) VALUES(?1,'Caster','EverQuest','who','test.log',2)",[&false_zone_at]).unwrap();
        connection.execute("INSERT INTO experience_gain_events(happened_at,character_name,raw_line,source_file,source_offset) VALUES(?1,'Caster','You gain experience!!','test.log',20)",[&happened_at]).unwrap();
        let event_id = connection.last_insert_rowid();
        connection.execute("INSERT INTO experience_gain_contexts(experience_gain_event_id,zone_name,zone_evidence) VALUES(?1,'EverQuest','who')",[event_id]).unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,outcome,source_file,first_source_offset,last_source_offset)
             VALUES('Caster','Grink',?1,?2,'active','test.log',10,19)",
            params![started_at, last_damage_at],
        ).unwrap();
        let encounter_id = connection.last_insert_rowid();
        drop(connection);

        let result = snapshot(&database).unwrap();
        assert_eq!(result.timers.len(), 1);
        assert_eq!(result.timers[0].mob_name, "Grink");
        assert_eq!(
            result.timers[0].zone_name.as_deref(),
            Some("Plane of Mischief")
        );
        assert_eq!(result.timers[0].origin_kind, "combatRecord");
        assert_eq!(result.timers[0].encounter_id, Some(encounter_id));
        assert_eq!(result.timers[0].match_kind, "catalogOverride");
    }

    #[test]
    fn caster_fallback_survives_normal_encounter_cleanup() {
        let directory = tempdir().unwrap();
        let database = Database::open(directory.path().join("xp-caster-cleanup.sqlite3")).unwrap();
        database.migrate().unwrap();
        let happened_at = (Local::now() - Duration::seconds(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let started_at = (Local::now() - Duration::minutes(3))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let last_damage_at = (Local::now() - Duration::minutes(2))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let connection = database.connect().unwrap();
        connection.execute("INSERT INTO experience_gain_events(happened_at,character_name,raw_line,source_file,source_offset) VALUES(?1,'Caster','You gain experience!!','test.log',20)",[&happened_at]).unwrap();
        let event_id = connection.last_insert_rowid();
        connection.execute("INSERT INTO experience_gain_contexts(experience_gain_event_id,zone_name,zone_evidence) VALUES(?1,'Plane of Mischief','entered')",[event_id]).unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,ended_at,last_damage_at,outcome,source_file,first_source_offset,last_source_offset)
             VALUES('Caster','Glonk',?1,?2,?2,'disengaged','test.log',10,19)",
            params![started_at, last_damage_at],
        ).unwrap();
        let encounter_id = connection.last_insert_rowid();
        drop(connection);

        let result = snapshot(&database).unwrap();
        assert_eq!(result.timers.len(), 1);
        assert_eq!(result.timers[0].mob_name, "Glonk");
        assert_eq!(result.timers[0].origin_kind, "combatRecord");
        assert_eq!(result.timers[0].encounter_id, Some(encounter_id));
        assert_eq!(result.timers[0].match_kind, "catalogOverride");
    }

    #[test]
    fn caster_fallback_refuses_to_guess_between_active_catalog_mobs() {
        let directory = tempdir().unwrap();
        let database =
            Database::open(directory.path().join("xp-caster-ambiguous.sqlite3")).unwrap();
        database.migrate().unwrap();
        let happened_at = (Local::now() - Duration::seconds(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let started_at = (Local::now() - Duration::minutes(3))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let last_damage_at = (Local::now() - Duration::minutes(2))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let connection = database.connect().unwrap();
        connection.execute("INSERT INTO experience_gain_events(happened_at,character_name,raw_line,source_file,source_offset) VALUES(?1,'Caster','You gain experience!!','test.log',20)",[&happened_at]).unwrap();
        let event_id = connection.last_insert_rowid();
        connection.execute("INSERT INTO experience_gain_contexts(experience_gain_event_id,zone_name,zone_evidence) VALUES(?1,'Plane of Mischief','entered')",[event_id]).unwrap();
        for (index, mob) in ["TwentyTwo", "Grink"].iter().enumerate() {
            connection.execute(
                "INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,outcome,source_file,first_source_offset,last_source_offset)
                 VALUES('Caster',?1,?2,?3,'active','test.log',?4,?4)",
                params![mob, started_at, last_damage_at, 10 + index as i64],
            ).unwrap();
        }
        drop(connection);

        let result = snapshot(&database).unwrap();
        assert_eq!(result.timers.len(), 1);
        assert_eq!(result.timers[0].mob_name, "XP kill fallback");
        assert_eq!(result.timers[0].origin_kind, "experienceFallback");
    }

    #[test]
    fn fresh_catalog_combat_wins_over_a_stale_active_catalog_encounter() {
        let directory = tempdir().unwrap();
        let database = Database::open(directory.path().join("xp-fresh-catalog.sqlite3")).unwrap();
        database.migrate().unwrap();
        let happened_at = (Local::now() - Duration::seconds(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let started_at = (Local::now() - Duration::minutes(7))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let stale_damage_at = (Local::now() - Duration::minutes(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let fresh_damage_at = (Local::now() - Duration::seconds(6))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let connection = database.connect().unwrap();
        connection.execute("INSERT INTO experience_gain_events(happened_at,character_name,raw_line,source_file,source_offset) VALUES(?1,'Caster','You gain experience!!','test.log',30)",[&happened_at]).unwrap();
        let event_id = connection.last_insert_rowid();
        connection.execute("INSERT INTO experience_gain_contexts(experience_gain_event_id,zone_name,zone_evidence) VALUES(?1,'Plane of Mischief','entered')",[event_id]).unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,outcome,source_file,first_source_offset,last_source_offset)
             VALUES('Caster','Brenn',?1,?2,'active','test.log',10,19)",
            params![started_at, stale_damage_at],
        ).unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,outcome,source_file,first_source_offset,last_source_offset)
             VALUES('Caster','Grenn',?1,?2,'active','test.log',20,29)",
            params![started_at, fresh_damage_at],
        ).unwrap();
        let grenn_encounter_id = connection.last_insert_rowid();
        drop(connection);

        let result = snapshot(&database).unwrap();
        assert_eq!(result.timers.len(), 1);
        assert_eq!(result.timers[0].mob_name, "Grenn");
        assert_eq!(result.timers[0].origin_kind, "combatRecord");
        assert_eq!(result.timers[0].encounter_id, Some(grenn_encounter_id));
    }

    #[test]
    fn correcting_an_experience_timer_updates_context_and_combat_record() {
        let directory = tempdir().unwrap();
        let database =
            Database::open(directory.path().join("correct-xp-spawn-timer.sqlite3")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        let happened_at = (Local::now() - Duration::seconds(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        connection.execute("INSERT INTO experience_gain_events(happened_at,character_name,raw_line,source_file,source_offset) VALUES(?1,'Tester','You gain experience!!','test.log',20)",[&happened_at]).unwrap();
        let event_id = connection.last_insert_rowid();
        connection.execute("INSERT INTO experience_gain_contexts(experience_gain_event_id,zone_name,zone_evidence) VALUES(?1,'Estate of Unrest','entered')",[event_id]).unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,ended_at,last_damage_at,outcome,source_file,first_source_offset,last_source_offset)
             VALUES('Tester','Treasure Chest',datetime(?1,'-10 seconds'),?1,?1,'slain','test.log',10,19)",
            [&happened_at],
        ).unwrap();
        let encounter_id = connection.last_insert_rowid();
        drop(connection);

        correct_mob(&database, event_id, Some(encounter_id), "Grenn").unwrap();
        let connection = database.connect().unwrap();
        let context_mob: String = connection
            .query_row(
                "SELECT mob_name FROM experience_gain_contexts WHERE experience_gain_event_id=?",
                [event_id],
                |row| row.get(0),
            )
            .unwrap();
        let encounter_mob: String = connection
            .query_row(
                "SELECT mob_name FROM damage_encounters WHERE id=?",
                [encounter_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(context_mob, "Grenn");
        assert_eq!(encounter_mob, "Grenn");
        drop(connection);

        let result = snapshot(&database).unwrap();
        assert_eq!(result.timers[0].mob_name, "Grenn");
        assert_eq!(result.timers[0].origin_kind, "correctedMob");
    }

    #[test]
    fn snapshot_correlates_a_recent_kill_with_the_characters_last_zone() {
        let directory = tempdir().unwrap();
        let database = Database::open(directory.path().join("spawn-timers.sqlite3")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        let zone_at = (Local::now() - Duration::seconds(10))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let killed_at = (Local::now() - Duration::seconds(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        connection.execute("INSERT INTO character_zone_history(happened_at,character_name,zone_name,evidence,source_file,source_offset) VALUES(?1,'Tester','Estate of Unrest','entered','test.log',1)",[&zone_at]).unwrap();
        connection.execute("INSERT INTO activity_mob_history(happened_at,character_name,mob_name,killer_name,raw_line,source_file,source_offset) VALUES(?1,'Tester','a death beetle','Tester','You have slain a death beetle!','test.log',2)",[&killed_at]).unwrap();
        drop(connection);

        let result = snapshot(&database).unwrap();
        assert_eq!(result.timers.len(), 1);
        assert_eq!(
            result.timers[0].zone_name.as_deref(),
            Some("Estate of Unrest")
        );
        assert_eq!(result.timers[0].respawn_seconds, 22 * 60);
        assert_eq!(result.timers[0].match_kind, "zoneDefault");
    }

    #[test]
    fn snapshot_rejects_an_unknown_players_remote_death_as_timer_evidence() {
        let directory = tempdir().unwrap();
        let database = Database::open(directory.path().join("player-death-timer.sqlite3")).unwrap();
        database.migrate().unwrap();
        let zone_at = (Local::now() - Duration::seconds(10))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let happened_at = (Local::now() - Duration::seconds(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let connection = database.connect().unwrap();
        connection.execute("INSERT INTO character_zone_history(happened_at,character_name,zone_name,evidence,source_file,source_offset) VALUES(?1,'Tester','Estate of Unrest','entered','test.log',1)",[&zone_at]).unwrap();
        connection.execute(
            "INSERT INTO activity_mob_history(happened_at,character_name,mob_name,killer_name,raw_line,source_file,source_offset)
             VALUES(?1,'Tester','Unseenraider','a death beetle',?2,'test.log',2)",
            params![
                &happened_at,
                "[Sat Oct 31 08:17:12 2026] Unseenraider has been slain by a death beetle!"
            ],
        ).unwrap();
        drop(connection);

        let result = snapshot(&database).unwrap();
        assert!(result.timers.is_empty());
    }

    #[test]
    fn snapshot_accepts_a_remote_catalogued_npc_kill_without_an_encounter() {
        let directory = tempdir().unwrap();
        let database =
            Database::open(directory.path().join("catalogued-remote-kill.sqlite3")).unwrap();
        database.migrate().unwrap();
        let zone_at = (Local::now() - Duration::seconds(10))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let happened_at = (Local::now() - Duration::seconds(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let connection = database.connect().unwrap();
        connection.execute("INSERT INTO character_zone_history(happened_at,character_name,zone_name,evidence,source_file,source_offset) VALUES(?1,'Tester','Kael Drakkel','entered','test.log',1)",[&zone_at]).unwrap();
        connection.execute(
            "INSERT INTO activity_mob_history(happened_at,character_name,mob_name,killer_name,raw_line,source_file,source_offset)
             VALUES(?1,'Tester','Derakor the Vindicator','Raider',?2,'test.log',2)",
            params![
                &happened_at,
                "[Sat Oct 31 08:17:12 2026] Derakor the Vindicator has been slain by Raider!"
            ],
        ).unwrap();
        drop(connection);

        let result = snapshot(&database).unwrap();
        let timer = result
            .timers
            .iter()
            .find(|timer| timer.mob_name == "Derakor the Vindicator")
            .unwrap();
        assert_eq!(timer.origin_kind, "confirmedKill");
        assert_eq!(timer.match_kind, "catalogOverride");
    }

    #[test]
    fn snapshot_accepts_a_remote_npc_kill_with_matching_encounter_evidence() {
        let directory = tempdir().unwrap();
        let database =
            Database::open(directory.path().join("encounter-remote-kill.sqlite3")).unwrap();
        database.migrate().unwrap();
        let zone_at = (Local::now() - Duration::seconds(20))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let started_at = (Local::now() - Duration::seconds(15))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let happened_at = (Local::now() - Duration::seconds(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let connection = database.connect().unwrap();
        connection.execute("INSERT INTO character_zone_history(happened_at,character_name,zone_name,evidence,source_file,source_offset) VALUES(?1,'Tester','Estate of Unrest','entered','test.log',1)",[&zone_at]).unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,ended_at,last_damage_at,outcome,source_file,first_source_offset,last_source_offset)
             VALUES('Tester','a death beetle',?1,?2,?2,'slain','test.log',2,3)",
            params![&started_at, &happened_at],
        ).unwrap();
        connection.execute(
            "INSERT INTO activity_mob_history(happened_at,character_name,mob_name,killer_name,raw_line,source_file,source_offset)
             VALUES(?1,'Tester','a death beetle','Raider',?2,'test.log',3)",
            params![
                &happened_at,
                "[Sat Oct 31 08:17:12 2026] a death beetle has been slain by Raider!"
            ],
        ).unwrap();
        drop(connection);

        let result = snapshot(&database).unwrap();
        let timer = result
            .timers
            .iter()
            .find(|timer| timer.mob_name == "a death beetle")
            .unwrap();
        assert_eq!(timer.origin_kind, "confirmedKill");
        assert_eq!(timer.match_kind, "zoneDefault");
    }

    #[test]
    fn manual_last_known_spawn_anchors_the_catalog_timer() {
        let directory = tempdir().unwrap();
        let database = Database::open(directory.path().join("manual-spawn.sqlite3")).unwrap();
        database.migrate().unwrap();
        let anchor = (Local::now() - Duration::hours(1)).to_rfc3339();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "UPDATE spawn_timer_overrides SET last_known_spawn_at=?1 WHERE nickname='vindi'",
                [&anchor],
            )
            .unwrap();
        drop(connection);

        let result = snapshot(&database).unwrap();
        let timer = result
            .timers
            .iter()
            .find(|timer| timer.mob_name == "Derakor the Vindicator")
            .unwrap();
        assert_eq!(timer.origin_kind, "manualSpawn");
        assert_eq!(timer.character, "Manual catalog entry");
        assert_eq!(timer.respawn_seconds, 8 * 60 * 60);
        assert_eq!(timer.window_number, 1);
    }

    #[test]
    fn scans_the_selected_characters_latest_kill_and_updates_the_catalog_anchor() {
        let directory = tempdir().unwrap();
        let database = Database::open(directory.path().join("kill-scan.sqlite3")).unwrap();
        database.migrate().unwrap();
        let older = (Local::now() - Duration::hours(2))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let latest = (Local::now() - Duration::hours(1))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let connection = database.connect().unwrap();
        connection.execute("INSERT INTO app_settings(key,value) VALUES('active_character','Tester') ON CONFLICT(key) DO UPDATE SET value=excluded.value",[]).unwrap();
        connection.execute("INSERT INTO activity_mob_history(happened_at,character_name,mob_name,killer_name,raw_line,source_file,source_offset) VALUES(?1,'Tester','Derakor the Vindicator','Tester','older kill','test.log',1)",[&older]).unwrap();
        connection.execute("INSERT INTO activity_mob_history(happened_at,character_name,mob_name,killer_name,raw_line,source_file,source_offset) VALUES(?1,'Tester','Derakor the Vindicator','Tester','latest kill','test.log',2)",[&latest]).unwrap();
        drop(connection);

        let options = kill_scan_options(&database).unwrap();
        assert_eq!(options.active_character.as_deref(), Some("Tester"));
        assert!(options.characters.iter().any(|name| name == "Tester"));
        assert!(options.mobs.iter().any(|mob| mob.nickname == "vindi"));

        let result = anchor_from_last_kill(&database, "vindi", "Tester").unwrap();
        assert_eq!(result.mob_name, "Derakor the Vindicator");
        assert_eq!(result.character, "Tester");
        assert_eq!(result.source_kind, "confirmedKill");
        assert_eq!(
            local_timestamp(&result.killed_at).unwrap(),
            local_timestamp(&latest).unwrap()
        );
        let stored: String = database
            .connect()
            .unwrap()
            .query_row(
                "SELECT last_known_spawn_at FROM spawn_timer_overrides WHERE nickname='vindi'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            local_timestamp(&stored).unwrap(),
            local_timestamp(&latest).unwrap()
        );
    }

    #[test]
    fn angry_goblin_trigger_drives_live_and_historical_timers_without_a_slain_record() {
        let directory = tempdir().unwrap();
        let database = Database::open(directory.path().join("angry-trigger.sqlite3")).unwrap();
        database.migrate().unwrap();
        let happened_at = (Local::now() - Duration::minutes(5))
            .naive_local()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO spawn_timer_trigger_history(
                happened_at,character_name,mob_name,trigger_kind,raw_line,source_file,source_offset
             ) VALUES(?1,'Tester','an angry goblin','triumphEmote',?2,'test.log',1)",
                params![
                    &happened_at,
                    "[Thu Oct 29 14:10:13 2026] an angry goblin howls in triumph!"
                ],
            )
            .unwrap();
        let slain_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM activity_mob_history", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(slain_rows, 0);
        drop(connection);

        let snapshot = snapshot(&database).unwrap();
        let timer = snapshot
            .timers
            .iter()
            .find(|timer| timer.mob_name == "an angry goblin")
            .unwrap();
        assert_eq!(timer.origin_kind, "timerTrigger");

        let result = anchor_from_last_kill(&database, "angry", "Tester").unwrap();
        assert_eq!(result.source_kind, "timerTrigger");
        assert_eq!(
            local_timestamp(&result.killed_at).unwrap(),
            local_timestamp(&happened_at).unwrap()
        );
    }

    #[test]
    fn derives_ring_and_scout_rolls_from_eqtool_source_events() {
        let now = local_timestamp("2026-10-29 08:00:00").unwrap();
        let source = vec![
            EqToolRollTimer {
                roll_timer_type: 2,
                guess: false,
                name: "Quake".into(),
                date_time: (now - Duration::hours(48)).to_rfc3339(),
            },
            EqToolRollTimer {
                roll_timer_type: 1,
                guess: false,
                name: "Scout Charisa".into(),
                date_time: (now + Duration::hours(2)).to_rfc3339(),
            },
        ];
        let timers = derive_global_timers("Green", &source, now);
        assert_eq!(timers.len(), 2);
        assert_eq!(timers[0].name, "Ring 8 roll");
        assert!(timers[0].due_at.is_some());
        assert_eq!(timers[1].origin, "Scout Charisa");
        assert_eq!(timers[1].duration_seconds, 10 * 60 * 60);
    }

    #[test]
    fn geb_timer_targets_seven_am_every_eq_day() {
        let now = local_timestamp("2026-10-29 08:00:00").unwrap();
        let sync = EqTimeSync {
            game_minutes: 6 * 60,
            synchronized_at: "2026-10-29 08:00:00".into(),
            game_date: "Tuesday, September 17, 3253".into(),
            character: "Tester".into(),
        };
        let timer = geb_global_timer("Green", Some(&sync), now);
        assert_eq!(timer.name, "Geb spawn");
        assert_eq!(timer.duration_seconds, 72 * 60);
        assert_eq!(
            timer.due_at.as_deref().and_then(local_timestamp),
            Some(now + Duration::minutes(3))
        );
        assert!(!timer.uncertain);
    }

    #[test]
    fn geb_timer_remains_visible_before_eq_time_is_synchronized() {
        let now = local_timestamp("2026-10-29 08:00:00").unwrap();
        let timer = geb_global_timer("Green", None, now);
        assert_eq!(timer.name, "Geb spawn");
        assert!(timer.due_at.is_none());
        assert!(timer.uncertain);
        assert_eq!(timer.origin, "Use /time to synchronize");
    }
}
