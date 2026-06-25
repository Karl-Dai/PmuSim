# 设计:主站非服务端自动重连

- 日期:2026-06-25
- 状态:已批准设计,待实现
- 范围:`crates/pmusim-app`(前端 `frontend/`)主站模拟器

## 背景与问题

主站第一次连接子站成功后,若 client 侧连接意外断开(子站掉线、网络中断、心跳超时),当前行为是:

- 后端 `master.rs` 把 `session.state = Disconnected`,发 `SessionDisconnected` / `HeartbeatTimeout` 事件;
- 前端 `usePmuEvents` 收到后 `removeSession`(删会话)+ 报错,**无任何自动重连**;
- 用户必须手动再点「连接」才能恢复。

注意:手动再点「连接」时,`startEverything` 的 `if(!running)` 守卫会让那次只重连 client 侧(`connect_substation` + 握手),本地 data server 不重启——这本身就是"非服务端重连"。本设计的目标是把这个过程**自动化**。

## 需求(已确认)

1. **触发方式**:断线后**自动**重连,无需人工点击。
2. **触发条件**:`SessionDisconnected` 与 `HeartbeatTimeout` 两类意外断开都触发。
3. **忠实恢复断线前状态**:断前在 streaming(已开流)→ 重连后自动握手 + 按原速率恢复数据流;断前只连着命令通道(已暂停 `close_data`)→ 只重连命令通道,不自动开流。
4. **重试策略**:指数退避无限重试 —— 间隔 1→2→4→8→16→**封顶 30s**,一直试到连上(重置)或用户手动停止。
5. **用户主动断开不重连**:点「停止」/「断开」属于主动断开,必须取消/不触发自动重连。
6. **server 不重启**:意外断线不影响本地 data server(`running` 保持),重连只走 client 侧。

### 非目标(YAGNI)

- 不做后端(`master.rs`)驱动的重连。
- 不新增独立的「停止重连」按钮——现有「停止」/「断开」即是停止入口(见边界处理)。
- 不持久化重连状态(app webview 常驻,无此需要)。

## 方案选择

### 方案 A — 前端驱动重连(采用)

新增 `useReconnect` composable:`usePmuEvents` 收到断线事件时触发它,用指数退避 `setTimeout` 调度,重放现有 `connect_substation` + 握手命令。后端**零改动**。

### 方案 B — 后端驱动重连(不采用)

`master.rs` 在 client 侧断开时自身发起 tokio 退避重连循环。需把重连参数(host/port/protocol/period/断前状态)缓存进后端,且 `master.rs` 是核心连接管理,历史上踩过"双 IDCODE 握手丢会话"的坑,改动风险高。

### 采用 A 的理由

1. 重连参数(connIp / 端口 / rateHz / protocol / 断前是否开流)本就活在前端;
2. 复用现有 invoke 命令,后端不动,风险最低;
3. 与现有"前端 poll_events 驱动一切"的架构一致;
4. 退避调度在 TS 里清晰、可单测。

## 详细设计

### 新组件 `frontend/src/composables/useReconnect.ts`

单一职责:管理 client 侧重连。模块级单例(与 `useSessions` / `useProtocol` 同风格)。

**状态**

- `desired: ReconnectTarget | null` —— "期望连接快照":
  `{ host, mgmtPort, dataPort, protocol, period, mode: 'normal' | 'skipCfg2', streaming: boolean }`
- `intentional: boolean` —— 用户是否主动断开(主动则不重连)。
- `attempt: number` —— 当前退避次数,决定下次延迟。
- `timer` —— 挂起的 `setTimeout` 句柄。
- `reconnecting: ref<boolean>` —— 供 UI 显示"重连中"。

**API**

