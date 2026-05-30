# pmusim-sub — PMU 子站（数据发送方）模拟器设计文档

## 概述

`pmusim-sub` 是一个 PMU 子站模拟器，与现有 PmuSim 主站对标——主站是数据接收方（数据集中器），子站是数据发送方（一次调频系统 / PMU）。用于在没有真实子站的情况下，向 PmuSim 主站或真实主站发送符合协议的配置帧与实时数据帧。

- **角色**：子站（数据发送方）
- **协议**：同时支持 V2（Q/GDW 131-2006）与 V3（GB/T 26865.2-2011），界面可切换
- **打包**：workspace 内**新增独立 Tauri App**（`crates/pmusim-sub` + `frontend-sub/`），与主站平行，复用 `pmusim-core`
- **数据**：可配置正弦相量（幅值/相角/系统频率偏移/ROCOF），模拟量与数字量可设定值
- **配置来源**：UI 表单手动配置，内置默认模板起步，支持保存/加载 JSON 预设
- **技术栈**：Rust + Tauri 2 + Vue 3（镜像主站）

### 成功标准

1. 启动 `pmusim-sub` 配置一套子站（站名/IDCODE/通道/速率），开始监听。
2. 用 PmuSim 主站（或真实主站）连接，完整跑通握手：
   `召唤 CFG-1 → 下传 CFG-2 命令 → 上传 CFG-2 → 召唤 CFG-2 → 打开实时数据`。
3. 主站数据表出现子站申报的通道名，且相量随时间按设定频率旋转、数值符合配置。
4. V2 与 V3 各跑通一遍（自动化 e2e + 手动两 App 对连）。

## 架构与代码组织（镜像主站）

```
PmuSim/
├── crates/
│   ├── pmusim-core/       # 协议库（不变，主/子站共用，补齐对称缺口）
│   ├── pmusim-app/        # 主站 Tauri App（不动）
│   └── pmusim-sub/        # 新增：子站 Tauri App
│       └── src/
│           ├── network/
│           │   ├── substation.rs   # SubStation：反向 TCP 角色 + 命令响应状态机
│           │   ├── session.rs       # 与主站的连接会话
│           │   └── mod.rs
│           ├── datagen.rs           # 纯逻辑：正弦相量 → DataFrame（Tauri-free，可单测）
│           ├── commands.rs          # Tauri 命令
│           ├── events.rs            # SubEvent 枚举（poll 缓冲模型）
│           ├── state.rs             # AppState + EventBuffer
│           ├── lib.rs / main.rs
│           └── tauri.conf.json / build.rs / capabilities / icons
├── frontend-sub/          # 新增：Vue3+Vite+TS，复用主站 i18n / 部分 composables / 表格组件
```

### pmusim-core 复用与缺口

core 已具备 `build_config`（建 CFG-1/CFG-2）、`build_data`（建数据帧）、`parse`（解析任意帧，含命令帧）——协议是对称的，子站直接复用。**实现阶段需核对/补齐的对称缺口**：

- `build_data` 对 V2/V3 的字段顺序与 IDCODE 差异分支是否完整（V3 数据帧含 IDCODE，V2 不含）。
- `parse` 返回的命令帧是否暴露命令字（CMD code），供子站状态机分派；如无，补一个便捷解析函数。
- `build_config` 是否能按需产出 CFG-1 与 CFG-2 两种（SYNC bit 区分）。

这些在 plan 阶段先写表征测试确认现状，再按需最小补齐，不重写 core。

## 协议规范摘要

### 帧类型与方向（子站视角）

| 帧类型 | 方向 | 子站职责 |
|--------|------|---------|
| 数据帧 Data | 子站→主站 | **生成并发送**（数据管道） |
| 配置帧 CFG-1 | 子站→主站 | 被召唤时**构建并上传**（管理管道） |
| 配置帧 CFG-2 | 双向 | 被召唤时**构建并上传**；接收主站下传的 CFG-2 命令 |
| 命令帧 Command | 主站→子站 | **接收并解析**，驱动状态机 |

### TCP 角色（子站 = 主站的镜像）

| 管道 | 主站角色 | **子站角色** | V2 端口 | V3 端口 |
|------|---------|-------------|---------|---------|
| 管理 Management | 客户端 | **服务端** | 7000 | 8000 |
| 数据 Data | V2=服务端 / V3=客户端 | V2=**客户端**（连主站数据口）/ V3=**服务端**（监听等主站连） | 7001 | 8001 |

### V2 与 V3 关键差异（与主站 spec 一致）

| 特性 | V2 (2006) | V3 (2011) |
|------|-----------|-----------|
| IDCODE 长度 | 2 字节 | 8 字节（ASCII） |
| 帧头字段顺序 | SYNC-SIZE-SOC-IDCODE | SYNC-SIZE-IDCODE-SOC |
| 数据帧 IDCODE | 无 | 有 |
| 时间质量 | 4 bit | 8 bit |
| 数据管道方向 | 主站=服务端 | 主站=客户端 |

## 网络层：命令响应状态机

