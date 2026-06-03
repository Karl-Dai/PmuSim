//! 忠实复现 App「连接」按钮在 V2 模式下的后端路径:
//!   MasterStation::new(V2) → start() → connect_to_substation → auto_handshake
//! 然后打印 12s 内的所有 PmuEvent,定位握手卡在哪一步。
//!
//! Usage:
//!   cargo run -p pmusim-app --example v2_master_repro -- <host> <mgmt_port> <listen_port>

use std::time::{Duration, Instant};

use pmusim_app::events::PmuEvent;
use pmusim_app::network::master::MasterStation;
use pmusim_core::protocol::constants::ProtocolVersion;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let mut args = std::env::args().skip(1);
    let host = args.next().unwrap_or_else(|| "10.15.48.182".to_string());
    let port: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(8000);
    let listen_port: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(8001);

    println!("V2 repro: connect master → {host}:{port}, local listen {listen_port}");

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(event_tx, listen_port, 5.0, ProtocolVersion::V2);

    match master.start().await {
        Ok(()) => println!("master.start() OK (V2 data listener on {})", master.data_port),
        Err(e) => { println!("master.start() FAIL: {e}"); return; }
    }

    // 复现 App:占位 idcode = host:port
    let placeholder = format!("{host}:{port}");
    master
        .connect_to_substation(host.clone(), port, 0, ProtocolVersion::V2)
        .await
        .unwrap();
    // App 用 rateHz=100 → period = round(1000/100*100/20)=50
    master.auto_handshake(placeholder.clone(), Some(50)).await.unwrap();

    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() { break; }
        match timeout(remaining, event_rx.recv()).await {
            Ok(Some(ev)) => {
                // RawFrame 太长,截断
                match &ev {
                    PmuEvent::RawFrame { idcode, direction, hex } => {
                        let h = if hex.len() > 48 { format!("{}…({}B)", &hex[..48], hex.len()/2) } else { hex.clone() };
                        println!("EVENT RawFrame {{ idcode: {idcode:?}, dir: {direction}, hex: {h} }}");
                    }
                    other => println!("EVENT {other:?}"),
                }
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
    println!("--- 12s 结束 ---");
    master.stop().await;
}
