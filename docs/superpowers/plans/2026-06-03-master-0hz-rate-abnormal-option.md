# 主站速率下拉新增「0 Hz (异常场景)」一键注入 PERIOD=0 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在主站速率下拉新增 `0 Hz (异常场景)` 档位,作为注入非法上送周期 PERIOD=0(子站应 NACK)的一键快捷入口,选中即弹原生确认框,确认后下发、取消则回退。

**Architecture:** 纯前端改动。把 Hz→PERIOD 换算抽成可单测的纯函数模块 `lib/rate.ts`(0Hz 特判为 0);在 `ConfigInfoPanel.vue` 把速率 `watch` 重构为「选中 0Hz 立即弹 `ask()` 确认 / 取消回退」,正常档位防抖下发路径不变;复用既有 `send_cfg2` 命令链路(后端 `period: Option<u16>` 本就收 0)。后端、capabilities 零改动。

**Tech Stack:** Vue 3.5 (`<script setup>`) + TypeScript + Vite + Tauri 2(`@tauri-apps/plugin-dialog`,后端插件已注册 + `dialog:default` 已授权)。测试用 Node 26 原生型ストリップ跑 `frontend/tests/*.test.mjs`(`node:assert/strict`)。

**Spec:** `docs/superpowers/specs/2026-06-03-master-0hz-rate-abnormal-option-design.md`

---

## File Structure

- **Create** `frontend/src/lib/rate.ts` — 纯函数 `hzToPeriod(hz)`,0Hz 特判返回 0,其余 `5000/hz`。唯一职责:速率换算,可单测。
- **Create** `frontend/tests/rate.test.mjs` — `hzToPeriod` 映射表单测(对齐现有 `frontend/tests/i18n/*.test.mjs` 风格)。
- **Modify** `frontend/src/i18n/messages.ts` — 新增 3 个 key × 中英两份(parity 测试强制对等)。
- **Modify** `frontend/package.json` — 加 `@tauri-apps/plugin-dialog` 依赖。
- **Modify** `frontend/src/components/ConfigInfoPanel.vue` — 引入 `hzToPeriod` + `ask`;下拉加 0Hz 选项;重构 `watch(rateHz)`;`startEverything` 换算改用 `hzToPeriod`。

任务顺序:先纯函数(可单测)→ i18n → 依赖 → 组装组件(依赖前三者)。

---

## Task 1: 抽出 `hzToPeriod` 纯函数 + 单测(TDD)

**Files:**
- Create: `frontend/src/lib/rate.ts`
- Test: `frontend/tests/rate.test.mjs`

- [ ] **Step 1: 写失败测试**

Create `frontend/tests/rate.test.mjs`:

```js
import assert from 'node:assert/strict'
import { hzToPeriod } from '../src/lib/rate.ts'

// 正常档位：PERIOD = round(5000/hz)
assert.equal(hzToPeriod(25), 200)
assert.equal(hzToPeriod(50), 100)
assert.equal(hzToPeriod(100), 50)
assert.equal(hzToPeriod(200), 25)
// 0Hz 特判 → PERIOD=0（非法上送周期，绕开 1000/hz 除零）
assert.equal(hzToPeriod(0), 0)

console.log('rate.test.mjs OK')
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd frontend && node tests/rate.test.mjs`
Expected: FAIL — `ERR_MODULE_NOT_FOUND`(`../src/lib/rate.ts` 不存在)。

- [ ] **Step 3: 写最小实现**

Create `frontend/src/lib/rate.ts`:

```ts
/// 用户可见 Hz → CFG-2 PERIOD（单位=工频周波×100）。
/// 0Hz 特判为 PERIOD=0：非法上送周期（子站应 NACK），同时绕开 1000/hz 除零。
/// 其余档位 PERIOD = round((1000/hz)*100/20) = round(5000/hz)
/// （100→50, 50→100, 25→200, 200→25）。
export function hzToPeriod(hz: number): number {
  if (hz === 0) return 0;
  return Math.round((1000 / hz) * 100 / 20);
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd frontend && node tests/rate.test.mjs`
Expected: PASS — 输出 `rate.test.mjs OK`。

