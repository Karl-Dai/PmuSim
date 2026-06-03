# 设计：主站速率下拉新增「0 Hz (异常场景)」一键注入 PERIOD=0

- 日期：2026-06-03
- 规约范围：仅 V3（GB/T 26865.2-2011），与 [2026-06-02-cfg2-illegal-period-injection-design](2026-06-02-cfg2-illegal-period-injection-design.md) 同一异常语义
- 改动面：**纯前端**（`frontend/`），后端零改动

## 1. 背景与目标

注入非法上送周期 **PERIOD=0**（子站应回 NACK）的端到端链路已在 V3 打通：主站 `send_cfg2` 的 `period: Option<u16>` 本就接受 0，子站 `illegal_period_reason()` 判非法后回 NACK 并保持原 fps，主站收 NACK 后 emit Error。

当前唯一入口是 `ConfigInfoPanel.vue` 的「异常注入」勾选区：勾选 → 手填原始 PERIOD（默认 0）→ 点「注入」。PERIOD=0 是这块最常用的异常值，但需要三步操作。

**目标**：把 PERIOD=0 提升为主站速率下拉里一个一键可选档位 **`0 Hz (异常场景)`**，选中即弹原生确认框，确认后下发。降低最常用异常场景的操作成本，同时保留原「异常注入」区注入其它非法值（1 / 65535 等）和「跳过 CFG-2」的能力。

## 2. 设计决策（已确认）

| 维度 | 取值 |
| --- | --- |
| 0Hz 语义映射 | `0 Hz` → `PERIOD=0`（与现有 rawPeriod=0 注入等价） |
| 选中 0Hz 的行为 | 实时下发，但**先弹原生确认框** |
| 确认时机 | 「选中即确认一次」——值变为 `"0"` 时立即确认（早于连接状态护栏） |
| 取消处理 | 回退下拉到上一档，**不下发**（用 suppress 标志避免回退引发的二次 CFG-2） |
| 确认机制 | Tauri `@tauri-apps/plugin-dialog` 的 `ask()`（插件已注册 + `dialog:default` 已授权） |
| 与现有「异常注入」区关系 | **完全保留不动**，0Hz 仅作快捷入口 |
| 选项位置 / 文案 | 下拉**最后一档**（异常离群），`0 Hz (异常场景)` / `0 Hz (abnormal)` |
| 后端 | 零改动（`send_cfg2` 已收 0；dialog 插件已注册 + 授权） |

## 3. 现状锚点（`frontend/src/components/ConfigInfoPanel.vue`）

- 速率下拉 `v-model="rateHz"`，选项 25/50/100/200 Hz（template `:289-294`），无 0 档。
- `watch(rateHz)`（`:257-271`）：streaming/cfg2_sent 时实时下发 CFG-2；换算 `Math.round((1000/hz)*100/20)`，护栏 `hz<=0 → return`（**当前 0 被无声吞掉**）。包了 `debounced(250)`。
- `startEverything`（`:166-171`）：`if (hz>0)` 才算 `periodVal`，否则 null（→ auto_handshake 用子站自身 period）。
- 后端 dialog：`crates/pmusim-app/src/main.rs:11` 已 `.plugin(tauri_plugin_dialog::init())`；`capabilities/default.json` 含 `dialog:default`（含 `allow-ask`）。`frontend/package.json` **尚未**含 `@tauri-apps/plugin-dialog`，需新增。

## 4. 改动清单（仅 `frontend/`）

### (A) 依赖：`frontend/package.json`

新增 `"@tauri-apps/plugin-dialog": "^2"`，`npm install`。

### (B) `ConfigInfoPanel.vue` —— 下拉选项

template 速率 `<select>` 末尾（200 Hz 之后）加：

```html
<option value="0">0 Hz ({{ t('config.rateAbnormalTag') }})</option>
```

### (C) `ConfigInfoPanel.vue` —— 抽 `hzToPeriod`

消除 `startEverything` 与 `watch` 两处重复换算，统一特判 0：

```ts
// Hz → CFG-2 PERIOD（工频周波×100）。0Hz 特判为 PERIOD=0（非法上送周期，
// 绕开 1000/hz 除零）；其余档位 = 5000/hz（100→50,50→100,25→200,200→25）。
function hzToPeriod(hz: number): number {
  if (hz === 0) return 0;
  return Math.round((1000 / hz) * 100 / 20);
}
```

`startEverything`（`:166-171`）：用 `hzToPeriod(hz)` 取代 `if (hz>0)` 分支，允许 period=0 随 `auto_handshake` 握手带下去。0Hz 在此**不再二次弹确认**（已在选中时确认过，见 (D)）。

### (D) `ConfigInfoPanel.vue` —— 选中 0Hz 的确认/注入/回退

