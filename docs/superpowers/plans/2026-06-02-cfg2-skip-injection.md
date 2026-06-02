# 受控注入 V4 — 跳过 CFG-2 仅凭 CFG-1 开流 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给主站加一条受控注入路径——召唤 CFG-1 后跳过下传/召唤 CFG-2，直接 OpenData，演示「子站无 CFG-2 也推流、主站凭 CFG-1 维度即可解码」。

**Architecture:** 后端在 `pmusim-app` 加一个 `do_skip_cfg2_open`（等于现有 `do_auto_handshake` 砍掉 CFG-2 三步），经 `MasterCmd::SkipCfg2Open` + Tauri 命令 `skip_cfg2_open` 暴露；新事件 `PmuEvent::Cfg2Skipped` 作注入标记。前端在「异常注入」区加「跳过 CFG-2 连接」按钮，复用正常连接管线但末步换成 `skip_cfg2_open`。子站零改动（收到 OpenData 即推流）。

**Tech Stack:** Rust（tokio / Tauri 2），Vue 3 + TS（Vite），集成测试 `crates/pmusim-sub/tests/e2e.rs`（真实 MasterStation+SubStation），无头验证用 Playwright。

**Spec:** `docs/superpowers/specs/2026-06-02-cfg2-skip-injection-design.md`
**Branch:** `feat/cfg2-skip-injection`（spec 已提交于 `e3ed901`）

---

## 文件结构

| 文件 | 职责 | 动作 |
| --- | --- | --- |
| `crates/pmusim-app/src/events.rs` | 主站事件枚举 | 加 `Cfg2Skipped` |
| `crates/pmusim-app/src/network/master.rs` | 命令枚举 + 公开方法 + command_loop + handler | 加 `SkipCfg2Open` / `skip_cfg2_open()` / 分派 / `do_skip_cfg2_open` |
| `crates/pmusim-app/src/commands.rs` | Tauri 命令层 | 加 `skip_cfg2_open` 命令 |
| `crates/pmusim-app/src/main.rs` | invoke_handler 注册 | 加 `commands::skip_cfg2_open` |
| `crates/pmusim-sub/tests/e2e.rs` | 集成测试 | 加 `v3_master_skips_cfg2_streams_via_cfg1` |
| `frontend/src/types/index.ts` | 前端事件类型 | 加 `Cfg2Skipped` 到 `PmuEvent` 联合 |
| `frontend/src/composables/usePmuEvents.ts` | 事件分派 | 加 `Cfg2Skipped` case |
| `frontend/src/i18n/messages.ts` | 中英文案 | 加 `config.skipCfg2*` + `event.cfg2Skipped` |
| `frontend/src/components/ConfigInfoPanel.vue` | 主站配置面板 | 加 `skipCfg2Connect()` + 按钮 |

---

## Task 1: 后端 — 跳过 CFG-2 开流命令 + 注入事件（TDD）

**Files:**
- Test: `crates/pmusim-sub/tests/e2e.rs`（在文件末尾追加，现有最后一个测试是 `v3_master_pushes_period_zero_gets_nacked`，:178-237）
- Modify: `crates/pmusim-app/src/events.rs:5-17`（`PmuEvent` 枚举）
- Modify: `crates/pmusim-app/src/network/master.rs`（`MasterCmd` :27-66 / 公开方法 :227 区域 / `command_loop` :422-494 / 新 handler 紧邻 `do_auto_handshake` :1501）
- Modify: `crates/pmusim-app/src/commands.rs:89-98`（仿 `auto_handshake` 命令）
- Modify: `crates/pmusim-app/src/main.rs:13-26`（invoke_handler）

- [ ] **Step 1: 写失败的集成测试**

