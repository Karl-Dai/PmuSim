use std::sync::Arc;

use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;

use pmusim_core::protocol::constants::ProtocolVersion;

use crate::datagen::{DataGen, SubConfig};
use crate::events::SubEvent;

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
        todo!("Task 7")
    }

    pub async fn stop(&mut self) {
        todo!("Task 7")
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
}
