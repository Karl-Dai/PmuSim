# Changelog

All notable changes to PmuSim are documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.6.0] - 2026-06-02

### Highlights / 亮点

- 🧪 受控异常注入 V3:主站可直发「非法上送周期」的 CFG-2(原始 PERIOD,含 0 与越界值),绕过 Hz→PERIOD 换算,用来验证子站对规约未定义周期的应对 / Controlled abnormal injection (V3): the master can now push a CFG-2 carrying an *illegal* reporting period (raw PERIOD, including 0 / out-of-range), bypassing the Hz→PERIOD conversion — for exercising how a substation handles a period the spec never defines.
- 🛡️ 子站校验上送周期:下传 CFG-2 时若 PERIOD 非法(0)则回 NACK 拒绝,不再盲目接受坏配置 / Substation now validates the reporting period on an incoming CFG-2 — an illegal PERIOD (0) is rejected with a NACK instead of being silently accepted.
- 📣 拒绝过程全程可见:子站事件区实时展示「已回 NACK 拒绝」(含原因),主站端 NACK 不再被静默吞掉而是上抛错误提示 / The rejection is fully visible end-to-end: the substation event log shows the NACK (with reason) live, and on the master side a NACK is no longer swallowed silently but surfaced as an error.

### Added 新增

- 主站「异常注入」入口:配置面板新增勾选项,展开后可填原始 PERIOD 值并一键「注入」直发 CFG-2,按钮仅在会话处于 streaming / cfg2_sent 时可用 / Master "abnormal injection" entry: a checkbox in the config panel reveals a raw-PERIOD field and an **Inject** button that sends the CFG-2 directly; enabled only while the session is in `streaming` / `cfg2_sent`.
- `pmusim-core` 新增 `ConfigFrame::illegal_period_reason`,集中判定上送周期是否非法(PERIOD=0)/ `pmusim-core` gains `ConfigFrame::illegal_period_reason`, centralizing the check for an illegal reporting period (PERIOD = 0).
- 子站新增 `Cfg2Rejected { reason }` 事件,前端事件区以错误样式展示拒绝原因 / New substation `Cfg2Rejected { reason }` event; the frontend event log renders the rejection reason in an error style.
- 端到端测试新增 `v3_master_pushes_period_zero_gets_nacked`,覆盖主站推送 PERIOD=0 → 子站 NACK 的完整链路 / Added the end-to-end test `v3_master_pushes_period_zero_gets_nacked` covering the full master-pushes-PERIOD-0 → substation-NACK path.

### Changed 改进

- 子站收到下传 CFG-2 时先校验上送周期再决定接受/拒绝,而非无条件接受 / On an incoming CFG-2 the substation validates the reporting period before accepting, rather than accepting unconditionally.

### Fixed 修复

- 主站在没有 ack waiter 时收到 NACK 不再静默丢弃,改为 emit Error 上抛 UI,避免拒绝结果"消失" / On the master, a NACK arriving with no ack waiter is no longer silently dropped — it now emits an Error to the UI so the rejection isn't lost.
- 异常注入的原始 PERIOD 增加 u16 上界校验(>65535 会回绕成 0),非法取值直接提示而不误发 / The raw-PERIOD field is now bounds-checked against u16 (values >65535 would wrap to 0); an illegal value is flagged instead of being sent by mistake.

## [0.5.0] - 2026-06-01

### Highlights / 亮点

- 🛰️ 全新 PMU 子站(数据发送方)模拟器 **PmuSub**：独立桌面应用，可与主站本地互测,不再需要真实子站或半成品脚本 / New PMU substation (data-sender) simulator **PmuSub** — a standalone desktop app you can test the master against locally, no real substation or half-broken script needed.
- 📦 一次发布两个安装包:主站 `PmuSim_*` 与子站 `PmuSub_*` 同 tag 随 release 一起分发 / Both apps ship from one release — master `PmuSim_*` and substation `PmuSub_*` installers attached to the same tag.
- 🟡 修复主站「假已连接」:TCP 连上但子站未回 CFG-1 时显示琥珀色「连接中」,真正收到 PMU 帧才转绿「已连接」 / Fixed the master's false "已连接": amber **连接中 (connecting)** while the TCP socket is up but no CFG-1 has arrived; green **已连接 (connected)** only after a real PMU frame.
- 📖 README 全面重做:SVG banner、中/EN 实拍截图、握手演示 GIF、引导式快速上手、FAQ/路线图/贡献 / README fully redesigned: SVG banner, 中/EN screenshots, handshake demo GIF, guided Quick Start, FAQ / Roadmap / Contributing.

