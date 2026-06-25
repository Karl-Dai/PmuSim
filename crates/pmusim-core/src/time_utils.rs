use std::time::{SystemTime, UNIX_EPOCH};

pub fn soc_to_beijing(soc: u32) -> String {
    let total_secs = soc as i64 + 8 * 3600;
    let days = total_secs.div_euclid(86400);
    let rem = total_secs.rem_euclid(86400) as u32;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let (y, mo, d) = days_to_date(days);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02}")
}

fn days_to_date(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + (era as i32) * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn fracsec_to_ms(fracsec: u32, meas_rate: u32, _version: u8) -> f64 {
    if meas_rate == 0 {
        return 0.0;
    }
    // FRACSEC bit23-0 是亚秒计数，bit31-24 是时标质量字节。C37.118.2 的
    // V2/V3 FRACSEC 布局一致(GB/T 26865.2 同),无论版本都须屏蔽高 8 位——
    // 否则真实对端置质量位/失锁翻转时会把质量码当计数,结果暴增导致误报。
    // 与前端 rate.ts::frameTimeMs 的无条件屏蔽保持一致。`_version` 仅为
    // 兼容既有调用方签名而保留。
    let count = fracsec & 0x00FF_FFFF;
    count as f64 / (meas_rate as f64 / 1000.0)
}

/// V3 §8.11 表 4 time-quality bits (bit27-24 of FRACSEC). Returns a
/// short Chinese label so the UI can surface GPS lock status alongside
/// the data frame.
pub fn fracsec_time_quality(fracsec: u32) -> (u8, &'static str) {
    let bits = ((fracsec >> 24) & 0x0F) as u8;
    let label = match bits {
        0b0000 => "时钟锁定",
        0b0001 => "失锁 <s",
        0b0010 => "失锁 <s",
        0b0011 => "失锁 <s",
        0b0100 => "失锁 <s",
        0b0101 => "失锁 <s",
        0b0110 => "失锁 <s",
        0b0111 => "失锁 <s",
        0b1000 => "失锁 <1s",
        0b1001 => "失锁 <0.1s",
        0b1010 => "失锁 <1s",
        0b1011 => "失锁 <10s",
        0b1100 => "保留",
        0b1101 => "保留",
        0b1110 => "保留",
        0b1111 => "时钟失效",
        _ => "未知",
    };
    (bits, label)
}

/// Encode a sub-second fraction (`0.0..1.0`) into a FRACSEC word — the
/// inverse of [`fracsec_to_ms`]. `count = round(fraction * meas_rate)`.
/// For V3 (`version >= 3`) the low 24 bits hold the count and bits 27-24
/// carry the time-quality nibble (§8.11 表 4, 0 = clock locked); V2 has
/// no quality bits so `time_quality` is ignored. Returns 0 when
/// `meas_rate == 0` (mirrors `fracsec_to_ms`'s guard).
pub fn fracsec_from_fraction(fraction: f64, meas_rate: u32, version: u8, time_quality: u8) -> u32 {
    if meas_rate == 0 {
        return 0;
    }
    let count = (fraction.clamp(0.0, 1.0) * meas_rate as f64).round() as u32;
    if version >= 3 {
        (count & 0x00FF_FFFF) | (((time_quality & 0x0F) as u32) << 24)
    } else {
        count
    }
}

/// 数据帧自带时间戳的绝对毫秒：SOC 秒 + FRACSEC 亚秒。FRACSEC 高 8 位
/// 时标质量码由 `fracsec_to_ms` 无条件屏蔽（见其文档），避免质量位翻转
/// 污染换算。`version` 仅为兼容签名保留。偏差测量与 `ts_monitor` 共用此
/// 定义，避免「帧绝对毫秒」散落两处。
pub fn frame_abs_ms(soc: u32, fracsec: u32, meas_rate: u32, version: u8) -> f64 {
    soc as f64 * 1000.0 + fracsec_to_ms(fracsec, meas_rate, version)
}

pub fn current_soc() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32
}

