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

/// 启动一个子站；端口 0 → OS 分配 mgmt,V3 数据口取 mgmt+1。mgmt+1 可能被
/// 其他并行测试占用导致 start() 绑定失败 —— 换一个 mgmt 端口重试,直到拿到
/// 一对空闲端口(与 pmusim-app/tests/e2e.rs 的端口重试同构,保证并行不撞)。
async fn spawn_substation(
    version: ProtocolVersion,
) -> (SubStation, mpsc::UnboundedReceiver<SubEvent>, u16) {
    for _ in 0..20 {
        let (tx, rx) = mpsc::unbounded_channel::<SubEvent>();
        let settings = SubSettings {
            version,
            mgmt_port: 0,
            data_port: 0,
            config: sub_config(version),
            gen: DataGen { freq_offset_hz: 0.1, rocof_hz_s: 0.0 },
        };
        let mut sub = SubStation::new(tx, settings);
        if sub.start().await.is_ok() {
            let port = sub.mgmt_port();
            return (sub, rx, port);
        }
    }
    panic!("substation start: 20 次端口绑定均失败");
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

async fn wait_sub_event<F: FnMut(&SubEvent) -> bool>(
    rx: &mut mpsc::UnboundedReceiver<SubEvent>,
    mut pred: F,
) -> SubEvent {
    loop {
        let ev = timeout(Duration::from_secs(8), rx.recv())
            .await
            .expect("sub event timeout")
            .expect("sub channel closed");
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
    let settings = SubSettings {
        version: ProtocolVersion::V2,
        mgmt_port: 0,
        data_port: master_data_port,
        config: sub_config(ProtocolVersion::V2),
        gen: DataGen { freq_offset_hz: 0.05, rocof_hz_s: 0.0 },
    };
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

    let cfg1 = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::Cfg1Received { .. })).await;
    if let PmuEvent::Cfg1Received { idcode, cfg } = cfg1 {
        assert_eq!(idcode, IDCODE);
        assert_eq!(cfg.stn, STN);
        assert_eq!(cfg.annmr, 2);
        assert_eq!(cfg.dgnmr, 1);
    }
    let _ = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::StreamingStarted { .. })).await;
    let data = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::DataFrame { .. })).await;
    if let PmuEvent::DataFrame { data, .. } = data {
        assert_eq!(data.analog.len(), 2);
        assert_eq!(data.digital, vec![0x000A]);
    }

    master.stop().await;
    sub.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn v3_master_pushes_period_zero_gets_nacked() {
    let (mut sub, mut sub_rx, mgmt_port) = spawn_substation(ProtocolVersion::V3).await;

    let (m_tx, mut m_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(m_tx, 0, 30.0, ProtocolVersion::V3);
    master.start().await.unwrap();
    master
        .connect_to_substation("127.0.0.1".into(), mgmt_port, 0, ProtocolVersion::V3)
        .await
        .unwrap();

    // 获取占位符 idcode（用于触发 auto_handshake）。
    let placeholder_idcode = match wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::SessionCreated { .. })).await {
        PmuEvent::SessionCreated { idcode, .. } => idcode,
        _ => unreachable!(),
    };

    // 先完成握手到 streaming（主站缓存 cfg1，子站开始按 50fps 发）。
    master.auto_handshake(placeholder_idcode.clone(), Some(100)).await.unwrap();
    // StreamingStarted 携带真实 IDCODE（auto_handshake 内 wait_for_cfg1 重新定位了会话）。
    let idcode = match wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::StreamingStarted { .. })).await {
        PmuEvent::StreamingStarted { idcode } => idcode,
        _ => unreachable!(),
    };

    // 等握手产生的 Cfg2Received 已入队后再清空,避免它被下方谓词误匹配。
    // 至少等到一帧 DataFrameSent,确认子站已处于稳定推流状态、握手事件已全部入队。
    let _ = wait_sub_event(&mut sub_rx, |e| matches!(e, SubEvent::DataFrameSent { .. })).await;
    while sub_rx.try_recv().is_ok() {} // 清空历史事件

    // 受控注入：下传一帧 PERIOD=0 的 CFG-2（fire-and-forget 路径，无 ack waiter）。
    // 两条命令顺序入 master 的单消费者 cmd_tx,投递顺序即上线顺序,无需 sleep 同步。
    master.send_command(idcode.clone(), "send_cfg2_cmd".into(), None).await.unwrap();
    master.send_command(idcode.clone(), "send_cfg2".into(), Some(0)).await.unwrap();

    // 子站应识别非法上送周期 → 回 NACK + emit Cfg2Rejected。
    // 谓词同时匹配 Cfg2Received(旧错误行为):若回归则立即退出循环并 assert 失败,
    // 而非在 50fps 的 DataFrameSent 流里挂死(per-recv 8s 超时永不触发)。
    let rejected = wait_sub_event(&mut sub_rx, |e| {
        matches!(e, SubEvent::Cfg2Rejected { .. } | SubEvent::Cfg2Received)
    })
    .await;
    let SubEvent::Cfg2Rejected { reason } = rejected else {
        panic!("期望 Cfg2Rejected,实际收到 {rejected:?}(子站应拒绝 PERIOD=0 的 CFG-2)");
    };
    assert!(reason.contains("上送周期"), "原因应说明上送周期非法: {reason}");

    // 保持原状：子站继续推数据（注入后仍能收到 DataFrameSent）。
    let _ = wait_sub_event(&mut sub_rx, |e| matches!(e, SubEvent::DataFrameSent { .. })).await;

    // 主站这端（fire-and-forget 路径无 ack waiter）也必须看见该 NACK。
    let err = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::Error { .. })).await;
    if let PmuEvent::Error { error, .. } = err {
        assert!(error.contains("NACK"), "主站应 surface NACK 错误: {error}");
    }

    master.stop().await;
    sub.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn v3_master_skips_cfg2_streams_via_cfg1() {
    let (mut sub, _sub_rx, mgmt_port) = spawn_substation(ProtocolVersion::V3).await;

    let (m_tx, mut m_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(m_tx, 0, 30.0, ProtocolVersion::V3);
    master.start().await.unwrap();
    master
        .connect_to_substation("127.0.0.1".into(), mgmt_port, 0, ProtocolVersion::V3)
        .await
        .unwrap();

    let placeholder = match wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::SessionCreated { .. })).await {
        PmuEvent::SessionCreated { idcode, .. } => idcode,
        _ => unreachable!(),
    };

    // 跳过 CFG-2 的握手:召唤 CFG-1 → 直接 OpenData(不发任何 CFG-2)。
    master.skip_cfg2_open(placeholder).await.unwrap();

    // 收集到 StreamingStarted 为止的所有主站事件,断言:见到 CFG-1 + Cfg2Skipped,
    // 且全程没有任何 CFG-2 事件(确实跳过)。match &ev 借用,避免在 panic 分支里 use-after-move。
    let mut saw_cfg1 = false;
    let mut saw_skipped = false;
    loop {
        let ev = timeout(Duration::from_secs(8), m_rx.recv())
            .await
            .expect("master event timeout")
            .expect("master channel closed");
        match &ev {
            PmuEvent::Cfg1Received { cfg, .. } => {
                assert_eq!(cfg.annmr, 2);
                assert_eq!(cfg.dgnmr, 1);
                saw_cfg1 = true;
            }
            PmuEvent::Cfg2Skipped { .. } => saw_skipped = true,
            PmuEvent::Cfg2Sent { .. } | PmuEvent::Cfg2Received { .. } => {
                panic!("跳过 CFG-2 路径不应出现 CFG-2 事件: {ev:?}");
            }
            PmuEvent::StreamingStarted { .. } => break,
            _ => {}
        }
    }
    assert!(saw_cfg1, "应收到 CFG-1(维度来源)");
    assert!(saw_skipped, "应收到 Cfg2Skipped 注入标记");

    // 凭 CFG-1 维度成功解出 DataFrame。
    let data = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::DataFrame { .. })).await;
    let PmuEvent::DataFrame { idcode, data } = data else { unreachable!() };
    assert_eq!(idcode, IDCODE);
    assert_eq!(data.phasors.len(), 1);
    assert_eq!(data.analog.len(), 2);
    assert_eq!(data.digital, vec![0x000A]);

    master.stop().await;
    sub.stop().await;
}

