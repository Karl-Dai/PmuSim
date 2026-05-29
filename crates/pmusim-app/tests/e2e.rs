//! End-to-end tests: drive `MasterStation` against an in-process mock
//! substation through the V3 handshake and verify the event stream.

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use pmusim_app::events::PmuEvent;
use pmusim_app::network::master::MasterStation;
use pmusim_core::protocol::builder::{build_command, build_config, build_data};
use pmusim_core::protocol::constants::{Cmd, FrameType, ProtocolVersion, SYNC_BYTE};
use pmusim_core::protocol::frame::{CommandFrame, ConfigFrame, DataFrame, Frame};
use pmusim_core::protocol::parser::parse;

const IDCODE: &str = "TESTPMU0";
const STN: &str = "TestStation";

fn make_cfg(cfg_type: u8) -> ConfigFrame {
    let annmr = 2u16;
    let dgnmr = 1u16;
    let mut channel_names: Vec<String> = (0..annmr).map(|i| format!("AN{i}")).collect();
    for i in 0..(dgnmr * 16) {
        channel_names.push(format!("D{i:02}"));
    }
    ConfigFrame {
        version: ProtocolVersion::V3,
        cfg_type,
        idcode: IDCODE.into(),
        soc: 0x67B2C719,
        fracsec: 0,
        d_frame: 0,
        meas_rate: 1_000_000,
        num_pmu: 1,
        stn: STN.into(),
        pmu_idcode: IDCODE.into(),
        format_flags: 0,
        phnmr: 0,
        annmr,
        dgnmr,
        channel_names,
        phunit: vec![],
        anunit: vec![100, 200],
        digunit: vec![(0x0001, 0x0000)],
        fnom: 0x0001,
        period: 100,
        pmu_blocks: vec![],
    }
}

fn make_data_frame(soc: u32) -> DataFrame {
    DataFrame {
        version: ProtocolVersion::V3,
        idcode: IDCODE.into(),
        soc,
        fracsec: 0,
        stat: 0x0000,
        format_flags: 0,
        phasors: vec![],
        freq: 0.0,
        dfreq: 0.0,
        analog: vec![300.0, 3000.0],
        digital: vec![0x000A],
    }
}

