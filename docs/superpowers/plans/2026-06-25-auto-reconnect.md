# 主站非服务端自动重连 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 主站 client 侧连接意外断开后,前端自动以指数退避重连并忠实恢复断线前状态,本地 data server 不重启。

**Architecture:** 前端驱动。新增模块级单例 composable `useReconnect`,持有"重连目标快照"并管理指数退避调度;`usePmuEvents` 在断线事件里触发它;`ConfigInfoPanel` 在连接成功时 `arm`、主动停止时 `cancel`。复用现有 invoke 命令(`connect_substation` / `auto_handshake` / `skip_cfg2_open`),后端零改动。

**Tech Stack:** Vue 3 `<script setup>` + TypeScript,Tauri `@tauri-apps/api/core` 的 `invoke`,vitest + happy-dom + `@vue/test-utils` 测试。

## Global Constraints

- 测试文件放 `frontend/tests/`,命名 `*.test.ts`(vitest.config.ts 只收集此模式;不要用 `*.spec.ts` 或 `__tests__/`)。
- 退避:`delayFor(attempt) = min(1000 * 2^attempt, 30000)` ms。序列为 1/2/4/8/16/30/30…s。
- 复用现有 invoke 命令,**后端不改**。
- 单 target UI(参考实现单子站)。只对"真实会话"(idcode 不含 `:`)触发重连;placeholder(`host:port`,连接尝试中)断开不触发。
- 提交:作者 `Karl-Dai Karl <kelsoprotein@gmail.com>`(用 `--author` 覆盖本地 noreply 配置);commit message 用简体中文;**禁止** Co-Authored-By 或任何生成署名。
- 每个任务结尾跑相关测试 + 提交;最后一个任务跑 `npm run build`(含 `vue-tsc` 类型检查)+ `npm run test:unit` 全绿。

---

### Task 1: useReconnect composable(核心,纯逻辑 TDD)

**Files:**
- Create: `frontend/src/composables/useReconnect.ts`
- Test: `frontend/tests/use-reconnect.test.ts`

**Interfaces:**
- Produces(后续任务依赖这些精确签名):
  - `export type ReconnectMode = "normal" | "skipCfg2"`
  - `export interface ReconnectTarget { host: string; mgmtPort: number; dataPort: number; protocol: "V2" | "V3"; period: number | null; mode: ReconnectMode }`
  - `export function useReconnect(): { reconnecting: Ref<boolean>; arm(target: ReconnectTarget): void; onDisconnect(wasStreaming: boolean): void; cancel(): void; _resetForTest(): void }`
  - `useReconnect()` 每次返回**同一个单例对象**(供后续任务 `vi.spyOn` 接线方法)。
- 重连动作(`onDisconnect` 触发后,每次重试执行):
  ```
  idcode = `${host}:${mgmtPort}`
  invoke("connect_substation", { host, port: mgmtPort, dataPort: protocol==="V3" ? dataPort : undefined })
  if (wasStreaming) {
    mode==="skipCfg2" ? invoke("skip_cfg2_open", { idcode })
                      : invoke("auto_handshake", { idcode, period })
  }
  ```

- [ ] **Step 1: 写失败测试**

创建 `frontend/tests/use-reconnect.test.ts`:

```ts
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { useReconnect, type ReconnectTarget } from "../src/composables/useReconnect";

const api = useReconnect();

const target = (over: Partial<ReconnectTarget> = {}): ReconnectTarget => ({
  host: "10.0.0.1",
  mgmtPort: 8000,
  dataPort: 8001,
  protocol: "V3",
  period: 100,
  mode: "normal",
  ...over,
});

beforeEach(() => {
  api._resetForTest();
  invoke.mockReset();
  invoke.mockResolvedValue(undefined);
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

describe("useReconnect", () => {
  it("指数退避序列 1/2/4/8/16/30/30s,connect 持续失败", async () => {
    invoke.mockRejectedValue(new Error("connect failed"));
    api.arm(target());
    api.onDisconnect(false); // wasStreaming=false → 每次只调 connect 一次

    const delays = [1000, 2000, 4000, 8000, 16000, 30000, 30000];
    let calls = 0;
    for (const d of delays) {
      await vi.advanceTimersByTimeAsync(d - 1);
      expect(invoke).toHaveBeenCalledTimes(calls); // 还没到点
      await vi.advanceTimersByTimeAsync(1);
      calls += 1;
      expect(invoke).toHaveBeenCalledTimes(calls); // 第 calls 次 connect 尝试
    }
  });

  it("连上后重置退避:成功一次 → reconnecting=false,再断从 1s 重新开始", async () => {
    invoke.mockRejectedValueOnce(new Error("fail")).mockResolvedValue(undefined);
    api.arm(target());
    api.onDisconnect(false);

    await vi.advanceTimersByTimeAsync(1000); // 第1次失败 → 排 2s
    await vi.advanceTimersByTimeAsync(2000); // 第2次成功
    expect(api.reconnecting.value).toBe(false);

    invoke.mockClear();
    api.onDisconnect(false); // 再次断开
    await vi.advanceTimersByTimeAsync(999);
    expect(invoke).toHaveBeenCalledTimes(0);
    await vi.advanceTimersByTimeAsync(1); // 1s 后(从 attempt=0 重新开始)
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("主动断开(cancel)后 onDisconnect 不重连", async () => {
    api.arm(target());
    api.cancel();
    api.onDisconnect(true);
    await vi.advanceTimersByTimeAsync(60000);
    expect(invoke).not.toHaveBeenCalled();
    expect(api.reconnecting.value).toBe(false);
  });

  it("未 arm 时 onDisconnect 忽略", async () => {
    api.onDisconnect(true);
    await vi.advanceTimersByTimeAsync(60000);
    expect(invoke).not.toHaveBeenCalled();
  });

  it("忠实恢复:wasStreaming=true & mode=normal → connect + auto_handshake(period)", async () => {
    api.arm(target({ mode: "normal", period: 500 }));
    api.onDisconnect(true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledWith("connect_substation", { host: "10.0.0.1", port: 8000, dataPort: 8001 });
    expect(invoke).toHaveBeenCalledWith("auto_handshake", { idcode: "10.0.0.1:8000", period: 500 });
  });

  it("忠实恢复:mode=skipCfg2 → connect + skip_cfg2_open", async () => {
    api.arm(target({ mode: "skipCfg2" }));
    api.onDisconnect(true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledWith("skip_cfg2_open", { idcode: "10.0.0.1:8000" });
  });

  it("忠实恢复:wasStreaming=false → 只 connect,不握手/开流", async () => {
    api.arm(target());
    api.onDisconnect(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("connect_substation", { host: "10.0.0.1", port: 8000, dataPort: 8001 });
  });

  it("V2 不传 dataPort", async () => {
    api.arm(target({ protocol: "V2" }));
    api.onDisconnect(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledWith("connect_substation", { host: "10.0.0.1", port: 8000, dataPort: undefined });
  });

  it("cancel 清挂起 timer", async () => {
    invoke.mockRejectedValue(new Error("fail"));
    api.arm(target());
    api.onDisconnect(false);
    await vi.advanceTimersByTimeAsync(1000); // 第1次失败,排了 2s
    invoke.mockClear();
    api.cancel();
    await vi.advanceTimersByTimeAsync(60000);
    expect(invoke).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd frontend && npx vitest run tests/use-reconnect.test.ts`
Expected: FAIL，报 `Failed to resolve import "../src/composables/useReconnect"`(文件还没建)。

- [ ] **Step 3: 写最小实现**

创建 `frontend/src/composables/useReconnect.ts`:

```ts
import { ref, type Ref } from "vue";
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

// 模块级单例状态(与 useSessions / useProtocol 同风格)。
let desired: ReconnectTarget | null = null;
let intentional = false;
let attempt = 0;
let pendingStreaming = false;
let timer: ReturnType<typeof setTimeout> | null = null;
const reconnecting: Ref<boolean> = ref(false);

function delayFor(a: number): number {
  return Math.min(BASE_DELAY_MS * 2 ** a, MAX_DELAY_MS);
}

function clearTimer(): void {
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
}

async function attemptReconnect(): Promise<void> {
  timer = null;
  if (!desired) {
    reconnecting.value = false;
    return;
  }
  const t = desired;
  const idcode = `${t.host}:${t.mgmtPort}`;
  try {
    await invoke("connect_substation", {
      host: t.host,
      port: t.mgmtPort,
      dataPort: t.protocol === "V3" ? t.dataPort : undefined,
    });
    if (pendingStreaming) {
      if (t.mode === "skipCfg2") {
        await invoke("skip_cfg2_open", { idcode });
      } else {
        await invoke("auto_handshake", { idcode, period: t.period });
      }
    }
    attempt = 0;
    reconnecting.value = false;
  } catch {
    attempt += 1;
    scheduleRetry();
  }
}

function scheduleRetry(): void {
  clearTimer();
  timer = setTimeout(() => {
    void attemptReconnect();
  }, delayFor(attempt));
}

function arm(t: ReconnectTarget): void {
  desired = t;
  intentional = false;
  attempt = 0;
}

function onDisconnect(wasStreaming: boolean): void {
  if (intentional || !desired) return;
  pendingStreaming = wasStreaming;
  reconnecting.value = true;
  scheduleRetry();
}

function cancel(): void {
  intentional = true;
  clearTimer();
  attempt = 0;
  reconnecting.value = false;
}

function _resetForTest(): void {
  desired = null;
  intentional = false;
  attempt = 0;
  pendingStreaming = false;
  clearTimer();
  reconnecting.value = false;
}

const api = { reconnecting, arm, onDisconnect, cancel, _resetForTest };

export function useReconnect() {
  return api;
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd frontend && npx vitest run tests/use-reconnect.test.ts`
Expected: PASS，9 个用例全绿。

- [ ] **Step 5: 提交**

```bash
cd "/Users/daichangyu/Library/Mobile Documents/com~apple~CloudDocs/code/PmuSim"
git add frontend/src/composables/useReconnect.ts frontend/tests/use-reconnect.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(reconnect): 新增 useReconnect 指数退避重连管理器 + 单测"
```

---

### Task 2: 在 usePmuEvents 接线断线触发

**Files:**
- Modify: `frontend/src/composables/usePmuEvents.ts`
- Test: `frontend/tests/use-pmu-events.reconnect.test.ts`(Create)

**Interfaces:**
- Consumes:`useReconnect().onDisconnect(wasStreaming: boolean)`(Task 1)。
- 接线规则:`SessionDisconnected` 与 `HeartbeatTimeout` 分支,在 `removeSession` **之前**读 `sessions.get(idcode)?.state === "streaming"` 得到 `wasStreaming`,然后调 `reconnect.onDisconnect(wasStreaming)`;`SessionDisconnected` 仅对真实会话(idcode 不含 `:`)调用,placeholder 不调。`reconnect` 必须以 `const reconnect = useReconnect()` 形式持有并以 `reconnect.onDisconnect(...)` 调用(不要解构,否则 spy 失效)。

- [ ] **Step 1: 写失败测试**

创建 `frontend/tests/use-pmu-events.reconnect.test.ts`:

```ts
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { usePmuEvents } from "../src/composables/usePmuEvents";
import { useReconnect } from "../src/composables/useReconnect";
import { useSessions } from "../src/composables/useSessions";

const reconnect = useReconnect();

beforeEach(() => {
  const { sessions, selectedIdcode } = useSessions();
  sessions.clear();
  selectedIdcode.value = "";
  reconnect._resetForTest();
  invoke.mockReset();
  vi.useFakeTimers();
});
afterEach(() => vi.useRealTimers());

function seedSession(idcode: string, state: string) {
  const { sessions } = useSessions();
  sessions.set(idcode, { idcode, peerIp: "1.1.1.1", state: state as never });
}

describe("usePmuEvents 断线触发自动重连", () => {
  it("真实会话 SessionDisconnected(streaming) → onDisconnect(true)", async () => {
    const spy = vi.spyOn(reconnect, "onDisconnect");
    seedSession("PMU1", "streaming");
    invoke.mockResolvedValueOnce([{ type: "SessionDisconnected", idcode: "PMU1" }]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);
    expect(spy).toHaveBeenCalledWith(true);
  });

  it("HeartbeatTimeout(非 streaming) → onDisconnect(false)", async () => {
    const spy = vi.spyOn(reconnect, "onDisconnect");
    seedSession("PMU1", "cfg2_sent");
    invoke.mockResolvedValueOnce([{ type: "HeartbeatTimeout", idcode: "PMU1" }]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);
    expect(spy).toHaveBeenCalledWith(false);
  });

  it("placeholder(host:port)SessionDisconnected 不触发重连", async () => {
    const spy = vi.spyOn(reconnect, "onDisconnect");
    invoke.mockResolvedValueOnce([{ type: "SessionDisconnected", idcode: "10.0.0.1:8000" }]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);
    expect(spy).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd frontend && npx vitest run tests/use-pmu-events.reconnect.test.ts`
