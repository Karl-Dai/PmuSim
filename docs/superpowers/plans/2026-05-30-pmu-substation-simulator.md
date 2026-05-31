# PMU 子站（数据发送方）模拟器 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 新增独立 Tauri App `pmusim-sub`，模拟 PMU 子站（数据发送方），与现有 PmuSim 主站对标，支持 V2/V3 双协议、可配置正弦相量数据、命令响应全握手。

**Architecture:** 复用 `pmusim-core`（`build_config`/`build_data`/`parse` 协议对称）。新增 `crates/pmusim-sub`：纯逻辑 `datagen`（正弦相量→DataFrame，可单测）+ `network/substation`（反向 TCP 角色 + 命令响应状态机，镜像 `pmusim-app/src/network/master.rs`）+ Tauri 壳（`poll_events` 缓冲事件模型）。前端 `frontend-sub/`（Vue3+Vite）复用主站 i18n/composables/样式。

**Tech Stack:** Rust · tokio · Tauri 2 · Vue 3 · TypeScript · Vite。

**关键参考文件（实现时对照）：**
- 协议：`crates/pmusim-core/src/protocol/{builder,parser,frame,constants}.rs`、`crates/pmusim-core/src/time_utils.rs`
- 网络层模板：`crates/pmusim-app/src/network/master.rs`
- Tauri 壳模板：`crates/pmusim-app/src/{commands,events,state,lib,main}.rs`、`tauri.conf.json`、`build.rs`、`capabilities/default.json`
- 集成测试模板（内含 mock 子站，与本实现高度同构）：`crates/pmusim-app/tests/e2e.rs`
- 前端模板：`frontend/`（`package.json`、`vite.config.ts`、`src/main.ts`、`src/i18n/`、`src/composables/`）

**对 spec 公式的实现性修正（已在 plan 中采用）：** spec 写的相角公式 `θ(t)=θ0+2π·(fnom+Δf)·t+π·ROCOF·t²` 会让相量以整 50Hz 旋转，在 25~100fps 采样率下每帧转过多圈、产生混叠，主站看到的相量像随机跳。同步相量约定里，相量在「以标称频率旋转的参考系」中只按**频率偏差 Δf** 旋转：`θ(t)=θ0+2π·(Δf·t+½·ROCOF·t²)`。本 plan 采用后者——Δf=0 时相量静止，Δf=0.1Hz 时缓慢旋转，主站显示直观。FREQ 字段按 mHz 偏差上报（`round(Δf·1000)`），DFREQ 按 `round(ROCOF·100)`。

---

## File Structure

新增/修改文件一览：

**`crates/pmusim-core/`（仅小幅补充，不重写）**
- Modify: `src/time_utils.rs` — 新增 `fracsec_from_fraction`（`fracsec_to_ms` 的逆）。

**`crates/pmusim-sub/`（新建 crate）**
- Create: `Cargo.toml`、`build.rs`、`tauri.conf.json`、`capabilities/default.json`、`icons/`（从 pmusim-app 复制）
- Create: `src/lib.rs`、`src/main.rs`
- Create: `src/datagen.rs` — 纯逻辑：正弦相量 → `DataFrame`
- Create: `src/events.rs` — `SubEvent` 枚举 + `ConfigInfo`/`DataInfo`
- Create: `src/state.rs` — `EventBuffer` + `AppState`
- Create: `src/commands.rs` — Tauri 命令
- Create: `src/network/mod.rs`、`src/network/substation.rs`
- Create: `tests/e2e.rs` — 用真实 `MasterStation` 驱动真实 `SubStation`（V2/V3）

**`frontend-sub/`（新建前端）**
- Create: `package.json`、`vite.config.ts`、`tsconfig*.json`、`index.html`、`src/main.ts`、`src/App.vue`
- Create: `src/types/index.ts`、`src/composables/useSubEvents.ts`、`src/components/{ConfigFormPanel,DataGenPanel,StatusLogPanel,SentDataPanel}.vue`
- Copy-adapt: `src/i18n/`、`src/composables/{useToast,useEventLog,useFrameRate}.ts`

**根目录**
- Modify: `Cargo.toml` — workspace members 增加 `crates/pmusim-sub`
- Modify: `.gitignore` — 增加 `frontend-sub/dist`、`frontend-sub/node_modules`
- Modify: `README.md` / `README_CN.md` / `CHANGELOG.md` — 子站说明

---

## Phase 1 — 核心：FRACSEC 编码器

### Task 1: `time_utils::fracsec_from_fraction`

**Files:**
- Modify: `crates/pmusim-core/src/time_utils.rs`
- Test: 同文件 `#[cfg(test)] mod tests`

- [ ] **Step 1: 写失败测试**（追加到 `time_utils.rs` 的 `mod tests` 内）

```rust
    #[test]
    fn fracsec_from_fraction_roundtrips_v2() {
        // 0.89s @ TIME_BASE=1_000_000 → count 890000；再过 fracsec_to_ms 还原
        let f = fracsec_from_fraction(0.89, 1_000_000, 2, 0);
        assert_eq!(f, 890_000);
        let ms = fracsec_to_ms(f, 1_000_000, 2);
        assert!((ms - 890.0).abs() < 0.1, "got {ms}");
    }

    #[test]
    fn fracsec_from_fraction_v3_packs_quality() {
        // V3：低 24 位是计数，bit27-24 是时间质量
        let f = fracsec_from_fraction(0.5, 1_000_000, 3, 0b1001);
        assert_eq!(f & 0x00FF_FFFF, 500_000);
        assert_eq!((f >> 24) & 0x0F, 0b1001);
        let ms = fracsec_to_ms(f, 1_000_000, 3);
        assert!((ms - 500.0).abs() < 0.1, "got {ms}");
    }

    #[test]
    fn fracsec_from_fraction_zero_rate() {
        assert_eq!(fracsec_from_fraction(0.5, 0, 3, 0), 0);
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p pmusim-core fracsec_from_fraction`
Expected: 编译失败 `cannot find function fracsec_from_fraction`

- [ ] **Step 3: 实现**（追加到 `time_utils.rs`，放在 `current_soc` 之后、`mod tests` 之前）

```rust
/// Encode a sub-second fraction (`0.0..1.0`) into a FRACSEC word — the
/// inverse of [`fracsec_to_ms`]. `count = round(fraction * meas_rate)`.
/// For V3 (`version >= 3`) the low 24 bits hold the count and bits 27-24
/// carry the time-quality nibble (§8.11 表 4, 0 = clock locked); V2 has
/// no quality bits so `time_quality` is ignored. Returns 0 when
/// `meas_rate == 0` (mirrors `fracsec_to_ms`'s guard).
pub fn fracsec_from_fraction(fraction: f64, meas_rate: u32, version: u8, time_quality: u8) -> u32 {
    if meas_rate == 0 {
        return 0;
    }
    let count = (fraction.clamp(0.0, 1.0) * meas_rate as f64).round() as u32;
    if version >= 3 {
        (count & 0x00FF_FFFF) | (((time_quality & 0x0F) as u32) << 24)
    } else {
        count
    }
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p pmusim-core fracsec_from_fraction`
Expected: 3 passed

- [ ] **Step 5: 提交**

```bash
git add crates/pmusim-core/src/time_utils.rs
git commit -m "feat(core): fracsec_from_fraction 编码子秒偏移(数据帧时标)"
```

---

## Phase 2 — 子站 crate 骨架 + 数据生成器

### Task 2: 新建 `pmusim-sub` crate 骨架（lib 优先，bin 暂为 stub）

> 说明：Tauri 的 `generate_context!` 在编译期要求 `frontendDist` 目录存在；为让本阶段到 Phase 4 的 `cargo test -p pmusim-sub` 不依赖前端产物，**先把 `main.rs` 写成空 stub**，真正的 Tauri bin（`generate_context!`、`build.rs`、`tauri.conf.json`）留到 Phase 5。库部分（含 `#[tauri::command]`）可独立编译。

**Files:**
- Create: `crates/pmusim-sub/Cargo.toml`
- Create: `crates/pmusim-sub/src/lib.rs`
- Create: `crates/pmusim-sub/src/main.rs`
- Modify: `Cargo.toml`（根，workspace members）

- [ ] **Step 1: 注册 workspace 成员**

把根 `Cargo.toml` 改为：

```toml
[workspace]
members = ["crates/pmusim-core", "crates/pmusim-app", "crates/pmusim-sub"]
resolver = "2"
```

- [ ] **Step 2: 写 `crates/pmusim-sub/Cargo.toml`**

```toml
[package]
name = "pmusim-sub"
version = "0.1.0"
edition = "2021"

[lib]
name = "pmusim_sub"
crate-type = ["staticlib", "cdylib", "rlib"]

[dependencies]
pmusim-core = { path = "../pmusim-core" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
tauri = { version = "2", features = [] }
tauri-plugin-log = "2"
tauri-plugin-store = "2"
tauri-plugin-dialog = "2"
tokio = { version = "1", features = ["full"] }

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dev-dependencies]
pmusim-app = { path = "../pmusim-app" }
hex = "0.4"
```

- [ ] **Step 3: 写 `crates/pmusim-sub/src/lib.rs`**

```rust
pub mod datagen;
```

- [ ] **Step 4: 写 `crates/pmusim-sub/src/main.rs`（stub，Phase 5 替换）**

```rust
// Placeholder binary. Real Tauri entry (generate_context!) lands in Phase 5
// once frontend-sub/dist exists. Until then this keeps `cargo build` green
// without requiring a frontend build.
fn main() {}
```

- [ ] **Step 5: 建空 `datagen.rs` 占位**（下个 Task 填充，先让 lib 能编译）

Create `crates/pmusim-sub/src/datagen.rs`:

```rust
// Filled in Task 3.
```

- [ ] **Step 6: 编译确认**

Run: `cargo build -p pmusim-sub`
Expected: 编译通过（bin 是空 main，lib 仅空模块）。`build.rs` 尚未加，`tauri-build` 不会运行——OK。

> 注意：`Cargo.toml` 声明了 `[build-dependencies] tauri-build` 但还没有 `build.rs` 文件，Cargo 会忽略未使用的 build-dep（不会报错）。若实现者偏好，可暂时删掉 `[build-dependencies]` 段，到 Phase 5 再加。

- [ ] **Step 7: 提交**

```bash
git add Cargo.toml crates/pmusim-sub/Cargo.toml crates/pmusim-sub/src/lib.rs crates/pmusim-sub/src/main.rs crates/pmusim-sub/src/datagen.rs
git commit -m "chore(sub): pmusim-sub crate 骨架(lib 优先, bin stub)"
```

