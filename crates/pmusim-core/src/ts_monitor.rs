//! 数据帧时间戳错乱检测。
//!
//! 主站接收侧逐帧喂入数据帧自带的 SOC + FRACSEC，换算成绝对毫秒后与
//! 上一帧比较，判断是否按"当前预期间隔"(由 CFG-2 PERIOD 反推)正常
//! 递增。回退 / 跳变(丢帧) / 停滞(含重复时间戳)即报，由调用方把异常
//! 报文曝给前端。纯逻辑、无 IO、无时钟，便于单测。

use crate::time_utils::fracsec_to_ms;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsAnomalyKind {
    /// 时间戳回退(delta < 0)：子站重启 / SOC 回绕 / 对端算错。
    Backward,
    /// 跳变 / 丢帧(delta 明显大于预期间隔)。
    Gap,
    /// 偏快 / 停滞(delta 明显小于预期间隔，含重复时间戳 delta≈0)。
    Stall,
}

impl TsAnomalyKind {
    /// 中文标签，用于告警消息。
    pub fn label(self) -> &'static str {
        match self {
            TsAnomalyKind::Backward => "回退",
            TsAnomalyKind::Gap => "跳变",
            TsAnomalyKind::Stall => "停滞",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TsReport {
    pub kind: TsAnomalyKind,
    /// 当前预期间隔(ms)。
    pub expected_ms: f64,
    /// 实际相邻帧间隔(ms)，回退时为负。
    pub actual_ms: f64,
    /// 触发异常的本帧 SOC(供定位)。
    pub soc: u32,
    /// 触发异常的本帧 FRACSEC(供定位)。
    pub fracsec: u32,
}

/// 单个数据连接(一个接收 task)持有一个实例，逐帧 `feed`。
#[derive(Debug, Default)]
pub struct TimestampMonitor {
    /// 上一帧的绝对毫秒戳(soc*1000 + 亚秒毫秒)；None 表示尚无基准。
    last_ms: Option<f64>,
    /// 上一次判定用的预期间隔；用于识别速率切换。None 表示尚未喂过。
    last_expected: Option<f64>,
}

impl TimestampMonitor {
    pub fn new() -> Self {
        Self {
            last_ms: None,
            last_expected: None,
        }
    }

    /// 喂入一帧。返回 `Some` 表示该帧时间戳错乱。
    ///
    /// - `expected_ms`：当前预期间隔，由 `ConfigFrame::period_ms()` 给出；
    ///   `<= 0`(PERIOD=0 受控注入)时跳过判定，仅更新基准。
    /// - 首帧只记基准、不报。
    /// - 每帧都把基准更新为本帧，异常只报一次、不连环误报。
    pub fn feed(
        &mut self,
        soc: u32,
        fracsec: u32,
        version: u8,
        meas_rate: u32,
        expected_ms: f64,
    ) -> Option<TsReport> {
        let cur_ms = soc as f64 * 1000.0 + fracsec_to_ms(fracsec, meas_rate, version);
        let prev = self.last_ms.replace(cur_ms);
        // 识别速率切换：本帧 expected 与上次不同 → 旧基准不可比。
        let expected_changed = self.last_expected != Some(expected_ms);
        self.last_expected = Some(expected_ms);

        // 预期间隔无效(PERIOD=0 受控注入)：只记基准，不判定。
        if expected_ms <= 0.0 {
            return None;
        }
        // 首帧：只记基准。
        let prev = prev?;
        // 速率切换的过渡帧：基准来自旧节拍，本帧只作新基准、不判定，
        // 避免把"正在按旧间隔正常递增"的过渡帧误报。
        if expected_changed {
            return None;
        }

        let delta = cur_ms - prev;
        // 容差取半个周期(纯比例)：高数据率(expected≤2ms)下不被绝对下限
        // 吞掉整个周期，低速率下也随周期放宽。
        let tol = expected_ms * 0.5;

        let kind = if delta < 0.0 {
            // 任何负 delta 都是时间戳未递增——回退优先于停滞判定，
            // 与 TsAnomalyKind::Backward 的语义(delta<0)一致。
            TsAnomalyKind::Backward
        } else if delta > expected_ms + tol {
            TsAnomalyKind::Gap
        } else if delta < expected_ms - tol {
            // 此处 delta>=0：偏快/停滞(含重复时间戳 delta=0)。
            TsAnomalyKind::Stall
        } else {
            return None;
        };

        Some(TsReport {
            kind,
            expected_ms,
            actual_ms: delta,
            soc,
            fracsec,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 1_000_000; // TIME_BASE 1µs
    const P50: f64 = 20.0; // 50Hz → 20ms

    /// 把"秒内毫秒"编码成 V2 fracsec(无质量位)。
    fn frac_v2(ms: u32) -> u32 {
        ms * (RATE / 1000)
    }
    /// V3：低 24 位计数 + 高位质量码。
    fn frac_v3(ms: u32, q: u8) -> u32 {
        (ms * (RATE / 1000)) | ((q as u32) << 24)
    }

    #[test]
    fn first_frame_is_baseline_no_report() {
        let mut m = TimestampMonitor::new();
        assert!(m.feed(100, 0, 2, RATE, P50).is_none());
    }

    #[test]
    fn normal_increasing_no_report() {
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v2(0), 2, RATE, P50);
        assert!(m.feed(100, frac_v2(20), 2, RATE, P50).is_none());
        assert!(m.feed(100, frac_v2(40), 2, RATE, P50).is_none());
        assert!(m.feed(100, frac_v2(60), 2, RATE, P50).is_none());
    }

    #[test]
    fn backward_timestamp_reports_backward() {
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v2(40), 2, RATE, P50);
        let r = m.feed(100, frac_v2(20), 2, RATE, P50).expect("应报回退");
        assert_eq!(r.kind, TsAnomalyKind::Backward);
        assert!((r.actual_ms - (-20.0)).abs() < 0.01);
    }

    #[test]
    fn dropped_frame_reports_gap() {
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v2(0), 2, RATE, P50);
        // 漏了 20ms 那帧，直接跳到 40ms。
        let r = m.feed(100, frac_v2(40), 2, RATE, P50).expect("应报跳变");
        assert_eq!(r.kind, TsAnomalyKind::Gap);
        assert!((r.actual_ms - 40.0).abs() < 0.01);
        assert!((r.expected_ms - 20.0).abs() < 0.01);
    }

    #[test]
    fn duplicate_timestamp_reports_stall() {
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v2(20), 2, RATE, P50);
        let r = m.feed(100, frac_v2(20), 2, RATE, P50).expect("应报停滞");
        assert_eq!(r.kind, TsAnomalyKind::Stall);
        assert!((r.actual_ms - 0.0).abs() < 0.01);
    }

    #[test]
    fn within_tolerance_no_report() {
        // tol = 20*0.5 = 10ms；delta=25 → |25-20|=5 < 10 → 不报。
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v2(0), 2, RATE, P50);
        assert!(m.feed(100, frac_v2(25), 2, RATE, P50).is_none());
    }

