use rusqlite::OptionalExtension;
use serde::Serialize;
use std::{
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::Path,
};

use crate::infrastructure::database::Database;

const EVIDENCE_LOOKBACK_BYTES: i64 = 64 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcEvidenceReport {
    encounter_id: i64,
    player_name: String,
    occurrences: Vec<ProcEvidenceOccurrence>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcEvidenceOccurrence {
    id: i64,
    spell_name: String,
    target_name: String,
    caster_name: String,
    happened_at: String,
    direct_damage: i64,
    source_name: Option<String>,
    source_file: String,
    landing_source_offset: i64,
    attribution_source_offset: i64,
    evidence_source: String,
    messages: Vec<ProcEvidenceMessage>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcEvidenceMessage {
    source_offset: i64,
    role: String,
    raw_line: String,
}

struct StoredOccurrence {
    id: i64,
    spell_name: String,
    target_name: String,
    caster_name: String,
    happened_at: String,
    direct_damage: i64,
    source_name: Option<String>,
    source_file: String,
    landing_source_offset: i64,
    attribution_source_offset: i64,
}

pub fn load(
    database: &Database,
    encounter_id: i64,
    player_name: &str,
) -> Result<ProcEvidenceReport, String> {
    let player_name = player_name.trim();
    if player_name.is_empty() || player_name.len() > 128 {
        return Err("Choose a valid proc contributor".into());
    }
    let connection = database.connect().map_err(|error| error.to_string())?;
    let encounter_exists = connection
        .query_row(
            "SELECT 1 FROM damage_encounters WHERE id=?",
            [encounter_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .is_some();
    if !encounter_exists {
        return Err("Damage encounter was not found".into());
    }
    let mut statement = connection
        .prepare(
            "SELECT p.id,p.spell_name,p.target_name,p.caster_name,p.happened_at,
                    p.direct_damage,a.source_name,p.source_file,
                    p.landing_source_offset,p.attribution_source_offset
             FROM proc_occurrences p
             LEFT JOIN combat_spell_activity a
               ON a.encounter_id=p.encounter_id
              AND a.source_file=p.source_file
              AND a.landing_source_offset=p.landing_source_offset
              AND a.spell_name=p.spell_name COLLATE NOCASE
             WHERE p.encounter_id=? AND p.caster_name=? COLLATE NOCASE
             ORDER BY p.happened_at,p.id
             LIMIT 1000",
        )
        .map_err(|error| error.to_string())?;
    let stored = statement
        .query_map(rusqlite::params![encounter_id, player_name], |row| {
            Ok(StoredOccurrence {
                id: row.get(0)?,
                spell_name: row.get(1)?,
                target_name: row.get(2)?,
                caster_name: row.get(3)?,
                happened_at: row.get(4)?,
                direct_damage: row.get(5)?,
                source_name: row.get(6)?,
                source_file: row.get(7)?,
                landing_source_offset: row.get(8)?,
                attribution_source_offset: row.get(9)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);

    let mut occurrences = Vec::with_capacity(stored.len());
    for occurrence in stored {
        let mut messages = capture_log_evidence(
            Path::new(&occurrence.source_file),
            occurrence.landing_source_offset,
            occurrence.attribution_source_offset,
        )
        .unwrap_or_default();
        let evidence_source = if messages.is_empty() {
            messages = retained_evidence(
                &connection,
                encounter_id,
                &occurrence.source_file,
                occurrence.landing_source_offset,
                occurrence.attribution_source_offset,
            )?;
            if messages.is_empty() {
                "unavailable"
            } else {
                "retained-events"
            }
        } else {
            "source-log"
        };
        occurrences.push(ProcEvidenceOccurrence {
            id: occurrence.id,
            spell_name: occurrence.spell_name,
            target_name: occurrence.target_name,
            caster_name: occurrence.caster_name,
            happened_at: occurrence.happened_at,
            direct_damage: occurrence.direct_damage,
            source_name: occurrence.source_name,
            source_file: occurrence.source_file,
            landing_source_offset: occurrence.landing_source_offset,
            attribution_source_offset: occurrence.attribution_source_offset,
            evidence_source: evidence_source.into(),
            messages,
        });
    }
    Ok(ProcEvidenceReport {
        encounter_id,
        player_name: player_name.into(),
        occurrences,
    })
}

fn capture_log_evidence(
    path: &Path,
    landing_offset: i64,
    attribution_offset: i64,
) -> Result<Vec<ProcEvidenceMessage>, String> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let lower = landing_offset.min(attribution_offset).max(0);
    let upper = landing_offset.max(attribution_offset).max(0);
    let seek_offset = lower.saturating_sub(EVIDENCE_LOOKBACK_BYTES).max(0) as u64;
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    file.seek(SeekFrom::Start(seek_offset))
        .map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(file);
    let mut offset = seek_offset as i64;
    if seek_offset > 0 {
        let mut partial = Vec::new();
        offset += reader
            .read_until(b'\n', &mut partial)
            .map_err(|error| error.to_string())? as i64;
    }
    let mut previous: Option<(i64, String)> = None;
    let mut selected = Vec::new();
    loop {
        let mut bytes = Vec::new();
        let read = reader
            .read_until(b'\n', &mut bytes)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        let line_offset = offset;
        offset += read as i64;
        while bytes
            .last()
            .is_some_and(|byte| *byte == b'\n' || *byte == b'\r')
        {
            bytes.pop();
        }
        let line = String::from_utf8_lossy(&bytes).into_owned();
        if line_offset < lower {
            previous = Some((line_offset, line));
            continue;
        }
        if selected.is_empty() {
            if let Some((source_offset, raw_line)) = previous.take() {
                selected.push(message(
                    source_offset,
                    landing_offset,
                    attribution_offset,
                    raw_line,
                ));
            }
        }
        selected.push(message(
            line_offset,
            landing_offset,
            attribution_offset,
            line,
        ));
        if line_offset >= upper {
            break;
        }
    }
    Ok(selected)
}

fn retained_evidence(
    connection: &rusqlite::Connection,
    encounter_id: i64,
    source_file: &str,
    landing_offset: i64,
    attribution_offset: i64,
) -> Result<Vec<ProcEvidenceMessage>, String> {
    let lower = landing_offset.min(attribution_offset);
    let upper = landing_offset.max(attribution_offset);
    let mut statement = connection
        .prepare(
            "SELECT source_offset,raw_line FROM damage_events
             WHERE encounter_id=? AND source_file=? AND source_offset BETWEEN ? AND ?
             ORDER BY source_offset,id",
        )
        .map_err(|error| error.to_string())?;
    let messages = statement
        .query_map(
            rusqlite::params![encounter_id, source_file, lower, upper],
            |row| {
                let source_offset = row.get::<_, i64>(0)?;
                Ok(message(
                    source_offset,
                    landing_offset,
                    attribution_offset,
                    row.get(1)?,
                ))
            },
        )
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(messages)
}

fn message(
    source_offset: i64,
    landing_offset: i64,
    attribution_offset: i64,
    raw_line: String,
) -> ProcEvidenceMessage {
    let role = if source_offset == landing_offset && source_offset == attribution_offset {
        "landing-confirmation"
    } else if source_offset == landing_offset {
        "landing"
    } else if source_offset == attribution_offset {
        "confirmation"
    } else if source_offset < landing_offset.min(attribution_offset) {
        "preceding-context"
    } else {
        "supporting"
    };
    ProcEvidenceMessage {
        source_offset,
        role: role.into(),
        raw_line,
    }
}

#[cfg(test)]
mod tests {
    use super::{capture_log_evidence, load};
    use crate::infrastructure::database::Database;
    use rusqlite::params;

    #[test]
    fn proc_evidence_returns_the_landing_and_attack_messages_from_the_source_log() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("eqlog_Test_P1999Green.txt");
        let context = "[Mon Sep 07 12:47:41 2026] Context line.\r\n";
        let damage = "[Mon Sep 07 12:47:42 2026] a helot spectre was hit by non-melee for 120 points of damage.\r\n";
        let landing =
            "[Mon Sep 07 12:47:42 2026] A helot spectre begins to spin from one hundred blows.\r\n";
        let attack = "[Mon Sep 07 12:47:42 2026] You try to crush a helot spectre, but miss!\r\n";
        let text = format!("{context}{damage}{landing}{attack}");
        std::fs::write(&log, text.as_bytes()).unwrap();
        let landing_offset = (context.len() + damage.len()) as i64;
        let attribution_offset = (context.len() + damage.len() + landing.len()) as i64;
        let captured = capture_log_evidence(&log, landing_offset, attribution_offset).unwrap();
        assert_eq!(captured.len(), 3);
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,source_file,first_source_offset,last_source_offset)
             VALUES('Test','a helot spectre','2026-09-07 12:47:42','2026-09-07 12:47:42',?,0,?)",
            params![log.display().to_string(), attribution_offset],
        ).unwrap();
        let encounter_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO proc_occurrences(encounter_id,spell_name,target_name,caster_name,happened_at,direct_damage,source_file,landing_source_offset,attribution_source_offset)
             VALUES(?,'One Hundred Blows','a helot spectre','Test','2026-09-07 12:47:42',120,?,?,?)",
            params![encounter_id, log.display().to_string(), landing_offset, attribution_offset],
        ).unwrap();
        drop(connection);

        let report = load(&database, encounter_id, "test").unwrap();
        assert_eq!(report.occurrences.len(), 1);
        let messages = &report.occurrences[0].messages;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "preceding-context");
        assert_eq!(messages[1].role, "landing");
        assert_eq!(messages[2].role, "confirmation");
        assert!(messages[0].raw_line.contains("non-melee for 120"));
        assert!(messages[2].raw_line.contains("but miss"));
    }
}
