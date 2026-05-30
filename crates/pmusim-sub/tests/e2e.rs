//! End-to-end: 真实 MasterStation 驱动真实 SubStation，跑完整握手 + 数据流。

use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::timeout;

use pmusim_app::events::PmuEvent;
use pmusim_app::network::master::MasterStation;
use pmusim_core::protocol::constants::ProtocolVersion;

use pmusim_sub::datagen::{DataGen, PhasorGen, SubConfig};
use pmusim_sub::events::SubEvent;
use pmusim_sub::network::substation::{SubSettings, SubStation};

const IDCODE: &str = "TESTPMU0";
const STN: &str = "SubTestStn";

fn sub_config(version: ProtocolVersion) -> SubConfig {
    SubConfig {
        version,
        idcode: IDCODE.into(),
        stn: STN.into(),
        data_rate_fps: 50,
        meas_rate: 1_000_000,
        format_flags: 0,
        phasors: vec![PhasorGen { magnitude: 1000.0, phase_deg: 0.0 }],
        analogs: vec![300.0, 3000.0],
        digitals: vec![0x000A],
    }
}

/// 在相邻端口 (p, p+1) 上启动一个子站，返回 (子站, mgmt_port)。
async fn spawn_substation(
    version: ProtocolVersion,
) -> (SubStation, mpsc::UnboundedReceiver<SubEvent>, u16) {
    let (tx, rx) = mpsc::unbounded_channel::<SubEvent>();
    let settings = SubSettings {
        version,
        mgmt_port: 0,
        data_port: 0,
        config: sub_config(version),
        gen: DataGen { freq_offset_hz: 0.1, rocof_hz_s: 0.0 },
    };
    let mut sub = SubStation::new(tx, settings);
    sub.start().await.expect("substation start");
    let port = sub.mgmt_port();
    (sub, rx, port)
}

async fn wait_master_event<F: FnMut(&PmuEvent) -> bool>(
    rx: &mut mpsc::UnboundedReceiver<PmuEvent>,
    mut pred: F,
) -> PmuEvent {
    loop {
        let ev = timeout(Duration::from_secs(8), rx.recv())
            .await
            .expect("master event timeout")
            .expect("master channel closed");
        if pred(&ev) {
            return ev;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn v3_master_drives_substation_to_streaming() {
    let (mut sub, _sub_rx, mgmt_port) = spawn_substation(ProtocolVersion::V3).await;

    let (m_tx, mut m_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(m_tx, 0, 30.0, ProtocolVersion::V3);
    master.start().await.unwrap();

    master
        .connect_to_substation("127.0.0.1".into(), mgmt_port, 0, ProtocolVersion::V3)
        .await
        .unwrap();

    let tmp = match wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::SessionCreated { .. })).await {
        PmuEvent::SessionCreated { idcode, .. } => idcode,
        _ => unreachable!(),
    };

    master.auto_handshake(tmp, Some(100)).await.unwrap();

    let cfg1 = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::Cfg1Received { .. })).await;
    if let PmuEvent::Cfg1Received { idcode, cfg } = cfg1 {
        assert_eq!(idcode, IDCODE);
        assert_eq!(cfg.stn, STN);
        assert_eq!(cfg.annmr, 2);
        assert_eq!(cfg.dgnmr, 1);
    }

    let _ = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::StreamingStarted { .. })).await;

    let data = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::DataFrame { .. })).await;
    if let PmuEvent::DataFrame { idcode, data } = data {
        assert_eq!(idcode, IDCODE);
        assert_eq!(data.analog.len(), 2);
        assert_eq!(data.digital, vec![0x000A]);
        assert_eq!(data.phasors.len(), 1);
    }

    master.stop().await;
    sub.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn v2_master_drives_substation_to_streaming() {
    // 主站 V2：先 start() 取得它绑定的数据监听口
    let (m_tx, mut m_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(m_tx, 0, 30.0, ProtocolVersion::V2);
    master.start().await.unwrap();
    let master_data_port = master.data_port; // V2 下非 0

    // 子站 V2：mgmt 自分配；data_port 指向主站数据口(用于连出)
    let (sub_tx, _sub_rx) = mpsc::unbounded_channel::<SubEvent>();
    let mut settings = SubSettings {
        version: ProtocolVersion::V2,
        mgmt_port: 0,
        data_port: master_data_port,
        config: sub_config(ProtocolVersion::V2),
        gen: DataGen { freq_offset_hz: 0.05, rocof_hz_s: 0.0 },
    };
    settings.config.version = ProtocolVersion::V2;
    let mut sub = SubStation::new(sub_tx, settings);
    sub.start().await.unwrap();
    let mgmt_port = sub.mgmt_port();

    master
        .connect_to_substation("127.0.0.1".into(), mgmt_port, 0, ProtocolVersion::V2)
        .await
        .unwrap();
    let tmp = match wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::SessionCreated { .. })).await {
        PmuEvent::SessionCreated { idcode, .. } => idcode,
        _ => unreachable!(),
    };
    master.auto_handshake(tmp, Some(100)).await.unwrap();

    let _ = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::Cfg1Received { .. })).await;
    let _ = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::StreamingStarted { .. })).await;
    let data = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::DataFrame { .. })).await;
    if let PmuEvent::DataFrame { data, .. } = data {
        assert_eq!(data.analog.len(), 2);
        assert_eq!(data.digital, vec![0x000A]);
    }

    master.stop().await;
    sub.stop().await;
}
