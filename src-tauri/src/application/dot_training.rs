use super::{dot_tracking::DotTracker, runtime::scan_damage_file_with_dots};
use crate::{
    domain::log_events::{parse_envelope, parse_log_event, LogEvent},
    infrastructure::{database::Database, spell_catalog::SpellCatalog},
};
use chrono::NaiveDateTime;
use serde::Serialize;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_WORKSPACE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DotTrainingReport {
    active_character: String,
    line_count: usize,
    recognized_count: usize,
    ignored_count: usize,
    dot_profile_count: usize,
    projected_all_ticks: bool,
    projected_through: Option<String>,
    lines: Vec<DotTrainingLine>,
    encounters: Vec<DotTrainingEncounter>,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DotTrainingLine {
    line_number: usize,
    source_offset: i64,
    happened_at: Option<String>,
    raw_line: String,
    status: String,
    parser_event: String,
    summary: String,
    decisions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DotTrainingApplication {
    id: i64,
    encounter_id: i64,
    spell_name: String,
    target_name: String,
    caster_name: String,
    attribution_method: String,
    landed_at: String,
    expires_at: String,
    damage_per_tick: u64,
    tick_interval_seconds: u32,
    total_ticks: u32,
    ticks_applied: u32,
    inference_enabled: bool,
    status: String,
    source_offset: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DotTrainingEvent {
    id: i64,
    encounter_id: i64,
    happened_at: String,
    attacker: String,
    damage_type: String,
    attack: String,
    damage: u64,
    inferred: bool,
    tick_index: Option<u32>,
    source_offset: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DotTrainingParticipant {
    name: String,
    total_damage: u64,
    hit_count: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DotTrainingEncounter {
    id: i64,
    mob_name: String,
    started_at: String,
    ended_at: Option<String>,
    last_damage_at: String,
    total_damage: u64,
    melee_damage: u64,
    spell_damage: u64,
    hit_count: u64,
    outcome: String,
    participants: Vec<DotTrainingParticipant>,
    events: Vec<DotTrainingEvent>,
    dots: Vec<DotTrainingApplication>,
}

struct TemporaryWorkspace(PathBuf);

impl Drop for TemporaryWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn analyze(
    spell_catalog: &SpellCatalog,
    text: &str,
    active_character: &str,
    project_all_ticks: bool,
) -> Result<DotTrainingReport, String> {
    let character = clean_character(active_character);
    let lines = source_lines(text);
    if lines.is_empty() {
        return Err("Paste at least one complete EverQuest log line.".into());
    }
    let workspace = temporary_workspace()?;
    let database =
        Database::open(workspace.0.join("training.db")).map_err(|error| error.to_string())?;
    database.migrate().map_err(|error| error.to_string())?;
    let log_path = workspace
        .0
        .join(format!("eqlog_{character}_P1999Green.txt"));
    let source_text = lines
        .iter()
        .map(|(_, _, line)| line.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(&log_path, source_text.as_bytes()).map_err(|error| error.to_string())?;

    let dot_profile_count = spell_catalog.dot_profiles()?.len();
    let mut tracker = DotTracker::new(spell_catalog.clone());
    scan_damage_file_with_dots(&database, &log_path, &mut tracker)?;

    let mut projected_through = None;
    if project_all_ticks {
        let connection = database.connect().map_err(|error| error.to_string())?;
        let expires = connection
            .query_row("SELECT MAX(expires_at) FROM dot_applications", [], |row| {
                row.get::<_, Option<String>>(0)
            })
            .map_err(|error| error.to_string())?;
        if let Some(value) = expires {
            let through = parse_database_time(&value)?;
            tracker.flush_through(&connection, through)?;
            projected_through = Some(value);
        }
    }

    let connection = database.connect().map_err(|error| error.to_string())?;
    let applications = load_applications(&connection)?;
    let events = load_events(&connection)?;
    let encounters = load_encounters(&connection, &events, &applications)?;
    let interpreted = interpret_lines(&lines, &character, &applications, &events);
    let recognized_count = interpreted
        .iter()
        .filter(|line| line.status != "ignored" && line.status != "invalid")
        .count();
    let ignored_count = interpreted.len() - recognized_count;
    let mut warnings = Vec::new();
    if dot_profile_count == 0 {
        warnings.push("The cached spell catalog contains no usable DoT profiles. Reload it on the System page before judging landing-message matches.".into());
    }
    if applications.is_empty() {
        warnings.push("No pasted line matched a cached DoT Cast on Other message.".into());
    }
    let unknown = applications
        .iter()
        .filter(|dot| dot.caster_name == "Unknown")
        .count();
    if unknown > 0 {
        warnings.push(format!(
            "{unknown} DoT application{} could not be attributed to a caster.",
            if unknown == 1 { "" } else { "s" }
        ));
    }
    if project_all_ticks && projected_through.is_some() {
        warnings.push("Projected ticks are simulated through expiration for visualization; they are clearly marked as inferred and are not saved to combat history.".into());
    }
    drop(connection);
    drop(database);

    Ok(DotTrainingReport {
        active_character: character,
        line_count: interpreted.len(),
        recognized_count,
        ignored_count,
        dot_profile_count,
        projected_all_ticks: project_all_ticks,
        projected_through,
        lines: interpreted,
        encounters,
        warnings,
    })
}

fn clean_character(value: &str) -> String {
    let cleaned = value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || "'_-".contains(character) {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if cleaned.is_empty() {
        "Tester".into()
    } else {
        cleaned
    }
}

fn source_lines(text: &str) -> Vec<(usize, i64, String)> {
    let mut offset = 0_i64;
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let row = (index + 1, offset, line.to_owned());
            offset += line.len() as i64 + 1;
            row
        })
        .collect()
}

fn temporary_workspace() -> Result<TemporaryWorkspace, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let sequence = NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "eq-loot-dot-training-{}-{stamp}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    Ok(TemporaryWorkspace(path))
}

fn parse_database_time(value: &str) -> Result<NaiveDateTime, String> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").map_err(|error| error.to_string())
}

fn load_applications(
    connection: &rusqlite::Connection,
) -> Result<Vec<DotTrainingApplication>, String> {
    let mut statement = connection.prepare("SELECT id,encounter_id,spell_name,target_name,caster_name,attribution_method,landed_at,expires_at,damage_per_tick,tick_interval_seconds,total_ticks,ticks_applied,inference_enabled,status,source_offset FROM dot_applications ORDER BY landed_at,id").map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(DotTrainingApplication {
                id: row.get(0)?,
                encounter_id: row.get(1)?,
                spell_name: row.get(2)?,
                target_name: row.get(3)?,
                caster_name: row.get(4)?,
                attribution_method: row.get(5)?,
                landed_at: row.get(6)?,
                expires_at: row.get(7)?,
                damage_per_tick: row.get(8)?,
                tick_interval_seconds: row.get(9)?,
                total_ticks: row.get(10)?,
                ticks_applied: row.get(11)?,
                inference_enabled: row.get::<_, i64>(12)? != 0,
                status: row.get(13)?,
                source_offset: row.get(14)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(rows)
}

fn load_events(connection: &rusqlite::Connection) -> Result<Vec<DotTrainingEvent>, String> {
    let mut statement = connection.prepare("SELECT id,encounter_id,happened_at,COALESCE(attacker_name,'Unknown'),damage_type,attack_kind,damage,source_file,source_offset FROM damage_events ORDER BY happened_at,id").map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            let source: String = row.get(7)?;
            let inferred = source.starts_with("dot://");
            Ok(DotTrainingEvent {
                id: row.get(0)?,
                encounter_id: row.get(1)?,
                happened_at: row.get(2)?,
                attacker: row.get(3)?,
                damage_type: row.get(4)?,
                attack: row.get(5)?,
                damage: row.get(6)?,
                inferred,
                tick_index: inferred.then(|| row.get::<_, u32>(8)).transpose()?,
                source_offset: row.get(8)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(rows)
}

fn load_encounters(
    connection: &rusqlite::Connection,
    events: &[DotTrainingEvent],
    applications: &[DotTrainingApplication],
) -> Result<Vec<DotTrainingEncounter>, String> {
    let mut statement = connection.prepare("SELECT id,mob_name,started_at,ended_at,last_damage_at,total_damage,melee_damage,spell_damage,hit_count,outcome FROM damage_encounters ORDER BY started_at,id").map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, u64>(5)?,
                row.get::<_, u64>(6)?,
                row.get::<_, u64>(7)?,
                row.get::<_, u64>(8)?,
                row.get::<_, String>(9)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    rows.into_iter().map(|(id,mob_name,started_at,ended_at,last_damage_at,total_damage,melee_damage,spell_damage,hit_count,outcome)| {
        let mut participant_statement = connection.prepare("SELECT attacker_name,total_damage,hit_count FROM damage_participant_summaries WHERE encounter_id=? ORDER BY total_damage DESC,attacker_name COLLATE NOCASE").map_err(|error| error.to_string())?;
        let participants = participant_statement.query_map([id], |row| Ok(DotTrainingParticipant { name: row.get(0)?, total_damage: row.get(1)?, hit_count: row.get(2)? })).map_err(|error| error.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;
        Ok(DotTrainingEncounter { id,mob_name,started_at,ended_at,last_damage_at,total_damage,melee_damage,spell_damage,hit_count,outcome,participants,events:events.iter().filter(|event|event.encounter_id==id).cloned().collect(),dots:applications.iter().filter(|dot|dot.encounter_id==id).cloned().collect() })
    }).collect()
}

fn interpret_lines(
    lines: &[(usize, i64, String)],
    character: &str,
    applications: &[DotTrainingApplication],
    events: &[DotTrainingEvent],
) -> Vec<DotTrainingLine> {
    lines.iter().map(|(line_number, offset, raw)| {
        let envelope = parse_envelope(raw);
        let parsed = parse_log_event(raw, character);
        let landed: Vec<_> = applications.iter().filter(|dot|dot.source_offset==*offset).collect();
        let direct: Vec<_> = events.iter().filter(|event|!event.inferred&&event.source_offset==*offset).collect();
        let mut decisions = Vec::new();
        for dot in &landed {
            decisions.push(format!("Matched {} from its cached Cast on Other message; target = {}.", dot.spell_name, dot.target_name));
            decisions.push(format!("Caster = {} via {}. {} damage every {} seconds for {} ticks ({} total if uninterrupted).", dot.caster_name, attribution_label(&dot.attribution_method), dot.damage_per_tick, dot.tick_interval_seconds, dot.total_ticks, dot.damage_per_tick * u64::from(dot.total_ticks)));
            if !dot.inference_enabled { decisions.push("Explicit spell damage was observed, so calculated ticks were disabled to prevent double counting.".into()); }
            if dot.status == "refreshed" { decisions.push("A later landing refreshed this application; remaining ticks from this instance were stopped instead of stacked.".into()); }
        }
        if let Some(LogEvent::SpellCastStarted { spell_name, .. }) = parsed.as_ref() {
            decisions.push(format!("Stored {spell_name} as an exact local-cast clue for a matching DoT landing within the next 15 seconds."));
        }
        if matches!(parsed, Some(LogEvent::ItemGlow { .. })) { decisions.push("Stored as a caster clue for a DoT landing within the next 3 seconds.".into()); }
        let preceding_proc = lines
            .iter()
            .position(|(_, candidate_offset, _)| candidate_offset == offset)
            .and_then(|index| index.checked_sub(1))
            .map(|index| lines[index].1)
            .and_then(|preceding_offset| {
                applications.iter().find(|dot| {
                    dot.source_offset == preceding_offset && dot.attribution_method == "proc"
                })
            });
        if let Some(dot) = preceding_proc {
            decisions.push(format!(
                "Attributed the immediately preceding {} landing on {} to {} as a weapon proc.",
                dot.spell_name, dot.target_name, dot.caster_name
            ));
        }
        for event in direct { decisions.push(format!("Recorded explicit {} damage: {} used {} for {}.", event.damage_type, event.attacker, event.attack, event.damage)); }
        let (parser_event, summary) = parsed.as_ref().map(describe_event).unwrap_or_else(|| ("none".into(), if envelope.is_some() { "No standard combat event recognized.".into() } else { "Invalid or missing EverQuest timestamp envelope.".into() }));
        let status = if !landed.is_empty() { "dot" } else if parsed.is_some() { "recognized" } else if envelope.is_none() { "invalid" } else { "ignored" };
        DotTrainingLine { line_number:*line_number, source_offset:*offset, happened_at:envelope.map(|value|value.0.to_string()), raw_line:raw.clone(), status:status.into(), parser_event, summary, decisions }
    }).collect()
}

fn attribution_label(value: &str) -> &str {
    match value {
        "item_glow" => "item glow",
        "direct_cast" => "direct spell cast",
        "proc" => "weapon proc",
        "next_attack" => "next attack/riposte",
        _ => "no reliable clue",
    }
}

fn describe_event(event: &LogEvent) -> (String, String) {
    match event {
        LogEvent::Damage {
            attacker_name,
            mob_name,
            attack,
            amount,
            damage_type,
            ..
        } => (
            "damage".into(),
            format!(
                "{attacker_name} -> {mob_name}: {} {attack} damage ({amount}).",
                damage_type.as_str()
            ),
        ),
        LogEvent::ObservedMelee {
            subject_name,
            target_name,
            attack,
            amount,
            ..
        } => (
            "observedMelee".into(),
            format!("{subject_name} -> {target_name}: {attack} ({amount})."),
        ),
        LogEvent::IncomingDamage {
            attacker_name,
            target_name,
            attack,
            amount,
            ..
        } => (
            "incomingDamage".into(),
            format!("{attacker_name} -> {target_name}: {attack} ({amount})."),
        ),
        LogEvent::CombatAttempt {
            attacker_name,
            mob_name,
            attack,
            ..
        } => (
            "combatAttempt".into(),
            format!("{attacker_name} attempted {attack} against {mob_name}."),
        ),
        LogEvent::ItemGlow {
            owner_name,
            item_name,
            ..
        } => (
            "itemGlow".into(),
            format!(
                "{} item {} began glowing.",
                owner_name.as_deref().unwrap_or("Local"),
                item_name
            ),
        ),
        LogEvent::MobSlain {
            mob_name, killer, ..
        } => (
            "mobSlain".into(),
            format!(
                "{mob_name} was slain{}.",
                killer
                    .as_ref()
                    .map(|value| format!(" by {value}"))
                    .unwrap_or_default()
            ),
        ),
        LogEvent::PlayerDeath { killer_name, .. } => (
            "playerDeath".into(),
            format!("The active character was slain by {killer_name}."),
        ),
        other => (
            event_kind(other).into(),
            "Recognized by the shared log parser but not used as outgoing damage.".into(),
        ),
    }
}

fn event_kind(event: &LogEvent) -> &'static str {
    match event {
        LogEvent::Loot { .. } => "loot",
        LogEvent::LevelChanged { .. } => "levelChanged",
        LogEvent::PlayerDeath { .. } => "playerDeath",
        LogEvent::Damage { .. } => "damage",
        LogEvent::ObservedMelee { .. } => "observedMelee",
        LogEvent::IncomingDamage { .. } => "incomingDamage",
        LogEvent::MobSlain { .. } => "mobSlain",
        LogEvent::GroupChange { .. } => "groupChange",
        LogEvent::GroupCleared { .. } => "groupCleared",
        LogEvent::ItemGlow { .. } => "itemGlow",
        LogEvent::SpellCastStarted { .. } => "spellCastStarted",
        LogEvent::CombatAttempt { .. } => "combatAttempt",
        LogEvent::MerchantListing { .. } => "merchantListing",
        LogEvent::DirectTell { .. } => "directTell",
        LogEvent::TradeOffer { .. } => "tradeOffer",
        LogEvent::LinkedItems { .. } => "linkedItems",
        LogEvent::ClericHealCall { .. } => "clericHealCall",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::spell_catalog::{SpellEffect, SpellInfo};

    #[test]
    fn training_preview_uses_real_dot_tracker_without_persisting() {
        let directory = tempfile::tempdir().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        catalog
            .save_for_test(&SpellInfo {
                spell_name: "Dawncall".into(),
                wiki_url: "https://wiki.project1999.com/Dawncall".into(),
                description: String::new(),
                classes: vec![],
                effects: vec![SpellEffect {
                    slot: 1,
                    description: "Decrease Hitpoints by 125 per tick".into(),
                }],
                mana: String::new(),
                skill: String::new(),
                casting_time: String::new(),
                recast_time: String::new(),
                fizzle_time: String::new(),
                resist: String::new(),
                range: String::new(),
                target_type: String::new(),
                spell_type: String::new(),
                duration: "6 ticks".into(),
                reagent: String::new(),
                focus: String::new(),
                where_to_obtain: String::new(),
                cast_on_you: String::new(),
                cast_on_other: "Someone staggers as the light of dawn washes over it.".into(),
                wears_off: String::new(),
                damage_kind: "dot".into(),
                damage_per_tick: Some(125),
                tick_count: Some(6),
                tick_interval_seconds: 6,
                total_dot_damage: Some(750),
                fetched_at: "2026-09-06T00:00:00Z".into(),
                stale: false,
            })
            .unwrap();
        let report=analyze(&catalog,"[Sun Sep 06 10:52:59 2026] You begin casting Dawncall.\n[Sun Sep 06 10:53:04 2026] Hexbone skeleton staggers as the light of dawn washes over it.","Asquatii",true).unwrap();
        assert_eq!(report.encounters.len(), 1);
        assert_eq!(report.encounters[0].spell_damage, 750);
        assert_eq!(report.encounters[0].dots[0].caster_name, "Asquatii");
        assert_eq!(
            report.encounters[0].dots[0].attribution_method,
            "direct_cast"
        );
        assert_eq!(report.lines[1].status, "dot");

        let proc_report = analyze(
            &catalog,
            "[Sun Sep 06 10:53:25 2026] a mortiferous golem staggers as the light of dawn washes over it.\n[Sun Sep 06 10:53:26 2026] Asquatii tries to crush a mortiferous golem, but a mortiferous golem ripostes!",
            "Asquatii",
            false,
        )
        .unwrap();
        assert_eq!(proc_report.encounters[0].dots[0].caster_name, "Asquatii");
        assert_eq!(proc_report.encounters[0].dots[0].attribution_method, "proc");
        assert!(proc_report.lines[1]
            .decisions
            .iter()
            .any(|decision| decision.contains("immediately preceding")));
    }
}
