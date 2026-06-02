# 受控注入 V3 下传 CFG-2 非法上送周期 PERIOD=0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让主站能有意下发 PERIOD=0 的 CFG-2，子站按规约 §6/§8.6 识别非法上送周期回 NACK 并保持原 fps，主站可见该 NACK。

**Architecture:** 在 `pmusim-core` 加一个纯函数式校验器 `ConfigFrame::illegal_period_reason()`；子站收到下传 CFG-2 时调用它，非法→回 NACK + emit `Cfg2Rejected` 事件、不改速率；主站修复「无 waiter 时 NACK 被静默丢弃」缺陷，改为 emit `Error`；主站前端加「异常注入」开关 + 原始 PERIOD 输入。仅覆盖 V3（GB/T 26865.2）。

**Tech Stack:** Rust（`pmusim-core` / `pmusim-app` / `pmusim-sub`，tokio）、Vue 3 + TypeScript（`frontend` / `frontend-sub`，Tauri 2）。

**Spec:** `docs/superpowers/specs/2026-06-02-cfg2-illegal-period-injection-design.md`

---

## 文件结构

| 文件 | 职责 | 动作 |
| --- | --- | --- |
| `crates/pmusim-core/src/protocol/frame.rs` | 协议帧结构 + 上送周期合法性校验器 | Modify（加方法 + 测试模块） |
| `crates/pmusim-sub/src/events.rs` | 子站事件枚举 | Modify（加 `Cfg2Rejected`） |
| `crates/pmusim-sub/src/network/substation.rs` | 子站收下传 CFG-2 的应答逻辑 | Modify（Config 臂校验 + NACK） |
| `crates/pmusim-sub/tests/e2e.rs` | 端到端：主站驱动子站 | Modify（加注入 PERIOD=0 测试） |
| `crates/pmusim-app/src/network/master.rs` | 主站收 ACK/NACK 路由 | Modify（无 waiter 的 NACK → Error） |
| `frontend-sub/src/types/index.ts` | 子站 TS 事件联合类型 | Modify（加 `Cfg2Rejected`） |
| `frontend-sub/src/composables/useSubEvents.ts` | 子站事件→UI 映射 | Modify（加 case） |
| `frontend-sub/src/i18n/messages.ts` | 子站文案 | Modify（加 `event.cfg2Rejected`） |
| `frontend/src/components/ConfigInfoPanel.vue` | 主站控制面板 | Modify（异常注入入口） |
| `frontend/src/i18n/messages.ts` | 主站文案 | Modify（注入相关键） |

---

## Task 1: core 校验器 `illegal_period_reason`

**Files:**
- Modify: `crates/pmusim-core/src/protocol/frame.rs`（在 `impl ConfigFrame` 内加方法；文件尾加 `#[cfg(test)]` 模块）

- [ ] **Step 1: 写失败测试**