- [ ] **Step 5: 提交**

```bash
git add frontend/src/lib/rate.ts frontend/tests/rate.test.mjs
git commit -m "feat(rate): 抽出 hzToPeriod 纯函数(0Hz→PERIOD=0)+单测"
```

---

## Task 2: 新增 i18n 文案(中英对等)

**Files:**
- Modify: `frontend/src/i18n/messages.ts`(zh `:45` 后 / en `:147` 后)
- Test: `frontend/tests/i18n/messages-parity.test.mjs`(既有,无需改)

- [ ] **Step 1: 加中文键**

在 `frontend/src/i18n/messages.ts` 的 zh 块,`'config.injectBadValue': 'PERIOD 取值非法',` 这一行之后插入:

```ts
    'config.rateAbnormalTag': '异常场景',
    'config.inject0Title': '异常注入确认',
    'config.inject0Confirm': '确认向子站注入非法上送周期 PERIOD=0?合规子站应以 NACK 拒绝。',
```

- [ ] **Step 2: 加英文键**

在同文件 en 块,`'config.injectBadValue': 'Invalid PERIOD value',` 这一行之后插入:

```ts
    'config.rateAbnormalTag': 'abnormal',
    'config.inject0Title': 'Abnormal injection',
    'config.inject0Confirm': 'Inject illegal reporting period PERIOD=0 to the substation? A compliant substation should reject with NACK.',
```

- [ ] **Step 3: 跑 parity 测试确认中英对等**

Run: `cd frontend && node tests/i18n/messages-parity.test.mjs`
Expected: PASS — 输出 `messages-parity.test.mjs OK (95 keys)`(原 92 + 3)。
> 若报 `keys missing from en/zh` 说明某一侧漏加,补齐对应键。

- [ ] **Step 4: 提交**

```bash
git add frontend/src/i18n/messages.ts
git commit -m "feat(i18n): 0Hz 异常档位文案(rateAbnormalTag/inject0Title/inject0Confirm)"
```

---

## Task 3: 新增 `@tauri-apps/plugin-dialog` 前端依赖

**Files:**
- Modify: `frontend/package.json`(`dependencies`)

> 后端 `crates/pmusim-app/src/main.rs:11` 已 `.plugin(tauri_plugin_dialog::init())`,`capabilities/default.json` 已含 `dialog:default`(含 `allow-ask`)。仅前端缺 npm 包。

- [ ] **Step 1: 安装依赖**

Run: `cd frontend && npm install @tauri-apps/plugin-dialog@^2`
Expected: `package.json` 的 `dependencies` 出现 `"@tauri-apps/plugin-dialog": "^2"`,`node_modules` 安装成功。

- [ ] **Step 2: 验证可解析**

Run: `cd frontend && node -e "import('@tauri-apps/plugin-dialog').then(m => console.log(typeof m.ask))"`
Expected: 输出 `function`(`ask` 可导入)。

- [ ] **Step 3: 提交**

```bash
git add frontend/package.json frontend/package-lock.json
git commit -m "build(frontend): 加 @tauri-apps/plugin-dialog 依赖"
```
> 若仓库无 `package-lock.json`(本项目当前未见),仅 `git add frontend/package.json`。

---

## Task 4: `ConfigInfoPanel.vue` 组装(选项 + 确认/回退 + 启动换算)

**Files:**
- Modify: `frontend/src/components/ConfigInfoPanel.vue`
  - import 区(`:1-12`)
  - `startEverything` 换算(`:166-171`)
  - `watch(rateHz)`(`:257-271`)
  - template 速率 `<select>`(`:290-293`)

> 本任务无组件级自动化测试(项目无 Vue 组件测试栈),验证 = `npm run build`(vue-tsc 类型检查 + vite 构建)+ 手动 e2e 清单。各步为同一文件的连续编辑,逐步替换。

- [ ] **Step 1: 加 import**

在 `frontend/src/components/ConfigInfoPanel.vue` 顶部 import 区(`useI18n` 那行之后,`:12`)追加两行:

```ts
import { ask } from "@tauri-apps/plugin-dialog";
import { hzToPeriod } from "../lib/rate";
```