在 `crates/pmusim-sub/tests/e2e.rs` 文件**末尾**追加（复用文件顶部已有的 `spawn_substation` / `wait_master_event` / `IDCODE` 等 helper）：

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn v3_master_skips_cfg2_streams_via_cfg1() {
    let (mut sub, mut sub_rx, mgmt_port) = spawn_substation(ProtocolVersion::V3).await;

    let (m_tx, mut m_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(m_tx, 0, 30.0, ProtocolVersion::V3);
    master.start().await.unwrap();
    master
        .connect_to_substation("127.0.0.1".into(), mgmt_port, 0, ProtocolVersion::V3)
        .await
        .unwrap();

    let placeholder = match wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::SessionCreated { .. })).await {
        PmuEvent::SessionCreated { idcode, .. } => idcode,
        _ => unreachable!(),
    };

    // 跳过 CFG-2 的握手:召唤 CFG-1 → 直接 OpenData(不发任何 CFG-2)。
    master.skip_cfg2_open(placeholder).await.unwrap();

    // 收集到 StreamingStarted 为止的所有主站事件,断言:见到 CFG-1 + Cfg2Skipped,
    // 且全程没有任何 CFG-2 事件(确实跳过)。match &ev 借用,避免在 panic 分支里 use-after-move。
    let mut saw_cfg1 = false;
    let mut saw_skipped = false;
    loop {
        let ev = timeout(Duration::from_secs(8), m_rx.recv())
            .await
            .expect("master event timeout")
            .expect("master channel closed");
        match &ev {
            PmuEvent::Cfg1Received { cfg, .. } => {
                assert_eq!(cfg.annmr, 2);
                assert_eq!(cfg.dgnmr, 1);
                saw_cfg1 = true;
            }
            PmuEvent::Cfg2Skipped { .. } => saw_skipped = true,
            PmuEvent::Cfg2Sent { .. } | PmuEvent::Cfg2Received { .. } => {
                panic!("跳过 CFG-2 路径不应出现 CFG-2 事件: {ev:?}");
            }
            PmuEvent::StreamingStarted { .. } => break,
            _ => {}
        }
    }
    assert!(saw_cfg1, "应收到 CFG-1(维度来源)");
    assert!(saw_skipped, "应收到 Cfg2Skipped 注入标记");

    // 凭 CFG-1 维度成功解出 DataFrame。
    let data = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::DataFrame { .. })).await;
    if let PmuEvent::DataFrame { idcode, data } = data {
        assert_eq!(idcode, IDCODE);
        assert_eq!(data.phasors.len(), 1);
        assert_eq!(data.analog.len(), 2);
        assert_eq!(data.digital, vec![0x000A]);
    }

    master.stop().await;
    sub.stop().await;
}
```

- [ ] **Step 2: 运行测试确认失败(编译失败)**

Run: `cargo test -p pmusim-sub --test e2e v3_master_skips_cfg2_streams_via_cfg1 2>&1 | tail -20`
Expected: 编译失败 —— `no method named skip_cfg2_open` + `no variant Cfg2Skipped`。

- [ ] **Step 3: 加 `PmuEvent::Cfg2Skipped`**

`crates/pmusim-app/src/events.rs`，在 `PmuEvent` 枚举里 `Cfg2Sent` 之后加一行（:9 之后）：

```rust
    Cfg2Sent { idcode: String },
    Cfg2Skipped { idcode: String },
```

- [ ] **Step 4: 加命令枚举项 + 公开方法 + command_loop 分派 + handler**

4a. `crates/pmusim-app/src/network/master.rs` 的 `enum MasterCmd`，在 `AutoHandshake { .. }`（:59-62）之后加：

```rust
    SkipCfg2Open {
        idcode: String,
    },
```

4b. 在公开方法区，紧接 `auto_handshake`（:227-235）之后加：

```rust
    pub async fn skip_cfg2_open(&self, idcode: String) -> Result<(), String> {
        self.cmd_tx
            .send(MasterCmd::SkipCfg2Open { idcode })
            .await
            .map_err(|e| e.to_string())
    }
```

4c. `command_loop` 的 `match cmd` 里，紧接 `MasterCmd::AutoHandshake { .. }` 分支（:487-489）之后加：

```rust
                MasterCmd::SkipCfg2Open { idcode } => {
                    Self::do_skip_cfg2_open(&sessions, &cmd_tx, &event_tx, &idcode).await;
                }