Expected: FAIL（`reconnect.onDisconnect` 从未被调用,断言不通过)。

- [ ] **Step 3: 改 usePmuEvents 接线**

`frontend/src/composables/usePmuEvents.ts`:

顶部加 import(在现有 import 区,第 8 行 `frameTimeMs` import 之后):
```ts
import { useReconnect } from "./useReconnect";
```

在 `usePmuEvents()` 体内,把现有解构补上 `sessions` 并取得 `reconnect`(现状是 `const { addSession, updateState, removeSession, setConfig, configs } = useSessions();`):
```ts
  const { sessions, addSession, updateState, removeSession, setConfig, configs } = useSessions();
  const reconnect = useReconnect();
```

把 `SessionDisconnected` 分支替换为:
```ts
      case "SessionDisconnected": {
        const wasStreaming = sessions.get(payload.idcode)?.state === "streaming";
        removeSession(payload.idcode);
        if (!payload.idcode.includes(":")) {
          pushEvent(t("event.pipeDisconnected", { idcode: payload.idcode }));
          reconnect.onDisconnect(wasStreaming);
        }
        resetFrameRate();
        break;
      }
```

把 `HeartbeatTimeout` 分支替换为:
```ts
      case "HeartbeatTimeout": {
        const wasStreaming = sessions.get(payload.idcode)?.state === "streaming";
        pushToast(t("event.heartbeatTimeoutToast", { idcode: payload.idcode }), "error");
        pushEvent(t("event.heartbeatTimeout", { idcode: payload.idcode }), "error");
        removeSession(payload.idcode);
        resetFrameRate();
        reconnect.onDisconnect(wasStreaming);
        break;
      }
```

(注意:给这两个 `case` 加了 `{ }` 块作用域以容纳 `const wasStreaming`。)

- [ ] **Step 4: 跑测试确认通过**

Run: `cd frontend && npx vitest run tests/use-pmu-events.reconnect.test.ts`
Expected: PASS，3 个用例全绿。

- [ ] **Step 5: 提交**

```bash
cd "/Users/daichangyu/Library/Mobile Documents/com~apple~CloudDocs/code/PmuSim"
git add frontend/src/composables/usePmuEvents.ts frontend/tests/use-pmu-events.reconnect.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(reconnect): 断线/心跳超时事件触发自动重连"
```

---

### Task 3: 在 ConfigInfoPanel 接线 arm/cancel + 重连中状态 + i18n

**Files:**
- Modify: `frontend/src/components/ConfigInfoPanel.vue`
- Modify: `frontend/src/i18n/messages.ts`
- Test: `frontend/tests/config-info-panel.reconnect.test.ts`(Create)

**Interfaces:**
- Consumes:`useReconnect().arm(target)`、`.cancel()`、`.reconnecting`(Task 1)。
- 接线规则:`startEverything` 成功末尾 `reconnect.arm({...表单, mode:"normal"})`;`skipCfg2Connect` 成功末尾 `reconnect.arm({...表单, mode:"skipCfg2"})`;`stopEverything` 成功后 `reconnect.cancel()`。target 取值:`host=connIp.value.trim()`、`mgmtPort=parseInt(connMgmtPort.value)`、`dataPort=parseInt(connDataPort.value)`、`protocol=protocol.value`、`period=Number.isFinite(parseFloat(rateHz.value))?hzToPeriod(parseFloat(rateHz.value)):null`。

- [ ] **Step 1: 写失败测试**

创建 `frontend/tests/config-info-panel.reconnect.test.ts`:

```ts
import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

const { invoke, ask } = vi.hoisted(() => ({ invoke: vi.fn(), ask: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ ask }));

import ConfigInfoPanel from "../src/components/ConfigInfoPanel.vue";
import { useSessions } from "../src/composables/useSessions";
import { useReconnect } from "../src/composables/useReconnect";
import { usePmuEvents } from "../src/composables/usePmuEvents";

const reconnect = useReconnect();

beforeEach(() => {
  const { sessions, selectedIdcode } = useSessions();
  sessions.clear();
  selectedIdcode.value = "";
  reconnect._resetForTest();
  invoke.mockReset();
  invoke.mockResolvedValue(undefined);
  ask.mockReset();
  // startEverything 第一步 await listenerReady;启动轮询使其 resolve。
  usePmuEvents().startListening();
});

function findButtonByText(wrapper: ReturnType<typeof mount>, text: string) {
  const btn = wrapper.findAll("button").find((b) => b.text().includes(text));
  if (!btn) throw new Error(`button "${text}" not found`);
  return btn;
}

describe("ConfigInfoPanel 重连接线", () => {
  it("连接成功后 arm(mode=normal) 携带表单快照", async () => {
    const armSpy = vi.spyOn(reconnect, "arm");
    const wrapper = mount(ConfigInfoPanel);
    await findButtonByText(wrapper, "连接").trigger("click");
    await flushPromises();
    expect(armSpy).toHaveBeenCalledTimes(1);
    const arg = armSpy.mock.calls[0][0];
    expect(arg).toMatchObject({ host: "10.15.48.12", mgmtPort: 8000, dataPort: 8001, protocol: "V3", mode: "normal" });
    wrapper.unmount();
  });

  it("停止后 cancel 被调用", async () => {
    const cancelSpy = vi.spyOn(reconnect, "cancel");
    const { sessions, selectedIdcode } = useSessions();
    sessions.set("PMU1", { idcode: "PMU1", peerIp: "1.1.1.1", state: "streaming" });
    selectedIdcode.value = "PMU1";
    const wrapper = mount(ConfigInfoPanel);
    await findButtonByText(wrapper, "停止").trigger("click");
    await flushPromises();
    expect(cancelSpy).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });
});
```

> 注:按钮中文文案见 `messages.ts`(`config.start`=「连接」、`config.stop`=「停止」)。实现 Step 时若按钮文案不同,以 `messages.ts` 实际值为准并同步改测试的查找文本。

- [ ] **Step 2: 跑测试确认失败**

Run: `cd frontend && npx vitest run tests/config-info-panel.reconnect.test.ts`
Expected: FAIL（`armSpy` / `cancelSpy` 未被调用)。

- [ ] **Step 3: 改 ConfigInfoPanel 接线**

`frontend/src/components/ConfigInfoPanel.vue`:

import 区(第 6 行 `useSessions` import 之后)加:
```ts
import { useReconnect } from "../composables/useReconnect";
```

setup 内(`const { sessions, configs, selectedIdcode } = useSessions();` 附近)加:
```ts
const reconnect = useReconnect();

function reconnectTarget(mode: "normal" | "skipCfg2") {
  const hz = parseFloat(rateHz.value);
  return {
    host: connIp.value.trim(),
    mgmtPort: parseInt(connMgmtPort.value),
    dataPort: parseInt(connDataPort.value),
    protocol: protocol.value,
    period: Number.isFinite(hz) ? hzToPeriod(hz) : null,
    mode,
  };
}
```

在 `startEverything` 的 `try` 块,最后一句 `await invoke("auto_handshake", ...)` 之后加:
```ts
    reconnect.arm(reconnectTarget("normal"));
```

在 `skipCfg2Connect` 的 `try` 块,最后一句 `await invoke("skip_cfg2_open", ...)` 之后加:
```ts
    reconnect.arm(reconnectTarget("skipCfg2"));
```

在 `stopEverything` 的 `try` 块,`running.value = false;` 之后加:
```ts
    reconnect.cancel();
```

- [ ] **Step 4: 加"重连中"状态文案 + 显示**

