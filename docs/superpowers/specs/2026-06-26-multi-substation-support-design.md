# 主站多子站接收支持 + 相量可视化设计文档

## 概述

当前主站(`pmusim-app`)"不支持接收多子站数据"。经核查,**根因在前端展示/状态层,后端已是完整多会话架构**:`MasterStation.sessions: HashMap<String, SubStationSession>` 按 IDCODE 分会话、每会话独立 `cfg1/cfg2`、所有事件携带 `idcode`、V2 按源 IP / V3 按帧内 IDCODE 区分入站、握手/心跳/时戳异常均按 idcode 隔离。旧 Python 版主站本就有 `StationListPanel` 多子站列表;迁移到 Rust+Vue 时后端能力保留、前端展示退化成了单子站。

本次在**不改后端**(唯一例外见"风险与约束")的前提下,补齐前端多子站能力,并顺带补上一个长期被静默丢弃的功能洞:**相量(PMU 的核心测量)从未在 UI 显示**——数据帧的 `DataInfo.phasors` 一直在传到前端却没有任何面板渲染。

本设计合并两件事:

1. **多子站接收**:左侧子站列表 + 切换查看,选中某子站时**全部面板**(数据表/配置/通信日志/异常)只显示该子站。
2. **相量可视化**:数据表补相量通道行,并新增实时极坐标相量图作为本工具的 signature。

### 成功标准

1. 依次连接 2~3 个子站(`pmusim-sub` 或真实子站),左侧列表逐行出现,每行带状态指示灯(LED)与实时 fps。
2. 点击列表任一行,数据表/配置/通信日志/异常面板**立即**整体切换到该子站,数据与表头不再错配,各子站数据互不覆盖、互不串台。
3. 数据表在 STAT 之后显示相量通道行(幅值/相角),并有一个实时旋转的极坐标相量图。
4. 任一子站断线只影响该行(独立重连退避),不影响其他子站。
5. V2(不同 IP)与 V3 各跑通一遍多子站(自动化 e2e/无头组件测 + 手动多 App 对连)。

## 架构判断

后端不动。把前端**所有按子站维度的运行态用 `idcode` 键控**,每个面板读 `xxxByIdcode.get(selectedIdcode)`;新增左侧子站列表当切换器;并解开连接流程对"单一选中会话"的把守。

### 单子站假设清单(待消除)

| 位置 | 现状(单子站假设) | 证据 |
|---|---|---|
| `useCommLog.ts` | `latestData` 单 ref,任何子站数据帧覆盖写 | `latestData.value = { idcode, data }` |
| `useFrameRate.ts` | 全局 fps,被任意子站喂、任意断开 reset | `usePmuEvents` DataFrame 分支无差别 `tickFrameRate` |
| `useTimeOffset.ts` | 全局 offset,同上 | 同上 `tickOffset` |
| `useEventLog.ts` | 条目**无 idcode** 字段 | `EventLogEntry { time, message, kind }` |
| `useReconnect.ts` | 模块级**单目标** FSM(`desired`/`timer`/`attempt`) | 单 `desired: ReconnectTarget \| null` |
| `useAnomalyLog.ts` | 全局数组(已带 idcode,仅缺过滤) | `AnomalyEntry.idcode` 已有 |
| `ConfigInfoPanel.startEverything` | `if (!session.value)` 把守 → 选中已有会话后再点"连接"不再新建,**加不了第二个子站** | line 253 |
| `ConfigInfoPanel` 读数/表单 | fps/offset/状态/IDCODE 绑"选中会话",但连接表单单组 | line 66-68, 184-194 |
| `DataTablePanel.vue` | 数据取全局 `latestData`、列定义取 `selectedIdcode` 的 cfg → 错配;**相量行未渲染** | line 31 vs 14;loop 仅 STAT/analog/digital |
| `App.vue` | 两栏布局,无子站列表组件 | `.content` 仅 Config+Data |