把 0Hz 的确认/回退判断从 `debounced` 包装里提到 `watch` 回调直接处理（需要可靠的「上一档」值，用 Vue 的 `(newV, oldV)`）；实际 CFG-2 下发仍走 250ms 防抖。结构示意（非最终代码）：

```ts
let suppress = false;
watch(rateHz, async (v, old) => {
  if (suppress) { suppress = false; return; }       // 回退引发的自触发，跳过
  if (v === "0") {
    const ok = await ask(t('config.inject0Confirm'),
                         { title: t('config.inject0Title'), kind: 'warning' });
    if (!ok) { suppress = true; rateHz.value = old; return; }  // 取消 → 回退，不下发
    // 已确认：streaming/cfg2_sent → 立即注入；未连接 → 仅保留选中，由 start 带下去
    const s = session.value;
    if (s && (s.state === 'streaming' || s.state === 'cfg2_sent')) {
      await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2_cmd", period: null });
      await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2", period: 0 });
      pushToast(t('config.injectSent', { period: "0" }), "info");
    }
    return;
  }
  normalRateApply(v);   // 25/50/100/200：现有防抖下发逻辑原样保留
});
```

> 要点：
> - 确认早于连接状态护栏 → 无论连接与否，选中 0Hz 都恰好确认一次。
> - 取消回退用 `suppress` 跳过自触发，避免误发上一档 CFG-2。
> - 正常档位的防抖下发路径**保持不变**。
> - 成功 toast 复用现有 `config.injectSent`（`已注入 PERIOD={period}`）。

### (E) `frontend/src/i18n/messages.ts` —— 新增键（中/英）

| key | 中文 | English |
| --- | --- | --- |
| `config.rateAbnormalTag` | `异常场景` | `abnormal` |
| `config.inject0Title` | `异常注入确认` | `Abnormal injection` |
| `config.inject0Confirm` | `确认向子站注入非法上送周期 PERIOD=0?合规子站应以 NACK 拒绝。` | `Inject illegal reporting period PERIOD=0 to the substation? A compliant substation should reject with NACK.` |

## 5. 交互流（选中 0Hz）

```
用户在速率下拉选 0 Hz (异常场景)
  → watch(rateHz, v="0") 立即 ask() 弹原生确认框
    ├─ 取消 → rateHz 回退上一档(suppress 跳过自触发)，无任何下发
    └─ 确认
        ├─ 当前 streaming/cfg2_sent → send_cfg2_cmd(null)+send_cfg2(0)
        │     → 子站 NACK + Cfg2Rejected(子站UI) → 主站 emit Error(主站UI) [V3 既有链路]
        └─ 未连接 → 仅保留 0Hz 选中；点「连接/启动」时 hzToPeriod(0)=0 随 auto_handshake 带下去
```

## 6. 错误处理 / 边界

- **除零**：`hzToPeriod(0)` 特判返回 0，永不 `1000/0`。
- **回退自触发**：`rateHz.value = old` 会再次触发 watch；`suppress` 标志在赋值前置位、在下一次 watch 回调首行消费，避免误发上一档 CFG-2。与 250ms 防抖不冲突（确认/回退在 watch 回调同步段处理，防抖只包正常档位的实际下发）。
- **未连接选 0Hz**：watch 已确认但 state 不满足 → 不立即下发，仅保留选中；`startEverything` 用 `hzToPeriod(0)=0` 带下去，不二次确认。
- **速率回显** `ratePeriodReadback`（`:128-135`）对 `period<=0` 返回空串，0Hz 注入后回显为空，不崩。
- **dialog 权限**：`dialog:default` 已含 `allow-ask`，无需改 capabilities。

## 7. 测试

- **单元**（若 `frontend/tests` 有 vitest）：`hzToPeriod` 映射表 —— `0→0, 25→200, 50→100, 100→50, 200→25`。
- **手动 e2e**：
  1. streaming 中选 `0 Hz (异常场景)` → 弹确认 →（确认）事件日志见下发 CFG-2 + 子站 NACK + 主站 Error；（取消）下拉回退原档、无下发。
  2. 未连接选 0Hz → 确认 → 点「连接」→ 握手带 PERIOD=0，子站 NACK。
  3. 正常档位 25/50/100/200 实时切换行为与防抖**不变**（回归）。

## 8. 明确不做（Out of scope）

- 不动现有「异常注入」勾选区（rawPeriod 手填 / 跳过 CFG-2）。
- 不改后端任何代码（`send_cfg2`、子站 NACK 链路、capabilities 均不动）。
- 不新增 0 以外的速率档位，不覆盖 V2。
- `startEverything` 路径不二次弹确认（确认归一到选中时刻）。
