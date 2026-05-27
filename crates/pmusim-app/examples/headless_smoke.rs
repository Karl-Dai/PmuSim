//! Headless smoke test: drives `MasterStation` directly, without Tauri.
//!
//! Usage:
//!   cargo run -p pmusim-app --example headless_smoke -- [host] [port] [data_port]
//!
//! Defaults: 10.15.48.12 8000 18001
//!
//! Exercises:
//!   * master.start() binds the data listener
//!   * connect_to_substation() with the new pending-placeholder semantics
//!   * the duplicate-connect guard (second call to same target should error)
//!   * the 5s TCP connect timeout (visible only if target is unreachable)
//!   * auto_handshake when a real IDCODE arrives
//!
//! All PmuEvents are printed to stdout with a millisecond timestamp.

use std::time::{Duration, Instant};

use pmusim_app::events::PmuEvent;
use pmusim_app::network::master::MasterStation;
use pmusim_core::protocol::constants::ProtocolVersion;
use tokio::sync::mpsc;
use tokio::time::timeout;

fn ts() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let secs = now.as_secs();
    let ms = now.subsec_millis();
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60,
        ms,
    )
}

async fn drain(
    rx: &mut mpsc::UnboundedReceiver<PmuEvent>,
    until: Instant,
    track_real_idcode: &mut Option<String>,
    placeholder: &str,
) -> usize {
    let mut n = 0;
    loop {
        let remaining = until.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match timeout(remaining, rx.recv()).await {
            Ok(Some(ev)) => {
                n += 1;
                println!("[{}] EVENT {:?}", ts(), ev);
                if let PmuEvent::SessionCreated { idcode, .. } = &ev {
                    if idcode != placeholder && !idcode.is_empty() {
                        *track_real_idcode = Some(idcode.clone());
                    }
                }
            }
            Ok(None) => break,
            Err(_) => break, // timeout reached
        }
    }
    n
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let mut args = std::env::args().skip(1);
    let host = args.next().unwrap_or_else(|| "10.15.48.12".to_string());
    let port: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(8000);
    let data_port: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(18001);
    let protocol = ProtocolVersion::V3;
    let placeholder = format!("{host}:{port}");

    println!("[{}] === PmuSim headless smoke test ===", ts());
    println!(
        "[{}] target = {host}:{port}, data_listener = 0.0.0.0:{data_port}, protocol = V3",
        ts()
    );

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(event_tx.clone(), data_port, 30.0, protocol);

    print!("[{}] → master.start() ... ", ts());
    match master.start().await {
        Ok(()) => println!("OK"),
        Err(e) => {
            println!("FAIL: {e}");
            return;
        }
    }

    print!(
        "[{}] → connect_to_substation({host}, {port}, V3) [#1] ... ",
        ts()
    );
    match master
        .connect_to_substation(host.clone(), port, protocol)
        .await
    {
        Ok(()) => println!("queued"),
        Err(e) => println!("queue err: {e}"),
    }

    // Yield so do_connect has a chance to insert the placeholder BEFORE the
    // duplicate call runs in the same command_loop iteration.
    tokio::time::sleep(Duration::from_millis(50)).await;

    print!(
        "[{}] → connect_to_substation({host}, {port}, V3) [#2 — expect dup-guard Error] ... ",
        ts()
    );
    match master
        .connect_to_substation(host.clone(), port, protocol)
        .await
    {
        Ok(()) => println!("queued"),
        Err(e) => println!("queue err: {e}"),
    }

    // Some substations close the mgmt pipe within seconds if no command is
    // sent, so kick auto_handshake immediately (against the placeholder id;
    // do_send_cmd waits briefly for the session to be ready). The substation's
    // first response will re-key the session to its real IDCODE.
    let mut real_idcode: Option<String> = None;
    print!(
        "[{}] → auto_handshake({placeholder}, period=50) ... ",
        ts()
    );
    match master.auto_handshake(placeholder.clone(), Some(50)).await {
        Ok(()) => println!("queued"),
        Err(e) => println!("queue err: {e}"),
    }

    // Observe handshake + initial data frames for 20s.
    println!("[{}] --- handshake + initial stream drain: 20s ---", ts());
    let n1 = drain(
        &mut event_rx,
        Instant::now() + Duration::from_secs(20),
        &mut real_idcode,
        &placeholder,
    )
    .await;
    println!("[{}] --- drain done, {n1} events ---", ts());
    let driver_id = real_idcode.clone().unwrap_or_else(|| placeholder.clone());

    print!("[{}] → disconnect_substation({driver_id}) ... ", ts());
    let target_id = real_idcode.clone().unwrap_or(driver_id);
    match master.disconnect_substation(target_id.clone()).await {
        Ok(()) => println!("queued"),
        Err(e) => println!("queue err: {e}"),
    }

    let n3 = drain(
        &mut event_rx,
        Instant::now() + Duration::from_secs(2),
        &mut real_idcode,
        &placeholder,
    )
    .await;
    println!("[{}] --- disconnect drain: {n3} events ---", ts());

    print!("[{}] → master.stop() ... ", ts());
    master.stop().await;
    println!("OK");

    // Drain any final events before dropping the sender.
    drop(event_tx);
    while let Some(ev) = event_rx.recv().await {
        println!("[{}] (post-stop) {:?}", ts(), ev);
    }

    println!("[{}] === done ===", ts());
}
