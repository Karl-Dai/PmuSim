# 异常报文跳帧监视面板 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在主站模拟器主界面新增底部全宽「异常报文跳帧」监视面板，把时间戳错乱异常从混杂日志中拆出，结构化分列展示，支持筛选 / 计数 / 导出 / 详情展开。

**Architecture:** 后端把现有 `check_frame_timestamp` 的字符串 `PmuEvent::Error` 改为结构化 `PmuEvent::TimestampAnomaly`，新增最小写盘命令 `save_text_file`；前端新增纯函数库 + `useAnomalyLog` 状态 composable + `AnomalyPanel.vue` 组件，经现有 100ms 轮询模型消费事件。异常不再进生命周期事件日志，toast 红框保留。

**Tech Stack:** Rust / Tauri 2（`pmusim-app`）、Vue 3.5 Composition API + TypeScript + Vite、Vitest + @vue/test-utils + happy-dom、`@tauri-apps/plugin-dialog`。

## Global Constraints

- 不改动 `crates/pmusim-core`（保持无 serde、无 IO；`TsAnomalyKind` 不加 `Serialize`）。
- 不引入新依赖：禁止 `@tauri-apps/plugin-fs`、Pinia / Vuex、任何外部 UI 库。
- 异常不再写入生命周期事件日志（`useEventLog`）；toast 红框保留。
- 异常条目 FIFO 上限 **500**。
- 沿用 `poll_events` 轮询模型，不使用 `listen/emit`。
- `expected_ms` / `actual_ms` 显示保留 **1 位小数**。
- 事件字段为 snake_case（`PmuEvent` 用 `#[serde(tag = "type")]`，无 `rename_all`）；前端条目内部用 camelCase。
- 文案中英双语，两份都加到 `frontend/src/i18n/messages.ts` 的 `zh` 与 `en`。
- 每次提交作者与提交者均须为 `Karl-Dai Karl <kelsoprotein@gmail.com>`，禁止任何 `Co-Authored-By` 或生成署名（本仓库 git config 的 email 不符，故每条 commit 用 `-c` 覆盖，见各 Commit 步骤）。
- 所有命令在 worktree 根目录执行：`/Users/daichangyu/Library/Mobile Documents/com~apple~CloudDocs/code/PmuSim/.claude/worktrees/anomaly-panel`。

---

## File Structure

**后端（Rust）**
- `crates/pmusim-app/src/events.rs` — 修改：`PmuEvent` 新增 `TimestampAnomaly` 变体。
- `crates/pmusim-app/src/network/master.rs` — 修改：`check_frame_timestamp` 改 emit 结构化事件 + `anomaly_code` 映射；删除 `format_ts_anomaly`，保留 `soc_to_beijing`。
- `crates/pmusim-app/src/commands.rs` — 修改：追加 `save_text_file` 命令。
- `crates/pmusim-app/src/main.rs` — 修改：`generate_handler!` 注册 `save_text_file`。

**前端（Vue/TS）**
- `frontend/src/types/index.ts` — 修改：`PmuEvent` 加变体 + 新增 `AnomalyEntry` 接口。
- `frontend/src/lib/anomaly.ts` — 新建：纯函数 `droppedFrames` / `buildCsv` / `kindI18nKey`。
- `frontend/src/composables/useAnomalyLog.ts` — 新建：模块级状态 + `push` / `clear` / `counts`。
- `frontend/src/composables/usePmuEvents.ts` — 修改：新增 `TimestampAnomaly` 分发。
- `frontend/src/components/AnomalyPanel.vue` — 新建：底部面板 UI。
- `frontend/src/i18n/messages.ts` — 修改：补 `anomaly.*` 文案（zh + en）。
- `frontend/src/App.vue` — 修改：挂载 `<AnomalyPanel />` + 折叠/拖高布局。

**测试**
- `frontend/tests/anomaly.test.ts` — 新建：纯函数单测。
- `frontend/tests/use-anomaly-log.test.ts` — 新建：状态单测。
- `frontend/tests/use-pmu-events.anomaly.test.ts` — 新建：事件分发单测。
- `frontend/tests/anomaly-panel.test.ts` — 新建：组件测试。

---

## Task 1: 后端结构化异常事件 + 检测改造

**Files:**
- Modify: `crates/pmusim-app/src/events.rs:17`
- Modify: `crates/pmusim-app/src/network/master.rs:1845-1875`

**Interfaces:**
- Produces: `PmuEvent::TimestampAnomaly { idcode: String, kind: String, expected_ms: f64, actual_ms: f64, soc: u32, fracsec: u32, frame_time: String }` — 前端 Task 3 依赖此 JSON 形状。
- Consumes: 现有 `TimestampMonitor::feed` 返回的 `TsReport`、现有 `soc_to_beijing(soc: u32) -> String`（`master.rs:1878`）、`TsAnomalyKind`（`pmusim_core`）。

本任务无新增单元测试：`check_frame_timestamp` 是私有函数且 emit 经 `EventSender`，难以独立单测。以 `cargo build` 编译通过 + 现有 `ts_monitor` 13 个单测保持绿作为验收门。

- [ ] **Step 1: 在 events.rs 的 PmuEvent 末尾新增变体**

把 `crates/pmusim-app/src/events.rs` 第 17 行 `Error { idcode: String, error: String },` 之后、枚举闭合 `}` 之前插入：

```rust
    TimestampAnomaly {
        idcode: String,
        /// "backward" | "gap" | "stall"
        kind: String,
        expected_ms: f64,
        /// 回退时为负
        actual_ms: f64,
        soc: u32,
        fracsec: u32,
        /// soc_to_beijing(soc) 算好的北京时间字符串
        frame_time: String,
    },
```

- [ ] **Step 2: 改 master.rs 的 check_frame_timestamp 与替换 format 函数**

把 `crates/pmusim-app/src/network/master.rs` 第 1845-1875 行（`check_frame_timestamp` 函数体 + `format_ts_anomaly` 函数整体）替换为：

