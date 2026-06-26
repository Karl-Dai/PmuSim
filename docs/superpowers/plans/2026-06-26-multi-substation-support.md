# 主站多子站接收 + 相量可视化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让主站 UI 能同时接收多个子站数据、用左侧列表切换查看(全部面板跟随选中子站),并把一直被丢弃的相量数据可视化。

**Architecture:** 后端不动(已是 `HashMap<idcode, Session>` 多会话)。前端把所有按子站维度的运行态(最新数据/帧率/时偏/事件日志/重连)从单例改为按 `idcode`(重连按拨号目标 `dialKey=host:mgmtPort`)键控,每个面板读 `xxxOf(selectedIdcode)`;新增机架+LED 子站列表当切换器;解开连接流程对单一会话的把守。再补相量行 + 实时极坐标相量图。

**Tech Stack:** Vue 3 (`<script setup>` + 模块级 reactive 单例 composable,无 Pinia)、TypeScript、Vite、Vitest + happy-dom + @vue/test-utils。Tauri 命令经 `invoke` 调后端。

## Global Constraints

- 面向用户文本一律走 i18n(`t("key")`),中英双语,key 加在 `frontend/src/i18n/messages.ts` 的 `zh` 与 `en` 两段。
- composable 为**模块级 reactive 单例**;新增/改动需提供按 idcode 复位的能力,测试在 `beforeEach` 复位。
- 测试放 `frontend/tests/*.test.ts`,happy-dom 环境;Tauri `invoke` 用 `vi.hoisted` + `vi.mock("@tauri-apps/api/core", …)` 打桩;组件用 `@vue/test-utils` `mount`,`setLocale("en")`。
- 命令统一在 `frontend/` 下用 `npm` 执行:`npm run test:unit`。
- 零圆角发丝边 + 阳极氧化金属铭牌的现有视觉语言不破坏(本计划不做 D2/D3/D5/D6 视觉打磨)。
- git 提交作者必须 `Karl-Dai Karl <kelsoprotein@gmail.com>`,提交信息禁止任何 AI 署名行。每个 Task 末尾提交一次。
- 重连 `dialKey` 跨"占位 idcode→真实 idcode"re-key 的传递依赖后端先发 `SessionCreated(占位 host:port)` 再发 `SessionCreated(真实 idcode)` 的顺序;A7 用表征测试锁住此假设。

## File Structure

新建:
- `frontend/src/components/StationListPanel.vue` — 左侧子站列表(机架+LED+实时 fps,点击切换 `selectedIdcode`,每行断开/重连)。
- `frontend/src/components/PhasorPlot.vue` — 选中子站的实时极坐标相量图(canvas)。
- `frontend/src/lib/phasor.ts` — 相量纯函数(直角↔极坐标、角度归一)。
- 对应测试:`frontend/tests/station-list-panel.test.ts`、`frontend/tests/phasor.test.ts`、`frontend/tests/phasor-plot.test.ts`。

改造(按 idcode 键控):
- `frontend/src/composables/useFrameRate.ts`、`useTimeOffset.ts`、`useCommLog.ts`、`useEventLog.ts`、`useReconnect.ts`、`useSessions.ts`、`usePmuEvents.ts`。
- `frontend/src/components/ConfigInfoPanel.vue`、`DataTablePanel.vue`、`AnomalyPanel.vue`、`App.vue`。
- `frontend/src/types/index.ts`(`SessionInfo.dialKey`)。

受影响的现有测试(随对应 Task 同步更新):`use-frame-rate.test.ts`、`use-time-offset.test.ts`、`use-reconnect.test.ts`、`use-pmu-events.reconnect.test.ts`、`config-info-panel.reconnect.test.ts`、`config-info-panel.0hz.test.ts`。

---

# Phase A — 多子站接收(A1–A11,可独立交付)

## Task A1: `useFrameRate` 按 idcode 键控

**Files:**
- Modify: `frontend/src/composables/useFrameRate.ts`
- Test: `frontend/tests/use-frame-rate.test.ts`

**Interfaces:**
- Produces: `useFrameRate(): { tick(idcode: string, tsMs: number): void; reset(idcode: string): void; fpsOf(idcode: string): number }`

- [x] **Step 1: 改写测试为按 idcode**

把 `frontend/tests/use-frame-rate.test.ts` 中 `describe("useFrameRate …")` 整块替换为(顶部 `frameTimeMs` 那个 describe 保持不变):

```ts
describe("useFrameRate 基于报文时间戳反推帧率(按 idcode)", () => {
  beforeEach(() => {
    const { reset } = useFrameRate();
    reset("A");
    reset("B");
  });

  it("以最近 1 秒(报文时间)内的帧数为 fps", () => {
    const { tick, fpsOf } = useFrameRate();
    for (let i = 0; i < 150; i++) tick("A", i * 10);
    expect(fpsOf("A")).toBe(101);
  });

  it("两个子站各自独立计数,互不串台", () => {
    const { tick, fpsOf } = useFrameRate();
    for (let i = 0; i < 150; i++) tick("A", i * 10); // 101
    tick("B", 0);
    tick("B", 500);
    expect(fpsOf("A")).toBe(101);
    expect(fpsOf("B")).toBe(2);
  });

  it("报文时间倒退时重置该 idcode 窗口", () => {
    const { tick, fpsOf } = useFrameRate();
    tick("A", 100_000);
    tick("A", 100_010);
    tick("A", 5); // 倒退 → 仅保留当前帧
    expect(fpsOf("A")).toBe(1);
  });

  it("reset(idcode) 清零且不影响其他 idcode", () => {
    const { tick, fpsOf, reset } = useFrameRate();
    tick("A", 0);
    tick("B", 0);
    reset("A");
    expect(fpsOf("A")).toBe(0);
    expect(fpsOf("B")).toBe(1);
  });

  it("未知 idcode 读数为 0", () => {
    expect(useFrameRate().fpsOf("nope")).toBe(0);
  });
});
```

- [x] **Step 2: 跑测试确认失败**

Run: `cd frontend && npm run test:unit -- use-frame-rate`
Expected: FAIL(`tick` 参数不匹配 / `fpsOf` 未定义)

- [x] **Step 3: 改写 `useFrameRate.ts` 全文**

```ts
import { reactive } from "vue";

// 按 idcode 维护滑动窗的帧率。窗口数组(plain)按 idcode 隔离;只把派生的
// fps 标量放进 reactive Map,供面板按选中子站读取。语义同单子站版:基于
// 报文 SOC/FRACSEC 时间戳(frameTimeMs)反推,报文时间倒退即重置该窗口。
const WINDOW_MS = 1000;
const windows = new Map<string, number[]>();
const fpsMap = reactive(new Map<string, number>());

export function useFrameRate() {
  function tick(idcode: string, tsMs: number) {
    let recent = windows.get(idcode);
    if (!recent) {
      recent = [];
      windows.set(idcode, recent);
    }
    if (recent.length > 0 && tsMs < recent[recent.length - 1]) recent.length = 0;
    recent.push(tsMs);
    const cutoff = tsMs - WINDOW_MS;
    while (recent.length > 0 && recent[0] < cutoff) recent.shift();
    fpsMap.set(idcode, recent.length);
  }
  function reset(idcode: string) {
    windows.delete(idcode);
    fpsMap.set(idcode, 0);
  }
  function fpsOf(idcode: string): number {
    return fpsMap.get(idcode) ?? 0;
  }
  return { tick, reset, fpsOf };
}
```

- [x] **Step 4: 跑测试确认通过**

Run: `cd frontend && npm run test:unit -- use-frame-rate`
Expected: PASS

- [x] **Step 5: 提交**

```bash
git add frontend/src/composables/useFrameRate.ts frontend/tests/use-frame-rate.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "refactor(frontend): useFrameRate 按 idcode 键控"
```

---

## Task A2: `useTimeOffset` 按 idcode 键控

**Files:**
- Modify: `frontend/src/composables/useTimeOffset.ts`
- Test: `frontend/tests/use-time-offset.test.ts`

**Interfaces:**
- Produces: `useTimeOffset(): { tick(idcode: string, ms: number): void; reset(idcode: string): void; offsetOf(idcode: string): number | null }`

- [x] **Step 1: 改写测试为按 idcode**

把 `frontend/tests/use-time-offset.test.ts` 全文替换为:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { useTimeOffset } from "../src/composables/useTimeOffset";

describe("useTimeOffset 报文-本机时偏滑动均值(按 idcode)", () => {
  beforeEach(() => {
    const { reset } = useTimeOffset();
    reset("A");
    reset("B");
  });

  it("均值随样本更新,正负保留", () => {
    const { tick, offsetOf } = useTimeOffset();
    tick("A", 10);
    tick("A", -10);
    tick("A", 30);
    expect(offsetOf("A")).toBeCloseTo(10, 5);
  });

  it("两个子站各自独立,互不串台", () => {
    const { tick, offsetOf } = useTimeOffset();
    tick("A", 100);
    tick("B", -50);
    expect(offsetOf("A")).toBe(100);
    expect(offsetOf("B")).toBe(-50);
  });

  it("无样本 / 未知 idcode 读数为 null", () => {
    const { offsetOf, reset } = useTimeOffset();
    reset("A");
    expect(offsetOf("A")).toBeNull();
    expect(offsetOf("nope")).toBeNull();
  });
});
```

- [x] **Step 2: 跑测试确认失败**

Run: `cd frontend && npm run test:unit -- use-time-offset`
Expected: FAIL

- [x] **Step 3: 改写 `useTimeOffset.ts` 全文**

```ts
import { reactive } from "vue";

// 报文时间与本机时钟偏差(ms)按 idcode 隔离的定长滑动均值。每帧偏差由后端
// 采样写入 DataInfo.local_offset_ms。正=报文滞后本地,负=超前。
const WINDOW = 50;
const windows = new Map<string, number[]>();
const offsetMap = reactive(new Map<string, number | null>());

