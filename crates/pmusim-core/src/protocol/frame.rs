use super::constants::ProtocolVersion;

#[derive(Debug, Clone)]
pub struct CommandFrame {
    pub version: ProtocolVersion,
    pub idcode: String,
    pub soc: u32,
    pub fracsec: u32,
    pub cmd: u16,
}

/// One PMU's worth of CFG fields. A V3 config frame can carry NUM_PMU of
/// these per spec §8.2 — earlier code only ever decoded the first block,
/// so any substation reporting NUM_PMU > 1 silently lost channels 2..n.
#[derive(Debug, Clone)]
pub struct PmuBlock {
    pub stn: String,
    pub pmu_idcode: String,
    pub format_flags: u16,
    pub phnmr: u16,
    pub annmr: u16,
    pub dgnmr: u16,
    pub channel_names: Vec<String>,
    pub phunit: Vec<u32>,
    pub anunit: Vec<u32>,
    pub digunit: Vec<(u16, u16)>,
    pub fnom: u16,
    pub period: u16,
}

#[derive(Debug, Clone)]
pub struct ConfigFrame {
    pub version: ProtocolVersion,
    pub cfg_type: u8,
    pub idcode: String,
    pub soc: u32,
    pub fracsec: u32,
    pub d_frame: u16,
    pub meas_rate: u32,
    pub num_pmu: u16,
    /// First-PMU convenience copies. Kept in sync with `pmu_blocks[0]`
    /// by the parser so existing consumers (Tauri ConfigInfo, frontend
    /// table) work unchanged. To handle multi-PMU substations, read
    /// `pmu_blocks` directly.
    pub stn: String,
    pub pmu_idcode: String,
    pub format_flags: u16,
    pub phnmr: u16,
    pub annmr: u16,
    pub dgnmr: u16,
    pub channel_names: Vec<String>,
    pub phunit: Vec<u32>,
    pub anunit: Vec<u32>,
    pub digunit: Vec<(u16, u16)>,
    pub fnom: u16,
    pub period: u16,
    /// All PMU blocks in this CFG frame, indexed by PMU position. Length
    /// equals `num_pmu` after parsing. For building, an empty vec falls
    /// back to constructing one block from the convenience fields.
    pub pmu_blocks: Vec<PmuBlock>,
}

impl ConfigFrame {
    pub fn period_ms(&self) -> f64 {
        let base_freq: f64 = if self.fnom & 1 != 0 { 50.0 } else { 60.0 };
        (self.period as f64 / 100.0) * (1000.0 / base_freq)
    }

    pub fn analog_factor(&self, index: usize) -> f64 {
        self.anunit[index] as f64 * 0.00001
    }
}

/// Decoded data frame. Numeric fields are stored as `f64` regardless of
/// on-wire representation (int16 vs IEEE-754 float per FORMAT flags
/// §8.5 表 8) so consumers don't have to branch. `format_flags` is
/// carried so `build_data` can round-trip without an extra arg.
///
/// Phasor pair semantics depend on FORMAT bit0:
///   0 = rectangular (real, imag)
///   1 = polar (magnitude, angle in radians ×10000 if int16 mode)
#[derive(Debug, Clone)]
pub struct DataFrame {
    pub version: ProtocolVersion,
    pub idcode: String,
    pub soc: u32,
    pub fracsec: u32,
    pub stat: u16,
    /// Carries FORMAT bits 0-3 so build_data knows how to encode.
    pub format_flags: u16,
    pub phasors: Vec<(f64, f64)>,
    pub freq: f64,
    pub dfreq: f64,
    pub analog: Vec<f64>,
    pub digital: Vec<u16>,
}

impl DataFrame {
    pub fn data_valid(&self) -> bool {
        (self.stat & 0x8000) == 0
    }

    pub fn sync_ok(&self) -> bool {
        (self.stat & 0x2000) == 0
    }

    /// FORMAT bit1: 0=int16 phasor (4 bytes/phasor), 1=float (8 bytes).
    pub fn phasors_are_float(format_flags: u16) -> bool { (format_flags & 0b0010) != 0 }
    /// FORMAT bit2: 0=int16 analog (2 bytes), 1=float (4 bytes).
    pub fn analog_is_float(format_flags: u16) -> bool { (format_flags & 0b0100) != 0 }
    /// FORMAT bit3: 0=int16 freq/dfreq (2 bytes each), 1=float (4 bytes).
    pub fn freq_is_float(format_flags: u16) -> bool { (format_flags & 0b1000) != 0 }
    /// FORMAT bit0: 0=rectangular (real,imag), 1=polar (mag,angle).
    pub fn phasors_are_polar(format_flags: u16) -> bool { (format_flags & 0b0001) != 0 }
}

#[derive(Debug, Clone)]
pub enum Frame {
    Command(CommandFrame),
    Config(ConfigFrame),
    Data(DataFrame),
}