```rust
/// 逐帧喂入时间戳监视器，异常则把结构化报文曝给前端。
fn check_frame_timestamp(
    monitor: &mut TimestampMonitor,
    df: &DataFrame,
    period_ms: f64,
    meas_rate: u32,
    event_tx: &EventSender,
    idcode: &str,
) {
    if let Some(r) = monitor.feed(df.soc, df.fracsec, df.version as u8, meas_rate, period_ms) {
        emit_event(
            event_tx,
            PmuEvent::TimestampAnomaly {
                idcode: idcode.to_string(),
                kind: anomaly_code(r.kind).to_string(),
                expected_ms: r.expected_ms,
                actual_ms: r.actual_ms,
                soc: r.soc,
                fracsec: r.fracsec,
                frame_time: soc_to_beijing(r.soc),
            },
        );
    }
}

/// TsAnomalyKind → 前端 code（不在 pmusim-core 加 serde，保持 core 纯逻辑）。
fn anomaly_code(kind: TsAnomalyKind) -> &'static str {
    match kind {
        TsAnomalyKind::Backward => "backward",
        TsAnomalyKind::Gap => "gap",
        TsAnomalyKind::Stall => "stall",
    }
}
```

注意：`soc_to_beijing`（master.rs:1878）保持不动。删除了 `format_ts_anomaly`，其唯一调用方就是上面改掉的 emit。

- [ ] **Step 3: 修正 imports**

确认 `master.rs` 顶部已 `use` 到 `TsAnomalyKind` 与 `TsReport`。原 `format_ts_anomaly` 用过 `TsReport`，现已删除；`anomaly_code` 用到 `TsAnomalyKind`。检查文件顶部 `use pmusim_core::...` 行：若只导入了 `TimestampMonitor` / `TsReport`，需补 `TsAnomalyKind`。

Run（确认当前导入）:
```bash
rg -n "use pmusim_core.*ts_monitor|TsAnomalyKind|TsReport|TimestampMonitor" crates/pmusim-app/src/network/master.rs
```

把含 `ts_monitor` 的 use 行改为同时包含三者（若 `TsReport` 已不再被任何代码使用，编译器会告警 unused，按需移除 `TsReport`）：

```rust
use pmusim_core::ts_monitor::{TimestampMonitor, TsAnomalyKind};
```

（实际模块路径以 Step 之前 `rg` 结果为准，仅替换花括号内的标识符集合。）

- [ ] **Step 4: 编译并跑后端测试**

Run:
```bash
cargo build 2>&1 | tail -20
cargo test -p pmusim-core ts_monitor 2>&1 | tail -20
```
Expected: `cargo build` 成功无 error；`ts_monitor` 测试全部 `ok`（13 passed）。若报 `unused import: TsReport`，从 Step 3 的 use 行删掉 `TsReport` 再重跑。

- [ ] **Step 5: Commit**

```bash
git add crates/pmusim-app/src/events.rs crates/pmusim-app/src/network/master.rs
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(anomaly): 时间戳异常改发结构化事件 TimestampAnomaly"
```

---

## Task 2: 后端 save_text_file 命令

**Files:**
- Modify: `crates/pmusim-app/src/commands.rs`（追加到文件末尾）
- Modify: `crates/pmusim-app/src/main.rs:13-27`

**Interfaces:**
- Produces: Tauri 命令 `save_text_file(path: String, content: String) -> Result<(), String>` — 前端 Task 6 的 CSV 导出依赖它。

本任务无单元测试（纯 IO 命令）；以 `cargo build` 通过为门。

- [ ] **Step 1: 追加命令到 commands.rs 末尾**

在 `crates/pmusim-app/src/commands.rs` 文件末尾追加：

```rust
/// 把文本写入指定路径。前端 CSV 导出用：plugin-dialog 的 save() 选好
/// 路径后调它落盘，避免引入 plugin-fs 插件 + capabilities 配置。
#[tauri::command]
pub fn save_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: 注册到 main.rs 的 generate_handler!**

在 `crates/pmusim-app/src/main.rs` 第 23 行 `commands::open_url,` 之后新增一行：

```rust
            commands::save_text_file,
```

- [ ] **Step 3: 编译**

Run:
```bash
cargo build 2>&1 | tail -20
```
Expected: 成功无 error。

- [ ] **Step 4: Commit**

```bash
git add crates/pmusim-app/src/commands.rs crates/pmusim-app/src/main.rs
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(anomaly): 新增 save_text_file 命令用于 CSV 导出"
```

---

## Task 3: 前端类型 + 纯函数库 + 单测

**Files:**
- Modify: `frontend/src/types/index.ts:44-56`
- Create: `frontend/src/lib/anomaly.ts`
- Create: `frontend/tests/anomaly.test.ts`

**Interfaces:**
- Consumes: Task 1 的事件 JSON 形状。
- Produces:
  - `interface AnomalyEntry { id, localTime, idcode, kind, expectedMs, actualMs, soc, fracsec, frameTime }`（在 `types/index.ts`）。
  - `droppedFrames(actualMs: number, expectedMs: number): number`
  - `buildCsv(entries: AnomalyEntry[]): string`
  - `kindI18nKey(kind: string): string`（返回如 `"anomaly.kind.gap"`）
  - Task 4/5/6 依赖这些。

- [ ] **Step 1: 在 types/index.ts 追加事件变体与 AnomalyEntry**

在 `frontend/src/types/index.ts` 第 56 行 `| { type: "Error"; idcode: string; error: string };` 之前插入一行（保持联合类型连贯）：

```ts
  | { type: "TimestampAnomaly"; idcode: string; kind: string; expected_ms: number; actual_ms: number; soc: number; fracsec: number; frame_time: string }
```

然后在文件末尾追加接口：

```ts
export interface AnomalyEntry {
  id: number;
  localTime: string; // 收报墙钟时刻 "HH:MM:SS"
  idcode: string;
  kind: string; // "backward" | "gap" | "stall" | 未知 code 原样
  expectedMs: number;
  actualMs: number; // 回退时为负
  soc: number;
  fracsec: number;
  frameTime: string; // 后端给的北京时间
}
```

- [ ] **Step 2: 写纯函数失败测试**

创建 `frontend/tests/anomaly.test.ts`：

```ts
import { describe, it, expect } from "vitest";
import { droppedFrames, buildCsv, kindI18nKey } from "../src/lib/anomaly";
import type { AnomalyEntry } from "../src/types";

