# 报文时间与本地时间偏差 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 主站接收侧逐帧采样「本机时钟 − 报文时间戳」偏差，在配置面板读数区实时显示。

**Architecture:** 后端在三个数据帧接收点用 `now_unix_ms() − frame_abs_ms(...)` 算偏差，随每帧既有 `DataFrame` 事件搭车上送(`DataInfo.local_offset_ms`)；前端新 `useTimeOffset` composable 对最近 50 帧求均值平滑，`ConfigInfoPanel` 加一行读数。不新增事件类型、不告警。

**Tech Stack:** Rust(pmusim-core / pmusim-app, tokio)、Vue 3 `<script setup>` + TypeScript、Vitest、i18n(zh/en 手维护字典)。

## Global Constraints

- 符号约定:`local_offset_ms = now_unix_ms − frame_abs_ms`。正=报文滞后本地,负=报文超前本地。**全程一致**。
- 仅主站接收侧(pmusim-app);**不改子站**(pmusim-sub 的同名 `DataInfo`/`data_frame_to_info` 独立,不动)。
- **不新增 PmuEvent 类型**;复用既有 `DataFrame` 事件携带偏差。
- 仅实时读数:**不告警**(无 toast / 事件日志 / 语义色)、不进数据表、不存档。
- 面向用户文本中英双语(`frontend/src/i18n/messages.ts` zh + en 两块同时加键)。
- `ts_monitor` 既有 13 单测在重构后须**全绿**(即抽取 `frame_abs_ms` 的无回归验证)。
- git 提交作者固定 `Karl-Dai Karl <kelsoprotein@gmail.com>`,提交信息**不得**含任何 AI 署名/Co-Authored-By。

---

### Task 1: pmusim-core — `frame_abs_ms` + `now_unix_ms`，重构 ts_monitor

**Files:**
- Modify: `crates/pmusim-core/src/time_utils.rs`(新增两个 pub fn + 单测)
- Modify: `crates/pmusim-core/src/ts_monitor.rs:8`(import)、`:75`(cur_ms 改调)

**Interfaces:**
- Produces:
  - `pub fn frame_abs_ms(soc: u32, fracsec: u32, meas_rate: u32, version: u8) -> f64`
  - `pub fn now_unix_ms() -> f64`

- [ ] **Step 1: 写失败测试** — 在 `crates/pmusim-core/src/time_utils.rs` 的 `#[cfg(test)] mod tests` 内追加：

```rust
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
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p pmusim-core frame_abs_ms`
Expected: 编译失败 `cannot find function frame_abs_ms`（及 `now_unix_ms`）。

- [ ] **Step 3: 实现两个 fn** — 在 `crates/pmusim-core/src/time_utils.rs` 中，`fracsec_to_ms`(结尾约 38 行)之后插入：

```rust
/// 数据帧自带时间戳的绝对毫秒：SOC 秒 + FRACSEC 亚秒。FRACSEC 高 8 位
/// 时标质量码由 `fracsec_to_ms` 无条件屏蔽（见其文档），避免质量位翻转
/// 污染换算。`version` 仅为兼容签名保留。偏差测量与 `ts_monitor` 共用此
/// 定义，避免「帧绝对毫秒」散落两处。
pub fn frame_abs_ms(soc: u32, fracsec: u32, meas_rate: u32, version: u8) -> f64 {
    soc as f64 * 1000.0 + fracsec_to_ms(fracsec, meas_rate, version)
}
```

并在 `current_soc`(约 85-90 行)之后插入：

```rust
/// 本机墙钟相对 UNIX epoch 的毫秒数（f64，保留亚毫秒）。偏差测量的
/// 「本地时间」基准——受本机时钟是否校准影响，这正是要暴露的对象。
/// 时钟早于 epoch（不可能但防御）时返回 0。
pub fn now_unix_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p pmusim-core frame_abs_ms && cargo test -p pmusim-core now_unix_ms`
Expected: PASS。

- [ ] **Step 5: 重构 ts_monitor 改调 frame_abs_ms**

