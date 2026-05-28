# 端口配置 UI 重设计 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 PmuSim GUI 的端口字段语义跟 PMU 协议的实际 V2/V3 数据通道方向对齐;V3 模式下 master 不再占无用的本地侦听端口,子站数据端口可在 UI 显式指定(默认跟随命令端口 +1)。

**Architecture:** SubStationSession 加一个 `peer_data_port` 字段,master 内部 `MasterCmd::Connect` 把 mgmt/data 两个端口都带过来,`do_open_data_v3` 直接读 session 的字段不再凭空算 `mgmt+1`。`MasterStation::start()` 按 `self.protocol` 分支:V2 照旧 bind 本地数据侦听,V3 完全不 bind。前端两个面板 conditional 显示对应字段,并在协议切换时联动默认值。

**Tech Stack:** Rust 1.x + tokio + Tauri 2 (后端);Vue 3 + TypeScript + Vite (前端);live verification 走 `cargo run --example headless_smoke -p pmusim-app -- 10.15.48.12 8000`。

**Spec:** `docs/superpowers/specs/2026-05-28-port-config-redesign-design.md`

---

## File Structure

会改 / 新增的文件:

| 文件 | 改动 |
|------|------|
| `crates/pmusim-app/src/network/session.rs` | +`peer_data_port: u16` 字段 |
| `crates/pmusim-app/src/network/master.rs` | `MasterCmd::Connect` 加 `data_port`;`connect_to_substation` 公共 API 加 `data_port`;`do_connect` 设置 placeholder.peer_data_port;`do_open_data_v3` 改用 `s.peer_data_port`;`start()` V2/V3 分支 |
| `crates/pmusim-app/src/commands.rs` | `connect_substation` Tauri 命令加 `data_port: Option<u16>` |
| `crates/pmusim-app/tests/e2e.rs` | 新增 `v3_handshake_with_explicit_data_port` 测试;现有两条 V3 测试更新调用 |
| `crates/pmusim-app/examples/headless_smoke.rs` | 更新 `connect_to_substation` 调用 |
| `frontend/src/components/ToolbarPanel.vue` | label 改名 + V3 v-if 隐藏 |
| `frontend/src/components/StationListPanel.vue` | 加 dataPort ref + dataPortDirty + V3 v-if + watch 联动 |

---

## Task 1: SubStationSession 加 `peer_data_port` 字段

**Files:**
- Modify: `crates/pmusim-app/src/network/session.rs`

- [ ] **Step 1: Add the field to the struct**

在 `src/network/session.rs` `pub struct SubStationSession` 里,`peer_mgmt_port` 下面加一行:

```rust
    pub peer_mgmt_port: u16,
    pub peer_data_port: u16,
    pub state: SessionState,
```

- [ ] **Step 2: Initialize in `new()`**

在 `impl SubStationSession` 的 `new()` 里,`peer_mgmt_port: 0,` 下面加:

```rust
            peer_mgmt_port: 0,
            peer_data_port: 0,
            state: SessionState::Connected,
```

- [ ] **Step 3: 编译验证**

Run: `cargo check -p pmusim-app --message-format short`
Expected: `Finished \`dev\` profile` 无错误(会有 `peer_data_port` 字段未用的 warning,后续 task 用上)

- [ ] **Step 4: Commit**

```bash
cd "/Users/daichangyu/Library/Mobile Documents/com~apple~CloudDocs/code/PmuSim"
git add crates/pmusim-app/src/network/session.rs
git commit -m "refactor(session): add peer_data_port field for V3 master-outbound data"
```

---

## Task 2: MasterCmd::Connect 带 data_port + 公共 API 扩展

**Files:**
- Modify: `crates/pmusim-app/src/network/master.rs`

- [ ] **Step 1: Extend MasterCmd::Connect**

`master.rs` 第 27 行附近 `enum MasterCmd`,把 Connect 改成:

```rust
    Connect {
        host: String,
        port: u16,
        data_port: u16,        // 0 = use mgmt+1 default
        version: ProtocolVersion,
    },
```

- [ ] **Step 2: Extend public `connect_to_substation`**

