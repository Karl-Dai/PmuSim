# 设计：受控注入 V4 — 跳过 CFG-2 仅凭 CFG-1 开流

- 日期：2026-06-02
- 规约范围：V3（GB/T 26865.2-2011）；V2 变体为可选测试
- 对照规约：《PMU 协议 V3 版报文解析指导手册》§6 / §8.4 / §8.6 / §8.11
- 关联：与 [受控注入 V3 下传非法 CFG-2 周期](2026-06-02-cfg2-illegal-period-injection-design.md) 是一对（那次是"下发坏 CFG-2 → NACK"，本次是"根本不发 CFG-2"）

## 1. 背景与问题

主站正常启动（`frontend/src/components/ConfigInfoPanel.vue` 的 `startEverything`，:139）会在连接后**自动跑完整握手** `auto_handshake`（:175）：CFG-1 → 下传 CFG-2 命令 → 下传 CFG-2 配置帧 → 召唤 CFG-2 → OpenData，一路到 streaming。

但规约层面 CFG-2 并非开流的硬前置：

- **子站侧无任何 CFG 门控**：`substation.rs:handle_command` 收到 `OpenData(0x0002)` 直接 `start_stream`（:257-260），从不检查是否发过/收过 CFG-2。
- **主站侧能凭 CFG-1 解码**：`data_read_loop_outbound` 取解帧维度是 `cfg2.as_ref().or(cfg1.as_ref())`（`master.rs:830`），CFG-1 的 `format_flags/phnmr/annmr/dgnmr` 就足以切出相量/模拟量/数字量（本项目 `build_config_frame` 对 CFG-1/CFG-2 生成同样内容，`substation.rs:398`）。
- 主站手动 `OpenData` 命令分支确有门控 `state ∈ {Cfg2Sent, Streaming}`（`master.rs:445-457`），但 `auto_handshake` 是直接调内部 `do_send_cmd(OpenData)`（:1587）**绕过该门控**的。

即：「跳过 CFG-2 直接开流」是规约与实现都允许的合法路径，只是当前 UI 没有入口去复现它。本设计补一个**受控注入入口**来演示这一点。

## 2. 规约依据

- **§6**：主站宜具备 CFG 校验/握手机制——但 CFG-2 是"主站向子站写配置"的可选动作，子站汇报自身配置用 CFG-1 即可表达通道维度。
- **§8.4 / §8.6**：下传 CFG-2 命令/帧需 ACK/NACK——本场景**不发**这两者，故无需应答。
- **§8.11**：数据帧 STAT 承载运行状态；子站在未协商 CFG-2 时按自身配置推流，属规约允许。

结论：跳过 CFG-2、仅以 CFG-1 完成"召唤 CFG-1 → OpenData"是合法握手子集。**验证点**：子站零 CFG-2 仍推流（印证子站无门控）；主站凭 CFG-1 维度成功解码（印证 CFG-2 对解码非必需）。

## 3. 设计决策（已确认）

| 维度 | 取值 |
| --- | --- |
| 实现路线 | 方案 A：专用一键注入命令，复用 `auto_handshake` 的 helper，砍掉 CFG-2 三步 |
| 进入方式 | 独立「跳过 CFG-2 连接」按钮，作为正常「连接」之外的另一启动入口（二选一） |
| 跳过程度 | 做 CFG-1，跳过下传/召唤 CFG-2，直接 OpenData（主站凭 CFG-1 解码） |
| 门控/状态 | 不动 OpenData 门控、不加 SessionState（复用 streaming） |
| 规约版本 | V3 为主；V2 变体仅作可选 e2e |

## 4. 改动清单

### (A) 主站 `pmusim-app` —— 新增"跳过 CFG-2 开流"命令

`crates/pmusim-app/src/network/master.rs`：

- `enum MasterCmd` 加 `SkipCfg2Open { idcode: String }`（:28 区域）。
- 新增公开方法 `MasterStation::skip_cfg2_open(idcode) -> Result<(), String>`，向 `cmd_tx` 投递该命令（仿 `auto_handshake`，:227）。
- `command_loop` 加分派分支 → `do_skip_cfg2_open`（:422 区域）。
- 新 handler `do_skip_cfg2_open(sessions, cmd_tx, event_tx, idcode)`，等于 `do_auto_handshake`（:1501）**砍掉第 2/3/4 步**：
  1. 捕获 peer（`peer_host:peer_mgmt_port`）。
  2. `do_send_cmd(SendCfg1)` + `wait_for_cfg1(2s)`；超时未到 → emit `Error` 中止（复用 :1529 逻辑）。
  3. **跳过**下传 CFG-2 命令、下传 CFG-2 配置帧、召唤 CFG-2。
  4. emit `PmuEvent::Cfg2Skipped { idcode }`（注入标记，见 (B)）。
  5. `do_open_data_v3`；失败 → 已 emit Error，直接 return（不谎报 streaming）。
  6. `do_send_cmd(OpenData)`（内部路径，**天然绕过** :445 门控）→ `state = Streaming`、清 `cfg_change_seen` → emit `StreamingStarted`。
