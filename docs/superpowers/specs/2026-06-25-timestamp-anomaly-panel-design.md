# 异常报文跳帧监视面板 设计

- 日期：2026-06-25
- 状态：已批准，待实现
- 关联：v0.10.0「数据帧时间戳错乱检测」(`docs/superpowers/specs/2026-06-23-data-frame-timestamp-anomaly-design.md`)

## 背景

主站接收侧已具备数据帧时间戳错乱检测（`crates/pmusim-core/src/ts_monitor.rs`），可判定三类异常：

- `Backward` 回退（delta < 0）
- `Gap` 跳变 / 丢帧（delta 明显大于预期间隔）
- `Stall` 停滞（delta 明显小于预期间隔，含重复时间戳）

当前检测结果被 `network/master.rs::check_frame_timestamp` 打包成**一整条中文字符串**，通过 `PmuEvent::Error { idcode, error }` 上报。前端 `usePmuEvents.ts` 把它 `pushToast`（右下角红框）+ `pushEvent`（左面板生命周期事件日志）。

问题：异常与连接 / 握手等生命周期事件**混在同一个日志里**（`useEventLog`，200 条上限），既无法分列检索（类型 / 预期 / 实际 / SOC / 时间各字段挤在一行字符串里），也无法按类型 / 子站筛选、统计、导出。多子站联调时尤其难定位。

## 目标

在主界面新增一个**专门的异常报文跳帧监视面板**，把异常从混杂日志中拆出，结构化分列展示，支持筛选、计数、导出、详情展开。

## 非目标

- 不做真正独立的操作系统窗口（Tauri 多窗口）。形态是主界面内的一块面板。
- 不改动 `pmusim-core` 的检测算法本身（判定逻辑、容差、速率切换处理保持原样）。
- 不引入 Pinia / Vuex / 外部 UI 库（沿用现有 composables + 纯 CSS 设计系统）。
- 不引入 `@tauri-apps/plugin-fs`。

## 关键决策（已确认）

1. **形态**：主界面内独立面板，非 OS 窗口。
2. **位置**：底部横置全宽区域，与左配置、右数据表同屏。
3. **数据来源**：后端新增结构化事件 `PmuEvent::TimestampAnomaly`，前端直接分列；不靠前端解析旧字符串。
4. **与现有日志分工**：异常移入新面板；**不再进**生命周期事件日志（`useEventLog`）；toast 红框保留作即时提醒。
5. **默认折叠**：面板默认折叠，仅留标题栏常驻计数徽章；点击展开，高度可拖拽。
6. **CSV 导出**：经新增的最小 Tauri 命令 `save_text_file(path, content)` 写盘，配合现有 `plugin-dialog` 的 `save()` 选路径；不引入 plugin-fs。

## 架构与数据流

```
ts_monitor.feed()  →  TsReport
        │ (network/master.rs::check_frame_timestamp)
        ▼
PmuEvent::TimestampAnomaly { idcode, kind, expected_ms, actual_ms, soc, fracsec, frame_time }
        │ emit_event → AppState 事件缓冲 (VecDeque)
        ▼
前端 poll_events (100ms 轮询)  →  usePmuEvents.handle()
        ├─ useAnomalyLog.push(entry)   → AnomalyPanel 响应式渲染
        └─ useToast.push(文案)          → 右下角红框即时提醒
```

沿用现有轮询模型（不碰 `listen/emit`，规避 macOS WebKit 竞态）。

## 后端改动（Rust）

### A. `crates/pmusim-app/src/events.rs`

`PmuEvent` 新增变体（`PmuEvent` 已是 `#[serde(tag = "type")]`，字段默认 snake_case，与现有 `Error { idcode, error }` 一致）：

```rust
TimestampAnomaly {
    idcode: String,
    kind: String,        // "backward" | "gap" | "stall"
    expected_ms: f64,
    actual_ms: f64,      // 回退时为负
    soc: u32,
    fracsec: u32,
    frame_time: String,  // soc_to_beijing(soc) 算好的北京时间字符串
},
```

前端收到的形状：`{ type: "TimestampAnomaly", idcode, kind, expected_ms, actual_ms, soc, fracsec, frame_time }`。

### B. `crates/pmusim-app/src/network/master.rs`

- `check_frame_timestamp` 由 emit `PmuEvent::Error` 改为 emit `PmuEvent::TimestampAnomaly`。
- 新增 `TsAnomalyKind → &str` 的 code 映射（不放进 `pmusim-core`，保持 core 无 serde 依赖）：
  ```rust
  fn anomaly_code(kind: TsAnomalyKind) -> &'static str {
      match kind {
          TsAnomalyKind::Backward => "backward",
          TsAnomalyKind::Gap => "gap",
          TsAnomalyKind::Stall => "stall",
      }
  }
  ```
- `frame_time` 复用现有 `soc_to_beijing(r.soc)`。
- `format_ts_anomaly`（拼整条中文串的函数）不再被事件使用；删除，或若别处仍引用则保留。toast 中文文案改由前端按 i18n 拼装。
- 注意：现有两处 V2、一处 V3 的检测挂载点均经由 `check_frame_timestamp`，改这一个函数即可覆盖全部。

### C. CSV 写盘命令

新增最小命令（`crates/pmusim-app/src/commands.rs` + `main.rs` 的 `invoke_handler` 注册）：

```rust
#[tauri::command]
pub fn save_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}
```

前端 `plugin-dialog` 的 `save()` 取路径后调用它。

## 前端改动（Vue3 + TS）

### A. `frontend/src/types/index.ts`