后端侧的 `sessions`(Map)、`configs`(Map)在 `useSessions` 已是多键容器,可直接复用;`selectedIdcode` 单选(查看用)保留。

### 实现路径(选 A)

| 路径 | 做法 | 取舍 |
|---|---|---|
| **A. 既有 composable 按 idcode 键控**(选定) | 上述 composable 内部状态改 `Map<idcode, T>`,面板统一 `.get(selectedIdcode)` | 改动最小、贴合现有"模块级 reactive 单例"风格、各 composable 可独立单测。缺点:per-substation 状态散在多个 composable |
| B. 统一 `useStations` 聚合 store | 一个 `Map<idcode, {data,fps,offset,events,…}>` 整记录 | 封装更干净但要合并多个 composable、触及面更大,超出本特性必要度。后续可朝 B 收敛 |
| C. 每子站一套面板实例按选中显隐 | N 份独立组件 | 重复 DOM/计算,与单例架构对冲,最差 |

## 布局变化

新增左侧子站列表 sidebar,插入 `App.vue` 的 `.content` 最左:

```
┌──────────────────────────────────────────────────────────┐
│ 标题栏                                                      │
├───────────┬─────────────────────┬────────────────────────┤
│ 子站列表   │ ConfigInfoPanel      │ DataTablePanel           │
│ (新增)     │ 连接表单+配置+日志    │ 选中子站:相量行+数据表    │
│ ◉ SUB_A   │                      │ + 极坐标相量图(signature)│
│ ◉ SUB_B   │                      │                        │
│ ○ SUB_C   │                      │                        │
├───────────┴─────────────────────┴────────────────────────┤
│ AnomalyPanel(选中子站异常)                                 │
└──────────────────────────────────────────────────────────┘
```

### `StationListPanel.vue`(新建)

- 遍历 `useSessions.sessions`,每行=一块"插卡":左侧 **pilot LED** + IDCODE + 实时 fps 小读数。
- LED 配色复用现有语义三色,按 `SessionInfo.state` 映射:
  - 绿(`--ok`)= `streaming`
  - 琥珀(`--warn`)= `connecting` / `cfg1_received` / `cfg2_sent`,或 `useReconnect.reconnecting` 为真
  - 红(`--err`)= `disconnected`
- 点击行 → `selectedIdcode = 该行 idcode`;选中行高亮(沿用 `DataTablePanel` 选中行的主蓝)。
- 每行悬浮出"断开/重连"操作(断开 = `stop`/`close_data` 该 idcode;重连 = 触发该行 dialKey 的 `useReconnect`)。
- 视觉走"继电保护机架/板卡槽位"隐喻,与现有阳极氧化金属铭牌语言一致,零新调色板。
- 占位会话(idcode 含 `:`,握手未起)显示为"连接中",不显示为已连接(沿用 `addSession` 现有判定)。

## 前端改动清单