export function useTimeOffset() {
  function tick(idcode: string, ms: number) {
    let samples = windows.get(idcode);
    if (!samples) {
      samples = [];
      windows.set(idcode, samples);
    }
    samples.push(ms);
    if (samples.length > WINDOW) samples.shift();
    const sum = samples.reduce((a, b) => a + b, 0);
    offsetMap.set(idcode, sum / samples.length);
  }
  function reset(idcode: string) {
    windows.delete(idcode);
    offsetMap.set(idcode, null);
  }
  function offsetOf(idcode: string): number | null {
    return offsetMap.get(idcode) ?? null;
  }
  return { tick, reset, offsetOf };
}
```

- [x] **Step 4: 跑测试确认通过**

Run: `cd frontend && npm run test:unit -- use-time-offset`
Expected: PASS

- [x] **Step 5: 提交**

```bash
git add frontend/src/composables/useTimeOffset.ts frontend/tests/use-time-offset.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "refactor(frontend): useTimeOffset 按 idcode 键控"
```

---

## Task A3: `useCommLog` 最新数据帧改 Map

**Files:**
- Modify: `frontend/src/composables/useCommLog.ts`
- Test: `frontend/tests/use-comm-log.test.ts`(新建)

**Interfaces:**
- Consumes: `DataInfo`(`../types`)。
- Produces: `useCommLog(): { logs; addLog(idcode,direction,summary,hex?); addData(idcode: string, data: DataInfo): void; latestOf(idcode: string): DataInfo | undefined; clear(): void }`

- [x] **Step 1: 写失败测试**

新建 `frontend/tests/use-comm-log.test.ts`:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { useCommLog } from "../src/composables/useCommLog";
import type { DataInfo } from "../src/types";

function mkData(stat: number): DataInfo {
  return {
    soc: 0, fracsec: 0, stat, format_flags: 1, time_quality: 0,
    freq: 0, dfreq: 0, analog: [], digital: [], phasors: [], local_offset_ms: 0,
  };
}

describe("useCommLog 按 idcode 存最新数据帧", () => {
  beforeEach(() => useCommLog().clear());

  it("不同子站的数据帧互不覆盖", () => {
    const { addData, latestOf } = useCommLog();
    addData("A", mkData(1));
    addData("B", mkData(2));
    addData("A", mkData(3));
    expect(latestOf("A")?.stat).toBe(3);
    expect(latestOf("B")?.stat).toBe(2);
  });

  it("未知 idcode 返回 undefined", () => {
    expect(useCommLog().latestOf("nope")).toBeUndefined();
  });

  it("clear() 清空", () => {
    const { addData, latestOf, clear } = useCommLog();
    addData("A", mkData(1));
    clear();
    expect(latestOf("A")).toBeUndefined();
  });
});
```

- [x] **Step 2: 跑测试确认失败**

Run: `cd frontend && npm run test:unit -- use-comm-log`
Expected: FAIL(`latestOf` 未定义)

- [x] **Step 3: 改 `useCommLog.ts`**

把第 13 行 `const latestData = …` 起到文件结尾替换为:

```ts
const latestByIdcode = reactive(new Map<string, DataInfo>());
const MAX_LOGS = 1000;

export function useCommLog() {
  function addLog(idcode: string, direction: string, summary: string, hex?: string) {
    const now = new Date();
    const time = `${now.getHours().toString().padStart(2, "0")}:${now.getMinutes().toString().padStart(2, "0")}:${now.getSeconds().toString().padStart(2, "0")}`;
    logs.unshift({ time, idcode, direction, summary, hex });
    if (logs.length > MAX_LOGS) logs.splice(MAX_LOGS);
  }

  function addData(idcode: string, data: DataInfo) {
    latestByIdcode.set(idcode, data);
  }

  function latestOf(idcode: string): DataInfo | undefined {
    return latestByIdcode.get(idcode);
  }

  function clear() {
    logs.splice(0);
    latestByIdcode.clear();
  }

  return { logs, addLog, addData, latestOf, clear };
}
```

(`import { ref, reactive }` 改为 `import { reactive }`,删掉已不用的 `ref`。)

- [x] **Step 4: 跑测试确认通过**

Run: `cd frontend && npm run test:unit -- use-comm-log`
Expected: PASS

- [x] **Step 5: 提交**

```bash
git add frontend/src/composables/useCommLog.ts frontend/tests/use-comm-log.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "refactor(frontend): useCommLog 最新数据帧按 idcode 存"
```

> 注:`DataTablePanel.vue` / `ConfigInfoPanel.vue` 当前 `import { latestData }` 会编译失败 —— 由 A9/A10 修复;在执行到那两个 Task 前 `npm run build` 会红,单测不受影响。

---

## Task A4: `useEventLog` 加 idcode 字段

**Files:**
- Modify: `frontend/src/composables/useEventLog.ts`
- Test: `frontend/tests/use-event-log.test.ts`(新建)

**Interfaces:**
- Produces: `EventLogEntry { time: string; idcode: string; message: string; kind: "info"|"error" }`;`useEventLog(): { events; push(idcode: string, message: string, kind?): void; entriesFor(idcode: string): EventLogEntry[]; clear(): void }`
- `entriesFor(idcode)` 返回 `idcode === 入参` 或 `idcode === ""`(广播/无归属)的条目。

- [x] **Step 1: 写失败测试**

新建 `frontend/tests/use-event-log.test.ts`:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { useEventLog } from "../src/composables/useEventLog";

describe("useEventLog 按 idcode 过滤", () => {
  beforeEach(() => useEventLog().clear());

  it("entriesFor 只返回该子站 + 广播(空 idcode)条目", () => {
    const { push, entriesFor } = useEventLog();
    push("A", "a1");
    push("B", "b1");
    push("", "broadcast", "error");
    const a = entriesFor("A").map((e) => e.message);
    expect(a).toContain("a1");
    expect(a).toContain("broadcast");
    expect(a).not.toContain("b1");
  });

  it("kind 默认 info", () => {
    const { push, entriesFor } = useEventLog();
    push("A", "x");
    expect(entriesFor("A")[0].kind).toBe("info");
  });
});
```

- [x] **Step 2: 跑测试确认失败**

Run: `cd frontend && npm run test:unit -- use-event-log`
Expected: FAIL

- [x] **Step 3: 改 `useEventLog.ts`**

`EventLogEntry` 接口加 `idcode: string`(放在 `time` 之后);`push` 与返回值替换为:

```ts
export interface EventLogEntry {
  time: string; // "YYYY/MM/DD HH:MM:SS"
  idcode: string;
  message: string;
  kind: "info" | "error";
}
```

```ts
export function useEventLog() {
  function push(idcode: string, message: string, kind: EventLogEntry["kind"] = "info") {
    events.unshift({ time: now(), idcode, message, kind });
    if (events.length > MAX_ENTRIES) events.splice(MAX_ENTRIES);
  }
  function entriesFor(idcode: string): EventLogEntry[] {
    return events.filter((e) => e.idcode === idcode || e.idcode === "");
  }
  function clear() {
    events.splice(0);
  }
  return { events, push, entriesFor, clear };
}
```

- [x] **Step 4: 跑测试确认通过**

Run: `cd frontend && npm run test:unit -- use-event-log`
Expected: PASS

- [x] **Step 5: 提交**

```bash
git add frontend/src/composables/useEventLog.ts frontend/tests/use-event-log.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "refactor(frontend): useEventLog 加 idcode 并按子站过滤"
```

> 注:`usePmuEvents.ts`(多处 `pushEvent(...)`)与 `ConfigInfoPanel.vue`(`v-for events`)将在 A7/A9 适配。

---

## Task A5: `useReconnect` 改为按 dialKey 的多目标 FSM

**Files:**
- Modify: `frontend/src/composables/useReconnect.ts`
- Test: `frontend/tests/use-reconnect.test.ts`(整文件替换)

**Interfaces:**
- `ReconnectTarget`(不变):`{ host; mgmtPort; dataPort; protocol: "V2"|"V3"; period: number|null; mode: "normal"|"skipCfg2" }`
- Produces: `useReconnect(): { arm(dialKey: string, t: ReconnectTarget): void; onDisconnect(dialKey: string, wasStreaming: boolean): void; cancel(dialKey: string): void; cancelAll(): void; reconnectingOf(dialKey: string): boolean; reconnecting: Ref<boolean> /*任一在重连*/; _resetForTest(): void }`
- `dialKey` 约定 = `${host}:${mgmtPort}`,与连接占位 idcode 一致。

- [x] **Step 1: 整文件替换测试 `use-reconnect.test.ts`**

```ts
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { useReconnect, type ReconnectTarget } from "../src/composables/useReconnect";

const r = useReconnect();

function target(host: string, mode: "normal" | "skipCfg2" = "normal"): ReconnectTarget {
  return { host, mgmtPort: 8000, dataPort: 8001, protocol: "V3", period: 50, mode };
}

