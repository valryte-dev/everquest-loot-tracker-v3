mod ch_replay_library;
mod ch_training;
mod combat_metrics;
mod damage_analytics;
mod data;
mod database_management;
mod dot_tracking;
mod dot_training;
mod model_pack;
mod proc_coach;
mod proc_evidence;
mod quest_catalog;
mod replay_library;
mod runtime;
mod services;
mod split_reconciliation;
mod system_tasks;
mod wardrobe_catalog;

pub use wardrobe_catalog::{
    wardrobe_catalog_items, wardrobe_catalog_set_items, wardrobe_catalog_sets,
};

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
pub async fn damage_proc_evidence(
    state: tauri::State<'_, AppState>,
    encounter_id: i64,
    player_name: String,
) -> Result<proc_evidence::ProcEvidenceReport, String> {
    let database = state.database.clone();
    tauri::async_runtime::spawn_blocking(move || {
        proc_evidence::load(&database, encounter_id, &player_name)
    })
    .await
    .map_err(|error| error.to_string())?
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
pub async fn database_fight_purge_preview(
    state: tauri::State<'_, AppState>,
    keep_days: u32,
) -> Result<database_management::FightPurgePreview, String> {
    let database = state.database.clone();
    tauri::async_runtime::spawn_blocking(move || {
        database_management::preview_fight_purge(&database, keep_days)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn database_protected_fights(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<database_management::ProtectedFight>, String> {
    let database = state.database.clone();
    tauri::async_runtime::spawn_blocking(move || database_management::protected_fights(&database))
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
pub async fn model_pack_status(
    state: tauri::State<'_, AppState>,
) -> Result<model_pack::ModelPackStatus, String> {
    let database = state.database.clone();
    tauri::async_runtime::spawn_blocking(move || model_pack::status(&database))
        .await
        .map_err(|error| error.to_string())?
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
        "modelPack.download" => {
            let database = state.database.clone();
            let tasks = state.tasks.clone();
            let handle = app_handle.clone();
            tasks.start(
                "model-pack-download",
                "Downloading character models",
                "Preparing model pack storage",
                None,
            );
            let _ = handle.emit("system-task-changed", "model-pack-download");
            let work_tasks = tasks.clone();
            let work_handle = handle.clone();
            let operation = tauri::async_runtime::spawn_blocking(move || {
                model_pack::download_base_pack(&database, |completed, total, detail| {
                    work_tasks.progress(
                        "model-pack-download",
                        detail,
                        Some(completed),
                        Some(total),
                    );
                    let _ = work_handle.emit("system-task-changed", "model-pack-download");
                })
            })
            .await
            .map_err(|error| error.to_string())?;
            let result = operation
                .and_then(|value| serde_json::to_value(value).map_err(|error| error.to_string()));
            tasks.finish("model-pack-download", &result, "Character model pack ready");
            let _ = handle.emit("system-task-changed", "model-pack-download");
            result
        }
        "modelPack.activate" => {
            let path = request
                .payload
                .get("path")
                .and_then(Value::as_str)
                .ok_or("path is required")?
                .to_owned();
            let database = state.database.clone();
            tauri::async_runtime::spawn_blocking(move || {
                model_pack::activate_directory(&database, &path)
            })
            .await
            .map_err(|error| error.to_string())?
            .and_then(|value| serde_json::to_value(value).map_err(|error| error.to_string()))
        }
        "modelPack.verify" => {
            let database = state.database.clone();
            tauri::async_runtime::spawn_blocking(move || model_pack::verify(&database))
                .await
                .map_err(|error| error.to_string())?
                .and_then(|value| serde_json::to_value(value).map_err(|error| error.to_string()))
        }
        "modelPack.disconnect" => model_pack::disconnect(&state.database)
            .and_then(|value| serde_json::to_value(value).map_err(|error| error.to_string())),
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
        "database.fightProtection" => database_management::set_fight_protection(
            &state.database,
            request
                .payload
                .get("id")
                .and_then(Value::as_i64)
                .ok_or("id is required")?,
            request
                .payload
                .get("protected")
                .and_then(Value::as_bool)
                .ok_or("protected is required")?,
        ),
        "database.combatPurge" => {
            let keep_days = request
                .payload
                .get("keepDays")
                .and_then(Value::as_u64)
                .ok_or("keepDays is required")? as u32;
            let database = state.database.clone();
            let purge_tasks = state.tasks.clone();
            let purge_handle = app_handle.clone();
            state.tasks.start(
                "combat-purge",
                "Backing up database",
                "Creating a safety backup before combat purge",
                None,
            );
            let _ = app_handle.emit("system-task-changed", "combat-purge");
            let operation = tauri::async_runtime::spawn_blocking(move || {
                let backup = services::backup_with_progress(&database, |completed, total| {
                    purge_tasks.progress(
                        "combat-purge",
                        "Creating a safety backup",
                        Some(completed),
                        Some(total),
                    );
                    let _ = purge_handle.emit("system-task-changed", "combat-purge");
                })?;
                purge_tasks.transition(
                    "combat-purge",
                    "Purging old combat fights",
                    "Deleting eligible fights in safe batches",
                    Some(0),
                    None,
                );
                let _ = purge_handle.emit("system-task-changed", "combat-purge");
                let result = database_management::purge_fights_with_progress(
                    &database,
                    keep_days,
                    |completed, total| {
                        purge_tasks.progress(
                            "combat-purge",
                            "Deleting eligible fights in safe batches",
                            Some(completed as u64),
                            Some(total as u64),
                        );
                        let _ = purge_handle.emit("system-task-changed", "combat-purge");
                    },
                )?;
                Ok::<_, String>((backup, result))
            })
            .await
            .map_err(|error| error.to_string())?;
            let (backup, result) = match operation {
                Ok(value) => {
                    state.tasks.finish(
                        "combat-purge",
                        &Ok(serde_json::json!({"completed":true})),
                        "Combat purge complete",
                    );
                    value
                }
                Err(error) => {
                    state.tasks.finish(
                        "combat-purge",
                        &Err::<Value, _>(error.clone()),
                        "Combat purge failed",
                    );
                    let _ = app_handle.emit("system-task-changed", "combat-purge");
                    return Err(error);
                }
            };
            let _ = app_handle.emit("system-task-changed", "combat-purge");
            let backup_path = backup.get("path").cloned().unwrap_or(Value::Null);
            let mut value = serde_json::to_value(&result).map_err(|error| error.to_string())?;
            value["backupPath"] = backup_path;
            Ok(value)
        }
        "database.deleteProtectedFights" => {
            let ids = request
                .payload
                .get("ids")
                .and_then(Value::as_array)
                .ok_or("ids are required")?
                .iter()
                .map(|value| value.as_i64().ok_or("Every fight id must be a number"))
                .collect::<Result<Vec<_>, _>>()?;
            let database = state.database.clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                database_management::delete_protected_fights(&database, &ids)
            })
            .await
            .map_err(|error| error.to_string())??;
            serde_json::to_value(result).map_err(|error| error.to_string())
        }
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
