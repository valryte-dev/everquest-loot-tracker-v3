use chrono::NaiveDateTime;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;

const QUERY_CHUNK_SIZE: usize = 400;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellDamageMetric {
    pub player_name: String,
    pub spell_name: String,
    pub proc_count: u64,
    pub direct_proc_damage: u64,
    pub dot_damage: u64,
    pub dot_tick_count: u64,
    pub proc_dot_damage: u64,
    pub total_proc_damage: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedSpellActivity {
    pub id: i64,
    pub spell_name: String,
    pub target_name: String,
    pub caster_name: String,
    pub source_kind: String,
    pub source_name: Option<String>,
    pub happened_at: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounterSpellMetrics {
    pub proc_count: u64,
    pub direct_proc_damage: u64,
    pub dot_damage: u64,
    pub proc_dot_damage: u64,
    pub total_proc_damage: u64,
    pub spells: Vec<SpellDamageMetric>,
}

pub(super) fn close_encounter(
    connection: &mut Connection,
    encounter_id: i64,
) -> Result<bool, String> {
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let changed = transaction
        .execute(
            "UPDATE damage_encounters
             SET outcome='disengaged',ended_at=last_damage_at
             WHERE id=? AND outcome='active'",
            [encounter_id],
        )
        .map_err(|error| error.to_string())?;
    if changed > 0 {
        transaction
            .execute(
                "DELETE FROM app_settings
                 WHERE key IN ('damage_target_character','damage_target_encounter_id')
                   AND EXISTS(
                     SELECT 1 FROM app_settings
                     WHERE key='damage_target_encounter_id' AND value=CAST(? AS TEXT)
                   )",
                [encounter_id],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "UPDATE dot_applications
                 SET status='disengaged',ended_at=(SELECT last_damage_at FROM damage_encounters WHERE id=?)
                 WHERE encounter_id=? AND status='active'",
                params![encounter_id, encounter_id],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(changed > 0)
}
pub(super) fn set_preferred_target(
    connection: &mut Connection,
    character: &str,
    encounter_id: Option<i64>,
) -> Result<(), String> {
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    if let Some(id) = encounter_id {
        let valid = transaction
            .query_row(
                "SELECT 1 FROM damage_encounters
                 WHERE id=? AND character_name=? COLLATE NOCASE AND outcome='active'",
                params![id, character],
                |_| Ok(()),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .is_some();
        if !valid {
            return Err(
                "The selected target is no longer an active encounter for this character.".into(),
            );
        }
        transaction
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('damage_target_character',?)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [character],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('damage_target_encounter_id',?)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [id.to_string()],
            )
            .map_err(|error| error.to_string())?;
    } else {
        transaction
            .execute(
                "DELETE FROM app_settings
                 WHERE key IN ('damage_target_character','damage_target_encounter_id')",
                [],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn insert_or_get_damage_encounter(
    connection: &Connection,
    source: &str,
    source_offset: i64,
    character: &str,
    happened_at: NaiveDateTime,
    mob_name: &str,
) -> Result<i64, String> {
    if let Some(id) = connection
        .query_row(
            "SELECT id FROM damage_encounters
             WHERE source_file=? AND first_source_offset=?",
            params![source, source_offset],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
    {
        return Ok(id);
    }
    let inserted = connection
        .execute(
            "INSERT OR IGNORE INTO damage_encounters(
                character_name,mob_name,started_at,last_damage_at,source_file,
                first_source_offset,last_source_offset
             ) VALUES(?,?,?,?,?,?,?)",
            params![
                character,
                mob_name,
                happened_at.to_string(),
                happened_at.to_string(),
                source,
                source_offset,
                source_offset
            ],
        )
        .map_err(|error| error.to_string())?;
    if inserted > 0 {
        return Ok(connection.last_insert_rowid());
    }
    connection
        .query_row(
            "SELECT id FROM damage_encounters
             WHERE source_file=? AND first_source_offset=?",
            params![source, source_offset],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
}
pub fn load_for_encounters(
    connection: &Connection,
    encounter_ids: &[i64],
) -> Result<HashMap<i64, EncounterSpellMetrics>, String> {
    let mut by_encounter = HashMap::new();
    for chunk in encounter_ids.chunks(QUERY_CHUNK_SIZE) {
        let placeholders = std::iter::repeat_n("?", chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT encounter_id,caster_name,spell_name,proc_count,direct_proc_damage,
                    dot_damage,dot_tick_count,proc_dot_damage
             FROM damage_spell_summaries
             WHERE encounter_id IN ({placeholders})
             ORDER BY encounter_id,caster_name COLLATE NOCASE,spell_name COLLATE NOCASE"
        );
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params_from_iter(chunk.iter().copied()), |row| {
                let direct_proc_damage = row.get::<_, u64>(4)?;
                let proc_dot_damage = row.get::<_, u64>(7)?;
                Ok((
                    row.get::<_, i64>(0)?,
                    SpellDamageMetric {
                        player_name: row.get(1)?,
                        spell_name: row.get(2)?,
                        proc_count: row.get(3)?,
                        direct_proc_damage,
                        dot_damage: row.get(5)?,
                        dot_tick_count: row.get(6)?,
                        proc_dot_damage,
                        total_proc_damage: direct_proc_damage + proc_dot_damage,
                    },
                ))
            })
            .map_err(|error| error.to_string())?;
        for row in rows {
            let (encounter_id, metric) = row.map_err(|error| error.to_string())?;
            let encounter = by_encounter
                .entry(encounter_id)
                .or_insert_with(EncounterSpellMetrics::default);
            encounter.proc_count += metric.proc_count;
            encounter.direct_proc_damage += metric.direct_proc_damage;
            encounter.dot_damage += metric.dot_damage;
            encounter.proc_dot_damage += metric.proc_dot_damage;
            encounter.total_proc_damage += metric.total_proc_damage;
            encounter.spells.push(metric);
        }
    }
    Ok(by_encounter)
}

pub fn load_activity_for_encounters(
    connection: &Connection,
    encounter_ids: &[i64],
) -> Result<HashMap<i64, Vec<TrackedSpellActivity>>, String> {
    let mut by_encounter: HashMap<i64, Vec<TrackedSpellActivity>> = HashMap::new();
    for chunk in encounter_ids.chunks(QUERY_CHUNK_SIZE) {
        let placeholders = std::iter::repeat_n("?", chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id,encounter_id,spell_name,target_name,caster_name,source_kind,source_name,happened_at
             FROM (
                SELECT id,encounter_id,spell_name,target_name,caster_name,source_kind,source_name,happened_at,
                       ROW_NUMBER() OVER(PARTITION BY encounter_id ORDER BY happened_at DESC,id DESC) AS position
                FROM combat_spell_activity WHERE encounter_id IN ({placeholders})
             ) WHERE position<=24 ORDER BY encounter_id,happened_at DESC,id DESC"
        );
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params_from_iter(chunk.iter().copied()), |row| {
                Ok((
                    row.get::<_, i64>(1)?,
                    TrackedSpellActivity {
                        id: row.get(0)?,
                        spell_name: row.get(2)?,
                        target_name: row.get(3)?,
                        caster_name: row.get(4)?,
                        source_kind: row.get(5)?,
                        source_name: row.get(6)?,
                        happened_at: row.get(7)?,
                    },
                ))
            })
            .map_err(|error| error.to_string())?;
        for row in rows {
            let (encounter_id, activity) = row.map_err(|error| error.to_string())?;
            by_encounter.entry(encounter_id).or_default().push(activity);
        }
    }
    Ok(by_encounter)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn record_inferred_proc_damage(
    connection: &rusqlite::Connection,
    encounter_id: i64,
    occurrence_id: i64,
    happened_at: NaiveDateTime,
    caster_name: &str,
    spell_name: &str,
    amount: u64,
) -> Result<usize, String> {
    let source = format!("proc://{occurrence_id}");
    let created = connection
        .execute(
            "INSERT OR IGNORE INTO damage_events(
                encounter_id,happened_at,damage_type,attack_kind,damage,raw_line,source_file,
                source_offset,weapon_loadout_id,attacker_name
             ) VALUES(?,?,'spell',?,?,?,?,1,NULL,?)",
            params![
                encounter_id,
                happened_at.to_string(),
                spell_name,
                amount as i64,
                format!("Inferred direct proc damage from {spell_name}"),
                source,
                caster_name,
            ],
        )
        .map_err(|error| error.to_string())?;
    if created > 0 {
        connection
            .execute(
                "UPDATE damage_encounters SET last_damage_at=MAX(last_damage_at,?),
                 total_damage=total_damage+?,spell_damage=spell_damage+?,hit_count=hit_count+1,
                 max_hit=MAX(max_hit,?) WHERE id=?",
                params![
                    happened_at.to_string(),
                    amount as i64,
                    amount as i64,
                    amount as i64,
                    encounter_id
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(created)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn record_inferred_dot_tick(
    connection: &rusqlite::Connection,
    encounter_id: i64,
    application_id: i64,
    tick_index: i64,
    happened_at: NaiveDateTime,
    caster_name: &str,
    spell_name: &str,
    amount: u64,
) -> Result<usize, String> {
    let source = format!("dot://{application_id}");
    let created = connection
        .execute(
            "INSERT OR IGNORE INTO damage_events(
            encounter_id,happened_at,damage_type,attack_kind,damage,raw_line,source_file,
            source_offset,weapon_loadout_id,attacker_name
         ) VALUES(?,?,'spell',?,?,?,?,?,NULL,?)",
            params![
                encounter_id,
                happened_at.to_string(),
                spell_name,
                amount as i64,
                format!("Inferred {spell_name} tick {tick_index}"),
                source,
                tick_index,
                caster_name,
            ],
        )
        .map_err(|error| error.to_string())?;
    if created > 0 {
        connection
            .execute(
                "UPDATE damage_encounters SET last_damage_at=MAX(last_damage_at,?),
             total_damage=total_damage+?,spell_damage=spell_damage+?,hit_count=hit_count+1,
             max_hit=MAX(max_hit,?) WHERE id=?",
                params![
                    happened_at.to_string(),
                    amount as i64,
                    amount as i64,
                    amount as i64,
                    encounter_id
                ],
            )
            .map_err(|error| error.to_string())?;
        let proc_dot_damage: i64 = connection
            .query_row(
                "SELECT CASE WHEN EXISTS(
                    SELECT 1 FROM proc_occurrences WHERE dot_application_id=?
                 ) THEN ? ELSE 0 END",
                params![application_id, amount as i64],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO damage_spell_summaries(
                    encounter_id,caster_name,spell_name,dot_damage,dot_tick_count,proc_dot_damage
                 ) VALUES(?,?,?, ?,1,?)
                 ON CONFLICT(encounter_id,caster_name,spell_name) DO UPDATE SET
                    dot_damage=dot_damage+excluded.dot_damage,
                    dot_tick_count=dot_tick_count+1,
                    proc_dot_damage=proc_dot_damage+excluded.proc_dot_damage",
                params![
                    encounter_id,
                    caster_name,
                    spell_name,
                    amount as i64,
                    proc_dot_damage
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(created)
}

pub(super) fn correct_encounter_target(
    connection: &mut Connection,
    encounter_id: i64,
    corrected_mob_name: &str,
) -> Result<i64, String> {
    let corrected = corrected_mob_name.trim();
    if corrected.is_empty() || corrected.len() > 120 || corrected.chars().any(char::is_control) {
        return Err("Enter a target name between 1 and 120 characters.".into());
    }
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let source: Option<(String, String, String)> = transaction
        .query_row(
            "SELECT character_name,mob_name,source_file FROM damage_encounters WHERE id=? AND outcome='active'",
            [encounter_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let (character, observed, source_file) = source.ok_or_else(|| {
        "The fight is no longer active and cannot be corrected on the fly.".to_string()
    })?;
    let destination = transaction
        .query_row(
            "SELECT id FROM damage_encounters
             WHERE id<>? AND character_name=? COLLATE NOCASE AND source_file=?
               AND mob_name=? COLLATE NOCASE AND outcome='active'
             ORDER BY last_damage_at DESC,id DESC LIMIT 1",
            params![encounter_id, character, source_file, corrected],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    for table in [
        "dot_applications",
        "proc_occurrences",
        "combat_spell_activity",
    ] {
        transaction
            .execute(
                &format!("UPDATE {table} SET target_name=? WHERE encounter_id=? AND target_name=? COLLATE NOCASE"),
                params![corrected, encounter_id, observed],
            )
            .map_err(|error| error.to_string())?;
    }

    let corrected_id = if let Some(target_id) = destination {
        let source_totals: (String, String, i64, i64, i64, i64, i64, i64, i64) = transaction
            .query_row(
                "SELECT started_at,last_damage_at,total_damage,melee_damage,spell_damage,
                        hit_count,max_hit,first_source_offset,last_source_offset
                 FROM damage_encounters WHERE id=?",
                [encounter_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO damage_participant_summaries(encounter_id,attacker_name,total_damage,hit_count,first_damage_at,last_damage_at)
                 SELECT ?,attacker_name,total_damage,hit_count,first_damage_at,last_damage_at
                 FROM damage_participant_summaries WHERE encounter_id=?
                 ON CONFLICT(encounter_id,attacker_name) DO UPDATE SET
                    total_damage=total_damage+excluded.total_damage,
                    hit_count=hit_count+excluded.hit_count,
                    first_damage_at=MIN(first_damage_at,excluded.first_damage_at),
                    last_damage_at=MAX(last_damage_at,excluded.last_damage_at)",
                params![target_id, encounter_id],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO damage_target_summaries(encounter_id,target_name,total_damage,hit_count,max_hit,first_damage_at,last_damage_at)
                 SELECT ?,target_name,total_damage,hit_count,max_hit,first_damage_at,last_damage_at
                 FROM damage_target_summaries WHERE encounter_id=?
                 ON CONFLICT(encounter_id,target_name) DO UPDATE SET
                    total_damage=total_damage+excluded.total_damage,
                    hit_count=hit_count+excluded.hit_count,
                    max_hit=MAX(max_hit,excluded.max_hit),
                    first_damage_at=MIN(first_damage_at,excluded.first_damage_at),
                    last_damage_at=MAX(last_damage_at,excluded.last_damage_at)",
                params![target_id, encounter_id],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM damage_participant_summaries WHERE encounter_id=?",
                [encounter_id],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM damage_target_summaries WHERE encounter_id=?",
                [encounter_id],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO damage_spell_summaries(
                    encounter_id,caster_name,spell_name,proc_count,direct_proc_damage,
                    dot_damage,dot_tick_count,proc_dot_damage
                 )
                 SELECT ?,caster_name,spell_name,proc_count,direct_proc_damage,
                        dot_damage,dot_tick_count,proc_dot_damage
                 FROM damage_spell_summaries WHERE encounter_id=?
                 ON CONFLICT(encounter_id,caster_name,spell_name) DO UPDATE SET
                    proc_count=proc_count+excluded.proc_count,
                    direct_proc_damage=direct_proc_damage+excluded.direct_proc_damage,
                    dot_damage=dot_damage+excluded.dot_damage,
                    dot_tick_count=dot_tick_count+excluded.dot_tick_count,
                    proc_dot_damage=proc_dot_damage+excluded.proc_dot_damage",
                params![target_id, encounter_id],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM damage_spell_summaries WHERE encounter_id=?",
                [encounter_id],
            )
            .map_err(|error| error.to_string())?;
        for table in [
            "damage_events",
            "damage_received_events",
            "dot_applications",
            "proc_occurrences",
            "combat_spell_activity",
        ] {
            transaction
                .execute(
                    &format!("UPDATE {table} SET encounter_id=? WHERE encounter_id=?"),
                    params![target_id, encounter_id],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction
            .execute("DELETE FROM damage_encounters WHERE id=?", [encounter_id])
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "UPDATE damage_encounters SET
                    started_at=MIN(started_at,?),last_damage_at=MAX(last_damage_at,?),
                    total_damage=total_damage+?,melee_damage=melee_damage+?,
                    spell_damage=spell_damage+?,hit_count=hit_count+?,max_hit=MAX(max_hit,?),
                    first_source_offset=MIN(first_source_offset,?),last_source_offset=MAX(last_source_offset,?)
                 WHERE id=?",
                params![
                    source_totals.0, source_totals.1, source_totals.2, source_totals.3,
                    source_totals.4, source_totals.5, source_totals.6, source_totals.7,
                    source_totals.8, target_id
                ],
            )
            .map_err(|error| error.to_string())?;
        target_id
    } else {
        transaction
            .execute(
                "UPDATE damage_encounters SET mob_name=? WHERE id=?",
                params![corrected, encounter_id],
            )
            .map_err(|error| error.to_string())?;
        encounter_id
    };

    remove_corrected_target_from_meters(&transaction, corrected_id, corrected)?;

    transaction
        .execute(
            "INSERT INTO app_settings(key,value) VALUES('damage_target_character',?)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [&character],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO app_settings(key,value) VALUES('damage_target_encounter_id',?)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [corrected_id.to_string()],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(corrected_id)
}

fn remove_corrected_target_from_meters(
    connection: &Connection,
    encounter_id: i64,
    target_name: &str,
) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM proc_occurrences
             WHERE encounter_id=? AND caster_name=? COLLATE NOCASE",
            params![encounter_id, target_name],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM dot_applications
             WHERE encounter_id=? AND caster_name=? COLLATE NOCASE",
            params![encounter_id, target_name],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM combat_spell_activity
             WHERE encounter_id=? AND caster_name=? COLLATE NOCASE",
            params![encounter_id, target_name],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM damage_spell_summaries
             WHERE encounter_id=? AND caster_name=? COLLATE NOCASE",
            params![encounter_id, target_name],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM damage_events
             WHERE encounter_id=? AND attacker_name=? COLLATE NOCASE",
            params![encounter_id, target_name],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM damage_participant_summaries
             WHERE encounter_id=? AND attacker_name=? COLLATE NOCASE",
            params![encounter_id, target_name],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM damage_received_events
             WHERE encounter_id=? AND target_name=? COLLATE NOCASE",
            params![encounter_id, target_name],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM damage_target_summaries
             WHERE encounter_id=? AND target_name=? COLLATE NOCASE",
            params![encounter_id, target_name],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "UPDATE damage_encounters SET
                total_damage=COALESCE((SELECT SUM(damage) FROM damage_events WHERE encounter_id=?),0),
                melee_damage=COALESCE((SELECT SUM(CASE WHEN damage_type='melee' THEN damage ELSE 0 END) FROM damage_events WHERE encounter_id=?),0),
                spell_damage=COALESCE((SELECT SUM(CASE WHEN damage_type<>'melee' THEN damage ELSE 0 END) FROM damage_events WHERE encounter_id=?),0),
                hit_count=COALESCE((SELECT COUNT(*) FROM damage_events WHERE encounter_id=?),0),
                max_hit=COALESCE((SELECT MAX(damage) FROM damage_events WHERE encounter_id=?),0)
             WHERE id=?",
            params![
                encounter_id,
                encounter_id,
                encounter_id,
                encounter_id,
                encounter_id,
                encounter_id
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::Database;

    #[test]
    fn manually_closing_an_encounter_preserves_it_as_disengaged() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let mut connection = database.connect().unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,source_file,first_source_offset,last_source_offset)
             VALUES('Tester','Target','2026-09-08 10:00:00','2026-09-08 10:00:07','test.log',1,2)",
            [],
        ).unwrap();
        let id = connection.last_insert_rowid();

        assert!(close_encounter(&mut connection, id).unwrap());
        let state: (String, Option<String>) = connection
            .query_row(
                "SELECT outcome,ended_at FROM damage_encounters WHERE id=?",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            state,
            ("disengaged".into(), Some("2026-09-08 10:00:07".into()))
        );
        assert!(!close_encounter(&mut connection, id).unwrap());
    }
    #[test]
    fn preferred_target_is_validated_scoped_and_clearable() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let mut connection = database.connect().unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,source_file,first_source_offset,last_source_offset)
             VALUES('Cleric','Raid Target','2026-09-08 10:00:00','2026-09-08 10:00:07','test.log',1,2)",
            [],
        ).unwrap();
        let id = connection.last_insert_rowid();

        assert!(set_preferred_target(&mut connection, "Other", Some(id)).is_err());
        set_preferred_target(&mut connection, "Cleric", Some(id)).unwrap();
        let selected: (String, String) = connection
            .query_row(
                "SELECT
               (SELECT value FROM app_settings WHERE key='damage_target_character'),
               (SELECT value FROM app_settings WHERE key='damage_target_encounter_id')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(selected, ("Cleric".into(), id.to_string()));

        set_preferred_target(&mut connection, "Cleric", None).unwrap();
        let remaining: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM app_settings WHERE key LIKE 'damage_target_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 0);
    }
    #[test]
    fn loads_bounded_spell_metrics_grouped_by_encounter() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO damage_encounters(
                character_name,mob_name,started_at,last_damage_at,source_file,
                first_source_offset,last_source_offset
             ) VALUES('Tester','Target','2026-09-06 10:00:00','2026-09-06 10:00:01','test',1,2)",
                [],
            )
            .unwrap();
        let encounter_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO damage_spell_summaries(
                encounter_id,caster_name,spell_name,proc_count,direct_proc_damage,
                dot_damage,dot_tick_count,proc_dot_damage
             ) VALUES(?,?,?,?,?,?,?,?)",
                rusqlite::params![encounter_id, "Judoku", "Essence Tap", 2, 40, 0, 0, 0],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO damage_spell_summaries(
                encounter_id,caster_name,spell_name,proc_count,direct_proc_damage,
                dot_damage,dot_tick_count,proc_dot_damage
             ) VALUES(?,?,?,?,?,?,?,?)",
                rusqlite::params![encounter_id, "Judoku", "Dawncall", 1, 0, 500, 4, 500],
            )
            .unwrap();

        let result = load_for_encounters(&connection, &[encounter_id]).unwrap();
        let metrics = result.get(&encounter_id).unwrap();
        assert_eq!(metrics.proc_count, 3);
        assert_eq!(metrics.direct_proc_damage, 40);
        assert_eq!(metrics.dot_damage, 500);
        assert_eq!(metrics.proc_dot_damage, 500);
        assert_eq!(metrics.total_proc_damage, 540);
        assert_eq!(metrics.spells.len(), 2);
        assert!(load_for_encounters(&connection, &[encounter_id + 1])
            .unwrap()
            .is_empty());
        connection.execute(
            "INSERT INTO combat_spell_activity(encounter_id,spell_name,target_name,caster_name,source_kind,source_name,happened_at,source_file,landing_source_offset) VALUES(?,'Dawncall','Target','Judoku','item_click','Great Spear of Dawn','2026-09-06 10:00:02','test',3)",
            [encounter_id],
        ).unwrap();
        let activity = load_activity_for_encounters(&connection, &[encounter_id]).unwrap();
        let row = &activity[&encounter_id][0];
        assert_eq!(row.caster_name, "Judoku");
        assert_eq!(row.source_kind, "item_click");
        assert_eq!(row.source_name.as_deref(), Some("Great Spear of Dawn"));
    }
    #[test]
    fn correcting_a_pet_target_merges_existing_mob_fight_without_losing_metrics() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let mut connection = database.connect().unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,total_damage,melee_damage,hit_count,max_hit,source_file,first_source_offset,last_source_offset)
             VALUES('Valmezz','Treasure Chest','2026-09-09 10:00:00','2026-09-09 10:00:06',100,100,2,60,'eqlog_Valmezz.txt',1,3)",
            [],
        ).unwrap();
        let mistaken_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,total_damage,melee_damage,hit_count,max_hit,source_file,first_source_offset,last_source_offset)
             VALUES('Valmezz','Grenn','2026-09-09 10:00:02','2026-09-09 10:00:08',200,200,3,100,'eqlog_Valmezz.txt',10,12)",
            [],
        ).unwrap();
        let grenn_id = connection.last_insert_rowid();
        connection.execute("INSERT INTO damage_events(encounter_id,happened_at,damage_type,attack_kind,damage,raw_line,source_file,source_offset,attacker_name) VALUES(?,'2026-09-09 10:00:01','melee','hit',100,'pet hit','eqlog_Valmezz.txt',2,'Valmezz')",[mistaken_id]).unwrap();
        connection.execute("INSERT INTO damage_events(encounter_id,happened_at,damage_type,attack_kind,damage,raw_line,source_file,source_offset,attacker_name) VALUES(?,'2026-09-09 10:00:03','melee','hit',200,'player hit','eqlog_Valmezz.txt',11,'Valmezz')",[grenn_id]).unwrap();
        connection.execute("INSERT INTO damage_events(encounter_id,happened_at,damage_type,attack_kind,damage,raw_line,source_file,source_offset,attacker_name) VALUES(?,'2026-09-09 10:00:04','melee','hit',75,'misclassified target hit','eqlog_Valmezz.txt',12,'Grenn')",[grenn_id]).unwrap();
        connection.execute("INSERT INTO damage_received_events(encounter_id,happened_at,attacker_name,target_name,attack_kind,damage,raw_line,source_file,source_offset) VALUES(?,'2026-09-09 10:00:04','Grenn','Valmezz','hit',50,'incoming','eqlog_Valmezz.txt',13)",[mistaken_id]).unwrap();
        connection.execute("INSERT INTO damage_received_events(encounter_id,happened_at,attacker_name,target_name,attack_kind,damage,raw_line,source_file,source_offset) VALUES(?,'2026-09-09 10:00:04','Other Mob','Grenn','hit',25,'misclassified incoming target','eqlog_Valmezz.txt',15)",[grenn_id]).unwrap();
        connection.execute("INSERT INTO damage_spell_summaries(encounter_id,caster_name,spell_name,proc_count,direct_proc_damage) VALUES(?,'Valmezz','Test Proc',1,20)",[mistaken_id]).unwrap();
        connection.execute("INSERT INTO damage_spell_summaries(encounter_id,caster_name,spell_name,proc_count,direct_proc_damage) VALUES(?,'Valmezz','Test Proc',2,40)",[grenn_id]).unwrap();
        connection.execute("INSERT INTO combat_spell_activity(encounter_id,spell_name,target_name,caster_name,source_kind,happened_at,source_file,landing_source_offset) VALUES(?,'Test Proc','Treasure Chest','Valmezz','proc','2026-09-09 10:00:05','eqlog_Valmezz.txt',14)",[mistaken_id]).unwrap();

        assert_eq!(
            correct_encounter_target(&mut connection, mistaken_id, "Grenn").unwrap(),
            grenn_id
        );
        let encounter: (String, i64, i64, i64) = connection
            .query_row(
                "SELECT mob_name,total_damage,hit_count,max_hit FROM damage_encounters WHERE id=?",
                [grenn_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(encounter, ("Grenn".into(), 300, 2, 200));
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM damage_events WHERE encounter_id=?",
                    [grenn_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            2
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM damage_received_events WHERE encounter_id=?",
                    [grenn_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        let participant_total: (i64, i64) = connection
            .query_row(
                "SELECT total_damage,hit_count FROM damage_participant_summaries WHERE encounter_id=? AND attacker_name='Valmezz'",
                [grenn_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(participant_total, (300, 2));
        let incoming_total: (i64, i64) = connection
            .query_row(
                "SELECT total_damage,hit_count FROM damage_target_summaries WHERE encounter_id=? AND target_name='Valmezz'",
                [grenn_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(incoming_total, (50, 1));
        let spell: (i64, i64) = connection.query_row("SELECT proc_count,direct_proc_damage FROM damage_spell_summaries WHERE encounter_id=? AND spell_name='Test Proc'",[grenn_id],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!(spell, (3, 60));
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM damage_participant_summaries WHERE encounter_id=? AND attacker_name='Grenn'",
                    [grenn_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM damage_target_summaries WHERE encounter_id=? AND target_name='Grenn'",
                    [grenn_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        let activity: (i64, String) = connection
            .query_row(
                "SELECT encounter_id,target_name FROM combat_spell_activity",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(activity, (grenn_id, "Grenn".into()));
        assert_eq!(
            connection
                .query_row(
                    "SELECT value FROM app_settings WHERE key='damage_target_encounter_id'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            grenn_id.to_string()
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM damage_encounters", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn correcting_a_target_renames_the_only_active_encounter() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let mut connection = database.connect().unwrap();
        connection.execute("INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,source_file,first_source_offset,last_source_offset) VALUES('Valmezz','Treasure Chest','2026-09-09 10:00:00','2026-09-09 10:00:06','eqlog_Valmezz.txt',1,3)",[]).unwrap();
        let id = connection.last_insert_rowid();
        connection.execute("INSERT INTO damage_events(encounter_id,happened_at,damage_type,attack_kind,damage,raw_line,source_file,source_offset,attacker_name) VALUES(?,'2026-09-09 10:00:01','melee','hit',100,'player hit','eqlog_Valmezz.txt',2,'Valmezz')",[id]).unwrap();
        connection.execute("INSERT INTO damage_events(encounter_id,happened_at,damage_type,attack_kind,damage,raw_line,source_file,source_offset,attacker_name) VALUES(?,'2026-09-09 10:00:02','melee','hit',50,'misclassified target hit','eqlog_Valmezz.txt',3,'Grenn')",[id]).unwrap();
        connection.execute("UPDATE damage_encounters SET total_damage=150,melee_damage=150,hit_count=2,max_hit=100 WHERE id=?",[id]).unwrap();
        connection.execute("INSERT INTO damage_spell_summaries(encounter_id,caster_name,spell_name,proc_count,direct_proc_damage) VALUES(?,'Grenn','Wrong Proc',1,50)",[id]).unwrap();
        assert_eq!(
            correct_encounter_target(&mut connection, id, "Grenn").unwrap(),
            id
        );
        let corrected: (String, i64, i64, i64) = connection
            .query_row(
                "SELECT mob_name,total_damage,hit_count,max_hit FROM damage_encounters WHERE id=?",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(corrected, ("Grenn".into(), 100, 1, 100));
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM damage_participant_summaries WHERE encounter_id=? AND attacker_name='Grenn'",
                    [id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM damage_spell_summaries WHERE encounter_id=? AND caster_name='Grenn'",
                    [id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }
}