beforeEach(() => {
  r._resetForTest();
  invoke.mockReset();
  invoke.mockResolvedValue(undefined);
  vi.useFakeTimers();
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("useReconnect 按 dialKey 多目标重连", () => {
  it("arm 后断线(streaming) → 退避后 connect + auto_handshake", async () => {
    r.arm("h1:8000", target("h1"));
    r.onDisconnect("h1:8000", true);
    expect(r.reconnectingOf("h1:8000")).toBe(true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledWith("connect_substation", { host: "h1", port: 8000, dataPort: 8001 });
    expect(invoke).toHaveBeenCalledWith("auto_handshake", { idcode: "h1:8000", period: 50 });
    expect(r.reconnectingOf("h1:8000")).toBe(false);
  });

  it("两个子站重连互不影响", async () => {
    r.arm("h1:8000", target("h1"));
    r.arm("h2:8000", target("h2"));
    r.onDisconnect("h1:8000", false);
    expect(r.reconnectingOf("h1:8000")).toBe(true);
    expect(r.reconnectingOf("h2:8000")).toBe(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledWith("connect_substation", { host: "h1", port: 8000, dataPort: 8001 });
    expect(invoke).not.toHaveBeenCalledWith("connect_substation", { host: "h2", port: 8000, dataPort: 8001 });
  });

  it("skipCfg2 模式重连走 skip_cfg2_open", async () => {
    r.arm("h1:8000", target("h1", "skipCfg2"));
    r.onDisconnect("h1:8000", true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledWith("skip_cfg2_open", { idcode: "h1:8000" });
  });

  it("cancel(dialKey) 停止该目标重连", async () => {
    r.arm("h1:8000", target("h1"));
    r.onDisconnect("h1:8000", true);
    r.cancel("h1:8000");
    expect(r.reconnectingOf("h1:8000")).toBe(false);
    await vi.advanceTimersByTimeAsync(5000);
    expect(invoke).not.toHaveBeenCalled();
  });

  it("connect 失败则指数退避重试", async () => {
    invoke.mockRejectedValueOnce(new Error("down")).mockResolvedValue(undefined);
    r.arm("h1:8000", target("h1"));
    r.onDisconnect("h1:8000", false);
    await vi.advanceTimersByTimeAsync(1000); // 第1次失败
    expect(r.reconnectingOf("h1:8000")).toBe(true);
    await vi.advanceTimersByTimeAsync(2000); // 退避到 2s 第2次成功
    expect(r.reconnectingOf("h1:8000")).toBe(false);
  });

  it("未 arm 的 dialKey onDisconnect 无副作用", async () => {
    r.onDisconnect("ghost:8000", true);
    await vi.advanceTimersByTimeAsync(5000);
    expect(invoke).not.toHaveBeenCalled();
  });
});
```

- [x] **Step 2: 跑测试确认失败**

Run: `cd frontend && npm run test:unit -- use-reconnect`
Expected: FAIL

- [x] **Step 3: 整文件替换 `useReconnect.ts`**

```ts
import { reactive, computed, type ComputedRef } from "vue";
import { invoke } from "@tauri-apps/api/core";

export type ReconnectMode = "normal" | "skipCfg2";

export interface ReconnectTarget {
  host: string;
  mgmtPort: number;
  dataPort: number;
  protocol: "V2" | "V3";
  period: number | null;
  mode: ReconnectMode;
}

const BASE_DELAY_MS = 1_000;
const MAX_DELAY_MS = 30_000;

interface RState {
  desired: ReconnectTarget;
  intentional: boolean;
  attempt: number;
  pendingStreaming: boolean;
  timer: ReturnType<typeof setTimeout> | null;
}

// dialKey(= `${host}:${mgmtPort}`,与连接占位 idcode 一致) → 该目标的重连 FSM。
const states = new Map<string, RState>();
// 正在重连的 dialKey 集合(reactive,供 LED / 状态读数响应)。
const reconnectingKeys = reactive(new Set<string>());

function delayFor(a: number): number {
  return Math.min(BASE_DELAY_MS * 2 ** a, MAX_DELAY_MS);
}

function clearTimer(s: RState): void {
  if (s.timer !== null) {
    clearTimeout(s.timer);
    s.timer = null;
  }
}

async function attemptReconnect(dialKey: string): Promise<void> {
  const s = states.get(dialKey);
  if (!s) return;
  s.timer = null;
  const t = s.desired;
  try {
    await invoke("connect_substation", {
      host: t.host,
      port: t.mgmtPort,
      dataPort: t.protocol === "V3" ? t.dataPort : undefined,
    });
    if (s.pendingStreaming) {
      if (t.mode === "skipCfg2") {
        await invoke("skip_cfg2_open", { idcode: dialKey });
      } else {
        await invoke("auto_handshake", { idcode: dialKey, period: t.period });
      }
    }
    s.attempt = 0;
    reconnectingKeys.delete(dialKey);
  } catch {
    s.attempt += 1;
    scheduleRetry(dialKey);
  }
}

function scheduleRetry(dialKey: string): void {
  const s = states.get(dialKey);
  if (!s) return;
  clearTimer(s);
  s.timer = setTimeout(() => void attemptReconnect(dialKey), delayFor(s.attempt));
}

export function useReconnect() {
  function arm(dialKey: string, t: ReconnectTarget): void {
    const existing = states.get(dialKey);
    if (existing) clearTimer(existing);
    states.set(dialKey, {
      desired: t,
      intentional: false,
      attempt: 0,
      pendingStreaming: false,
      timer: null,
    });
  }
  function onDisconnect(dialKey: string, wasStreaming: boolean): void {
    const s = states.get(dialKey);
    if (!s || s.intentional) return;
    s.pendingStreaming = wasStreaming;
    reconnectingKeys.add(dialKey);
    scheduleRetry(dialKey);
  }
  function cancel(dialKey: string): void {
    const s = states.get(dialKey);
    if (!s) return;
    s.intentional = true;
    clearTimer(s);
    s.attempt = 0;
    reconnectingKeys.delete(dialKey);
  }
  function cancelAll(): void {
    for (const key of [...states.keys()]) cancel(key);
  }
  function reconnectingOf(dialKey: string): boolean {
    return reconnectingKeys.has(dialKey);
  }
  function _resetForTest(): void {
    for (const s of states.values()) clearTimer(s);
    states.clear();
    reconnectingKeys.clear();
  }
  const reconnecting: ComputedRef<boolean> = computed(() => reconnectingKeys.size > 0);
  return { arm, onDisconnect, cancel, cancelAll, reconnectingOf, reconnecting, _resetForTest };
}
```

- [x] **Step 4: 跑测试确认通过**

Run: `cd frontend && npm run test:unit -- use-reconnect`
Expected: PASS

- [x] **Step 5: 提交**

```bash
git add frontend/src/composables/useReconnect.ts frontend/tests/use-reconnect.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "refactor(frontend): useReconnect 改按 dialKey 多目标重连"
```

> 注:`useReconnect.reconnecting` 由 `Ref<boolean>` 变为 `ComputedRef<boolean>`(只读),消费侧仍 `.value`,行为兼容。`onDisconnect`/`arm`/`cancel` 签名变化由 A7/A9 适配。

---

## Task A6: `useSessions` 加 dialKey、新增即选中、移除回退

**Files:**
- Modify: `frontend/src/types/index.ts:1-5`、`frontend/src/composables/useSessions.ts`
- Test: `frontend/tests/use-sessions.test.ts`(新建)

**Interfaces:**
- `SessionInfo` 加 `dialKey?: string`。
- `addSession(idcode, peerIp, dialKey?)`:新增的会话**总是**成为 `selectedIdcode`。
- `removeSession(idcode)`:若移除的是选中项,回退到剩余会话的第一个,无则置空。
- 新增 `setDialKey(idcode: string, dialKey: string): void`。

- [x] **Step 1: 写失败测试**

新建 `frontend/tests/use-sessions.test.ts`:

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { useSessions } from "../src/composables/useSessions";

describe("useSessions 多子站选中管理", () => {
  beforeEach(() => useSessions().clear());

  it("新增会话即成为选中项", () => {
    const { addSession, selectedIdcode } = useSessions();
    addSession("A", "1.1.1.1");
    expect(selectedIdcode.value).toBe("A");
    addSession("B", "2.2.2.2");
    expect(selectedIdcode.value).toBe("B");
  });

  it("移除选中项回退到剩余第一个", () => {
    const { addSession, removeSession, selectedIdcode } = useSessions();
    addSession("A", "1.1.1.1");
    addSession("B", "2.2.2.2");
    selectedIdcode.value = "A";
    removeSession("A");
    expect(selectedIdcode.value).toBe("B");
  });

  it("移除最后一个会话置空选中", () => {
    const { addSession, removeSession, selectedIdcode } = useSessions();
    addSession("A", "1.1.1.1");
    removeSession("A");
    expect(selectedIdcode.value).toBe("");
  });

  it("setDialKey 写入会话", () => {
    const { addSession, setDialKey, sessions } = useSessions();
    addSession("A", "1.1.1.1");
    setDialKey("A", "1.1.1.1:8000");
    expect(sessions.get("A")?.dialKey).toBe("1.1.1.1:8000");
  });
});
```

- [x] **Step 2: 跑测试确认失败**

Run: `cd frontend && npm run test:unit -- use-sessions`
Expected: FAIL(`setDialKey` 未定义 / 选中逻辑不符)

- [x] **Step 3a: 改 `types/index.ts` 的 `SessionInfo`**

```ts
export interface SessionInfo {
  idcode: string;
  peerIp: string;
  state: "connecting" | "connected" | "cfg1_received" | "cfg2_sent" | "streaming" | "disconnected";
  /** 拨号目标 `${host}:${mgmtPort}`,跨 re-key 稳定,用于按目标重连。 */
  dialKey?: string;
}
```

- [x] **Step 3b: 改 `useSessions.ts`**

`addSession` 签名与体、`removeSession`、新增 `setDialKey` 替换为:

```ts
  function addSession(idcode: string, peerIp: string, dialKey?: string) {
    const state: SessionInfo["state"] = idcode.includes(":") ? "connecting" : "connected";
    sessions.set(idcode, { idcode, peerIp, state, dialKey });
    // 新增即选中,让用户立刻看到最新连入的子站握手/推流。
    selectedIdcode.value = idcode;
  }
  function setDialKey(idcode: string, dialKey: string) {
    const s = sessions.get(idcode);
    if (s) s.dialKey = dialKey;
  }
  function removeSession(idcode: string) {
    sessions.delete(idcode);
    configs.delete(idcode);
    if (selectedIdcode.value === idcode) {
      const next = sessions.keys().next();
      selectedIdcode.value = next.done ? "" : next.value;
    }
  }
```

并在 `return { … }` 加入 `setDialKey`:
`return { sessions, configs, selectedIdcode, addSession, setDialKey, updateState, removeSession, setConfig, clear };`

- [x] **Step 4: 跑测试确认通过**

Run: `cd frontend && npm run test:unit -- use-sessions`
Expected: PASS

- [x] **Step 5: 提交**

```bash
git add frontend/src/types/index.ts frontend/src/composables/useSessions.ts frontend/tests/use-sessions.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(frontend): useSessions 支持 dialKey 与多子站选中"
```

---

## Task A7: `usePmuEvents` 全量按 idcode 路由 + dialKey 解析

**Files:**
- Modify: `frontend/src/composables/usePmuEvents.ts`
- Test: `frontend/tests/use-pmu-events.reconnect.test.ts`(更新)、`frontend/tests/use-pmu-events.multi.test.ts`(新建)

**Interfaces:**
- Consumes: A1–A6 的新签名(`tick(idcode,…)`、`addData(idcode,…)`、`pushEvent(idcode,…)`、`reconnect.onDisconnect(dialKey,…)`、`useSessions.setDialKey/addSession`)。
- 行为:
  - `SessionCreated(占位 host:port)`:`addSession(idcode, peer_ip, idcode)`(占位 idcode 即 dialKey),并记 `pendingDialKey = idcode`。
  - `SessionCreated(真实 idcode)`:`addSession(idcode, peer_ip, pendingDialKey ?? undefined)`,随后 `pendingDialKey = null`。
  - `SessionDisconnected` / `HeartbeatTimeout`:用 `sessions.get(idcode)?.dialKey` → `reconnect.onDisconnect(dialKey, wasStreaming)`(无 dialKey 或占位则不重连)。
  - 所有 `tickFrameRate/tickOffset/resetFrameRate/resetOffset/pushEvent` 带 `idcode`。

- [x] **Step 1: 更新 `use-pmu-events.reconnect.test.ts`**

该文件断言 `onDisconnect` 调用,签名已变。把三个 `it` 的断言改为带 dialKey,并给 streaming 用例补 dialKey:

```ts
function seedSession(idcode: string, state: string, dialKey?: string) {
  const { sessions } = useSessions();
  sessions.set(idcode, { idcode, peerIp: "1.1.1.1", state: state as never, dialKey });
}

describe("usePmuEvents 断线触发自动重连", () => {
  it("真实会话 SessionDisconnected(streaming) → onDisconnect(dialKey,true)", async () => {
    const spy = vi.spyOn(reconnect, "onDisconnect");
    seedSession("PMU1", "streaming", "1.1.1.1:8000");
    invoke.mockResolvedValueOnce([{ type: "SessionDisconnected", idcode: "PMU1" }]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);
    expect(spy).toHaveBeenCalledWith("1.1.1.1:8000", true);
  });

  it("HeartbeatTimeout(非 streaming) → onDisconnect(dialKey,false)", async () => {
    const spy = vi.spyOn(reconnect, "onDisconnect");
    seedSession("PMU1", "cfg2_sent", "1.1.1.1:8000");
    invoke.mockResolvedValueOnce([{ type: "HeartbeatTimeout", idcode: "PMU1" }]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);
    expect(spy).toHaveBeenCalledWith("1.1.1.1:8000", false);
  });

  it("无 dialKey 的会话断开不触发重连", async () => {
    const spy = vi.spyOn(reconnect, "onDisconnect");
    seedSession("PMU1", "streaming"); // 无 dialKey
    invoke.mockResolvedValueOnce([{ type: "SessionDisconnected", idcode: "PMU1" }]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);
    expect(spy).not.toHaveBeenCalled();
  });
});
```

- [x] **Step 2: 写多子站表征测试(新建 `use-pmu-events.multi.test.ts`)**

锁住 占位→真实 re-key 的 dialKey 传递,以及数据不串台:

```ts
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { usePmuEvents } from "../src/composables/usePmuEvents";
import { useSessions } from "../src/composables/useSessions";
import { useCommLog } from "../src/composables/useCommLog";
import { useReconnect } from "../src/composables/useReconnect";

const reconnect = useReconnect();

function mkData(stat: number) {
  return { soc: 0, fracsec: 0, stat, format_flags: 1, time_quality: 0, freq: 0, dfreq: 0, analog: [], digital: [], phasors: [], local_offset_ms: 0 };
}

beforeEach(() => {
  const { clear } = useSessions();
  clear();
  useCommLog().clear();
  reconnect._resetForTest();
  invoke.mockReset();
  vi.useFakeTimers();
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("usePmuEvents 多子站", () => {
  it("占位→真实 re-key 后,真实会话继承 dialKey", async () => {
    const { sessions } = useSessions();
    invoke.mockResolvedValueOnce([
      { type: "SessionCreated", idcode: "10.0.0.1:8000", peer_ip: "10.0.0.1" },
      { type: "SessionDisconnected", idcode: "10.0.0.1:8000" },
      { type: "SessionCreated", idcode: "PMU_A", peer_ip: "10.0.0.1" },
    ]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);
    expect(sessions.get("PMU_A")?.dialKey).toBe("10.0.0.1:8000");
  });

  it("两个子站的数据帧分别落到各自 idcode", async () => {
    const { latestOf } = useCommLog();
    invoke.mockResolvedValueOnce([
      { type: "DataFrame", idcode: "PMU_A", data: mkData(1) },
      { type: "DataFrame", idcode: "PMU_B", data: mkData(2) },
    ]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);
    expect(latestOf("PMU_A")?.stat).toBe(1);
    expect(latestOf("PMU_B")?.stat).toBe(2);
  });
});
```

- [x] **Step 3: 跑测试确认失败**

Run: `cd frontend && npm run test:unit -- use-pmu-events`
Expected: FAIL

- [x] **Step 4: 改 `usePmuEvents.ts`**

4a. 解构补 `setDialKey`、`tick`/`reset` 重命名带参,`useTimeOffset` 同理。把第 32-40 行解构区改为:

```ts
  const { sessions, addSession, setDialKey, updateState, removeSession, setConfig, configs } = useSessions();
  const reconnect = useReconnect();
  const { addData } = useCommLog();
  const { push: pushToast } = useToast();
  const { push: pushEvent } = useEventLog();
  const { tick: tickFrameRate, reset: resetFrameRate } = useFrameRate();
  const { tick: tickOffset, reset: resetOffset } = useTimeOffset();
  const { push: pushAnomaly } = useAnomalyLog();

  // 占位 SessionCreated 暂存的 dialKey,供紧随其后的真实 idcode SessionCreated 继承。
  let pendingDialKey: string | null = null;
```

4b. `handle` 的各 case 改为带 idcode(整段 `function handle` 替换):

```ts
  function handle(payload: PmuEvent) {
    switch (payload.type) {
      case "SessionCreated":
        if (payload.idcode.includes(":")) {
          // 占位会话:占位 idcode 本身即 dialKey。
          pendingDialKey = payload.idcode;
          addSession(payload.idcode, payload.peer_ip, payload.idcode);
        } else {
          addSession(payload.idcode, payload.peer_ip, pendingDialKey ?? undefined);
          pendingDialKey = null;
          pushEvent(payload.idcode, t("event.mgmtEstablished", { idcode: payload.idcode, ip: payload.peer_ip }));
        }
        break;
      case "SessionDisconnected": {
        const s = sessions.get(payload.idcode);
        const wasStreaming = s?.state === "streaming";
        const dialKey = s?.dialKey;
        removeSession(payload.idcode);
        if (!payload.idcode.includes(":")) {
          pushEvent(payload.idcode, t("event.pipeDisconnected", { idcode: payload.idcode }));
        }
        resetFrameRate(payload.idcode);
        resetOffset(payload.idcode);
        if (dialKey) reconnect.onDisconnect(dialKey, wasStreaming);
        break;
      }
      case "Cfg1Received":
        updateState(payload.idcode, "cfg1_received");
        setConfig(payload.idcode, payload.cfg);
        pushEvent(payload.idcode, t("event.cfg1Received", { analog: payload.cfg.annmr, digital: payload.cfg.dgnmr }));
        break;
      case "Cfg2Sent":
        updateState(payload.idcode, "cfg2_sent");
        pushEvent(payload.idcode, t("event.cfg2Sent"));
        break;
      case "Cfg2Skipped":
        pushEvent(payload.idcode, t("event.cfg2Skipped"), "info");
        break;
      case "Cfg2Received":
        setConfig(payload.idcode, payload.cfg);
        break;
      case "StreamingStarted":
        updateState(payload.idcode, "streaming");
        pushEvent(payload.idcode, t("event.dataEstablished"));
        break;
      case "StreamingStopped":
        updateState(payload.idcode, "cfg2_sent");
        pushEvent(payload.idcode, t("event.dataPaused"));
        resetFrameRate(payload.idcode);
        resetOffset(payload.idcode);
        break;
      case "DataFrame": {
        addData(payload.idcode, payload.data);
        const measRate = configs.get(payload.idcode)?.measRate ?? 1_000_000;
        tickFrameRate(payload.idcode, frameTimeMs(payload.data.soc, payload.data.fracsec, measRate));
        tickOffset(payload.idcode, payload.data.local_offset_ms);
        break;
      }
      case "RawFrame":
        break;
      case "HeartbeatTimeout": {
        const s = sessions.get(payload.idcode);
        const wasStreaming = s?.state === "streaming";
        const dialKey = s?.dialKey;
        pushToast(t("event.heartbeatTimeoutToast", { idcode: payload.idcode }), "error");
        pushEvent(payload.idcode, t("event.heartbeatTimeout", { idcode: payload.idcode }), "error");
        removeSession(payload.idcode);
        resetFrameRate(payload.idcode);
        resetOffset(payload.idcode);
        if (dialKey) reconnect.onDisconnect(dialKey, wasStreaming);
        break;
      }
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
      case "Error":
        pushToast(payload.idcode ? `${payload.idcode}: ${payload.error}` : payload.error, "error");
        pushEvent(payload.idcode ?? "", payload.error, "error");
        break;
    }
  }
```

- [x] **Step 5: 跑测试确认通过**

Run: `cd frontend && npm run test:unit -- use-pmu-events`
Expected: PASS

- [x] **Step 6: 提交**

```bash
git add frontend/src/composables/usePmuEvents.ts frontend/tests/use-pmu-events.reconnect.test.ts frontend/tests/use-pmu-events.multi.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(frontend): usePmuEvents 全量按 idcode 路由并解析 dialKey"
```

---

## Task A8: 新建 `StationListPanel` 并接入 `App.vue`

**Files:**
- Create: `frontend/src/components/StationListPanel.vue`
- Modify: `frontend/src/App.vue`(`.content` 最左插入)、`frontend/src/i18n/messages.ts`(新 key)
- Test: `frontend/tests/station-list-panel.test.ts`(新建)

**Interfaces:**
- Consumes: `useSessions`(`sessions`/`selectedIdcode`)、`useFrameRate.fpsOf`、`useReconnect.reconnectingOf`、`useI18n`。
- 行为:遍历 `sessions` 渲染行,LED 类按 state/reconnecting 映射;点击行设 `selectedIdcode`。

- [x] **Step 1: 加 i18n key**

在 `frontend/src/i18n/messages.ts` 的 `zh` 段加:

```ts
    'station.title': '子站列表',
    'station.empty': '暂无子站',
    'station.fpsUnit': 'fps',
```

`en` 段加:

```ts
    'station.title': 'Substations',
    'station.empty': 'No substations',
    'station.fpsUnit': 'fps',
```

- [x] **Step 2: 写失败测试 `station-list-panel.test.ts`**

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import StationListPanel from "../src/components/StationListPanel.vue";
import { useSessions } from "../src/composables/useSessions";
import { setLocale } from "../src/i18n";

beforeEach(() => {
  setLocale("en");
  useSessions().clear();
});

describe("StationListPanel", () => {
  it("渲染所有会话行并按 state 映射 LED 类", () => {
    const { addSession, updateState } = useSessions();
    addSession("PMU_A", "1.1.1.1", "1.1.1.1:8000");
    updateState("PMU_A", "streaming");
    addSession("PMU_B", "2.2.2.2", "2.2.2.2:8000");
    updateState("PMU_B", "disconnected");
    const wrapper = mount(StationListPanel);
    const rows = wrapper.findAll(".station-row");
    expect(rows.length).toBe(2);
    expect(wrapper.find(".led-ok").exists()).toBe(true);
    expect(wrapper.find(".led-err").exists()).toBe(true);
  });

  it("点击行切换 selectedIdcode", async () => {
    const { addSession, selectedIdcode } = useSessions();
    addSession("PMU_A", "1.1.1.1", "1.1.1.1:8000");
    addSession("PMU_B", "2.2.2.2", "2.2.2.2:8000"); // 新增即选中 B
    const wrapper = mount(StationListPanel);
    await wrapper.findAll(".station-row")[0].trigger("click");
    expect(selectedIdcode.value).toBe("PMU_A");
  });

  it("无会话显示空态", () => {
    const wrapper = mount(StationListPanel);
    expect(wrapper.find(".station-empty").exists()).toBe(true);
  });
});
```

- [x] **Step 3: 跑测试确认失败**

Run: `cd frontend && npm run test:unit -- station-list-panel`
Expected: FAIL(组件不存在)

- [x] **Step 4: 写 `StationListPanel.vue`**

```vue
<script setup lang="ts">
import { computed } from "vue";
import { useSessions } from "../composables/useSessions";
import { useFrameRate } from "../composables/useFrameRate";
import { useReconnect } from "../composables/useReconnect";
import { useI18n } from "../i18n";
import type { SessionInfo } from "../types";

const { t } = useI18n();
const { sessions, selectedIdcode } = useSessions();
const { fpsOf } = useFrameRate();
const { reconnectingOf } = useReconnect();

const rows = computed(() => [...sessions.values()]);

// LED 语义:重连中/握手中=琥珀,streaming=绿,disconnected=红,其余(connecting)=琥珀。
function ledClass(s: SessionInfo): string {
  if (s.dialKey && reconnectingOf(s.dialKey)) return "led-warn";
  if (s.state === "streaming") return "led-ok";
  if (s.state === "disconnected") return "led-err";
  if (s.state === "connecting") return "led-warn";
  return "led-warn";
}
function select(idcode: string) {
  selectedIdcode.value = idcode;
}
</script>

<template>
  <section class="station-panel">
    <div class="panel-hd">{{ t("station.title") }}</div>
    <div class="station-list">
      <div
        v-for="s in rows"
        :key="s.idcode"
        class="station-row"
        :class="{ selected: s.idcode === selectedIdcode }"
        @click="select(s.idcode)"
      >
        <span class="led" :class="ledClass(s)"></span>
        <span class="station-id">{{ s.idcode }}</span>
        <span class="station-fps">{{ fpsOf(s.idcode) }} {{ t("station.fpsUnit") }}</span>
      </div>
      <div v-if="rows.length === 0" class="station-empty">{{ t("station.empty") }}</div>
    </div>
  </section>
</template>

<style scoped>
.station-panel {
  width: 188px;
  min-width: 188px;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border);
  background: var(--bg-panel);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.55);
  overflow: hidden;
}
/* 复用 ConfigInfoPanel 的金属铭牌表头风格(简化) */
.panel-hd {
  padding: 6px 10px 6px 14px;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 1.5px;
  color: #44443d;
  background: linear-gradient(180deg, #e4e3d9, #bbbaae);
  border-bottom: 1px solid var(--border);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.75);
  text-shadow: 0 1px 0 rgba(255, 255, 255, 0.6);
  user-select: none;
}
.station-list {
  flex: 1;
  overflow: auto;
  background: var(--bg-content);
}
/* 机架插卡:每行一块卡,左侧 pilot LED */
.station-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-bottom: 1px solid var(--border-soft);
  cursor: pointer;
  font-size: 12px;
}
.station-row:hover { background: var(--accent-tint); }
.station-row.selected { background: var(--accent); color: #fff; }
.led {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  flex-shrink: 0;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
}
.led-ok { background: var(--ok); box-shadow: 0 0 4px var(--ok), inset 0 0 0 1px rgba(0,0,0,0.2); }
.led-warn { background: var(--warn); box-shadow: 0 0 4px var(--warn), inset 0 0 0 1px rgba(0,0,0,0.2); }
.led-err { background: var(--err); box-shadow: inset 0 0 0 1px rgba(0,0,0,0.2); }
.station-id {
  flex: 1;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.station-fps {
  font-size: 11px;
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.station-row.selected .station-fps { color: var(--accent-on-sel); }
.station-empty {
  padding: 16px 10px;
  text-align: center;
  color: var(--text-faint);
  font-style: italic;
  font-size: 12px;
}
</style>
```

- [x] **Step 5: 接入 `App.vue`**

`import` 区(第 9 行后)加:

```ts
import StationListPanel from "./components/StationListPanel.vue";
```

`.content` 区(第 86-89 行)改为:

```html
    <div class="content">
      <StationListPanel />
      <ConfigInfoPanel />
      <DataTablePanel />
    </div>
```

- [x] **Step 6: 跑测试确认通过 + 全量回归**

Run: `cd frontend && npm run test:unit -- station-list-panel && npm run test:unit`
Expected: PASS(全部)

- [x] **Step 7: 提交**

```bash
git add frontend/src/components/StationListPanel.vue frontend/src/App.vue frontend/src/i18n/messages.ts frontend/tests/station-list-panel.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(frontend): 新增机架式子站列表面板(StationListPanel)"
```

---

## Task A9: `ConfigInfoPanel` 解开单会话把守 + 按选中读数

**Files:**
- Modify: `frontend/src/components/ConfigInfoPanel.vue`
- Test: `frontend/tests/config-info-panel.reconnect.test.ts`(更新)

**Interfaces:**
- Consumes: A3/A5/A6 的 `latestOf`、`reconnect.arm(dialKey,target)`/`cancelAll`、`reconnect.reconnectingOf`、`useSessions.setDialKey`、`useFrameRate.fpsOf`、`useTimeOffset.offsetOf`、`useEventLog.entriesFor`。
- 行为:
  - "连接"按钮不再被 `running` 禁用(`:disabled="busy"`),每次点击用当前表单值**新增**一个子站(已存在同 dialKey 则跳过 connect)。
  - 连接后 `reconnect.arm(dialKey, target)` 并 `setDialKey(realOrPlaceholderId, dialKey)`(此处用 dialKey 作 target idcode,真实 id 由 A7 继承)。
  - "停止"调用 `reconnect.cancelAll()`。
  - fps/offset/状态/最新时间/事件日志 全部读 `selectedIdcode`。

- [x] **Step 1: 更新 `config-info-panel.reconnect.test.ts`**

`arm` 现在两参,`cancel` 改 `cancelAll`。把两个 `it` 改为:

```ts
  it("连接成功后 arm(dialKey, mode=normal) 携带表单快照", async () => {
    const armSpy = vi.spyOn(reconnect, "arm");
    const wrapper = mount(ConfigInfoPanel);
    await findButtonByText(wrapper, "Start").trigger("click");
    await flushPromises();
    expect(armSpy).toHaveBeenCalledTimes(1);
    expect(armSpy.mock.calls[0][0]).toBe("10.15.48.12:8000");
    expect(armSpy.mock.calls[0][1]).toMatchObject({
      host: "10.15.48.12", mgmtPort: 8000, dataPort: 8001, protocol: "V3", mode: "normal",
    });
    wrapper.unmount();
  });

  it("停止后 cancelAll 被调用", async () => {
    const cancelSpy = vi.spyOn(reconnect, "cancelAll");
    const { running } = await import("../src/composables/useServerStatus").then((m) => m.useServerStatus());
    running.value = true;
    const wrapper = mount(ConfigInfoPanel);
    await findButtonByText(wrapper, "Stop").trigger("click");
    await flushPromises();
    expect(cancelSpy).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });
```

(`beforeEach` 里 `reconnect._resetForTest()` 已有;若引用了 `useServerStatus`,在 `afterEach` 或下个 `beforeEach` 重置 `running.value=false` 以免泄漏 —— 在文件 `beforeEach` 末尾加 `(await import("../src/composables/useServerStatus")).useServerStatus().running.value = false;` 或直接在测试内联管理。)

- [x] **Step 2: 跑测试确认失败**

Run: `cd frontend && npm run test:unit -- config-info-panel.reconnect`
Expected: FAIL

- [x] **Step 3a: 改 `ConfigInfoPanel.vue` 的 composable 解构 + 读数计算**

第 36-39 行附近(`useCommLog`/`useFrameRate`/`useTimeOffset` 解构)替换为:

```ts
const { latestOf } = useCommLog();
const { entriesFor } = useEventLog();
const { fpsOf } = useFrameRate();
const { offsetOf } = useTimeOffset();

const fps = computed(() => fpsOf(selectedIdcode.value));
const offsetMs = computed(() => offsetOf(selectedIdcode.value));
const selectedEvents = computed(() => entriesFor(selectedIdcode.value));
```

(原 `const { fps } = useFrameRate();`、`const { offsetMs } = useTimeOffset();`、`const { events } = useEventLog();`、`const { latestData } = useCommLog();` 删除。)

`latestTime`(第 210 行)与 `clockOffsetText` 中对 `latestData.value?.data` / `offsetMs.value` 的引用改为:

```ts
const latestTime = computed(() => {
  const d = latestOf(selectedIdcode.value);
  if (!d) return "—";
  const measRate = cfg.value?.measRate ?? 1_000_000;
  const ms = frameTimeMs(d.soc, d.fracsec, measRate);
  const date = new Date(ms);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${date.getFullYear()}/${pad(date.getMonth() + 1)}/${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${pad(date.getMilliseconds()).padStart(3, "0")}`;
});
```

(`clockOffsetText` 内 `offsetMs.value` 现指向新 computed,无需改写其余逻辑。)

`displayState`/`stateClass` 中的 `reconnect.reconnecting.value` 改为按选中会话的 dialKey:

```ts
const selectedReconnecting = computed(() => {
  const dk = session.value?.dialKey;
  return dk ? reconnect.reconnectingOf(dk) : false;
});
const displayState = computed(() => (selectedReconnecting.value ? t("state.reconnecting") : stateLabel.value));
```

`stateClass` 内 `reconnect.reconnecting.value` 同改为 `selectedReconnecting.value`。

- [x] **Step 3b: 改连接流程(`startEverything` / `skipCfg2Connect` / `stopEverything`)**

`startEverything` 第 3-4 步(连接 + 握手 + arm)替换为:

```ts
    // 3. 用当前表单值新增一个子站(已存在同 dialKey 则跳过 connect)
    const host = connIp.value.trim();
    const mgmt = parseInt(connMgmtPort.value);
    const dialKey = `${host}:${mgmt}`;
    const exists = sessions.get(dialKey) || [...sessions.values()].some((s) => s.dialKey === dialKey);
    if (!exists) {
      const data = protocol.value === "V3" ? parseInt(connDataPort.value) : undefined;
      await invoke("connect_substation", { host, port: mgmt, dataPort: data });
    }
    // 4. auto_handshake 用 dialKey 作占位 idcode(后端 resolve_peer_idcode 处理)
    const hz = parseFloat(rateHz.value);
    const periodVal: number | null = Number.isFinite(hz) ? hzToPeriod(hz) : null;
    await invoke("auto_handshake", { idcode: dialKey, period: periodVal });
    reconnect.arm(dialKey, reconnectTarget("normal"));
```

`skipCfg2Connect` 第 3-4 步同理(末步用 `skip_cfg2_open` + `arm(dialKey, reconnectTarget("skipCfg2"))`):

```ts
    const host = connIp.value.trim();
    const mgmt = parseInt(connMgmtPort.value);
    const dialKey = `${host}:${mgmt}`;
    const exists = sessions.get(dialKey) || [...sessions.values()].some((s) => s.dialKey === dialKey);
    if (!exists) {
      const data = protocol.value === "V3" ? parseInt(connDataPort.value) : undefined;
      await invoke("connect_substation", { host, port: mgmt, dataPort: data });
    }
    await invoke("skip_cfg2_open", { idcode: dialKey });
    reconnect.arm(dialKey, reconnectTarget("skipCfg2"));
```

`stopEverything` 内 `reconnect.cancel();` 改为 `reconnect.cancelAll();`。

- [x] **Step 3c: 改模板**

- "连接"按钮 `:disabled="busy || running"` 改为 `:disabled="busy"`(允许运行中继续加子站)。
- 事件日志 `v-for="(e, i) in events"` 改为 `v-for="(e, i) in selectedEvents"`。

- [x] **Step 4: 跑测试确认通过 + 回归**

Run: `cd frontend && npm run test:unit`
Expected: PASS(全部,含 `config-info-panel.0hz`)

> 若 `0hz` 测试因 `running` 禁用变化而断言失真,核对其断言仅依赖 0Hz 确认弹窗逻辑(未触碰 disabled),应不受影响。

- [x] **Step 5: 提交**

```bash
git add frontend/src/components/ConfigInfoPanel.vue frontend/tests/config-info-panel.reconnect.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(frontend): ConfigInfoPanel 支持新增多子站并按选中读数"
```

---

## Task A10: `DataTablePanel` 读选中子站数据

**Files:**
- Modify: `frontend/src/components/DataTablePanel.vue:8-31`
- Test: `frontend/tests/data-table-panel.test.ts`(新建)

**Interfaces:**
- Consumes: `useCommLog.latestOf`、`useSessions`(`selectedIdcode`/`configs`)。

- [x] **Step 1: 写失败测试 `data-table-panel.test.ts`**

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import DataTablePanel from "../src/components/DataTablePanel.vue";
import { useSessions } from "../src/composables/useSessions";
import { useCommLog } from "../src/composables/useCommLog";
import { setLocale } from "../src/i18n";
import type { ConfigInfo, DataInfo } from "../src/types";

function cfg(over: Partial<ConfigInfo> = {}): ConfigInfo {
  return { cfgType: 2, version: 3, stn: "S", idcode: "X", formatFlags: 1, period: 100, measRate: 1_000_000, phnmr: 0, annmr: 1, dgnmr: 0, channelNames: ["AN1"], anunit: [0], ...over };
}
function data(stat: number, analog: number[]): DataInfo {
  return { soc: 0, fracsec: 0, stat, format_flags: 1, time_quality: 0, freq: 0, dfreq: 0, analog, digital: [], phasors: [], local_offset_ms: 0 };
}

beforeEach(() => {
  setLocale("en");
  useSessions().clear();
  useCommLog().clear();
});

describe("DataTablePanel 跟随选中子站", () => {
  it("显示选中子站的数据与其 cfg,不串台", async () => {
    const { addSession, setConfig, selectedIdcode } = useSessions();
    const { addData } = useCommLog();
    addSession("A", "1.1.1.1", "1.1.1.1:8000");
    setConfig("A", cfg({ channelNames: ["A_AN"] }));
    addData("A", data(0, [111]));
    addSession("B", "2.2.2.2", "2.2.2.2:8000");
    setConfig("B", cfg({ channelNames: ["B_AN"] }));
    addData("B", data(0, [222]));
    selectedIdcode.value = "A";
    const wrapper = mount(DataTablePanel);
    expect(wrapper.text()).toContain("A_AN");
    expect(wrapper.text()).toContain("111");
    expect(wrapper.text()).not.toContain("222");
  });
});
```

- [x] **Step 2: 跑测试确认失败**

Run: `cd frontend && npm run test:unit -- data-table-panel`
Expected: FAIL

- [x] **Step 3: 改 `DataTablePanel.vue`**

第 9 行 `const { latestData } = useCommLog();` 改为 `const { latestOf } = useCommLog();`。
第 31 行 `const data = latestData.value?.data;` 改为 `const data = latestOf(selectedIdcode.value);`。

- [x] **Step 4: 跑测试确认通过**

Run: `cd frontend && npm run test:unit -- data-table-panel`
Expected: PASS

- [x] **Step 5: 提交**

```bash
git add frontend/src/components/DataTablePanel.vue frontend/tests/data-table-panel.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(frontend): DataTablePanel 读选中子站数据"
```

---

## Task A11: `AnomalyPanel` 默认过滤到选中子站

**Files:**
- Modify: `frontend/src/components/AnomalyPanel.vue:15-17,40-52`
- Test: `frontend/tests/anomaly-panel.test.ts`(更新/补 1 例)

**Interfaces:**
- Consumes: `useSessions.selectedIdcode`。
- 行为:`filterStation` 默认值改为跟随 `selectedIdcode`(初始 = 当前选中;用户仍可在下拉手动改为 "all" 或其他子站)。

- [x] **Step 1: 补失败测试**

向 `frontend/tests/anomaly-panel.test.ts` 增加(import 处确保有 `useSessions`):

```ts
  it("默认只显示选中子站的异常", async () => {
    const { entries, push } = useAnomalyLog();
    entries.splice(0);
    push({ type: "TimestampAnomaly", idcode: "A", kind: "gap", expected_ms: 10, actual_ms: 30, soc: 0, fracsec: 0, frame_time: "t" } as never);
    push({ type: "TimestampAnomaly", idcode: "B", kind: "gap", expected_ms: 10, actual_ms: 30, soc: 0, fracsec: 0, frame_time: "t" } as never);
    const { addSession, selectedIdcode } = useSessions();
    addSession("A", "1.1.1.1", "1.1.1.1:8000");
    selectedIdcode.value = "A";
    const wrapper = mount(AnomalyPanel);
    await wrapper.find(".anomaly-header").trigger("click"); // 展开
    const bodyText = wrapper.find(".anomaly-body").text();
    expect(bodyText).toContain("A");
    expect(bodyText).not.toContain("B");
  });
```

(文件顶部若无 `useSessions`/`useAnomalyLog` import 则补;`beforeEach` 里 `useSessions().clear()`。)

- [x] **Step 2: 跑测试确认失败**

Run: `cd frontend && npm run test:unit -- anomaly-panel`
Expected: FAIL

- [x] **Step 3: 改 `AnomalyPanel.vue`**

`<script setup>` 顶部加 import 与选中联动:

```ts
import { computed, ref, watch } from "vue";
import { useSessions } from "../composables/useSessions";
```

```ts
const { selectedIdcode } = useSessions();
// 默认过滤到当前选中子站;选中切换时同步(用户手动改过下拉则尊重其选择直到再次切换)。
const filterStation = ref<string>(selectedIdcode.value || "all");
watch(selectedIdcode, (id) => { filterStation.value = id || "all"; });
```

(删除原 `const filterStation = ref<string>("all");`;`filtered` 计算逻辑不变,已按 `filterStation` 过滤。)

- [x] **Step 4: 跑测试确认通过 + 全量回归**

Run: `cd frontend && npm run test:unit`
Expected: PASS(全部)

- [x] **Step 5: 提交**

```bash
git add frontend/src/components/AnomalyPanel.vue frontend/tests/anomaly-panel.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(frontend): AnomalyPanel 默认过滤到选中子站"
```

---

> **Phase A 验收(手动)**:`cd frontend && npm run build` 通过(无 `latestData` 残留报错);`npm run test:unit` 全绿。本机起两个 `pmusim-sub`(V3 不同 IDCODE)连主站:左侧列表两行、LED 正确、点击切换数据表/配置/日志/异常,数据不串台,断一个不影响另一个。

---

# Phase B — 相量可视化(B12–B14)

## Task B12: `lib/phasor.ts` 相量换算纯函数

**Files:**
- Create: `frontend/src/lib/phasor.ts`
- Test: `frontend/tests/phasor.test.ts`(新建)

**Interfaces:**
- Produces:
  - `phasorMagAngle(pair: [number, number], polar: boolean): { mag: number; angleDeg: number }` — `polar=true` 时 pair=`(mag, angleRad)` 直接用(角度转度);`polar=false` 时 pair=`(re, im)`,`mag=hypot`,`angleDeg=atan2`(度)。
  - 角度统一规整到 `(-180, 180]`。

> 角度单位假设:C37.118 极坐标相角为**弧度**。B13 执行前若发现后端已转度,改 `polar` 分支的 `* 180/Math.PI` 为直通,并更新本测试。

- [x] **Step 1: 写失败测试 `phasor.test.ts`**

```ts
import { describe, it, expect } from "vitest";
import { phasorMagAngle } from "../src/lib/phasor";

describe("phasorMagAngle", () => {
  it("直角坐标 → 幅值/相角(度)", () => {
    const r = phasorMagAngle([3, 4], false);
    expect(r.mag).toBeCloseTo(5, 6);
    expect(r.angleDeg).toBeCloseTo(53.1301, 3);
  });

  it("极坐标(弧度)→ 角度转度", () => {
    const r = phasorMagAngle([10, Math.PI / 2], true);
    expect(r.mag).toBe(10);
    expect(r.angleDeg).toBeCloseTo(90, 6);
  });

  it("角度规整到 (-180,180]", () => {
    const r = phasorMagAngle([-1, 0], false); // atan2(0,-1)=π → 180
    expect(r.angleDeg).toBeCloseTo(180, 6);
  });
});
```

- [x] **Step 2: 跑测试确认失败**

Run: `cd frontend && npm run test:unit -- phasor`
Expected: FAIL

- [x] **Step 3: 写 `lib/phasor.ts`**

```ts
// 相量换算:CFG-2 FORMAT bit0=1 → 数据帧相量为 (magnitude, angle);bit0=0 → (real, imag)。
// C37.118 极坐标相角定义为弧度,此处统一输出"度"并规整到 (-180,180]。
function normalizeDeg(deg: number): number {
  let d = deg % 360;
  if (d <= -180) d += 360;
  if (d > 180) d -= 360;
  return d;
}

export function phasorMagAngle(pair: [number, number], polar: boolean): { mag: number; angleDeg: number } {
  if (polar) {
    return { mag: pair[0], angleDeg: normalizeDeg((pair[1] * 180) / Math.PI) };
  }
  const [re, im] = pair;
  return { mag: Math.hypot(re, im), angleDeg: normalizeDeg((Math.atan2(im, re) * 180) / Math.PI) };
}
```

- [x] **Step 4: 跑测试确认通过**

Run: `cd frontend && npm run test:unit -- phasor`
Expected: PASS

- [x] **Step 5: 提交**

```bash
git add frontend/src/lib/phasor.ts frontend/tests/phasor.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(frontend): 新增相量换算纯函数 lib/phasor"
```

---

## Task B13: `DataTablePanel` 增相量行 + freq/dfreq 读数

**Files:**
- Modify: `frontend/src/components/DataTablePanel.vue`(`displayRows` computed、i18n)
- Modify: `frontend/src/i18n/messages.ts`
- Test: `frontend/tests/data-table-panel.test.ts`(补相量用例)

**Interfaces:**
- Consumes: `phasorMagAngle`(B12)。
- 行为:STAT 4 行之后、模拟量行之前插入 `phnmr` 行相量(名称 `channelNames[0..phnmr-1]`,值=`mag ∠ angleDeg°`);序号整体顺延(相量占 `05..04+phnmr`,模拟量从 `05+phnmr` 起,数字量从 `05+phnmr+annmr` 起)。freq/dfreq 各 1 行追加到 STAT 区(序号 05/06 之前?为不打乱编号,作为 STAT 区附加两行,key `freq`/`dfreq`,不占数字序号)。

- [x] **Step 1: 加 i18n key**

`messages.ts` `zh` 段加:

```ts
    'data.phasor': '相量',
    'data.freq': '系统频率',
    'data.rocof': '频率变化率',
    'data.angleUnit': '°',
    'data.hzUnit': 'Hz',
```

`en` 段加:

```ts
    'data.phasor': 'Phasor',
    'data.freq': 'Frequency',
    'data.rocof': 'ROCOF',
    'data.angleUnit': '°',
    'data.hzUnit': 'Hz',
```

- [x] **Step 2: 补失败测试**

向 `data-table-panel.test.ts` 增加:

```ts
  it("渲染相量行(直角坐标→幅值/相角)", () => {
    const { addSession, setConfig, selectedIdcode } = useSessions();
    const { addData } = useCommLog();
    addSession("A", "1.1.1.1", "1.1.1.1:8000");
    setConfig("A", cfg({ phnmr: 1, annmr: 0, channelNames: ["Ua"] }));
    const d = data(0, []);
    d.format_flags = 0; // 直角
    d.phasors = [[3, 4]]; // mag 5, angle 53.13°
    addData("A", d);
    selectedIdcode.value = "A";
    const wrapper = mount(DataTablePanel);
    expect(wrapper.text()).toContain("Ua");
    expect(wrapper.text()).toContain("5.000");
    expect(wrapper.text()).toContain("53.13");
  });
```

- [x] **Step 3: 跑测试确认失败**

Run: `cd frontend && npm run test:unit -- data-table-panel`
Expected: FAIL

- [x] **Step 4: 改 `DataTablePanel.vue` 的 `displayRows`**

顶部 import 加:`import { phasorMagAngle } from "../lib/phasor";`。

在 STAT 4 行 push 之后、`if (!c) return rows;` 之前,插入 freq/dfreq 两行:

```ts
  // 系统频率 / ROCOF —— 数据帧自带,作 STAT 区附加读数(不占数字序号)。
  rows.push({ key: "freq", num: "—", name: t("data.freq"),
    value: data ? `${data.freq.toFixed(3)} ${t("data.hzUnit")}` : "-", extra: "" });
  rows.push({ key: "dfreq", num: "—", name: t("data.rocof"),
    value: data ? data.dfreq.toFixed(4) : "-", extra: "" });

  if (!c) return rows;

  // 相量行:format bit0 决定极/直角。名称取 channelNames[0..phnmr-1]。
  const polar = (data?.format_flags ?? c.formatFlags) & 1 ? true : false;
  for (let i = 0; i < c.phnmr; i++) {
    const pair = data?.phasors[i];
    const r = pair ? phasorMagAngle(pair, polar) : null;
    rows.push({
      key: `ph-${i}`,
      num: String(5 + i).padStart(2, "0"),
      name: c.channelNames[i] || `PH_${i + 1}`,
      value: r ? `${r.mag.toFixed(3)} ∠ ${r.angleDeg.toFixed(2)}${t("data.angleUnit")}` : "-",
      extra: t("data.phasor"),
    });
  }
```

把原模拟量/数字量循环的序号基准与 channelNames 偏移整体加上 `phnmr` 的偏移:

- 模拟量循环:`num: String(5 + i)` → `String(5 + c.phnmr + i)`;名称仍 `c.channelNames[analogStart + i]`(`analogStart = c.phnmr` 不变)。
- 数字量循环:`num: String(5 + c.annmr + i)` → `String(5 + c.phnmr + c.annmr + i)`;名称 `c.channelNames[digitalStart + i]`(`digitalStart = c.phnmr + c.annmr` 不变)。

- [x] **Step 5: 跑测试确认通过 + 回归**

Run: `cd frontend && npm run test:unit`
Expected: PASS(全部)

- [x] **Step 6: 提交**

```bash
git add frontend/src/components/DataTablePanel.vue frontend/src/i18n/messages.ts frontend/tests/data-table-panel.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(frontend): 数据表显示相量行与 freq/ROCOF 读数"
```

---

## Task B14: `PhasorPlot` 实时极坐标相量图

**Files:**
- Create: `frontend/src/components/PhasorPlot.vue`
- Modify: `frontend/src/components/DataTablePanel.vue`(在表格上方嵌入 `<PhasorPlot />`)
- Test: `frontend/tests/phasor-plot.test.ts`(新建)

**Interfaces:**
- Consumes: `useCommLog.latestOf`、`useSessions`(`selectedIdcode`/`configs`)、`phasorMagAngle`。
- 行为:`<canvas>` 极坐标图;每路相量一根矢量(长度按当前帧最大幅值归一化、方向=相角),随选中子站最新帧重绘;`prefers-reduced-motion` 下不做额外过渡(本实现本就逐帧重绘静态末态,无显式动画,天然满足)。
- 可测点:无数据时渲染占位 `.phasor-empty`;有相量数据时渲染 `<canvas>` 且 `width>0`。canvas 2D 绘制内容不在 happy-dom 断言(getContext 为 stub),仅断言结构与 `drawScheduled` 计算出的矢量数量。

> 为可测,绘制逻辑抽出纯函数 `computeVectors(phasors, polar)`(放 `lib/phasor.ts`,返回每路 `{mag, angleDeg, normLen}`),组件只负责把它画到 canvas。B14 Step 0 先给 `lib/phasor.ts` 补该函数及其单测。

- [x] **Step 1: 给 `lib/phasor.ts` 补 `computeVectors` + 测试**

向 `lib/phasor.ts` 追加:

```ts
export interface PhasorVector {
  mag: number;
  angleDeg: number;
  normLen: number; // 0..1,按本帧最大幅值归一化
}

export function computeVectors(phasors: [number, number][], polar: boolean): PhasorVector[] {
  const polarVals = phasors.map((p) => phasorMagAngle(p, polar));
  const maxMag = polarVals.reduce((m, v) => Math.max(m, v.mag), 0);
  return polarVals.map((v) => ({ mag: v.mag, angleDeg: v.angleDeg, normLen: maxMag > 0 ? v.mag / maxMag : 0 }));
}
```

向 `phasor.test.ts` 追加:

```ts
import { computeVectors } from "../src/lib/phasor";

describe("computeVectors", () => {
  it("按最大幅值归一化", () => {
    const v = computeVectors([[3, 4], [6, 8]], false); // mag 5, 10
    expect(v[0].normLen).toBeCloseTo(0.5, 6);
    expect(v[1].normLen).toBeCloseTo(1, 6);
  });
  it("全零相量 normLen=0 不除零", () => {
    const v = computeVectors([[0, 0]], false);
    expect(v[0].normLen).toBe(0);
  });
});
```

Run: `cd frontend && npm run test:unit -- phasor`
Expected: 追加用例 PASS。

- [x] **Step 2: 写失败测试 `phasor-plot.test.ts`**

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import PhasorPlot from "../src/components/PhasorPlot.vue";
import { useSessions } from "../src/composables/useSessions";
import { useCommLog } from "../src/composables/useCommLog";
import { setLocale } from "../src/i18n";
import type { ConfigInfo, DataInfo } from "../src/types";

function cfg(phnmr: number): ConfigInfo {
  return { cfgType: 2, version: 3, stn: "S", idcode: "X", formatFlags: 1, period: 100, measRate: 1_000_000, phnmr, annmr: 0, dgnmr: 0, channelNames: Array.from({ length: phnmr }, (_, i) => `PH${i}`), anunit: [] };
}
function data(phasors: [number, number][]): DataInfo {
  return { soc: 0, fracsec: 0, stat: 0, format_flags: 0, time_quality: 0, freq: 0, dfreq: 0, analog: [], digital: [], phasors, local_offset_ms: 0 };
}

beforeEach(() => {
  setLocale("en");
  useSessions().clear();
  useCommLog().clear();
});

describe("PhasorPlot", () => {
  it("无相量数据显示占位", () => {
    const wrapper = mount(PhasorPlot);
    expect(wrapper.find(".phasor-empty").exists()).toBe(true);
  });

  it("有相量数据渲染 canvas", async () => {
    const { addSession, setConfig, selectedIdcode } = useSessions();
    const { addData } = useCommLog();
    addSession("A", "1.1.1.1", "1.1.1.1:8000");
    setConfig("A", cfg(2));
    addData("A", data([[3, 4], [1, 0]]));
    selectedIdcode.value = "A";
    const wrapper = mount(PhasorPlot);
    expect(wrapper.find("canvas").exists()).toBe(true);
  });
});
```

- [x] **Step 3: 跑测试确认失败**

Run: `cd frontend && npm run test:unit -- phasor-plot`
Expected: FAIL

- [x] **Step 4: 写 `PhasorPlot.vue`**

```vue
<script setup lang="ts">
import { computed, ref, watch, onMounted, nextTick } from "vue";
import { useSessions } from "../composables/useSessions";
import { useCommLog } from "../composables/useCommLog";
import { computeVectors } from "../lib/phasor";

const { selectedIdcode, configs } = useSessions();
const { latestOf } = useCommLog();
const canvas = ref<HTMLCanvasElement | null>(null);
const SIZE = 160;
const COLORS = ["#2563a8", "#c02626", "#1d7a3e", "#b06a00", "#6b3fa0", "#0a7d8c"];

const vectors = computed(() => {
  const data = latestOf(selectedIdcode.value);
  const cfg = configs.get(selectedIdcode.value);
  if (!data || !cfg || cfg.phnmr === 0 || data.phasors.length === 0) return [];
  const polar = (data.format_flags & 1) === 1;
  return computeVectors(data.phasors.slice(0, cfg.phnmr), polar);
});

function draw() {
  const el = canvas.value;
  if (!el) return;
  const ctx = el.getContext("2d");
  if (!ctx) return;
  const c = SIZE / 2;
  const R = c - 12;
  ctx.clearRect(0, 0, SIZE, SIZE);
  // 刻度盘
  ctx.strokeStyle = "#d8d6cc";
  ctx.lineWidth = 1;
  for (const f of [0.5, 1]) {
    ctx.beginPath();
    ctx.arc(c, c, R * f, 0, Math.PI * 2);
    ctx.stroke();
  }
  ctx.beginPath();
  ctx.moveTo(c - R, c); ctx.lineTo(c + R, c);
  ctx.moveTo(c, c - R); ctx.lineTo(c, c + R);
  ctx.stroke();
  // 矢量(0° 指向右,逆时针为正,屏幕 y 向下故取负角)
  vectors.value.forEach((v, i) => {
    const rad = (-v.angleDeg * Math.PI) / 180;
    const x = c + R * v.normLen * Math.cos(rad);
    const y = c + R * v.normLen * Math.sin(rad);
    ctx.strokeStyle = COLORS[i % COLORS.length];
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(c, c); ctx.lineTo(x, y);
    ctx.stroke();
    ctx.fillStyle = COLORS[i % COLORS.length];
    ctx.beginPath();
    ctx.arc(x, y, 2.5, 0, Math.PI * 2);
    ctx.fill();
  });
}

watch(vectors, () => { void nextTick(draw); });
onMounted(draw);
</script>

<template>
  <div class="phasor-plot">
    <canvas v-if="vectors.length" ref="canvas" :width="SIZE" :height="SIZE"></canvas>
    <div v-else class="phasor-empty">—</div>
  </div>
</template>

<style scoped>
.phasor-plot {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 8px;
  background: var(--bg-content);
  border-bottom: 1px solid var(--border-soft);
}
.phasor-empty {
  height: 160px;
  display: flex;
  align-items: center;
  color: var(--text-faint);
}
</style>
```

- [x] **Step 5: 嵌入 `DataTablePanel.vue`**

`<script setup>` import 加:`import PhasorPlot from "./PhasorPlot.vue";`。
模板最外层 `.data-table-wrap` 之前插入 `<PhasorPlot />`(放在同一根容器内;若当前根是单个 `.data-table-wrap`,用 `<div class="data-pane">` 包住两者):

```html
<template>
  <div class="data-pane">
    <PhasorPlot />
    <div class="data-table-wrap">
      <!-- 原 table 不变 -->
    </div>
  </div>
</template>
```

并加样式:

```css
.data-pane { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
```

(原 `.data-table-wrap { flex: 1; … }` 不变,作为 `.data-pane` 的可滚动主体。)

- [x] **Step 6: 跑测试确认通过 + 全量回归 + 构建**

Run: `cd frontend && npm run test:unit && npm run build`
Expected: 测试全 PASS,`build` 成功。

- [x] **Step 7: 提交**

```bash
git add frontend/src/components/PhasorPlot.vue frontend/src/components/DataTablePanel.vue frontend/src/lib/phasor.ts frontend/tests/phasor.test.ts frontend/tests/phasor-plot.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(frontend): 实时极坐标相量图(PhasorPlot)"
```

---

## 自检(Self-Review)结论

- **Spec 覆盖**:多子站(A1–A11)+ 机架 LED(A8)+ 解单会话把守(A9)+ 按 dialKey 重连(A5/A7)+ 相量行/freq/ROCOF(B13)+ 极坐标相量图(B12/B14)+ 异常按选中过滤(A11)均有对应 Task。V2 同 IP 限制为后端非目标,不落 Task(spec 已记)。
- **类型一致**:`fpsOf`/`offsetOf`/`latestOf`/`entriesFor`/`reconnectingOf`/`arm(dialKey,target)`/`onDisconnect(dialKey,wasStreaming)`/`cancelAll`/`setDialKey`/`dialKey?` 在定义 Task 与消费 Task(A7/A8/A9/A10)间一致。
- **占位扫描**:无 TBD/TODO;每个改码步骤含完整代码。两处显式假设(C37.118 相角弧度、后端 re-key 事件顺序)已在对应 Task 标注校验点。
