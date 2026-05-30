//! 子站数据生成器：把可配置正弦相量 + 静态模拟量/数字量编成一个
//! `DataFrame`。纯逻辑、无 IO、无 Tauri —— 由 `network::substation` 的
//! 推流循环按帧调用，也可独立单测。
//!
//! 同步相量约定：相量在「以标称频率旋转的参考系」中只按频率偏差 Δf
//! 旋转，θ(t)=θ0+2π·(Δf·t+½·ROCOF·t²)。Δf=0 时相量静止。

use std::f64::consts::PI;

use pmusim_core::protocol::constants::ProtocolVersion;
use pmusim_core::protocol::frame::DataFrame;
use pmusim_core::time_utils::fracsec_from_fraction;

/// 触发帧在 STAT 里置的标记位。避开主站会检查的 0x8000(数据无效)、
/// 0x2000(失步)、0x0400(配置变更) 位，选 bit11(0x0800) 作触发指示。
pub const TRIGGER_STAT_BIT: u16 = 0x0800;

/// 单个相量通道的生成参数（初始幅值/相角）。
#[derive(Debug, Clone)]
pub struct PhasorGen {
    /// 幅值（整数刻度，直接作为 int16 直角坐标的模；典型 0~32767）。
    pub magnitude: f64,
    /// 初始相角（度）。
    pub phase_deg: f64,
}

/// 子站静态配置 —— 与 CFG-2 申报保持一致的通道布局 + 速率 + 格式。
#[derive(Debug, Clone)]
pub struct SubConfig {
    pub version: ProtocolVersion,
    pub idcode: String,
    /// 站名（GBK）。由 network 层的 CFG 构建器写入 CFG-1/CFG-2，本模块不直接使用。
    pub stn: String,
    /// 帧率（帧/秒），由 CFG-2 period/fnom 推得（见 network 层）。
    pub data_rate_fps: u32,
    /// TIME_BASE（FRACSEC 分辨率，典型 1_000_000）。
    pub meas_rate: u32,
    /// FORMAT bits 0-3。本工具默认 0 = 直角坐标 int16。
    pub format_flags: u16,
    pub phasors: Vec<PhasorGen>,
    /// 模拟量定值（个数 = ANNMR）。
    pub analogs: Vec<f64>,
    /// 数字量定值（个数 = DGNMR，每个是 16 位掩码）。
    pub digitals: Vec<u16>,
}

/// 运行期可调的频率行为。
#[derive(Debug, Clone, Copy)]
pub struct DataGen {
    /// 频率偏差 Δf（Hz）。
    pub freq_offset_hz: f64,
    /// 频率变化率 ROCOF（Hz/s）。
    pub rocof_hz_s: f64,
}

