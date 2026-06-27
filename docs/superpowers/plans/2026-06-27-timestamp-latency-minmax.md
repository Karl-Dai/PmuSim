# 主站时标延迟极值读数 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在主站读数面板「本地时间偏差」行下新增「最大时标延迟」「最小时标延迟」两行,复用现有 50 帧滑动窗口的 `local_offset_ms` 样本取 max/min。

**Architecture:** 纯前端改动,后端零改动。`useTimeOffset` 在现有 50 帧定长窗口里除均值外再维护 max/min 两个 reactive map,极值样本被滑出时重扫窗口重算。`ConfigInfoPanel` 读数面板加两行,格式复用抽出的 `fmtSignedMs` 纯函数。i18n 加两条 key。

**Tech Stack:** Vue 3 + TypeScript, vitest, Tauri 2。

**Spec:** `docs/superpowers/specs/2026-06-27-timestamp-latency-minmax-design.md`

---

## 文件结构

| 文件 | 责任 | 动作 |
|------|------|------|
| `frontend/src/composables/useTimeOffset.ts` | 50 帧窗口的均值 + max/min 聚合,按 idcode 隔离 | 修改 |
| `frontend/tests/use-time-offset.test.ts` | useTimeOffset 单测 | 修改(扩写) |
| `frontend/src/i18n/messages.ts` | 中英 i18n 文案 | 修改(加 4 条) |
| `frontend/src/components/ConfigInfoPanel.vue` | 读数面板渲染 | 修改(加 2 行 + 抽函数) |

无新文件。后端(`crates/`)零改动。

---

### Task 1: useTimeOffset max/min — 测试先行

**Files:**
- Modify: `frontend/tests/use-time-offset.test.ts`

- [ ] **Step 1: 在测试文件末尾追加 max/min 用例**

在 `frontend/tests/use-time-offset.test.ts` 末尾(`describe` 块内最后一个 `it` 之后)追加以下 `it` 块:

```ts
  it("max/min 随样本更新,正负保留", () => {
    const { tick, offsetOf, maxOf, minOf } = useTimeOffset();
    tick("A", 10);
    tick("A", -10);
    tick("A", 30);
    expect(maxOf("A")).toBe(30);
    expect(minOf("A")).toBe(-10);
    expect(offsetOf("A")).toBeCloseTo(10, 5);
  });

  it("max 滑出窗口后重扫回落", () => {
    const { tick, maxOf, minOf } = useTimeOffset();
    // 窗口 50 帧:第一帧是最大值 100,其余 49 帧为 1。
    tick("A", 100);
    for (let i = 0; i < 49; i++) tick("A", 1);
    expect(maxOf("A")).toBe(100);
    expect(minOf("A")).toBe(1);
    // 第 51 帧=1,滑出 100(当前 max)→ 重扫,max 回落到 1。
    tick("A", 1);
    expect(maxOf("A")).toBe(1);
    expect(minOf("A")).toBe(1);
  });

  it("min 滑出窗口后重扫回落", () => {
    const { tick, maxOf, minOf } = useTimeOffset();
    // 第一帧是最小值 -100,其余 49 帧为 0。
    tick("A", -100);
    for (let i = 0; i < 49; i++) tick("A", 0);
    expect(minOf("A")).toBe(-100);
    // 第 51 帧=0,滑出 -100(当前 min)→ 重扫,min 回落到 0。
    tick("A", 0);
    expect(minOf("A")).toBe(0);
    expect(maxOf("A")).toBe(0);
  });

  it("max/min 两子站隔离", () => {
    const { tick, maxOf, minOf } = useTimeOffset();
    tick("A", 100);
    tick("B", -50);
    tick("A", 5);
    tick("B", 200);
    expect(maxOf("A")).toBe(100);
    expect(minOf("A")).toBe(5);
    expect(maxOf("B")).toBe(200);
    expect(minOf("B")).toBe(-50);
  });

  it("reset 后 max/min 归 null,未知 idcode 也为 null", () => {
    const { tick, maxOf, minOf, reset } = useTimeOffset();
    tick("A", 10);
    tick("A", -5);
    expect(maxOf("A")).toBe(10);
    reset("A");
    expect(maxOf("A")).toBeNull();
    expect(minOf("A")).toBeNull();
    expect(maxOf("nope")).toBeNull();
    expect(minOf("nope")).toBeNull();
  });
```