- `PmuEvent` 联合新增：
  ```ts
  | { type: "TimestampAnomaly"; idcode: string; kind: string;
      expected_ms: number; actual_ms: number; soc: number;
      fracsec: number; frame_time: string }
  ```
- 新增前端条目接口：
  ```ts
  export interface AnomalyEntry {
    id: number;            // 自增，作 v-for key 与去重
    localTime: string;     // 收报墙钟时刻 "HH:MM:SS"
    idcode: string;
    kind: "backward" | "gap" | "stall" | string;  // 容错未知 code
    expectedMs: number;
    actualMs: number;
    soc: number;
    fracsec: number;
    frameTime: string;     // 后端给的北京时间
  }
  ```

### B. 新 composable `frontend/src/composables/useAnomalyLog.ts`

模块级共享状态（对齐 `useEventLog.ts` 风格）：

- `entries: reactive<AnomalyEntry[]>`，新条目 `unshift`，FIFO 上限 **500**。
- `push(ev)`：把 `TimestampAnomaly` 事件转 `AnomalyEntry`（补 `id` 自增、`localTime`）后插入。
- `clear()`：清空。
- `counts`：computed，按 kind 统计 `{ backward, gap, stall, total }`。
- `toCsv()`：生成 CSV 文本（表头 + 各列；数值保留 1 位小数，与 UI 一致）。

### C. `frontend/src/composables/usePmuEvents.ts`

- 新增 `case "TimestampAnomaly"`：
  - `pushAnomaly(payload)`（来自 `useAnomalyLog`）。
  - `pushToast(...)`：用 i18n 模板拼中文文案（保留即时红框）。
  - **不** `pushEvent`。
- 原 `case "Error"` 保留不动（其它后端错误仍 toast + 生命周期日志）。

### D. 新组件 `frontend/src/components/AnomalyPanel.vue`

底部全宽面板。

- **顶栏**：
  - 计数徽章：`回退 N · 跳变 N · 停滞 N · 总计 N`，按类型语义色。
  - 类型筛选：全部 / 回退 / 跳变 / 停滞。
  - 子站筛选：下拉，取当前出现过的 idcode 去重。
  - 清空按钮。
  - 导出 CSV 按钮（无数据时禁用）。
  - 折叠 / 展开切换（点标题栏整体切换）。
- **表格列**（按序）：
  `时刻 | 子站 | 类型 | 预期ms | 实际ms | 丢帧≈ | SOC | 帧时间(北京) | FRACSEC`
  - 丢帧≈ 仅 `gap` 行有值：`Math.max(1, Math.round(actualMs / expectedMs) - 1)`，显示 `≈N`；其它类型留空。
  - 类型列语义色：回退 `--warn`（琥珀）、跳变 `--err`（红）、停滞 `--text-dim`（灰）。
  - FRACSEC 以 `0x%08x` hex 显示。
- **点行展开**：展开该行详情（完整 FRACSEC hex、各字段原值、可一键复制整条），复用数据表已有的选中高亮交互风格。
- 折叠态：仅标题栏 + 计数徽章常驻；有新异常时徽章红色高亮提示。展开态高度可拖拽。

### E. `frontend/src/App.vue`

- `.content` 区域下方挂 `<AnomalyPanel />`。
- 外层 `.app` 已是 flex 列布局；底部面板折叠态固定矮高、展开态可拖高，且不挤压上方两列（上方 `.content` 仍 `flex: 1`）。

### F. `frontend/src/i18n/messages.ts`

补中英文案：面板标题、各列名、三类异常标签（回退 / 跳变 / 停滞）、按钮（清空 / 导出 CSV / 折叠展开）、toast 模板、CSV 表头。

## 错误处理

- 未知 `kind` code：类型列原样显示该字符串、不崩，不计入三类徽章但计入 total。
- 导出无数据：按钮禁用。
- `save_text_file` 失败：catch 后 `pushToast` 报错；用户取消 `save()` 对话框时静默。
- FIFO 500 上限：防极端高频异常（每帧报）撑爆内存。
- 数值显示：`expected_ms` / `actual_ms` 保留 1 位小数，与现有日志文案口径一致。

## 测试

### 前端（Vitest）

- `useAnomalyLog` 单测：
  - `push` 转换字段正确（snake_case 事件 → camelCase 条目）。
  - FIFO 截断到 500。
  - `counts` 按 kind 统计正确（含未知 code 仅计 total）。
  - 丢帧估算：`gap` 行 `actual/expected` 各场景取整正确（如 40/20→≈1、60/20→≈2）。
  - `toCsv()` 表头与行格式。
- `AnomalyPanel` 组件测试：列渲染、类型 / 子站筛选联动、清空、行展开、空态导出禁用。
- 风格参考 `frontend/tests/use-pmu-events.reconnect.test.ts`。

### 后端（cargo test）

- `ts_monitor.rs` 现有 13 个单测保持绿（算法未动）。
- 若 `master.rs` 有覆盖事件 emit 的测试，同步改成断言 `TimestampAnomaly`。

### 验收标准

- `cargo test` 全绿。
- `cd frontend && npm run test:unit` 全绿。
- `npm run build`（含 `vue-tsc`）类型检查通过。
- 手动：触发一次跳帧（如跳过某帧），面板出现对应行、计数 +1、toast 弹出、生命周期日志不再混入该异常；导出 CSV 内容正确。

## 实现顺序建议

1. 后端：`events.rs` 新变体 → `master.rs` 改 emit + code 映射 → `save_text_file` 命令注册。`cargo test`。
2. 前端类型 + `useAnomalyLog` + 单测。
3. `usePmuEvents` 分发 + i18n 文案。
4. `AnomalyPanel.vue` + `App.vue` 挂载 + 组件测试。
5. 全量测试 + 手动验证。