`crates/pmusim-core/src/ts_monitor.rs:8` 把
```rust
use crate::time_utils::fracsec_to_ms;
```
改为
```rust
use crate::time_utils::frame_abs_ms;
```

`crates/pmusim-core/src/ts_monitor.rs:75` 把
```rust
        let cur_ms = soc as f64 * 1000.0 + fracsec_to_ms(fracsec, meas_rate, version);
```
改为
```rust
        let cur_ms = frame_abs_ms(soc, fracsec, meas_rate, version);
```

- [ ] **Step 6: 跑 pmusim-core 全量测试（含 ts_monitor 13 单测回归）**

Run: `cargo test -p pmusim-core`
Expected: 全 PASS，无 unused import 警告（`fracsec_to_ms` 仍被 `frame_abs_ms` 使用，无悬空导入）。

- [ ] **Step 7: 提交**

```bash
git add crates/pmusim-core/src/time_utils.rs crates/pmusim-core/src/ts_monitor.rs
git -c user.name='Karl-Dai Karl' -c user.email='kelsoprotein@gmail.com' \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "feat(core): 抽 frame_abs_ms + now_unix_ms，ts_monitor 改调"
```

---

### Task 2: pmusim-app — `DataInfo.local_offset_ms` + 三接收点采样

**Files:**
- Modify: `crates/pmusim-app/src/events.rs:41-55`(DataInfo 加字段)
- Modify: `crates/pmusim-app/src/network/master.rs:18`(import)、`:364-372`、`:388-396`、`:935-941`(三接收点)、`:1818-1831`(data_frame_to_info)、文件尾(新增测试模块)

**Interfaces:**
- Consumes: `frame_abs_ms`、`now_unix_ms`(Task 1)。
- Produces: `DataInfo.local_offset_ms: f64`(经 `DataFrame` 事件流向前端)；`data_frame_to_info(df: &DataFrame, local_offset_ms: f64) -> DataInfo`。

