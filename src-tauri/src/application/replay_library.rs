use super::proc_coach::{validate_saved_review, ProcCoachSavedReview};
use crate::infrastructure::{database::Database, paths};
use chrono::{NaiveDateTime, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{BufRead, BufReader, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

const FORMAT_VERSION: u32 = 1;
const CONTEXT_LINES: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayFightSummary {
    pub mob_name: String,
    pub started_at: String,
    pub duration_seconds: u64,
    pub total_damage: u64,
    pub participant_count: usize,
    pub event_count: usize,
    pub proc_count: u64,
    pub dot_damage: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaySource {
    pub encounter_id: i64,
    pub source_file: String,
    pub first_source_offset: i64,
    pub last_source_offset: i64,
    pub capture_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayFile {
    pub format_version: u32,
    pub title: String,
    pub saved_at: String,
    pub active_character: String,
    pub project_all_ticks: bool,
    pub summary: ReplayFightSummary,
    pub source: ReplaySource,
    pub log_lines: Vec<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub coach_reviews: Vec<ProcCoachSavedReview>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayFileEntry {
    pub file_name: String,
    pub path: String,
    pub title: String,
    pub saved_at: String,
    pub active_character: String,
    pub capture_mode: String,
    pub line_count: usize,
    pub coach_review_count: usize,
    #[serde(flatten)]
    pub summary: ReplayFightSummary,
}

#[derive(Debug)]
struct EncounterSource {
    id: i64,
    character: String,
    mob_name: String,
    started_at: String,
    last_damage_at: String,
    total_damage: u64,
    event_count: usize,
    participant_count: usize,
    proc_count: u64,
    dot_damage: u64,
    source_file: String,
    first_offset: i64,
    last_offset: i64,
}

pub fn replay_directory() -> Result<PathBuf, String> {
    let directory = paths::data_directory()
        .map_err(|error| error.to_string())?
        .join("combat-replays");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    Ok(directory)
}

pub fn list() -> Result<Vec<ReplayFileEntry>, String> {
    let directory = replay_directory()?;
    let mut entries = Vec::new();
    for item in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = item.map_err(|error| error.to_string())?.path();
        if !is_replay_path(&path) {
            continue;
        }
        if let Ok(document) = read_document(&path) {
            entries.push(to_entry(&path, &document));
        }
    }
    entries.sort_by(|left, right| right.saved_at.cmp(&left.saved_at));
    Ok(entries)
}

pub fn load(path: &str) -> Result<ReplayFile, String> {
    read_document(Path::new(path))
}

pub fn append_coach_review(path: &str, review: ProcCoachSavedReview) -> Result<ReplayFile, String> {
    validate_saved_review(&review)?;
    let path = Path::new(path);
    let mut document = read_document(path)?;
    document.coach_reviews.push(review);
    write_document(path, &document)?;
    Ok(document)
}

pub fn import(path: &str) -> Result<ReplayFileEntry, String> {
    let source = Path::new(path);
    let document = read_document(source)?;
    let destination =
        unique_destination(&document, source.file_name().and_then(|name| name.to_str()))?;
    write_document(&destination, &document)?;
    Ok(to_entry(&destination, &document))
}

pub fn save_encounter(database: &Database, encounter_id: i64) -> Result<ReplayFileEntry, String> {
    let source = encounter_source(database, encounter_id)?;
    let (log_lines, capture_mode) = capture_lines(database, &source)?;
    if log_lines.is_empty() {
        return Err("No retained log lines were available for this encounter".into());
    }
    let document = ReplayFile {
        format_version: FORMAT_VERSION,
        title: format!("{} vs {}", source.character, source.mob_name),
        saved_at: Utc::now().to_rfc3339(),
        active_character: source.character.clone(),
        project_all_ticks: true,
        summary: ReplayFightSummary {
            mob_name: source.mob_name,
            started_at: source.started_at.clone(),
            duration_seconds: duration_seconds(&source.started_at, &source.last_damage_at),
            total_damage: source.total_damage,
            participant_count: source.participant_count,
            event_count: source.event_count,
            proc_count: source.proc_count,
            dot_damage: source.dot_damage,
        },
        source: ReplaySource {
            encounter_id,
            source_file: source.source_file,
            first_source_offset: source.first_offset,
            last_source_offset: source.last_offset,
            capture_mode,
        },
        log_lines,
        notes: String::new(),
        coach_reviews: Vec::new(),
    };
    let destination = unique_destination(&document, None)?;
    write_document(&destination, &document)?;
    Ok(to_entry(&destination, &document))
}

fn encounter_source(database: &Database, encounter_id: i64) -> Result<EncounterSource, String> {
    database
        .connect()
        .map_err(|error| error.to_string())?
        .query_row(
            "SELECT e.id,e.character_name,e.mob_name,e.started_at,e.last_damage_at,e.total_damage,e.hit_count,
                    COALESCE(NULLIF((SELECT COUNT(*) FROM damage_participant_summaries WHERE encounter_id=e.id),0),
                             (SELECT COUNT(DISTINCT COALESCE(NULLIF(attacker_name,''),e.character_name)) FROM damage_events WHERE encounter_id=e.id),0),
                    COALESCE((SELECT SUM(proc_count) FROM damage_spell_summaries WHERE encounter_id=e.id),0),
                    COALESCE((SELECT SUM(dot_damage) FROM damage_spell_summaries WHERE encounter_id=e.id),0),e.source_file,
                    e.first_source_offset,e.last_source_offset
             FROM damage_encounters e WHERE e.id=?",
            [encounter_id],
            |row| {
                Ok(EncounterSource {
                    id: row.get(0)?,
                    character: row.get(1)?,
                    mob_name: row.get(2)?,
                    started_at: row.get(3)?,
                    last_damage_at: row.get(4)?,
                    total_damage: row.get::<_, i64>(5)?.max(0) as u64,
                    event_count: row.get::<_, i64>(6)?.max(0) as usize,
                    participant_count: row.get::<_, i64>(7)?.max(0) as usize,
                    proc_count: row.get::<_, i64>(8)?.max(0) as u64,
                    dot_damage: row.get::<_, i64>(9)?.max(0) as u64,
                    source_file: row.get(10)?,
                    first_offset: row.get(11)?,
                    last_offset: row.get(12)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Damage encounter was not found".into())
}

fn capture_lines(
    database: &Database,
    source: &EncounterSource,
) -> Result<(Vec<String>, String), String> {
    let path = Path::new(&source.source_file);
    if path.is_file() {
        if let Ok(lines) = capture_source_window(path, source.first_offset, source.last_offset) {
            if !lines.is_empty() {
                return Ok((lines, "source-window".into()));
            }
        }
    }
    Ok((
        retained_event_lines(database, source.id)?,
        "retained-events".into(),
    ))
}

fn capture_source_window(
    path: &Path,
    first_offset: i64,
    last_offset: i64,
) -> Result<Vec<String>, String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let seek_offset = first_offset.saturating_sub(256 * 1024).max(0) as u64;
    file.seek(SeekFrom::Start(seek_offset))
        .map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(file);
    let mut before = VecDeque::with_capacity(CONTEXT_LINES);
    let mut selected = Vec::new();
    let mut offset = seek_offset as i64;
    let mut after = 0_usize;
    if seek_offset > 0 {
        let mut partial = Vec::new();
        let read = reader
            .read_until(b'\n', &mut partial)
            .map_err(|error| error.to_string())?;
        offset += read as i64;
    }
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
        if line_offset < first_offset {
            if before.len() == CONTEXT_LINES {
                before.pop_front();
            }
            before.push_back(line);
            continue;
        }
        if selected.is_empty() {
            selected.extend(before.drain(..));
        }
        selected.push(line);
        if line_offset >= last_offset {
            after += 1;
            if after >= CONTEXT_LINES {
                break;
            }
        }
    }
    Ok(selected)
}

fn retained_event_lines(database: &Database, encounter_id: i64) -> Result<Vec<String>, String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "SELECT raw_line,source_offset FROM damage_events WHERE encounter_id=?
             UNION ALL
             SELECT raw_line,source_offset FROM damage_received_events WHERE encounter_id=?
             ORDER BY source_offset",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![encounter_id, encounter_id], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn read_document(path: &Path) -> Result<ReplayFile, String> {
    let bytes = fs::read(path).map_err(|error| format!("Could not read replay file: {error}"))?;
    let document: ReplayFile =
        serde_json::from_slice(&bytes).map_err(|error| format!("Invalid replay file: {error}"))?;
    if document.format_version != FORMAT_VERSION {
        return Err(format!(
            "Unsupported replay format version {}",
            document.format_version
        ));
    }
    if document.active_character.trim().is_empty() || document.log_lines.is_empty() {
        return Err("Replay file is missing its character or log lines".into());
    }
    Ok(document)
}

fn write_document(path: &Path, document: &ReplayFile) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("Replay destination has no parent folder")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("eqfight.json.tmp");
    let mut file = File::create(&temporary).map_err(|error| error.to_string())?;
    serde_json::to_writer_pretty(&mut file, document).map_err(|error| error.to_string())?;
    file.flush().map_err(|error| error.to_string())?;
    drop(file);
    if !path.exists() {
        return fs::rename(&temporary, path).map_err(|error| error.to_string());
    }
    let backup = path.with_extension(format!(
        "eqfight.json.backup-{}",
        Utc::now().timestamp_millis()
    ));
    fs::rename(path, &backup).map_err(|error| error.to_string())?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::rename(&backup, path);
        return Err(error.to_string());
    }
    let _ = fs::remove_file(backup);
    Ok(())
}

fn unique_destination(
    document: &ReplayFile,
    preferred_name: Option<&str>,
) -> Result<PathBuf, String> {
    let directory = replay_directory()?;
    let base = preferred_name
        .and_then(|name| name.strip_suffix(".eqfight.json"))
        .map(sanitize)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| {
            sanitize(&format!(
                "{}-{}",
                document.summary.started_at, document.title
            ))
        });
    for suffix in 0..10_000 {
        let file_name = if suffix == 0 {
            format!("{base}.eqfight.json")
        } else {
            format!("{base}-{suffix}.eqfight.json")
        };
        let candidate = directory.join(file_name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("Could not allocate a unique replay file name".into())
}

fn sanitize(value: &str) -> String {
    let mut result = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            result.push(character.to_ascii_lowercase());
        } else if !result.ends_with('-') {
            result.push('-');
        }
    }
    result.trim_matches('-').to_string()
}

fn is_replay_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(".eqfight.json"))
}