    #[test]
    fn tolerance_boundary_not_reported() {
        // delta = expected + tol = 30ms 恰好边界，用 `>` 判定 → 不报。
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v2(0), 2, RATE, P50);
        assert!(m.feed(100, frac_v2(30), 2, RATE, P50).is_none());
    }

    #[test]
    fn just_over_tolerance_reports_gap() {
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v2(0), 2, RATE, P50);
        assert!(m.feed(100, frac_v2(31), 2, RATE, P50).is_some());
    }

    #[test]
    fn zero_hz_injection_skips_detection() {
        // expected_ms=0(PERIOD=0 受控注入)：任何跳变都不报。
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v2(0), 2, RATE, 0.0);
        assert!(m.feed(200, frac_v2(500), 2, RATE, 0.0).is_none());
    }

    #[test]
    fn second_rollover_no_false_positive() {
        // 980ms → 下一秒 0ms，delta=20ms 正常，不应误报。
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v2(980), 2, RATE, P50);
        assert!(m.feed(101, frac_v2(0), 2, RATE, P50).is_none());
    }

    #[test]
    fn v3_quality_bits_masked_no_false_positive() {
        // V3 高 8 位质量码不应影响间隔判定。
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v3(0, 0xF), 3, RATE, P50);
        assert!(m.feed(100, frac_v3(20, 0xF), 3, RATE, P50).is_none());
    }

    #[test]
    fn report_carries_offending_timestamp() {
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v2(0), 2, RATE, P50);
        let r = m.feed(777, frac_v2(40), 2, RATE, P50).expect("应报异常");
        assert_eq!(r.soc, 777);
        assert_eq!(r.fracsec, frac_v2(40));
    }

    #[test]
    fn v2_quality_bits_masked_no_false_positive() {
        // V2 FRACSEC 高 8 位同样是时标质量码(与 V3 同布局)，须屏蔽。
        // 否则 GPS 失锁瞬间质量位翻转会让 cur_ms 暴增，把真实仍 20ms
        // 递增的报文误判为跳变。第一帧 q=0、第二帧 q=0xF。
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v2(0), 2, RATE, P50);
        assert!(
            m.feed(100, frac_v2(20) | (0xF << 24), 2, RATE, P50).is_none(),
            "V2 质量位翻转不应误判为时间戳错乱"
        );
    }

    #[test]
    fn high_rate_500fps_detects_stall_and_gap() {
        // expected=2ms(500fps)。容差若有 2ms 绝对下限会吞掉整个周期：
        // 重复戳 delta=0、丢帧 delta=4 都漏报。纯比例容差(tol=1ms)修复。
        let p500 = 2.0;
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v2(0), 2, RATE, p500);
        let stall = m
            .feed(100, frac_v2(0), 2, RATE, p500)
            .expect("500fps 重复时间戳应报停滞");
        assert_eq!(stall.kind, TsAnomalyKind::Stall);

        let mut m2 = TimestampMonitor::new();
        m2.feed(100, frac_v2(0), 2, RATE, p500);
        let gap = m2
            .feed(100, frac_v2(4), 2, RATE, p500)
            .expect("500fps 丢一帧应报跳变");
        assert_eq!(gap.kind, TsAnomalyKind::Gap);
    }

    #[test]
    fn rate_change_skips_transition_frame() {
        // 50Hz(20ms)→10Hz(100ms)。切换瞬间过渡帧仍按旧节拍(delta≈20ms)
        // 到达，但 expected 已变 100ms——不应误报，本帧只作新基准。
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v2(0), 2, RATE, 20.0);
        assert!(m.feed(100, frac_v2(20), 2, RATE, 20.0).is_none());
        // 改速率：expected 变 100ms，过渡帧 delta 仍 20ms → 跳过不报。
        assert!(
            m.feed(100, frac_v2(40), 2, RATE, 100.0).is_none(),
            "速率切换过渡帧不应误报"
        );
        // 之后按新节拍 100ms 正常递增 → 不报。
        assert!(m.feed(100, frac_v2(140), 2, RATE, 100.0).is_none());
        // 新速率下真实丢一帧(应 100ms,实到 200ms) → 报跳变。
        let gap = m
            .feed(100, frac_v2(340), 2, RATE, 100.0)
            .expect("新速率下丢帧应报跳变");
        assert_eq!(gap.kind, TsAnomalyKind::Gap);
    }

    #[test]
    fn small_backward_reports_backward() {
        // 不足半周期的小幅回退(-tol<delta<0)仍是回退，不应误标为停滞——
        // 任何 delta<0 都是时间戳未递增。
        let mut m = TimestampMonitor::new();
        m.feed(100, frac_v2(40), 2, RATE, P50);
        let r = m
            .feed(100, frac_v2(35), 2, RATE, P50)
            .expect("小幅回退应报回退");
        assert_eq!(r.kind, TsAnomalyKind::Backward);
        assert!((r.actual_ms - (-5.0)).abs() < 0.01);
    }
}
