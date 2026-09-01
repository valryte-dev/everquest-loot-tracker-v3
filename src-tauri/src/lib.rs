mod application;
mod domain;
mod infrastructure;

use application::{
    app_snapshot, bootstrap_status, mutate_app, parse_inventory_preview, parse_log_preview,
    reload_spell_catalog, spell_catalog_status, spell_info, AppState,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::initialize().expect("V3 application state must initialize");
    state.start_runtime();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            bootstrap_status,
            app_snapshot,
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