- [ ] **Step 2: `startEverything` 换算改用 `hzToPeriod`(允许 0)**

把 `:166-171`:

```ts
    const hz = parseFloat(rateHz.value);
    let periodVal: number | null = null;
    if (Number.isFinite(hz) && hz > 0) {
      // period_ms = 1000/Hz; cycles = period_ms * 50/1000 = period_ms/20; PERIOD = cycles*100
      periodVal = Math.round((1000 / hz) * 100 / 20);
    }
```

替换为:

```ts
    // hzToPeriod(0)=0 → 0Hz 选中时握手即带非法 PERIOD=0(选中时已弹确认,此处不再二次确认)。
    const hz = parseFloat(rateHz.value);
    const periodVal: number | null = Number.isFinite(hz) ? hzToPeriod(hz) : null;
```

- [ ] **Step 3: 重构 `watch(rateHz)` —— 0Hz 确认/回退,正常档位防抖不变**

把整块 `:257-271`:

```ts
watch(rateHz, debounced<string>(250, async (v) => {
  const s = session.value;
  if (!s) return;
  if (s.state !== "streaming" && s.state !== "cfg2_sent") return;
  const hz = parseFloat(v);
  if (!Number.isFinite(hz) || hz <= 0) return;
  const periodVal = Math.round((1000 / hz) * 100 / 20);
  try {
    await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2_cmd", period: null });
    await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2", period: periodVal });
    pushToast(t("config.rateApplied", { hz: String(hz) }), "info");
  } catch (e) {
    pushToast(t("config.rateFailed", { error: toastError(e) }), "error");
  }
}));
```

替换为:

```ts
// 正常档位(25/50/100/200)实时下发 CFG-2 —— 防抖路径,行为不变。
const applyNormalRate = debounced<string>(250, async (v) => {
  const s = session.value;
  if (!s) return;
  if (s.state !== "streaming" && s.state !== "cfg2_sent") return;
  const hz = parseFloat(v);
  if (!Number.isFinite(hz) || hz <= 0) return;
  const periodVal = hzToPeriod(hz);
  try {
    await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2_cmd", period: null });
    await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2", period: periodVal });
    pushToast(t("config.rateApplied", { hz: String(hz) }), "info");
  } catch (e) {
    pushToast(t("config.rateFailed", { error: toastError(e) }), "error");
  }
});

// 选中「0 Hz (异常场景)」即弹原生确认框:取消→回退上一档(suppress 跳过回退引发的自触发,
// 避免误发上一档 CFG-2);确认→在线则立即注入 PERIOD=0,未连接则仅保留选中由启动带下去。
// 确认早于连接状态护栏 → 无论连接与否,选中 0Hz 都恰好确认一次。
let suppressRateWatch = false;
watch(rateHz, async (v, old) => {
  if (suppressRateWatch) { suppressRateWatch = false; return; }
  if (v === "0") {
    const ok = await ask(t("config.inject0Confirm"), {
      title: t("config.inject0Title"),
      kind: "warning",
    });
    if (!ok) {
      suppressRateWatch = true;
      rateHz.value = old;
      return;
    }
    const s = session.value;
    if (s && (s.state === "streaming" || s.state === "cfg2_sent")) {
      try {
        await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2_cmd", period: null });
        await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2", period: 0 });
        pushToast(t("config.injectSent", { period: "0" }), "info");
      } catch (e) {
        pushToast(t("config.injectFailed", { error: toastError(e) }), "error");
      }
    }
    return;
  }
  applyNormalRate(v);
});
```

- [ ] **Step 4: 下拉加 0Hz 选项**

把 template 速率 `<select>`(`:290-293`)的 `200 Hz` 那行之后追加一项:

```html
            <option value="200">200 Hz</option>
            <option value="0">0 Hz ({{ t('config.rateAbnormalTag') }})</option>
```

- [ ] **Step 5: 类型检查 + 构建**