```

4d. 紧接 `do_auto_handshake` 方法（:1501-1603）之后、`}` 关闭 `impl` 之前，加新 handler：

```rust
    /// 受控注入 V4：跳过 CFG-2 仅凭 CFG-1 开流。等于 do_auto_handshake 砍掉
    /// 「下传 CFG-2 命令 / 下传 CFG-2 配置帧 / 召唤 CFG-2」三步:召唤 CFG-1 →
    /// (跳过) → OpenData。走内部 do_send_cmd(OpenData),天然绕过 command_loop
    /// 里手动 OpenData 的 Cfg2Sent|Streaming 门控。
    async fn do_skip_cfg2_open(
        sessions: &Arc<RwLock<HashMap<String, SubStationSession>>>,
        cmd_tx: &mpsc::Sender<MasterCmd>,
        event_tx: &EventSender,
        idcode: &str,
    ) {
        // 捕获 peer,跨 re-key 跟踪会话(SendCfg1 后子站真实 IDCODE 到达会重命名会话)。
        let peer = {
            let r = sessions.read().await;
            r.get(idcode).map(|s| (s.peer_host.clone(), s.peer_mgmt_port))
        };
        let Some((peer_host, peer_port)) = peer else {
            emit_event(event_tx, PmuEvent::Error {
                idcode: idcode.to_string(),
                error: "Session not found".into(),
            });
            return;
        };

        // Step 1: 召唤 CFG-1,等它到达(维度缓存进 session.cfg1)。
        Self::do_send_cmd(sessions, event_tx, idcode, Cmd::SendCfg1 as u16).await;
        let current = match wait_for_cfg1(
            sessions,
            &peer_host,
            peer_port,
            std::time::Duration::from_secs(2),
        )
        .await
        {
            Some(id) => id,
            None => {
                emit_event(event_tx, PmuEvent::Error {
                    idcode: idcode.to_string(),
                    error: "CFG-1 not received after request".into(),
                });
                return;
            }
        };

        // 跳过:下传 CFG-2 命令 / 下传 CFG-2 配置帧 / 召唤 CFG-2。打一个注入标记事件。
        emit_event(event_tx, PmuEvent::Cfg2Skipped { idcode: current.clone() });

        // Step 5: 开数据管道(V3) + OpenData。管道打开失败则不发 OpenData、不谎报 streaming。
        if !Self::do_open_data_v3(sessions, cmd_tx, event_tx, &current).await {
            return;
        }
        Self::do_send_cmd(sessions, event_tx, &current, Cmd::OpenData as u16).await;
        {
            let mut sessions_w = sessions.write().await;
            if let Some(session) = sessions_w.get_mut(&current) {
                session.state = SessionState::Streaming;
                session.cfg_change_seen = false;
            }
        }
        emit_event(event_tx, PmuEvent::StreamingStarted { idcode: current });
    }
```

- [ ] **Step 5: 加 Tauri 命令 + 注册**

5a. `crates/pmusim-app/src/commands.rs`，紧接 `auto_handshake` 命令（:89-98）之后加：

```rust
#[tauri::command]
pub async fn skip_cfg2_open(
    state: State<'_, AppState>,
    idcode: String,
) -> Result<(), String> {
    let guard = state.master.lock().await;
    let master = guard.as_ref().ok_or("Server not running")?;
    master.skip_cfg2_open(idcode).await
}
```

5b. `crates/pmusim-app/src/main.rs`，在 `commands::auto_handshake,`（:19）之后加一行：

```rust
            commands::auto_handshake,
            commands::skip_cfg2_open,
```

- [ ] **Step 6: 运行新测试确认通过**

Run: `cargo test -p pmusim-sub --test e2e v3_master_skips_cfg2_streams_via_cfg1 2>&1 | tail -15`
Expected: `test result: ok. 1 passed; 0 failed`。

- [ ] **Step 7: 跑全量后端测试确保未回归**

Run: `cargo test --workspace --lib --tests 2>&1 | tail -20`
Expected: 全绿（含原有 `v3_master_drives_substation_to_streaming` / `v3_master_pushes_period_zero_gets_nacked` 等);0 failed。

- [ ] **Step 8: 提交**

```bash
git add crates/pmusim-app/src/events.rs crates/pmusim-app/src/network/master.rs crates/pmusim-app/src/commands.rs crates/pmusim-app/src/main.rs crates/pmusim-sub/tests/e2e.rs
git commit -m "feat(master): 跳过 CFG-2 仅凭 CFG-1 开流(受控注入) + Cfg2Skipped 事件 + e2e"
```

---

## Task 2: 前端 — 注入入口按钮 + 事件展示 + i18n

**Files:**
- Modify: `frontend/src/types/index.ts:44-55`（`PmuEvent` 联合）
- Modify: `frontend/src/composables/usePmuEvents.ts:54-57`（`Cfg2Sent` case 之后）
- Modify: `frontend/src/i18n/messages.ts`（zh :45 / en :144 后加 config.*；zh :98 / en :197 后加 event.*）
- Modify: `frontend/src/components/ConfigInfoPanel.vue`（script :139-181 区域加函数；template :271-281 的 `v-if="injectAbnormal"` 块加按钮）

- [ ] **Step 1: 前端类型加 `Cfg2Skipped`**

`frontend/src/types/index.ts`，在 `Cfg2Sent` 那一行（:48）之后加：

```ts
  | { type: "Cfg2Sent"; idcode: string }
  | { type: "Cfg2Skipped"; idcode: string }