`master.rs` `impl MasterStation` 里找 `pub async fn connect_to_substation`,把签名和 body 改成:

```rust
    pub async fn connect_to_substation(
        &self,
        host: String,
        mgmt_port: u16,
        data_port: u16,
        version: ProtocolVersion,
    ) -> Result<(), String> {
        self.cmd_tx
            .send(MasterCmd::Connect {
                host,
                port: mgmt_port,
                data_port,
                version,
            })
            .await
            .map_err(|e| e.to_string())
    }
```

- [ ] **Step 3: Update command_loop dispatch**

找到 `MasterCmd::Connect { host, port, version }` 这条 arm(在 `command_loop` 函数里),改成:

```rust
                MasterCmd::Connect { host, port, data_port, version } => {
                    Self::do_connect(host, port, data_port, version, sessions.clone(), event_tx.clone()).await;
                }
```

- [ ] **Step 4: Update `do_connect` signature + populate placeholder.peer_data_port**

找到 `async fn do_connect(`,在签名里加 `data_port: u16`(在 `version` 之前):

```rust
    async fn do_connect(
        host: String,
        port: u16,
        data_port: u16,
        version: ProtocolVersion,
        sessions: Arc<RwLock<HashMap<String, SubStationSession>>>,
        event_tx: EventSender,
    ) {
```

然后在创建 placeholder 的地方(`let mut placeholder = SubStationSession::new(...)` 后面),加 data_port 计算和赋值:

```rust
            let mut placeholder = SubStationSession::new(tmp_id.clone(), version, host.clone());
            placeholder.peer_host = host.clone();
            placeholder.peer_mgmt_port = port;
            placeholder.peer_data_port = if data_port == 0 {
                port.saturating_add(1)
            } else {
                data_port
            };
            // No reader/writer yet — the TCP connect hasn't returned.
            sessions_w.insert(tmp_id.clone(), placeholder);
```

- [ ] **Step 5: 编译验证(会有 callsite 不匹配错误,Task 3/8 修)**

Run: `cargo check -p pmusim-app --message-format short 2>&1 | head -20`
Expected: errors about callsites missing `data_port`(会在 commands.rs / e2e / headless_smoke 报)

- [ ] **Step 6: Commit (intermediate, 通过暂时编译失败)**

先修 commands.rs 调用站点让 cargo check 通过:在 `commands.rs::connect_substation` 找到:

```rust
master.connect_to_substation(host, port, version).await
```

改成(暂时用默认 0):

```rust
master.connect_to_substation(host, port, 0, version).await
```

Run: `cargo check -p pmusim-app --message-format short`
Expected: only e2e / headless_smoke errors remain(测试和 example 在后续 task 修),库本身编译通过

```bash
git add crates/pmusim-app/src/network/master.rs crates/pmusim-app/src/commands.rs
git commit -m "refactor(master): MasterCmd::Connect carries data_port; connect_to_substation public API extended"
```

---

## Task 3: do_open_data_v3 改用 session.peer_data_port + e2e 测试

**Files:**
- Modify: `crates/pmusim-app/src/network/master.rs` (do_open_data_v3)
- Modify: `crates/pmusim-app/tests/e2e.rs` (callsite + new test)

- [ ] **Step 1: 写新失败测试 `v3_handshake_with_explicit_data_port`**

在 `crates/pmusim-app/tests/e2e.rs` 文件末尾(在 `v3_auto_handshake_from_tmp_id_reaches_streaming` 之后)加:

```rust
#[tokio::test]
async fn v3_handshake_with_explicit_data_port() {
    // Bind mgmt + a NON-adjacent data port (mgmt + 10) so the master must
    // use the explicit data_port argument, not the mgmt+1 default.
    let mgmt_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mgmt_port = mgmt_listener.local_addr().unwrap().port();
    let custom_data_port = mgmt_port.checked_add(10).expect("port + 10");
    let data_listener = TcpListener::bind(("127.0.0.1", custom_data_port)).await.unwrap();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(event_tx, 0, 30.0, ProtocolVersion::V3);
    master.start().await.unwrap();

    // Reuse the mock substation body — accepts mgmt, replies to handshake,
    // serves data on whichever port the master happens to dial.
    let obs: ObsHandle = Arc::new(Mutex::new(MockObservations::default()));
    let obs_for_task = obs.clone();
    let mock_task = tokio::spawn(async move {
        let (stream, _) = mgmt_listener.accept().await.unwrap();
        let (mut reader, mut writer) = stream.into_split();
        let mut data_writer: Option<tokio::net::tcp::OwnedWriteHalf> = None;
        let (data_tx, mut data_rx) =
            mpsc::unbounded_channel::<tokio::net::tcp::OwnedWriteHalf>();
        tokio::spawn(async move {
            if let Ok((s, _)) = data_listener.accept().await {
                let (_, dw) = s.into_split();
                let _ = data_tx.send(dw);
            }
        });
        loop {
            let frame_data = match read_one_frame(&mut reader).await {
                Ok(d) => d,
                Err(_) => break,
            };
            match parse(&frame_data, 0, 0, 0) {
                Ok(Frame::Command(cmd)) => match cmd.cmd {
                    c if c == Cmd::SendCfg1 as u16 => {
                        writer.write_all(&build_config(&make_cfg(FrameType::Cfg1 as u8)).unwrap()).await.unwrap();
                    }
                    c if c == Cmd::SendCfg2Cmd as u16 => {
                        writer.write_all(&ack_command()).await.unwrap();
                    }
                    c if c == Cmd::SendCfg2 as u16 => {
                        writer.write_all(&build_config(&make_cfg(FrameType::Cfg2 as u8)).unwrap()).await.unwrap();
                    }
                    c if c == Cmd::OpenData as u16 => {
                        if data_writer.is_none() {
                            data_writer = data_rx.recv().await;
                        }
                        let bytes = build_data(&make_data_frame(0x67A99D11), 0, 2, 1).unwrap();
                        if let Some(dw) = data_writer.as_mut() {
                            dw.write_all(&bytes).await.unwrap();
                        }
                    }
                    _ => {}
                },
                Ok(Frame::Config(cfg)) => {
                    obs_for_task.lock().await.received_cfg_types.push(cfg.cfg_type);
                    writer.write_all(&ack_command()).await.unwrap();
                }
                _ => {}
            }
        }
    });

    // Connect with EXPLICIT non-default data_port
    master
        .connect_to_substation(
            "127.0.0.1".into(),
            mgmt_port,
            custom_data_port,
            ProtocolVersion::V3,
        )
        .await
        .unwrap();

    let tmp_id = match wait_event(&mut event_rx, |e| {
        matches!(e, PmuEvent::SessionCreated { .. })
    })
    .await
    {
        PmuEvent::SessionCreated { idcode, .. } => idcode,
        _ => unreachable!(),
    };
    master.auto_handshake(tmp_id, None).await.unwrap();

    // Walk through to a DataFrame to prove the master dialed the custom data port.
    let _ = wait_event(&mut event_rx, |e| matches!(e, PmuEvent::Cfg2Sent { .. })).await;
    let _ = wait_event(&mut event_rx, |e| matches!(e, PmuEvent::Cfg2Received { .. })).await;
    let _ = wait_event(&mut event_rx, |e| matches!(e, PmuEvent::StreamingStarted { .. })).await;
    let data_event = wait_event(&mut event_rx, |e| matches!(e, PmuEvent::DataFrame { .. })).await;
    if let PmuEvent::DataFrame { idcode, .. } = data_event {
        assert_eq!(idcode, IDCODE);
    }

    let _ = obs;
    master.stop().await;
    mock_task.abort();
}
```

- [ ] **Step 2: Run new test — should compile-fail because existing connect_to_substation callsites in this file are 3-arg**

Run: `cargo test --test e2e v3_handshake_with_explicit_data_port -- --nocapture 2>&1 | head -30`
Expected: compile error in e2e.rs at the existing V3 tests — they still pass 3 args. Fix them next.

- [ ] **Step 3: Fix existing V3 e2e tests to pass `0` sentinel**

在 `tests/e2e.rs` 找两处 `.connect_to_substation("127.0.0.1".into(), mock_port, ProtocolVersion::V3)`,改成:

```rust
        .connect_to_substation("127.0.0.1".into(), mock_port, 0, ProtocolVersion::V3)
```

(在 `v3_full_handshake_streams_data` 和 `v3_auto_handshake_from_tmp_id_reaches_streaming` 各一处)

- [ ] **Step 4: Run the new test — should FAIL because do_open_data_v3 still uses mgmt+1 not session.peer_data_port**

Run: `cargo test --test e2e v3_handshake_with_explicit_data_port -- --nocapture 2>&1 | tail -20`
Expected: test times out / errors — the master tries to connect to `mgmt_port + 1` (which nothing listens on; the mock listens on `mgmt_port + 10`), gets "Data connect ... timed out" and never sees DataFrame.

- [ ] **Step 5: Switch do_open_data_v3 to use session.peer_data_port**

找到 `async fn do_open_data_v3` 顶部的 dims-snapshot block:

```rust
        let (peer_host, data_port, version, already_open) = {
            let sessions_r = sessions.read().await;
            let Some(s) = sessions_r.get(idcode) else {
                return false; // session vanished — don't proceed with OpenData
            };
            // Substation data port = mgmt port + 1 by GB/T 26865.2 convention
            // (8000/8001 for V3, 7000/7001 for V2). Falls back to the default
            // table only if mgmt port is 0 (e.g. OS-assigned).
            let data_port = if s.peer_mgmt_port == 0 {
                default_ports(s.version).1
            } else {
                s.peer_mgmt_port.saturating_add(1)
            };
            (
                s.peer_host.clone(),
                data_port,
                s.version,
                s.data_connected(),
            )
        };
```

改成:

```rust
        let (peer_host, data_port, version, already_open) = {
            let sessions_r = sessions.read().await;
            let Some(s) = sessions_r.get(idcode) else {
                return false; // session vanished — don't proceed with OpenData
            };
            // peer_data_port was populated in do_connect; explicit override
            // wins, otherwise it's mgmt_port + 1 by GB/T 26865.2 convention.
            (
                s.peer_host.clone(),
                s.peer_data_port,
                s.version,
                s.data_connected(),
            )
        };
```

由于不再用 `default_ports`,删除 `master.rs` 顶部 `use pmusim_core::protocol::constants::{default_ports, ...}` 里的 `default_ports`:

```rust
use pmusim_core::protocol::constants::{
    Cmd, FrameType, ProtocolVersion, IDCODE_LEN, SYNC_BYTE,
};
```

- [ ] **Step 6: Run new test — should PASS**

Run: `cargo test --test e2e v3_handshake_with_explicit_data_port -- --nocapture 2>&1 | tail -5`
Expected: `test result: ok. 1 passed`

- [ ] **Step 7: Run all e2e tests — should all PASS**

Run: `cargo test --test e2e 2>&1 | tail -10`
Expected: `test result: ok. 3 passed; 0 failed`

- [ ] **Step 8: Commit**

```bash
git add crates/pmusim-app/src/network/master.rs crates/pmusim-app/tests/e2e.rs
git commit -m "feat(master): do_open_data_v3 uses session.peer_data_port (explicit override)"
```

---

## Task 4: MasterStation::start() V3 模式跳过本地 bind

**Files:**
- Modify: `crates/pmusim-app/src/network/master.rs` (start)
- Modify: `crates/pmusim-app/tests/e2e.rs` (new test)

- [ ] **Step 1: 写新失败测试 `v3_start_does_not_bind_local_data_port`**

在 `tests/e2e.rs` 末尾加:

```rust
#[tokio::test]
async fn v3_start_does_not_bind_local_data_port() {
    let (event_tx, _event_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(event_tx, 0, 30.0, ProtocolVersion::V3);
    master.start().await.unwrap();
    assert_eq!(master.data_port, 0, "V3 master must not bind a local data listener");
    master.stop().await;
}

#[tokio::test]
async fn v2_start_still_binds_local_data_port() {
    let (event_tx, _event_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(event_tx, 0, 30.0, ProtocolVersion::V2);
    master.start().await.unwrap();
    assert!(master.data_port != 0, "V2 master must bind a real local data listener");
    master.stop().await;
}
```

- [ ] **Step 2: Run — V3 test should FAIL (current start binds for both)**