function entry(over: Partial<AnomalyEntry> = {}): AnomalyEntry {
  return {
    id: 1,
    localTime: "14:30:45",
    idcode: "PMU1",
    kind: "gap",
    expectedMs: 20,
    actualMs: 40,
    soc: 1781,
    fracsec: 0x000d9490,
    frameTime: "2026-06-23 14:30:45",
    ...over,
  };
}

describe("droppedFrames", () => {
  it("丢一帧 40/20 → 1", () => {
    expect(droppedFrames(40, 20)).toBe(1);
  });
  it("丢两帧 60/20 → 2", () => {
    expect(droppedFrames(60, 20)).toBe(2);
  });
  it("结果至少为 1（轻微超界 31/20 也算丢 1 帧）", () => {
    expect(droppedFrames(31, 20)).toBe(1);
  });
  it("expected<=0 → 0（防除零）", () => {
    expect(droppedFrames(40, 0)).toBe(0);
  });
});

describe("kindI18nKey", () => {
  it("映射已知 code", () => {
    expect(kindI18nKey("gap")).toBe("anomaly.kind.gap");
    expect(kindI18nKey("backward")).toBe("anomaly.kind.backward");
    expect(kindI18nKey("stall")).toBe("anomaly.kind.stall");
  });
  it("未知 code 走 unknown key", () => {
    expect(kindI18nKey("weird")).toBe("anomaly.kind.unknown");
  });
});

describe("buildCsv", () => {
  it("首行是表头，gap 行带丢帧数，数值 1 位小数，FRACSEC 为 hex", () => {
    const csv = buildCsv([entry()]);
    const lines = csv.split("\r\n");
    expect(lines.length).toBe(2);
    expect(lines[0]).toContain("FRACSEC");
    expect(lines[1]).toContain("14:30:45");
    expect(lines[1]).toContain("PMU1");
    expect(lines[1]).toContain("20.0");
    expect(lines[1]).toContain("40.0");
    expect(lines[1]).toContain("0x000d9490");
    // 丢帧列 = 1
    expect(lines[1].split(",")).toContain("1");
  });
  it("非 gap 行丢帧列为空", () => {
    const csv = buildCsv([entry({ kind: "stall", actualMs: 0 })]);
    const cells = csv.split("\r\n")[1].split(",");
    // 丢帧列（索引 5）为空字符串
    expect(cells[5]).toBe("");
  });
  it("含逗号的字段被双引号包裹转义", () => {
    const csv = buildCsv([entry({ idcode: "A,B" })]);
    expect(csv).toContain('"A,B"');
  });
});
```

- [ ] **Step 3: 跑测试确认失败**

Run:
```bash
cd frontend && npx vitest run tests/anomaly.test.ts 2>&1 | tail -20
```
Expected: FAIL —— 模块 `../src/lib/anomaly` 不存在 / 导入报错。

- [ ] **Step 4: 实现 lib/anomaly.ts**

创建 `frontend/src/lib/anomaly.ts`：

```ts
import type { AnomalyEntry } from "../types";

/** Gap 估算丢了几帧：四舍五入间隔倍数减 1，至少 1。expected<=0 返回 0。 */
export function droppedFrames(actualMs: number, expectedMs: number): number {
  if (expectedMs <= 0) return 0;
  return Math.max(1, Math.round(actualMs / expectedMs) - 1);
}

const KNOWN = new Set(["backward", "gap", "stall"]);

/** 异常 code → i18n key，未知 code 归到 unknown。 */
export function kindI18nKey(kind: string): string {
  return KNOWN.has(kind) ? `anomaly.kind.${kind}` : "anomaly.kind.unknown";
}

const CSV_HEADER = [
  "时刻",
  "子站",
  "类型",
  "预期ms",
  "实际ms",
  "丢帧",
  "SOC",
  "帧时间",
  "FRACSEC",
];