### Task 3: `datagen.rs` — 正弦相量数据生成器（纯逻辑）

**Files:**
- Modify: `crates/pmusim-sub/src/datagen.rs`
- Test: 同文件 `#[cfg(test)] mod tests`

- [ ] **Step 1: 写失败测试**（`datagen.rs` 末尾）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pmusim_core::protocol::constants::ProtocolVersion;
    use pmusim_core::protocol::frame::Frame;
    use pmusim_core::protocol::parser::parse;

    fn cfg() -> SubConfig {
        SubConfig {
            version: ProtocolVersion::V3,
            idcode: "TESTPMU0".into(),
            stn: "T".into(),
            data_rate_fps: 50,
            meas_rate: 1_000_000,
            format_flags: 0, // 直角坐标 int16
            phasors: vec![PhasorGen { magnitude: 1000.0, phase_deg: 0.0 }],
            analogs: vec![300.0, 3000.0],
            digitals: vec![0x000A],
        }
    }

    #[test]
    fn stationary_when_no_offset() {
        let g = DataGen { freq_offset_hz: 0.0, rocof_hz_s: 0.0 };
        let c = cfg();
        // Δf=0 → 相量不旋转：第 0 帧与第 50 帧相量一致
        let f0 = next_data_frame(&c, &g, 1000, 0, false);
        let f50 = next_data_frame(&c, &g, 1000, 50, false);
        assert_eq!(f0.phasors[0].0.round(), 1000.0); // real≈mag
        assert_eq!(f0.phasors[0].1.round(), 0.0);     // imag≈0
        assert_eq!(f50.phasors[0].0.round(), 1000.0);
    }

    #[test]
    fn rotates_with_offset() {
        // Δf=0.25Hz, fps=50 → 每帧转 2π·0.25/50 rad；第 50 帧(=1s)转过 2π·0.25=90°
        let g = DataGen { freq_offset_hz: 0.25, rocof_hz_s: 0.0 };
        let c = cfg();
        let f = next_data_frame(&c, &g, 0, 50, false);
        // 90°：real≈0, imag≈+1000
        assert!(f.phasors[0].0.abs() < 1.0, "real={}", f.phasors[0].0);
        assert!((f.phasors[0].1 - 1000.0).abs() < 1.0, "imag={}", f.phasors[0].1);
    }

    #[test]
    fn soc_and_fracsec_advance() {
        let g = DataGen { freq_offset_hz: 0.0, rocof_hz_s: 0.0 };
        let c = cfg(); // fps=50
        // 第 75 帧 = 1 整秒 + 25/50 秒
        let f = next_data_frame(&c, &g, 1000, 75, false);
        assert_eq!(f.soc, 1001);
        // fraction=0.5 → count=500000
        assert_eq!(f.fracsec & 0x00FF_FFFF, 500_000);
    }

    #[test]
    fn frame_builds_and_parses() {
        let g = DataGen { freq_offset_hz: 0.1, rocof_hz_s: 0.0 };
        let c = cfg();
        let df = next_data_frame(&c, &g, 1000, 3, false);
        let bytes = pmusim_core::protocol::builder::build_data(&df, 0, 0, 0).unwrap();
        // 用 CFG-2 维度解析：phnmr=1, annmr=2, dgnmr=1
        let parsed = parse(&bytes, c.format_flags, 1, 2, 1).unwrap();
        match parsed {
            Frame::Data(d) => {
                assert_eq!(d.idcode, "TESTPMU0");
                assert_eq!(d.analog.len(), 2);
                assert_eq!(d.digital, vec![0x000A]);
                assert_eq!(d.phasors.len(), 1);
            }
            _ => panic!("expected Data frame"),
        }
    }

    #[test]
    fn trigger_sets_stat_bit() {
        let g = DataGen { freq_offset_hz: 0.0, rocof_hz_s: 0.0 };
        let c = cfg();
        let normal = next_data_frame(&c, &g, 0, 0, false);
        let trig = next_data_frame(&c, &g, 0, 0, true);
        assert_eq!(normal.stat & TRIGGER_STAT_BIT, 0);
        assert_eq!(trig.stat & TRIGGER_STAT_BIT, TRIGGER_STAT_BIT);
        // 不得置「数据无效(0x8000)」或「失步(0x2000)」位
        assert_eq!(trig.stat & 0x8000, 0);
        assert_eq!(trig.stat & 0x2000, 0);
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p pmusim-sub datagen`
Expected: 编译失败（`SubConfig`/`PhasorGen`/`DataGen`/`next_data_frame`/`TRIGGER_STAT_BIT` 未定义）

- [ ] **Step 3: 实现**（替换 `datagen.rs` 顶部占位为完整实现，置于 `mod tests` 之前）

```rust
//! 子站数据生成器：把可配置正弦相量 + 静态模拟量/数字量编成一个
//! `DataFrame`。纯逻辑、无 IO、无 Tauri —— 由 `network::substation` 的
//! 推流循环按帧调用，也可独立单测。
//!
//! 同步相量约定：相量在「以标称频率旋转的参考系」中只按频率偏差 Δf
//! 旋转，θ(t)=θ0+2π·(Δf·t+½·ROCOF·t²)。Δf=0 时相量静止。

use std::f64::consts::PI;

use pmusim_core::protocol::constants::ProtocolVersion;
use pmusim_core::protocol::frame::DataFrame;
use pmusim_core::time_utils::fracsec_from_fraction;

/// 触发帧在 STAT 里置的标记位。避开主站会检查的 0x8000(数据无效)、
/// 0x2000(失步)、0x0400(配置变更) 位，选 bit11(0x0800) 作触发指示。
pub const TRIGGER_STAT_BIT: u16 = 0x0800;

/// 单个相量通道的生成参数（初始幅值/相角）。
#[derive(Debug, Clone)]
pub struct PhasorGen {
    /// 幅值（整数刻度，直接作为 int16 直角坐标的模；典型 0~32767）。
    pub magnitude: f64,
    /// 初始相角（度）。
    pub phase_deg: f64,
}

/// 子站静态配置 —— 与 CFG-2 申报保持一致的通道布局 + 速率 + 格式。
#[derive(Debug, Clone)]
pub struct SubConfig {
    pub version: ProtocolVersion,
    pub idcode: String,
    pub stn: String,
    /// 帧率（帧/秒），由 CFG-2 period/fnom 推得（见 network 层）。
    pub data_rate_fps: u32,
    /// TIME_BASE（FRACSEC 分辨率，典型 1_000_000）。
    pub meas_rate: u32,
    /// FORMAT bits 0-3。本工具默认 0 = 直角坐标 int16。
    pub format_flags: u16,
    pub phasors: Vec<PhasorGen>,
    /// 模拟量定值（个数 = ANNMR）。
    pub analogs: Vec<f64>,
    /// 数字量定值（个数 = DGNMR，每个是 16 位掩码）。
    pub digitals: Vec<u16>,
}

/// 运行期可调的频率行为。
#[derive(Debug, Clone, Copy)]
pub struct DataGen {
    /// 频率偏差 Δf（Hz）。
    pub freq_offset_hz: f64,
    /// 频率变化率 ROCOF（Hz/s）。
    pub rocof_hz_s: f64,
}

/// 生成第 `frame_index` 帧（从 0 计）。`base_soc` 是推流开始时刻的 SOC。
pub fn next_data_frame(
    cfg: &SubConfig,
    gen: &DataGen,
    base_soc: u32,
    frame_index: u64,
    trigger: bool,
) -> DataFrame {
    let fps = cfg.data_rate_fps.max(1) as u64;
    let whole = (frame_index / fps) as u32;
    let sub = (frame_index % fps) as f64 / fps as f64;
    let soc = base_soc.wrapping_add(whole);
    let version_u8 = cfg.version as u8;
    let fracsec = fracsec_from_fraction(sub, cfg.meas_rate, version_u8, 0);

    // 自推流起的连续时间 t（秒），用于相量旋转。
    let t = frame_index as f64 / fps as f64;
    let two_pi = 2.0 * PI;
    let rot = two_pi * (gen.freq_offset_hz * t + 0.5 * gen.rocof_hz_s * t * t);

    let phasors: Vec<(f64, f64)> = cfg
        .phasors
        .iter()
        .map(|p| {
            let theta = p.phase_deg.to_radians() + rot;
            // 直角坐标：(real, imag) = (mag·cosθ, mag·sinθ)。build_data 在
            // format bit1=0 时按 i16 截断。
            (p.magnitude * theta.cos(), p.magnitude * theta.sin())
        })
        .collect();

    let stat = if trigger { TRIGGER_STAT_BIT } else { 0x0000 };

    DataFrame {
        version: cfg.version,
        idcode: cfg.idcode.clone(),
        soc,
        fracsec,
        stat,
        format_flags: cfg.format_flags,
        phasors,
        // FREQ 按 mHz 偏差上报，DFREQ 按 ROCOF·100；format bit3=0 时写 i16。
        freq: (gen.freq_offset_hz * 1000.0).round(),
        dfreq: (gen.rocof_hz_s * 100.0).round(),
        analog: cfg.analogs.clone(),
        digital: cfg.digitals.clone(),
    }
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p pmusim-sub datagen`
Expected: 5 passed

- [ ] **Step 5: 提交**

```bash
git add crates/pmusim-sub/src/datagen.rs
git commit -m "feat(sub): datagen 正弦相量数据生成器(纯逻辑+单测)"
```

---

## Phase 3 — 子站网络层

### Task 4: `events.rs` — `SubEvent` 与信息结构

**Files:**
- Create: `crates/pmusim-sub/src/events.rs`
- Modify: `crates/pmusim-sub/src/lib.rs`

- [ ] **Step 1: 写 `events.rs`**（对照 `pmusim-app/src/events.rs` 的 camelCase 约定）

```rust
use serde::Serialize;

/// 子站事件，经 `poll_events` 推给前端（与主站一致的缓冲轮询模型）。
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum SubEvent {
    /// 子站已在管理端口开始监听。
    Listening { mgmt_port: u16, data_port: u16 },
    /// 主站建立了管理连接。
    MasterConnected { peer_ip: String },
    /// 主站断开。
    MasterDisconnected { peer_ip: String },
    /// 收到主站命令（cmd 为命令字）。
    CommandReceived { cmd: u16, name: String },
    /// 已上传 CFG-1 / CFG-2。
    Cfg1Sent,
    Cfg2Sent,
    /// 收到主站下传的 CFG-2 配置帧。
    Cfg2Received,
    /// 数据推流开始/停止。
    StreamingStarted,
    StreamingStopped,
    /// 已发出一帧数据（携带预览信息）。
    DataFrameSent { data: DataInfo },
    /// 任意方向的原始帧（hex 按需）。
    RawFrame { direction: String, hex: String },
    Error { error: String },
}

/// 数据帧预览（驼峰命名以对齐前端 TS 类型）。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataInfo {
    pub soc: u32,
    pub fracsec: u32,
    pub stat: u16,
    pub freq: f64,
    pub dfreq: f64,
    pub phasors: Vec<(f64, f64)>,
    pub analog: Vec<f64>,
    pub digital: Vec<u16>,
}
```

- [ ] **Step 2: 在 `lib.rs` 注册模块**

```rust
pub mod datagen;
pub mod events;
```

- [ ] **Step 3: 编译确认**

Run: `cargo build -p pmusim-sub`
Expected: 通过

- [ ] **Step 4: 提交**

```bash
git add crates/pmusim-sub/src/events.rs crates/pmusim-sub/src/lib.rs
git commit -m "feat(sub): SubEvent 事件类型"
```

### Task 5: `network/substation.rs` 骨架 + 公共 API（含 todo! 体）

> 先确立 `SubStation` 公开接口，使 Task 6 的集成测试能编译；实现留到 Task 7/8。

**Files:**
- Create: `crates/pmusim-sub/src/network/mod.rs`
- Create: `crates/pmusim-sub/src/network/substation.rs`
- Modify: `crates/pmusim-sub/src/lib.rs`

- [ ] **Step 1: `network/mod.rs`**

```rust
pub mod substation;
```

- [ ] **Step 2: `lib.rs` 增加 network**

```rust
pub mod datagen;
pub mod events;
pub mod network;
```

- [ ] **Step 3: `network/substation.rs` 骨架**

```rust
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;

use pmusim_core::protocol::constants::ProtocolVersion;

use crate::datagen::{DataGen, SubConfig};
use crate::events::SubEvent;

pub type EventSender = mpsc::UnboundedSender<SubEvent>;

/// 子站运行时配置：监听端口 + 协议 + 通道/速率（datagen 用） + 主站数据口。
#[derive(Debug, Clone)]
pub struct SubSettings {
    pub version: ProtocolVersion,
    /// 管理端口（子站作服务端监听）。V2 默认 7000，V3 默认 8000。
    pub mgmt_port: u16,
    /// 数据端口。V3：子站监听此口等主站连入；V2：子站作客户端连主站此口。
    pub data_port: u16,
    pub config: SubConfig,
    pub gen: DataGen,
}

pub struct SubStation {
    settings: Arc<RwLock<SubSettings>>,
    /// 运行期可调的频率参数（推流循环每帧读取，无需重启任务）。
    gen: Arc<RwLock<DataGen>>,
    /// 一次性触发标志，被推流循环消费。
    trigger: Arc<std::sync::atomic::AtomicBool>,
    event_tx: EventSender,
    mgmt_port: u16,
    data_port: u16,
    tasks: Vec<JoinHandle<()>>,
    /// 当前数据写入端（V2=连出主站后填入；V3=接受主站连入后填入）。
    data_writer: Arc<Mutex<Option<tokio::net::tcp::OwnedWriteHalf>>>,
    /// 推流任务句柄（OpenData 启动，CloseData 中止）。
    stream_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl SubStation {
    pub fn new(event_tx: EventSender, settings: SubSettings) -> Self {
        let gen = settings.gen;
        let mgmt_port = settings.mgmt_port;
        let data_port = settings.data_port;
        Self {
            settings: Arc::new(RwLock::new(settings)),
            gen: Arc::new(RwLock::new(gen)),
            trigger: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            event_tx,
            mgmt_port,
            data_port,
            tasks: Vec::new(),
            data_writer: Arc::new(Mutex::new(None)),
            stream_task: Arc::new(Mutex::new(None)),
        }
    }

    /// 实际绑定到的管理端口（端口传 0 时由 OS 分配，便于测试）。
    pub fn mgmt_port(&self) -> u16 { self.mgmt_port }
    /// 实际绑定到的数据端口（V3）。
    pub fn data_port(&self) -> u16 { self.data_port }

    /// 绑定监听并启动命令响应循环。
    pub async fn start(&mut self) -> Result<(), String> {
        todo!("Task 7")
    }

    pub async fn stop(&mut self) {
        todo!("Task 7")
    }

    /// 运行期更新通道配置（站名/通道/速率等）。
    pub async fn update_config(&self, config: SubConfig) {
        let mut s = self.settings.write().await;
        s.config = config;
    }

    /// 运行期更新频率参数（Δf/ROCOF），推流循环下一帧生效。
    pub async fn update_gen(&self, gen: DataGen) {
        *self.gen.write().await = gen;
        let mut s = self.settings.write().await;
        s.gen = gen;
    }

    /// 触发一帧带触发标记的数据帧。
    pub fn trigger(&self) {
        self.trigger.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
```

- [ ] **Step 4: 编译确认（含 todo! 警告）**

Run: `cargo build -p pmusim-sub`
Expected: 通过（`todo!()` 仅运行期 panic，编译 OK）

- [ ] **Step 5: 提交**

```bash
git add crates/pmusim-sub/src/network crates/pmusim-sub/src/lib.rs
git commit -m "feat(sub): SubStation 网络层骨架与公共 API"
```

### Task 6: 集成测试（红）—— 真实主站 ↔ 真实子站 V3

**Files:**
- Create: `crates/pmusim-sub/tests/e2e.rs`

> 用 `pmusim-app` 的真实 `MasterStation`（dev-dependency）驱动真实 `SubStation`，验证 spec 成功标准。对照模板：`crates/pmusim-app/tests/e2e.rs`。

- [ ] **Step 1: 写 `tests/e2e.rs`（V3）**

```rust
//! End-to-end: 真实 MasterStation 驱动真实 SubStation，跑完整握手 + 数据流。

use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::timeout;

use pmusim_app::events::PmuEvent;
use pmusim_app::network::master::MasterStation;
use pmusim_core::protocol::constants::ProtocolVersion;

use pmusim_sub::datagen::{DataGen, PhasorGen, SubConfig};
use pmusim_sub::events::SubEvent;
use pmusim_sub::network::substation::{SubSettings, SubStation};

const IDCODE: &str = "TESTPMU0";
const STN: &str = "SubTestStn";

fn sub_config(version: ProtocolVersion) -> SubConfig {
    SubConfig {
        version,
        idcode: IDCODE.into(),
        stn: STN.into(),
        data_rate_fps: 50,
        meas_rate: 1_000_000,
        format_flags: 0,
        phasors: vec![PhasorGen { magnitude: 1000.0, phase_deg: 0.0 }],
        analogs: vec![300.0, 3000.0],
        digitals: vec![0x000A],
    }
}

/// 在相邻端口 (p, p+1) 上启动一个子站，返回 (子站, mgmt_port)。
async fn spawn_substation(
    version: ProtocolVersion,
) -> (SubStation, mpsc::UnboundedReceiver<SubEvent>, u16) {
    let (tx, rx) = mpsc::unbounded_channel::<SubEvent>();
    // 端口 0：start() 内部用 OS 分配的 mgmt 口，并取 mgmt+1 作 data 口。
    let settings = SubSettings {
        version,
        mgmt_port: 0,
        data_port: 0,
        config: sub_config(version),
        gen: DataGen { freq_offset_hz: 0.1, rocof_hz_s: 0.0 },
    };
    let mut sub = SubStation::new(tx, settings);
    sub.start().await.expect("substation start");
    let port = sub.mgmt_port();
    (sub, rx, port)
}

async fn wait_master_event<F: FnMut(&PmuEvent) -> bool>(
    rx: &mut mpsc::UnboundedReceiver<PmuEvent>,
    mut pred: F,
) -> PmuEvent {
    loop {
        let ev = timeout(Duration::from_secs(8), rx.recv())
            .await
            .expect("master event timeout")
            .expect("master channel closed");
        if pred(&ev) {
            return ev;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn v3_master_drives_substation_to_streaming() {
    let (mut sub, _sub_rx, mgmt_port) = spawn_substation(ProtocolVersion::V3).await;

    let (m_tx, mut m_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(m_tx, 0, 30.0, ProtocolVersion::V3);
    master.start().await.unwrap();

    master
        .connect_to_substation("127.0.0.1".into(), mgmt_port, 0, ProtocolVersion::V3)
        .await
        .unwrap();

    let tmp = match wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::SessionCreated { .. })).await {
        PmuEvent::SessionCreated { idcode, .. } => idcode,
        _ => unreachable!(),
    };

    // 自动握手：Request CFG-1 → SendCfg2Cmd(ACK) → 下传 CFG-2(ACK) → 召唤 CFG-2 → OpenData
    master.auto_handshake(tmp, Some(100)).await.unwrap();

    // 子站申报的站名/通道应出现在主站 CFG-1
    let cfg1 = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::Cfg1Received { .. })).await;
    if let PmuEvent::Cfg1Received { idcode, cfg } = cfg1 {
        assert_eq!(idcode, IDCODE);
        assert_eq!(cfg.stn, STN);
        assert_eq!(cfg.annmr, 2);
        assert_eq!(cfg.dgnmr, 1);
    }

    let _ = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::StreamingStarted { .. })).await;

    // 数据帧：相量随时间旋转(Δf=0.1Hz)，模拟量定值 300/3000，数字量 0x000A
    let data = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::DataFrame { .. })).await;
    if let PmuEvent::DataFrame { idcode, data } = data {
        assert_eq!(idcode, IDCODE);
        assert_eq!(data.analog.len(), 2);
        assert_eq!(data.digital, vec![0x000A]);
        assert_eq!(data.phasors.len(), 1);
    }

    master.stop().await;
    sub.stop().await;
}
```

- [ ] **Step 2: 跑测试确认失败（红）**

Run: `cargo test -p pmusim-sub --test e2e v3_master_drives_substation_to_streaming`
Expected: 运行期 panic `not yet implemented`（来自 `start()` 的 `todo!`）

- [ ] **Step 3: 提交（红测试入库）**

```bash
git add crates/pmusim-sub/tests/e2e.rs
git commit -m "test(sub): V3 主站↔子站 e2e(红,待实现)"
```

### Task 7: 实现 `SubStation`（V3 路径转绿）

**Files:**
- Modify: `crates/pmusim-sub/src/network/substation.rs`

> 实现要点对照 `pmusim-app/tests/e2e.rs` 的 mock 子站（命令分派完全一致）+ `master.rs` 的 `read_frame`/`hex_encode`/`emit_event` 自由函数。

- [ ] **Step 1: 顶部 imports 补齐**

把 `substation.rs` 顶部 import 段替换为：

```rust
use std::sync::atomic::Ordering;
use std::sync::Arc;

