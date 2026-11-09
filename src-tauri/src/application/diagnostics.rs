use crate::infrastructure::{database::Database, paths};
use chrono::Utc;
use rusqlite::Result as SqlResult;
use serde::Serialize;
use std::{fs, path::Path};

const MAX_FILE_TAIL_BYTES: usize = 128 * 1024;
const MAX_DISCORD_CHARS: usize = 1_850;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsReport {
    pub generated_at: String,
    pub compact_text: String,
    pub full_text: String,
    pub log_directory: String,
}

pub fn report(database: &Database) -> Result<DiagnosticsReport, String> {
    let generated_at = Utc::now().to_rfc3339();
    let directory = paths::diagnostics_directory().map_err(|error| error.to_string())?;
    let mut sections = Vec::new();
    sections.push(format!(
        "EverQuest Loot Tracker diagnostics\nVersion: {}\nPlatform: {}\nGenerated: {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        generated_at
    ));
    let database_lines = recent_database_logs(database).unwrap_or_else(|error| {
        vec![format!(
            "WARN [diagnostics] Structured application logs were unavailable: {error}"
        )]
    });
    sections.push(format!(
        "Recent application messages\n{}",
        if database_lines.is_empty() {
            "(none)".into()
        } else {
            database_lines.join("\n")
        }
    ));
    let file_lines = recent_file_logs(&directory);
    sections.push(format!(
        "Runtime and crash logs\n{}",
        if file_lines.is_empty() {
            "(none)".into()
        } else {
            file_lines.join("\n")
        }
    ));
    let full_text = sections.join("\n\n");
    let compact_text = compact_report(&generated_at, &database_lines, &file_lines);
    Ok(DiagnosticsReport {
        generated_at,
        compact_text,
        full_text,
        log_directory: directory.to_string_lossy().into_owned(),
    })
}

pub fn record_client(kind: &str, message: &str, stack: Option<&str>) {
    let kind = single_line(kind, 80);
    let message = single_line(message, 2_000);
    let stack = stack
        .map(|value| truncate(value, 8_000))
        .unwrap_or_default();
    tracing::error!(target: "frontend", kind = %kind, message = %message, stack = %stack, "frontend diagnostic");
}

fn recent_database_logs(database: &Database) -> Result<Vec<String>, String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    let mut statement = connection
        .prepare(
            "SELECT happened_at,level,area,message FROM application_logs ORDER BY id DESC LIMIT 300",
        )
        .map_err(|error| error.to_string())?;
    let rows: SqlResult<Vec<String>> = statement
        .query_map([], |row| {
            Ok(format!(
                "{} {} [{}] {}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?.to_uppercase(),
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?
            ))
        })
        .map_err(|error| error.to_string())?
        .collect();
    let mut rows = rows.map_err(|error| error.to_string())?;
    rows.reverse();
    Ok(rows)
}

fn recent_file_logs(directory: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            name == "crash.log" || name.starts_with("loot-tracker.log")
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|entry| entry.metadata().and_then(|value| value.modified()).ok());
    files
        .into_iter()
        .rev()
        .take(4)
        .flat_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let content = fs::read(entry.path()).unwrap_or_default();
            let start = content.len().saturating_sub(MAX_FILE_TAIL_BYTES);
            let text = String::from_utf8_lossy(&content[start..]);
            std::iter::once(format!("--- {name} ---"))
                .chain(text.lines().map(str::to_owned).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn compact_report(generated_at: &str, database_lines: &[String], file_lines: &[String]) -> String {
    let mut candidates = database_lines
        .iter()
        .chain(file_lines.iter())
        .filter(|line| {
            let value = line.to_ascii_lowercase();
            value.contains("error") || value.contains("warn") || value.contains("panic")
        })
        .cloned()
        .collect::<Vec<_>>();
    candidates.extend(database_lines.iter().rev().take(8).rev().cloned());
    candidates.extend(file_lines.iter().rev().take(12).rev().cloned());
    candidates.dedup();
    let header = format!(
        "**EverQuest Loot Tracker diagnostic**\nVersion {} | {} | {}\n```text\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        generated_at
    );
    let suffix = "\n```";
    let available = MAX_DISCORD_CHARS.saturating_sub(header.chars().count() + suffix.len());
    let body = truncate(&candidates.join("\n"), available);
    format!("{header}{body}{suffix}")
}

fn single_line(value: &str, max: usize) -> String {
    truncate(&value.replace(['\r', '\n'], " "), max)
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_owned();
    }
    let keep = max.saturating_sub(1);
    format!("{}…", value.chars().take(keep).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::{compact_report, MAX_DISCORD_CHARS};

    #[test]
    fn compact_report_fits_in_one_discord_message_and_keeps_errors() {
        let database = vec!["2026 ERROR [watcher] database locked".into()];
        let files = vec!["normal startup".repeat(500)];
        let result = compact_report("2026-11-09T12:00:00Z", &database, &files);
        assert!(result.chars().count() <= MAX_DISCORD_CHARS);
        assert!(result.contains("database locked"));
        assert!(result.ends_with("```"));
    }
}