- `arm(snapshot)` —— 连接成功后记录期望快照;`intentional = false`。
- `setStreaming(on)` —— 用户 `close_data`(暂停)时 `desired.streaming = false`;重新开流时置回 `true`。
- `onDisconnect()` —— 断线事件调用。若 `intentional` 或 `desired == null` 则忽略;否则启动退避重连。
- `cancel(intentional = true)` —— 用户主动停止/断开时调用:清挂起 timer、置 `intentional`、`reconnecting = false`。
- 内部 `scheduleRetry()` —— `setTimeout(delay(attempt))` → 尝试重连;成功 `attempt = 0` + `reconnecting = false`;失败 `attempt++` 后继续。
- `delay(attempt)` = `min(2^attempt * 1000, 30000)` ms。

**重连动作**(重放,等价于一次"非服务端连接"):

```
await invoke("connect_substation", { host, port: mgmtPort, dataPort: protocol==='V3' ? dataPort : undefined })
if (desired.streaming) {
  if (mode === 'skipCfg2') await invoke("skip_cfg2_open", { idcode: target })
  else await invoke("auto_handshake", { idcode: target, period })
}
```

`target = `${host}:${mgmtPort}``(与 `startEverything` 一致,后端从 peer 解析真实 idcode)。

### 集成点(3 处接线)

1. **`ConfigInfoPanel.vue`**
   - `startEverything` 成功末尾:`reconnect.arm({ ...表单值, mode: 'normal', streaming: true })`。
   - `skipCfg2Connect` 成功末尾:`reconnect.arm({ ..., mode: 'skipCfg2', streaming: true })`。
   - 暂停数据(`close_data`)成功:`reconnect.setStreaming(false)`。
   - `stopServer` / 断开:`reconnect.cancel(true)`。
2. **`usePmuEvents.ts`**
   - `SessionDisconnected` 与 `HeartbeatTimeout` 分支,在 `removeSession` 之后调用 `reconnect.onDisconnect()`。
3. **事件日志**
   - 重连中:每次重试记一条"重连中…(第 N 次)",不弹 toast 风暴。
   - 连上后:记一条"已重连"。

### 关键边界

- **主动 vs 意外**:`cancel(true)` 在用户主动操作时置 `intentional`,挡掉随之而来的 `SessionDisconnected`。下一次正常连接 `arm` 会清回 `false`。
- **配置变更 / 手动停止取消挂起重试**:`cancel` 清 timer,避免连到旧目标或重连风暴。
- **server 不重启**:意外断线不改 `running`,本地 data server 仍监听,重连只走 client 侧。
- **忠实恢复**:由 `desired.streaming` 驱动——streaming 则恢复开流,暂停则只连命令通道。

### 错误处理

- 每次重试失败(`connect_substation` 抛错)→ `attempt++`,按退避继续,不弹错误风暴。
- 仅事件日志记录进度,UI 通过 `reconnecting` 显示状态。

## 测试计划(先写测试)

`frontend/src/composables/__tests__/useReconnect.spec.ts`(vitest,假定时器):

1. **退避序列**:连续失败时延迟为 1s/2s/4s/8s/16s/30s/30s…(封顶 30s)。
2. **连上后重置**:一次成功后 `attempt` 归零、`reconnecting=false`。
3. **主动断开不重连**:`cancel(true)` 后 `onDisconnect()` 不发起重连;挂起 timer 被清。
4. **忠实恢复**:`desired.streaming=true` 时重连调 `auto_handshake`(`mode:normal`)/ `skip_cfg2_open`(`mode:skipCfg2`);`streaming=false` 时只调 `connect_substation`。
5. **V2/V3 端口**:V3 传 `dataPort`,V2 不传。
6. **配置变更取消**:`cancel` 后旧 timer 不再触发。

### 后端验证点

实现时需验证一点(非改动,仅确认):旧 session 已 `Disconnected`/`removed` 后,对同一 target 重新 `connect_substation` 不被残留状态阻塞。若被阻塞,再评估是否需要后端小修。

## 影响文件

- 新增:`frontend/src/composables/useReconnect.ts`、对应 `__tests__/useReconnect.spec.ts`
- 修改:`frontend/src/composables/usePmuEvents.ts`、`frontend/src/components/ConfigInfoPanel.vue`
- i18n:`frontend/src/i18n/messages.ts` 新增"重连中…/已重连"文案
