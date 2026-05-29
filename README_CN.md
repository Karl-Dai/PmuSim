<div align="center">

# 📡 PmuSim

**跨平台 PMU 主站模拟器 — 一个桌面工具同时跑 Q/GDW 131-2006 (V2) 与 GB/T 26865.2-2011 (V3)。**

[![Release](https://img.shields.io/github/v/release/Karl-Dai/PmuSim?label=release&color=2ea043)](https://github.com/Karl-Dai/PmuSim/releases)
[![Downloads](https://img.shields.io/github/downloads/Karl-Dai/PmuSim/total?color=1f6feb)](https://github.com/Karl-Dai/PmuSim/releases)
[![Stars](https://img.shields.io/github/stars/Karl-Dai/PmuSim?color=e3b341)](https://github.com/Karl-Dai/PmuSim/stargazers)
[![License: MIT](https://img.shields.io/badge/License-MIT-lightgrey.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20·%20macOS%20·%20Linux-informational)]()

基于 **Rust** · **Tauri 2** · **Vue 3**

[English](README.md) · **中文**

![PmuSim 主窗口](docs/screenshots/main.png)

</div>

---

## 这是个什么项目

测试 PMU 主站通常两种痛苦: 借一台真实子站, 或者跑一段半坏的 Python 脚本, 而且常常只支持一个版本的规约。PmuSim 把完整的主站装进桌面端:

- 📡 **两个规约版本共用一个二进制** — Q/GDW 131-2006 (V2) 与 GB/T 26865.2-2011 (V3), 帧格式与端口差异都已对齐。
- 🤝 **TCP 角色正确** — 管理通道主站作 client; V3 数据通道主站也作 client (按规约), V2 数据通道主站作 server。不用再在每次 review 里解释一遍方向。
- ⚡ **一键握手** — `请求 CFG-1 → 发 CFG-2 命令 → 发 CFG-2 → 请求 CFG-2 → 启动数据` 全流程自动化。
- 🔄 **应用内自动更新** — ed25519 签名安装包, 4 路 endpoint 回退 (国内 3 个镜像 + GitHub), 国内用户也能拿到更新。
- 🪶 **小体积原生** — Rust + Tauri 2; 没 JVM, 没 Python runtime, 没 Electron。

## 目录

- [截图](#截图)
- [功能](#功能)
- [下载](#下载)
- [快速上手](#快速上手)
- [从源码构建](#从源码构建)
- [规约支持](#规约支持)
- [项目结构](#项目结构)
- [更新日志](#更新日志)
- [macOS 首次启动](#macos-首次启动)
- [许可证](#许可证)

## 截图

**主窗口 · `simpmufep` 风格的单子站布局**

左侧是连接表单 + 事件日志, 右侧是数据表格, CFG-2 通道名映射到每一行。标题栏的「检查更新」按钮挂接到应用内更新器。

![PmuSim 主窗口](docs/screenshots/main.png)

## 功能

### 📡 规约

- **V2 (Q/GDW 131-2006)** — 2 字节 IDCODE, 帧头顺序 `SYNC-SIZE-SOC-IDCODE`, 4-bit 时间质量。
- **V3 (GB/T 26865.2-2011)** — 8 字节 ASCII IDCODE, 帧头顺序 `SYNC-SIZE-IDCODE-SOC`, 8-bit 时间质量, 数据帧带 IDCODE。
- **CRC-CCITT** (`poly=0x1021`, `init=0x0000`), V2 / V3 全部有 round-trip 测试。
- **GBK 通道名**通过 `encoding_rs` 解码 — 中文站名能直接落到表格里, 不乱码。

### 🌐 网络

- **方向按规约正确** — V2 数据通道主站是 TCP 服务端; V3 数据通道主站是 TCP 客户端 (主站外联)。UI 会根据当前规约隐藏无关字段。
- **管理 / 数据端口独立** — 端口字段拆分, 带自动跟随 + dirty tracking, 手动改一个端口不会偷偷把另一个改回去。
- **完整握手流程** — `请求 CFG-1 → 发 CFG-2 命令 → 发 CFG-2 → 请求 CFG-2 → 启动数据`, 也提供一键 "auto handshake" 按钮。
- **心跳** — 间隔可配, 改完不用重连; 按住 ↑/↓ 时做了 debounce, 不会一次按键打两个命令。

### 🖥️ 界面

- `simpmufep` 风格的单子站布局 (Vue 3 + TypeScript + Vite, ~95 KB gzipped)。
- 实时事件日志, 支持复制 / 清空, hex 按需展开。
- 实时数据表带 CFG-2 通道名 + 模拟量 scale factor + 数字量掩码标签。
- 错误 toast 双语 (UI 中文, 错误原文保留 `PmuError` 上游字符串)。

### 🔄 更新

- **启动静默自检**, 6 小时内最多一次。
- **手动「检查更新」**按钮绕开节流与 snooze。
- **同版本 24 小时 snooze** — 用户点了「稍后」就 24h 不再弹。
- **ed25519 签名**, `tauri-plugin-updater` 验签后才安装。
- **4 路 endpoint 回退**: `ghfast.top` / `gh-proxy.com` / `gh.idayer.com` → GitHub。

## 下载

预编译安装包在 **[Releases 页面](https://github.com/Karl-Dai/PmuSim/releases)**, 每个文件都做了 minisign 签名, 应用内更新器会验签后才安装。

| 平台    | 安装包                                                                              |
|---------|-------------------------------------------------------------------------------------|
| Windows | x64: `PmuSim_<ver>_x64-setup.exe` (NSIS) · `PmuSim_<ver>_x64_en-US.msi` — ARM64: `PmuSim_<ver>_arm64-setup.exe` (NSIS) |
| macOS   | `PmuSim_<ver>_aarch64.dmg` (Apple Silicon) · `PmuSim_<ver>_x64.dmg` (Intel)        |
| Linux   | `PmuSim_<ver>_amd64.AppImage` · `PmuSim_<ver>_amd64.deb` · `PmuSim-<ver>-1.x86_64.rpm` |

v0.3.0 起支持应用内自动更新。0.1.0 / 0.2.0 的用户需要先手动装一次 0.3.0+, 之后自动更新接管。macOS 首次启动需要[一步操作](#macos-首次启动)。

### 国内镜像

国内用户访问 GitHub Releases 可能不稳, 直接下载推荐走镜像:

- <https://ghfast.top/https://github.com/Karl-Dai/PmuSim/releases/latest>

v0.3.0 起, 应用内更新器自动在多个 proxy 间回退, 无需手动操作。但 **从 v0.2.0 及之前首次升级**, 用的是旧二进制里的 endpoint (那时候根本没有 updater); 请先通过上面的镜像装一次 v0.3.0, 之后更新器就会走 proxy 了。

## 快速上手

1. **启动 PmuSim**, 选规约 (V2 / V3)。默认 `10.15.48.12 : 8000` (V3 管理端口) 可改。
2. 点 **开始** — V2 主站开始监听, V3 主站待命; 然后点 **连接** 与子站握手。
3. CFG-1 / CFG-2 往返后 IDCODE 落入只读字段; 数据表在 *Open Data* 成功后开始填充。
4. 用 **暂停 / 触发** 中止/恢复数据流, 或单发一次 trigger 帧。

## 从源码构建

### 前置依赖

- [Rust](https://rustup.rs/) 1.77+
- [Node.js](https://nodejs.org/) 18+
- [Tauri CLI](https://tauri.app/) — `cargo install tauri-cli --version '^2'`
- 系统依赖: 见 [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/)

### 步骤

```bash
# 一次性: 安装前端依赖
cd frontend && npm install

# dev 模式
cd ../crates/pmusim-app && cargo tauri dev

# 生产构建 (Tauri 自动触发前端构建)
cargo tauri build
```

`cargo test --workspace` 跑核心协议测试 (帧解析、CRC、时间工具 round-trip)。

## 规约支持

### 帧类型

| SYNC   | 帧类型 | 方向                            |
|--------|--------|---------------------------------|
| 0xAA0x | 数据帧 | 子站 → 主站 (数据通道)         |
| 0xAA2x | CFG-1  | 子站 → 主站 (管理通道)         |
| 0xAA3x | CFG-2  | 双向 (管理通道)                 |
| 0xAA4x | 命令帧 | 主站 → 子站 (管理通道)         |

### 命令

| 代码    | 命令             | 说明                                  |
|---------|------------------|---------------------------------------|
| 0x0001  | 关数据           | 停止实时数据流                        |
| 0x0002  | 开数据           | 启动实时数据流                        |
| 0x0004  | 请求 CFG-1       | 请求配置帧 1                          |
| 0x0005  | 请求 CFG-2       | 请求配置帧 2                          |
| 0x4000  | 心跳             | 保活心跳                              |
| 0x8000  | 发 CFG-2 通知    | 通知子站即将下发 CFG-2               |

### 通道方向

| 通道   | 主站 TCP 角色 (V2) | 主站 TCP 角色 (V3) | V2 端口 | V3 端口 |
|--------|--------------------|--------------------|---------|---------|
| 管理   | client             | client             | 7000    | 8000    |
| 数据   | server             | client (外联)      | 7001    | 8001    |

### V2 vs V3 差异

| 特性               | V2 (2006)            | V3 (2011)              |
|--------------------|----------------------|------------------------|
| 管理端口           | 7000                 | 8000                   |
| 数据端口           | 7001                 | 8001                   |
| IDCODE 长度        | 2 字节               | 8 字节 (ASCII)         |
| 帧头字段顺序       | SYNC-SIZE-SOC-IDCODE | SYNC-SIZE-IDCODE-SOC   |
| 数据帧带 IDCODE    | 否                   | 是                     |
| 时间质量           | 4-bit                | 8-bit                  |
| 数据通道主站角色   | server               | client                 |

## 项目结构

```
PmuSim/
├── crates/
│   ├── pmusim-core/      # 协议库 (无 Tauri 依赖)
│   └── pmusim-app/       # Tauri 桌面应用
├── frontend/             # Vue 3 + TypeScript SPA
├── scripts/              # release 脚本 (updater manifest、release notes)
└── .github/workflows/    # CI: release.yml (签名 + 发布)
```

| 层       | 技术栈                                                       |
|----------|--------------------------------------------------------------|
| 后端     | Rust, [tokio](https://tokio.rs/) (async TCP), `encoding_rs` (GBK) |
| 前端     | Vue 3, TypeScript, Vite                                       |
| 桌面层   | [Tauri 2](https://tauri.app/) + `tauri-plugin-updater`        |

## 更新日志

完整历史见 [CHANGELOG.md](CHANGELOG.md), 签名安装包和 updater manifest 见 [Releases 页面](https://github.com/Karl-Dai/PmuSim/releases)。

应用内自动更新从 v0.3.0 起启用。v0.1.0 / v0.2.0 的用户需要先手动装一次 v0.3.0+, 之后更新器接管。

## macOS 首次启动

安装包**没经过 Apple 公证** (没买 Developer Program)。首次启动 macOS 会弹「PmuSim 无法打开 — Apple 无法验证…」, 只有「完成」和「移到废纸篓」两个按钮。这是 macOS 15 (Sequoia) 对 ad-hoc 签名应用的常规拦截 — 应用**没有损坏**。

<details>
<summary><b>放行方法 (二选一)</b></summary>

**1. GUI 路径**

- 双击 `.app`, 看到拦截对话框, 点「完成」。
- 打开*系统设置 → 隐私与安全性*, 滚到最下面。
- 会看到「PmuSim 已被阻止…」 — 点「仍要打开」, 输入密码。
- 弹出的对话框里有「打开」按钮, 点一下。之后启动就不会再被拦截。

**2. 终端一行命令**

```bash
xattr -dr com.apple.quarantine "/Applications/PmuSim.app"
```

清掉 quarantine 标记, macOS 就不再拦截了。

</details>

## 许可证

[MIT](LICENSE)