| 文件 | 改法 |
|---|---|
| `useCommLog.ts` | `latestData` 单 ref → `reactive(Map<idcode, DataInfo>)`;`addData(idcode,d)` 分键存;导出按 idcode 取数。raw `logs`(已带 idcode)在面板侧按 `selectedIdcode` 过滤 |
| `useFrameRate.ts` | 内部状态 → `Map<idcode, …>`;`tick(idcode, frameTimeMs)`、`reset(idcode)`、`fpsOf(idcode)` |
| `useTimeOffset.ts` | 同上按 idcode 键控:`tick(idcode, offsetMs)`、`reset(idcode)`、`offsetOf(idcode)` |
| `useEventLog.ts` | `EventLogEntry` 加 `idcode` 字段;`push(idcode, msg, kind)`;面板按 `selectedIdcode` 过滤显示 |
| `useAnomalyLog.ts` | 增加按 idcode 过滤的 getter;`AnomalyPanel` 默认过滤到 `selectedIdcode`(保留现有"全部/分类"筛选作为叠加) |
| `useReconnect.ts` | 单目标 → `Map<dialKey, ReconnectState>`,`dialKey = ${host}:${mgmtPort}`(拨号目标,跨 placeholder→real re-key 稳定);`arm/onDisconnect/cancel/reconnecting` 均按 dialKey 路由 |
| `usePmuEvents.ts` | 所有 `pushEvent`/`tickFrameRate`/`tickOffset`/`reset*` 调用补 `payload.idcode`;`reconnect.onDisconnect` 按断线会话的 dialKey 路由 |
| `ConfigInfoPanel.vue` | "连接"流程去掉 `!session.value` 把守 → 永远用表单值**新增**子站;fps/offset/配置/日志/状态读数改读 `selectedIdcode`;相量/freq 相关读数(见 D1) |
| `DataTablePanel.vue` | 数据改读 `latestDataByIdcode.get(selectedIdcode)`;**补相量行**;新增极坐标相量图(见 D1) |
| `StationListPanel.vue` | 新建(见上) |
| `App.vue` | `.content` 最左插 `<StationListPanel />` |
| `types/index.ts` | `SessionInfo` 增 `dialKey?: string`(连接时由 ConfigInfoPanel 写入,供断线 → ReconnectTarget 反查) |

## 选中态与连接/重连流程

- **连接**:点"连接" → `connect_substation(表单值)` 新增会话 → 后端回 `SessionCreated`(先 placeholder `host:port`,re-key 后 real idcode)→ 列表多一行 → **自动选中最新**(让用户立刻看到它握手/推流)。`useSessions.addSession` 现有"首个自动选中"逻辑改为"新增即选中"。
- **重连键控**:`useReconnect` 改按 `dialKey` 维护多套退避 FSM。连接时 ConfigInfoPanel 以 `arm(dialKey, target)` 登记,并把 `dialKey` 写入对应 `SessionInfo`。`SessionDisconnected`/`HeartbeatTimeout`(携带 real idcode)→ 用 `sessions.get(idcode).dialKey` 找回 FSM 触发重连;各子站退避独立。
  - 实现细节(留给 plan):placeholder→real re-key 时需保留 `dialKey`;`do_connect` 已阻止同 `host:mgmtPort` 重复连接,故 dialKey 唯一。
- **断开/移除选中子站**:该会话被 `removeSession` 后,若它是 `selectedIdcode`,回退到列表中第一个剩余会话,无则清空(`useSessions.removeSession` 现已清 `selectedIdcode`,需补"回退到下一个")。

## D1 — 相量可视化

`DataInfo.phasors`、`freq`、`dfreq`、`format_flags`、`time_quality` 一直在传到前端但未显示。本节补齐。

### 数据表相量行

在 `DataTablePanel` 的 STAT 4 行之后、模拟量行之前插入相量行:

- 行数 = `cfg.phnmr`;名称取 `cfg.channelNames[0 .. phnmr-1]`(channelNames 顺序为 相量→模拟量→数字量,现有代码已用 `analogStart = phnmr` 印证)。
- 每行显示**幅值**与**相角**:
  - `data.format_flags & 1`(bit0)=1 → `phasors[i]` 是 `(magnitude, angle)`;
  - bit0=0 → `phasors[i]` 是 `(real, imag)`,换算 `magnitude = hypot(re,im)`、`angle = atan2(im,re)`。
  - 角度单位以后端 parser 输出为准(C37.118 极坐标定义为弧度);显示**统一换算为度**。plan 阶段先写表征测试锁定后端实际输出单位,避免假设。
- 序号编排:相量行占 `05 .. 04+phnmr`,模拟量/数字量序号顺延(现有 `String(5+i)` 等需相应偏移)。
- 同时把 `freq`(系统频率)/`dfreq`(ROCOF)作为两行专用读数显示(可放在 STAT 区或 ConfigInfoPanel 读数区),`time_quality` 作为同步质量小标识(可选)。