- **无 `period` 参数**（CFG-2 被跳过，子站按自身 `data_rate_fps` 推流）。
- 注册 Tauri 命令 `skip_cfg2_open`（`crates/pmusim-app/src/commands.rs` + `invoke_handler` 列表，仿现有 `auto_handshake` 命令）。

### (B) 主站事件 —— 注入标记

`crates/pmusim-app/src/events.rs`：`enum PmuEvent` 加 `Cfg2Skipped { idcode: String }`（对称子站的 `Cfg2Rejected`）。前端据此在事件区记一行醒目日志。

### (C) 主站前端 `frontend` —— 独立注入入口

`frontend/src/components/ConfigInfoPanel.vue`：

- 在既有「异常注入」勾选区内加按钮 **「跳过 CFG-2 连接」**。
- 其 handler `skipCfg2Connect()` 复用 `startEverything` 的连接管线（`start_server` 若未起 + `set_heartbeat_interval` + `connect_substation` 若无 session），末步把 `auto_handshake` 换成 `invoke("skip_cfg2_open", { idcode: target })`（**不传 period**，`target` 同 :174 的 `host:port` 占位逻辑）。
- 共用 `busy` 守卫，避免与正常「连接」并发。
- `frontend/src/composables/usePmuEvents.ts`：处理 `Cfg2Skipped` 事件 → `pushEvent(t("event.cfg2Skipped"), "warn")`（注入标记，非错误，故用 warn 级）。
- `frontend/src/types/index.ts`：`PmuEvent` 联合类型加 `{ type: "Cfg2Skipped"; idcode: string }`。
- `frontend/src/i18n/messages.ts`：加按钮文案 `config.skipCfg2Connect` + 事件文案 `event.cfg2Skipped`（中/英）。

### (D) 子站 `pmusim-sub`

**零改动**——已能在无 CFG-2 时凭 OpenData 推流。

## 5. 端到端数据流（注入后）

```
主站UI[跳过 CFG-2 连接]
  → start_server + connect_substation(不 auto_handshake)
  → skip_cfg2_open
     → do_send_cmd(SendCfg1) → 子站回 CFG-1 帧 → session.cfg1 缓存维度
     → (跳过 下传CFG-2命令 / 下传CFG-2帧 / 召唤CFG-2)
     → emit Cfg2Skipped(主站UI事件区可见)
     → do_open_data_v3(开管道) → do_send_cmd(OpenData)
        → 子站 handle_command(OpenData) → start_stream(无视 CFG-2) ✓
        → 主站 data_read_loop_outbound 用 cfg1 维度解 DataFrame ✓ → state=Streaming
```

## 6. 错误处理 / 边界

- CFG-1 超时未到 → `wait_for_cfg1` 返回 None → emit `Error` 并中止，不进 OpenData。
- 数据管道打开失败（refused/timeout）→ `do_open_data_v3` 已 emit `Error` 并返回 false → 跳过 OpenData，不谎报 streaming（沿用现有约定）。
- 用户在已正常 streaming 的会话上又点该按钮：命令会重发 CFG-1 + OpenData，幂等无害；按钮意在新连接复现，不额外加守卫（YAGNI）。
- 状态复用 `Streaming`，前端状态栏显示 `streaming`；异常性由事件区的 `Cfg2Skipped` 日志体现，不新增状态/徽标。

## 7. 测试

- **集成 e2e**（`crates/pmusim-sub/tests/e2e.rs` —— 该文件已把真实 `MasterStation` + `SubStation` 接在一起，`v3_master_pushes_period_zero_gets_nacked` 即在此）：
  - `v3_master_skips_cfg2_streams_via_cfg1`：起子站 → 主站 `skip_cfg2_open` → 断言 ① 收到 `DataFrame` 且相量/模拟量/数字量个数与子站 CFG-1 一致（凭 CFG-1 解码成功）；② 全程**无** `Cfg2Sent`（主站事件）/ `Cfg2Received`（子站事件）、有主站 `Cfg2Skipped`（确实跳过）。
- **可选** `v2_master_skips_cfg2_streams_via_cfg1`：V2 变体（数据管道方向相反，子站连出主站）。
- 现有 e2e（`v3_master_drives_substation_to_streaming` 等）须保持全绿（证明正常握手未被破坏）。

## 8. 前端无头验证（强制）

含前端改动（ConfigInfoPanel 新按钮 + 事件），发版/合并前用无头浏览器实测：dev server 真实点击「跳过 CFG-2 连接」按钮 → 断言按钮渲染、可见在视口内、未被裁剪;事件区出现 `Cfg2Skipped` 文案。jsdom 不计算布局，必须真实浏览器验证。

## 9. 明确不做（Out of scope）

- 不加「连接后停在 Connected 的手动握手模式」（方案 B/手动分步入口）。
- 不动 `OpenData` 门控的正常语义（仍要求 `Cfg2Sent|Streaming`）。
- 不新增 SessionState/状态徽标（方案 C）。
- V2 变体仅作可选 e2e，UI 不区分 V2/V3 入口。