```

- [ ] **Step 2: 事件分派加 case**

`frontend/src/composables/usePmuEvents.ts`，在 `case "Cfg2Sent":` 块（:54-57）之后加：

```ts
      case "Cfg2Skipped":
        pushEvent(t("event.cfg2Skipped"), "info");
        break;
```

（`useEventLog` 仅支持 `"info" | "error"`，注入标记非错误，用 `"info"`。）

- [ ] **Step 3: 加 i18n key（中 + 英）**

3a. `frontend/src/i18n/messages.ts` 的 `zh` 块,在 `'config.injectBadValue': 'PERIOD 取值非法',`（:45）之后加：

```ts
    'config.skipCfg2': '跳过 CFG-2',
    'config.skipCfg2Connect': '跳过 CFG-2 连接',
```

3b. 同文件 `zh` 块,在 `'event.dataEstablished': '数据管道建立',`（:98）之后加：

```ts
    'event.cfg2Skipped': '异常注入: 跳过 CFG-2,仅凭 CFG-1 开流',
```

3c. `en` 块,在 `'config.injectBadValue': 'Invalid PERIOD value',`（:144）之后加：

```ts
    'config.skipCfg2': 'Skip CFG-2',
    'config.skipCfg2Connect': 'Connect, skip CFG-2',
```

3d. `en` 块,在 `'event.dataEstablished': 'Data pipe established',`（:197）之后加：

```ts
    'event.cfg2Skipped': 'Injection: skipped CFG-2, streaming via CFG-1 only',
```

- [ ] **Step 4: 加 `skipCfg2Connect()` 函数 + 模板按钮**

4a. `frontend/src/components/ConfigInfoPanel.vue`，在 `startEverything` 函数闭合 `}`（:181）之后加一个并列函数（复用同一批已在作用域内的 ref：`busy` / `running` / `protocol` / `connDataPort` / `connMgmtPort` / `connIp` / `heartbeatSecs` / `session` / `listenerReady` / `invoke` / `pushToast` / `t` / `toastError`）：

```ts
// 受控注入:跳过 CFG-2,仅凭 CFG-1 开流。与正常「连接」二选一 —— 同样
// start_server + connect,但末步用 skip_cfg2_open 取代 auto_handshake,
// 且不传 period(CFG-2 被跳过,子站按自身 fps 推流)。
async function skipCfg2Connect() {
  if (busy.value) return;
  busy.value = true;
  try {
    await listenerReady;
    if (!running.value) {
      const dataPort = protocol.value === "V3" ? 0 : parseInt(connDataPort.value);
      await invoke("start_server", { dataPort, protocol: protocol.value });
      running.value = true;
    }
    const hb = parseFloat(heartbeatSecs.value);
    if (Number.isFinite(hb) && hb > 0) {
      await invoke("set_heartbeat_interval", { seconds: hb });
    }
    if (!session.value) {
      const mgmt = parseInt(connMgmtPort.value);
      const data = protocol.value === "V3" ? parseInt(connDataPort.value) : undefined;
      await invoke("connect_substation", { host: connIp.value.trim(), port: mgmt, dataPort: data });
    }
    const target = session.value?.idcode ?? `${connIp.value.trim()}:${connMgmtPort.value}`;
    await invoke("skip_cfg2_open", { idcode: target });
  } catch (e) {
    pushToast(t("config.startFailed", { error: toastError(e) }), "error");
  } finally {
    busy.value = false;
  }
}
```

4b. 在 template 的 `v-if="injectAbnormal"` 块（:271-281，即原始 PERIOD 注入那一行）**之后**、`config.heartbeat` 行（:282）**之前**，加一行按钮：

```html
      <div class="row" v-if="injectAbnormal">
        <label>{{ t("config.skipCfg2") }}</label>
        <button class="btn" @click="skipCfg2Connect" :disabled="busy">{{ t("config.skipCfg2Connect") }}</button>
      </div>