### 极坐标相量图(signature)

数据区放一个零依赖 `<canvas>` 极坐标"钟面"矢量图:

- 每路相量一根从原点出发的矢量:**长度∝幅值**(按当前帧各相量最大幅值归一化)、**方向=相角**。
- 多相量用不同颜色(优先复用通道既有语义;或按相序 A/B/C 分色),图例取通道名。
- 随数据帧更新重绘,呈现相量实时旋转——直接回答"数据在不在流、转得对不对"。
- 仅渲染选中子站;`reduced-motion` 下降级为静态末态(不旋转)。

> 动效自查:旋转矢量是数据语义本身,非装饰,符合"motion 服务主题"。

## D4 — 机架 + LED(并入本次)

即上文 `StationListPanel` 的"机架/插卡 + pilot LED"隐喻——它本身就是多子站切换器,故并入本次实现,不另列 backlog。

## 视觉打磨 backlog(本次不阻塞)

以下来自前端设计评审,记录备查,**不在本次范围**:

- **D2 排版**:引入有性格的工业等宽/窄体作铭牌标题与主读数,建立真正的字号/字重阶梯(主读数 18–22px tabular,次级 13px,caption 11px)。
- **D3 层级**:确立数据/相量区为 hero,config 表单与日志降为次级 chrome。
- **D5 一致性**:`AnomalyPanel` 圆角 pill 徽章(`border-radius:8px`)、面板 `border-radius:4px` 与全站零圆角发丝边语言冲突,收成方角;徽章可改 LED 点 + 数字与 D4 灯语统一。
- **D6 动效即语义**:数据表"刚更新 cell"浅色 flash、fps 读数每帧轻脉冲;仅此一组,克制。

## 风险与约束

- ⚠️ **V2 同 IP 冲突(唯一后端限制)**:V2 数据管道主站为服务端、按**源 IP** 配对子站(V2 数据帧无 IDCODE)。两个子站若来自同一 IP(如本机跑两个 `pmusim-sub`),数据管道无法区分,会串台。**V3 数据帧带 IDCODE 无此问题。** 结论:V2 多子站需用不同 IP 测;本机同 IP 多子站仅 V3 支持。本次记为已知约束,不在前端解决。
- **重连键控**:dialKey 跨 re-key 的保留是主要实现风险,plan 阶段先写表征测试锁住"占位→真实 idcode 后断线仍能按原拨号目标重连"。

## 测试策略

- **composable 单测**(vitest):每个按 idcode 键控的 composable,用两子站交叉喂数据,断言互不串台、互不覆盖、按 idcode 取数正确;`useReconnect` 双 dialKey 独立退避。
- **无头组件测**(vitest + happy-dom,沿用现有 `ConfigInfoPanel` 测试风格):
  - `StationListPanel`:多 session 渲染、LED 状态映射、点击切换 `selectedIdcode`。
  - `DataTablePanel`:相量行渲染(极坐标/直角两种 format)、选中切换后数据与表头一致。
- **相量换算单测**:`(re,im)→(mag,angle)` 已知输入→已知输出;归一化逻辑。
- **手动**:启动 2~3 个 `pmusim-sub`(V3 同机不同 IDCODE;V2 不同 IP)连主站,验证列表、切换、相量图、独立重连全流程。

## 范围与非目标

- **包含**:多子站列表+切换(全部面板跟随选中)、机架+LED sidebar(D4)、连接流程解单会话把守、按 idcode 键控全部运行态、按 dialKey 多目标重连、相量行+极坐标相量图+freq/dfreq 读数(D1)。
- **不做(YAGNI/后续)**:
  - 多子站同屏并列(本次只做切换)。
  - 连接预设管理器(多组可保存目标 + 批量连接)。
  - 启动时自动重连整批。
  - V2 同 IP 多子站区分(后端限制,见风险)。
  - 视觉打磨 D2/D3/D5/D6(见 backlog)。
