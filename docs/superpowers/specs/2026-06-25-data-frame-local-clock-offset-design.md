# 报文时间与本地时间偏差 — 设计

## 背景与目标

主站(pmusim-app)接收子站/真实网关发来的数据帧时，报文自带的时间戳
(SOC+FRACSEC 换算的绝对毫秒)是对端 PMU 按其 GPS 时钟打的。把这个**报文
时间**与**主站本机时钟**相减，得到的偏差反映「网络传输延迟 + 对端时钟相对
本机的偏差」。在读数区实时显示这个偏差，便于联调时一眼看出对端时钟是否同
本机/标准时间对齐、数据是否严重滞后或带未来戳。

与已有的「数据帧时间戳错乱检测」(`ts_monitor`，逐帧比较**帧间**间隔)正交：
那个看相邻帧是否按速率递增，这个看单帧相对**本地时钟**的绝对偏差。

仅主站接收侧。**仅实时读数，不告警**(不超阈 toast、不进数据表)。复用现有
`DataFrame` 事件搭车上送偏差，**不新增事件类型**。

## 符号约定

```
local_offset_ms = now_unix_ms − frame_abs_ms
```

- 正值 = 报文时间落后于本地(数据延迟 / 对端时钟慢)。
- 负值 = 报文时间超前于本地(对端时钟快 / 未来戳)。

## pmusim-core（纯逻辑 + 单测）

### `time_utils::frame_abs_ms`

把「帧绝对毫秒」的定义收敛到一处(此前内联在 `ts_monitor::feed` 里)：

```rust
/// 数据帧自带时间戳的绝对毫秒：SOC 秒 + FRACSEC 亚秒。FRACSEC 高 8 位
/// 时标质量码由 fracsec_to_ms 无条件屏蔽（见其文档），避免质量位翻转
/// 污染换算。`version` 仅为兼容签名保留。
pub fn frame_abs_ms(soc: u32, fracsec: u32, meas_rate: u32, version: u8) -> f64 {
    soc as f64 * 1000.0 + fracsec_to_ms(fracsec, meas_rate, version)
}
```

`ts_monitor::feed` 的 `cur_ms` 改调 `frame_abs_ms`(DRY，行为不变——既有
13 个单测须仍全绿，即为无回归验证)。

### `time_utils::now_unix_ms`

```rust
/// 本机墙钟相对 UNIX epoch 的毫秒数（f64，保留亚毫秒）。偏差测量的
/// 「本地时间」基准。受本机时钟是否校准影响——这正是要暴露的对象。
pub fn now_unix_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}
```

偏差本身是 `now_unix_ms() - frame_abs_ms(...)`，无需新结构体。

## pmusim-app（后端接入）

### `DataInfo` 加字段

`events.rs`：

```rust
pub struct DataInfo {
    // …既有字段…
    /// 接收时刻本机时钟与本帧报文时间戳之差(ms)：now − 报文时间。
    /// 正=报文滞后本地，负=报文超前本地。
    pub local_offset_ms: f64,
}
```

### 接收点采样

`data_frame_to_info` 改签名带入偏差，避免在内部再取一次时钟：

```rust
fn data_frame_to_info(df: &DataFrame, local_offset_ms: f64) -> DataInfo { … }
```

三处接收点在 `parse → Frame::Data(df)` 成功后、`emit DataFrame` 之前算偏差：

```rust
let offset = now_unix_ms() - frame_abs_ms(df.soc, df.fracsec, meas_rate, df.version as u8);
… PmuEvent::DataFrame { idcode, data: data_frame_to_info(&df, offset) }
```

- V2 首帧块(`handle_data_connection`)：`meas_rate` 取 `cfg2.meas_rate`。
- V2 续读循环：同上。
- V3 续读循环(`data_read_loop_outbound`)：`meas_rate` 取已现取的
  `(period_ms, meas_rate)` 中的 `meas_rate`(与时间戳检测同源，不另取锁)。

无 CFG / `meas_rate` 不可得的兜底路径(V3 `dims` 缺失时)：`frame_abs_ms`
在 `meas_rate=0` 时 `fracsec_to_ms` 返回 0，退化为 `soc*1000`，偏差仍可算，
不 panic。

## 前端

### 类型

`types/index.ts` 的 `DataInfo` 加 `local_offset_ms: number`。

### `useTimeOffset` composable（镜像 `useFrameRate`）

1s 滑窗均值，抹平逐帧网络抖动：

```ts
export function useTimeOffset() {
  function tick(offsetMs: number) { /* 推入带时间窗的样本，均值写 offsetMs */ }
  function reset() { /* 清空，offsetMs=null */ }
  return { offsetMs, tick, reset };
}
```

- `offsetMs` 为 `number | null`，`null` 表示尚无样本(显示「—」)。
- 平滑用**定长计数窗**:保留最近 `N=50` 帧偏差样本求均值。不依赖报文时间
  单调(偏差样本本身即可能因对端回退而跳变)，也无需墙钟——按到达顺序入队、
  超长丢队首即可。`reset()` 清空队列。

### `usePmuEvents` 接线

- `DataFrame` 分支：在既有 `tickFrameRate(...)` 旁加 `tickOffset(payload.data.local_offset_ms)`。
- `SessionDisconnected` / `StreamingStopped` / `HeartbeatTimeout`：在既有
  `resetFrameRate()` 旁加 `resetOffset()`。

### `ConfigInfoPanel` 读数区

在「上传速率」行下加一行：

```
本地时间偏差: +xx ms
```

- 带符号整数(`+12 ms` / `−1450 ms`)，无样本显示「—」，`mono` 等宽对齐。
- i18n 新增 `config.clockOffset`(zh「本地时间偏差」/ en「Clock offset」)、
  `config.msUnit`(zh/en 均「ms」)。
- 纯读数，不着语义色(本功能不告警)。

## 测试

- `frame_abs_ms` 单测：普通帧、跨秒进位、V2/V3 质量位屏蔽不污染。
- `now_unix_ms` 合理性(> 某固定 epoch、单调非负)。
- 偏差正负号断言：构造 `frame_abs_ms` 已知值，`now_ms` 取大/小于它，验证
  符号与量级。
- `ts_monitor` 既有 13 单测全绿(抽取 `frame_abs_ms` 无回归)。
- `cargo test` 全绿；前端 `tsc` 类型检查 + `npm run build` 通过。

## 不做（YAGNI）

- 不超阈告警(无 toast / 事件日志 / 语义色)。
- 不进数据表、不画历史曲线、不存档。
- 不做主站 NTP 自校或时钟校准。
- 不改子站生成侧。