Run: `cargo test --test e2e v3_start_does_not_bind -- --nocapture 2>&1 | tail -10`
Expected: assertion fails — data_port is non-zero because start binds.

- [ ] **Step 3: Refactor `start()` to split on protocol**

找到 `pub async fn start(&mut self) -> Result<(), String>`,整段替换为:

```rust
    /// Start the data TCP listener (V2 only), command loop, and heartbeat loop.
    /// V3 (master = data client) skips the listener; data_port is reset to 0.
    pub async fn start(&mut self) -> Result<(), String> {
        match self.protocol {
            ProtocolVersion::V2 => {
                let listener = TcpListener::bind(("0.0.0.0", self.data_port))
                    .await
                    .map_err(|e| format!("Failed to bind data port {}: {e}", self.data_port))?;
                self.data_port = listener
                    .local_addr()
                    .map(|a| a.port())
                    .unwrap_or(self.data_port);

                info!("MasterStation started (V2), data listener on port {}", self.data_port);

                let sessions = self.sessions.clone();
                let handle = self.event_tx.clone();
                self.tasks.push(tokio::spawn(async move {
                    Self::data_listener_loop(listener, sessions, handle).await;
                }));
            }
            ProtocolVersion::V3 => {
                self.data_port = 0;
                info!("MasterStation started (V3), no local data listener (master-outbound only)");
            }
        }

        // Spawn command loop.
        let cmd_rx = self
            .cmd_rx
            .take()
            .ok_or_else(|| "start() called twice".to_string())?;
        let sessions = self.sessions.clone();
        let handle = self.event_tx.clone();
        let hb_interval = self.heartbeat_interval;
        self.tasks.push(tokio::spawn(async move {
            Self::command_loop(cmd_rx, sessions.clone(), handle.clone()).await;
        }));

        // Spawn heartbeat loop.
        let sessions = self.sessions.clone();
        let handle = self.event_tx.clone();
        self.tasks.push(tokio::spawn(async move {
            Self::heartbeat_loop(sessions, handle, hb_interval).await;
        }));

        Ok(())
    }
```

- [ ] **Step 4: Run both new start tests — both PASS**

Run: `cargo test --test e2e start_ -- --nocapture 2>&1 | tail -10`
Expected: `test result: ok. 2 passed`

- [ ] **Step 5: Run full e2e — all 5 PASS**

Run: `cargo test --test e2e 2>&1 | tail -5`
Expected: `test result: ok. 5 passed; 0 failed`

- [ ] **Step 6: Commit**

```bash
git add crates/pmusim-app/src/network/master.rs crates/pmusim-app/tests/e2e.rs
git commit -m "feat(master): V3 start() skips local data listener bind (master-outbound only)"
```

---

## Task 5: Tauri commands.rs — connect_substation 接受 Option<u16> data_port

**Files:**
- Modify: `crates/pmusim-app/src/commands.rs`

- [ ] **Step 1: 更新 connect_substation 签名 + body**

`commands.rs` 找到 `pub async fn connect_substation`,整段替换为:

```rust
#[tauri::command]
pub async fn connect_substation(
    state: State<'_, AppState>,
    host: String,
    port: u16,
    data_port: Option<u16>,
) -> Result<(), String> {
    let guard = state.master.lock().await;
    let master = guard.as_ref().ok_or("Server not running")?;
    let version = master.protocol;
    // None → 0 sentinel; do_connect interprets it as mgmt_port + 1.
    master
        .connect_to_substation(host, port, data_port.unwrap_or(0), version)
        .await
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo check -p pmusim-app --message-format short`
Expected: `Finished` 无错误

- [ ] **Step 3: Commit**

```bash
git add crates/pmusim-app/src/commands.rs
git commit -m "feat(commands): connect_substation accepts optional data_port for V3 override"
```

---

## Task 6: 新增 useProtocol composable(ToolbarPanel/StationListPanel 共享)

**Files:**
- Create: `frontend/src/composables/useProtocol.ts`

- [ ] **Step 1: Create the composable**

新文件 `frontend/src/composables/useProtocol.ts`:

```ts
import { ref } from "vue";

// Single shared protocol selection across the toolbar + station list panel.
// Lifting it out of either panel ensures the data-port field in
// StationListPanel responds to the toolbar's protocol toggle.
export type Protocol = "V2" | "V3";

const protocol = ref<Protocol>("V3");

export function useProtocol() {
  return { protocol };
}
```

- [ ] **Step 2: typecheck**

Run: `cd frontend && rm -f tsconfig.tsbuildinfo tsconfig.node.tsbuildinfo && npx vue-tsc -b; echo $?`
Expected: `0`

- [ ] **Step 3: Commit**

```bash
git add frontend/src/composables/useProtocol.ts
git commit -m "feat(ui): add useProtocol shared composable for cross-panel protocol state"
```

---

## Task 7: ToolbarPanel.vue — V3 隐藏数据端口字段 + 文案改名

**Files:**
- Modify: `frontend/src/components/ToolbarPanel.vue`

- [ ] **Step 1: 整段替换 ToolbarPanel.vue**

替换为:

```vue
<script setup lang="ts">
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useToast, toastError } from "../composables/useToast";
import { useProtocol } from "../composables/useProtocol";

const { push: pushToast } = useToast();
const { protocol } = useProtocol();

// V2 only: master 本地侦听端口(子站会主动连这里上送数据)
const localListenPort = ref("8001");
const running = ref(false);

watch(protocol, (p) => {
  // V2 默认 7001, V3 默认 8001(虽然 V3 不会用,但留个合理值)
  localListenPort.value = p === "V2" ? "7001" : "8001";
});

async function start() {
  try {
    // V3 模式后端会忽略 data_port,但 Tauri 命令签名仍要求 u16,传 0 不通过
    // 这里照旧传值即可 — 后端 start() 按 protocol 分支处理
    const dataPort = protocol.value === "V3" ? 0 : parseInt(localListenPort.value);
    await invoke("start_server", { dataPort, protocol: protocol.value });
    running.value = true;
    pushToast(
      protocol.value === "V3"
        ? `已启动 (V3, 数据走 master-outbound)`
        : `已启动 (V2, 本地侦听端口 ${localListenPort.value})`,
      "success",
    );
  } catch (e) {
    pushToast(`启动失败: ${toastError(e)}`, "error");
  }
}

async function stop() {
  try {
    await invoke("stop_server");
    running.value = false;
    pushToast("已停止", "info");
  } catch (e) {
    pushToast(`停止失败: ${toastError(e)}`, "error");
  }
}
</script>

<template>
  <div class="toolbar">
    <button @click="start" :disabled="running">&#9654; 启动</button>
    <button @click="stop" :disabled="!running">&#9632; 停止</button>
    <span class="sep"></span>
    <label>协议:</label>
    <select v-model="protocol" :disabled="running">
      <option>V2</option>
      <option>V3</option>
    </select>
    <template v-if="protocol === 'V2'">
      <span class="sep"></span>
      <label>本地侦听端口:</label>
      <input v-model="localListenPort" type="text" style="width: 70px" :disabled="running" />
    </template>
  </div>
</template>

<style scoped>
.toolbar { display: flex; align-items: center; gap: 6px; padding: 6px 8px; background: #e8e8e8; border-bottom: 1px solid #ccc; }
.toolbar button { padding: 4px 12px; border: 1px solid #bbb; border-radius: 3px; background: #ddd; cursor: pointer; }
.toolbar button:disabled { opacity: 0.5; cursor: default; }
.toolbar input, .toolbar select { padding: 2px 4px; border: 1px solid #bbb; border-radius: 3px; }
.toolbar select:disabled, .toolbar input:disabled { opacity: 0.6; background: #f5f5f5; }
.sep { width: 1px; height: 20px; background: #bbb; margin: 0 4px; }
label { color: #555; }
</style>
```

- [ ] **Step 2: 前端 typecheck**

