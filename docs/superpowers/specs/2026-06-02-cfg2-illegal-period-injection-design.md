# 设计：受控注入 V3 下传 CFG-2 的非法上送周期 PERIOD=0

- 日期：2026-06-02
- 规约范围：仅 V3（GB/T 26865.2-2011）
- 对照规约：《PMU 协议 V3 版报文解析指导手册》§6 / §8.4 / §8.6

## 1. 背景与问题

主站（`pmusim-app`）可主动构建并下发 CFG-2（`master.rs:do_send_cfg2`），并带一个用户指定的上送周期 `period` 覆盖值。当前对 `period` **零校验**：PERIOD=0 会被原样写入 CFG-2 帧的 PMU 块及顶层 `period`，`build_config` 序列化后直接上线。

但这个「异常场景」目前**端到端无意义**：

- 子站（`pmusim-sub`）收到主站下发的 CFG-2 后（`substation.rs:211-215`）**只 emit `Cfg2Received` + 无条件回 ACK，从不校验 period、也不据此改速率**。子站真实速率永远来自它自己的 `data_rate_fps`，且 `start_stream` 两处 `.max(1)` 兜底（`:278`/`:307`），不会停发/狂发/除零。
- 主站 `ConfigFrame::period_ms()`（`frame.rs:88`）是死代码（全仓无人调用），0 也不触发除零。

即：当前对 PERIOD=0 的行为是「无校验放行 + 下游无反应」，谈不上「支持异常场景」。

### 既存缺陷（顺带修复）

主站对 NACK 的处理在「实时改速率 / 注入」这类 fire-and-forget 路径下**静默丢弃**：

- `do_auto_handshake`（`master.rs:1544-1555`）会先 `install_ack_waiter` 再下发，`wait_for_ack` 能消费 NACK 并 emit Error ✓。
- 但实时改速率路径（前端 `watch(rateHz)` → `send_cfg2` → `do_send_cfg2`）**不装 ack waiter**。子站若回 NACK，到 `master.rs:1109-1120` 时 `pending_ack` 为 None → **被静默丢弃，UI 看不到**。

## 2. 规约依据

PERIOD=0 在 IEEE C37.118.2（GB/T 26865.2 V3 的母规约）数据率字段中**超出定义域**：该字段只定义「正值=帧/秒、负值=秒/帧」两种合法语义，**0 不在定义域内，属未定义行为**。GB/T 26865.2 V3 在本仓库的 PERIOD（u16，单位=工频周波×100，`period_ms=(period/100)×(1000/工频)`）同理只表达合法上送周期，PERIOD=0 = 周期为零 = 无穷速率，物理无意义、非合法取值。

因此规约**不赋予 PERIOD=0 任何语义**，也不规定「停发/狂发」等接收方行为——这正是它属于「异常场景」的根本原因。但规约规定了处理**非法配置**的机制：

- **§6**：「主站宜具有 CFG1/CFG2 配置帧的**校验机制**」（仓库 `docs/TODO.md` §7 已据此在主站侧加了通道名长度校验）。
- **§8.4 / §8.6**：子站收到下传的 CFG-2 命令/帧后**必须回 ACK(0xE000) 或 NACK(0x2000)**，NACK 表示「配置不兼容/拒绝」。

合起来，规约一致的「应对」即：**子站校验出非法上送周期 → 回 NACK 拒绝、不应用、保持原状；主站收到 NACK → 中止并告警。** 本设计严格只走这条规约一致路径，不引入臆造的故障语义。

## 3. 设计决策（已确认）

| 维度 | 取值 |
| --- | --- |
| 子站应对路线 | 纯规约 NACK（识别非法 → NACK + 保持原 fps + 事件） |
| 「非法」判定边界 | 仅 `PERIOD==0`（校验函数预留扩展点） |
| 主站注入入口 | 异常开关 + 原始 PERIOD 输入（允许 0，绕过 Hz 换算） |
| 规约版本 | 仅 V3（GB/T 26865.2） |

## 4. 改动清单

### (A) 协议库 `pmusim-core` —— 新增校验器

`crates/pmusim-core/src/protocol/frame.rs`：给 `ConfigFrame` 加方法

```rust
/// 上送周期合法性校验（规约 §6 校验机制精神）。
/// PERIOD=0 在 GB/T 26865.2 / IEEE C37.118.2 数据率字段中超出定义域
/// （未定义行为），判为非法。返回 Some(中文原因) 表示非法，None 表示合法。
/// 扩展点：未来如需校验合法上下限，在此追加分支。
pub fn illegal_period_reason(&self) -> Option<String>
```