```

- [ ] **Step 5: 前端类型检查**

Run: `cd frontend && npx --no-install vue-tsc --noEmit; echo "exit: $status"`
Expected: `exit: 0`（无类型错误）。

- [ ] **Step 6: 提交**

```bash
git add frontend/src/types/index.ts frontend/src/composables/usePmuEvents.ts frontend/src/i18n/messages.ts frontend/src/components/ConfigInfoPanel.vue
git commit -m "feat(ui): 异常注入入口 — 跳过 CFG-2 连接(仅凭 CFG-1 开流) + Cfg2Skipped 事件展示"
```

---

## Task 3: 无头浏览器验证（强制，前端有改动）

**Files:** 无（验证 only，产物用后即删）

- [ ] **Step 1: 后台起 dev server**

Run（后台）: `cd frontend && npm run dev`（端口 5173）。等输出 `ready`。

- [ ] **Step 2: 导航 + 断言注入按钮渲染可见**

用 Playwright（`mcp__playwright__browser_*`）`navigate` 到 `http://localhost:5173/`，然后：
- `browser_click` 勾选 `.config-panel input[type=checkbox]`（异常注入开关）。
- `browser_evaluate` 断言:存在文本含「跳过 CFG-2 连接 / Connect, skip CFG-2」的 `.config-panel button`；其 `getBoundingClientRect()` 在视口内（top≥0、bottom≤innerHeight、宽高>0）;沿祖先链无 `overflow:hidden` 把它裁出框外。
- `browser_take_screenshot` 存一张证。

Expected: 按钮存在、可见、未被裁剪。

- [ ] **Step 3: 清理**

关浏览器（`browser_close`）、停 dev server（TaskStop 后台任务）、`rm -f frontend/../<截图>`、`git status` 确认无残留（尤其 iCloud `* 2.ext` 冲突副本与 `.playwright-mcp/` 是否已被 .gitignore 忽略）。

任一断言不过 → 回 Task 2 修，不要继续。

---

## Task 4: 收尾 — 全量验证

- [ ] **Step 1: 全量后端测试**

Run: `cargo test --workspace --lib --tests 2>&1 | tail -15`
Expected: 全绿，0 failed。

- [ ] **Step 2: 两前端类型检查（确认 sub 端未被波及）**

Run: `cd frontend && npx --no-install vue-tsc --noEmit; echo "fe: $status"`
Expected: `fe: 0`。（本特性不动 `frontend-sub`，无需检 sub。）

- [ ] **Step 3: 确认分支状态**

Run: `git -C . log --oneline -3 && git status -s`
Expected: 看到 Task1/Task2 两个 feat 提交;工作树干净。

> 后续(超出本计划范围):合并到 main / 发版,由 finishing-a-development-branch 或 release skill 处理。

---

## Self-Review（写计划者自查结果）

- **Spec 覆盖**:§4(A) 后端命令+handler→Task1 Step4;§4(B) Cfg2Skipped 事件→Task1 Step3 + Task2 Step1-2;§4(C) 前端入口+事件+types+i18n→Task2;§4(D) 子站零改动→无任务（正确）;§7 e2e→Task1 Step1;§8 无头验证→Task3。V2 变体(§7「可选」)未列任务——按 spec 标注为可选,刻意不做,执行者如需可仿 `v2_master_drives_substation_to_streaming` 追加。
- **占位符扫描**:无 TBD/TODO;每个代码步均给出完整代码。
- **类型一致**:`Cfg2Skipped { idcode }`（Rust 枚举 / TS 联合 / e2e matches! / usePmuEvents case）四处一致;`skip_cfg2_open`（公开方法 / Tauri 命令 / invoke 调用 / 测试调用）命名一致;`pushEvent(..., "info")` 与 `useEventLog` 的 `info|error` 一致（已据此修正 spec 原 "warn"）。
