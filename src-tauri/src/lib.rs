mod application;
mod domain;
mod infrastructure;

use application::{bootstrap_status, parse_inventory_preview, parse_log_preview, AppState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::initialize().expect("V3 application state must initialize");
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            bootstrap_status,
            parse_log_preview,
            parse_inventory_preview
        ])
        .run(tauri::generate_context!())
        .expect("error while running EverQuest Loot Tracker V3");
}
