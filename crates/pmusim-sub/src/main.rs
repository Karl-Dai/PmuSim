#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use pmusim_sub::{commands, state::AppState};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::start_substation,
            commands::stop_substation,
            commands::update_config,
            commands::update_gen,
            commands::fire_trigger,
            commands::poll_events,
            commands::open_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PmuSub");
}
