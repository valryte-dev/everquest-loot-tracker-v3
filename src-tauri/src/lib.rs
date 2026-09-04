mod application;
mod domain;
mod infrastructure;

use application::{
    activity_history_snapshot, app_page_snapshot, app_revision, app_snapshot, bootstrap_status,
    death_report_details, mutate_app, parse_inventory_preview, parse_log_preview,
    reload_spell_catalog, spell_catalog_status, spell_info, AppState,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::initialize().expect("V3 application state must initialize");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .setup(|app| {
            app.state::<AppState>().start_runtime(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap_status,
            app_revision,
            app_page_snapshot,
            app_snapshot,
            activity_history_snapshot,
            death_report_details,
            mutate_app,
            parse_log_preview,
            parse_inventory_preview,
            spell_info,
            spell_catalog_status,
            reload_spell_catalog
        ])
        .run(tauri::generate_context!())
        .expect("error while running EverQuest Loot Tracker V3");
}
