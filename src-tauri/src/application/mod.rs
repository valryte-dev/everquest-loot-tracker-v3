use serde::Serialize;
use std::path::PathBuf;

use crate::domain::{
    inventory::{parse_inventory, InventoryItem},
    log_events::{parse_log_event, LogEvent},
};
use crate::infrastructure::{database::Database, paths};

pub struct AppState {
    database_path: PathBuf,
    schema_version: i64,
    legacy_database: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapStatus {
    app_version: &'static str,
    platform: &'static str,
    database_path: String,
    database_ready: bool,
    schema_version: i64,
    legacy_database: bool,
}

impl AppState {
    pub fn initialize() -> Result<Self, String> {
        let database_path = paths::database_path().map_err(|error| error.to_string())?;
        let legacy_database = database_path.exists();
        let database = Database::open(&database_path).map_err(|error| error.to_string())?;
        let schema_version = database.migrate().map_err(|error| error.to_string())?;
        Ok(Self {
            database_path,
            schema_version,
            legacy_database,
        })
    }
}

#[tauri::command]
pub fn bootstrap_status(state: tauri::State<'_, AppState>) -> BootstrapStatus {
    BootstrapStatus {
        app_version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
        database_path: state.database_path.display().to_string(),
        database_ready: true,
        schema_version: state.schema_version,
        legacy_database: state.legacy_database,
    }
}

#[tauri::command]
pub fn parse_log_preview(line: String, active_character: String) -> Option<LogEvent> {
    parse_log_event(&line, &active_character)
}

#[tauri::command]
pub fn parse_inventory_preview(text: String) -> Vec<InventoryItem> {
    parse_inventory(&text)
}