use log::{error, info, warn};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;

use pmusim_core::protocol::builder::{build_command, build_config, build_data};
use pmusim_core::protocol::constants::{Cmd, FrameType, ProtocolVersion, SYNC_BYTE};
use pmusim_core::protocol::frame::{CommandFrame, ConfigFrame, Frame, PmuBlock};
use pmusim_core::protocol::parser::parse;
use pmusim_core::time_utils::current_soc;

use crate::datagen::{next_data_frame, DataGen, SubConfig};
use crate::events::{DataInfo, SubEvent};
```

- [ ] **Step 2: 实现 `start()`（替换 `todo!("Task 7")`）**

```rust
    pub async fn start(&mut self) -> Result<(), String> {
        // 管理管道：子站永远是服务端。
        let mgmt_listener = TcpListener::bind(("0.0.0.0", self.mgmt_port))
            .await
            .map_err(|e| format!("绑定管理端口 {} 失败: {e}", self.mgmt_port))?;
        self.mgmt_port = mgmt_listener.local_addr().map(|a| a.port()).unwrap_or(self.mgmt_port);

        let version = { self.settings.read().await.version };

        // V3：数据管道子站作服务端，开机即监听等主站连入。
        // V2：数据管道子站作客户端，OpenData 时再连出，这里不绑定。
        let data_listener = if version == ProtocolVersion::V3 {
            let want = if self.data_port == 0 { self.mgmt_port + 1 } else { self.data_port };
            let l = TcpListener::bind(("0.0.0.0", want))
                .await
                .map_err(|e| format!("绑定数据端口 {want} 失败: {e}"))?;
            self.data_port = l.local_addr().map(|a| a.port()).unwrap_or(want);
            Some(l)
        } else {
            self.data_port = 0;
            None
        };

        emit_event(&self.event_tx, SubEvent::Listening {
            mgmt_port: self.mgmt_port,
            data_port: self.data_port,
        });
        info!("SubStation listening: mgmt={} data={}", self.mgmt_port, self.data_port);

        // V3 数据 accept 任务：把主站连入的写半填进 data_writer。
        if let Some(listener) = data_listener {
            let dw = self.data_writer.clone();
            let evt = self.event_tx.clone();
            self.tasks.push(tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((stream, _addr)) => {
                            let (_r, w) = stream.into_split();
                            *dw.lock().await = Some(w);
                            info!("V3 数据管道:主站已连入");
                        }
                        Err(e) => {
                            warn!("数据监听 accept 出错: {e}");
                            let _ = evt; // 仅日志
                            break;
                        }
                    }
                }
            }));
        }

        // 管理 accept 循环：每个主站连接起一个命令响应任务。
        let settings = self.settings.clone();
        let gen = self.gen.clone();
        let trigger = self.trigger.clone();
        let evt = self.event_tx.clone();
        let dw = self.data_writer.clone();
        let stream_task = self.stream_task.clone();
        self.tasks.push(tokio::spawn(async move {
            loop {
                let Ok((stream, addr)) = mgmt_listener.accept().await else { break; };
                let peer_ip = addr.ip().to_string();
                emit_event(&evt, SubEvent::MasterConnected { peer_ip: peer_ip.clone() });
                let (reader, writer) = stream.into_split();
                let writer = Arc::new(Mutex::new(writer));
                Self::mgmt_loop(
                    reader, writer, peer_ip,
                    settings.clone(), gen.clone(), trigger.clone(),
                    evt.clone(), dw.clone(), stream_task.clone(),
                ).await;
            }
        }));

        Ok(())
    }