在 `crates/pmusim-core/src/protocol/frame.rs` **文件末尾**追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::constants::ProtocolVersion;

    /// 构造一个最小 ConfigFrame：顶层 period + 任意个 PMU 块（各自 period）。
    fn cfg(period: u16, block_periods: &[u16]) -> ConfigFrame {
        ConfigFrame {
            version: ProtocolVersion::V3,
            cfg_type: 3,
            idcode: "TEST".into(),
            soc: 0,
            fracsec: 0,
            d_frame: 0,
            meas_rate: 1_000_000,
            num_pmu: 1,
            stn: "S".into(),
            pmu_idcode: "TEST".into(),
            format_flags: 0,
            phnmr: 0,
            annmr: 0,
            dgnmr: 0,
            channel_names: vec![],
            phunit: vec![],
            anunit: vec![],
            digunit: vec![],
            fnom: 1,
            period,
            pmu_blocks: block_periods
                .iter()
                .map(|&p| PmuBlock {
                    stn: "S".into(),
                    pmu_idcode: "TEST".into(),
                    format_flags: 0,
                    phnmr: 0,
                    annmr: 0,
                    dgnmr: 0,
                    channel_names: vec![],
                    phunit: vec![],
                    anunit: vec![],
                    digunit: vec![],
                    fnom: 1,
                    period: p,
                })
                .collect(),
        }
    }

    #[test]
    fn period_zero_is_illegal() {
        // 顶层 period=0 且无 PMU 块 → 非法
        assert!(cfg(0, &[]).illegal_period_reason().is_some());
        // 多 PMU 块中任一块 period=0 → 非法
        assert!(cfg(100, &[100, 0]).illegal_period_reason().is_some());
    }

    #[test]
    fn legal_period_passes() {
        // 有块且都非 0 → 合法
        assert!(cfg(100, &[100]).illegal_period_reason().is_none());
        // 无块、顶层非 0 → 合法
        assert!(cfg(100, &[]).illegal_period_reason().is_none());
    }
}
```

- [ ] **Step 2: 跑测试确认失败（方法未定义）**

Run: `cargo test -p pmusim-core illegal_period -- --nocapture`
Expected: 编译失败，`no method named illegal_period_reason found for struct ConfigFrame`。

- [ ] **Step 3: 写最小实现**

在 `crates/pmusim-core/src/protocol/frame.rs` 的 `impl ConfigFrame { ... }` 块内（紧接现有 `period_ms` 方法之后）加入：

```rust
    /// 上送周期合法性校验（规约 §6 校验机制）。PERIOD=0 在 GB/T 26865.2 /
    /// IEEE C37.118.2 数据率字段中超出定义域（未定义行为），判为非法。
    /// 返回 `Some(中文原因)` 表示非法，`None` 表示合法。
    /// 扩展点：未来如需校验合法上下限，在此追加分支。
    pub fn illegal_period_reason(&self) -> Option<String> {
        // 多 PMU：任一块周期为 0 即非法；无块时退回顶层便利字段 period。
        let any_zero = if self.pmu_blocks.is_empty() {
            self.period == 0
        } else {
            self.pmu_blocks.iter().any(|b| b.period == 0)
        };
        if any_zero {
            Some("上送周期为 0（非法，超出规约定义域）".to_string())
        } else {
            None
        }
    }
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p pmusim-core illegal_period`
Expected: `test period_zero_is_illegal ... ok` 与 `test legal_period_passes ... ok`，全绿。

- [ ] **Step 5: 提交**

```bash
git add crates/pmusim-core/src/protocol/frame.rs
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "feat(core): ConfigFrame::illegal_period_reason 校验上送周期(PERIOD=0 非法)"
```

---

## Task 2: 子站校验下传 CFG-2 并回 NACK

**Files:**
- Modify: `crates/pmusim-sub/src/events.rs`（加 `Cfg2Rejected` 变体）
- Modify: `crates/pmusim-sub/src/network/substation.rs:211-215`（Config 臂校验）
- Test: `crates/pmusim-sub/tests/e2e.rs`（加 `wait_sub_event` 助手 + 注入测试，本任务先只断言子站侧）

- [ ] **Step 1: 加 `Cfg2Rejected` 事件变体**

在 `crates/pmusim-sub/src/events.rs` 的 `SubEvent` 枚举里，`Cfg2Received,` 之后插入：

```rust
    /// 收到主站下传的 CFG-2 但上送周期非法 → 已回 NACK 拒绝（携原因）。
    Cfg2Rejected { reason: String },
