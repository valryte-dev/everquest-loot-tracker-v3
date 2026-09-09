use crate::domain::log_events::{parse_envelope, parse_log_event, LogEvent};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChTrainingReport {
    pub active_character: String,
    pub line_count: usize,
    pub recognized_count: usize,
    pub ignored_count: usize,
    pub calls: Vec<ChTrainingCall>,
    pub lines: Vec<ChTrainingLine>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChTrainingCall {
    pub id: i64,
    pub happened_at: String,
    pub character: String,
    pub cleric_name: String,
    pub call_number: u32,
    pub target_name: Option<String>,
    pub channel: String,
    pub message: String,
    pub source_file: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChTrainingLine {
    pub line_number: usize,
    pub happened_at: Option<String>,
    pub raw_line: String,
    pub status: String,
    pub parser_event: String,
    pub summary: String,
}

pub fn analyze(text: &str, active_character: &str) -> ChTrainingReport {
    let character = active_character.trim();
    let mut calls = Vec::new();
    let mut lines = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let raw_line = raw.trim_end_matches('\r').to_string();
        if raw_line.trim().is_empty() {
            continue;
        }
        let happened_at =
            parse_envelope(&raw_line).map(|(time, _)| time.format("%Y-%m-%d %H:%M:%S").to_string());
        match parse_log_event(&raw_line, character) {
            Some(LogEvent::ClericHealCall {
                happened_at,
                cleric_name,
                call_number,
                target_name,
                channel,
                message,
            }) => {
                calls.push(ChTrainingCall {
                    id: (index + 1) as i64,
                    happened_at: happened_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                    character: character.to_string(),
                    cleric_name: cleric_name.clone(),
                    call_number,
                    target_name: target_name.clone(),
                    channel: channel.as_str().into(),
                    message,
                    source_file: "CH Lab".into(),
                });
                lines.push(ChTrainingLine {
                    line_number: index + 1,
                    happened_at: Some(happened_at.format("%Y-%m-%d %H:%M:%S").to_string()),
                    raw_line,
                    status: "recognized".into(),
                    parser_event: "Complete Heal call".into(),
                    summary: format!(
                        "{} called #{:03}{}",
                        cleric_name,
                        call_number,
                        target_name
                            .map(|target| format!(" on {target}"))
                            .unwrap_or_default()
                    ),
                });
            }
            Some(_) => lines.push(ChTrainingLine {
                line_number: index + 1,
                happened_at,
                raw_line,
                status: "other".into(),
                parser_event: "Other log event".into(),
                summary:
                    "The production parser recognized this line, but it is not a guild CH call."
                        .into(),
            }),
            None => lines.push(ChTrainingLine {
                line_number: index + 1,
                happened_at,
                raw_line,
                status: "ignored".into(),
                parser_event: "Ignored".into(),
                summary: "No production CH event matched this line.".into(),
            }),
        }
    }
    ChTrainingReport {
        active_character: character.to_string(),
        line_count: lines.len(),
        recognized_count: calls.len(),
        ignored_count: lines.len().saturating_sub(calls.len()),
        calls,
        lines,
    }
}

#[cfg(test)]
mod tests {
    use super::analyze;

    #[test]
    fn uses_production_parser_and_ignores_group_chatter() {
        let report = analyze("[Tue Sep 08 10:00:00 2026] Bakamore tells the guild, 'LoF 001 CH - Forsure'\n[Tue Sep 08 10:00:09 2026] You say to your guild, 'lof 002 ch - Forsure'\n[Tue Sep 08 10:00:10 2026] Clerica tells the group, 'LoF 003 CH - Forsure'", "Tester");
        assert_eq!(
            (
                report.line_count,
                report.recognized_count,
                report.ignored_count
            ),
            (3, 2, 1)
        );
        assert_eq!(report.calls[0].cleric_name, "Bakamore");
        assert_eq!(report.calls[1].cleric_name, "Tester");
        assert_eq!(report.calls[1].call_number, 2);
    }
}
