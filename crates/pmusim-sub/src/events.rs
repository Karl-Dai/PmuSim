use serde::Serialize;

/// 子站事件，经 `poll_events` 推给前端（与主站一致的缓冲轮询模型）。
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum SubEvent {
    /// 子站已在管理端口开始监听。
    Listening { mgmt_port: u16, data_port: u16 },
    /// 主站建立了管理连接。
    MasterConnected { peer_ip: String },
    /// 主站断开。
    MasterDisconnected { peer_ip: String },
    /// 收到主站命令（cmd 为命令字）。
    CommandReceived { cmd: u16, name: String },
    /// 已上传 CFG-1 / CFG-2。
    Cfg1Sent,
    Cfg2Sent,
    /// 收到主站下传的 CFG-2 配置帧。
    Cfg2Received,
    /// 收到主站下传的 CFG-2 但上送周期非法 → 已回 NACK 拒绝（携原因）。
    Cfg2Rejected { reason: String },
    /// 数据推流开始/停止。
    StreamingStarted,
    StreamingStopped,
    /// 已发出一帧数据（携带预览信息）。
    DataFrameSent { data: DataInfo },
    /// 任意方向的原始帧（hex 按需）。
    RawFrame { direction: String, hex: String },
    Error { error: String },
}

/// 数据帧预览（驼峰命名以对齐前端 TS 类型）。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataInfo {
    pub soc: u32,
    pub fracsec: u32,
    pub stat: u16,
    pub freq: f64,
    pub dfreq: f64,
    pub phasors: Vec<(f64, f64)>,
    pub analog: Vec<f64>,
    pub digital: Vec<u16>,
}