`frontend/src/i18n/messages.ts`:在 zh 段 `'state.disconnected': '已断开',` 之后加:
```ts
    'state.reconnecting': '重连中…',
```
在 en 段 `'state.disconnected': 'Disconnected',` 之后加:
```ts
    'state.reconnecting': 'Reconnecting…',
```

`frontend/src/components/ConfigInfoPanel.vue` 模板:把现有状态标签(`stateLabel`,即 `{{ stateLabel }}` 所在元素)的显示文本改为优先显示重连中。先在 setup 加:
```ts
const displayState = computed(() => (reconnect.reconnecting.value ? t("state.reconnecting") : stateLabel.value));
```
再把模板里渲染 `stateLabel` 的插值改为 `displayState`(只改显示文本那一处,`stateClass` 保持不变)。

- [ ] **Step 5: 跑测试确认通过**

Run: `cd frontend && npx vitest run tests/config-info-panel.reconnect.test.ts`
Expected: PASS，2 个用例全绿。

- [ ] **Step 6: 提交**

```bash
cd "/Users/daichangyu/Library/Mobile Documents/com~apple~CloudDocs/code/PmuSim"
git add frontend/src/components/ConfigInfoPanel.vue frontend/src/i18n/messages.ts frontend/tests/config-info-panel.reconnect.test.ts
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "feat(reconnect): 连接成功 arm/停止 cancel 接线 + 重连中状态显示"
```

---

### Task 4: 集成验证 + 后端重连验证点

**Files:** 无新增(整体验证)。

- [ ] **Step 1: 全量类型检查 + 构建**

Run: `cd frontend && npm run build`
Expected: `vue-tsc` 无类型错误,`vite build` 成功输出 dist。

- [ ] **Step 2: 全量单测**

Run: `cd frontend && npm run test:unit`
Expected: 所有测试文件通过(原有 2 文件 + 本次新增 3 文件)。

- [ ] **Step 3: 手动端到端验证(后端验证点)**

跑 `cargo tauri dev`(或 `npm run dev` + tauri),连上一个子站使其进入 streaming,然后断开子站(或拔网络),观察:
1. 状态变「重连中…」,事件日志出现「管道断开」。
2. 退避节奏重连(1→2→4…s),子站恢复后自动连回并恢复数据流。
3. **关键验证**:旧 session 已移除后,对同一 target 的 `connect_substation` 不被后端残留状态阻塞(能成功重建会话)。若被阻塞,记录现象——可能需要后端在 `do_connect`/`do_disconnect`(`crates/pmusim-app/src/network/master.rs`)清理同 peer 残留,届时新开一个修复任务。
4. 点「停止」后不再自动重连(`cancel` 生效)。

- [ ] **Step 4: 最终提交(若 Step 3 仅验证无改动则跳过)**

如手动验证触发了任何后端小修,按 TDD 补测试后:
```bash
cd "/Users/daichangyu/Library/Mobile Documents/com~apple~CloudDocs/code/PmuSim"
git add -A
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "fix(reconnect): <按实际修复填写>"
```

---

## Self-Review

**Spec coverage:**
- 自动重连(SessionDisconnected + HeartbeatTimeout)→ Task 2 ✓
- 忠实恢复(streaming/暂停)→ Task 1 `wasStreaming` + Task 2 断线瞬间读 state ✓
- 指数退避封顶 30s 无限 → Task 1 `delayFor` + 退避序列测试 ✓
- 主动断开不重连 → Task 1 `cancel`/`intentional` + Task 3 `stopEverything` 接线 ✓
- server 不重启 → 复用 `connect_substation`,不碰 `start_server`(Task 1 重连动作),`running` 不变 ✓
- 只对真实会话重连(placeholder 不触发)→ Task 2 测试 ✓
- 重连中 UI 反馈 → Task 3 `state.reconnecting` ✓
- 后端验证点 → Task 4 Step 3 ✓

**Placeholder scan:** 无 TBD/TODO;所有步骤含完整代码与命令。

**Type consistency:** `ReconnectTarget` 字段(host/mgmtPort/dataPort/protocol/period/mode)在 Task 1 定义,Task 3 `reconnectTarget()` 构造一致;`onDisconnect(wasStreaming)`、`arm(target)`、`cancel()`、`reconnecting` 在三任务间签名一致。
