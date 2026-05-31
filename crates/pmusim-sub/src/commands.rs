use serde::Deserialize;
use tauri::State;
use tokio::sync::mpsc;

use pmusim_core::protocol::constants::ProtocolVersion;

use crate::datagen::{DataGen, PhasorGen, SubConfig};
use crate::events::SubEvent;
use crate::network::substation::{SubSettings, SubStation};
use crate::state::AppState;

/// 前端传入的相量定义。
#[derive(Debug, Clone, Deserialize)]
pub struct PhasorInput {
    pub magnitude: f64,
    pub phase_deg: f64,
}

/// 前端传入的完整子站配置。
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigInput {
    pub protocol: String,       // "V2" | "V3"
    pub idcode: String,
    pub stn: String,
    pub mgmt_port: u16,
    pub data_port: u16,         // V3 监听口 / V2 主站数据口；0=默认
    pub data_rate_fps: u32,
    pub phasors: Vec<PhasorInput>,
    pub analogs: Vec<f64>,
    pub digitals: Vec<u16>,
}

fn to_settings(c: &ConfigInput) -> Result<SubSettings, String> {
    let version = match c.protocol.as_str() {
        "V2" => ProtocolVersion::V2,
        "V3" => ProtocolVersion::V3,
        other => return Err(format!("未知协议: {other}")),
    };
    let config = SubConfig {
        version,
        idcode: c.idcode.clone(),
        stn: c.stn.clone(),
        data_rate_fps: c.data_rate_fps.max(1),
        meas_rate: 1_000_000,
        format_flags: 0,
        phasors: c.phasors.iter().map(|p| PhasorGen { magnitude: p.magnitude, phase_deg: p.phase_deg }).collect(),
        analogs: c.analogs.clone(),
        digitals: c.digitals.clone(),
    };
    Ok(SubSettings {
        version,
        mgmt_port: c.mgmt_port,
        data_port: c.data_port,
        config,
        gen: DataGen { freq_offset_hz: 0.0, rocof_hz_s: 0.0 },
    })
}

#[tauri::command]
pub async fn start_substation(state: State<'_, AppState>, config: ConfigInput) -> Result<(), String> {
    let mut guard = state.sub.lock().await;
    if guard.is_some() {
        return Err("子站已在运行".into());
    }
    let settings = to_settings(&config)?;
    let (tx, mut rx) = mpsc::unbounded_channel::<SubEvent>();
    let buffer = state.events.clone();
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await { buffer.push(ev); }
    });
    let mut sub = SubStation::new(tx, settings);
    sub.start().await?;
    *guard = Some(sub);
    Ok(())
}

#[tauri::command]
pub async fn stop_substation(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.sub.lock().await;
    if let Some(sub) = guard.as_mut() {
        let _ = state.events.drain();
        sub.stop().await;
    }
    *guard = None;
    Ok(())
}

#[tauri::command]
pub async fn update_config(state: State<'_, AppState>, config: ConfigInput) -> Result<(), String> {
    let settings = to_settings(&config)?;
    let guard = state.sub.lock().await;
    let sub = guard.as_ref().ok_or("子站未运行")?;
    sub.update_config(settings.config).await;
    Ok(())
}

#[tauri::command]
pub async fn update_gen(state: State<'_, AppState>, freq_offset_hz: f64, rocof_hz_s: f64) -> Result<(), String> {
    let guard = state.sub.lock().await;
    let sub = guard.as_ref().ok_or("子站未运行")?;
    sub.update_gen(DataGen { freq_offset_hz, rocof_hz_s }).await;
    Ok(())
}

#[tauri::command]
pub async fn fire_trigger(state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.sub.lock().await;
    let sub = guard.as_ref().ok_or("子站未运行")?;
    sub.trigger();
    Ok(())
}

#[tauri::command]
pub fn poll_events(state: State<'_, AppState>) -> Vec<SubEvent> {
    state.events.drain()
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("only http(s) urls allowed".into());
    }
    #[cfg(target_os = "macos")]
    let (cmd, args): (&str, Vec<&str>) = ("open", vec![url.as_str()]);
    #[cfg(target_os = "windows")]
    let (cmd, args): (&str, Vec<&str>) = ("cmd", vec!["/C", "start", "", url.as_str()]);
    #[cfg(target_os = "linux")]
    let (cmd, args): (&str, Vec<&str>) = ("xdg-open", vec![url.as_str()]);
    std::process::Command::new(cmd).args(&args).spawn().map_err(|e| e.to_string())?;
    Ok(())
}
