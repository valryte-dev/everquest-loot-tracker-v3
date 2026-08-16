mod application;
mod domain;
mod infrastructure;

use application::{
    app_snapshot, bootstrap_status, mutate_app, parse_inventory_preview, parse_log_preview,
    AppState,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::initialize().expect("V3 application state must initialize");
    state.start_runtime();
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            bootstrap_status,
            app_snapshot,
            mutate_app,
            parse_log_preview,
            parse_inventory_preview
        ])
        .run(tauri::generate_context!())
        .expect("error while running EverQuest Loot Tracker V3");
}