```

- [ ] **Step 3: 实现 `stop()`**

```rust
    pub async fn stop(&mut self) {
        if let Some(h) = self.stream_task.lock().await.take() { h.abort(); }
        for t in self.tasks.drain(..) { t.abort(); }
        *self.data_writer.lock().await = None;
        info!("SubStation stopped");
    }
```

- [ ] **Step 4: 实现命令响应循环 + 辅助方法**（追加到 `impl SubStation`，在 `trigger()` 之后）

```rust
    /// 管理管道命令响应循环。读主站帧 → 分派 → 回应。完全镜像
    /// pmusim-app/tests/e2e.rs 的 mock 子站命令表。
    #[allow(clippy::too_many_arguments)]
    async fn mgmt_loop(
        mut reader: OwnedReadHalf,
        writer: Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
        peer_ip: String,
        settings: Arc<RwLock<SubSettings>>,
        gen: Arc<RwLock<DataGen>>,
        trigger: Arc<std::sync::atomic::AtomicBool>,
        evt: EventSender,
        data_writer: Arc<Mutex<Option<tokio::net::tcp::OwnedWriteHalf>>>,
        stream_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    ) {
        loop {
            let frame_data = match read_frame(&mut reader).await {
                Ok(d) => d,
                Err(_) => break,
            };
            emit_event(&evt, SubEvent::RawFrame { direction: "recv".into(), hex: hex_encode(&frame_data) });

            let parsed = match parse(&frame_data, 0, 0, 0, 0) {
                Ok(f) => f,
                Err(e) => { warn!("解析主站帧失败: {e}"); continue; }
            };

            match parsed {
                Frame::Command(cmd) => {
                    emit_event(&evt, SubEvent::CommandReceived { cmd: cmd.cmd, name: cmd_name(cmd.cmd) });
                    Self::handle_command(
                        cmd.cmd, &writer, &settings, &gen, &trigger,
                        &evt, &data_writer, &stream_task,
                    ).await;
                }
                Frame::Config(_cfg) => {
                    // 主站下传 CFG-2 配置帧 → 回 ACK（V3 §8.6）。
                    emit_event(&evt, SubEvent::Cfg2Received);
                    Self::send_cmd(&writer, &settings, &evt, Cmd::Ack as u16).await;
                }
                Frame::Data(_) => { /* 子站不应在管理管道收数据帧 */ }
            }
        }
        // 清理
        if let Some(h) = stream_task.lock().await.take() { h.abort(); }
        *data_writer.lock().await = None;
        emit_event(&evt, SubEvent::MasterDisconnected { peer_ip });
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_command(
        cmd: u16,
        writer: &Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
        settings: &Arc<RwLock<SubSettings>>,
        gen: &Arc<RwLock<DataGen>>,
        trigger: &Arc<std::sync::atomic::AtomicBool>,
        evt: &EventSender,
        data_writer: &Arc<Mutex<Option<tokio::net::tcp::OwnedWriteHalf>>>,
        stream_task: &Arc<Mutex<Option<JoinHandle<()>>>>,
    ) {
        match cmd {
            c if c == Cmd::SendCfg1 as u16 => {
                Self::send_config(writer, settings, evt, FrameType::Cfg1 as u8, SubEvent::Cfg1Sent).await;
            }
            c if c == Cmd::SendCfg2 as u16 => {
                Self::send_config(writer, settings, evt, FrameType::Cfg2 as u8, SubEvent::Cfg2Sent).await;
            }
            c if c == Cmd::SendCfg2Cmd as u16 => {
                // 主站「下传 CFG-2 命令」通知 → 回 ACK（V3 §8.4）。
                Self::send_cmd(writer, settings, evt, Cmd::Ack as u16).await;
            }
            c if c == Cmd::OpenData as u16 => {
                Self::start_stream(settings, gen, trigger, evt, data_writer, stream_task).await;
                emit_event(evt, SubEvent::StreamingStarted);
            }
            c if c == Cmd::CloseData as u16 => {
                if let Some(h) = stream_task.lock().await.take() { h.abort(); }
                emit_event(evt, SubEvent::StreamingStopped);
            }
            c if c == Cmd::Heartbeat as u16 => {
                // §8.13：子站回送心跳（这里回同名命令字即可）。
                Self::send_cmd(writer, settings, evt, Cmd::Heartbeat as u16).await;
            }
            c if c == Cmd::Trigger as u16 => {
                trigger.store(true, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    /// 启动数据推流任务。V3：用已 accept 的 data_writer；V2：先连出主站数据口。
    async fn start_stream(
        settings: &Arc<RwLock<SubSettings>>,
        gen: &Arc<RwLock<DataGen>>,
        trigger: &Arc<std::sync::atomic::AtomicBool>,
        evt: &EventSender,
        data_writer: &Arc<Mutex<Option<tokio::net::tcp::OwnedWriteHalf>>>,
        stream_task: &Arc<Mutex<Option<JoinHandle<()>>>>,
    ) {
        // 已在推流则先停。
        if let Some(h) = stream_task.lock().await.take() { h.abort(); }

        let (version, period_ms, base_soc) = {
            let s = settings.read().await;
            let fps = s.config.data_rate_fps.max(1);
            (s.version, (1000.0 / fps as f64) as u64, current_soc())
        };

        // V2：数据管道子站作客户端，连主站数据口。主站 IP 取 mgmt 对端在此简化为
        // 127.0.0.1（本地互测）/ 由 settings.data_port 指定主站数据口；实现者若需
        // 远程，可在 SubSettings 增加 master_host/master_data_port 字段。
        if version == ProtocolVersion::V2 {
            let port = { settings.read().await.data_port };
            let target = ("127.0.0.1", if port == 0 { 7001 } else { port });
            match tokio::time::timeout(std::time::Duration::from_secs(5), TcpStream::connect(target)).await {
                Ok(Ok(stream)) => {
                    let (_r, w) = stream.into_split();
                    *data_writer.lock().await = Some(w);
                }
                _ => {
                    emit_event(evt, SubEvent::Error { error: format!("V2 数据连出 {target:?} 失败") });
                    return;
                }
            }
        }

        let settings = settings.clone();
        let gen = gen.clone();
        let trigger = trigger.clone();
        let evt = evt.clone();
        let dw = data_writer.clone();
        let handle = tokio::spawn(async move {
            let mut frame_index: u64 = 0;
            let mut ticker = tokio::time::interval(std::time::Duration::from_millis(period_ms.max(1)));
            loop {
                ticker.tick().await;
                let cfg = { settings.read().await.config.clone() };
                let g = { *gen.read().await };
                let trig = trigger.swap(false, Ordering::Relaxed);
                let df = next_data_frame(&cfg, &g, base_soc, frame_index, trig);
                let bytes = match build_data(&df, 0, 0, 0) {
                    Ok(b) => b,
                    Err(e) => { error!("build_data 失败: {e}"); continue; }
                };
                let mut guard = dw.lock().await;
                let Some(w) = guard.as_mut() else { break; };
                if w.write_all(&bytes).await.is_err() { break; }
                let _ = w.flush().await;
                drop(guard);
                emit_event(&evt, SubEvent::DataFrameSent { data: data_frame_to_info(&df) });
                frame_index += 1;
            }
        });
        *stream_task.lock().await = Some(handle);
    }

    /// 按当前配置构建并发送 CFG-1/CFG-2。
    async fn send_config(
        writer: &Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
        settings: &Arc<RwLock<SubSettings>>,
        evt: &EventSender,
        cfg_type: u8,
        sent_event: SubEvent,
    ) {
        let cfg = { build_config_frame(&settings.read().await.config, cfg_type) };
        let raw = match build_config(&cfg) {
            Ok(r) => r,
            Err(e) => { emit_event(evt, SubEvent::Error { error: format!("build_config 失败: {e}") }); return; }
        };
        let mut w = writer.lock().await;
        if w.write_all(&raw).await.is_ok() {
            let _ = w.flush().await;
            drop(w);
            emit_event(evt, SubEvent::RawFrame { direction: "send".into(), hex: hex_encode(&raw) });
            emit_event(evt, sent_event);
        }
    }

    /// 发送一条命令帧（ACK/心跳等）。
    async fn send_cmd(
        writer: &Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
        settings: &Arc<RwLock<SubSettings>>,
        evt: &EventSender,
        cmd: u16,
    ) {
        let (version, idcode) = {
            let s = settings.read().await;
            (s.version, s.config.idcode.clone())
        };
        let frame = CommandFrame { version, idcode, soc: current_soc(), fracsec: 0, cmd };
        let raw = match build_command(&frame) {
            Ok(r) => r,
            Err(e) => { error!("build_command 失败: {e}"); return; }
        };
        let mut w = writer.lock().await;
        if w.write_all(&raw).await.is_ok() {
            let _ = w.flush().await;
            drop(w);
            emit_event(evt, SubEvent::RawFrame { direction: "send".into(), hex: hex_encode(&raw) });
        }
    }
```

- [ ] **Step 5: 实现自由函数（文件末尾，`impl` 之外）**

```rust
/// 从 SubConfig 构建一个 ConfigFrame（CFG-1 或 CFG-2）。
fn build_config_frame(c: &SubConfig, cfg_type: u8) -> ConfigFrame {
    let phnmr = c.phasors.len() as u16;
    let annmr = c.analogs.len() as u16;
    let dgnmr = c.digitals.len() as u16;

    // 通道名顺序：相量名 + 模拟量名 + 16×数字量名（与 parser 期望一致）。
    let mut channel_names: Vec<String> = Vec::new();
    for i in 0..phnmr { channel_names.push(format!("PH{i:02}")); }
    for i in 0..annmr { channel_names.push(format!("AN{i:02}")); }
    for i in 0..(dgnmr * 16) { channel_names.push(format!("D{i:02}")); }

    let phunit = vec![0x0000_0001u32; phnmr as usize]; // 电压相量,比例 1
    let anunit = vec![0x0000_0064u32; annmr as usize]; // 比例因子 100
    let digunit = vec![(0x0001u16, 0x0000u16); dgnmr as usize];

    // period/fnom：fnom bit0=1 → 50Hz 基；period 取使帧率=data_rate_fps 的值。
    // period_ms = (period/100)*(1000/50)=period/5 → period = data_rate_period_ms*5。
    let fnom: u16 = 0x0001;
    let period_ms = 1000.0 / c.data_rate_fps.max(1) as f64;
    let period = (period_ms * 5.0).round() as u16;

    ConfigFrame {
        version: c.version,
        cfg_type,
        idcode: c.idcode.clone(),
        soc: current_soc(),
        fracsec: 0,
        d_frame: 0,
        meas_rate: c.meas_rate,
        num_pmu: 1,
        stn: c.stn.clone(),
        pmu_idcode: c.idcode.clone(),
        format_flags: c.format_flags,
        phnmr, annmr, dgnmr,
        channel_names,
        phunit, anunit, digunit,
        fnom,
        period,
        pmu_blocks: vec![PmuBlock {
            stn: c.stn.clone(),
            pmu_idcode: c.idcode.clone(),
            format_flags: c.format_flags,
            phnmr, annmr, dgnmr,
            channel_names: {
                let mut v = Vec::new();
                for i in 0..phnmr { v.push(format!("PH{i:02}")); }
                for i in 0..annmr { v.push(format!("AN{i:02}")); }
                for i in 0..(dgnmr * 16) { v.push(format!("D{i:02}")); }
                v
            },
            phunit: vec![0x0000_0001u32; phnmr as usize],
            anunit: vec![0x0000_0064u32; annmr as usize],
            digunit: vec![(0x0001u16, 0x0000u16); dgnmr as usize],
            fnom,
            period,
        }],
    }
}

fn cmd_name(cmd: u16) -> String {
    match cmd {
        0x0001 => "关闭数据", 0x0002 => "打开数据", 0x0004 => "召唤CFG-1",
        0x0005 => "召唤CFG-2", 0x4000 => "心跳", 0x8000 => "下传CFG-2命令",
        0xA000 => "触发", _ => "其他",
    }.to_string()
}

async fn read_frame(reader: &mut OwnedReadHalf) -> Result<Vec<u8>, String> {
    let mut header = [0u8; 4];
    reader.read_exact(&mut header).await.map_err(|e| format!("read header: {e}"))?;
    if header[0] != SYNC_BYTE {
        return Err(format!("Invalid sync byte: {:#04x}", header[0]));
    }
    let frame_size = u16::from_be_bytes([header[2], header[3]]) as usize;
    if frame_size < 4 {
        return Err(format!("Invalid frame size: {frame_size}"));
    }
    let mut buf = vec![0u8; frame_size];
    buf[..4].copy_from_slice(&header);
    reader.read_exact(&mut buf[4..]).await.map_err(|e| format!("read body: {e}"))?;
    Ok(buf)
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

fn data_frame_to_info(df: &pmusim_core::protocol::frame::DataFrame) -> DataInfo {
    DataInfo {
        soc: df.soc,
        fracsec: df.fracsec,
        stat: df.stat,
        freq: df.freq,
        dfreq: df.dfreq,
        phasors: df.phasors.clone(),
        analog: df.analog.clone(),
        digital: df.digital.clone(),
    }
}

fn emit_event(tx: &EventSender, ev: SubEvent) {
    if let Err(e) = tx.send(ev) { error!("emit_event 失败: {e}"); }
}
```

- [ ] **Step 6: 跑 V3 e2e 转绿**

Run: `cargo test -p pmusim-sub --test e2e v3_master_drives_substation_to_streaming`
Expected: 1 passed

> 若失败排查：CFG-1 通道名个数必须 = phnmr+annmr+16×dgnmr（主站 `do_send_cfg2` 会校验并拒绝，导致握手卡住）；本实现已对齐。

- [ ] **Step 7: 提交**

```bash
git add crates/pmusim-sub/src/network/substation.rs
git commit -m "feat(sub): 实现 SubStation 命令响应+数据推流(V3 e2e 转绿)"
```

### Task 8: V2 路径 + V2 e2e

**Files:**
- Modify: `crates/pmusim-sub/tests/e2e.rs`

> V2：主站是数据**服务端**（start 时绑定数据监听口），子站是数据**客户端**（OpenData 时连出）。测试需把子站的 `data_port` 指向主站实际绑定的数据口。

- [ ] **Step 1: 追加 V2 测试到 `tests/e2e.rs`**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn v2_master_drives_substation_to_streaming() {
    // 主站 V2：先 start() 取得它绑定的数据监听口
    let (m_tx, mut m_rx) = mpsc::unbounded_channel::<PmuEvent>();
    let mut master = MasterStation::new(m_tx, 0, 30.0, ProtocolVersion::V2);
    master.start().await.unwrap();
    let master_data_port = master.data_port; // V2 下非 0

    // 子站 V2：mgmt 自分配；data_port 指向主站数据口(用于连出)
    let (sub_tx, _sub_rx) = mpsc::unbounded_channel::<SubEvent>();
    let mut settings = SubSettings {
        version: ProtocolVersion::V2,
        mgmt_port: 0,
        data_port: master_data_port,
        config: sub_config(ProtocolVersion::V2),
        gen: DataGen { freq_offset_hz: 0.05, rocof_hz_s: 0.0 },
    };
    settings.config.version = ProtocolVersion::V2;
    let mut sub = SubStation::new(sub_tx, settings);
    sub.start().await.unwrap();
    let mgmt_port = sub.mgmt_port();

    master
        .connect_to_substation("127.0.0.1".into(), mgmt_port, 0, ProtocolVersion::V2)
        .await
        .unwrap();
    let tmp = match wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::SessionCreated { .. })).await {
        PmuEvent::SessionCreated { idcode, .. } => idcode,
        _ => unreachable!(),
    };
    master.auto_handshake(tmp, Some(100)).await.unwrap();

    let _ = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::Cfg1Received { .. })).await;
    let _ = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::StreamingStarted { .. })).await;
    let data = wait_master_event(&mut m_rx, |e| matches!(e, PmuEvent::DataFrame { .. })).await;
    if let PmuEvent::DataFrame { data, .. } = data {
        assert_eq!(data.analog.len(), 2);
        assert_eq!(data.digital, vec![0x000A]);
    }

    master.stop().await;
    sub.stop().await;
}
```

- [ ] **Step 2: 跑全部 e2e**

Run: `cargo test -p pmusim-sub --test e2e`
Expected: 2 passed（V2 + V3）

> 若 V2 数据连出时序问题（子站收到 OpenData 即连出，主站数据监听已就绪）导致偶发失败，确认主站在 `do_open_data_v3` 对 V2 是 no-op、且 V2 主站 start() 已绑定监听（`v2_start_still_binds_local_data_port` 印证）。

- [ ] **Step 3: 提交**

```bash
git add crates/pmusim-sub/tests/e2e.rs
git commit -m "test(sub): V2 主站↔子站 e2e 转绿"
```

---

## Phase 4 — Tauri 壳（state / commands）

### Task 9: `state.rs` 事件缓冲 + AppState

**Files:**
- Create: `crates/pmusim-sub/src/state.rs`
- Modify: `crates/pmusim-sub/src/lib.rs`

- [ ] **Step 1: 写 `state.rs`**（对照 `pmusim-app/src/state.rs`）

```rust
use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex;

use crate::events::SubEvent;
use crate::network::substation::SubStation;

const MAX_BUFFER: usize = 4096;

#[derive(Default)]
pub struct EventBuffer {
    inner: StdMutex<VecDeque<SubEvent>>,
}

impl EventBuffer {
    pub fn push(&self, ev: SubEvent) {
        let mut q = self.inner.lock().expect("event buffer poisoned");
        if q.len() >= MAX_BUFFER {
            // 优先丢高频帧事件，保留生命周期事件。
            let drop_idx = q.iter().position(|e| {
                matches!(e, SubEvent::DataFrameSent { .. } | SubEvent::RawFrame { .. })
            });
            if let Some(i) = drop_idx { q.remove(i); } else { q.pop_front(); }
        }
        q.push_back(ev);
    }
    pub fn drain(&self) -> Vec<SubEvent> {
        let mut q = self.inner.lock().expect("event buffer poisoned");
        q.drain(..).collect()
    }
}

pub struct AppState {
    pub sub: Arc<Mutex<Option<SubStation>>>,
    pub events: Arc<EventBuffer>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            sub: Arc::new(Mutex::new(None)),
            events: Arc::new(EventBuffer::default()),
        }
    }
}
```

- [ ] **Step 2: `lib.rs` 注册**

```rust
pub mod commands;
pub mod datagen;
pub mod events;
pub mod network;
pub mod state;
```

（`commands` 下个 Task 创建；本步会编译失败，属正常——先把 `commands` 注释掉或先做 Step 3 再解开。实现者：本步只加 `datagen/events/network/state`，留 `commands` 到 Task 10 一起加。）

修正后的 `lib.rs`（本 Task 提交版本）：

```rust
pub mod datagen;
pub mod events;
pub mod network;
pub mod state;
```

- [ ] **Step 3: 编译**

Run: `cargo build -p pmusim-sub`
Expected: 通过

- [ ] **Step 4: 提交**

```bash
git add crates/pmusim-sub/src/state.rs crates/pmusim-sub/src/lib.rs
git commit -m "feat(sub): AppState 事件缓冲(poll 模型)"
```

### Task 10: `commands.rs` Tauri 命令

**Files:**
- Create: `crates/pmusim-sub/src/commands.rs`
- Modify: `crates/pmusim-sub/src/lib.rs`

前端↔后端契约（命令）：`start_substation`、`stop_substation`、`update_config`、`update_gen`、`fire_trigger`、`poll_events`、`open_url`。

- [ ] **Step 1: 写 `commands.rs`**

```rust
use serde::Deserialize;
use tauri::State;
use tokio::sync::mpsc;

use pmusim_core::protocol::constants::ProtocolVersion;

use crate::datagen::{DataGen, PhasorGen, SubConfig};
use crate::events::SubEvent;
use crate::network::substation::{SubSettings, SubStation};
use crate::state::AppState;

/// 前端传入的相量定义。
#[derive(Debug, Clone, Deserialize)]
pub struct PhasorInput {
    pub magnitude: f64,
    pub phase_deg: f64,
}

/// 前端传入的完整子站配置。
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigInput {
    pub protocol: String,       // "V2" | "V3"
    pub idcode: String,
    pub stn: String,
    pub mgmt_port: u16,
    pub data_port: u16,         // V3 监听口 / V2 主站数据口；0=默认
    pub data_rate_fps: u32,
    pub phasors: Vec<PhasorInput>,
    pub analogs: Vec<f64>,
    pub digitals: Vec<u16>,
}

fn to_settings(c: &ConfigInput) -> Result<SubSettings, String> {
    let version = match c.protocol.as_str() {
        "V2" => ProtocolVersion::V2,
        "V3" => ProtocolVersion::V3,
        other => return Err(format!("未知协议: {other}")),
    };
    let config = SubConfig {
        version,
        idcode: c.idcode.clone(),
        stn: c.stn.clone(),
        data_rate_fps: c.data_rate_fps.max(1),
        meas_rate: 1_000_000,
        format_flags: 0,
        phasors: c.phasors.iter().map(|p| PhasorGen { magnitude: p.magnitude, phase_deg: p.phase_deg }).collect(),
        analogs: c.analogs.clone(),
        digitals: c.digitals.clone(),
    };
    Ok(SubSettings {
        version,
        mgmt_port: c.mgmt_port,
        data_port: c.data_port,
        config,
        gen: DataGen { freq_offset_hz: 0.0, rocof_hz_s: 0.0 },
    })
}

#[tauri::command]
pub async fn start_substation(state: State<'_, AppState>, config: ConfigInput) -> Result<(), String> {
    let mut guard = state.sub.lock().await;
    if guard.is_some() {
        return Err("子站已在运行".into());
    }
    let settings = to_settings(&config)?;
    let (tx, mut rx) = mpsc::unbounded_channel::<SubEvent>();
    let buffer = state.events.clone();
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await { buffer.push(ev); }
    });
    let mut sub = SubStation::new(tx, settings);
    sub.start().await?;
    *guard = Some(sub);
    Ok(())
}

#[tauri::command]
pub async fn stop_substation(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.sub.lock().await;
    if let Some(sub) = guard.as_mut() {
        let _ = state.events.drain();
        sub.stop().await;
    }
    *guard = None;
    Ok(())
}

#[tauri::command]
pub async fn update_config(state: State<'_, AppState>, config: ConfigInput) -> Result<(), String> {
    let settings = to_settings(&config)?;
    let guard = state.sub.lock().await;
    let sub = guard.as_ref().ok_or("子站未运行")?;
    sub.update_config(settings.config).await;
    Ok(())
}

#[tauri::command]
pub async fn update_gen(state: State<'_, AppState>, freq_offset_hz: f64, rocof_hz_s: f64) -> Result<(), String> {
    let guard = state.sub.lock().await;
    let sub = guard.as_ref().ok_or("子站未运行")?;
    sub.update_gen(DataGen { freq_offset_hz, rocof_hz_s }).await;
    Ok(())
}

#[tauri::command]
pub async fn fire_trigger(state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.sub.lock().await;
    let sub = guard.as_ref().ok_or("子站未运行")?;
    sub.trigger();
    Ok(())
}

#[tauri::command]
pub fn poll_events(state: State<'_, AppState>) -> Vec<SubEvent> {
    state.events.drain()
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("only http(s) urls allowed".into());
    }
    #[cfg(target_os = "macos")]
    let (cmd, args): (&str, Vec<&str>) = ("open", vec![url.as_str()]);
    #[cfg(target_os = "windows")]
    let (cmd, args): (&str, Vec<&str>) = ("cmd", vec!["/C", "start", "", url.as_str()]);
    #[cfg(target_os = "linux")]
    let (cmd, args): (&str, Vec<&str>) = ("xdg-open", vec![url.as_str()]);
    std::process::Command::new(cmd).args(&args).spawn().map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 2: `lib.rs` 加 `commands`**

```rust
pub mod commands;
pub mod datagen;
pub mod events;
pub mod network;
pub mod state;
```

- [ ] **Step 3: 编译**

Run: `cargo build -p pmusim-sub`
Expected: 通过

- [ ] **Step 4: 全量回归**

Run: `cargo test --workspace`
Expected: 全绿（core + app 既有测试 + sub datagen/e2e）

- [ ] **Step 5: 提交**

```bash
git add crates/pmusim-sub/src/commands.rs crates/pmusim-sub/src/lib.rs
git commit -m "feat(sub): Tauri 命令(start/stop/update/trigger/poll)"
```

---

## Phase 5 — Tauri bin + 前端

### Task 11: Tauri bin 真身（build.rs / tauri.conf.json / capabilities / main.rs）

**Files:**
- Create: `crates/pmusim-sub/build.rs`
- Create: `crates/pmusim-sub/tauri.conf.json`
- Create: `crates/pmusim-sub/capabilities/default.json`
- Create: `crates/pmusim-sub/icons/`（从 pmusim-app 复制）
- Modify: `crates/pmusim-sub/src/main.rs`
- Modify: `.gitignore`

> `generate_context!` 编译期需要 `frontendDist` 存在。本 Task 末尾先建占位 dist；Task 12 用真实 vite 产物覆盖。

- [ ] **Step 1: `build.rs`**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 2: 复制图标**

```bash
mkdir -p crates/pmusim-sub/icons
cp crates/pmusim-app/icons/* crates/pmusim-sub/icons/
```

- [ ] **Step 3: `tauri.conf.json`**（无 updater；指向 frontend-sub，devUrl 用 5174 避免与主站 5173 冲突）

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "PmuSub",
  "version": "0.1.0",
  "identifier": "com.pmusim.sub",
  "build": {
    "devUrl": "http://localhost:5174",
    "beforeDevCommand": { "script": "npm run dev", "cwd": "../../frontend-sub" },
    "beforeBuildCommand": { "script": "npm run build", "cwd": "../../frontend-sub" },
    "frontendDist": "../../frontend-sub/dist"
  },
  "app": {
    "windows": [
      {
        "title": "PmuSim 子站 - PMU Substation Simulator",
        "width": 1100,
        "height": 700,
        "minWidth": 900,
        "minHeight": 500,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "macOS": { "minimumSystemVersion": "10.15", "signingIdentity": "-" },
    "windows": { "webviewInstallMode": { "type": "downloadBootstrapper" } }
  }
}
```

- [ ] **Step 4: `capabilities/default.json`**（无 updater/process 权限）

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "enables the default permissions",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "store:default",
    "dialog:default"
  ]
}
```

- [ ] **Step 5: 真实 `main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use pmusim_sub::{commands, state::AppState};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::start_substation,
            commands::stop_substation,
            commands::update_config,
            commands::update_gen,
            commands::fire_trigger,
            commands::poll_events,
            commands::open_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PmuSub");
}
```

- [ ] **Step 6: `.gitignore` 追加**

```
frontend-sub/dist
frontend-sub/node_modules
```

- [ ] **Step 7: 占位 dist 让 bin 编译**

```bash
mkdir -p frontend-sub/dist
printf '<!doctype html><title>PmuSub</title><div id="app"></div>' > frontend-sub/dist/index.html
```

- [ ] **Step 8: 编译 bin**

Run: `cargo build -p pmusim-sub --bin pmusim-sub`
Expected: 通过（`tauri_build` 跑，`generate_context!` 读到占位 dist）

- [ ] **Step 9: 提交**

```bash
git add crates/pmusim-sub/build.rs crates/pmusim-sub/tauri.conf.json crates/pmusim-sub/capabilities crates/pmusim-sub/icons crates/pmusim-sub/src/main.rs .gitignore
git commit -m "feat(sub): Tauri bin(无 updater) + 配置/权限/图标"
```

### Task 12: 前端 `frontend-sub` 脚手架（复用主站基础设施）

**Files:**
- Create: `frontend-sub/{package.json,vite.config.ts,tsconfig.json,tsconfig.node.json,index.html}`
- Create: `frontend-sub/src/{main.ts,env.d.ts}`
- Copy: `frontend/src/i18n/` → `frontend-sub/src/i18n/`，`frontend/src/composables/{useToast,useEventLog,useFrameRate}.ts`

- [ ] **Step 1: 复制可复用基础设施**

```bash
mkdir -p frontend-sub/src/{components,composables,i18n,types}
cp frontend/tsconfig.json frontend/tsconfig.node.json frontend-sub/
cp frontend/src/env.d.ts frontend-sub/src/env.d.ts
cp -r frontend/src/i18n/* frontend-sub/src/i18n/
cp frontend/src/composables/useToast.ts frontend-sub/src/composables/
cp frontend/src/composables/useEventLog.ts frontend-sub/src/composables/
cp frontend/src/composables/useFrameRate.ts frontend-sub/src/composables/
```

- [ ] **Step 2: `frontend-sub/package.json`**

```json
{
  "name": "pmusim-sub-frontend",
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vue-tsc -b && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "vue": "^3.5"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@vitejs/plugin-vue": "^5",
    "typescript": "~5.7",
    "vite": "^6",
    "vue-tsc": "^2"
  }
}
```

- [ ] **Step 3: `frontend-sub/vite.config.ts`**（端口 5174）

```ts
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: { port: 5174, strictPort: true },
});
```

- [ ] **Step 4: `frontend-sub/index.html`**

```html
<!DOCTYPE html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>PmuSim 子站</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

- [ ] **Step 5: `frontend-sub/src/main.ts`**

```ts
import { createApp } from "vue";
import App from "./App.vue";
import { useSubEvents } from "./composables/useSubEvents";

createApp(App).mount("#app");
useSubEvents().startListening();
```

- [ ] **Step 6: 提交（脚手架）**

```bash
git add frontend-sub/package.json frontend-sub/vite.config.ts frontend-sub/tsconfig.json frontend-sub/tsconfig.node.json frontend-sub/index.html frontend-sub/src/main.ts frontend-sub/src/env.d.ts frontend-sub/src/i18n frontend-sub/src/composables
git commit -m "chore(sub-fe): frontend-sub 脚手架(复用 i18n/toast/log)"
```

### Task 13: 前端类型 + 事件 composable + 视图

**Files:**
- Create: `frontend-sub/src/types/index.ts`
- Create: `frontend-sub/src/composables/useSubEvents.ts`
- Create: `frontend-sub/src/App.vue`
- Create: `frontend-sub/src/components/{ConfigFormPanel,DataGenPanel,StatusLogPanel,SentDataPanel}.vue`

> 样式可对照 `frontend/src/App.vue` 与 `frontend/src/components/*.vue` 充实；以下给出可运行的精简实现。i18n 文案：在 `frontend-sub/src/i18n/messages.ts` 里追加子站相关 key（如 `sub.start`/`sub.stop`/`sub.idcode`...），或先用中文字面量，后续补 i18n。

- [ ] **Step 1: `types/index.ts`**

```ts
export interface SubDataInfo {
  soc: number; fracsec: number; stat: number;
  freq: number; dfreq: number;
  phasors: [number, number][];
  analog: number[]; digital: number[];
}

export type SubEvent =
  | { type: "Listening"; mgmt_port: number; data_port: number }
  | { type: "MasterConnected"; peer_ip: string }
  | { type: "MasterDisconnected"; peer_ip: string }
  | { type: "CommandReceived"; cmd: number; name: string }
  | { type: "Cfg1Sent" }
  | { type: "Cfg2Sent" }
  | { type: "Cfg2Received" }
  | { type: "StreamingStarted" }
  | { type: "StreamingStopped" }
  | { type: "DataFrameSent"; data: SubDataInfo }
  | { type: "RawFrame"; direction: string; hex: string }
  | { type: "Error"; error: string };

export interface PhasorInput { magnitude: number; phase_deg: number }
export interface ConfigInput {
  protocol: "V2" | "V3";
  idcode: string; stn: string;
  mgmt_port: number; data_port: number;
  data_rate_fps: number;
  phasors: PhasorInput[];
  analogs: number[]; digitals: number[];
}
```

- [ ] **Step 2: `composables/useSubEvents.ts`**（对照 `frontend/src/composables/usePmuEvents.ts` 的 poll 模型）

```ts
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SubEvent, SubDataInfo } from "../types";
import { useToast } from "./useToast";
import { useEventLog } from "./useEventLog";
import { useFrameRate } from "./useFrameRate";

const POLL_INTERVAL_MS = 100;

// 全局响应式状态（单例）
export const running = ref(false);
export const masterPeer = ref<string | null>(null);
export const listenPorts = ref<{ mgmt: number; data: number } | null>(null);
export const streaming = ref(false);
export const lastData = ref<SubDataInfo | null>(null);
export const sentCount = ref(0);

export function useSubEvents() {
  const { push: pushToast } = useToast();
  const { push: pushEvent } = useEventLog();
  const { tick: tickRate, reset: resetRate } = useFrameRate();

  function handle(ev: SubEvent) {
    switch (ev.type) {
      case "Listening":
        listenPorts.value = { mgmt: ev.mgmt_port, data: ev.data_port };
        pushEvent(`监听中 mgmt=${ev.mgmt_port} data=${ev.data_port}`);
        break;
      case "MasterConnected":
        masterPeer.value = ev.peer_ip;
        pushEvent(`主站已连接 ${ev.peer_ip}`);
        break;
      case "MasterDisconnected":
        masterPeer.value = null; streaming.value = false; resetRate();
        pushEvent(`主站断开 ${ev.peer_ip}`);
        break;
      case "CommandReceived":
        pushEvent(`收到命令 ${ev.name}(0x${ev.cmd.toString(16)})`);
        break;
      case "Cfg1Sent": pushEvent("已上传 CFG-1"); break;
      case "Cfg2Sent": pushEvent("已上传 CFG-2"); break;
      case "Cfg2Received": pushEvent("收到主站下传 CFG-2"); break;
      case "StreamingStarted": streaming.value = true; pushEvent("开始推流"); break;
      case "StreamingStopped": streaming.value = false; resetRate(); pushEvent("停止推流"); break;
      case "DataFrameSent": lastData.value = ev.data; sentCount.value++; tickRate(); break;
      case "RawFrame": break;
      case "Error": pushToast(ev.error, "error"); pushEvent(ev.error, "error"); break;
    }
  }

  function startListening() {
    const pollOnce = async () => {
      try {
        const events = await invoke<SubEvent[]>("poll_events");
        for (const ev of events) handle(ev);
      } catch (e) {
        console.warn("poll_events failed", e);
      } finally {
        setTimeout(pollOnce, POLL_INTERVAL_MS);
      }
    };
    pollOnce();
  }

  return { startListening };
}
```

- [ ] **Step 3: `components/ConfigFormPanel.vue`**

```vue
<script setup lang="ts">
import { reactive, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { ConfigInput } from "../types";
import { running, listenPorts } from "../composables/useSubEvents";

const form = reactive<ConfigInput>({
  protocol: "V3",
  idcode: "0GX00GP1",
  stn: "测试子站",
  mgmt_port: 8000,
  data_port: 0,
  data_rate_fps: 50,
  phasors: [{ magnitude: 1000, phase_deg: 0 }],
  analogs: [300, 3000],
  digitals: [10],
});

const phasorCount = computed({
  get: () => form.phasors.length,
  set: (n: number) => {
    const cur = form.phasors.length;
    if (n > cur) for (let i = cur; i < n; i++) form.phasors.push({ magnitude: 1000, phase_deg: 0 });
    else form.phasors.length = Math.max(0, n);
  },
});

async function start() {
  // 切协议时同步默认端口
  if (form.protocol === "V2" && form.mgmt_port === 8000) form.mgmt_port = 7000;
  if (form.protocol === "V3" && form.mgmt_port === 7000) form.mgmt_port = 8000;
  await invoke("start_substation", { config: { ...form } });
  running.value = true;
}
async function stop() {
  await invoke("stop_substation");
  running.value = false;
}
async function apply() {
  await invoke("update_config", { config: { ...form } });
}
</script>

<template>
  <section class="panel">
    <h3>子站配置</h3>
    <label>协议
      <select v-model="form.protocol" :disabled="running">
        <option value="V2">V2 (Q/GDW 131-2006)</option>
        <option value="V3">V3 (GB/T 26865.2-2011)</option>
      </select>
    </label>
    <label>站名 <input v-model="form.stn" /></label>
    <label>IDCODE <input v-model="form.idcode" maxlength="8" /></label>
    <label>管理端口 <input type="number" v-model.number="form.mgmt_port" :disabled="running" /></label>
    <label>数据端口 <input type="number" v-model.number="form.data_port" :disabled="running" /></label>
    <label>帧率(fps) <input type="number" v-model.number="form.data_rate_fps" /></label>
    <label>相量个数 <input type="number" min="0" v-model.number="phasorCount" /></label>
    <div v-for="(p, i) in form.phasors" :key="i" class="phasor-row">
      相量{{ i }}: 幅值 <input type="number" v-model.number="p.magnitude" />
      相角° <input type="number" v-model.number="p.phase_deg" />
    </div>
    <div class="actions">
      <button v-if="!running" @click="start">开始</button>
      <button v-else @click="stop">停止</button>
      <button :disabled="!running" @click="apply">应用配置</button>
    </div>
    <p v-if="listenPorts">监听: mgmt={{ listenPorts.mgmt }} data={{ listenPorts.data }}</p>
  </section>
</template>

<style scoped>
.panel { display: flex; flex-direction: column; gap: 6px; padding: 12px; }
label { display: flex; justify-content: space-between; gap: 8px; }
.phasor-row { font-size: 12px; }
.actions { display: flex; gap: 8px; margin-top: 8px; }
</style>
```

- [ ] **Step 4: `components/DataGenPanel.vue`**

```vue
<script setup lang="ts">
import { reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { running } from "../composables/useSubEvents";

const gen = reactive({ freq_offset_hz: 0.0, rocof_hz_s: 0.0 });

let timer: number | undefined;
watch(gen, () => {
  // 防抖：拖动滑块时合并更新
  clearTimeout(timer);
  timer = window.setTimeout(async () => {
    if (running.value) await invoke("update_gen", { freqOffsetHz: gen.freq_offset_hz, rocofHzS: gen.rocof_hz_s });
  }, 120);
});

async function trigger() {
  if (running.value) await invoke("fire_trigger");
}
</script>

<template>
  <section class="panel">
    <h3>数据生成</h3>
    <label>频率偏差 Δf (Hz)
      <input type="range" min="-2" max="2" step="0.01" v-model.number="gen.freq_offset_hz" />
      <span>{{ gen.freq_offset_hz.toFixed(2) }}</span>
    </label>
    <label>ROCOF (Hz/s)
      <input type="range" min="-5" max="5" step="0.1" v-model.number="gen.rocof_hz_s" />
      <span>{{ gen.rocof_hz_s.toFixed(1) }}</span>
    </label>
    <button :disabled="!running" @click="trigger">触发一帧</button>
  </section>
</template>

<style scoped>
.panel { display: flex; flex-direction: column; gap: 8px; padding: 12px; }
label { display: flex; align-items: center; gap: 8px; }
</style>
```

- [ ] **Step 5: `components/StatusLogPanel.vue`**

```vue
<script setup lang="ts">
import { masterPeer, streaming, sentCount } from "../composables/useSubEvents";
import { useEventLog } from "../composables/useEventLog";
const { entries } = useEventLog();
</script>

<template>
  <section class="panel">
    <h3>状态</h3>
    <p>主站: {{ masterPeer ?? "未连接" }}</p>
    <p>推流: {{ streaming ? "进行中" : "停止" }} · 已发 {{ sentCount }} 帧</p>
    <h4>事件日志</h4>
    <ul class="log">
      <li v-for="(e, i) in entries" :key="i" :class="e.level">{{ e.text }}</li>
    </ul>
  </section>
</template>

<style scoped>
.panel { padding: 12px; }
.log { max-height: 240px; overflow: auto; font-size: 12px; font-family: monospace; }
.error { color: #d33; }
</style>
```

> 注：`useEventLog` 的导出名（`entries`/`push` 及条目结构 `{text, level}`）以 `frontend/src/composables/useEventLog.ts` 实际实现为准；若字段名不同，按其签名调整本组件与 `useSubEvents.ts` 的 `pushEvent` 调用。

- [ ] **Step 6: `components/SentDataPanel.vue`**

```vue
<script setup lang="ts">
import { lastData } from "../composables/useSubEvents";
</script>

<template>
  <section class="panel">
    <h3>最近发送数据</h3>
    <template v-if="lastData">
      <p>SOC={{ lastData.soc }} FRACSEC={{ lastData.fracsec }} STAT=0x{{ lastData.stat.toString(16) }}</p>
      <p>FREQ={{ lastData.freq }} DFREQ={{ lastData.dfreq }}</p>
      <table>
        <thead><tr><th>相量</th><th>real</th><th>imag</th></tr></thead>
        <tbody>
          <tr v-for="(ph, i) in lastData.phasors" :key="i">
            <td>PH{{ i }}</td><td>{{ ph[0].toFixed(1) }}</td><td>{{ ph[1].toFixed(1) }}</td>
          </tr>
        </tbody>
      </table>
      <p>模拟量: {{ lastData.analog.join(", ") }}</p>
      <p>数字量: {{ lastData.digital.map(d => "0x" + d.toString(16)).join(", ") }}</p>
    </template>
    <p v-else>暂无数据</p>
  </section>
</template>

<style scoped>
.panel { padding: 12px; }
table { width: 100%; font-size: 12px; }
</style>
```

- [ ] **Step 7: `App.vue`**

```vue
<script setup lang="ts">
import ConfigFormPanel from "./components/ConfigFormPanel.vue";
import DataGenPanel from "./components/DataGenPanel.vue";
import StatusLogPanel from "./components/StatusLogPanel.vue";
import SentDataPanel from "./components/SentDataPanel.vue";
</script>

<template>
  <div class="app">
    <header><h1>PmuSim 子站</h1></header>
    <main class="layout">
      <div class="left">
        <ConfigFormPanel />
        <DataGenPanel />
        <StatusLogPanel />
      </div>
      <div class="right">
        <SentDataPanel />
      </div>
    </main>
  </div>
</template>

<style>
body { margin: 0; font-family: system-ui, sans-serif; }
.app { display: flex; flex-direction: column; height: 100vh; }
header { padding: 8px 16px; border-bottom: 1px solid #ddd; }
header h1 { font-size: 16px; margin: 0; }
.layout { display: grid; grid-template-columns: 380px 1fr; flex: 1; overflow: hidden; }
.left { overflow: auto; border-right: 1px solid #eee; }
.right { overflow: auto; }
</style>
```

- [ ] **Step 8: 安装依赖 + 构建前端**

Run:
```bash
cd frontend-sub && npm install && npm run build
```
Expected: `vue-tsc` 类型检查通过 + `dist/` 生成（无 TS 报错）

> 若 `vue-tsc` 因复用的 i18n/composables 类型报错，按报错最小修正（通常是 `useEventLog`/`useToast` 的导出签名差异）。

- [ ] **Step 9: 提交**

```bash
git add frontend-sub/src/types frontend-sub/src/composables/useSubEvents.ts frontend-sub/src/App.vue frontend-sub/src/components
git commit -m "feat(sub-fe): 子站界面(配置/数据生成/状态日志/已发数据)"
```

### Task 14: 整体冒烟（dev/build）+ 手动两 App 对连

**Files:** 无（验证）

- [ ] **Step 1: 后端整编 + 全测**

Run: `cargo build --workspace && cargo test --workspace`
Expected: 全绿

- [ ] **Step 2: 子站 dev 启动冒烟**

Run（前台手动）: `cd crates/pmusim-sub && cargo tauri dev`
Expected: 窗口出现「PmuSim 子站」，可填配置点「开始」，状态显示监听端口。
（实现者若无 GUI 环境，跳过此步并说明。）

- [ ] **Step 3: 手动对连验证（成功标准）**

1. 启 PmuSim 子站，协议 V3，mgmt=8000，点「开始」。
2. 启 PmuSim 主站，协议 V3，连 `127.0.0.1:8000`，自动握手。
3. 断言：主站数据表出现子站站名/通道，相量随 Δf 旋转；子站「已发帧数」增长。
4. 切 V2（子站 mgmt=7000、data 指向主站数据口 7001）重复。

> 自动化 e2e（Task 6/8）已覆盖此流程的机器可断言部分；本步是人工确认 UI 与真实双进程链路。

- [ ] **Step 4: 提交（如有微调）**

```bash
git add -A && git commit -m "chore(sub): 冒烟与对连验证修整" || echo "无改动"
```

---

## Phase 6 — 文档

### Task 15: README / CHANGELOG

**Files:**
- Modify: `README.md`、`README_CN.md`、`CHANGELOG.md`

- [ ] **Step 1: README 增加子站段落**

在 `README.md` / `README_CN.md` 的 Architecture 段补充 `crates/pmusim-sub` 与 `frontend-sub/`，并加一节「Substation simulator (`pmusim-sub`)」说明：独立 App、V2/V3、可配置正弦相量、与主站本地互测方法（启主站连子站）、构建命令 `cd crates/pmusim-sub && cargo tauri dev`。

- [ ] **Step 2: CHANGELOG 增加 Unreleased 条目**

```markdown
## [Unreleased]
### Added
- 新增 PMU 子站(数据发送方)模拟器 `pmusim-sub`：独立 Tauri App，支持 V2/V3 双协议、命令响应全握手、可配置正弦相量数据生成，与主站对标可本地互测。
```

- [ ] **Step 3: 提交**

```bash
git add README.md README_CN.md CHANGELOG.md
git commit -m "docs: 子站模拟器 pmusim-sub 说明"
```

---

## Self-Review

**Spec 覆盖：**
- 独立 App `pmusim-sub` → Task 2/11；复用 pmusim-core → 全程（无重写）。✓
- V2+V3 双协议 → Task 7(V3)/8(V2)，角色表已落实(mgmt server / V2 data client / V3 data server)。✓
- 命令响应全握手(召唤CFG-1/CFG-2、下传CFG-2命令、打开/关闭数据、心跳) → Task 7 `handle_command`。✓
- 可配置正弦相量 + Δf/ROCOF + 模拟/数字量定值 + 触发 → Task 3 datagen、Task 13 DataGenPanel。✓
- UI 表单 + 运行期改配置/频率参数 → Task 13 ConfigFormPanel/DataGenPanel + Task 10 update_config/update_gen。✓
- poll 缓冲事件模型镜像主站 → Task 9/10/13。✓
- 测试：datagen 单测(Task 3) + 真实主站↔子站 e2e V2/V3(Task 6/8) + 手动对连(Task 14)。✓
- v1 非目标(无独立 updater/安装包发布) → Task 2/11 显式不引入 updater 插件/权限。✓
- 预设保存/加载 JSON → **本 plan 未单列任务**。属 spec「配置与界面」一项。**补充说明**：归入 Task 13 之后的可选增量；当前 ConfigFormPanel 已是受控表单，加「导出/导入 JSON」按钮即可（`tauri-plugin-dialog` 已在依赖与权限中）。实现者可在 Task 13 追加一步，或作后续小改。已在依赖/权限层预留，不阻塞主线。

**Placeholder 扫描：** 无 TBD/TODO 残留；所有代码步骤含完整代码。前端样式标注「可对照主站充实」属增强提示，非占位（功能完整）。

**类型一致性：** 后端 `SubEvent`/`DataInfo`(camelCase) ↔ 前端 `SubEvent`/`SubDataInfo` 字段一致；`ConfigInput`(Rust Deserialize) ↔ TS `ConfigInput` 一致；命令名 `start_substation`/`update_gen`/`fire_trigger` 前后端一致；Tauri 参数 `freqOffsetHz`/`rocofHzS`(JS 驼峰) ↔ Rust `freq_offset_hz`/`rocof_hz_s`(Tauri 自动 snake_case 映射) 一致。

**已知风险 / 实现者注意：**
- `build_data` 的 `phnmr/annmr/dgnmr` 参数在现实现中未使用(下划线前缀)，传 0 即可——数据帧布局由 `DataFrame` 字段长度决定，须与 CFG-2 申报的通道数一致(datagen 保证)。
- 主站 `do_send_cfg2` 会校验 CFG-1 通道名个数 == phnmr+annmr+16×dgnmr，否则拒绝下传 CFG-2 → 握手卡住。`build_config_frame` 已按此构造通道名。
- V2 数据连出目标主机本 plan 简化为 `127.0.0.1`(本地互测)。远程主站需在 `SubSettings`/`ConfigInput` 增 `master_host` 字段并在 `start_stream` 使用——属后续增量，不影响本地对连成功标准。