```

- [ ] **Step 2: 写失败测试（端到端注入，断言子站 Cfg2Rejected）**

在 `crates/pmusim-sub/tests/e2e.rs` 末尾追加一个 `wait_sub_event` 助手（紧跟现有 `wait_master_event` 之后）与新测试：

```rust
async fn wait_sub_event<F: FnMut(&SubEvent) -> bool>(
    rx: &mut mpsc::UnboundedReceiver<SubEvent>,
    mut pred: F,
) -> SubEvent {
    loop {
        let ev = timeout(Duration::from_secs(8), rx.recv())
            .await
            .expect("sub event timeout")
            .expect("sub channel closed");
        if pred(&ev) {
            return ev;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn v3_master_pushes_period_zero_gets_nacked() {
    let (mut sub, mut sub_rx, mgmt_port) = spawn_substation(ProtocolVersion::V3).await;

    let (m_tx, mut m_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(m_tx, 0, 30.0, ProtocolVersion::V3);
    master.start().await.unwrap();
    master
        .connect_to_substation("127.0.0.1".into(), mgmt_port, 0, ProtocolVersion::V3)
        .await
        .unwrap();

    let idcode = match wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::SessionCreated { .. })).await {
        PmuEvent::SessionCreated { idcode, .. } => idcode,
        _ => unreachable!(),
    };

    // 先完成握手到 streaming（主站缓存 cfg1，子站开始按 50fps 发）。
    master.auto_handshake(idcode.clone(), Some(100)).await.unwrap();
    let _ = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::StreamingStarted { .. })).await;

    // 受控注入：下传一帧 PERIOD=0 的 CFG-2（fire-and-forget 路径，无 ack waiter）。
    master.send_command(idcode.clone(), "send_cfg2_cmd".into(), None).await.unwrap();
    master.send_command(idcode.clone(), "send_cfg2".into(), Some(0)).await.unwrap();

    // 子站应识别非法上送周期 → 回 NACK + emit Cfg2Rejected。
    let rejected = wait_sub_event(&mut sub_rx, |e| matches!(e, SubEvent::Cfg2Rejected { .. })).await;
    if let SubEvent::Cfg2Rejected { reason } = rejected {
        assert!(reason.contains("上送周期"), "原因应说明上送周期非法: {reason}");
    }

    // 保持原状：子站继续推数据（注入后仍能收到 DataFrameSent）。
    let _ = wait_sub_event(&mut sub_rx, |e| matches!(e, SubEvent::DataFrameSent { .. })).await;

    master.stop().await;
    sub.stop().await;
}
```

- [ ] **Step 3: 跑测试确认失败（子站仍无条件 ACK，无 Cfg2Rejected）**

Run: `cargo test -p pmusim-sub v3_master_pushes_period_zero_gets_nacked`
Expected: FAIL —— `sub event timeout`（子站当前对 Config 帧无条件回 ACK + emit `Cfg2Received`，永不发 `Cfg2Rejected`）。

- [ ] **Step 4: 实现子站 Config 臂校验**

在 `crates/pmusim-sub/src/network/substation.rs` 把现有 Config 臂（`211-215` 行）：

```rust
                Frame::Config(_cfg) => {
                    // 主站下传 CFG-2 配置帧 → 回 ACK（V3 §8.6）。
                    emit_event(&evt, SubEvent::Cfg2Received);
                    Self::send_cmd(&writer, &settings, &evt, Cmd::Ack as u16).await;
                }
```

替换为：

```rust
                Frame::Config(cfg) => {
                    // 主站下传 CFG-2 配置帧 → 先按 §6 校验上送周期合法性，
                    // 再按 §8.6 回 ACK / NACK。
                    match cfg.illegal_period_reason() {
                        Some(reason) => {
                            // 非法上送周期（如 PERIOD=0）：回 NACK 拒绝，保持原 fps 不变。
                            emit_event(&evt, SubEvent::Cfg2Rejected { reason });
                            Self::send_cmd(&writer, &settings, &evt, Cmd::Nack as u16).await;
                        }
                        None => {
                            emit_event(&evt, SubEvent::Cfg2Received);
                            Self::send_cmd(&writer, &settings, &evt, Cmd::Ack as u16).await;
                        }
                    }
                }
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p pmusim-sub v3_master_pushes_period_zero_gets_nacked`
Expected: PASS（收到 `Cfg2Rejected` 且注入后仍有 `DataFrameSent`）。

- [ ] **Step 6: 跑全子站测试确保未回归（合法 CFG-2 仍 ACK）**

Run: `cargo test -p pmusim-sub`
Expected: 含 `v3_master_drives_substation_to_streaming` 等既有测试全绿（握手里下传的是合法 period=100，仍走 ACK 路径）。

- [ ] **Step 7: 提交**

```bash
git add crates/pmusim-sub/src/events.rs crates/pmusim-sub/src/network/substation.rs crates/pmusim-sub/tests/e2e.rs
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "feat(sub): 下传 CFG-2 校验上送周期,非法回 NACK + Cfg2Rejected 事件"
```

---

## Task 3: 主站修复无 waiter 时 NACK 被静默丢弃

**Files:**
- Modify: `crates/pmusim-app/src/network/master.rs:1109-1120`（ACK/NACK 路由）
- Test: `crates/pmusim-sub/tests/e2e.rs`（在 Task 2 的测试里追加主站 Error 断言）

- [ ] **Step 1: 给 Task 2 的测试追加「主站收到 NACK→Error」断言**

在 `crates/pmusim-sub/tests/e2e.rs` 的 `v3_master_pushes_period_zero_gets_nacked` 测试里，**在 `master.stop().await;` 之前**插入：

```rust
    // 主站这端（fire-and-forget 路径无 ack waiter）也必须看见该 NACK。
    let err = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::Error { .. })).await;
    if let PmuEvent::Error { error, .. } = err {
        assert!(error.contains("NACK"), "主站应 surface NACK 错误: {error}");
    }