### Added 新增

- PMU 子站模拟器 `pmusim-sub`(产品名 PmuSub):独立 Tauri 2 App,支持 V2/V3 双协议、命令响应全握手、可配置正弦相量数据生成(Δf/ROCOF)、中/英双语界面、CFG-2 配置预设(JSON 保存/加载),与主站对标可本地互测 / Substation simulator `pmusim-sub` (product PmuSub): standalone Tauri 2 app — V2/V3 dual protocol, full command-response handshake, configurable sinusoidal phasor data generation (Δf/ROCOF), bilingual 中/EN UI, CFG-2 config presets (JSON save/load); designed to interop-test against the master locally.
- 会话新增「连接中」状态(idcode 仍是占位 `host:port` 时),与「已连接」(子站已回帧、re-key 为真实 IDCODE)区分 / New `connecting` session state (while the idcode is still the placeholder `host:port`), distinct from `connected` (substation has replied and the session re-keyed to its real IDCODE).

### Changed 改进

- 发布流程同时构建并附带子站安装包:`release.yml` 新增非阻塞 `release-sub` job(macOS×2 / Windows x64 / Linux),`build-release-notes` 下载表拆分为「主站 PmuSim / 子站 PmuSub」两节。子站无 in-app updater,故不签名、不进 update manifest / Release pipeline now also builds the substation: `release.yml` gains a non-blocking `release-sub` job (macOS×2 / Windows x64 / Linux); the release-notes download table splits into 主站 PmuSim / 子站 PmuSub. The sub has no in-app updater, so it is unsigned and excluded from the update manifest.
- README.md / README_CN.md 精简重构,协议大表折叠进 `<details>`,新增 FAQ / 路线图 / 贡献指南 / 致谢;旧 simpmufep 截图替换为当前 PmuSim 实拍 / README.md / README_CN.md restructured leaner — protocol tables folded into `<details>`, new FAQ / Roadmap / Contributing / Acknowledgments; stale simpmufep screenshot replaced with current PmuSim captures.

### Fixed 修复

- 主站「假已连接」:子站命令端口能 TCP accept 但不是真 PMU(不回 CFG-1)时,状态栏曾一直绿字「已连接」直到心跳超时。现占位会话(`host:port`)显示琥珀色「连接中」,仅在子站真正回帧后才转绿 / Master false "已连接": when a command port accepts TCP but isn't a real PMU (never returns CFG-1), the status stayed green "已连接" until heartbeat timeout. Placeholder sessions (`host:port`) now read amber **连接中**; green only after the substation actually replies.

## [0.4.0] - 2026-05-29

### Highlights / 亮点