fn to_entry(path: &Path, document: &ReplayFile) -> ReplayFileEntry {
    ReplayFileEntry {
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .into(),
        path: path.display().to_string(),
        title: document.title.clone(),
        saved_at: document.saved_at.clone(),
        active_character: document.active_character.clone(),
        capture_mode: document.source.capture_mode.clone(),
        line_count: document.log_lines.len(),
        coach_review_count: document.coach_reviews.len(),
        summary: document.summary.clone(),
    }
}

fn duration_seconds(started_at: &str, last_damage_at: &str) -> u64 {
    let format = "%Y-%m-%d %H:%M:%S";
    match (
        NaiveDateTime::parse_from_str(started_at, format),
        NaiveDateTime::parse_from_str(last_damage_at, format),
    ) {
        (Ok(start), Ok(end)) => (end - start).num_seconds().max(0) as u64,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        append_coach_review, capture_source_window, sanitize, write_document, ReplayFightSummary,
        ReplayFile, ReplaySource,
    };
    use crate::application::proc_coach::ProcCoachSavedReview;
    use std::{fs, io::Write};

    #[test]
    fn source_window_keeps_leading_and_trailing_proc_context() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fight.log");
        let mut file = fs::File::create(&path).unwrap();
        let mut offsets = Vec::new();
        let mut offset = 0_i64;
        for index in 0..40 {
            offsets.push(offset);
            let line = format!("line {index}\n");
            file.write_all(line.as_bytes()).unwrap();
            offset += line.len() as i64;
        }
        drop(file);
        let lines = capture_source_window(&path, offsets[15], offsets[20]).unwrap();
        assert_eq!(lines.first().unwrap(), "line 3");
        assert_eq!(lines.last().unwrap(), "line 31");
    }

    #[test]
    fn coach_review_is_appended_to_an_existing_replay() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fight.eqfight.json");
        let replay = ReplayFile {
            format_version: 1,
            title: "Tester vs a mob".into(),
            saved_at: "2026-09-08T00:00:00Z".into(),
            active_character: "Tester".into(),
            project_all_ticks: true,
            summary: ReplayFightSummary {
                mob_name: "a mob".into(),
                started_at: "2026-09-08 00:00:00".into(),
                duration_seconds: 1,
                total_damage: 10,
                participant_count: 1,
                event_count: 1,
                proc_count: 0,
                dot_damage: 0,
            },
            source: ReplaySource {
                encounter_id: 1,
                source_file: "eqlog_Tester_P1999Green.txt".into(),
                first_source_offset: 0,
                last_source_offset: 1,
                capture_mode: "source-window".into(),
            },
            log_lines: vec![
                "[Tue Sep 08 00:00:00 2026] You hit a mob for 10 points of damage.".into(),
            ],
            notes: String::new(),
            coach_reviews: Vec::new(),
        };
        write_document(&path, &replay).unwrap();
        let updated = append_coach_review(
            path.to_str().unwrap(),
            ProcCoachSavedReview {
                reviewed_at: "2026-09-08T00:01:00Z".into(),
                model: "test-model".into(),
                agent_summary: "No proc.".into(),
                findings: Vec::new(),
                decisions: Vec::new(),
            },
        )
        .unwrap();
        assert_eq!(updated.coach_reviews.len(), 1);
        assert_eq!(
            super::load(path.to_str().unwrap())
                .unwrap()
                .coach_reviews
                .len(),
            1
        );
    }
    #[test]
    fn file_names_are_portable() {
        assert_eq!(
            sanitize("2026-09-07: Me vs. A Mob!"),
            "2026-09-07-me-vs-a-mob"
        );
    }
}