```

- [ ] **Step 2: 跑测试确认失败（主站静默丢弃 NACK）**

Run: `cargo test -p pmusim-sub v3_master_pushes_period_zero_gets_nacked`
Expected: FAIL —— `master event timeout`（NACK 到达时 `pending_ack` 为 None，被静默丢弃，从不 emit `Error`）。

- [ ] **Step 3: 实现主站 NACK surfacing**

在 `crates/pmusim-app/src/network/master.rs` 把现有 ACK/NACK 分支（`1109-1120` 行）：

```rust
                } else if cmd.cmd == Cmd::Ack as u16 || cmd.cmd == Cmd::Nack as u16 {
                    // Deliver to whoever is awaiting (do_auto_handshake step).
                    // Without this, NACK on CFG-2 download is silently ignored
                    // and we proceed to OpenData on a half-broken handshake.
                    let tx = {
                        let mut sessions_w = sessions.write().await;
                        sessions_w.get_mut(idcode).and_then(|s| s.pending_ack.take())
                    };
                    if let Some(tx) = tx {
                        let _ = tx.send(cmd.cmd);
                    }
                }
```

替换为：

```rust
                } else if cmd.cmd == Cmd::Ack as u16 || cmd.cmd == Cmd::Nack as u16 {
                    // Deliver to whoever is awaiting (do_auto_handshake step).
                    // Without this, NACK on CFG-2 download is silently ignored
                    // and we proceed to OpenData on a half-broken handshake.
                    let tx = {
                        let mut sessions_w = sessions.write().await;
                        sessions_w.get_mut(idcode).and_then(|s| s.pending_ack.take())
                    };
                    match tx {
                        Some(tx) => {
                            let _ = tx.send(cmd.cmd);
                        }
                        // fire-and-forget 路径（实时改速率 / 异常注入）没有装
                        // ack waiter；若不在此 surface，子站的 NACK 会被静默
                        // 丢弃，UI 永远看不到「配置被拒」。
                        None if cmd.cmd == Cmd::Nack as u16 => {
                            emit_event(
                                event_tx,
                                PmuEvent::Error {
                                    idcode: idcode.to_string(),
                                    error: "子站 NACK：CFG-2 配置被拒".into(),
                                },
                            );
                        }
                        None => {} // 无 waiter 的 ACK：正常，忽略
                    }
                }
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p pmusim-sub v3_master_pushes_period_zero_gets_nacked`
Expected: PASS（子站 `Cfg2Rejected` + 主站 `Error` 含 "NACK" 均断言成立）。

- [ ] **Step 5: 跑主站测试确保未回归**

Run: `cargo test -p pmusim-app`
Expected: 既有握手 / NACK 等待相关测试全绿（`do_auto_handshake` 的 waiter 路径未改动，ACK 仍走 `tx.send`）。

- [ ] **Step 6: 提交**

```bash
git add crates/pmusim-app/src/network/master.rs crates/pmusim-sub/tests/e2e.rs
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "fix(master): 无 ack waiter 时 NACK 不再静默丢弃,emit Error 上抛 UI"
```

---

## Task 4: 子站前端展示 Cfg2Rejected

**Files:**
- Modify: `frontend-sub/src/types/index.ts`（事件联合类型）
- Modify: `frontend-sub/src/composables/useSubEvents.ts`（事件 case）
- Modify: `frontend-sub/src/i18n/messages.ts`（zh + en 文案）

- [ ] **Step 1: 扩展 TS 事件联合类型**

在 `frontend-sub/src/types/index.ts` 的 `SubEvent` 联合里，`| { type: "Cfg2Received" }` 之后插入：

```ts
  | { type: "Cfg2Rejected"; reason: string }
