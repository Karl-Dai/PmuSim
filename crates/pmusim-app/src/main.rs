#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use pmusim_app::{commands, state::AppState};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::start_server,
            commands::stop_server,
            commands::connect_substation,
            commands::disconnect_substation,
            commands::send_command,
            commands::auto_handshake,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PmuSim");
}