async fn read_one_frame(reader: &mut OwnedReadHalf) -> Result<Vec<u8>, String> {
    let mut header = [0u8; 4];
    reader
        .read_exact(&mut header)
        .await
        .map_err(|e| e.to_string())?;
    if header[0] != SYNC_BYTE {
        return Err(format!("bad sync {:#04x}", header[0]));
    }
    let size = u16::from_be_bytes([header[2], header[3]]) as usize;
    if size < 4 {
        return Err(format!("bad size {size}"));
    }
    let mut buf = vec![0u8; size];
    buf[..4].copy_from_slice(&header);
    reader
        .read_exact(&mut buf[4..])
        .await
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

async fn wait_event<F>(rx: &mut mpsc::UnboundedReceiver<PmuEvent>, mut pred: F) -> PmuEvent
where
    F: FnMut(&PmuEvent) -> bool,
{
    loop {
        let ev = timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("event timeout")
            .expect("event channel closed");
        if pred(&ev) {
            return ev;
        }
    }
}

fn ack_command() -> Vec<u8> {
    build_command(&CommandFrame {
        version: ProtocolVersion::V3,
        idcode: IDCODE.into(),
        soc: 0,
        fracsec: 0,
        cmd: Cmd::Ack as u16,
    })
    .unwrap()
}

fn nack_command() -> Vec<u8> {
    build_command(&CommandFrame {
        version: ProtocolVersion::V3,
        idcode: IDCODE.into(),
        soc: 0,
        fracsec: 0,
        cmd: Cmd::Nack as u16,
    })
    .unwrap()
}

/// Records every CFG frame the mock substation receives from the master.
#[derive(Default)]
struct MockObservations {
    received_cfg_types: Vec<u8>,
}

type ObsHandle = Arc<Mutex<MockObservations>>;

/// Spawn a V3 mock substation. Returns its mgmt port, its task handle, and a
/// shared observation log. In V3 the substation is the TCP server on BOTH
/// mgmt (port N) and data (port N+1) — master initiates connections to both
/// per the do_open_data_v3 mgmt+1 convention.
async fn spawn_mock_substation(_master_data_port: u16) -> (u16, JoinHandle<()>, ObsHandle) {
    // Bind adjacent ports so the master's `peer_mgmt_port + 1` convention
    // resolves to the listener we control. Retry until we can hold both.
    let (mgmt_listener, mgmt_port, data_listener) = loop {
        let m = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let p = m.local_addr().unwrap().port();
        match TcpListener::bind(("127.0.0.1", p + 1)).await {
            Ok(d) => break (m, p, d),
            Err(_) => continue, // p+1 was busy, get a fresh mgmt port
        }
    };
    let obs: ObsHandle = Arc::new(Mutex::new(MockObservations::default()));
    let obs_for_task = obs.clone();

    let task = tokio::spawn(async move {
        let (stream, _) = mgmt_listener.accept().await.unwrap();
        let (mut reader, mut writer) = stream.into_split();
        let mut data_writer: Option<tokio::net::tcp::OwnedWriteHalf> = None;

        // Spawn an accept task that captures the master's inbound V3 data
        // connect (which is triggered immediately before Cmd::OpenData).
        let (data_tx, mut data_rx) =
            mpsc::unbounded_channel::<tokio::net::tcp::OwnedWriteHalf>();
        tokio::spawn(async move {
            if let Ok((s, _)) = data_listener.accept().await {
                let (_, dw) = s.into_split();
                let _ = data_tx.send(dw);
            }
        });

        loop {
            let frame_data = match read_one_frame(&mut reader).await {
                Ok(d) => d,
                Err(_) => break,
            };
            let parsed = parse(&frame_data, 0, 0, 0, 0);
            match parsed {
                Ok(Frame::Command(cmd)) => match cmd.cmd {
                    c if c == Cmd::SendCfg1 as u16 => {
                        let bytes = build_config(&make_cfg(FrameType::Cfg1 as u8)).unwrap();
                        writer.write_all(&bytes).await.unwrap();
                    }
                    c if c == Cmd::SendCfg2Cmd as u16 => {
                        writer.write_all(&ack_command()).await.unwrap();
                    }
                    c if c == Cmd::SendCfg2 as u16 => {
                        let bytes = build_config(&make_cfg(FrameType::Cfg2 as u8)).unwrap();
                        writer.write_all(&bytes).await.unwrap();
                    }
                    c if c == Cmd::OpenData as u16 => {
                        // Master should have already dialed our data listener
                        // before sending Cmd::OpenData. Pick up that writer
                        // half and start streaming on it.
                        if data_writer.is_none() {
                            data_writer = data_rx.recv().await;
                        }
                        let bytes = build_data(&make_data_frame(0x67A99D11), 0, 2, 1).unwrap();
                        if let Some(dw) = data_writer.as_mut() {
                            dw.write_all(&bytes).await.unwrap();
                        }
                    }
                    c if c == Cmd::CloseData as u16 => {
                        data_writer.take();
                    }
                    c if c == Cmd::Heartbeat as u16 => {
                        let hb = build_command(&CommandFrame {
                            version: ProtocolVersion::V3,
                            idcode: IDCODE.into(),
                            soc: 0,
                            fracsec: 0,
                            cmd: Cmd::Heartbeat as u16,
                        })
                        .unwrap();
                        writer.write_all(&hb).await.unwrap();
                    }
                    _ => {}
                },
                Ok(Frame::Config(cfg)) => {
                    obs_for_task.lock().await.received_cfg_types.push(cfg.cfg_type);
                    writer.write_all(&ack_command()).await.unwrap();
                }
                _ => {}
            }
        }
    });

    (mgmt_port, task, obs)
}

#[tokio::test]
async fn v3_full_handshake_streams_data() {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(event_tx, 0, 30.0, ProtocolVersion::V3);
    master.start().await.unwrap();
    let master_data_port = master.data_port;

    let (mock_port, mock_task, obs) = spawn_mock_substation(master_data_port).await;

    master
        .connect_to_substation("127.0.0.1".into(), mock_port, 0, ProtocolVersion::V3)
        .await
        .unwrap();

    let tmp_id = match wait_event(&mut event_rx, |e| {
        matches!(e, PmuEvent::SessionCreated { .. })
    })
    .await
    {
        PmuEvent::SessionCreated { idcode, .. } => idcode,
        _ => unreachable!(),
    };
    assert!(tmp_id.contains("127.0.0.1"), "tmp id was {tmp_id}");

    master
        .send_command(tmp_id, "request_cfg1".into(), None)
        .await
        .unwrap();
    let cfg1_event = wait_event(&mut event_rx, |e| {
        matches!(e, PmuEvent::Cfg1Received { .. })
    })
    .await;
    let real_idcode = if let PmuEvent::Cfg1Received { idcode, cfg } = cfg1_event {
        assert_eq!(cfg.stn, STN);
        assert_eq!(cfg.annmr, 2);
        assert_eq!(cfg.dgnmr, 1);
        idcode
    } else {
        unreachable!()
    };
    assert_eq!(real_idcode, IDCODE);

    master
        .send_command(real_idcode.clone(), "send_cfg2_cmd".into(), None)
        .await
        .unwrap();
    master
        .send_command(real_idcode.clone(), "send_cfg2".into(), Some(100))
        .await
        .unwrap();
    let _ = wait_event(&mut event_rx, |e| matches!(e, PmuEvent::Cfg2Sent { .. })).await;

    // Master must downstream a real CFG-2 frame (cfg_type=3), not CFG-1.
    // Wait until the mock has actually parsed the incoming frame.
    let received = timeout(Duration::from_secs(2), async {
        loop {
            let v = obs.lock().await.received_cfg_types.clone();
            if !v.is_empty() {
                return v;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("mock never recorded CFG download");
    assert_eq!(
        received,
        vec![FrameType::Cfg2 as u8],
        "master downstreamed wrong cfg_type"
    );

    master
        .send_command(real_idcode.clone(), "request_cfg2".into(), None)
        .await
        .unwrap();
    let _ = wait_event(&mut event_rx, |e| {
        matches!(e, PmuEvent::Cfg2Received { .. })
    })
    .await;

    master
        .send_command(real_idcode.clone(), "open_data".into(), None)
        .await
        .unwrap();
    let _ = wait_event(&mut event_rx, |e| {
        matches!(e, PmuEvent::StreamingStarted { .. })
    })
    .await;

    let data_event = wait_event(&mut event_rx, |e| matches!(e, PmuEvent::DataFrame { .. })).await;
    if let PmuEvent::DataFrame { idcode, data } = data_event {
        assert_eq!(idcode, IDCODE);
        assert_eq!(data.soc, 0x67A99D11);
        assert_eq!(data.analog.len(), 2);
        assert_eq!(data.digital, vec![0x000A]);
    }

    master.stop().await;
    mock_task.abort();
}

#[tokio::test]
async fn v3_auto_handshake_from_tmp_id_reaches_streaming() {
    // Exercises the re-key path: caller passes tmp_id, but the substation
    // replies with its real IDCODE mid-handshake. `auto_handshake` should
    // still drive the session through to OpenData.
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(event_tx, 0, 30.0, ProtocolVersion::V3);
    master.start().await.unwrap();
    let master_data_port = master.data_port;

    let (mock_port, mock_task, _obs) = spawn_mock_substation(master_data_port).await;

    master
        .connect_to_substation("127.0.0.1".into(), mock_port, 0, ProtocolVersion::V3)
        .await
        .unwrap();

    let tmp_id = match wait_event(&mut event_rx, |e| {
        matches!(e, PmuEvent::SessionCreated { .. })
    })
    .await
    {
        PmuEvent::SessionCreated { idcode, .. } => idcode,
        _ => unreachable!(),
    };
    assert!(tmp_id.contains("127.0.0.1"));

    // Kick off auto-handshake using the tmp_id. After CFG-1 the session is
    // re-keyed; the handshake must follow it.
    master.auto_handshake(tmp_id, None).await.unwrap();

    let cfg1_event = wait_event(&mut event_rx, |e| {
        matches!(e, PmuEvent::Cfg1Received { .. })
    })
    .await;
    if let PmuEvent::Cfg1Received { idcode, .. } = cfg1_event {
        assert_eq!(idcode, IDCODE);
    }

    let _ = wait_event(&mut event_rx, |e| matches!(e, PmuEvent::Cfg2Sent { .. })).await;
    let _ = wait_event(&mut event_rx, |e| {
        matches!(e, PmuEvent::Cfg2Received { .. })
    })
    .await;
    let _ = wait_event(&mut event_rx, |e| {
        matches!(e, PmuEvent::StreamingStarted { .. })
    })
    .await;
    let data_event = wait_event(&mut event_rx, |e| matches!(e, PmuEvent::DataFrame { .. })).await;
    if let PmuEvent::DataFrame { idcode, .. } = data_event {
        assert_eq!(idcode, IDCODE);
    }

    master.stop().await;
    mock_task.abort();
}

#[tokio::test]
async fn v3_handshake_with_explicit_data_port() {
    // Bind mgmt + a NON-adjacent data port (mgmt + 10) so the master must
    // use the explicit data_port argument, not the mgmt+1 default. Retry
    // until both ports are free so this test doesn't flake under parallel
    // execution where another test's spawn_mock_substation might have
    // already taken mgmt_port + 10.
    let (mgmt_listener, mgmt_port, data_listener, custom_data_port) = loop {
        let m = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let p = m.local_addr().unwrap().port();
        let Some(dp) = p.checked_add(10) else { continue };
        match TcpListener::bind(("127.0.0.1", dp)).await {
            Ok(d) => break (m, p, d, dp),
            Err(_) => continue,
        }
    };

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(event_tx, 0, 30.0, ProtocolVersion::V3);
    master.start().await.unwrap();

    // Reuse the mock substation body — accepts mgmt, replies to handshake,
    // serves data on whichever port the master happens to dial.
    let obs: ObsHandle = Arc::new(Mutex::new(MockObservations::default()));
    let obs_for_task = obs.clone();
    let mock_task = tokio::spawn(async move {
        let (stream, _) = mgmt_listener.accept().await.unwrap();
        let (mut reader, mut writer) = stream.into_split();
        let mut data_writer: Option<tokio::net::tcp::OwnedWriteHalf> = None;
        let (data_tx, mut data_rx) =
            mpsc::unbounded_channel::<tokio::net::tcp::OwnedWriteHalf>();
        tokio::spawn(async move {
            if let Ok((s, _)) = data_listener.accept().await {
                let (_, dw) = s.into_split();
                let _ = data_tx.send(dw);
            }
        });
        loop {
            let frame_data = match read_one_frame(&mut reader).await {
                Ok(d) => d,
                Err(_) => break,
            };
            match parse(&frame_data, 0, 0, 0, 0) {
                Ok(Frame::Command(cmd)) => match cmd.cmd {
                    c if c == Cmd::SendCfg1 as u16 => {
                        writer.write_all(&build_config(&make_cfg(FrameType::Cfg1 as u8)).unwrap()).await.unwrap();
                    }
                    c if c == Cmd::SendCfg2Cmd as u16 => {
                        writer.write_all(&ack_command()).await.unwrap();
                    }
                    c if c == Cmd::SendCfg2 as u16 => {
                        writer.write_all(&build_config(&make_cfg(FrameType::Cfg2 as u8)).unwrap()).await.unwrap();
                    }
                    c if c == Cmd::OpenData as u16 => {
                        if data_writer.is_none() {
                            data_writer = data_rx.recv().await;
                        }
                        let bytes = build_data(&make_data_frame(0x67A99D11), 0, 2, 1).unwrap();
                        if let Some(dw) = data_writer.as_mut() {
                            dw.write_all(&bytes).await.unwrap();
                        }
                    }
                    _ => {}
                },
                Ok(Frame::Config(cfg)) => {
                    obs_for_task.lock().await.received_cfg_types.push(cfg.cfg_type);
                    writer.write_all(&ack_command()).await.unwrap();
                }
                _ => {}
            }
        }
    });

    // Connect with EXPLICIT non-default data_port
    master
        .connect_to_substation(
            "127.0.0.1".into(),
            mgmt_port,
            custom_data_port,
            ProtocolVersion::V3,
        )
        .await
        .unwrap();

    let tmp_id = match wait_event(&mut event_rx, |e| {
        matches!(e, PmuEvent::SessionCreated { .. })
    })
    .await
    {
        PmuEvent::SessionCreated { idcode, .. } => idcode,
        _ => unreachable!(),
    };
    master.auto_handshake(tmp_id, None).await.unwrap();

    // Walk through to a DataFrame to prove the master dialed the custom data port.
    let _ = wait_event(&mut event_rx, |e| matches!(e, PmuEvent::Cfg2Sent { .. })).await;
    let _ = wait_event(&mut event_rx, |e| matches!(e, PmuEvent::Cfg2Received { .. })).await;
    let _ = wait_event(&mut event_rx, |e| matches!(e, PmuEvent::StreamingStarted { .. })).await;
    let data_event = wait_event(&mut event_rx, |e| matches!(e, PmuEvent::DataFrame { .. })).await;
    if let PmuEvent::DataFrame { idcode, .. } = data_event {
        assert_eq!(idcode, IDCODE);
    }

    let _ = obs;
    master.stop().await;
    mock_task.abort();
}

/// When the substation NACKs the SendCfg2Cmd, the master must abort the
/// handshake before sending CFG-2 / OpenData and surface an Error event —
/// not silently proceed (which it did before pending_ack was added).
#[tokio::test]
async fn v3_nack_on_send_cfg2_cmd_aborts_handshake() {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(event_tx, 0, 30.0, ProtocolVersion::V3);
    master.start().await.unwrap();

    let mgmt_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mgmt_port = mgmt_listener.local_addr().unwrap().port();

    let mock_task = tokio::spawn(async move {
        let (stream, _) = mgmt_listener.accept().await.unwrap();
        let (mut reader, mut writer) = stream.into_split();
        loop {
            let frame_data = match read_one_frame(&mut reader).await {
                Ok(d) => d,
                Err(_) => break,
            };
            if let Ok(Frame::Command(cmd)) = parse(&frame_data, 0, 0, 0, 0) {
                if cmd.cmd == Cmd::SendCfg1 as u16 {
                    writer.write_all(&build_config(&make_cfg(FrameType::Cfg1 as u8)).unwrap()).await.unwrap();
                } else if cmd.cmd == Cmd::SendCfg2Cmd as u16 {
                    // NACK instead of ACK. Master must NOT proceed.
                    writer.write_all(&nack_command()).await.unwrap();
                } else if cmd.cmd == Cmd::SendCfg2 as u16 {
                    panic!("master sent召唤CFG-2 despite NACK on SendCfg2Cmd");
                } else if cmd.cmd == Cmd::OpenData as u16 {
                    panic!("master sent OpenData despite NACK on SendCfg2Cmd");
                }
            }
        }
    });

    master
        .connect_to_substation("127.0.0.1".into(), mgmt_port, 0, ProtocolVersion::V3)
        .await
        .unwrap();
    let tmp_id = match wait_event(&mut event_rx, |e| matches!(e, PmuEvent::SessionCreated { .. })).await {
        PmuEvent::SessionCreated { idcode, .. } => idcode,
        _ => unreachable!(),
    };
    master.auto_handshake(tmp_id, None).await.unwrap();

    // Expect an Error event mentioning NACK; must arrive within 5s.
    let err = wait_event(&mut event_rx, |e| {
        matches!(e, PmuEvent::Error { error, .. } if error.contains("NACK"))
    })
    .await;
    if let PmuEvent::Error { error, .. } = err {
        assert!(error.contains("NACK"), "expected NACK error, got: {error}");
    }

    // Give the mock a brief grace period to assert no further frames.
    tokio::time::sleep(Duration::from_millis(200)).await;

    master.stop().await;
    mock_task.abort();
}

#[tokio::test]
async fn v3_start_does_not_bind_local_data_port() {
    let (event_tx, _event_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(event_tx, 0, 30.0, ProtocolVersion::V3);
    master.start().await.unwrap();
    assert_eq!(master.data_port, 0, "V3 master must not bind a local data listener");
    master.stop().await;
}

#[tokio::test]
async fn v2_start_still_binds_local_data_port() {
    let (event_tx, _event_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(event_tx, 0, 30.0, ProtocolVersion::V2);
    master.start().await.unwrap();
    assert!(master.data_port != 0, "V2 master must bind a real local data listener");
    master.stop().await;
}
