#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use pmusim_app::{commands, state::AppState, update};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::start_server,
            commands::stop_server,
            commands::connect_substation,
            commands::disconnect_substation,
            commands::send_command,
            commands::auto_handshake,
            commands::skip_cfg2_open,
            commands::set_heartbeat_interval,
            commands::poll_events,
            commands::open_url,
            update::check_for_update,
            update::install_update,
            update::snooze_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PmuSim");
}
