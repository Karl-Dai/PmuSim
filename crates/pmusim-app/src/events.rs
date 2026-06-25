use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum PmuEvent {
    SessionCreated { idcode: String, peer_ip: String },
    SessionDisconnected { idcode: String },
    Cfg1Received { idcode: String, cfg: ConfigInfo },
    Cfg2Sent { idcode: String },
    Cfg2Skipped { idcode: String },
    Cfg2Received { idcode: String, cfg: ConfigInfo },
    StreamingStarted { idcode: String },
    StreamingStopped { idcode: String },
    DataFrame { idcode: String, data: DataInfo },
    RawFrame { idcode: String, direction: String, hex: String },
    HeartbeatTimeout { idcode: String },
    Error { idcode: String, error: String },
}

// Match the TypeScript ConfigInfo type (camelCase). Without this rename,
// `cfg.channelNames` etc. are undefined on the frontend and the data table
// silently shows only the STAT rows even though CFG-2 arrived intact.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfo {
    pub cfg_type: u8,
    pub version: u8,
    pub stn: String,
    pub idcode: String,
    pub format_flags: u16,
    pub period: u16,
    pub meas_rate: u32,
    pub phnmr: u16,
    pub annmr: u16,
    pub dgnmr: u16,
    pub channel_names: Vec<String>,
    pub anunit: Vec<u32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DataInfo {
    pub soc: u32,
    pub fracsec: u32,
    pub stat: u16,
    /// FORMAT bits 0-3 from the matching CFG-2 — frontend can use bit0
    /// to decide phasor display (rectangular vs polar).
    pub format_flags: u16,
    /// FRACSEC bit27-24 = §8.11 表 4 GPS time-quality code. 0 = locked.
    pub time_quality: u8,
    pub freq: f64,
    pub dfreq: f64,
    pub analog: Vec<f64>,
    pub digital: Vec<u16>,
    pub phasors: Vec<(f64, f64)>,
    /// 接收时刻本机时钟与本帧报文时间戳之差(ms)：now − 报文时间。
    /// 正=报文滞后本地，负=报文超前本地。仅展示用，不参与编码。
    pub local_offset_ms: f64,
}

impl From<&pmusim_core::protocol::frame::ConfigFrame> for ConfigInfo {
    fn from(cfg: &pmusim_core::protocol::frame::ConfigFrame) -> Self {
        Self {
            cfg_type: cfg.cfg_type,
            version: cfg.version as u8,
            stn: cfg.stn.clone(),
            idcode: cfg.pmu_idcode.clone(),
            format_flags: cfg.format_flags,
            period: cfg.period,
            meas_rate: cfg.meas_rate,
            phnmr: cfg.phnmr,
            annmr: cfg.annmr,
            dgnmr: cfg.dgnmr,
            channel_names: cfg.channel_names.clone(),
            anunit: cfg.anunit.clone(),
        }
    }
}
