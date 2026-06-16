# 主站速率下拉支持 10Hz 合法档位 — 设计

日期：2026-06-16

## 背景与目标

主站速率下拉当前提供 `25 / 50 / 100 / 200 Hz` 四个正常档位与 `0 Hz（异常场景）`。本次新增 **10Hz** 作为正常合法档位。

IEEE C37.118 中 10fps 对 50Hz 系统是标准合法上送率，因此 10Hz 与 25/50/100/200 同列，**走正常路径**：选中即下发 CFG-2，不弹确认框、不带"异常"标签。

换算：`hzToPeriod(10) = round((1000/10)*100/20) = round(5000/10) = 500`，即 PERIOD=500。

## 前置确认（已验证现有代码已支持）

- 后端 `do_send_cfg2`（`crates/pmusim-app/src/network/master.rs:1369`）对 `period: Option<u16>` 不做取值校验，直接写入 PMU#0，PERIOD=500 即生效。**无需后端改动。**
- 速率 watch（`frontend/src/components/ConfigInfoPanel.vue:275`）仅把 `v === "0"` 路由到异常确认框，其余档位走 `applyNormalRate`（防抖 250ms 后下发 CFG-2）。10 自动走正常路径。
- readback（`ConfigInfoPanel.vue:128`）：period=500 → `periodMs = (500/100)*(1000/50) = 100ms` → `1000/100 = 10.0Hz`，回显正确。
- 档位标签硬编码（与 `25 Hz`/`50 Hz` 一致），仅"异常场景"标签走 i18n。**无需 i18n 改动。**

## 改动点

### 1. `frontend/src/components/ConfigInfoPanel.vue`（唯一生产代码改动）

在速率 `<select>`（当前 318–324 行）最前插入：

```html
<option value="10">10 Hz</option>
```

最终顺序：`10 / 25 / 50 / 100 / 200 / 0(异常)`（数值升序，异常 0 殿后）。

- 不改默认值（仍 `ref("100")`）。
- 不改 watch、`applyNormalRate`、readback。

### 2. 后端：无改动

### 3. i18n：无改动

### 4. 新增测试 `frontend/tests/config-info-panel.10hz.test.ts`

对称于既有 `config-info-panel.0hz.test.ts`，沿用其 `invoke`/`ask` mock 与 `setStreaming()` 套路。

- **用例一**：streaming 时选 10Hz → **不调用** `ask`（无确认框）；`invoke` 以 `{ idcode:"PMU1", cmd:"send_cfg2", period:500 }` 被调用（前置 `send_cfg2_cmd` period:null 同 0Hz 路径）。
- **用例二（可选）**：未 streaming 时选 10Hz → 不下发任何 CFG-2。

实现注意：正常档位经 `applyNormalRate = debounced(250ms)`，测试需 `vi.useFakeTimers()` 并推进 250ms 后再断言（0Hz 路径不防抖，故原测试无此问题）。

## 数据流

选 10Hz → watch 命中 `v !== "0"` → `applyNormalRate("10")`（防抖）→ `hzToPeriod(10)=500` → `invoke send_cfg2_cmd(null)` + `invoke send_cfg2(500)` → CFG-2 下发；readback 回显 `(10.0Hz)`。

## 不做（YAGNI）

- 不改默认档位。
- 不动子站模拟器（本次仅"主站"）。
- 不碰版本号 / CHANGELOG（发版由 `/release` 单独处理）。

## 验收标准

1. 下拉出现 `10 Hz`，位于最前。
2. streaming 时选 10Hz 无确认框，下发 CFG-2 PERIOD=500，readback 显示 `(10.0Hz)`。
3. 新增 vitest 用例通过；现有测试全绿。
