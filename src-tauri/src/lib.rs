mod application;
mod domain;
mod infrastructure;

use application::{
    activity_history_snapshot, app_page_snapshot, app_revision, app_snapshot, bootstrap_status,
    ch_replay_library_list, ch_replay_library_load, ch_training_preview, damage_encounter_details,
    damage_proc_evidence, database_cleanup_preview, database_fight_purge_preview,
    database_protected_fights, database_stats, death_report_details, dot_training_preview,
    global_status_snapshot, model_pack_status, mutate_app, parse_inventory_preview,
    parse_log_preview, proc_coach_analyze, proc_coach_delete_key, proc_coach_save_key,
    proc_coach_status, reload_spell_catalog, replay_library_import, replay_library_list,
    replay_library_load, replay_library_save_coach_review, save_ch_replay, save_damage_replay,
    spell_catalog_entries, spell_catalog_status, spell_info, wardrobe_catalog_item,
    wardrobe_catalog_items, wardrobe_catalog_set_items, wardrobe_catalog_sets, AppState,
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
            global_status_snapshot,
            model_pack_status,
            app_page_snapshot,
            app_snapshot,
            database_stats,
            database_cleanup_preview,
            database_fight_purge_preview,
            database_protected_fights,
            activity_history_snapshot,
            death_report_details,
            damage_encounter_details,
            damage_proc_evidence,
            dot_training_preview,
            ch_training_preview,
            ch_replay_library_list,
            ch_replay_library_load,
            save_ch_replay,
            replay_library_list,
            replay_library_load,
            replay_library_import,
            replay_library_save_coach_review,
            proc_coach_status,
            proc_coach_save_key,
            proc_coach_delete_key,
            proc_coach_analyze,
            save_damage_replay,
            mutate_app,
            parse_log_preview,
            parse_inventory_preview,
            spell_info,
            spell_catalog_entries,
            spell_catalog_status,
            reload_spell_catalog,
            wardrobe_catalog_item,
            wardrobe_catalog_items,
            wardrobe_catalog_sets,
            wardrobe_catalog_set_items
        ])
        .run(tauri::generate_context!())
        .expect("error while running EverQuest Loot Tracker V3");
}
