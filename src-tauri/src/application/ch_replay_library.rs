use crate::infrastructure::paths;
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClericHealReplayCall {
    pub id: i64,
    pub happened_at: String,
    pub character: String,
    pub cleric_name: String,
    pub call_number: i64,
    pub target_name: Option<String>,
    pub channel: String,
    pub message: String,
    pub source_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleric_gap_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClericHealReplaySummary {
    pub started_at: String,
    pub ended_at: String,
    pub duration_seconds: u64,
    pub call_count: usize,
    pub healer_count: usize,
    pub average_gap_seconds: f64,
    pub longest_gap_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClericHealReplayFile {
    pub format_version: u32,
    pub title: String,
    pub saved_at: String,
    pub character: String,
    pub target_mob: String,
    pub summary: ClericHealReplaySummary,
    pub calls: Vec<ClericHealReplayCall>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveClericHealReplayRequest {
    pub character: String,
    pub target_mob: String,
    pub calls: Vec<ClericHealReplayCall>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClericHealReplayEntry {
    pub file_name: String,
    pub path: String,
    pub title: String,
    pub saved_at: String,
    pub character: String,
    pub target_mob: String,
    #[serde(flatten)]
    pub summary: ClericHealReplaySummary,
}

pub fn save(request: SaveClericHealReplayRequest) -> Result<ClericHealReplayEntry, String> {
    validate_calls(&request.calls)?;
    let summary = summarize(&request.calls)?;
    let character = request.character.trim().to_string();
    if character.is_empty() {
        return Err("CH replay character is required".into());
    }
    let target_mob = request.target_mob.trim().to_string();
    let document = ClericHealReplayFile {
        format_version: FORMAT_VERSION,
        title: format!(
            "{} - {}",
            if target_mob.is_empty() {
                "Unknown tank"
            } else {
                &target_mob
            },
            summary.started_at
        ),
        saved_at: Utc::now().to_rfc3339(),
        character,
        target_mob: if target_mob.is_empty() {
            "Unknown tank".into()
        } else {
            target_mob
        },
        summary,
        calls: request.calls,
    };
    let path = unique_destination(&document)?;
    write_document(&path, &document)?;
    Ok(to_entry(&path, &document))
}

pub fn list() -> Result<Vec<ClericHealReplayEntry>, String> {
    let mut entries = Vec::new();
    for item in fs::read_dir(replay_directory()?).map_err(|error| error.to_string())? {
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

pub fn load(path: &str) -> Result<ClericHealReplayFile, String> {
    read_document(Path::new(path))
}

fn replay_directory() -> Result<PathBuf, String> {
    let directory = paths::data_directory()
        .map_err(|error| error.to_string())?
        .join("ch-chain-replays");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    Ok(directory)
}

fn validate_calls(calls: &[ClericHealReplayCall]) -> Result<(), String> {
    if calls.is_empty() {
        return Err("A CH replay needs at least one call".into());
    }
    if calls.len() > 10_000 {
        return Err("A CH replay cannot exceed 10,000 calls".into());
    }
    if calls
        .iter()
        .any(|call| call.cleric_name.trim().is_empty() || call.happened_at.trim().is_empty())
    {
        return Err("A CH replay call is missing its healer or timestamp".into());
    }
    Ok(())
}

fn summarize(calls: &[ClericHealReplayCall]) -> Result<ClericHealReplaySummary, String> {
    let mut ordered = calls.to_vec();
    ordered.sort_by(|left, right| {
        left.happened_at
            .cmp(&right.happened_at)
            .then(left.id.cmp(&right.id))
    });
    let first = ordered.first().ok_or("CH replay has no calls")?;
    let last = ordered.last().ok_or("CH replay has no calls")?;
    let gaps: Vec<f64> = ordered.iter().filter_map(|call| call.gap_seconds).collect();
    let healer_count = ordered
        .iter()
        .map(|call| call.cleric_name.to_lowercase())
        .collect::<HashSet<_>>()
        .len();
    Ok(ClericHealReplaySummary {
        started_at: first.happened_at.clone(),
        ended_at: last.happened_at.clone(),
        duration_seconds: duration_seconds(&first.happened_at, &last.happened_at),
        call_count: ordered.len(),
        healer_count,
        average_gap_seconds: if gaps.is_empty() {
            0.0
        } else {
            gaps.iter().sum::<f64>() / gaps.len() as f64
        },
        longest_gap_seconds: gaps.into_iter().fold(0.0, f64::max),
    })
}

fn duration_seconds(started_at: &str, ended_at: &str) -> u64 {
    let format = "%Y-%m-%d %H:%M:%S";
    match (
        NaiveDateTime::parse_from_str(started_at, format),
        NaiveDateTime::parse_from_str(ended_at, format),
    ) {
        (Ok(start), Ok(end)) => (end - start).num_seconds().max(0) as u64,
        _ => 0,
    }
}

fn read_document(path: &Path) -> Result<ClericHealReplayFile, String> {
    if !is_replay_path(path) {
        return Err("CH replay must use the .eqch.json extension".into());
    }
    let bytes = fs::read(path).map_err(|error| format!("Could not read CH replay: {error}"))?;
    let document: ClericHealReplayFile =
        serde_json::from_slice(&bytes).map_err(|error| format!("Invalid CH replay: {error}"))?;
    if document.format_version != FORMAT_VERSION {
        return Err(format!(
            "Unsupported CH replay format version {}",
            document.format_version
        ));
    }
    validate_calls(&document.calls)?;
    Ok(document)
}

fn write_document(path: &Path, document: &ClericHealReplayFile) -> Result<(), String> {
    let temporary = path.with_extension("eqch.json.tmp");
    let mut file = File::create(&temporary).map_err(|error| error.to_string())?;
    serde_json::to_writer_pretty(&mut file, document).map_err(|error| error.to_string())?;
    file.flush().map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

fn unique_destination(document: &ClericHealReplayFile) -> Result<PathBuf, String> {
    let base = sanitize(&format!(
        "{}-{}",
        document.summary.started_at, document.target_mob
    ));
    let directory = replay_directory()?;
    for suffix in 0..10_000 {
        let name = if suffix == 0 {
            format!("{base}.eqch.json")
        } else {
            format!("{base}-{suffix}.eqch.json")
        };
        let path = directory.join(name);
        if !path.exists() {
            return Ok(path);
        }
    }
    Err("Could not allocate a unique CH replay file name".into())
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
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(".eqch.json"))
}

fn to_entry(path: &Path, document: &ClericHealReplayFile) -> ClericHealReplayEntry {
    ClericHealReplayEntry {
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .into(),
        path: path.display().to_string(),
        title: document.title.clone(),
        saved_at: document.saved_at.clone(),
        character: document.character.clone(),
        target_mob: document.target_mob.clone(),
        summary: document.summary.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{sanitize, summarize, ClericHealReplayCall};

    fn call(id: i64, at: &str, healer: &str, gap: Option<f64>) -> ClericHealReplayCall {
        ClericHealReplayCall {
            id,
            happened_at: at.into(),
            character: "Tester".into(),
            cleric_name: healer.into(),
            call_number: id,
            target_name: None,
            channel: "guild".into(),
            message: String::new(),
            source_file: "eqlog_Tester.txt".into(),
            gap_seconds: gap,
            cleric_gap_seconds: None,
        }
    }

    #[test]
    fn summary_preserves_chain_metrics() {
        let summary = summarize(&[
            call(1, "2026-09-08 10:00:00", "A", None),
            call(2, "2026-09-08 10:00:09", "B", Some(9.0)),
            call(3, "2026-09-08 10:00:19", "A", Some(10.0)),
        ])
        .unwrap();
        assert_eq!(
            (
                summary.call_count,
                summary.healer_count,
                summary.duration_seconds
            ),
            (3, 2, 19)
        );
        assert!((summary.average_gap_seconds - 9.5).abs() < f64::EPSILON);
    }

    #[test]
    fn names_are_portable() {
        assert_eq!(
            sanitize("2026-09-08 10:00:00 - A Dragon!"),
            "2026-09-08-10-00-00-a-dragon"
        );
    }
}