```
子站                                        主站
 │ 监听管理端口(7000/8000)                      │
 │◄────────────── 建立管理连接 ───────────────│
 │◄──── 召唤 CFG-1 (CMD 0x0004) ───────────────│
 │──────────────── 上传 CFG-1 ────────────────►│
 │◄──── 下传 CFG-2 命令 (CMD 0x8000) ──────────│
 │──────────────── 肯定确认 ──────────────────►│
 │◄──── 召唤 CFG-2 (CMD 0x0005) ───────────────│
 │──────────────── 上传 CFG-2 ────────────────►│
 │◄──── 打开实时数据 (CMD 0x0002) ─────────────│
 │   (V2: 子站主动连主站数据口 7001)            │
 │   (V3: 子站在数据口 8001 等主站连入)         │
 │═══════════════ 实时数据帧流 ═══════════════►│  按 DATA_RATE 定时推送
 │◄──── 心跳 (CMD 0x4000) ─────────────────────│  记录/应答
 │◄──── 关闭实时数据 (CMD 0x0001) ─────────────│  停止推流
```

命令分派表：

| 命令 | CMD | 子站动作 |
|------|-----|---------|
| 关闭实时数据 | 0x0001 | 停止数据推流任务 |
| 打开实时数据 | 0x0002 | 建立/启用数据管道，按速率推流 |
| 召唤 CFG-1 | 0x0004 | `build_config`(CFG-1) 回发 |
| 召唤 CFG-2 | 0x0005 | `build_config`(CFG-2) 回发 |
| 心跳 | 0x4000 | 记录，必要时应答 |
| 下传 CFG-2 命令 | 0x8000 | 记录主站通知，回肯定确认 |

数据管道建立时机由 `打开实时数据` 触发，方向按协议（V2 子站 connect 主站数据口；V3 子站 accept）。实现复用 tokio 异步，参照 `master.rs` 的任务/通道结构，角色相反。

## 数据生成（datagen，纯逻辑可单测）

- **相量通道**：每路可设幅值、初相角；输出格式跟随 CFG-2 format flags（极/直角、float/int）。
- **系统频率**：50Hz 标称 + 可设偏移 Δf + ROCOF（df/dt）。第 n 帧时刻 `t = n / DATA_RATE`，相角 `θ(t) = θ0 + 2π·(fnom+Δf)·t + π·ROCOF·t²`。
- **模拟量 / 数字量**：可设定值；数字量为位掩码。
- **时标**：SOC / FRACSEC 由 `time_utils` 生成（V2 4bit / V3 8bit 时间质量）。
- **速率**：按 CFG-2 `DATA_RATE`（25 / 50 / 100 fps）用定时器推帧。
- **触发**：一次性触发帧（置 trigger 状态位）。
- datagen 输入为配置 + 帧序号/时刻，输出 `DataFrame` 结构，交 `build_data` 编码——不依赖 Tauri、不做 IO，便于单测。

## 配置与界面

复用主站 `simpmufep` 风格布局（左：配置 + 事件日志；右：数据/状态），i18n 中英可切换。

- **CFG-2 编辑表单**：站名（GBK）、IDCODE（V2 2字节 / V3 8字节 ASCII）、相量/模拟量/数字量通道（名称 + 变比 + 类型）、FNOM、DATA_RATE。
- **数据生成面板**：每相量幅值/相角、Δf、ROCOF、模拟量/数字量值，运行中可改（live-update）。
- **预设**：保存/加载 JSON（沿用主站 `tauri-plugin-store` 或文件对话框）。
- **状态/日志**：监听/连接状态、收到的命令、发出的帧（hex on demand）、已发数据预览表。

### Tauri 面（沿用主站 poll 缓冲模型）

- 命令：`start_substation`（按协议/角色起监听）、`stop_substation`、`update_config`（CFG 定义）、`update_datagen`（生成参数）、`trigger`、`poll_events`、`open_url`。
- 事件 `SubEvent`：`Listening`、`MasterConnected`、`CommandReceived`、`Cfg1Sent`、`Cfg2Sent`、`StreamingStarted/Stopped`、`DataFrameSent`、`RawFrame`、`Error`。

## 测试策略

- **core 单测**：datagen 相量旋转（已知输入→已知幅值/相角）、V2/V3 数据帧字节布局、CRC round-trip。
- **集成 e2e**：在测试里用 `pmusim-app`（库）作主站连 `pmusim-sub` 子站，跑完整握手，断言主站收到正确的 CFG（站名/通道）与数据（相量值随时间变化）。V2、V3 各一例。参照现有 `crates/pmusim-app/tests/e2e.rs` 与 `examples/headless_smoke.rs`。
- **手动**：两个 App 本地对连验证全流程 + UI。

## 范围与非目标（v1）

- **包含**：V2+V3 双协议、命令响应全流程、正弦相量数据生成、CFG-2 表单+预设、镜像主站 UI、自动化 e2e。
- **不做（YAGNI，后续）**：
  - 子站独立签名安装包 + 自动更新流水线（主站那套 updater/release 较重）——v1 仅源码构建 + 本地使用。
  - 回放抓包文件、随机噪声等其他数据来源。
  - 多子站并发（先单子站，结构上不排斥后续扩展）。