Run: `cd frontend && rm -f tsconfig.tsbuildinfo tsconfig.node.tsbuildinfo && npx vue-tsc -b; echo $?`
Expected: `0`

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/ToolbarPanel.vue
git commit -m "feat(ui): toolbar — hide data port in V3, rename to '本地侦听端口' for V2"
```

---

## Task 8: StationListPanel.vue — 加 mgmt/data 端口 + dirty tracking

**Files:**
- Modify: `frontend/src/components/StationListPanel.vue`

- [ ] **Step 1: 整段替换 StationListPanel.vue 的 `<script setup>` 部分**

找到 `<script setup lang="ts">` 到 `</script>` 之间所有内容,整段替换为:

```vue
<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useSessions } from "../composables/useSessions";
import { useToast, toastError } from "../composables/useToast";
import { useProtocol } from "../composables/useProtocol";

const { sessions, selectedIdcode, removeSession } = useSessions();
const { push: pushToast } = useToast();
const { protocol } = useProtocol();
const connIp = ref("127.0.0.1");
const connMgmtPort = ref("8000");
const connDataPort = ref("8001");
const connDataPortDirty = ref(false);
const period = ref("");
const busy = ref(false);

const stationList = computed(() => Array.from(sessions.values()));

// 协议切换时联动 mgmt + data 默认值(用户未编辑则跟随)
watch(protocol, (p) => {
  connMgmtPort.value = p === "V2" ? "7000" : "8000";
  if (!connDataPortDirty.value) {
    connDataPort.value = p === "V2" ? "7001" : "8001";
  }
});

// 命令端口手动改时,数据端口自动跟随 mgmt+1(除非用户编辑过 data)
watch(connMgmtPort, (newMgmt) => {
  if (!connDataPortDirty.value) {
    const m = parseInt(newMgmt);
    if (Number.isFinite(m)) connDataPort.value = String(m + 1);
  }
});

function onDataPortInput(e: Event) {
  // 用户手动编辑 → 锁定为用户值
  connDataPortDirty.value = true;
  connDataPort.value = (e.target as HTMLInputElement).value;
}

function selectStation(idcode: string) {
  selectedIdcode.value = idcode;
}

async function connect() {
  if (busy.value) return;
  busy.value = true;
  const host = connIp.value;
  const mgmt = parseInt(connMgmtPort.value);
  const data = protocol.value === "V3" ? parseInt(connDataPort.value) : undefined;
  const target = `${host}:${mgmt}`;
  const p = period.value ? parseInt(period.value) : null;
  try {
    await invoke("connect_substation", { host, port: mgmt, dataPort: data });
    await invoke("auto_handshake", { idcode: target, period: p });
  } catch (e) {
    pushToast(`连接失败: ${toastError(e)}`, "error");
  } finally {
    busy.value = false;
  }
}

async function disconnect() {
  if (!selectedIdcode.value) return;
  const id = selectedIdcode.value;
  try {
    await invoke("disconnect_substation", { idcode: id });
    removeSession(id);
    pushToast(`已断开 ${id}`, "info");
  } catch (e) {
    pushToast(`断开失败: ${toastError(e)}`, "error");
  }
}

async function sendCmd(cmd: string) {
  if (!selectedIdcode.value) {
    pushToast("请先选择一个子站", "error");
    return;
  }
  const p = period.value ? parseInt(period.value) : null;
  try {
    if (cmd === "auto_handshake") {
      await invoke("auto_handshake", { idcode: selectedIdcode.value, period: p });
    } else {
      await invoke("send_command", { idcode: selectedIdcode.value, cmd, period: p });
    }
  } catch (e) {
    pushToast(`命令 ${cmd} 失败: ${toastError(e)}`, "error");
  }
}
</script>
```

- [ ] **Step 2: 整段替换 StationListPanel.vue 的 `<template>` 中的 `连接子站` fieldset**

找到 `<fieldset><legend>连接子站</legend>` 这个 fieldset(包到 `</fieldset>`),整段替换为:

```vue
    <fieldset>
      <legend>连接子站</legend>
      <div class="form-row"><label>IP:</label><input v-model="connIp" style="width:110px" /></div>
      <div class="form-row"><label>命令端口:</label><input v-model="connMgmtPort" style="width:60px" /></div>
      <div class="form-row" v-if="protocol === 'V3'">
        <label>数据端口:</label>
        <input :value="connDataPort" @input="onDataPortInput" style="width:60px" :placeholder="String(parseInt(connMgmtPort) + 1)" />
      </div>
      <button class="full-btn" :disabled="busy" @click="connect">{{ busy ? '连接中…' : '连接' }}</button>
      <button class="full-btn" :disabled="!selectedIdcode" @click="disconnect">断开所选</button>
    </fieldset>
