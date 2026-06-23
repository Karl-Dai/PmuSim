# 数据帧时间戳错乱检测 — 设计

## 背景与目标

主站(pmusim-app)接收子站/真实网关发来的数据帧时，若报文自带的时间戳(SOC+FRACSEC 换算的绝对毫秒)不按当前数据速率对应的间隔正常递增（回退、跳变/丢帧、停滞），把这条**异常报文曝出来**（前端 toast 告警 + 事件日志），便于联调排查对端时钟问题。

仅在**主站接收侧**检测。复用现有 `PmuEvent::Error` 通道，**前端零改动**（`usePmuEvents.ts` 已把 Error 映射为 toast + 事件日志）。

## 检测组件 `TimestampMonitor`

新文件 `crates/pmusim-core/src/ts_monitor.rs`，纯逻辑、无 IO、无时钟，便于单测。每个接收 task 持有一个实例，逐帧 `feed`。

```rust
pub enum TsAnomalyKind { Backward, Gap, Stall }

pub struct TsReport {
    pub kind: TsAnomalyKind,
    pub expected_ms: f64,    // 当前预期间隔
    pub actual_ms: f64,      // 实际相邻帧间隔 delta
    pub soc: u32,
    pub fracsec: u32,
}

pub struct TimestampMonitor {
    last_ms: Option<f64>,       // 上一帧绝对毫秒基准
    last_expected: Option<f64>, // 上次预期间隔，用于识别速率切换
}

impl TimestampMonitor {
    pub fn new() -> Self;
    /// 返回 Some(报告) 表示本帧时间戳错乱。
    pub fn feed(&mut self, soc: u32, fracsec: u32, version: u8,
                meas_rate: u32, expected_ms: f64) -> Option<TsReport>;
}
```

`feed` 逻辑：
1. `cur_ms = soc*1000 + fracsec_to_ms(fracsec, meas_rate, version)`。`fracsec_to_ms` **无条件**屏蔽 FRACSEC 高 8 位时标质量字节（C37.118.2 的 V2/V3 布局一致，与前端 `rate.ts::frameTimeMs` 对齐），避免对端质量位翻转使 cur_ms 暴增而误报。
2. 取出旧基准 `prev`，无条件把 `last_ms` 更新为 `cur_ms`；记 `expected_changed = last_expected != Some(expected_ms)`，再更新 `last_expected`。
3. **0Hz 注入**（`expected_ms <= 0`，CFG-2 `period=0` 受控注入）：返回 None。
4. **首帧**（`prev` 为 None）：返回 None。
5. **速率切换过渡帧**（`expected_changed`）：本帧只作新基准、返回 None——避免把按旧节拍正常递增的过渡帧误报。
6. `delta = cur_ms - prev`；`tol = expected_ms * 0.5`（纯比例，无绝对下限：否则高数据率 `expected≤2ms` 时下限会吞掉整个周期，使 Stall/Gap 漏报）。
7. 判定：
   - `delta < 0` → `Backward`（任何负 delta 即回退，优先于停滞）
   - `delta > expected_ms + tol` → `Gap`（跳变/丢帧）
   - `delta < expected_ms - tol` → `Stall`（此时 delta≥0；偏快/停滞，含重复时间戳 delta=0）
   - 否则 → None（正常递增）

无节流：每条异常都返回 Some，由调用方逐条 emit（按用户明确要求）。

## 告警出口

调用方拿到 `TsReport` 后 emit `PmuEvent::Error`，消息含可定位信息：

```
时间戳错乱[跳变]: 预期 20.0ms 实际 40.0ms | SOC=1781... (北京时间 2026-06-23 ...) FRACSEC=0x000d9490
```

`[跳变|回退|停滞]` 由 `kind` 决定；北京时间由 `time_utils::soc_to_beijing(soc)` 生成。

## 挂载点

两条接收路径，均在 `parse → Frame::Data(df)` 成功后、`emit DataFrame` 之前调用 `feed`：

- V2：`master.rs::handle_data_connection` 的首帧块 + 续读循环（共用一个 monitor，首帧作基准）。
- V3：`master.rs::data_read_loop_outbound` 续读循环。

预期间隔取自该 session 的 `cfg2.period_ms()`，`meas_rate` 取 `cfg2.meas_rate`，`version` 取 `df.version as u8`。

- 两条路径均**每帧**从 session 现取 `(period_ms, meas_rate)`（不快照）。理由：主站可在流式中下传新 CFG-2 实时改速率（`do_send_cfg2` 不拆数据管道），而子站改速率不保证置 STAT bit10——快照会长期失效，使 monitor 用旧 `expected_ms` 持续误报。read 锁在无写竞争时开销极小，与 V2 原有的每帧读模式一致。
- 每个数据连接 task 各持一个 `TimestampMonitor`。

## 测试

`ts_monitor.rs` 单元测试覆盖：
- 正常等间隔递增 → 不报
- 回退（delta<0）→ Backward；小幅回退（-tol<delta<0）也判 Backward 而非 Stall
- 跳变/丢帧（delta≈2×预期）→ Gap
- 停滞（重复时间戳 delta=0）→ Stall
- 容差边界（delta 在 ±tol 内）→ 不报
- 首帧 → 不报
- 0Hz（expected_ms=0）→ 不报
- 跨秒进位（fracsec 回 0、soc+1）→ 正常递增不误报
- V2/V3 fracsec 高 8 位质量码均屏蔽（含 V2 质量位翻转不误报）
- 高数据率（500fps, expected=2ms）→ 重复戳报 Stall、丢帧报 Gap
- 速率切换（expected 变化）→ 过渡帧跳过、新节拍下正常检测

## 不做（YAGNI）

- 不新增事件类型，不改前端，不 dump 整帧 hex。
- 不做子站生成侧自检。
- 不做节流。