/// 生成第 `frame_index` 帧（从 0 计）。`base_soc` 是推流开始时刻的 SOC。
pub fn next_data_frame(
    cfg: &SubConfig,
    gen: &DataGen,
    base_soc: u32,
    frame_index: u64,
    trigger: bool,
) -> DataFrame {
    let fps = cfg.data_rate_fps.max(1) as u64;
    let whole = (frame_index / fps) as u32;
    let sub = (frame_index % fps) as f64 / fps as f64;
    let soc = base_soc.wrapping_add(whole);
    let version_u8 = cfg.version as u8;
    let fracsec = fracsec_from_fraction(sub, cfg.meas_rate, version_u8, 0);

    // 自推流起的连续时间 t（秒），用于相量旋转。
    let t = frame_index as f64 / fps as f64;
    let two_pi = 2.0 * PI;
    let rot = two_pi * (gen.freq_offset_hz * t + 0.5 * gen.rocof_hz_s * t * t);

    let phasors: Vec<(f64, f64)> = cfg
        .phasors
        .iter()
        .map(|p| {
            let theta = p.phase_deg.to_radians() + rot;
            // 直角坐标：(real, imag) = (mag·cosθ, mag·sinθ)。build_data 在
            // format bit1=0 时按 i16 截断。
            (p.magnitude * theta.cos(), p.magnitude * theta.sin())
        })
        .collect();

    let stat = if trigger { TRIGGER_STAT_BIT } else { 0x0000 };

    DataFrame {
        version: cfg.version,
        idcode: cfg.idcode.clone(),
        soc,
        fracsec,
        stat,
        format_flags: cfg.format_flags,
        phasors,
        // FREQ 按 mHz 偏差上报，DFREQ 按 ROCOF·100；format bit3=0 时写 i16。
        freq: (gen.freq_offset_hz * 1000.0).round(),
        dfreq: (gen.rocof_hz_s * 100.0).round(),
        analog: cfg.analogs.clone(),
        digital: cfg.digitals.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmusim_core::protocol::constants::ProtocolVersion;
    use pmusim_core::protocol::frame::Frame;
    use pmusim_core::protocol::parser::parse;

    fn cfg() -> SubConfig {
        SubConfig {
            version: ProtocolVersion::V3,
            idcode: "TESTPMU0".into(),
            stn: "T".into(),
            data_rate_fps: 50,
            meas_rate: 1_000_000,
            format_flags: 0, // 直角坐标 int16
            phasors: vec![PhasorGen { magnitude: 1000.0, phase_deg: 0.0 }],
            analogs: vec![300.0, 3000.0],
            digitals: vec![0x000A],
        }
    }

    #[test]
    fn stationary_when_no_offset() {
        let g = DataGen { freq_offset_hz: 0.0, rocof_hz_s: 0.0 };
        let c = cfg();
        // Δf=0 → 相量不旋转：第 0 帧与第 50 帧相量一致
        let f0 = next_data_frame(&c, &g, 1000, 0, false);
        let f50 = next_data_frame(&c, &g, 1000, 50, false);
        assert_eq!(f0.phasors[0].0.round(), 1000.0); // real≈mag
        assert_eq!(f0.phasors[0].1.round(), 0.0);     // imag≈0
        assert_eq!(f50.phasors[0].0.round(), 1000.0);
    }

    #[test]
    fn rotates_with_offset() {
        // Δf=0.25Hz, fps=50 → 第 50 帧(=1s)转过 2π·0.25=90°
        let g = DataGen { freq_offset_hz: 0.25, rocof_hz_s: 0.0 };
        let c = cfg();
        let f = next_data_frame(&c, &g, 0, 50, false);
        // 90°：real≈0, imag≈+1000
        assert!(f.phasors[0].0.abs() < 1.0, "real={}", f.phasors[0].0);
        assert!((f.phasors[0].1 - 1000.0).abs() < 1.0, "imag={}", f.phasors[0].1);
    }

    #[test]
    fn soc_and_fracsec_advance() {
        let g = DataGen { freq_offset_hz: 0.0, rocof_hz_s: 0.0 };
        let c = cfg(); // fps=50
        // 第 75 帧 = 1 整秒 + 25/50 秒
        let f = next_data_frame(&c, &g, 1000, 75, false);
        assert_eq!(f.soc, 1001);
        // fraction=0.5 → count=500000
        assert_eq!(f.fracsec & 0x00FF_FFFF, 500_000);
    }

    #[test]
    fn frame_builds_and_parses() {
        let g = DataGen { freq_offset_hz: 0.1, rocof_hz_s: 0.0 };
        let c = cfg();
        let df = next_data_frame(&c, &g, 1000, 3, false);
        let bytes = pmusim_core::protocol::builder::build_data(&df, 0, 0, 0).unwrap();
        // 用 CFG-2 维度解析：phnmr=1, annmr=2, dgnmr=1
        let parsed = parse(&bytes, c.format_flags, 1, 2, 1).unwrap();
        match parsed {
            Frame::Data(d) => {
                assert_eq!(d.idcode, "TESTPMU0");
                assert_eq!(d.analog.len(), 2);
                assert_eq!(d.digital, vec![0x000A]);
                assert_eq!(d.phasors.len(), 1);
                assert_eq!(d.freq, 100.0, "Δf=0.1Hz 应编码为 100 mHz");
                assert!((d.phasors[0].0 - 999.3).abs() < 1.5, "real={}", d.phasors[0].0);
                assert!((d.phasors[0].1 - 37.7).abs() < 1.5, "imag={}", d.phasors[0].1);
            }
            _ => panic!("expected Data frame"),
        }
    }

    #[test]
    fn nonzero_phase_offset() {
        // phase_deg=90, Δf=0 → real≈0, imag≈+mag,与帧序号无关
        let g = DataGen { freq_offset_hz: 0.0, rocof_hz_s: 0.0 };
        let mut c = cfg();
        c.phasors = vec![PhasorGen { magnitude: 1000.0, phase_deg: 90.0 }];
        for &idx in &[0u64, 7, 123] {
            let f = next_data_frame(&c, &g, 0, idx, false);
            assert!(f.phasors[0].0.abs() < 1.0, "real={}", f.phasors[0].0);
            assert!((f.phasors[0].1 - 1000.0).abs() < 1.0, "imag={}", f.phasors[0].1);
        }
    }

    #[test]
    fn trigger_sets_stat_bit() {
        let g = DataGen { freq_offset_hz: 0.0, rocof_hz_s: 0.0 };
        let c = cfg();
        let normal = next_data_frame(&c, &g, 0, 0, false);
        let trig = next_data_frame(&c, &g, 0, 0, true);
        assert_eq!(normal.stat & TRIGGER_STAT_BIT, 0);
        assert_eq!(trig.stat & TRIGGER_STAT_BIT, TRIGGER_STAT_BIT);
        // 不得置「数据无效(0x8000)」或「失步(0x2000)」位
        assert_eq!(trig.stat & 0x8000, 0);
        assert_eq!(trig.stat & 0x2000, 0);
    }
}