```

- [ ] **Step 3: 前端 typecheck**

Run: `cd frontend && rm -f tsconfig.tsbuildinfo tsconfig.node.tsbuildinfo && npx vue-tsc -b; echo $?`
Expected: `0`

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/StationListPanel.vue
git commit -m "feat(ui): station panel — split mgmt/data ports for V3 with auto-follow + dirty tracking"
```

---

## Task 9: headless_smoke example — 更新 connect_to_substation 调用 + 加 data_port CLI 参数

**Files:**
- Modify: `crates/pmusim-app/examples/headless_smoke.rs`

- [ ] **Step 1: 加可选 data_port CLI 位置参数**

在 `main()` 顶部 args 解析处:

```rust
    let mut args = std::env::args().skip(1);
    let host = args.next().unwrap_or_else(|| "10.15.48.12".to_string());
    let port: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(8000);
    let data_port: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(18001);
```

把 `data_port` 这一行的语义注释加上、再加一个新位置参数 `sub_data_port`:

```rust
    let mut args = std::env::args().skip(1);
    let host = args.next().unwrap_or_else(|| "10.15.48.12".to_string());
    let port: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(8000);
    // 第 3 个参数: V2 master 本地侦听端口;V3 模式下被 start() 忽略
    let data_port: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(18001);
    // 第 4 个参数(可选): 子站数据端口;省略时走 mgmt+1 默认
    let sub_data_port: u16 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
```

- [ ] **Step 2: 更新 connect_to_substation 调用**

找到:

```rust
    match master
        .connect_to_substation(host.clone(), port, protocol)
        .await
```

(共 2 处 — 第一次 connect + 第二次 dup-guard 测试),都改成:

```rust
    match master
        .connect_to_substation(host.clone(), port, sub_data_port, protocol)
        .await
```

- [ ] **Step 3: 编译 + 跑 smoke 验证**

Run: `cargo build --example headless_smoke -p pmusim-app --message-format short 2>&1 | tail -5`
Expected: `Finished`

Run(假设子站还在 10.15.48.12 运行):

```bash
cargo run --quiet --example headless_smoke -p pmusim-app -- 10.15.48.12 8000 18001
```

Expected: 看到 `EVENT DataFrame { ... }` 出现 1000+ 次,跟之前一样

- [ ] **Step 4: Commit**

```bash
git add crates/pmusim-app/examples/headless_smoke.rs
git commit -m "test(smoke): add optional sub_data_port CLI arg; update connect_to_substation call"
```

---

## Task 10: 全量验证 + 收尾

- [ ] **Step 1: 跑 workspace 全测试**

Run: `cargo test --workspace --quiet 2>&1 | tail -15`
Expected: 41+ tests passed(原 39 + 新增 3 个 e2e),0 failed

- [ ] **Step 2: 前端最终 typecheck**

Run: `cd frontend && rm -f tsconfig.tsbuildinfo tsconfig.node.tsbuildinfo && npx vue-tsc -b; echo $?`
Expected: `0`

- [ ] **Step 3: 跑 GUI 手动验证(可选,有 live 子站时)**

```bash
cd crates/pmusim-app && cargo tauri dev
```

预期:
- 启动 / 协议下拉切到 V3 → 工具栏没有"本地侦听端口"字段
- 切到 V2 → 工具栏出现"本地侦听端口"字段,默认 7001
- 连接子站面板 V3 模式下显示三个字段:IP / 命令端口 / 数据端口
- 修改命令端口 8000 → 8888 后,数据端口自动联动到 8889
- 手动改数据端口 → 后续改命令端口数据端口不再联动(dirty)
- 切到 V2 → 数据端口字段消失

- [ ] **Step 4: 推荐用户检查推送**

```bash
git log --oneline @{upstream}..HEAD
```

提示用户:本地领先 origin/main N commits,准备好可以推。