- 🌐 中/英界面运行时切换: 标题栏 `中 / EN` 开关, 首次按系统语言自动判断, 选择本地记忆, 全部 UI 文案与语义标签即时切换 / Runtime 中/English UI toggle in the title bar; first run follows the OS locale, the choice is remembered locally, and all UI text + semantic labels switch live.
- 🔌 修正 V2 (2006) 数据端口字段: 重新暴露并正名为「本地侦听端口」(主站作为数据通道服务端的本地绑定口) / Fixed the V2 (2006) data-port field — re-exposed and renamed to "本地侦听端口" (the master's local bind port as data-pipe server).
- 💄 配置面板金属铭牌化重做, 标题栏品牌名统一为 PmuSim / Config panel restyled with a metal-nameplate header; title-bar product name unified to PmuSim.

### Added 新增

- 中/英界面切换: 标题栏 `中 / EN` 开关, 首次按系统语言自动判断(`navigator.language`), 选择记忆在本地(`localStorage`, 同步读取无闪屏); 覆盖全部 UI 文案与前端语义标签(STAT 解码 / 触发原因 / 开关量 / 会话状态)。零新依赖, 自研 `useI18n` composable, 82 个翻译 key 双语对齐 / Runtime 中/English UI toggle in the title bar; first run follows the OS locale (`navigator.language`), the choice is remembered in `localStorage` (synchronous read, no flash); covers all UI text and frontend semantic labels (STAT decode / trigger reason / digital state / session state). Dependency-free in-house `useI18n` composable, 82 translation keys aligned across both locales.

### Changed 改进

- `ConfigInfoPanel`: V2 (2006) 也显示数据端口字段并命名为「本地侦听端口」(默认 命令端口+1, 可编辑) —— V2 主站是数据通道的**服务端**, 在本地 `bind` 此端口等子站主动上送; V3 (2011) 仍显示「数据端口」(主站外连子站的远程数据口)。修正 0.3.x simpmufep UI 重做时把端口字段可见性反转、V2 本地侦听端口被一并隐藏的问题 / `ConfigInfoPanel`: V2 (2006) now also shows the data-port field, labeled "本地侦听端口" (defaults to command-port + 1, editable) — the V2 master is the data-pipe **server** and binds this local port for the substation to push to; V3 (2011) keeps "数据端口" (the remote substation port the master dials out to). Fixes the 0.3.x simpmufep redesign hiding the V2 local-listen port.
- `ConfigInfoPanel` 由 fieldset/legend 改为 `.panel` 金属铭牌表头结构, 事件日志面板同步 flex 布局 / `ConfigInfoPanel` migrated from fieldset/legend to a `.panel` metal-nameplate header layout; event-log panel updated to a matching flex layout.
- 标题栏产品名由 simpmufep 统一为 PmuSim / Title-bar product name unified from simpmufep to PmuSim.

## [0.3.1] - 2026-05-29

### Highlights / 亮点

- 🪟 新增 Windows on ARM (aarch64) 原生安装包, ARM 设备不再依赖 x64 仿真, 且同样支持应用内自动更新 / Native Windows on ARM (aarch64) installer — ARM devices no longer rely on x64 emulation and get in-app auto-update too.

### Added 新增

- Windows on ARM (aarch64) NSIS 安装包: release 矩阵新增 `aarch64-pc-windows-msvc` 交叉编译, 产出 `PmuSim_<ver>_arm64-setup.exe` 并纳入 `windows-aarch64` updater manifest / Windows on ARM (aarch64) NSIS installer: the release matrix now cross-compiles `aarch64-pc-windows-msvc`, producing `PmuSim_<ver>_arm64-setup.exe` and registering it under the `windows-aarch64` updater key.

### Internal 内部

- arm64 矩阵 leg 设为 `continue-on-error`(首次交叉编译容错): 该 leg 失败也不会拖垮其余平台发版或把 release 卡在 draft / The arm64 matrix leg is `continue-on-error` (first cross-compile, fault-tolerant) so its failure can't block the other platforms or leave the release stuck as a draft.

## [0.3.0] - 2026-05-28

### Highlights / 亮点

- 🔄 应用内自动更新: 启动静默检查 + 标题栏「检查更新」按钮; 6h 节流, 24h snooze 同版本 / In-app auto-update: silent check on startup + "检查更新" button in the title bar; 6h throttle, 24h snooze per version.
- 🇨🇳 国内三镜像 + GitHub 四路回退, 直连失败也能装到最新版 / Four-way endpoint fallback (ghfast.top / gh-proxy.com / gh.idayer.com + GitHub) so mainland users still pick up updates when the direct GitHub link is blocked.
- 🔐 GitHub Actions 用 minisign 签名 updater 产物, app 端验签后才安装 / Updater artifacts are minisign-signed in GitHub Actions; the app verifies signatures before installing — no MITM via the CN proxies.
- 📝 Release body 自动从 CHANGELOG 渲染: 平台下载表 + 本版本变更 + macOS Gatekeeper 提示 / Release body now auto-renders from CHANGELOG via `scripts/build-release-notes.mjs` — platform download table, version section, and macOS Gatekeeper note all included.
- 💄 配置/数据面板小幅 UI 收敛: label 去冒号, 端口字段加 `inputmode=numeric`, 速率/读回值并排 / Minor UI polish on the config & data panels: colons dropped from labels, port fields opt into numeric IME, sample-rate readback inlined next to the dropdown.

### Added 新增

- `tauri-plugin-updater` 接入, 新增 `check_for_update / install_update / snooze_update` 三个 command (`crates/pmusim-app/src/update.rs`) / Wired `tauri-plugin-updater`; new commands `check_for_update / install_update / snooze_update` (`crates/pmusim-app/src/update.rs`).
- 标题栏「检查更新」按钮 + `UpdateDialog.vue` (markdown 渲染 + 进度条 + 错误重试) / Title-bar "检查更新" button + `UpdateDialog.vue` (lightweight markdown rendering, progress bar, retry-on-error).
- `scripts/gen-update-manifest.mjs` + `scripts/build-release-notes.mjs`: 拉 release assets, 生成 4 份 `latest-pmusim{,-cn1,-cn2,-cn3}.json` updater manifest, 渲染富格式 release body / `scripts/gen-update-manifest.mjs` + `scripts/build-release-notes.mjs`: pull release assets, emit four `latest-pmusim{,-cn{1,2,3}}.json` updater manifests, render the rich release body.
- `crates/pmusim-app/capabilities/default.json`: updater/process/store/dialog 权限 / `crates/pmusim-app/capabilities/default.json` granting updater/process/store/dialog permissions.

### Changed 改进

- `.github/workflows/release.yml`: 加 `TAURI_SIGNING_PRIVATE_KEY` env + `includeUpdaterJson: false`, 新增 `publish-manifest` job 跑 manifest 生成 + release body 渲染 / `.github/workflows/release.yml` now signs updater bundles (`TAURI_SIGNING_PRIVATE_KEY`), skips tauri-action's clobber-prone `latest.json`, and runs a new `publish-manifest` job to emit signed manifests and replace the release body.
- `crates/pmusim-app/tauri.conf.json`: `createUpdaterArtifacts: true`, 配置 4 个 updater endpoints (CN proxy 优先, GitHub 兜底), 嵌入 minisign 公钥 / `crates/pmusim-app/tauri.conf.json` enables `createUpdaterArtifacts`, lists four updater endpoints (CN proxies first, GitHub last), embeds the minisign pubkey.
- 版本号统一到 0.3.0 (Cargo.toml / tauri.conf.json / frontend/package.json 之前在 0.1.0 与 0.2.0 之间错位) / Version unified to 0.3.0 across Cargo.toml, tauri.conf.json and frontend/package.json (they had drifted between 0.1.0 and 0.2.0).
- ConfigInfoPanel / DataTablePanel: label 去冒号, 端口字段 `inputmode=numeric`, 速率行加 `.ctl-with-suffix` 容器 / `ConfigInfoPanel` / `DataTablePanel`: colons removed from labels, port inputs use `inputmode="numeric"`, sample-rate row wrapped in `.ctl-with-suffix`.

## [0.2.0] - 2026-04-16

### Added
- Vue 3 + TypeScript + Vite 前端脚手架, 实现 toolbar/stations/config/data/log 全部
  面板; useProtocol 跨面板共享协议状态; useServerStatus/useToast composable.
- Tauri 2 应用层 (`pmusim-app`): MasterStation tokio 网络层, 事件总线 (后端缓冲 +
  前端轮询), 自动握手, V3 主站发起的数据通道 (GB/T 26865.2-2011).
- `pmusim-core` 协议库: 帧解析/构建 (V2/V3), CommandFrame/ConfigFrame/DataFrame,
  SOC/FRACSEC 时间转换工具.
- 跨平台 release 工作流 (Tauri cross-platform build): macOS aarch64/x86_64,
  Linux x64, Windows x64.
- 端口配置重设计: V3 隐藏 data port (主站外联), V2 命名为「本地侦听端口」;
  station 面板 mgmt/data 端口分列 + 自动跟随 + dirty tracking.
  (注: 此可见性规则在 0.3.x simpmufep UI 重做后一度反转为「V2 隐藏 / V3 显示数据端口」,
  Unreleased 已把 V2 的本地侦听端口重新暴露 —— 见顶部 Unreleased 段。)
- headless_smoke example 驱动 MasterStation 全链路 PMU 集成测试.

### Fixed
- ConfigInfo serde `rename_all = "camelCase"` 使 channelNames 正确落到前端.
- 主站防重复连接竞态; 5s TCP 超时; 改用原子 pending 占位.
- V2 CFG-2 downstream 类型修正; re-keyed session 正确跟随.
- port-collision 重试测试; 协议切换重置 dataPort dirty 标志.

## [0.1.0] - 2026-04-15

### Added
- 初始版本: PMU 协议核心库骨架, Tauri 应用占位, PyInstaller CI.
