use std::sync::atomic::Ordering;
use std::sync::Arc;

use log::{error, info, warn};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;

use pmusim_core::protocol::builder::{build_command, build_config, build_data};
use pmusim_core::protocol::constants::{Cmd, FrameType, ProtocolVersion, SYNC_BYTE};
use pmusim_core::protocol::frame::{CommandFrame, ConfigFrame, Frame, PmuBlock};
use pmusim_core::protocol::parser::parse;
use pmusim_core::time_utils::current_soc;

use crate::datagen::{next_data_frame, DataGen, SubConfig};
use crate::events::{DataInfo, SubEvent};

pub type EventSender = mpsc::UnboundedSender<SubEvent>;

/// 子站运行时配置：监听端口 + 协议 + 通道/速率（datagen 用） + 主站数据口。
#[derive(Debug, Clone)]
pub struct SubSettings {
    pub version: ProtocolVersion,
    /// 管理端口（子站作服务端监听）。V2 默认 7000，V3 默认 8000。
    pub mgmt_port: u16,
    /// 数据端口。V3：子站监听此口等主站连入；V2：子站作客户端连主站此口。
    pub data_port: u16,
    pub config: SubConfig,
    pub gen: DataGen,
}

pub struct SubStation {
    settings: Arc<RwLock<SubSettings>>,
    /// 运行期可调的频率参数（推流循环每帧读取，无需重启任务）。
    gen: Arc<RwLock<DataGen>>,
    /// 一次性触发标志，被推流循环消费。
    trigger: Arc<std::sync::atomic::AtomicBool>,
    event_tx: EventSender,
    mgmt_port: u16,
    data_port: u16,
    tasks: Vec<JoinHandle<()>>,
    /// 当前数据写入端（V2=连出主站后填入；V3=接受主站连入后填入）。
    data_writer: Arc<Mutex<Option<tokio::net::tcp::OwnedWriteHalf>>>,
    /// 推流任务句柄（OpenData 启动，CloseData 中止）。
    stream_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl SubStation {
    pub fn new(event_tx: EventSender, settings: SubSettings) -> Self {
        let gen = settings.gen;
        let mgmt_port = settings.mgmt_port;
        let data_port = settings.data_port;
        Self {
            settings: Arc::new(RwLock::new(settings)),
            gen: Arc::new(RwLock::new(gen)),
            trigger: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            event_tx,
            mgmt_port,
            data_port,
            tasks: Vec::new(),
            data_writer: Arc::new(Mutex::new(None)),
            stream_task: Arc::new(Mutex::new(None)),
        }
    }

    /// 实际绑定到的管理端口（端口传 0 时由 OS 分配，便于测试）。
    pub fn mgmt_port(&self) -> u16 { self.mgmt_port }
    /// 实际绑定到的数据端口（V3）。
    pub fn data_port(&self) -> u16 { self.data_port }

    /// 绑定监听并启动命令响应循环。
    pub async fn start(&mut self) -> Result<(), String> {
        // 管理管道：子站永远是服务端。
        let mgmt_listener = TcpListener::bind(("0.0.0.0", self.mgmt_port))
            .await
            .map_err(|e| format!("绑定管理端口 {} 失败: {e}", self.mgmt_port))?;
        self.mgmt_port = mgmt_listener.local_addr().map(|a| a.port()).unwrap_or(self.mgmt_port);

        let version = { self.settings.read().await.version };

        // V3：数据管道子站作服务端，开机即监听等主站连入。
        // V2：数据管道子站作客户端，OpenData 时再连出，这里不绑定。
        let data_listener = if version == ProtocolVersion::V3 {
            let want = if self.data_port == 0 { self.mgmt_port + 1 } else { self.data_port };
            let l = TcpListener::bind(("0.0.0.0", want))
                .await
                .map_err(|e| format!("绑定数据端口 {want} 失败: {e}"))?;
            self.data_port = l.local_addr().map(|a| a.port()).unwrap_or(want);
            Some(l)
        } else {
            self.data_port = 0;
            None
        };

        emit_event(&self.event_tx, SubEvent::Listening {
            mgmt_port: self.mgmt_port,
            data_port: self.data_port,
        });
        info!("SubStation listening: mgmt={} data={}", self.mgmt_port, self.data_port);

        // V3 数据 accept 任务：把主站连入的写半填进 data_writer。
        if let Some(listener) = data_listener {
            let dw = self.data_writer.clone();
            let evt = self.event_tx.clone();
            self.tasks.push(tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((stream, _addr)) => {
                            let (_r, w) = stream.into_split();
                            *dw.lock().await = Some(w);
                            info!("V3 数据管道:主站已连入");
                        }
                        Err(e) => {
                            warn!("数据监听 accept 出错: {e}");
                            emit_event(&evt, SubEvent::Error { error: format!("数据监听 accept 出错: {e}") });
                            break;
                        }
                    }
                }
            }));
        }

        // 管理 accept 循环：每个主站连接起一个命令响应任务。
        let settings = self.settings.clone();
        let gen = self.gen.clone();
        let trigger = self.trigger.clone();
        let evt = self.event_tx.clone();
        let dw = self.data_writer.clone();
        let stream_task = self.stream_task.clone();
        // 单子站设计:mgmt_loop 在 accept 循环内直接 await(非 spawn),
        // 同一时刻只服务一个主站连接;当前主站断开后才接受下一个。
        self.tasks.push(tokio::spawn(async move {
            loop {
                let Ok((stream, addr)) = mgmt_listener.accept().await else { break; };
                let peer_ip = addr.ip().to_string();
                emit_event(&evt, SubEvent::MasterConnected { peer_ip: peer_ip.clone() });
                let (reader, writer) = stream.into_split();
                let writer = Arc::new(Mutex::new(writer));
                Self::mgmt_loop(
                    reader, writer, peer_ip,
                    settings.clone(), gen.clone(), trigger.clone(),
                    evt.clone(), dw.clone(), stream_task.clone(),
                ).await;
            }
        }));

        Ok(())
    }

    pub async fn stop(&mut self) {
        if let Some(h) = self.stream_task.lock().await.take() { h.abort(); }
        for t in self.tasks.drain(..) { t.abort(); }
        *self.data_writer.lock().await = None;
        info!("SubStation stopped");
    }

    /// 运行期更新通道配置（站名/通道/速率等）。
    pub async fn update_config(&self, config: SubConfig) {
        let mut s = self.settings.write().await;
        s.config = config;
    }

    /// 运行期更新频率参数（Δf/ROCOF），推流循环下一帧生效。
    pub async fn update_gen(&self, gen: DataGen) {
        *self.gen.write().await = gen;
        let mut s = self.settings.write().await;
        s.gen = gen;
    }

    /// 触发一帧带触发标记的数据帧。
    pub fn trigger(&self) {
        self.trigger.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// 管理管道命令响应循环。读主站帧 → 分派 → 回应。完全镜像
    /// pmusim-app/tests/e2e.rs 的 mock 子站命令表。
    #[allow(clippy::too_many_arguments)]
    async fn mgmt_loop(
        mut reader: OwnedReadHalf,
        writer: Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
        peer_ip: String,
        settings: Arc<RwLock<SubSettings>>,
        gen: Arc<RwLock<DataGen>>,
        trigger: Arc<std::sync::atomic::AtomicBool>,
        evt: EventSender,
        data_writer: Arc<Mutex<Option<tokio::net::tcp::OwnedWriteHalf>>>,
        stream_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    ) {
        loop {
            let frame_data = match read_frame(&mut reader).await {
                Ok(d) => d,
                Err(_) => break,
            };
            emit_event(&evt, SubEvent::RawFrame { direction: "recv".into(), hex: hex_encode(&frame_data) });

            let parsed = match parse(&frame_data, 0, 0, 0, 0) {
                Ok(f) => f,
                Err(e) => { warn!("解析主站帧失败: {e}"); continue; }
            };

            match parsed {
                Frame::Command(cmd) => {
                    emit_event(&evt, SubEvent::CommandReceived { cmd: cmd.cmd, name: cmd_name(cmd.cmd) });
                    Self::handle_command(
                        cmd.cmd, &writer, &settings, &gen, &trigger,
                        &evt, &data_writer, &stream_task,
                    ).await;
                }
                Frame::Config(cfg) => {
                    // 主站下传 CFG-2 配置帧 → 先按 §6 校验上送周期合法性，
                    // 再按 §8.6 回 ACK / NACK。
                    match cfg.illegal_period_reason() {
                        Some(reason) => {
                            // 非法上送周期（如 PERIOD=0）：回 NACK 拒绝，保持原 fps 不变。
                            emit_event(&evt, SubEvent::Cfg2Rejected { reason });
                            Self::send_cmd(&writer, &settings, &evt, Cmd::Nack as u16).await;
                        }
                        None => {
                            emit_event(&evt, SubEvent::Cfg2Received);
                            Self::send_cmd(&writer, &settings, &evt, Cmd::Ack as u16).await;
                        }
                    }
                }
                Frame::Data(_) => { /* 子站不应在管理管道收数据帧 */ }
            }
        }
        // 清理
        if let Some(h) = stream_task.lock().await.take() { h.abort(); }
        *data_writer.lock().await = None;
        emit_event(&evt, SubEvent::MasterDisconnected { peer_ip });
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_command(
        cmd: u16,
        writer: &Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
        settings: &Arc<RwLock<SubSettings>>,
        gen: &Arc<RwLock<DataGen>>,
        trigger: &Arc<std::sync::atomic::AtomicBool>,
        evt: &EventSender,
        data_writer: &Arc<Mutex<Option<tokio::net::tcp::OwnedWriteHalf>>>,
        stream_task: &Arc<Mutex<Option<JoinHandle<()>>>>,
    ) {
        match cmd {
            c if c == Cmd::SendCfg1 as u16 => {
                Self::send_config(writer, settings, evt, FrameType::Cfg1 as u8, SubEvent::Cfg1Sent).await;
            }
            c if c == Cmd::SendCfg2 as u16 => {
                Self::send_config(writer, settings, evt, FrameType::Cfg2 as u8, SubEvent::Cfg2Sent).await;
            }
            c if c == Cmd::SendCfg2Cmd as u16 => {
                // 主站「下传 CFG-2 命令」通知 → 回 ACK（V3 §8.4）。
                Self::send_cmd(writer, settings, evt, Cmd::Ack as u16).await;
            }
            c if c == Cmd::OpenData as u16 => {
                Self::start_stream(settings, gen, trigger, evt, data_writer, stream_task).await;
                emit_event(evt, SubEvent::StreamingStarted);
            }
            c if c == Cmd::CloseData as u16 => {
                if let Some(h) = stream_task.lock().await.take() { h.abort(); }
                emit_event(evt, SubEvent::StreamingStopped);
            }
            c if c == Cmd::Heartbeat as u16 => {
                Self::send_cmd(writer, settings, evt, Cmd::Heartbeat as u16).await;
            }
            c if c == Cmd::Trigger as u16 => {
                trigger.store(true, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    /// 启动数据推流任务。V3：用已 accept 的 data_writer；V2：先连出主站数据口。
    async fn start_stream(
        settings: &Arc<RwLock<SubSettings>>,
        gen: &Arc<RwLock<DataGen>>,
        trigger: &Arc<std::sync::atomic::AtomicBool>,
        evt: &EventSender,
        data_writer: &Arc<Mutex<Option<tokio::net::tcp::OwnedWriteHalf>>>,
        stream_task: &Arc<Mutex<Option<JoinHandle<()>>>>,
    ) {
        if let Some(h) = stream_task.lock().await.take() { h.abort(); }

        let (version, period_ms, base_soc) = {
            let s = settings.read().await;
            let fps = s.config.data_rate_fps.max(1);
            (s.version, (1000.0 / fps as f64) as u64, current_soc())
        };

        // V2：数据管道子站作客户端，连主站数据口。
        // 限制：目标 IP 硬编码为 127.0.0.1，仅支持本地互测。
        // 若需连接远端主站，需在 mgmt accept 时捕获对端 IP 并传入此处。
        if version == ProtocolVersion::V2 {
            let port = { settings.read().await.data_port };
            let target = ("127.0.0.1", if port == 0 { 7001 } else { port });
            match tokio::time::timeout(std::time::Duration::from_secs(5), TcpStream::connect(target)).await {
                Ok(Ok(stream)) => {
                    let (_r, w) = stream.into_split();
                    *data_writer.lock().await = Some(w);
                }
                _ => {
                    emit_event(evt, SubEvent::Error { error: format!("V2 数据连出 {target:?} 失败") });
                    return;
                }
            }
        }

        let settings = settings.clone();
        let gen = gen.clone();
        let trigger = trigger.clone();
        let evt = evt.clone();
        let dw = data_writer.clone();
        let handle = tokio::spawn(async move {
            let mut frame_index: u64 = 0;
            let mut ticker = tokio::time::interval(std::time::Duration::from_millis(period_ms.max(1)));
            loop {
                ticker.tick().await;
                // 写端未就绪(数据管道尚未连入/已断)→ 跳过本拍,不消费 trigger/frame_index,
                // 避免在连接窗口内丢掉一次触发。快速取放锁,不跨 settings/gen 读持有。
                if dw.lock().await.is_none() {
                    continue;
                }
                let cfg = { settings.read().await.config.clone() };
                let g = { *gen.read().await };
                let trig = trigger.swap(false, Ordering::Relaxed);
                let df = next_data_frame(&cfg, &g, base_soc, frame_index, trig);
                let bytes = match build_data(&df, 0, 0, 0) {
                    Ok(b) => b,
                    Err(e) => { error!("build_data 失败: {e}"); continue; }
                };
                let mut guard = dw.lock().await;
                // 取到写帧锁后再次确认(上一步释放后可能断开);此处丢帧已消费 trigger,属罕见断开竞态。
                let Some(w) = guard.as_mut() else { continue; };
                if let Err(e) = w.write_all(&bytes).await {
                    emit_event(&evt, SubEvent::Error { error: format!("数据发送失败,推流停止: {e}") });
                    emit_event(&evt, SubEvent::StreamingStopped);
                    break;
                }
                let _ = w.flush().await;
                drop(guard);
                emit_event(&evt, SubEvent::DataFrameSent { data: data_frame_to_info(&df) });
                frame_index += 1;
            }
        });
        *stream_task.lock().await = Some(handle);
    }

    /// 按当前配置构建并发送 CFG-1/CFG-2。
    async fn send_config(
        writer: &Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
        settings: &Arc<RwLock<SubSettings>>,
        evt: &EventSender,
        cfg_type: u8,
        sent_event: SubEvent,
    ) {
        let cfg = { build_config_frame(&settings.read().await.config, cfg_type) };
        let raw = match build_config(&cfg) {
            Ok(r) => r,
            Err(e) => { emit_event(evt, SubEvent::Error { error: format!("build_config 失败: {e}") }); return; }
        };
        let mut w = writer.lock().await;
        if w.write_all(&raw).await.is_ok() {
            let _ = w.flush().await;
            drop(w);
            emit_event(evt, SubEvent::RawFrame { direction: "send".into(), hex: hex_encode(&raw) });
            emit_event(evt, sent_event);
        }
    }

    /// 发送一条命令帧（ACK/心跳等）。
    async fn send_cmd(
        writer: &Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
        settings: &Arc<RwLock<SubSettings>>,
        evt: &EventSender,
        cmd: u16,
    ) {
        let (version, idcode) = {
            let s = settings.read().await;
            (s.version, s.config.idcode.clone())
        };
        let frame = CommandFrame { version, idcode, soc: current_soc(), fracsec: 0, cmd };
        let raw = match build_command(&frame) {
            Ok(r) => r,
            Err(e) => { error!("build_command 失败: {e}"); return; }
        };
        let mut w = writer.lock().await;
        if w.write_all(&raw).await.is_ok() {
            let _ = w.flush().await;
            drop(w);
            emit_event(evt, SubEvent::RawFrame { direction: "send".into(), hex: hex_encode(&raw) });
        }
    }
}

/// 从 SubConfig 构建一个 ConfigFrame（CFG-1 或 CFG-2）。
fn build_config_frame(c: &SubConfig, cfg_type: u8) -> ConfigFrame {
    let phnmr = c.phasors.len() as u16;
    let annmr = c.analogs.len() as u16;
    let dgnmr = c.digitals.len() as u16;

    // 通道名顺序：相量名 + 模拟量名 + 16×数字量名（与 parser 期望一致）。
    let mut channel_names: Vec<String> = Vec::new();
    for i in 0..phnmr { channel_names.push(format!("PH{i:02}")); }
    for i in 0..annmr { channel_names.push(format!("AN{i:02}")); }
    for i in 0..(dgnmr * 16) { channel_names.push(format!("D{i:02}")); }

    let phunit = vec![0x0000_0001u32; phnmr as usize];
    let anunit = vec![0x0000_0064u32; annmr as usize];
    let digunit = vec![(0x0001u16, 0x0000u16); dgnmr as usize];

    let fnom: u16 = 0x0001;
    let period_ms = 1000.0 / c.data_rate_fps.max(1) as f64;
    let period = (period_ms * 5.0).round() as u16;

    ConfigFrame {
        version: c.version,
        cfg_type,
        idcode: c.idcode.clone(),
        soc: current_soc(),
        fracsec: 0,
        d_frame: 0,
        meas_rate: c.meas_rate,
        num_pmu: 1,
        stn: c.stn.clone(),
        pmu_idcode: c.idcode.clone(),
        format_flags: c.format_flags,
        phnmr, annmr, dgnmr,
        channel_names,
        phunit, anunit, digunit,
        fnom,
        period,
        pmu_blocks: vec![PmuBlock {
            stn: c.stn.clone(),
            pmu_idcode: c.idcode.clone(),
            format_flags: c.format_flags,
            phnmr, annmr, dgnmr,
            channel_names: {
                let mut v = Vec::new();
                for i in 0..phnmr { v.push(format!("PH{i:02}")); }
                for i in 0..annmr { v.push(format!("AN{i:02}")); }
                for i in 0..(dgnmr * 16) { v.push(format!("D{i:02}")); }
                v
            },
            phunit: vec![0x0000_0001u32; phnmr as usize],
            anunit: vec![0x0000_0064u32; annmr as usize],
            digunit: vec![(0x0001u16, 0x0000u16); dgnmr as usize],
            fnom,
            period,
        }],
    }
}

fn cmd_name(cmd: u16) -> String {
    match cmd {
        0x0001 => "关闭数据", 0x0002 => "打开数据", 0x0004 => "召唤CFG-1",
        0x0005 => "召唤CFG-2", 0x4000 => "心跳", 0x8000 => "下传CFG-2命令",
        0xA000 => "触发", _ => "其他",
    }.to_string()
}

async fn read_frame(reader: &mut OwnedReadHalf) -> Result<Vec<u8>, String> {
    let mut header = [0u8; 4];
    reader.read_exact(&mut header).await.map_err(|e| format!("read header: {e}"))?;
    if header[0] != SYNC_BYTE {
        return Err(format!("Invalid sync byte: {:#04x}", header[0]));
    }
    let frame_size = u16::from_be_bytes([header[2], header[3]]) as usize;
    if frame_size < 4 {
        return Err(format!("Invalid frame size: {frame_size}"));
    }
    let mut buf = vec![0u8; frame_size];
    buf[..4].copy_from_slice(&header);
    reader.read_exact(&mut buf[4..]).await.map_err(|e| format!("read body: {e}"))?;
    Ok(buf)
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

fn data_frame_to_info(df: &pmusim_core::protocol::frame::DataFrame) -> DataInfo {
    DataInfo {
        soc: df.soc,
        fracsec: df.fracsec,
        stat: df.stat,
        freq: df.freq,
        dfreq: df.dfreq,
        phasors: df.phasors.clone(),
        analog: df.analog.clone(),
        digital: df.digital.clone(),
    }
}

fn emit_event(tx: &EventSender, ev: SubEvent) {
    if let Err(e) = tx.send(ev) { error!("emit_event 失败: {e}"); }
}