function csvCell(s: string): string {
  return /[",\r\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

function fracHex(fracsec: number): string {
  return "0x" + (fracsec >>> 0).toString(16).padStart(8, "0");
}

/** 生成 CSV 文本（CRLF 行尾，表头固定中文，与 UI 列序一致）。 */
export function buildCsv(entries: AnomalyEntry[]): string {
  const rows = entries.map((e) => [
    e.localTime,
    e.idcode,
    e.kind,
    e.expectedMs.toFixed(1),
    e.actualMs.toFixed(1),
    e.kind === "gap" ? String(droppedFrames(e.actualMs, e.expectedMs)) : "",
    String(e.soc),
    e.frameTime,
    fracHex(e.fracsec),
  ]);
  return [CSV_HEADER, ...rows].map((r) => r.map(csvCell).join(",")).join("\r\n");
}
```

- [ ] **Step 5: 跑测试确认通过**

Run:
```bash
cd frontend && npx vitest run tests/anomaly.test.ts 2>&1 | tail -20
```
Expected: PASS（所有用例绿）。

- [ ] **Step 6: Commit**

```bash
git add frontend/src/types/index.ts frontend/src/lib/anomaly.ts frontend/tests/anomaly.test.ts
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(anomaly): 前端异常类型与纯函数库(丢帧估算/CSV)"
```

---

## Task 4: useAnomalyLog 状态 composable + 单测

**Files:**
- Create: `frontend/src/composables/useAnomalyLog.ts`
- Create: `frontend/tests/use-anomaly-log.test.ts`

**Interfaces:**
- Consumes: `AnomalyEntry`（types）、事件变体 `TimestampAnomaly`。
- Produces: `useAnomalyLog()` 返回 `{ entries: AnomalyEntry[], push(ev), clear(), counts: ComputedRef<{backward,gap,stall,total}> }`。Task 5/6 依赖。

- [ ] **Step 1: 写失败测试**

创建 `frontend/tests/use-anomaly-log.test.ts`：

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { useAnomalyLog } from "../src/composables/useAnomalyLog";
import type { PmuEvent } from "../src/types";

function ev(over: Partial<Extract<PmuEvent, { type: "TimestampAnomaly" }>> = {}) {
  return {
    type: "TimestampAnomaly",
    idcode: "PMU1",
    kind: "gap",
    expected_ms: 20,
    actual_ms: 40,
    soc: 1781,
    fracsec: 0x000d9490,
    frame_time: "2026-06-23 14:30:45",
    ...over,
  } as Extract<PmuEvent, { type: "TimestampAnomaly" }>;
}

beforeEach(() => {
  useAnomalyLog().clear();
});

describe("useAnomalyLog", () => {
  it("push 把 snake_case 事件转成 camelCase 条目，最新在前", () => {
    const { entries, push } = useAnomalyLog();
    push(ev({ soc: 1 }));
    push(ev({ soc: 2 }));
    expect(entries[0].soc).toBe(2);
    expect(entries[0].expectedMs).toBe(20);
    expect(entries[0].actualMs).toBe(40);
    expect(entries[0].frameTime).toBe("2026-06-23 14:30:45");
    expect(entries.length).toBe(2);
  });

  it("每条 id 唯一", () => {
    const { entries, push } = useAnomalyLog();
    push(ev());
    push(ev());
    expect(entries[0].id).not.toBe(entries[1].id);
  });

  it("counts 按 kind 统计，未知 code 仅计 total", () => {
    const { counts, push } = useAnomalyLog();
    push(ev({ kind: "gap" }));
    push(ev({ kind: "backward" }));
    push(ev({ kind: "stall" }));
    push(ev({ kind: "weird" }));
    expect(counts.value).toEqual({ backward: 1, gap: 1, stall: 1, total: 4 });
  });

  it("FIFO 截断到 500", () => {
    const { entries, push } = useAnomalyLog();
    for (let i = 0; i < 520; i++) push(ev({ soc: i }));
    expect(entries.length).toBe(500);
    // 最新（soc 519）在前，最旧的被丢
    expect(entries[0].soc).toBe(519);
  });

  it("clear 清空", () => {
    const { entries, push, clear } = useAnomalyLog();
    push(ev());
    clear();
    expect(entries.length).toBe(0);
  });
});
```

- [ ] **Step 2: 跑测试确认失败**

Run:
```bash
cd frontend && npx vitest run tests/use-anomaly-log.test.ts 2>&1 | tail -20
```
Expected: FAIL —— `../src/composables/useAnomalyLog` 不存在。

- [ ] **Step 3: 实现 useAnomalyLog.ts**

创建 `frontend/src/composables/useAnomalyLog.ts`：

```ts
import { reactive, computed } from "vue";
import type { AnomalyEntry, PmuEvent } from "../types";

// 模块级共享状态，对齐 useEventLog.ts 风格（无 Pinia）。
const entries = reactive<AnomalyEntry[]>([]);
const MAX_ENTRIES = 500;
let nextId = 1;

type AnomalyEvent = Extract<PmuEvent, { type: "TimestampAnomaly" }>;

function localNow(): string {
  const d = new Date();
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function useAnomalyLog() {
  function push(ev: AnomalyEvent) {
    entries.unshift({
      id: nextId++,
      localTime: localNow(),
      idcode: ev.idcode,
      kind: ev.kind,
      expectedMs: ev.expected_ms,
      actualMs: ev.actual_ms,
      soc: ev.soc,
      fracsec: ev.fracsec,
      frameTime: ev.frame_time,
    });
    if (entries.length > MAX_ENTRIES) entries.splice(MAX_ENTRIES);
  }

  function clear() {
    entries.splice(0);
  }

  const counts = computed(() => {
    let backward = 0;
    let gap = 0;
    let stall = 0;
    for (const e of entries) {
      if (e.kind === "backward") backward++;
      else if (e.kind === "gap") gap++;
      else if (e.kind === "stall") stall++;
    }
    return { backward, gap, stall, total: entries.length };
  });

  return { entries, push, clear, counts };
}
```

- [ ] **Step 4: 跑测试确认通过**

Run:
```bash
cd frontend && npx vitest run tests/use-anomaly-log.test.ts 2>&1 | tail -20
```
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add frontend/src/composables/useAnomalyLog.ts frontend/tests/use-anomaly-log.test.ts
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(anomaly): useAnomalyLog 状态(push/FIFO/计数/清空)"
```

---

## Task 5: usePmuEvents 分发 + i18n 文案 + 单测

**Files:**
- Modify: `frontend/src/composables/usePmuEvents.ts:28-33, 94-98`
- Modify: `frontend/src/i18n/messages.ts`（zh 与 en 各补 `anomaly.*`）
- Create: `frontend/tests/use-pmu-events.anomaly.test.ts`

**Interfaces:**
- Consumes: `useAnomalyLog().push`、`useToast().push`、`kindI18nKey`、`t`。
- Produces: 收到 `TimestampAnomaly` 时 → `pushAnomaly` + toast；不进 `useEventLog`。

- [ ] **Step 1: 写失败测试**

创建 `frontend/tests/use-pmu-events.anomaly.test.ts`：

```ts
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { usePmuEvents } from "../src/composables/usePmuEvents";
import { useAnomalyLog } from "../src/composables/useAnomalyLog";
import { useEventLog } from "../src/composables/useEventLog";
import { useToast } from "../src/composables/useToast";

const anomalyEvent = {
  type: "TimestampAnomaly",
  idcode: "PMU1",
  kind: "gap",
  expected_ms: 20,
  actual_ms: 40,
  soc: 1781,
  fracsec: 0x000d9490,
  frame_time: "2026-06-23 14:30:45",
};

beforeEach(() => {
  useAnomalyLog().clear();
  useEventLog().clear();
  invoke.mockReset();
  vi.useFakeTimers();
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("usePmuEvents 处理 TimestampAnomaly", () => {
  it("异常进入 anomaly log，不进生命周期事件日志，并弹 toast", async () => {
    const toast = useToast();
    const toastSpy = vi.spyOn(toast, "push");
    invoke.mockResolvedValueOnce([anomalyEvent]).mockResolvedValue([]);

    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);

    const { entries } = useAnomalyLog();
    expect(entries.length).toBe(1);
    expect(entries[0].idcode).toBe("PMU1");
    expect(entries[0].kind).toBe("gap");

    // 不混入生命周期日志
    expect(useEventLog().events.length).toBe(0);

    // 弹了一次错误 toast
    expect(toastSpy).toHaveBeenCalledTimes(1);
    expect(toastSpy.mock.calls[0][1]).toBe("error");
  });
});
```

- [ ] **Step 2: 跑测试确认失败**

Run:
```bash
cd frontend && npx vitest run tests/use-pmu-events.anomaly.test.ts 2>&1 | tail -20
```
Expected: FAIL —— `TimestampAnomaly` 未被处理，`entries.length` 为 0。

- [ ] **Step 3: 在 usePmuEvents.ts 接入 anomaly log**

在 `frontend/src/composables/usePmuEvents.ts` 顶部 import 区追加：

```ts
import { useAnomalyLog } from "./useAnomalyLog";
import { kindI18nKey } from "../lib/anomaly";
```

在 `usePmuEvents()` 函数体内（第 33 行 `const { tick: tickFrameRate, ... } = useFrameRate();` 之后）追加：

```ts
  const { push: pushAnomaly } = useAnomalyLog();
```

在 `handle` 的 `switch` 内、`case "Error":` 之前新增分支：

```ts
      case "TimestampAnomaly": {
        pushAnomaly(payload);
        const label = t(kindI18nKey(payload.kind));
        pushToast(
          t("anomaly.toast", {
            idcode: payload.idcode,
            kind: label,
            expected: payload.expected_ms.toFixed(1),
            actual: payload.actual_ms.toFixed(1),
          }),
          "error",
        );
        break;
      }
```

- [ ] **Step 4: 在 messages.ts 补文案（zh 与 en 都要）**

在 `frontend/src/i18n/messages.ts` 的 `zh: { ... }` 对象内追加（放在 `'config.skipCfg2Connect'` 行附近、对象闭合 `}` 之前即可）：

```ts
    // Anomaly panel
    'anomaly.title': '异常报文跳帧',
    'anomaly.kind.backward': '回退',
    'anomaly.kind.gap': '跳变',
    'anomaly.kind.stall': '停滞',
    'anomaly.kind.unknown': '未知',
    'anomaly.count.backward': '回退 {n}',
    'anomaly.count.gap': '跳变 {n}',
    'anomaly.count.stall': '停滞 {n}',
    'anomaly.count.total': '总计 {n}',
    'anomaly.filterKind': '类型',
    'anomaly.filterKindAll': '全部',
    'anomaly.filterStation': '子站',
    'anomaly.filterStationAll': '全部子站',
    'anomaly.clear': '清空',
    'anomaly.export': '导出 CSV',
    'anomaly.empty': '暂无异常',
    'anomaly.colTime': '时刻',
    'anomaly.colStation': '子站',
    'anomaly.colKind': '类型',
    'anomaly.colExpected': '预期ms',
    'anomaly.colActual': '实际ms',
    'anomaly.colDropped': '丢帧≈',
    'anomaly.colSoc': 'SOC',
    'anomaly.colFrameTime': '帧时间(北京)',
    'anomaly.colFracsec': 'FRACSEC',
    'anomaly.toast': '{idcode}: 时间戳{kind} 预期{expected}ms 实际{actual}ms',
    'anomaly.copy': '复制',
    'anomaly.copied': '已复制',
    'anomaly.exportFailed': '导出失败: {error}',
    'anomaly.exportDone': '已导出 {n} 条',
    'anomaly.exportEmpty': '无数据可导出',
    'anomaly.csvName': '异常跳帧记录.csv',
```

在 `en: { ... }` 对象内追加对应英文：

```ts
    // Anomaly panel
    'anomaly.title': 'Frame Anomalies',
    'anomaly.kind.backward': 'Backward',
    'anomaly.kind.gap': 'Gap',
    'anomaly.kind.stall': 'Stall',
    'anomaly.kind.unknown': 'Unknown',
    'anomaly.count.backward': 'Backward {n}',
    'anomaly.count.gap': 'Gap {n}',
    'anomaly.count.stall': 'Stall {n}',
    'anomaly.count.total': 'Total {n}',
    'anomaly.filterKind': 'Kind',
    'anomaly.filterKindAll': 'All',
    'anomaly.filterStation': 'Station',
    'anomaly.filterStationAll': 'All stations',
    'anomaly.clear': 'Clear',
    'anomaly.export': 'Export CSV',
    'anomaly.empty': 'No anomalies',
    'anomaly.colTime': 'Time',
    'anomaly.colStation': 'Station',
    'anomaly.colKind': 'Kind',
    'anomaly.colExpected': 'Expected ms',
    'anomaly.colActual': 'Actual ms',
    'anomaly.colDropped': 'Dropped≈',
    'anomaly.colSoc': 'SOC',
    'anomaly.colFrameTime': 'Frame time (BJT)',
    'anomaly.colFracsec': 'FRACSEC',
    'anomaly.toast': '{idcode}: timestamp {kind}, expected {expected}ms actual {actual}ms',
    'anomaly.copy': 'Copy',
    'anomaly.copied': 'Copied',
    'anomaly.exportFailed': 'Export failed: {error}',
    'anomaly.exportDone': 'Exported {n} rows',
    'anomaly.exportEmpty': 'Nothing to export',
    'anomaly.csvName': 'frame-anomalies.csv',
```

- [ ] **Step 5: 跑测试确认通过**

Run:
```bash
cd frontend && npx vitest run tests/use-pmu-events.anomaly.test.ts 2>&1 | tail -20
```
Expected: PASS。

- [ ] **Step 6: Commit**

```bash
git add frontend/src/composables/usePmuEvents.ts frontend/src/i18n/messages.ts frontend/tests/use-pmu-events.anomaly.test.ts
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(anomaly): 事件分发到异常面板 + i18n 文案"
```

---

## Task 6: AnomalyPanel.vue 组件 + 组件测试

**Files:**
- Create: `frontend/src/components/AnomalyPanel.vue`
- Create: `frontend/tests/anomaly-panel.test.ts`

**Interfaces:**
- Consumes: `useAnomalyLog`、`buildCsv` / `droppedFrames` / `kindI18nKey`（lib/anomaly）、`useToast`、`useI18n`、`@tauri-apps/plugin-dialog` 的 `save`、`invoke("save_text_file", ...)`。
- Produces: 默认导出组件 `AnomalyPanel`，供 Task 7 挂载。

- [ ] **Step 1: 写失败测试**

创建 `frontend/tests/anomaly-panel.test.ts`：

```ts
import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount } from "@vue/test-utils";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
const { save } = vi.hoisted(() => ({ save: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save }));

import AnomalyPanel from "../src/components/AnomalyPanel.vue";
import { useAnomalyLog } from "../src/composables/useAnomalyLog";
import type { PmuEvent } from "../src/types";

function ev(over: Partial<Extract<PmuEvent, { type: "TimestampAnomaly" }>> = {}) {
  return {
    type: "TimestampAnomaly",
    idcode: "PMU1",
    kind: "gap",
    expected_ms: 20,
    actual_ms: 40,
    soc: 1781,
    fracsec: 0x000d9490,
    frame_time: "2026-06-23 14:30:45",
    ...over,
  } as Extract<PmuEvent, { type: "TimestampAnomaly" }>;
}

beforeEach(() => {
  useAnomalyLog().clear();
  invoke.mockReset();
  save.mockReset();
});

describe("AnomalyPanel", () => {
  it("展开后渲染异常行，行数随数据增长", async () => {
    const { push } = useAnomalyLog();
    push(ev({ kind: "gap" }));
    push(ev({ kind: "backward", idcode: "PMU2" }));
    const wrapper = mount(AnomalyPanel);
    // 默认折叠，先展开
    await wrapper.find(".anomaly-header").trigger("click");
    expect(wrapper.findAll(".anomaly-row").length).toBe(2);
  });

  it("按类型筛选只显示该类型", async () => {
    const { push } = useAnomalyLog();
    push(ev({ kind: "gap" }));
    push(ev({ kind: "stall" }));
    const wrapper = mount(AnomalyPanel);
    await wrapper.find(".anomaly-header").trigger("click");
    await wrapper.find("select.filter-kind").setValue("gap");
    expect(wrapper.findAll(".anomaly-row").length).toBe(1);
  });

  it("清空后无行", async () => {
    const { push } = useAnomalyLog();
    push(ev());
    const wrapper = mount(AnomalyPanel);
    await wrapper.find(".anomaly-header").trigger("click");
    await wrapper.find("button.btn-clear").trigger("click");
    expect(wrapper.findAll(".anomaly-row").length).toBe(0);
  });

  it("空态时导出按钮禁用", async () => {
    const wrapper = mount(AnomalyPanel);
    await wrapper.find(".anomaly-header").trigger("click");
    expect(wrapper.find("button.btn-export").attributes("disabled")).toBeDefined();
  });

  it("有数据时导出调用 save 与 save_text_file", async () => {
    const { push } = useAnomalyLog();
    push(ev());
    save.mockResolvedValueOnce("/tmp/out.csv");
    invoke.mockResolvedValueOnce(undefined);
    const wrapper = mount(AnomalyPanel);
    await wrapper.find(".anomaly-header").trigger("click");
    await wrapper.find("button.btn-export").trigger("click");
    await Promise.resolve();
    await Promise.resolve();
    expect(save).toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith("save_text_file", expect.objectContaining({ path: "/tmp/out.csv" }));
  });

  it("gap 行显示丢帧估算", async () => {
    const { push } = useAnomalyLog();
    push(ev({ kind: "gap", expected_ms: 20, actual_ms: 60 }));
    const wrapper = mount(AnomalyPanel);
    await wrapper.find(".anomaly-header").trigger("click");
    expect(wrapper.find(".anomaly-row .col-dropped").text()).toContain("2");
  });
});
```

- [ ] **Step 2: 跑测试确认失败**

Run:
```bash
cd frontend && npx vitest run tests/anomaly-panel.test.ts 2>&1 | tail -20
```
Expected: FAIL —— 组件文件不存在。

- [ ] **Step 3: 实现 AnomalyPanel.vue**

创建 `frontend/src/components/AnomalyPanel.vue`：

```vue
<script setup lang="ts">
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { useAnomalyLog } from "../composables/useAnomalyLog";
import { useToast } from "../composables/useToast";
import { useI18n } from "../i18n";
import { buildCsv, droppedFrames, kindI18nKey } from "../lib/anomaly";
import type { AnomalyEntry } from "../types";

const { entries, clear, counts } = useAnomalyLog();
const { push: pushToast } = useToast();
const { t } = useI18n();

const collapsed = ref(true);
const filterKind = ref<string>("all");
const filterStation = ref<string>("all");
const expandedId = ref<number | null>(null);

// 拖拽高度（展开态）。
const panelHeight = ref(220);
let dragStartY = 0;
let dragStartH = 0;
function onDragStart(e: PointerEvent) {
  dragStartY = e.clientY;
  dragStartH = panelHeight.value;
  window.addEventListener("pointermove", onDragMove);
  window.addEventListener("pointerup", onDragEnd);
}
function onDragMove(e: PointerEvent) {
  // 顶边手柄上拖增高。
  const delta = dragStartY - e.clientY;
  panelHeight.value = Math.min(560, Math.max(120, dragStartH + delta));
}
function onDragEnd() {
  window.removeEventListener("pointermove", onDragMove);
  window.removeEventListener("pointerup", onDragEnd);
}

const stations = computed(() => {
  const set = new Set<string>();
  for (const e of entries) set.add(e.idcode);
  return [...set];
});

const filtered = computed(() =>
  entries.filter(
    (e) =>
      (filterKind.value === "all" || e.kind === filterKind.value) &&
      (filterStation.value === "all" || e.idcode === filterStation.value),
  ),
);

function kindLabel(kind: string): string {
  return t(kindI18nKey(kind));
}
function fracHex(f: number): string {
  return "0x" + (f >>> 0).toString(16).padStart(8, "0");
}
function droppedText(e: AnomalyEntry): string {
  return e.kind === "gap" ? "≈" + droppedFrames(e.actualMs, e.expectedMs) : "";
}
function toggleRow(id: number) {
  expandedId.value = expandedId.value === id ? null : id;
}
function rowDetail(e: AnomalyEntry): string {
  return [
    `${t("anomaly.colStation")}=${e.idcode}`,
    `${t("anomaly.colKind")}=${kindLabel(e.kind)}`,
    `${t("anomaly.colExpected")}=${e.expectedMs.toFixed(1)}`,
    `${t("anomaly.colActual")}=${e.actualMs.toFixed(1)}`,
    `SOC=${e.soc}`,
    `${t("anomaly.colFrameTime")}=${e.frameTime}`,
    `FRACSEC=${fracHex(e.fracsec)}`,
  ].join("  ");
}
async function copyDetail(e: AnomalyEntry) {
  try {
    await navigator.clipboard.writeText(rowDetail(e));
    pushToast(t("anomaly.copied"), "success");
  } catch {
    /* 剪贴板不可用时静默 */
  }
}

async function onExport() {
  if (!entries.length) {
    pushToast(t("anomaly.exportEmpty"), "info");
    return;
  }
  try {
    const path = await save({ defaultPath: t("anomaly.csvName") });
    if (!path) return; // 用户取消
    await invoke("save_text_file", { path, content: buildCsv(entries) });
    pushToast(t("anomaly.exportDone", { n: entries.length }), "success");
  } catch (e) {
    pushToast(t("anomaly.exportFailed", { error: String(e) }), "error");
  }
}
</script>

<template>
  <section class="anomaly-panel" :class="{ collapsed }" :style="!collapsed ? { height: panelHeight + 'px' } : undefined">
    <div v-if="!collapsed" class="drag-handle" @pointerdown="onDragStart"></div>
    <div class="anomaly-header" @click="collapsed = !collapsed">
      <span class="caret">{{ collapsed ? '▸' : '▾' }}</span>
      <span class="title">{{ t("anomaly.title") }}</span>
      <span class="badges" @click.stop>
        <span class="badge b-backward">{{ t("anomaly.count.backward", { n: counts.backward }) }}</span>
        <span class="badge b-gap">{{ t("anomaly.count.gap", { n: counts.gap }) }}</span>
        <span class="badge b-stall">{{ t("anomaly.count.stall", { n: counts.stall }) }}</span>
        <span class="badge b-total" :class="{ alert: counts.total > 0 }">{{ t("anomaly.count.total", { n: counts.total }) }}</span>
      </span>
      <span class="spacer"></span>
      <span v-if="!collapsed" class="tools" @click.stop>
        <select class="filter-kind" v-model="filterKind">
          <option value="all">{{ t("anomaly.filterKindAll") }}</option>
          <option value="backward">{{ t("anomaly.kind.backward") }}</option>
          <option value="gap">{{ t("anomaly.kind.gap") }}</option>
          <option value="stall">{{ t("anomaly.kind.stall") }}</option>
        </select>
        <select class="filter-station" v-model="filterStation">
          <option value="all">{{ t("anomaly.filterStationAll") }}</option>
          <option v-for="s in stations" :key="s" :value="s">{{ s }}</option>
        </select>
        <button class="btn-clear" @click="clear">{{ t("anomaly.clear") }}</button>
        <button class="btn-export" :disabled="!entries.length" @click="onExport">{{ t("anomaly.export") }}</button>
      </span>
    </div>

    <div v-if="!collapsed" class="anomaly-body">
      <table class="anomaly-table">
        <thead>
          <tr>
            <th>{{ t("anomaly.colTime") }}</th>
            <th>{{ t("anomaly.colStation") }}</th>
            <th>{{ t("anomaly.colKind") }}</th>
            <th>{{ t("anomaly.colExpected") }}</th>
            <th>{{ t("anomaly.colActual") }}</th>
            <th>{{ t("anomaly.colDropped") }}</th>
            <th>SOC</th>
            <th>{{ t("anomaly.colFrameTime") }}</th>
            <th>FRACSEC</th>
          </tr>
        </thead>
        <tbody>
          <template v-for="e in filtered" :key="e.id">
            <tr class="anomaly-row" :class="['k-' + e.kind, { selected: expandedId === e.id }]" @click="toggleRow(e.id)">
              <td>{{ e.localTime }}</td>
              <td>{{ e.idcode }}</td>
              <td class="col-kind">{{ kindLabel(e.kind) }}</td>
              <td class="num">{{ e.expectedMs.toFixed(1) }}</td>
              <td class="num">{{ e.actualMs.toFixed(1) }}</td>
              <td class="num col-dropped">{{ droppedText(e) }}</td>
              <td class="num">{{ e.soc }}</td>
              <td>{{ e.frameTime }}</td>
              <td class="mono">{{ fracHex(e.fracsec) }}</td>
            </tr>
            <tr v-if="expandedId === e.id" class="anomaly-detail">
              <td colspan="9">
                <span class="detail-text">{{ rowDetail(e) }}</span>
                <button class="btn-copy" @click.stop="copyDetail(e)">{{ t("anomaly.copy") }}</button>
              </td>
            </tr>
          </template>
          <tr v-if="!filtered.length" class="anomaly-empty">
            <td colspan="9">{{ t("anomaly.empty") }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>

<style scoped>
.anomaly-panel {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg-panel);
  border-top: 1px solid var(--border);
  margin: 0 8px 8px;
  border-radius: 4px;
  overflow: hidden;
  position: relative;
}
.anomaly-panel.collapsed { height: auto; }
.drag-handle {
  height: 5px;
  cursor: ns-resize;
  background: var(--border-soft);
  flex-shrink: 0;
}
.anomaly-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px;
  cursor: pointer;
  user-select: none;
  background: var(--bg-content);
  border-bottom: 1px solid var(--border-soft);
  flex-wrap: wrap;
}
.caret { width: 12px; color: var(--text-dim); }
.title { font-weight: 600; }
.spacer { flex: 1; }
.badges { display: flex; gap: 6px; }
.badge {
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 8px;
  background: var(--bg-disabled);
  color: var(--text-dim);
}
.badge.b-backward { color: var(--warn); }
.badge.b-gap { color: var(--err); }
.badge.b-stall { color: var(--text-dim); }
.badge.b-total.alert { background: var(--err); color: #fff; }
.tools { display: flex; gap: 6px; align-items: center; }
.tools select, .tools button {
  font-size: 11px;
  padding: 2px 6px;
  border: 1px solid var(--border-soft);
  border-radius: 3px;
  background: var(--bg-input);
  cursor: pointer;
}
.tools button:disabled { opacity: 0.5; cursor: default; }
.anomaly-body { flex: 1; overflow: auto; }
.anomaly-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
  font-family: "SF Mono", Menlo, Consolas, monospace;
}
.anomaly-table th {
  position: sticky;
  top: 0;
  background: var(--bg-content);
  text-align: left;
  padding: 3px 8px;
  border-bottom: 1px solid var(--border-soft);
  font-weight: 600;
  white-space: nowrap;
}
.anomaly-table td { padding: 2px 8px; border-bottom: 1px solid var(--border-soft); white-space: nowrap; }
.anomaly-table .num { text-align: right; }
.anomaly-row { cursor: pointer; }
.anomaly-row:hover { background: var(--accent-tint); }
.anomaly-row.selected { background: var(--accent-tint); }
.anomaly-row.k-backward .col-kind { color: var(--warn); }
.anomaly-row.k-gap .col-kind { color: var(--err); font-weight: 600; }
.anomaly-row.k-stall .col-kind { color: var(--text-dim); }
.anomaly-detail td { background: var(--bg-content); color: var(--text-dim); }
.detail-text { margin-right: 8px; }
.btn-copy {
  font-size: 11px;
  padding: 1px 6px;
  border: 1px solid var(--border-soft);
  border-radius: 3px;
  background: var(--bg-input);
  cursor: pointer;
}
.anomaly-empty td { text-align: center; color: var(--text-faint); padding: 12px; }
</style>
```

- [ ] **Step 4: 跑测试确认通过**

Run:
```bash
cd frontend && npx vitest run tests/anomaly-panel.test.ts 2>&1 | tail -30
```
Expected: PASS（6 个用例全绿）。若 happy-dom 缺 `navigator.clipboard`，copy 用例未触发不受影响（导出用例已 mock）。

- [ ] **Step 5: Commit**

```bash
git add frontend/src/components/AnomalyPanel.vue frontend/tests/anomaly-panel.test.ts
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(anomaly): AnomalyPanel 面板组件(筛选/导出/展开)"
```

---

## Task 7: App.vue 挂载 + 全量验证

**Files:**
- Modify: `frontend/src/App.vue:6-8, 85-88`

**Interfaces:**
- Consumes: `AnomalyPanel` 默认导出。

- [ ] **Step 1: import AnomalyPanel**

在 `frontend/src/App.vue` 第 8 行 `import UpdateDialog from "./components/UpdateDialog.vue";` 之后追加：

```ts
import AnomalyPanel from "./components/AnomalyPanel.vue";
```

- [ ] **Step 2: 在 content 之后挂载面板**

把 `frontend/src/App.vue` 第 85-88 行：

```vue
    <div class="content">
      <ConfigInfoPanel />
      <DataTablePanel />
    </div>