```

- [ ] **Step 2: 加事件处理 case**

在 `frontend-sub/src/composables/useSubEvents.ts` 的 `switch (ev.type)` 里，`case "Cfg2Received": ...` 那一行之后插入：

```ts
      case "Cfg2Rejected":
        pushEvent(t("event.cfg2Rejected", { reason: ev.reason }), "error");
        break;
```

- [ ] **Step 3: 加 i18n 文案（zh + en）**

在 `frontend-sub/src/i18n/messages.ts` 的 **zh** 块里，`'event.cfg2Received': '收到主站下传 CFG-2',` 之后插入：

```ts
    'event.cfg2Rejected': '收到非法上送周期,已回 NACK 拒绝: {reason}',
```

在 **en** 块里，`'event.cfg2Received': 'CFG-2 received from master',` 之后插入：

```ts
    'event.cfg2Rejected': 'Illegal reporting period — sent NACK: {reason}',
```

- [ ] **Step 4: 类型检查**

Run: `cd frontend-sub && npx vue-tsc -b`
Expected: 无类型错误（`ev.reason` 在 `Cfg2Rejected` 分支内已被联合类型收窄）。

- [ ] **Step 5: 提交**

```bash
git add frontend-sub/src/types/index.ts frontend-sub/src/composables/useSubEvents.ts frontend-sub/src/i18n/messages.ts
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "feat(sub-ui): 展示 Cfg2Rejected 事件(非法上送周期被 NACK)"
```

---

## Task 5: 主站前端「异常注入」入口

**Files:**
- Modify: `frontend/src/components/ConfigInfoPanel.vue`（script + template）
- Modify: `frontend/src/i18n/messages.ts`（zh + en 文案）

- [ ] **Step 1: 加注入相关响应式状态 + 函数（script）**

在 `frontend/src/components/ConfigInfoPanel.vue` 的 `<script setup>` 里，`const rateHz = ref("100");`（第 66 行）之后插入：

```ts
// 异常注入：勾选后用原始 PERIOD 值直发 CFG-2（允许 0），绕过 Hz→PERIOD 换算，
// 用于受控注入规约未定义的非法上送周期，验证子站 NACK 应对。
const injectAbnormal = ref(false);
const rawPeriod = ref("0");

async function injectPeriod() {
  const s = session.value;
  if (!s) return; // 按钮 disabled 已兜底
  if (s.state !== "streaming" && s.state !== "cfg2_sent") return;
  const p = parseInt(rawPeriod.value);
  if (!Number.isFinite(p) || p < 0) {
    pushToast(t("config.injectBadValue"), "error");
    return;
  }
  try {
    await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2_cmd", period: null });
    await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2", period: p });
    pushToast(t("config.injectSent", { period: String(p) }), "info");
  } catch (e) {
    pushToast(t("config.injectFailed", { error: toastError(e) }), "error");
  }
}
```

- [ ] **Step 2: 加 UI 控件（template）**

在 `frontend/src/components/ConfigInfoPanel.vue` 的 template 里，速率那一行 `<div class="row"> ... rate ... </div>`（`232-243` 行，以 `{{ t("config.rate") }}` 起头的那个 `.row`）**之后**插入：

```html
      <div class="row">
        <label>{{ t("config.abnormalInject") }}</label>
        <input type="checkbox" v-model="injectAbnormal" />
      </div>
      <div class="row" v-if="injectAbnormal">
        <label>{{ t("config.rawPeriod") }}</label>
        <div class="ctl-with-suffix">
          <input v-model="rawPeriod" inputmode="numeric" style="width: 80px" />
          <button
            class="btn"
            @click="injectPeriod"
            :disabled="!session || (session.state !== 'streaming' && session.state !== 'cfg2_sent')"
          >{{ t("config.inject") }}</button>
        </div>
      </div>
