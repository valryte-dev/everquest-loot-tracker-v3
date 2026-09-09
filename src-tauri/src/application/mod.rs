mod ch_replay_library;
mod ch_training;
mod combat_metrics;
mod data;
mod database_management;
mod dot_tracking;
mod dot_training;
mod proc_coach;
mod replay_library;
mod runtime;
mod services;
mod split_reconciliation;
mod system_tasks;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tauri::Emitter;

use crate::domain::{
    inventory::{parse_inventory, InventoryItem},
    log_events::{parse_log_event, LogEvent},
};
use crate::infrastructure::{
    database::Database,
    paths,
    spell_catalog::{SpellCatalog, SpellCatalogStatus, SpellInfo},
};

pub struct AppState {
    database: Database,
    database_path: PathBuf,
    schema_version: i64,
    legacy_database: bool,
    spell_catalog: SpellCatalog,
    revision: Arc<AtomicU64>,
    tasks: system_tasks::TaskRegistry,
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
        data::sync_normalized_models(&database)?;
        let spell_catalog =
            SpellCatalog::open(paths::spell_database_path().map_err(|error| error.to_string())?)?;
        clear_current_group(&database)?;
        Ok(Self {
            database,
            database_path,
            schema_version,
            legacy_database,
            spell_catalog,
            revision: Arc::new(AtomicU64::new(1)),
            tasks: system_tasks::TaskRegistry::default(),
        })
    }

    pub fn start_runtime(&self, app_handle: tauri::AppHandle) {
        self.spell_catalog.start_if_needed();
        runtime::start(
            self.database.clone(),
            app_handle.clone(),
            self.revision.clone(),
            self.tasks.clone(),
            self.spell_catalog.clone(),
        );
        services::start_update_check(
            self.database_path.clone(),
            app_handle.clone(),
            self.revision.clone(),
        );
        services::start_market_refresh(
            self.database_path.clone(),
            app_handle,
            self.revision.clone(),
        );
        services::start_web(self.database_path.clone());
    }
}