Run: `cd frontend && npm run build`
Expected: PASS — vue-tsc 无类型错误(`ask`/`hzToPeriod` 已正确 import,`old` 推断为 `string`),vite 构建产出 `dist/`。
> 若 `old` 报「可能 undefined」类型问题:`watch` 同步触发时 `old` 为上一档字符串,可断言 `rateHz.value = old as string`;但默认 `rateHz` 初值 "100" 且 0 永不为初值,正常推断即为 `string`。

- [ ] **Step 6: 回归既有前端测试**

Run: `cd frontend && node tests/rate.test.mjs && node tests/i18n/messages-parity.test.mjs && node tests/i18n/detect.test.mjs`
Expected: 三个均 PASS。

- [ ] **Step 7: 提交**

```bash
git add frontend/src/components/ConfigInfoPanel.vue
git commit -m "feat(master): 速率下拉新增 0Hz(异常场景)一键注入 PERIOD=0(弹确认/取消回退)"
```

---

## Task 5: 手动端到端验证(子站联调)

> 后端注入链路在 V3 已打通,本次仅 UI 提面,故以手动 e2e 收尾。需同时运行主站 `pmusim-app` 与子站 `pmusim-sub`(或实验室子站)。

- [ ] **Step 1: 启动主站 + 子站,建立 streaming**

Run(主站):`cargo run -p pmusim-app`;子站另起 `cargo run -p pmusim-sub`。
主站填子站地址 → 选 100 Hz → 点「连接/启动」→ 状态变 streaming、有数据。

- [ ] **Step 2: streaming 中选 0Hz —— 确认路径**

速率下拉选 `0 Hz (异常场景)` → 弹原生确认框 → 点确认。
Expected:事件日志出现「下发 CFG-2」+ 子站回 NACK(主站 emit Error「子站 NACK」),子站侧 `Cfg2Rejected`、fps 不变。

- [ ] **Step 3: streaming 中选 0Hz —— 取消路径**

再次把档位切到 100 Hz(正常),然后选 `0 Hz (异常场景)` → 弹确认框 → 点取消。
Expected:下拉回退到 100 Hz,无任何 CFG-2 下发(事件日志无新增注入),速率仍 100Hz 流。

- [ ] **Step 4: 未连接选 0Hz → 启动带下去**

停止 → 速率下拉选 `0 Hz (异常场景)` → 弹确认 → 确认 → 下拉保持 0Hz → 点「连接/启动」。
Expected:握手即以 PERIOD=0 下发,子站 NACK;启动过程**不**二次弹确认。

- [ ] **Step 5: 回归正常档位**

切 25/50/100/200 Hz,各档实时下发 + 速率回显正常,行为与改动前一致(无确认框、防抖生效)。

- [ ] **Step 6: 收尾提交(如手动验证中发现并修了小问题)**

```bash
git add -A && git commit -m "fix(master): 0Hz 注入手动 e2e 修正"
```
> 若无修正则跳过本步。

---

## Self-Review

**Spec coverage(逐条对照 spec §4 改动清单):**
- (A) 依赖 `@tauri-apps/plugin-dialog` → Task 3 ✓
- (B) 下拉新增 `0 Hz (异常场景)` 选项 → Task 4 Step 4 ✓
- (C) 抽 `hzToPeriod` + `startEverything` 改用 → Task 1 + Task 4 Step 2 ✓
- (D) `watch` 确认/注入/回退(suppress) → Task 4 Step 3 ✓
- (E) i18n 3 键中英 → Task 2 ✓
- spec §7 测试:`hzToPeriod` 映射单测 → Task 1;手动 e2e → Task 5 ✓
- spec §8 不做项(不动异常注入区、后端零改、不二次确认)→ 各任务均未触及,Task 4 Step 2 注释明确「不再二次确认」✓

**Placeholder scan:** 无 TBD/TODO;所有代码步均含完整代码块与确切命令、预期输出。

**Type consistency:** 函数名全程 `hzToPeriod`;新 i18n 键 `config.rateAbnormalTag` / `config.inject0Title` / `config.inject0Confirm` 在 Task 2 定义、Task 4 Step 3/4 引用,拼写一致;`ask` 选项 `{ title, kind }` 与 plugin-dialog 签名一致;复用既有 `config.injectSent`/`config.injectFailed`(已存在,无需新增)。
