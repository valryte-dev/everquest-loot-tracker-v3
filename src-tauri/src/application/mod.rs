mod data;
mod runtime;
mod services;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;

use crate::domain::{
    inventory::{parse_inventory, InventoryItem},
    log_events::{parse_log_event, LogEvent},
};
use crate::infrastructure::{database::Database, paths};

pub struct AppState {
    database: Database,
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
        clear_current_group(&database)?;
        Ok(Self {
            database,
            database_path,
            schema_version,
            legacy_database,
        })
    }

    pub fn start_runtime(&self) {
        runtime::start(self.database_path.clone());
        services::start_web(self.database_path.clone());
    }
}

fn clear_current_group(database: &Database) -> Result<(), String> {
    database
        .connect()
        .map_err(|error| error.to_string())?
        .execute("DELETE FROM current_group", [])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationRequest {
    action: String,
    #[serde(default)]
    payload: Value,
}

#[tauri::command]
pub fn app_snapshot(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    data::snapshot(&state.database)
}

#[tauri::command]
pub fn mutate_app(
    state: tauri::State<'_, AppState>,
    request: MutationRequest,
) -> Result<Value, String> {
    match request.action.as_str() {
        "market.refresh" => services::refresh_market(&state.database),
        "planner.upload" => services::upload_exports(&state.database),
        "planner.uploadFiles" => services::upload_file_payloads(&state.database, &request.payload),
        "wts.export" => services::export_wts(
            &state.database,
            request
                .payload
                .get("id")
                .and_then(Value::as_i64)
                .ok_or("id is required")?,
        ),
        "database.backup" => services::backup(&state.database),
        "database.restore" => services::restore(
            &state.database,
            request
                .payload
                .get("path")
                .and_then(Value::as_str)
                .ok_or("path is required")?,
        ),
        _ => data::mutate(&state.database, &request.action, &request.payload),
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

#[cfg(test)]
mod tests {
    use super::clear_current_group;
    use crate::infrastructure::database::Database;

    #[test]
    fn startup_clear_preserves_remembered_characters() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO known_members(name) VALUES('Youngman'),('Posed')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO current_group(member_id) SELECT id FROM known_members",
                [],
            )
            .unwrap();
        drop(connection);

        clear_current_group(&database).unwrap();

        let connection = database.connect().unwrap();
        let active: i64 = connection
            .query_row("SELECT COUNT(*) FROM current_group", [], |row| row.get(0))
            .unwrap();
        let remembered: i64 = connection
            .query_row("SELECT COUNT(*) FROM known_members", [], |row| row.get(0))
            .unwrap();
        assert_eq!((active, remembered), (0, 2));
    }
}