fn clear_current_group(database: &Database) -> Result<(), String> {
    database
        .connect()
        .map_err(|error| error.to_string())?
        .execute_batch("DELETE FROM current_group; DELETE FROM app_settings WHERE key IN ('damage_target_character','damage_target_encounter_id');")
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
pub fn app_page_snapshot(state: tauri::State<'_, AppState>, page: String) -> Result<Value, String> {
    data::page_snapshot(&state.database, &page)
}

#[tauri::command]
pub fn activity_history_snapshot(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    data::activity_history_snapshot(&state.database)
}

#[tauri::command]
pub fn death_report_details(state: tauri::State<'_, AppState>, id: i64) -> Result<Value, String> {
    data::death_report_details(&state.database, id)
}

#[tauri::command]
pub async fn ch_training_preview(
    text: String,
    active_character: String,
) -> Result<ch_training::ChTrainingReport, String> {
    tauri::async_runtime::spawn_blocking(move || ch_training::analyze(&text, &active_character))
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub async fn ch_replay_library_list(
) -> Result<Vec<ch_replay_library::ClericHealReplayEntry>, String> {
    tauri::async_runtime::spawn_blocking(ch_replay_library::list)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn ch_replay_library_load(
    path: String,
) -> Result<ch_replay_library::ClericHealReplayFile, String> {
    tauri::async_runtime::spawn_blocking(move || ch_replay_library::load(&path))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn save_ch_replay(
    request: ch_replay_library::SaveClericHealReplayRequest,
) -> Result<ch_replay_library::ClericHealReplayEntry, String> {
    tauri::async_runtime::spawn_blocking(move || ch_replay_library::save(request))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
pub async fn replay_library_list() -> Result<Vec<replay_library::ReplayFileEntry>, String> {
    tauri::async_runtime::spawn_blocking(replay_library::list)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn replay_library_load(path: String) -> Result<replay_library::ReplayFile, String> {
    tauri::async_runtime::spawn_blocking(move || replay_library::load(&path))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn replay_library_import(
    path: String,
) -> Result<replay_library::ReplayFileEntry, String> {
    tauri::async_runtime::spawn_blocking(move || replay_library::import(&path))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn proc_coach_status() -> Result<proc_coach::ProcCoachStatus, String> {
    tauri::async_runtime::spawn_blocking(proc_coach::credential_status)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn proc_coach_save_key(api_key: String) -> Result<proc_coach::ProcCoachStatus, String> {
    tauri::async_runtime::spawn_blocking(move || proc_coach::save_api_key(&api_key))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn proc_coach_delete_key() -> Result<proc_coach::ProcCoachStatus, String> {
    tauri::async_runtime::spawn_blocking(proc_coach::delete_api_key)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn proc_coach_analyze(
    state: tauri::State<'_, AppState>,
    text: String,
    active_character: String,
    project_all_ticks: bool,
) -> Result<proc_coach::ProcCoachReview, String> {
    let spell_catalog = state.spell_catalog.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let report =
            dot_training::analyze(&spell_catalog, &text, &active_character, project_all_ticks)?;
        let parser_report = serde_json::to_string(&report).map_err(|error| error.to_string())?;
        proc_coach::analyze(&text, &active_character, &parser_report)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn replay_library_save_coach_review(
    path: String,
    review: proc_coach::ProcCoachSavedReview,
) -> Result<replay_library::ReplayFile, String> {
    tauri::async_runtime::spawn_blocking(move || replay_library::append_coach_review(&path, review))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
pub async fn save_damage_replay(
    state: tauri::State<'_, AppState>,
    encounter_id: i64,
) -> Result<replay_library::ReplayFileEntry, String> {
    let database = state.database.clone();
    tauri::async_runtime::spawn_blocking(move || {
        replay_library::save_encounter(&database, encounter_id)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn damage_encounter_details(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Value, String> {
    data::damage_encounter_details(&state.database, id)
}
#[tauri::command]
pub async fn dot_training_preview(
    state: tauri::State<'_, AppState>,
    text: String,
    active_character: String,
    project_all_ticks: bool,
) -> Result<dot_training::DotTrainingReport, String> {
    let spell_catalog = state.spell_catalog.clone();
    tauri::async_runtime::spawn_blocking(move || {
        dot_training::analyze(&spell_catalog, &text, &active_character, project_all_ticks)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn database_stats(
    state: tauri::State<'_, AppState>,
) -> Result<database_management::DatabaseStats, String> {
    let database = state.database.clone();
    tauri::async_runtime::spawn_blocking(move || database_management::stats(&database))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn database_cleanup_preview(
    state: tauri::State<'_, AppState>,
    keep_days: u32,
) -> Result<database_management::CleanupPreview, String> {
    let database = state.database.clone();
    tauri::async_runtime::spawn_blocking(move || {
        database_management::preview_combat_retention(&database, keep_days)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn app_revision(state: tauri::State<'_, AppState>) -> u64 {
    state.revision.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn global_status_snapshot(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let mut status = data::global_combat_snapshot(&state.database)?;
    status["tasks"] =
        serde_json::to_value(state.tasks.snapshot()).map_err(|error| error.to_string())?;
    Ok(status)
}

#[tauri::command]
pub async fn mutate_app(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    request: MutationRequest,
) -> Result<Value, String> {
    let result = match request.action.as_str() {
        "update.check" => services::check_for_update(&state.database),
        "market.refresh" => services::refresh_market(&state.database),
        "activityHistory.scan" => {
            let database = state.database.clone();
            state.tasks.start(
                "history-scan",
                "Scanning log history",
                "Reading configured log files",
                None,
            );
            let _ = app_handle.emit("system-task-changed", "history-scan");
            let result =
                tauri::async_runtime::spawn_blocking(move || runtime::scan_history(&database))
                    .await
                    .map_err(|error| error.to_string())?;
            state
                .tasks
                .finish("history-scan", &result, "Log history scan complete");
            let _ = app_handle.emit("system-task-changed", "history-scan");
            result
        }
        "damageTracker.rescan" => {
            let database = state.database.clone();
            let spell_catalog = state.spell_catalog.clone();
            let progress_handle = app_handle.clone();
            let progress_tasks = state.tasks.clone();
            progress_tasks.start(
                "damage-rescan",
                "Rebuilding damage history",
                "Discovering log files",
                None,
            );
            let _ = app_handle.emit("system-task-changed", "damage-rescan");
            let result = tauri::async_runtime::spawn_blocking(move || {
                runtime::rescan_damage(&database, spell_catalog, |progress| {
                    let detail = if progress.current_file.is_empty() {
                        "Discovering log files"
                    } else {
                        progress.current_file.as_str()
                    };
                    progress_tasks.progress(
                        "damage-rescan",
                        detail,
                        Some(progress.completed as u64),
                        Some(progress.total as u64),
                    );
                    let _ = progress_handle.emit("damage-rescan-progress", &progress);
                    let _ = progress_handle.emit("system-task-changed", "damage-rescan");
                })
            })
            .await
            .map_err(|error| error.to_string())?;
            state
                .tasks
                .finish("damage-rescan", &result, "Damage history rebuilt");
            let _ = app_handle.emit("system-task-changed", "damage-rescan");
            result
        }
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
    };
    if result.is_ok() {
        state.revision.fetch_add(1, Ordering::Relaxed);
        let _ = app_handle.emit("data-changed", request.action);
    }
    result
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

#[tauri::command]
pub fn spell_info(
    state: tauri::State<'_, AppState>,
    spell_name: String,
) -> Result<SpellInfo, String> {
    state.spell_catalog.get(&spell_name)
}

#[tauri::command]
pub fn spell_catalog_entries(state: tauri::State<'_, AppState>) -> Result<Vec<SpellInfo>, String> {
    state.spell_catalog.list()
}

#[tauri::command]
pub fn spell_catalog_status(
    state: tauri::State<'_, AppState>,
) -> Result<SpellCatalogStatus, String> {
    state.spell_catalog.status()
}

#[tauri::command]
pub fn reload_spell_catalog(state: tauri::State<'_, AppState>) -> SpellCatalogStatus {
    state.spell_catalog.start_refresh()
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
        connection.execute_batch("INSERT INTO app_settings(key,value) VALUES('damage_target_character','Youngman'); INSERT INTO app_settings(key,value) VALUES('damage_target_encounter_id','99');").unwrap();
        drop(connection);

        clear_current_group(&database).unwrap();

        let connection = database.connect().unwrap();
        let active: i64 = connection
            .query_row("SELECT COUNT(*) FROM current_group", [], |row| row.get(0))
            .unwrap();
        let remembered: i64 = connection
            .query_row("SELECT COUNT(*) FROM known_members", [], |row| row.get(0))
            .unwrap();
        let target_preferences: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM app_settings WHERE key LIKE 'damage_target_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((active, remembered), (0, 2));
        assert_eq!(target_preferences, 0);
    }
}
