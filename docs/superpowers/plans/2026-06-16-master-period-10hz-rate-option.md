# 主站速率下拉支持 10Hz 合法档位 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 主站速率下拉新增 10Hz 合法档位（PERIOD=500），与 25/50/100/200 同等走正常 CFG-2 下发路径。

**Architecture:** 仅前端单文件改动——在 `ConfigInfoPanel.vue` 速率 `<select>` 最前插入 `10 Hz` 选项。后端 `do_send_cfg2` 已透传任意 `period`、watch 已把非 "0" 档位路由到 `applyNormalRate`、readback 已通用，故无需任何后端/逻辑/i18n 改动。TDD：先加失败的回归测试，再加选项使其转绿。

**Tech Stack:** Vue 3 SFC、TypeScript、Tauri `invoke`、Vitest 4 + @vue/test-utils（happy-dom）。

---

## File Structure

- **Modify:** `frontend/src/components/ConfigInfoPanel.vue` — 速率 `<select>`（318–324 行）新增一个 `<option>`。
- **Modify:** `frontend/tests/config-info-panel.0hz.test.ts` — 新增 10Hz 正常档位回归用例。

> **对 spec 的细化（DRY）：** spec 原写"新增 `config-info-panel.10hz.test.ts`"。但该 0Hz 测试文件已混入正常档位用例（现有 50Hz→PERIOD=100 的 `it()`），且共享 `invoke`/`ask` mock、`setStreaming()`、`rateSelect()` 等全部脚手架。新建独立文件会复制这些样板，违反 DRY。故改为在既有文件追加一条 `it()`。

---

## Task 1: 新增 10Hz 正常档位（TDD）

**Files:**
- Modify: `frontend/tests/config-info-panel.0hz.test.ts`（在 `describe(...)` 块末尾、最后一个 `it(...)` 之后、`});` 之前追加）
- Modify: `frontend/src/components/ConfigInfoPanel.vue:318-324`

- [ ] **Step 1: 写失败测试**

在 `frontend/tests/config-info-panel.0hz.test.ts` 中，最后一个 `it("streaming 时选正常档位 50Hz ...")` 用例之后、`describe` 收尾的 `});` 之前，插入：

```typescript
  it("streaming 时选 10Hz → 防抖后下发 PERIOD=500，且无确认框", async () => {
    setStreaming();
    const wrapper = mount(ConfigInfoPanel);

    await rateSelect(wrapper).setValue("10");
    await new Promise((r) => setTimeout(r, 300)); // 等 250ms 防抖
    await flushPromises();

    expect(ask).not.toHaveBeenCalled(); // 正常档位：不弹确认框
    expect(invoke).toHaveBeenCalledWith("send_command", { idcode: "PMU1", cmd: "send_cfg2_cmd", period: null });
    expect(invoke).toHaveBeenCalledWith("send_command", { idcode: "PMU1", cmd: "send_cfg2", period: 500 }); // hzToPeriod(10)
    wrapper.unmount();
  });
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd frontend && npx vitest run tests/config-info-panel.0hz.test.ts`
Expected: 新用例 FAIL —— 因 `<select>` 无 `value="10"` 选项，`setValue("10")` 不会落到 `"10"`，`applyNormalRate("10")` 不触发，`send_cfg2 period:500` 从未调用（断言 `toHaveBeenCalledWith(... period: 500)` 失败）。其余既有用例仍 PASS。

- [ ] **Step 3: 加下拉选项（最小实现）**

编辑 `frontend/src/components/ConfigInfoPanel.vue`，把速率 `<select>` 改为（在 `25 Hz` 行之前插入 `10 Hz` 行）：

```html
          <select v-model="rateHz">
            <option value="10">10 Hz</option>
            <option value="25">25 Hz</option>
            <option value="50">50 Hz</option>
            <option value="100">100 Hz</option>
            <option value="200">200 Hz</option>
            <option value="0">0 Hz ({{ t('config.rateAbnormalTag') }})</option>
          </select>
```

不改 `rateHz` 默认值、不改 watch / `applyNormalRate` / readback。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd frontend && npx vitest run tests/config-info-panel.0hz.test.ts`
Expected: PASS（含新 10Hz 用例）。

再跑全套确认无回归：
Run: `cd frontend && npm run test:unit`
Expected: 全部 PASS。

- [ ] **Step 5: 提交**

```bash
git add frontend/src/components/ConfigInfoPanel.vue frontend/tests/config-info-panel.0hz.test.ts
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "feat(master): 速率下拉新增 10Hz 合法档位(PERIOD=500)"
```

---

## 验收标准（与 spec 对齐）

1. 下拉出现 `10 Hz` 且位于最前（顺序 `10/25/50/100/200/0`）。
2. streaming 时选 10Hz：无确认框；下发 `send_cfg2_cmd(null)` + `send_cfg2(500)`；readback 显示 `(10.0Hz)`（period=500 → 100ms → 10.0Hz，已由现有 readback 逻辑保证，无需新代码）。
3. 新增 vitest 用例通过；`npm run test:unit` 全绿。

## 不做（YAGNI）

- 不改默认档位、不动子站模拟器、不碰版本号/CHANGELOG（发版由 `/release` 单独处理）。