/// 本机墙钟相对 UNIX epoch 的毫秒数（f64，保留亚毫秒）。偏差测量的
/// 「本地时间」基准——受本机时钟是否校准影响，这正是要暴露的对象。
/// 时钟早于 epoch（不可能但防御）时返回 0。
pub fn now_unix_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soc_epoch_beijing() {
        assert_eq!(soc_to_beijing(0), "1970-01-01 08:00:00");
    }

    #[test]
    fn soc_2024_12_10() {
        assert_eq!(soc_to_beijing(0x6757DD1D), "2024-12-10 14:18:05");
    }

    #[test]
    fn soc_2025_02_17() {
        assert_eq!(soc_to_beijing(0x67B2C719), "2025-02-17 13:20:25");
    }

    #[test]
    fn fracsec_v2_890ms() {
        let ms = fracsec_to_ms(0x000D9490, 1_000_000, 2);
        assert!((ms - 890.0).abs() < 0.1, "expected ~890.0, got {ms}");
    }

    #[test]
    fn fracsec_zero() {
        let ms = fracsec_to_ms(0, 1_000_000, 3);
        assert!((ms - 0.0).abs() < 0.001);
    }

    #[test]
    fn fracsec_v3_quality_bits() {
        // Upper 8 bits are quality flags, lower 24 = 0x07A120 = 500000
        let ms = fracsec_to_ms(0x0F07A120, 1_000_000, 3);
        assert!((ms - 500.0).abs() < 0.1, "expected ~500.0, got {ms}");
    }

    #[test]
    fn fracsec_zero_meas_rate() {
        let ms = fracsec_to_ms(1000, 0, 3);
        assert!((ms - 0.0).abs() < 0.001);
    }

    #[test]
    fn fracsec_from_fraction_roundtrips_v2() {
        // 0.89s @ TIME_BASE=1_000_000 → count 890000；再过 fracsec_to_ms 还原
        let f = fracsec_from_fraction(0.89, 1_000_000, 2, 0);
        assert_eq!(f, 890_000);
        let ms = fracsec_to_ms(f, 1_000_000, 2);
        assert!((ms - 890.0).abs() < 0.1, "got {ms}");
    }

    #[test]
    fn fracsec_from_fraction_v3_packs_quality() {
        // V3：低 24 位是计数，bit27-24 是时间质量
        let f = fracsec_from_fraction(0.5, 1_000_000, 3, 0b1001);
        assert_eq!(f & 0x00FF_FFFF, 500_000);
        assert_eq!((f >> 24) & 0x0F, 0b1001);
        let ms = fracsec_to_ms(f, 1_000_000, 3);
        assert!((ms - 500.0).abs() < 0.1, "got {ms}");
    }

    #[test]
    fn fracsec_from_fraction_zero_rate() {
        assert_eq!(fracsec_from_fraction(0.5, 0, 3, 0), 0);
    }

    #[test]
    fn frame_abs_ms_seconds_plus_subsecond() {
        // soc=100s, fracsec=0 → 100_000ms；20ms 亚秒 → 100_020ms。
        assert!((frame_abs_ms(100, 0, 1_000_000, 2) - 100_000.0).abs() < 0.001);
        let frac20 = 20 * (1_000_000 / 1000); // 20ms @ TIME_BASE 1µs
        assert!((frame_abs_ms(100, frac20, 1_000_000, 2) - 100_020.0).abs() < 0.001);
    }

    #[test]
    fn frame_abs_ms_cross_second() {
        // 980ms 亚秒 → 100_980ms（验证 soc*1000 与亚秒相加）。
        let frac980 = 980 * (1_000_000 / 1000);
        assert!((frame_abs_ms(100, frac980, 1_000_000, 2) - 100_980.0).abs() < 0.001);
    }

    #[test]
    fn frame_abs_ms_masks_quality_bits() {
        // V2/V3 FRACSEC 高 8 位质量码须被屏蔽，不污染绝对毫秒。
        let frac = 20 * (1_000_000 / 1000) | (0x0F << 24);
        assert!((frame_abs_ms(100, frac, 1_000_000, 3) - 100_020.0).abs() < 0.001);
    }

    #[test]
    fn now_unix_ms_is_sane() {
        // 必为正且晚于 2023-01-01（1_672_531_200_000ms）。
        let now = now_unix_ms();
        assert!(now > 1_672_531_200_000.0, "got {now}");
    }
}
