use pmusim_core::protocol::constants::ProtocolVersion;
use tauri::State;
use tokio::sync::mpsc;

use crate::events::PmuEvent;
use crate::network::master::MasterStation;
use crate::state::AppState;

#[tauri::command]
pub async fn start_server(
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
    let buffer = state.events.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            buffer.push(event);
        }
    });
    let mut master = MasterStation::new(event_tx, data_port, 30.0, version);
    master.start().await?;
    *guard = Some(master);
    Ok(())
}

/// Drain all buffered events. Called by the frontend on a short interval
/// (e.g. 100 ms) instead of relying on AppHandle::emit + JS listen(),
/// which races webview-ready on macOS WebKit and can lose every event
/// emitted during the handshake.
#[tauri::command]
pub fn poll_events(state: State<'_, AppState>) -> Vec<PmuEvent> {
    state.events.drain()
}

#[tauri::command]
pub async fn stop_server(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.master.lock().await;
    if let Some(master) = guard.as_mut() {
        // Drop in-flight stragglers (last DataFrame, last RawFrame...)
        // BEFORE stop() so the SessionDisconnected events stop() pushes
        // are guaranteed delivered to the next frontend poll. Drain
        // ordering matters: drain-then-stop-then-(let buffer fill via
        // forward task)-then-poll cleanly hands off "tear-down" events.
        let _ = state.events.drain();
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
    data_port: Option<u16>,
) -> Result<(), String> {
    let guard = state.master.lock().await;
    let master = guard.as_ref().ok_or("Server not running")?;
    let version = master.protocol;
    master
        .connect_to_substation(host, port, data_port.unwrap_or(0), version)
        .await
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

#[tauri::command]
pub async fn disconnect_substation(
    state: State<'_, AppState>,
    idcode: String,
) -> Result<(), String> {
    let guard = state.master.lock().await;
    let master = guard.as_ref().ok_or("Server not running")?;
    master.disconnect_substation(idcode).await
}

#[tauri::command]
pub async fn set_heartbeat_interval(
    state: State<'_, AppState>,
    seconds: f64,
) -> Result<(), String> {
    let guard = state.master.lock().await;
    let master = guard.as_ref().ok_or("Server not running")?;
    master.set_heartbeat_interval(seconds);
    Ok(())
}