```

- [ ] **Step 3: 加 i18n 文案（zh + en）**

在 `frontend/src/i18n/messages.ts` 的 **zh** 块里，`'config.rateFailed': '修改速率失败: {error}',`（第 39 行附近）之后插入：

```ts
    'config.abnormalInject': '异常注入',
    'config.rawPeriod': '原始 PERIOD',
    'config.inject': '注入',
    'config.injectSent': '已注入 PERIOD={period}',
    'config.injectFailed': '注入失败: {error}',
    'config.injectBadValue': 'PERIOD 取值非法',
```

在 **en** 块里，`'config.rateFailed': 'Failed to change rate: {error}',`（第 132 行附近）之后插入：

```ts
    'config.abnormalInject': 'Abnormal injection',
    'config.rawPeriod': 'Raw PERIOD',
    'config.inject': 'Inject',
    'config.injectSent': 'Injected PERIOD={period}',
    'config.injectFailed': 'Injection failed: {error}',
    'config.injectBadValue': 'Invalid PERIOD value',
```

- [ ] **Step 4: 类型检查**

Run: `cd frontend && npx vue-tsc -b`
Expected: 无类型错误。

- [ ] **Step 5: 手动验证（端到端 UI）**

启动主站与子站应用（V3）：勾选「异常注入」→ `原始 PERIOD` 填 `0` → 点「注入」。预期：
- 主站事件日志出现 Error「子站 NACK：CFG-2 配置被拒」。
- 子站事件日志出现「收到非法上送周期,已回 NACK 拒绝」。
- 子站数据流不中断（上送速率读数维持原值）。

- [ ] **Step 6: 提交**

```bash
git add frontend/src/components/ConfigInfoPanel.vue frontend/src/i18n/messages.ts
git -c user.name="Karl-Dai Karl" -c user.email="kelsoprotein@gmail.com" \
  commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" \
  -m "feat(ui): 主站异常注入入口 — 直发原始 PERIOD(含 0)的 CFG-2"
```

---

## 收尾验证

- [ ] **全量测试**

Run: `cargo test --workspace`
Expected: 全绿（core 单测 + sub/app e2e）。

- [ ] **两前端类型检查**

Run: `cd frontend && npx vue-tsc -b && cd ../frontend-sub && npx vue-tsc -b`
Expected: 均无类型错误。

---

## Self-Review 记录

- **Spec coverage：** (A) core 校验器 → Task 1；(B) 子站校验+NACK+事件 → Task 2 + Task 4；(C) 主站 NACK surfacing → Task 3；(D) 前端异常注入入口 → Task 5。Spec §7 测试三项分别落在 Task 1 单测、Task 2/3 的 e2e（注入 period=0 断言 Cfg2Rejected + 不停流 + 主站 Error；合法 period 仍 ACK 由 Task 2 Step 6 既有测试覆盖）。
- **主站单测取舍：** spec §7「构造无 waiter 的 NACK → emit Error」改由 Task 3 的 e2e 断言覆盖（`process_mgmt_frame` 为私有 async 且需完整 sessions map，独立单测脆弱），行为等价且更贴近真实路径。
- **类型一致性：** 后端 `Cfg2Rejected { reason: String }`（Rust）↔ 前端 `{ type: "Cfg2Rejected"; reason: string }`（TS）字段名一致；`illegal_period_reason` 在 Task 1 定义、Task 2 调用，签名一致。
- **Out of scope（不实现）：** 合法 CFG-2 改速率时子站应用新 fps；0 以外取值校验；V2 覆盖。
