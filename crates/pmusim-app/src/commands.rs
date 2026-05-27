use pmusim_core::protocol::constants::ProtocolVersion;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;

use crate::network::master::MasterStation;
use crate::state::AppState;

#[tauri::command]
pub async fn start_server(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    data_port: u16,
    protocol: String,
) -> Result<(), String> {
    let version = match protocol.as_str() {
        "V2" => ProtocolVersion::V2,
        "V3" => ProtocolVersion::V3,
        other => return Err(format!("Unknown protocol: {other}")),
    };
    let mut guard = state.master.lock().await;
    if guard.is_some() {
        return Err("Server already running".into());
    }
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let forward_handle = app_handle.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if let Err(e) = forward_handle.emit("pmu-event", &event) {
                log::error!("Failed to forward event: {e}");
            }
        }
    });
    let mut master = MasterStation::new(event_tx, data_port, 30.0, version);
    master.start().await?;
    *guard = Some(master);
    Ok(())
}

#[tauri::command]
pub async fn stop_server(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.master.lock().await;
    if let Some(master) = guard.as_mut() {
        master.stop().await;
    }
    *guard = None;
    Ok(())
}

#[tauri::command]
pub async fn connect_substation(
    state: State<'_, AppState>,
    host: String,
    port: u16,
) -> Result<(), String> {
    let guard = state.master.lock().await;
    let master = guard.as_ref().ok_or("Server not running")?;
    let version = master.protocol;
    master.connect_to_substation(host, port, version).await
}

#[tauri::command]
pub async fn send_command(
    state: State<'_, AppState>,
    idcode: String,
    cmd: String,
    period: Option<u32>,
) -> Result<(), String> {
    let guard = state.master.lock().await;
    let master = guard.as_ref().ok_or("Server not running")?;
    master.send_command(idcode, cmd, period).await
}

#[tauri::command]
pub async fn auto_handshake(
    state: State<'_, AppState>,
    idcode: String,
    period: Option<u32>,
) -> Result<(), String> {
    let guard = state.master.lock().await;
    let master = guard.as_ref().ok_or("Server not running")?;
    master.auto_handshake(idcode, period).await
}
