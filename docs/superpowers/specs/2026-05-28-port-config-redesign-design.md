# 端口配置 UI 重设计

## 背景

当前 UI(`ToolbarPanel.vue` + `StationListPanel.vue`)有两个端口输入框,语义跟 PMU 协议的实际方向不对应,易误导用户填错:

- **工具栏 `数据端口`** — 代码语义是 `MasterStation.data_port`,即 master **本地 TCP listen 端口**(V2 子站连过来上送数据用)。V3 模式下 `data_listener_loop` 仍然 bind 0.0.0.0:data_port,但 V3 实际是 master 主动 outbound,所以这个 listen **完全没用**,只占端口。
- **连接子站 `端口`** — 代码语义是 `peer_mgmt_port`,即 **子站的命令端口**,master 主动连接它。

实测用户在 V3 模式下把 `数据端口` 填成 8000、`端口` 填成 8001(刚好跟 PMU 协议约定相反),导致 master 试图把 mgmt 命令塞进子站的数据端口、数据连接发到 8002,完全不通。

## PMU 协议端口方向规则(权威)

> ⚠ 规约文档 `PMU协议V3版报文解析指导手册.docx` 第 6 节描述"等待子站建立数据流管道的申请",字面方向跟下表 V3 行**相反**;以下表是 PMU 协议实际方向(经实测金风 IEMP 子站 1789 帧 V3 数据流验证):

| | 命令端口 (默认 mgmt+0) | 数据端口 (默认 mgmt+1) |
|--|---------------------|---------------------|
| **V2 (2006)** | master = client → 子站 = server | **子站 = client → master = server** |
| **V3 (2011)** | master = client → 子站 = server | **master = client → 子站 = server** |

约定的默认端口:
- V2: 命令 7000, 数据 7001
- V3: 命令 8000, 数据 8001

## 设计目标

1. UI 字段名跟实际 TCP 角色一致,不再误导
2. V3 模式下不再 bind 无用的本地端口
3. 子站数据端口可显式指定(默认跟随 `命令端口 + 1`)
4. V2/V3 切换协议时,默认值联动更新
5. 后端 backwards-compatible:e2e 测试、headless_smoke 不破坏

## UI 字段重设计

### 工具栏 (`ToolbarPanel.vue`)

| 字段 | V2 显示 | V3 显示 | 说明 |
|------|---------|---------|------|
| 启动 / 停止 | ✓ | ✓ | 不变 |
| 协议 (V2/V3) | ✓ | ✓ | 不变 |
| **本地数据侦听端口** | ✓ 默认 7001 | ✗ 隐藏 | (原 `数据端口` 改名) — V3 不需要 master 这边 listen |

切换协议时(`dataPort` ref + `dataPortDirty` ref):
- 用户从未编辑过(dataPortDirty=false): V2→V3 隐藏并清零;V3→V2 重置为 `7001`
- 用户编辑过(dataPortDirty=true): V2→V3 隐藏并清零;V3→V2 恢复用户上次编辑的值(从单独的 `lastV2DataPort` ref 取)
- 数据端口仅在 V2 模式下意义存在,所以 V3 期间用户编辑事件不可能发生

### 连接子站面板 (`StationListPanel.vue`)

| 字段 | V2 显示 | V3 显示 | 默认 |
|------|---------|---------|------|
| IP | ✓ | ✓ | `127.0.0.1` |
| **命令端口** | ✓ | ✓ | V2 = 7000, V3 = 8000 (协议联动) |
| **数据端口** | ✗ 隐藏 | ✓ | 自动跟随 `命令端口 + 1`,placeholder `= 命令端口+1`,可手动覆盖 |
| 连接 | ✓ | ✓ | |
| 断开所选 | ✓ | ✓ | |

V2 隐藏数据端口字段的理由:V2 是子站主动连 master,master 在工具栏的"本地侦听端口"上 listen,跟子站数据端口无关。

数据端口字段的"自动跟随":只要用户没手动改过,数据端口的 value 永远等于"命令端口+1"。一旦用户编辑过,锁定为用户值(用 dirty flag 跟踪)。切换协议时:dirty=false → 重置跟随,dirty=true → 保留用户值。

## 后端改动

### `SubStationSession` (`session.rs`)

新增字段:

```rust
pub struct SubStationSession {
    ...
    pub peer_mgmt_port: u16,
    pub peer_data_port: u16,   // 新增 — V3 master 主动连这个端口取数据
    ...
}
```

`new(idcode, version, peer_ip)` 把 `peer_data_port` 初始化为 0,真正的端口由 do_connect 在插入 placeholder 时填入。

### `MasterStation::start()` (`master.rs`)

V3 模式跳过 data listener bind:

```rust
pub async fn start(&mut self) -> Result<(), String> {
    if self.protocol == ProtocolVersion::V2 {
        let listener = TcpListener::bind(("0.0.0.0", self.data_port)).await
            .map_err(|e| format!("Failed to bind data port {}: {e}", self.data_port))?;
        self.data_port = listener.local_addr().map(|a| a.port()).unwrap_or(self.data_port);
        let sessions = self.sessions.clone();
        let handle = self.event_tx.clone();
        self.tasks.push(tokio::spawn(async move {
            Self::data_listener_loop(listener, sessions, handle).await;
        }));
        info!("MasterStation started (V2), data listener on port {}", self.data_port);
    } else {
        self.data_port = 0;
        info!("MasterStation started (V3), no local data listener (outbound only)");
    }
    // ... command_loop / heartbeat_loop unchanged
}
```