- [ ] **Step 1: 写失败测试** — 在 `crates/pmusim-app/src/network/master.rs` 文件末尾追加测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pmusim_core::protocol::constants::ProtocolVersion;
    use pmusim_core::protocol::frame::DataFrame;

    fn sample_df() -> DataFrame {
        DataFrame {
            version: ProtocolVersion::V2,
            idcode: "TEST".into(),
            soc: 100,
            fracsec: 0,
            stat: 0,
            format_flags: 0,
            phasors: vec![],
            freq: 0.0,
            dfreq: 0.0,
            analog: vec![],
            digital: vec![],
        }
    }

    #[test]
    fn data_frame_to_info_carries_offset() {
        // 偏差作为参数传入，应原样落入 DataInfo（确定性，不依赖时钟）。
        let df = sample_df();
        let info = data_frame_to_info(&df, 123.5);
        assert_eq!(info.local_offset_ms, 123.5);
        assert_eq!(info.soc, 100);
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p pmusim-app data_frame_to_info_carries_offset`
Expected: 编译失败——`data_frame_to_info` 实参个数不符 + `DataInfo` 无 `local_offset_ms` 字段。

- [ ] **Step 3a: DataInfo 加字段** — `crates/pmusim-app/src/events.rs`，在 `DataInfo` 的 `phasors` 字段后(约第 54 行)加：

```rust
    /// 接收时刻本机时钟与本帧报文时间戳之差(ms)：now − 报文时间。
    /// 正=报文滞后本地，负=报文超前本地。仅展示用，不参与编码。
    pub local_offset_ms: f64,
```

- [ ] **Step 3b: 改 data_frame_to_info 签名 + 落值** — `crates/pmusim-app/src/network/master.rs:1818`：

```rust
fn data_frame_to_info(df: &pmusim_core::protocol::frame::DataFrame, local_offset_ms: f64) -> DataInfo {
    DataInfo {
        soc: df.soc,
        fracsec: df.fracsec,
        stat: df.stat,
        format_flags: df.format_flags,
        time_quality: ((df.fracsec >> 24) & 0x0F) as u8,
        freq: df.freq,
        dfreq: df.dfreq,
        analog: df.analog.clone(),
        digital: df.digital.clone(),
        phasors: df.phasors.clone(),
        local_offset_ms,
    }
}
```

- [ ] **Step 3c: 加 import** — `crates/pmusim-app/src/network/master.rs:18`：

```rust
use pmusim_core::time_utils::{current_soc, frame_abs_ms, now_unix_ms, soc_to_beijing};
```

- [ ] **Step 3d: 接收点①(V2 首帧, 约 364-372 行)** — 把该 `if let Ok(Frame::Data(df))` 块改为：

```rust
                    if let Ok(Frame::Data(df)) = parse(&frame_data, cfg2.format_flags, cfg2.phnmr, cfg2.annmr, cfg2.dgnmr) {
                        check_frame_timestamp(&mut ts_monitor, &df, cfg2.period_ms(), cfg2.meas_rate, &event_tx, &session_idcode);
                        let local_offset_ms = now_unix_ms()
                            - frame_abs_ms(df.soc, df.fracsec, cfg2.meas_rate, df.version as u8);
                        emit_event(
                            &event_tx,
                            PmuEvent::DataFrame {
                                idcode: session_idcode.clone(),
                                data: data_frame_to_info(&df, local_offset_ms),
                            },
                        );
                    }
```

- [ ] **Step 3e: 接收点②(V2 续读循环, 约 388-396 行)** — 同样改为：

```rust
                    if let Ok(Frame::Data(df)) = parse(&frame_data, cfg2.format_flags, cfg2.phnmr, cfg2.annmr, cfg2.dgnmr) {
                        check_frame_timestamp(&mut ts_monitor, &df, cfg2.period_ms(), cfg2.meas_rate, &event_tx, &session_idcode);
                        let local_offset_ms = now_unix_ms()
                            - frame_abs_ms(df.soc, df.fracsec, cfg2.meas_rate, df.version as u8);
                        emit_event(
                            &event_tx,
                            PmuEvent::DataFrame {
                                idcode: session_idcode.clone(),
                                data: data_frame_to_info(&df, local_offset_ms),
                            },
                        );
                    }
```

- [ ] **Step 3f: 接收点③(V3 续读循环, 约 935-941 行)** — `ts_params: Option<(f64, u32)>`(`(f64,u32)` 为 Copy，line 893 `if let` 后仍可用)。把该 `emit_event(... PmuEvent::DataFrame ...)` 改为先算偏差：

```rust
                let local_offset_ms = now_unix_ms()
                    - frame_abs_ms(df.soc, df.fracsec, ts_params.map(|(_, mr)| mr).unwrap_or(0), df.version as u8);
                emit_event(
                    &event_tx,
                    PmuEvent::DataFrame {
                        idcode: idcode.clone(),
                        data: data_frame_to_info(&df, local_offset_ms),
                    },
                );
```

(`meas_rate=0` 兜底:`frame_abs_ms` 退化为 `soc*1000`，偏差仍可算、不 panic。)

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p pmusim-app data_frame_to_info_carries_offset`
Expected: PASS。

- [ ] **Step 5: 跑 pmusim-app 全量(含既有 e2e)确认无回归**

Run: `cargo test -p pmusim-app`
Expected: 全 PASS。

- [ ] **Step 6: 提交**

```bash
git add crates/pmusim-app/src/events.rs crates/pmusim-app/src/network/master.rs
git -c user.name='Karl-Dai Karl' -c user.email='kelsoprotein@gmail.com' \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "feat(master): 数据帧加本地时钟偏差 local_offset_ms，三接收点采样"
```

---

### Task 3: 前端 — `useTimeOffset` composable + `DataInfo` 类型字段

**Files:**
- Create: `frontend/src/composables/useTimeOffset.ts`
- Create: `frontend/tests/use-time-offset.test.ts`
- Modify: `frontend/src/types/index.ts:22-36`(DataInfo 加字段)

**Interfaces:**
- Produces: `useTimeOffset(): { offsetMs: Ref<number|null>, tick(ms:number):void, reset():void }`——**模块级单例状态**(与 `useFrameRate` 一致，ticker 与读取方共享同一窗口)。
- Consumes: `DataInfo.local_offset_ms: number`(Task 2)。

- [ ] **Step 1: 写失败测试** — 创建 `frontend/tests/use-time-offset.test.ts`：

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { useTimeOffset } from "../src/composables/useTimeOffset";

describe("useTimeOffset 最近 50 帧偏差滑动均值", () => {
  beforeEach(() => useTimeOffset().reset());

  it("样本数 < 窗口时取全部样本均值", () => {
    const { tick, offsetMs } = useTimeOffset();
    tick(100);
    tick(200);
    expect(offsetMs.value).toBe(150);
  });

  it("超过 50 帧只保留最近 50 帧求均值", () => {
    const { tick, offsetMs } = useTimeOffset();
    // 推 60 帧 0..59 → 窗口保留 10..59，均值 (10+59)/2 = 34.5。
    for (let i = 0; i < 60; i++) tick(i);
    expect(offsetMs.value).toBe(34.5);
  });

  it("负偏差(报文超前本地)如实平均", () => {
    const { tick, offsetMs } = useTimeOffset();
    tick(-20);
    tick(-40);
    expect(offsetMs.value).toBe(-30);
  });

  it("reset() 后回到 null（显示 —）", () => {
    const { tick, offsetMs, reset } = useTimeOffset();
    tick(10);
    reset();
    expect(offsetMs.value).toBeNull();
  });
});
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd frontend && npx vitest run tests/use-time-offset.test.ts`
Expected: FAIL——`Cannot find module ../src/composables/useTimeOffset`。

- [ ] **Step 3: 实现 composable** — 创建 `frontend/src/composables/useTimeOffset.ts`：

```ts
import { ref } from "vue";

// 报文时间与本机时钟的偏差(ms)滑动均值。每帧偏差由后端在接收时刻采样
// (now − 报文时间戳)写入 DataInfo.local_offset_ms；这里保留最近 N 帧定长
// 计数窗求均值，抹平逐帧网络抖动。正=报文滞后本地，负=报文超前本地。
// 模块级单例：usePmuEvents 逐帧 tick、ConfigInfoPanel 读 offsetMs，共享同窗。

const WINDOW = 50;
const samples: number[] = [];
const offsetMs = ref<number | null>(null);

export function useTimeOffset() {
  function tick(ms: number) {
    samples.push(ms);
    if (samples.length > WINDOW) samples.shift();
    const sum = samples.reduce((a, b) => a + b, 0);
    offsetMs.value = sum / samples.length;
  }
  function reset() {
    samples.length = 0;
    offsetMs.value = null;
  }
  return { offsetMs, tick, reset };
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd frontend && npx vitest run tests/use-time-offset.test.ts`
Expected: PASS（4 个用例）。

- [ ] **Step 5: DataInfo TS 类型加字段** — `frontend/src/types/index.ts`，在 `DataInfo` 的 `phasors` 字段后(约第 35 行)加：

```ts
  /** 后端接收时刻 now − 报文时间戳(ms)。正=报文滞后本地，负=超前。 */
  local_offset_ms: number;
```

- [ ] **Step 6: 提交**

```bash
git add frontend/src/composables/useTimeOffset.ts frontend/tests/use-time-offset.test.ts frontend/src/types/index.ts
git -c user.name='Karl-Dai Karl' -c user.email='kelsoprotein@gmail.com' \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "feat(frontend): useTimeOffset 滑动均值 composable + DataInfo 类型字段"
```

---

### Task 4: 前端 — 接线 `usePmuEvents` + `ConfigInfoPanel` 读数 + i18n

**Files:**
- Modify: `frontend/src/composables/usePmuEvents.ts`(import、destructure、DataFrame 分支、三个 reset 点)
- Modify: `frontend/src/components/ConfigInfoPanel.vue`(import、destructure、computed、读数行)
- Modify: `frontend/src/i18n/messages.ts:31`(zh)、`:136`(en)

**Interfaces:**
- Consumes: `useTimeOffset`(Task 3)、`DataInfo.local_offset_ms`(Task 2/3)。

- [ ] **Step 1: usePmuEvents 接线** — `frontend/src/composables/usePmuEvents.ts`：

import 区(约第 8 行 `import { useFrameRate }` 后)加：
```ts
import { useTimeOffset } from "./useTimeOffset";
```

在 `const { tick: tickFrameRate, reset: resetFrameRate } = useFrameRate();`(约第 33 行)后加：
```ts
  const { tick: tickOffset, reset: resetOffset } = useTimeOffset();
```

`DataFrame` 分支内 `tickFrameRate(frameTimeMs(...));` 之后加：
```ts
        tickOffset(payload.data.local_offset_ms);
```

三处 `resetFrameRate();` 各自下一行补 `resetOffset();`：`SessionDisconnected`、`StreamingStopped`、`HeartbeatTimeout` 分支。例如 `SessionDisconnected`：
```ts
        resetFrameRate();
        resetOffset();
```
（另两处同样紧跟 `resetFrameRate();` 加 `resetOffset();`。）

- [ ] **Step 2: ConfigInfoPanel 显示** — `frontend/src/components/ConfigInfoPanel.vue`：

import 区(约第 9 行 `import { useFrameRate }` 后)加：
```ts
import { useTimeOffset } from "../composables/useTimeOffset";
```

在 `const { fps } = useFrameRate();`(约第 22 行)后加：
```ts
const { offsetMs } = useTimeOffset();
// 偏差读数:带符号整数 ms；无样本显示「—」。正号显式加，负号由数字自带。
const clockOffsetText = computed(() => {
  const v = offsetMs.value;
  if (v === null) return "—";
  const r = Math.round(v);
  return (r > 0 ? "+" : "") + r;
});
```

读数区在「上传速率」行(约第 441 行)后加一行：
```html
        <div class="rd-row"><label>{{ t("config.clockOffset") }}</label><span class="rd-val mono">{{ clockOffsetText }}<span v-if="offsetMs !== null" class="unit">{{ t("config.msUnit") }}</span></span></div>
```

- [ ] **Step 3: i18n 加键** — `frontend/src/i18n/messages.ts`：

zh 块 `'config.fpsUnit': '帧/秒',`(第 31 行)后加：
```ts
    'config.clockOffset': '本地时间偏差',
    'config.msUnit': 'ms',
```

en 块 `'config.fpsUnit': 'fps',`(第 136 行)后加：
```ts
    'config.clockOffset': 'Clock offset',
    'config.msUnit': 'ms',
```

- [ ] **Step 4: 类型检查 + 构建 + 全量前端测试**

Run: `cd frontend && npm run build && npm run test:unit`
Expected: `vue-tsc` 无类型错误、`vite build` 成功、Vitest 全绿(含既有 `use-frame-rate`、`config-info-panel.0hz` 与新 `use-time-offset`)。

- [ ] **Step 5: 后端全量回归(确认跨 crate 无遗漏)**

Run: `cargo test`
Expected: workspace 全 PASS。

- [ ] **Step 6: 提交**

```bash
git add frontend/src/composables/usePmuEvents.ts frontend/src/components/ConfigInfoPanel.vue frontend/src/i18n/messages.ts
git -c user.name='Karl-Dai Karl' -c user.email='kelsoprotein@gmail.com' \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "feat(frontend): 配置面板显示本地时间偏差读数"
```

---

## 验证清单(完工后)

- [ ] `cargo test`(workspace 全绿,ts_monitor 13 单测无回归)。
- [ ] `cd frontend && npm run build && npm run test:unit`(类型/构建/单测全绿)。
- [ ] 手动冒烟:连真实子站/网关推流,读数区「本地时间偏差」应显示小幅 +xx ms(LAN 上多为个位/十位 ms);把本机时钟人为调快几秒,偏差应同步变负数验证符号正确。