```

改为（紧随其后加面板）：

```vue
    <div class="content">
      <ConfigInfoPanel />
      <DataTablePanel />
    </div>

    <AnomalyPanel />
```

`.app` 已是 `flex-direction: column`；`AnomalyPanel` 根元素 `flex-shrink: 0`，折叠态仅占标题栏高度、展开态占 `panelHeight`，上方 `.content` 保持 `flex: 1` 自适应，无需改动 App.vue 的 `<style>`。

- [ ] **Step 3: 类型检查 + 构建**

Run:
```bash
cd frontend && npm run build 2>&1 | tail -25
```
Expected: `vue-tsc` 无类型错误，`vite build` 成功产出。

- [ ] **Step 4: 全量前端测试**

Run:
```bash
cd frontend && npm run test:unit 2>&1 | tail -25
```
Expected: 全部测试套件 PASS（含已有的 reconnect 等 + 新增 4 个文件）。

- [ ] **Step 5: 后端全量测试 + 构建**

Run:
```bash
cargo test 2>&1 | tail -20
cargo build 2>&1 | tail -10
```
Expected: 全绿、构建成功。

- [ ] **Step 6: 手动验证（人工）**

```bash
cd frontend && npm run tauri dev
```
检查：①底部出现「异常报文跳帧」折叠条，计数徽章为 0；②触发一次跳帧（连真实/模拟子站并制造丢帧）→ 徽章 +1、total 变红、右下角弹 toast、生命周期事件日志**不**出现该异常；③点标题展开看到对应行、丢帧≈ 有值、点行展开详情可复制；④类型/子站筛选生效；⑤点导出 CSV 选路径，打开文件内容正确；⑥拖动顶边手柄可调高度。

- [ ] **Step 7: Commit**

```bash
git add frontend/src/App.vue
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(anomaly): 主界面挂载异常报文跳帧面板"
```

---

## 自审清单（写计划后核对，实现时无需重跑）

- **Spec 覆盖**：①结构化事件→Task1；②save_text_file→Task2；③类型+纯函数(丢帧/CSV)→Task3；④useAnomalyLog(FIFO/计数)→Task4；⑤事件分发不进日志+toast+i18n→Task5；⑥面板(筛选/导出/展开/折叠/拖高/计数徽章)→Task6+7；⑦测试与验收→各 Task + Task7。全部命中。
- **类型一致**：事件字段 snake_case（`expected_ms`/`actual_ms`/`frame_time`）在 events.rs(Task1)、types(Task3)、useAnomalyLog(Task4)、测试中一致；条目 camelCase（`expectedMs`/`actualMs`/`frameTime`）在 types/composable/组件/lib 一致；`counts` 形状 `{backward,gap,stall,total}` 跨 Task4/6 一致；`kindI18nKey` 返回的 key 与 messages.ts 的 key 一致。
- **无占位符**：所有步骤含完整代码与命令。
```