### `connect_to_substation` 签名扩展 (`master.rs` 公共 API)

```rust
pub async fn connect_to_substation(
    &self,
    host: String,
    mgmt_port: u16,
    data_port: u16,   // 新增显式参数 — 0 表示用默认 mgmt+1
    version: ProtocolVersion,
) -> Result<(), String>
```

`MasterCmd::Connect` 同步扩展。`do_connect` 设置 placeholder 时:

```rust
let effective_data_port = if data_port == 0 {
    mgmt_port.saturating_add(1)
} else {
    data_port
};
placeholder.peer_mgmt_port = mgmt_port;
placeholder.peer_data_port = effective_data_port;
```

### `do_open_data_v3` 简化 (`master.rs`)

去掉 `default_ports` + mgmt+1 计算逻辑,直接用 `session.peer_data_port`:

```rust
let (peer_host, data_port, version, already_open) = {
    let sessions_r = sessions.read().await;
    let Some(s) = sessions_r.get(idcode) else { return false };
    (s.peer_host.clone(), s.peer_data_port, s.version, s.data_connected())
};
```

### `commands::start_server` (`commands.rs`)

工具栏的 `data_port` 只在 V2 时传非零值。Tauri 命令签名保持现状(`data_port: u16`),V3 直接传 0:

```rust
#[tauri::command]
pub async fn start_server(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    data_port: u16,        // V2: 用户填的侦听端口;V3: 0 (UI 强制)
    protocol: String,
) -> Result<(), String>
```

### `commands::connect_substation` (`commands.rs`)

```rust
#[tauri::command]
pub async fn connect_substation(
    state: State<'_, AppState>,
    host: String,
    port: u16,                       // 命令端口
    data_port: Option<u16>,          // 可选数据端口,V3 默认跟随
) -> Result<(), String> {
    let guard = state.master.lock().await;
    let master = guard.as_ref().ok_or("Server not running")?;
    // None → 0 哨兵,master 内部转为 mgmt_port + 1
    master.connect_to_substation(host, port, data_port.unwrap_or(0), master.protocol).await
}
```

`None` 通过 `unwrap_or(0)` 转成 master 公共 API 的 sentinel `0`,在 `do_connect` 内部转回 `mgmt_port + 1`。

## 数据流方向矩阵(实现层)

| | 命令通道 (master client) | 数据通道 |
|--|----------------------|---------|
| V2 (2006) | `TcpStream::connect((host, 7000))` | **master TcpListener::bind(0.0.0.0:7001)**,子站连过来 |
| V3 (2011) | `TcpStream::connect((host, 8000))` | **master TcpStream::connect((host, 8001))** |

## 测试 / 兼容性

- e2e 测试 `crates/pmusim-app/tests/e2e.rs`:mock 已在 `mgmt_port + 1` bind data listener,跟 master 的默认 `peer_data_port = mgmt + 1` 对齐,所以 `connect_to_substation` 调用只需要补一个 `data_port: 0` 哨兵(走默认),不需要显式传 mock 端口
- headless_smoke example:CLI 位置参数 `host mgmt_port [master_v2_listen_port]`,第 3 个参数语义改为"V2 master 侦听端口",V3 跑时忽略(master 不 bind);子站数据端口固定走默认 mgmt+1 = 8001。需要在 main.rs 顶部加注释说明
- 旧版前端 invoke 调用 (`connect_substation { host, port }`,无 data_port) 仍然兼容 — Tauri 对 `Option<u16>` 缺省接收为 None,后端走默认 mgmt+1
- `MasterStation::start()` 把 V3 模式 `data_port` 强制设为 0 — 任何旧测试如果断言 `master.data_port == 某非零值` 需要更新预期(无,已确认)

## 影响范围

| 文件 | 改动量 |
|------|--------|
| `crates/pmusim-app/src/network/session.rs` | +1 字段 |
| `crates/pmusim-app/src/network/master.rs` | start() 分支 (~15 LOC),do_connect placeholder 扩展 (~3 LOC),do_open_data_v3 简化 (~5 LOC),MasterCmd::Connect 字段 +1 |
| `crates/pmusim-app/src/commands.rs` | connect_substation 加可选参数 (~3 LOC) |
| `crates/pmusim-app/tests/e2e.rs` | 测试构造调用更新 (~5 LOC) |
| `crates/pmusim-app/examples/headless_smoke.rs` | 调用更新 + CLI 参数语义说明 (~5 LOC) |
| `frontend/src/components/ToolbarPanel.vue` | 字段重命名 + v-if 协议条件 (~10 LOC) |
| `frontend/src/components/StationListPanel.vue` | 加 dataPort ref + dirty 跟踪 + v-if V3 显示 (~25 LOC) |
| `frontend/src/types/index.ts` | 可选 SessionInfo.peerDataPort 字段 |

合计 ~70 LOC,单 PR 范围。

## 未做的事(YAGNI)

- 不实现 0x0003 发送头文件 / 0x6000 系统复位 / 0xA000 联网触发 — 一次调频业务不用
- 不暴露"V3 数据方向选项"切换 — PMU 协议规则锁死,不让用户选错
- 不做"双向"fallback(V3 同时 listen 8001 + 主动 outbound) — 实测子站只支持 outbound,inbound 永远不会触发