- [ ] **Step 2: 运行测试,确认失败**

Run: `cd frontend && npx vitest run tests/use-time-offset.test.ts`
Expected: FAIL — `maxOf is not a function`(尚未实现)。

- [ ] **Step 3: Commit (红测试)**

```bash
git add frontend/tests/use-time-offset.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "test(time-offset): 追加 max/min 极值用例(红)"
```

---

### Task 2: useTimeOffset max/min — 实现

**Files:**
- Modify: `frontend/src/composables/useTimeOffset.ts`

- [ ] **Step 1: 用以下内容整体替换 `frontend/src/composables/useTimeOffset.ts`**

```ts
import { reactive } from "vue";

// 报文时间与本机时钟偏差(ms)按 idcode 隔离的定长滑动窗口。每帧偏差由后端
// 采样写入 DataInfo.local_offset_ms。正=报文滞后本地,负=超前。
// 窗口内同时维护均值、最大值、最小值;极值样本被滑出时重扫窗口重算。
const WINDOW = 50;
const windows = new Map<string, number[]>();
const offsetMap = reactive(new Map<string, number | null>());
const maxMap = reactive(new Map<string, number | null>());
const minMap = reactive(new Map<string, number | null>());

export function useTimeOffset() {
  function tick(idcode: string, ms: number) {
    let samples = windows.get(idcode);
    if (!samples) {
      samples = [];
      windows.set(idcode, samples);
    }
    samples.push(ms);
    let rescanMax = false;
    let rescanMin = false;
    if (samples.length > WINDOW) {
      const evicted = samples.shift() as number;
      if (evicted === maxMap.get(idcode)) rescanMax = true;
      if (evicted === minMap.get(idcode)) rescanMin = true;
    }
    const curMax = maxMap.get(idcode);
    if (curMax == null) {
      // 首帧:max=min=新样本。
      maxMap.set(idcode, ms);
      minMap.set(idcode, ms);
    } else {
      if (rescanMax) maxMap.set(idcode, Math.max(...samples));
      else if (ms > curMax) maxMap.set(idcode, ms);
      if (rescanMin) minMap.set(idcode, Math.min(...samples));
      else if (ms < (minMap.get(idcode) as number)) minMap.set(idcode, ms);
    }
    const sum = samples.reduce((a, b) => a + b, 0);
    offsetMap.set(idcode, sum / samples.length);
  }
  function reset(idcode: string) {
    windows.delete(idcode);
    offsetMap.set(idcode, null);
    maxMap.set(idcode, null);
    minMap.set(idcode, null);
  }
  function offsetOf(idcode: string): number | null {
    return offsetMap.get(idcode) ?? null;
  }
  function maxOf(idcode: string): number | null {
    return maxMap.get(idcode) ?? null;
  }
  function minOf(idcode: string): number | null {
    return minMap.get(idcode) ?? null;
  }
  return { tick, reset, offsetOf, maxOf, minOf };
}
```

- [ ] **Step 2: 运行测试,确认全绿**

Run: `cd frontend && npx vitest run tests/use-time-offset.test.ts`
Expected: PASS — 所有用例(含原有 3 个 + 新增 5 个)通过。

- [ ] **Step 3: Commit**

```bash
git add frontend/src/composables/useTimeOffset.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(time-offset): 窗口内维护 max/min 极值读数"
```

---

### Task 3: i18n 文案

**Files:**
- Modify: `frontend/src/i18n/messages.ts`

- [ ] **Step 1: 在 zh 区块加两条 key**

在 `frontend/src/i18n/messages.ts` 的 `zh` 对象里,`'config.clockOffset': '本地时间偏差',` 行之后插入:

```ts
    'config.maxLatency': '最大时标延迟',
    'config.minLatency': '最小时标延迟',
```

- [ ] **Step 2: 在 en 区块加两条 key**

在 `en` 对象里,`'config.clockOffset': 'Clock offset',` 行之后插入:

```ts
    'config.maxLatency': 'Max latency',
    'config.minLatency': 'Min latency',
```