判定逻辑：任一 `pmu_blocks[].period == 0`；若 `pmu_blocks` 为空，则看顶层 `period == 0`。命中即返回如「上送周期为 0（非法，超出规约定义域）」。

### (B) 子站 `pmusim-sub` —— 校验并按规约应答

`crates/pmusim-sub/src/network/substation.rs:211-215`：现状对任何 Config 帧无条件 ACK。改为：

- 调用 `cfg.illegal_period_reason()`。
- 非法 → `Self::send_cmd(&writer, &settings, &evt, Cmd::Nack as u16)` + emit `SubEvent::Cfg2Rejected { reason }`，**不改 `data_rate_fps`**（保持原 fps）。
- 合法 → 维持现状：emit `SubEvent::Cfg2Received` + 回 ACK。

校验在已成功 `parse` 出 `Frame::Config` 之后（`substation.rs:198` 用 dims `0,0,0,0`，对 Config 帧解析无影响），`cfg.period` / `cfg.pmu_blocks` 可靠可读。

配套：

- `crates/pmusim-sub/src/events.rs`：加 `Cfg2Rejected { reason: String }`。
- `frontend-sub/src/composables/useSubEvents.ts` + `frontend-sub/src/i18n/messages.ts`：加事件展示与 i18n（「收到非法上送周期，已 NACK 拒绝」）。

### (C) 主站 `pmusim-app` —— 修复 NACK 静默丢弃

`crates/pmusim-app/src/network/master.rs:1109-1120`：ACK/NACK 到达且 `pending_ack` 为 None 时，现状静默丢弃。改为：NACK 且无 waiter → emit `PmuEvent::Error { error: "子站 NACK：CFG-2 配置被拒" }`。

- 这样注入路径（fire-and-forget）的 NACK 必被 UI 看到，并顺带修好既有的「实时改速率静默丢 NACK」问题。
- **不动** `do_auto_handshake` 的 waiter 路径，避免双 waiter 冲突。

### (D) 主站前端 `frontend` —— 异常注入入口

`frontend/src/components/ConfigInfoPanel.vue`：

- 加「异常注入」开关。勾选后，速率输入框从 Hz 切为**直填 PERIOD 原始值**（`number`，允许 0），绕过 `hz>0` 守卫（`:145`/`:208`）与 `round(5000/hz)` 换算（`:147`/`:209`）。
- 注入动作复用既有命令路径：`invoke("send_command", { cmd: "send_cfg2_cmd", period: null })` → `invoke("send_command", { cmd: "send_cfg2", period: rawPeriod })`（与 `:211-212` 同形，仅 period 改为原始值、允许 0）。
- `frontend/src/i18n/messages.ts`：加开关标签 + 提示文案。

## 5. 端到端数据流（注入后）

```
主站UI(勾选异常注入, PERIOD=0) → send_cfg2(period=0)
  → do_send_cfg2 零校验放行(master.rs:1325) → build_config 上线
    → 子站收 CFG-2 → illegal_period_reason=Some → 回 NACK + Cfg2Rejected 事件(子站UI可见)
      → 主站 mgmt_read 收 NACK, 无 waiter → emit Error(主站UI可见)
子站数据流: data_rate_fps 不变, 继续按原 fps 发(保持原状) ✓
```

## 6. 错误处理 / 边界

- 子站校验在 `parse` 成功后进行，`cfg.period` 可靠。
- 主站乐观缓存 period=0 的 cfg2（`master.rs:1377-1389`）**无害**：数据解析维度来自 format_flags/phnmr/annmr/dgnmr（未变），period 不参与解帧；UI 速率回显对 `periodMs<=0` 返回空串（`ConfigInfoPanel.vue:110`），不崩。故**不做缓存回滚**（YAGNI）。

## 7. 测试

- **core 单测**：`illegal_period_reason` 对 `period=0`（非法）/ 正常值（合法）/ 多 PMU 块部分为 0 的判定。
- **子站 e2e**（`crates/pmusim-sub/tests/e2e.rs`）：主站下发 period=0 → 断言子站回 NACK + `data_rate_fps` 不变 + emit `Cfg2Rejected`；下发合法 period → 仍 ACK + `Cfg2Received`。
- **主站**：构造无 waiter 的 NACK 到达 → 断言 emit `Error`。

## 8. 明确不做（Out of scope）

- **合法 CFG-2 改速率时子站并不应用新 fps** 这一既存事实**不在本次修**（属另一独立特性；本次只处理非法 PERIOD 的 NACK 路径）。
- 不校验 0 以外的取值。
- 不覆盖 V2（Q/GDW 131-2006）。
