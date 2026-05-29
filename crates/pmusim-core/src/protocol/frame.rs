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

/// IEEE C37.118 / V3 §8.5 ANUNIT high-byte interpretation. The low 24
/// bits are the user-scaling multiplier; the high byte tags the
/// physical interpretation. Treating the whole u32 as a factor (which
/// we did before) produces a ~16-million-fold blowup for any RMS- or
/// peak-tagged channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalogType {
    SinglePoint = 0,
    Rms = 1,
    Peak = 2,
    Unknown = 0xFF,
}

impl AnalogType {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0 => Self::SinglePoint,
            1 => Self::Rms,
            2 => Self::Peak,
            _ => Self::Unknown,
        }
    }
}

impl ConfigFrame {
    pub fn period_ms(&self) -> f64 {
        let base_freq: f64 = if self.fnom & 1 != 0 { 50.0 } else { 60.0 };
        (self.period as f64 / 100.0) * (1000.0 / base_freq)
    }

    /// Scaling factor for analog channel `index`. Reads the low 24 bits
    /// of ANUNIT as a signed integer and applies the spec multiplier
    /// 0.00001. The high byte (analog type code) is masked off so an
    /// RMS-tagged (0x01...) or peak-tagged channel does not get
    /// interpreted as a factor of ~16.7M.
    pub fn analog_factor(&self, index: usize) -> f64 {
        let raw = self.anunit[index];
        // Sign-extend the 24-bit signed integer in the low 3 bytes.
        let low24 = raw & 0x00FFFFFF;
        let signed: i32 = if low24 & 0x800000 != 0 {
            (low24 | 0xFF000000) as i32
        } else {
            low24 as i32
        };
        (signed as f64) * 0.00001
    }

    /// Analog-type tag (ANUNIT high byte) for channel `index`. Useful
    /// for UI display (e.g. "rms" vs "peak").
    pub fn analog_type(&self, index: usize) -> AnalogType {
        AnalogType::from_byte((self.anunit[index] >> 24) as u8)
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
