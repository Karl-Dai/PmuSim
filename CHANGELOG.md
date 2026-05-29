# Changelog

All notable changes to PmuSim are documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