- [ ] **Step 3: 类型检查 + 构建确认无回归**

Run: `cd frontend && npm run build`
Expected: `vue-tsc -b` 与 `vite build` 均通过,无类型错误。

- [ ] **Step 4: Commit**

```bash
git add frontend/src/i18n/messages.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "i18n: 新增最大/最小时标延迟词条"
```

---

### Task 4: ConfigInfoPanel 读数面板加两行

**Files:**
- Modify: `frontend/src/components/ConfigInfoPanel.vue`

- [ ] **Step 1: 在 `<script setup>` 里接入 maxOf/minOf 并抽格式函数**

在 `frontend/src/components/ConfigInfoPanel.vue` 的 `<script setup>` 块中,找到:

```ts
const { offsetOf } = useTimeOffset();

const fps = computed(() => fpsOf(selectedIdcode.value));
const offsetMs = computed(() => offsetOf(selectedIdcode.value));
const selectedEvents = computed(() => entriesFor(selectedIdcode.value));
// 偏差读数:带符号整数 ms；无样本显示「—」。正号显式加，负号由数字自带。
const clockOffsetText = computed(() => {
  const v = offsetMs.value;
  if (v === null) return "—";
  const r = Math.round(v);
  return (r > 0 ? "+" : "") + r;
});
```

替换为:

```ts
const { offsetOf, maxOf, minOf } = useTimeOffset();

const fps = computed(() => fpsOf(selectedIdcode.value));
const offsetMs = computed(() => offsetOf(selectedIdcode.value));
const maxMs = computed(() => maxOf(selectedIdcode.value));
const minMs = computed(() => minOf(selectedIdcode.value));
const selectedEvents = computed(() => entriesFor(selectedIdcode.value));
// 偏差读数:带符号整数 ms；无样本显示「—」。正号显式加，负号由数字自带。
function fmtSignedMs(v: number | null): string {
  if (v === null) return "—";
  const r = Math.round(v);
  return (r > 0 ? "+" : "") + r;
}
const clockOffsetText = computed(() => fmtSignedMs(offsetMs.value));
const maxLatencyText = computed(() => fmtSignedMs(maxMs.value));
const minLatencyText = computed(() => fmtSignedMs(minMs.value));
```

- [ ] **Step 2: 在读数面板模板加两行**

在同一文件模板中,找到读数面板的 `本地时间偏差` 行:

```html
        <div class="rd-row"><label>{{ t("config.clockOffset") }}</label><span class="rd-val mono">{{ clockOffsetText }}<span v-if="offsetMs !== null" class="unit">{{ t("config.msUnit") }}</span></span></div>
```

在其后追加两行:

```html
        <div class="rd-row"><label>{{ t("config.maxLatency") }}</label><span class="rd-val mono">{{ maxLatencyText }}<span v-if="maxMs !== null" class="unit">{{ t("config.msUnit") }}</span></span></div>
        <div class="rd-row"><label>{{ t("config.minLatency") }}</label><span class="rd-val mono">{{ minLatencyText }}<span v-if="minMs !== null" class="unit">{{ t("config.msUnit") }}</span></span></div>
```

- [ ] **Step 3: 类型检查 + 构建**

Run: `cd frontend && npm run build`
Expected: 通过,无类型 / 模板错误。

- [ ] **Step 4: 跑全量前端测试确认无回归**

Run: `cd frontend && npx vitest run`
Expected: 全部测试通过(含 use-time-offset 及其它既有用例)。

- [ ] **Step 5: Commit**

```bash
git add frontend/src/components/ConfigInfoPanel.vue
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(master): 读数面板新增最大/最小时标延迟两行"
```

---

## 完成验证

- [ ] `cd frontend && npx vitest run` 全绿
- [ ] `cd frontend && npm run build` 通过
- [ ] `cargo test --workspace` 全绿(后端未改,应无回归)
- [ ] 手动或 headless 冒烟:连一台子站,读数面板应显示「本地时间偏差 / 最大时标延迟 / 最小时标延迟」三行,断开后三行归 `—`。

## 不做(YAGNI)

- 不告警、不进数据表、不画曲线。
- 不动后端、不新增事件类型。
- 不另开更长窗口。
