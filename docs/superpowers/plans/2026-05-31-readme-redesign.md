# README Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite `README.md` (EN) and `README_CN.md` (ZH) into a lean, visually guided "classic GitHub README" with a self-made SVG banner, freshly captured screenshots (hero + 4 Quick-Start steps), a demo GIF, and new FAQ / Roadmap / Contributing / Acknowledgments sections — replacing the stale `simpmufep` screenshot.

**Architecture:** Screenshots are captured by rendering the Vite dev server (`npm run dev`, frontend only — the blue title bar is the app's own HTML `.title-bar`, not OS chrome) in a headless browser driven by the Playwright MCP tools at 1200×800. A **temporary, dev-only** mock-seed module (`frontend/src/dev/mockSeed.ts`, gated by `import.meta.env.DEV` + `?mock=` query param) fills the Vue store singletons so the UI shows realistic connecting / connected / streaming states without a real substation. The mock is **deleted before the final commit** — the merged PR contains zero `frontend/` changes. GIFs are assembled from captured frames with ffmpeg (gifski is not installed). Each README embeds its own UI-language asset set; `banner.svg` is shared.

**Tech Stack:** Vue 3 + Vite (dev server), Playwright MCP (`mcp__playwright__browser_*`), ffmpeg (`palettegen`/`paletteuse`), hand-authored SVG, Markdown.

**Working directory:** worktree `.claude/worktrees/readme-redesign` on branch `worktree-readme-redesign`, based on `origin/main` @ `165900e` (includes the merged 连接中 fix). Spec: `docs/superpowers/specs/2026-05-30-readme-redesign-design.md`.

**Author for every commit:** `Karl-Dai Karl <kelsoprotein@gmail.com>` — no AI co-author / generated-signature lines. Run all `git` from the worktree root.

---

## File Structure

| File | Responsibility | Lifecycle |
|------|----------------|-----------|
| `docs/screenshots/banner.svg` | Shared top banner (radar/phasor wordmark) | committed |
| `docs/screenshots/hero-en.png` / `hero-zh.png` | Hero streaming shot per language | committed |
| `docs/screenshots/step1-en.png`…`step4-en.png` | EN Quick-Start step shots | committed |
| `docs/screenshots/step1-zh.png`…`step4-zh.png` | ZH Quick-Start step shots | committed |
| `docs/screenshots/demo-en.gif` / `demo-zh.gif` | State-transition walkthrough per language | committed |
| `docs/screenshots/main.png` | Stale `simpmufep` shot | **deleted** |
| `frontend/src/dev/mockSeed.ts` | Dev-only screenshot state seeder | **created then deleted** (never in final diff) |
| `frontend/src/main.ts` | Add 4-line dev-only mock wiring, then revert | temporarily modified |
| `README.md` | English README (full rewrite) | committed |
| `README_CN.md` | Chinese README (full rewrite) | committed |

---

## Task 1: Author the SVG banner

**Files:**
- Create: `docs/screenshots/banner.svg`

- [ ] **Step 1: Create the banner SVG**

Write `docs/screenshots/banner.svg` (a flat, dependency-free SVG — renders on GitHub):

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="880" height="180" viewBox="0 0 880 180" role="img" aria-label="PmuSim — PMU Master Station Simulator">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#2563a8"/>
      <stop offset="1" stop-color="#1d4f88"/>
    </linearGradient>
  </defs>
  <rect width="880" height="180" rx="14" fill="url(#bg)"/>
  <!-- radar sweep arcs -->
  <g fill="none" stroke="#ffffff" stroke-opacity="0.18" stroke-width="2">
    <circle cx="120" cy="90" r="34"/>
    <circle cx="120" cy="90" r="58"/>
    <circle cx="120" cy="90" r="82"/>
  </g>
  <!-- phasor vectors -->
  <g stroke-width="3" stroke-linecap="round">
    <line x1="120" y1="90" x2="120" y2="34" stroke="#e3b341"/>
    <line x1="120" y1="90" x2="168" y2="118" stroke="#7fd0a0"/>
    <line x1="120" y1="90" x2="72" y2="118" stroke="#9ec5f0"/>
  </g>
  <circle cx="120" cy="90" r="5" fill="#ffffff"/>
  <!-- wordmark -->
  <text x="250" y="86" font-family="-apple-system, 'Segoe UI', Roboto, sans-serif" font-size="58" font-weight="800" fill="#ffffff">PmuSim</text>
  <text x="252" y="126" font-family="-apple-system, 'Segoe UI', Roboto, sans-serif" font-size="21" fill="#cfe0f5">PMU Master-Station Simulator · V2 (2006) &amp; V3 (2011)</text>
</svg>
```

- [ ] **Step 2: Verify it is well-formed XML**

Run: `xmllint --noout docs/screenshots/banner.svg && echo OK`
Expected: `OK` (no parse errors). If `xmllint` is absent, run `python3 -c "import xml.dom.minidom,sys; xml.dom.minidom.parse('docs/screenshots/banner.svg'); print('OK')"`.

- [ ] **Step 3: Commit**

```bash
git add docs/screenshots/banner.svg
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "assets(readme): add SVG banner (radar/phasor wordmark)"
```

---

## Task 2: Add the dev-only mock-seed harness (temporary scaffolding)

This module is **scaffolding** — it is created here, used for capture (Tasks 3–5), and **deleted in Task 6**. Do **not** commit it.

**Files:**
- Create: `frontend/src/dev/mockSeed.ts`
- Modify: `frontend/src/main.ts` (add dev-only wiring after the existing `usePmuEvents().startListening();` line)

- [ ] **Step 1: Create the mock-seed module**

Write `frontend/src/dev/mockSeed.ts` exactly:

```ts
// TEMPORARY dev-only screenshot harness — NOT shipped. Deleted before commit.
// Seeds the Vue store singletons so screenshots show realistic states without
// a real substation. Reached only via import.meta.env.DEV + ?mock=<state>.
import { useSessions } from "../composables/useSessions";
import { useCommLog } from "../composables/useCommLog";
import { useFrameRate } from "../composables/useFrameRate";
import { useEventLog } from "../composables/useEventLog";
import type { ConfigInfo, DataInfo } from "../types";

const IDCODE = "q1234567";
const PEER = "10.15.48.12";

// phnmr(4) + annmr(3) + 16*dgnmr(16) = 23 channel names, matching the
// real CFG-2 shape (DataTablePanel indexes channelNames by this layout).
const CFG: ConfigInfo = {
  cfgType: 3,
  version: 3,
  stn: "PMU-STATION-01",
  idcode: IDCODE,
  formatFlags: 0x0010,
  period: 100,
  measRate: 1_000_000,
  phnmr: 4,
  annmr: 3,
  dgnmr: 1,
  channelNames: [
    "Ua", "Ub", "Uc", "Uz",
    "有功 P", "无功 Q", "频率 F",
    "断路器", "隔离开关", "接地刀闸", "远方就地",
    "保护动作", "重合闸", "通道告警", "GPS 失步", "电源故障",
    "备用1", "备用2", "备用3", "备用4", "备用5", "备用6", "备用7",
  ],
  anunit: [0x000186a0, 0x000186a0, 0x00000064],
};

function dataFrame(): DataInfo {
  return {
    soc: 1_780_000_000,
    fracsec: 0x00000000,
    stat: 0x0000, // 数据可用 / 装置正常 / 同步 / 无触发
    format_flags: 0x0010,
    time_quality: 0,
    freq: 50.012,
    dfreq: 0.003,
    analog: [220.5, 12.3, 49.98],
    digital: [0b0000_0000_0001_0101],
    phasors: [
      [59800, 0],
      [59750, -2094],
      [59820, 2090],
      [120, 1500],
    ],
  };
}

export function applyMock(state: string): void {
  const { addSession, updateState, setConfig } = useSessions();
  const { addData } = useCommLog();
  const { tick } = useFrameRate();
  const { push } = useEventLog();

  switch (state) {
    case "idle":
      return; // empty UI

    case "connecting":
      // Placeholder keyed by host:port reads as 连接中 (per the merged fix).
      addSession(`${PEER}:8000`, PEER);
      return;

    case "connected":
      addSession(IDCODE, PEER); // real idcode (no ":") => 已连接
      updateState(IDCODE, "cfg1_received");
      setConfig(IDCODE, CFG);
      push(`管理管道建立: ${IDCODE}@${PEER}`);
      push("收到 CFG-1 (3 模拟量 / 1 开关量组)");
      return;

    case "streaming":
      addSession(IDCODE, PEER);
      updateState(IDCODE, "streaming");
      setConfig(IDCODE, CFG);
      push(`管理管道建立: ${IDCODE}@${PEER}`);
      push("收到 CFG-1 (3 模拟量 / 1 开关量组)");
      push("已下传 CFG-2");
      push("数据管道建立");
      addData(IDCODE, dataFrame());
      for (let i = 0; i < 95; i++) tick(); // seed 上传速率 ≈ 95 帧/秒
      return;
  }
}
```

- [ ] **Step 2: Wire it into `main.ts` (dev-only)**

Append after the final line `usePmuEvents().startListening();` in `frontend/src/main.ts`:

```ts

// TEMP dev-only screenshot mock (remove before commit).
if (import.meta.env.DEV) {
  const mock = new URLSearchParams(location.search).get("mock");
  if (mock) void import("./dev/mockSeed").then(({ applyMock }) => applyMock(mock));
}
```

- [ ] **Step 3: Verify the project still type-checks and builds**

Run: `cd frontend && npm run build`
Expected: `vue-tsc -b` passes (no type errors — the mock uses the real `ConfigInfo`/`DataInfo` types) and `vite build` prints `✓ built in …`.

- [ ] **Step 4: Do NOT commit**

This is scaffolding. Leave it uncommitted. (Verify with `git status --porcelain frontend/` showing `mockSeed.ts` untracked and `main.ts` modified.)

---

## Task 3: Capture English screenshots

**Files:**
- Produce: `docs/screenshots/hero-en.png`, `step1-en.png`, `step2-en.png`, `step3-en.png`, `step4-en.png`

- [ ] **Step 1: Start the dev server (background)**

Run (from worktree root): `cd frontend && npm run dev`
Run it in the background. Note the URL (Vite default `http://localhost:5173`). Confirm it is serving: `curl -sSf http://localhost:5173 >/dev/null && echo UP`.

- [ ] **Step 2: Size the browser viewport to 1200×800**

Use `mcp__playwright__browser_resize` with `width: 1200, height: 800` (matches the original `main.png` dimensions).

- [ ] **Step 3: Force English UI**

Use `mcp__playwright__browser_navigate` to `http://localhost:5173/?mock=idle`, then `mcp__playwright__browser_evaluate` with:
```js
() => { localStorage.setItem('pmusim.locale', 'en'); }
```
This makes every subsequent load render the English UI (the i18n layer reads `localStorage['pmusim.locale']` synchronously on boot).

- [ ] **Step 4: Capture step 1 — protocol picker (idle)**

Navigate to `http://localhost:5173/?mock=idle`. Wait for the title bar text "PmuSim — PMU Master Station Simulator" via `mcp__playwright__browser_wait_for` (text: `PmuSim`). Capture full page with `mcp__playwright__browser_take_screenshot` → save as `docs/screenshots/step1-en.png`.

- [ ] **Step 5: Capture step 2 — address/port filled (idle, same form)**

The connection form already shows the default `10.15.48.12 : 8000`. Reuse the idle view: capture again → `docs/screenshots/step2-en.png`. (Step 2's caption in the README points at the address/port fields; the same idle frame serves it.)

- [ ] **Step 6: Capture step 3 — connecting → connected**

Navigate to `http://localhost:5173/?mock=connected`. Wait for status text `Connected`. Capture → `docs/screenshots/step3-en.png`.

- [ ] **Step 7: Capture step 4 — streaming (data table full)**

Navigate to `http://localhost:5173/?mock=streaming`. Wait for a channel-name cell (text: `Ua`). Capture → `docs/screenshots/step4-en.png`.

- [ ] **Step 8: Capture the hero (streaming, hi-fidelity)**

Reuse the streaming view from Step 7 (or re-navigate to `?mock=streaming`). Capture → `docs/screenshots/hero-en.png`.

- [ ] **Step 9: Verify all five PNGs exist at 1200×800**

Run:
```bash
for f in hero step1 step2 step3 step4; do
  p="docs/screenshots/${f}-en.png"
  [ -f "$p" ] && echo "$p $(magick identify -format '%wx%h' "$p")" || echo "MISSING $p"
done
```
Expected: each line ends with `1200x800` (or close — full-page capture may extend height; that is acceptable). No `MISSING`.

---

## Task 4: Capture Chinese screenshots

**Files:**
- Produce: `docs/screenshots/hero-zh.png`, `step1-zh.png`, `step2-zh.png`, `step3-zh.png`, `step4-zh.png`

- [ ] **Step 1: Switch UI to Chinese**

With the dev server still running and viewport still 1200×800, `mcp__playwright__browser_navigate` to `http://localhost:5173/?mock=idle`, then `mcp__playwright__browser_evaluate`:
```js
() => { localStorage.setItem('pmusim.locale', 'zh'); }
```

- [ ] **Step 2: Capture the four steps + hero (Chinese)**

Repeat Task 3 Steps 4–8 with the `-zh` suffix, waiting on Chinese anchor texts instead:
- `?mock=idle` → wait text `PmuSim — PMU 主站模拟器` → `step1-zh.png`, and reuse for `step2-zh.png`.
- `?mock=connected` → wait text `已连接` → `step3-zh.png`.
- `?mock=streaming` → wait text `Ua` → `step4-zh.png` and `hero-zh.png`.

- [ ] **Step 3: Verify all five ZH PNGs exist at 1200×800**

Run:
```bash
for f in hero step1 step2 step3 step4; do
  p="docs/screenshots/${f}-zh.png"
  [ -f "$p" ] && echo "$p $(magick identify -format '%wx%h' "$p")" || echo "MISSING $p"
done
```
Expected: no `MISSING`; dimensions `1200x800`.

---

## Task 5: Build the demo GIFs

**Files:**
- Produce: `docs/screenshots/demo-en.gif`, `docs/screenshots/demo-zh.gif`

- [ ] **Step 1: Capture the EN frame sequence**

With UI in English (localStorage `pmusim.locale=en`), capture four frames into `/tmp/pmusim-gif-en/` by navigating and screenshotting each state:
- `?mock=idle` → `/tmp/pmusim-gif-en/01.png`
- `?mock=connecting` → `/tmp/pmusim-gif-en/02.png` (wait text `Connecting`)
- `?mock=connected` → `/tmp/pmusim-gif-en/03.png` (wait text `Connected`)
- `?mock=streaming` → `/tmp/pmusim-gif-en/04.png` (wait text `Ua`)

Create the dir first: `mkdir -p /tmp/pmusim-gif-en`.

- [ ] **Step 2: Assemble the EN GIF with ffmpeg**

Write the concat manifest and run ffmpeg (palette pipeline, width-capped to 900px, looping):

```bash
cat > /tmp/pmusim-gif-en/frames.txt <<'EOF'
file '/tmp/pmusim-gif-en/01.png'
duration 1.2
file '/tmp/pmusim-gif-en/02.png'
duration 1.2
file '/tmp/pmusim-gif-en/03.png'
duration 1.2
file '/tmp/pmusim-gif-en/04.png'
duration 2.0
file '/tmp/pmusim-gif-en/04.png'
EOF
ffmpeg -y -f concat -safe 0 -i /tmp/pmusim-gif-en/frames.txt \
  -vf "scale=900:-1:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse" \
  -loop 0 docs/screenshots/demo-en.gif
```

Expected: ffmpeg exits 0 and `docs/screenshots/demo-en.gif` is created. (The trailing duplicate frame holds the final streaming state before the loop restarts.)

- [ ] **Step 3: Capture the ZH frame sequence and assemble the ZH GIF**

Switch UI to Chinese (localStorage `pmusim.locale=zh`), `mkdir -p /tmp/pmusim-gif-zh`, capture `01.png`–`04.png` into it (anchor texts: `连接中`, `已连接`, `Ua`), then repeat the ffmpeg block with `/tmp/pmusim-gif-zh/` and output `docs/screenshots/demo-zh.gif`.

- [ ] **Step 4: Verify GIF sizes are within budget**

Run:
```bash
for g in demo-en demo-zh; do
  p="docs/screenshots/${g}.gif"
  [ -f "$p" ] && echo "$p $(du -h "$p" | cut -f1) $(magick identify -format '%n frames, %wx%h' "$p[0]" 2>/dev/null)" || echo "MISSING $p"
done
```
Expected: each GIF exists and is **under ~1.5 MB**. If a GIF exceeds 1.5 MB, re-run Step 2 with `scale=720` and/or drop the hold-frame duration, then re-check. `log()` (report to user) the final sizes.

---

## Task 6: Remove the mock scaffolding

**Files:**
- Delete: `frontend/src/dev/mockSeed.ts`
- Revert: `frontend/src/main.ts` (remove the dev-only block added in Task 2)

- [ ] **Step 1: Delete the mock module**

Run: `rm frontend/src/dev/mockSeed.ts && rmdir frontend/src/dev 2>/dev/null; echo done`

- [ ] **Step 2: Revert `main.ts`**

Run: `git checkout -- frontend/src/main.ts`

- [ ] **Step 3: Verify `frontend/` is pristine**

Run: `git status --porcelain frontend/`
Expected: **no output** except possibly the untracked `node_modules` symlink (`?? frontend/node_modules`). There must be **no** `mockSeed.ts` and **no** modified `main.ts`.

- [ ] **Step 4: Verify the build still passes without the mock**

Run: `cd frontend && npm run build`
Expected: `vue-tsc -b` + `vite build` succeed (proves nothing in the committed tree depends on the deleted mock).

---

## Task 7: Commit the visual assets

**Files:**
- Delete: `docs/screenshots/main.png`
- Add: all `docs/screenshots/*-en.png`, `*-zh.png`, `demo-*.gif` (banner already committed in Task 1)

- [ ] **Step 1: Remove the stale screenshot**

Run: `git rm docs/screenshots/main.png`

- [ ] **Step 2: Stage and report total asset size**

Run:
```bash
git add docs/screenshots/hero-en.png docs/screenshots/hero-zh.png \
  docs/screenshots/step1-en.png docs/screenshots/step2-en.png docs/screenshots/step3-en.png docs/screenshots/step4-en.png \
  docs/screenshots/step1-zh.png docs/screenshots/step2-zh.png docs/screenshots/step3-zh.png docs/screenshots/step4-zh.png \
  docs/screenshots/demo-en.gif docs/screenshots/demo-zh.gif
du -ch docs/screenshots/*.png docs/screenshots/*.gif docs/screenshots/*.svg | tail -1
```
Expected: total under ~3 MB. `log()` the total to the user.

- [ ] **Step 3: Commit**

```bash
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "assets(readme): fresh PmuSim screenshots + demo GIFs (中/EN), drop stale simpmufep shot"
```

---

## Task 8: Rewrite `README.md` (English)

**Files:**
- Modify (full rewrite): `README.md`

- [ ] **Step 1: Replace `README.md` with the new structure**

Write `README.md` with these sections in order. Preserve all existing factual content (badges, installer table, China mirror, protocol tables, build steps, macOS block) — only restructure, tighten prose, fold the protocol tables into `<details>`, and add the new blocks. Use these exact asset links and section anchors:

```markdown
<div align="center">

<img src="docs/screenshots/banner.svg" alt="PmuSim" width="100%">

[![Release](https://img.shields.io/github/v/release/Karl-Dai/PmuSim?label=release&color=2ea043)](https://github.com/Karl-Dai/PmuSim/releases)
[![Downloads](https://img.shields.io/github/downloads/Karl-Dai/PmuSim/total?color=1f6feb)](https://github.com/Karl-Dai/PmuSim/releases)
[![Stars](https://img.shields.io/github/stars/Karl-Dai/PmuSim?color=e3b341)](https://github.com/Karl-Dai/PmuSim/stargazers)
[![License: MIT](https://img.shields.io/badge/License-MIT-lightgrey.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20·%20macOS%20·%20Linux-informational)]()

**Cross-platform PMU master-station simulator — one desktop tool for Q/GDW 131-2006 (V2) and GB/T 26865.2-2011 (V3).**

Built with **Rust** · **Tauri 2** · **Vue 3** — **English** · [中文](README_CN.md)

<img src="docs/screenshots/hero-en.png" alt="PmuSim streaming live PMU data" width="100%">

</div>

---

## See it run

![PmuSim handshake-to-streaming walkthrough](docs/screenshots/demo-en.gif)

Idle → **连接中 (connecting)** → **已连接 (connected)** → live data streaming. The status pill stays amber while the TCP socket is up but the substation hasn't replied with CFG-1; it only turns green once a real PMU frame arrives.

## Why this project

Testing a PMU master is usually one of two pains: borrow a real substation, or run a half-broken script that speaks only one protocol revision. PmuSim puts a full master on your desktop:

- 📡 **Two protocol revisions, one binary** — Q/GDW 131-2006 (V2) and GB/T 26865.2-2011 (V3), wire-format and port quirks included.
- 🤝 **Correct TCP roles per spec** — mgmt pipe master-as-client; V3 data pipe master-as-client; V2 data pipe master-as-server.
- ⚡ **One-click handshake** — `CFG-1 → CFG-2 command → CFG-2 → Request CFG-2 → Open Data`, automated end-to-end with ACK/NACK waits.
- 🔄 **In-app auto-update** — ed25519-signed bundles, 4-way endpoint fallback (3 China proxies + GitHub).
- 🪶 **Small native binary** — Rust + Tauri 2; no JVM, no Python runtime, no Electron.

## Quick Start

1. **Pick a protocol (V2 / V3).** The default target `10.15.48.12 : 8000` is editable.
   <br><img src="docs/screenshots/step1-en.png" alt="Step 1 — pick protocol" width="100%">
2. **Set the substation address and ports.** The data port auto-follows the command port (editable).
   <br><img src="docs/screenshots/step2-en.png" alt="Step 2 — address and ports" width="100%">
3. **Click 开始 then 连接.** Status goes 连接中 → 已连接 once the substation replies; the IDCODE lands in the readonly field.
   <br><img src="docs/screenshots/step3-en.png" alt="Step 3 — connected" width="100%">
4. **Watch the data table fill** with CFG-2 channel names, analog scale factors, and digital labels; 上传速率 shows live fps.
   <br><img src="docs/screenshots/step4-en.png" alt="Step 4 — streaming data" width="100%">

## Download

Pre-built installers for every platform are on the **[Releases page](https://github.com/Karl-Dai/PmuSim/releases)**. Every binary is minisign-signed and verified by the in-app updater before install.

| Platform | Installer |
|----------|-----------|
| Windows  | x64: `PmuSim_<ver>_x64-setup.exe` (NSIS) · `PmuSim_<ver>_x64_en-US.msi` — ARM64: `PmuSim_<ver>_arm64-setup.exe` (NSIS) |
| macOS    | `PmuSim_<ver>_aarch64.dmg` (Apple Silicon) · `PmuSim_<ver>_x64.dmg` (Intel) |
| Linux    | `PmuSim_<ver>_amd64.AppImage` · `PmuSim_<ver>_amd64.deb` · `PmuSim-<ver>-1.x86_64.rpm` |

Auto-update is enabled from v0.3.0 onward; older builds install v0.3.0+ once, then the updater takes over. macOS users need [one extra step on first launch](#macos-first-launch).

**China mirror** (mainland GitHub access can be unstable): <https://ghfast.top/https://github.com/Karl-Dai/PmuSim/releases/latest>. From v0.3.0 the in-app updater auto-falls-back through proxies; the *first* upgrade from a pre-updater build must be installed once via the mirror.

## Build from Source

Prereqs: [Rust](https://rustup.rs/) 1.77+, [Node.js](https://nodejs.org/) 18+, Tauri CLI (`cargo install tauri-cli --version '^2'`), and the [Tauri 2 OS deps](https://v2.tauri.app/start/prerequisites/).

```bash
cd frontend && npm install          # one-time
cd ../crates/pmusim-app && cargo tauri dev   # dev
cargo tauri build                    # production bundle
```

`cargo test --workspace` runs the core protocol tests (frame parser, CRC, time-utils round-trip).

## Protocol Support

<details>
<summary><b>Frame types, commands, connection model, V2 vs V3</b></summary>

### Frame Types

| SYNC   | Frame Type | Direction                        |
|--------|------------|----------------------------------|
| 0xAA0x | Data       | Substation → Master (data pipe)  |
| 0xAA2x | CFG-1      | Substation → Master (mgmt pipe)  |
| 0xAA3x | CFG-2      | Bidirectional (mgmt pipe)        |
| 0xAA4x | Command    | Master → Substation (mgmt pipe)  |

### Commands

| Code   | Command        | Description                            |
|--------|----------------|----------------------------------------|
| 0x0001 | Close Data     | Stop real-time data stream             |
| 0x0002 | Open Data      | Start real-time data stream            |
| 0x0004 | Send CFG-1     | Request configuration frame 1          |
| 0x0005 | Send CFG-2     | Request configuration frame 2          |
| 0x4000 | Heartbeat      | Keep-alive heartbeat                   |
| 0x8000 | Send CFG-2 Cmd | Notify substation before sending CFG-2 |

### Connection Model

| Pipe       | Master Role (V2) | Master Role (V3) | V2 Port | V3 Port |
|------------|------------------|------------------|---------|---------|
| Management | Client           | Client           | 7000    | 8000    |
| Data       | Server           | Client (outbound)| 7001    | 8001    |

### V2 vs V3

| Feature             | V2 (2006)            | V3 (2011)            |
|---------------------|----------------------|----------------------|
| Management port     | 7000                 | 8000                 |
| Data port           | 7001                 | 8001                 |
| IDCODE length       | 2 bytes              | 8 bytes (ASCII)      |
| Header field order  | SYNC-SIZE-SOC-IDCODE | SYNC-SIZE-IDCODE-SOC |
| Data frame IDCODE   | Not present          | Present              |
| Time quality        | 4-bit                | 8-bit                |
| Data pipe direction | Master = Server      | Master = Client      |

</details>

## Architecture

```
PmuSim/
├── crates/
│   ├── pmusim-core/      # Protocol library (no Tauri dependency)
│   └── pmusim-app/       # Tauri desktop application
├── frontend/             # Vue 3 + TypeScript SPA
├── scripts/              # Release scripts (updater manifest, release notes)
└── .github/workflows/    # CI: release.yml (sign + publish)
```

| Layer    | Stack                                                            |
|----------|------------------------------------------------------------------|
| Backend  | Rust, [tokio](https://tokio.rs/) (async TCP), `encoding_rs` (GBK) |
| Frontend | Vue 3, TypeScript, Vite                                          |
| Desktop  | [Tauri 2](https://tauri.app/) + `tauri-plugin-updater`           |

## FAQ / Troubleshooting

<details>
<summary><b>Status shows 已连接 but the data table stays empty / 上传速率 is 0</b></summary>

The status pill distinguishes two states: **连接中 (connecting)** means the TCP socket is open but the substation hasn't returned CFG-1 yet; **已连接 (connected)** means a real PMU frame has arrived. If you see 连接中 (amber) plus an event-log line like `CFG-1 not received after request`, the command port accepted the TCP connection but nothing is speaking the PMU protocol behind it — check that the substation's command service is actually running on that port.
</details>

<details>
<summary><b>macOS: "PmuSim cannot be opened — Apple could not verify…"</b></summary>

The bundles are ad-hoc-signed (not notarized). See [macOS First Launch](#macos-first-launch) for the one-time allow step.
</details>

<details>
<summary><b>GitHub downloads are slow or blocked (mainland China)</b></summary>

Use the China mirror in [Download](#download); the in-app updater also auto-falls-back through proxies from v0.3.0.
</details>

<details>
<summary><b>Which side binds the data port?</b></summary>

V2: the master is the data-pipe **server** and binds the local listen port (command port + 1 by default). V3: the master is a **client** and dials out to the substation's remote data port. The UI labels the field accordingly and hides the irrelevant one per protocol.
</details>

## Roadmap

V3 spec-conformance items from `docs/TODO.md` are all shipped: FORMAT-flag decoding (float / rectangular), multi-PMU config frames, CFG-2 ACK/NACK waits, heartbeat-timeout hardening, STAT bit10 config-change re-handshake, ANUNIT type-byte masking, GPS time-quality decode, off-lock-safe IDCODE bytes, and OpenData state-gating. Remaining: the lab substation's data source (IEMP pipeline) currently emits all-zero samples — a substation-side concern, tracked in `docs/TODO.md`. New ideas and field reports are welcome via issues.

## Contributing

1. Open an issue describing the bug/feature before a large PR.
2. Branch from `main`; keep PRs focused.
3. Before opening a PR, run `cargo test --workspace` and `cd frontend && npm run build` — both must pass.
4. **Commit authorship:** all commits must be authored `Karl-Dai Karl <kelsoprotein@gmail.com>` with **no** AI co-author or generated-signature trailer lines.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for full history, or the [Releases page](https://github.com/Karl-Dai/PmuSim/releases) for signed installers and updater manifests.

## macOS First Launch

The bundles are **not Apple-notarized** (no paid Developer Program). On first launch macOS shows *"PmuSim cannot be opened — Apple could not verify…"* — the app is **not damaged**, this is the standard macOS block for ad-hoc-signed apps.

<details>
<summary><b>How to allow it (pick one)</b></summary>

**1. GUI path** — double-click the `.app`, click *Done*; open *System Settings → Privacy & Security*, scroll down, click *Open Anyway*, enter your password; click *Open* on the next dialog. Subsequent launches go straight through.

**2. One-line Terminal**

```bash
xattr -dr com.apple.quarantine "/Applications/PmuSim.app"
```

</details>

## Acknowledgments

Built on [Tauri 2](https://tauri.app/), [Vue 3](https://vuejs.org/), [tokio](https://tokio.rs/), [`encoding_rs`](https://github.com/hsivonen/encoding_rs), and `tauri-plugin-updater`. Protocol behavior follows the GB/T 26865.2-2011 (V3) and Q/GDW 131-2006 (V2) specifications.

## License

[MIT](LICENSE)
```

- [ ] **Step 2: Verify every referenced asset exists**

Run:
```bash
grep -oE 'docs/screenshots/[A-Za-z0-9._-]+' README.md | sort -u | while read -r f; do
  [ -f "$f" ] && echo "OK $f" || echo "MISSING $f"
done
```
Expected: every line starts with `OK`. No `MISSING`.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "docs(readme): rewrite English README — banner, guided screenshots, GIF, FAQ/Roadmap/Contributing"
```

---

## Task 9: Rewrite `README_CN.md` (Chinese)

**Files:**
- Modify (full rewrite): `README_CN.md`

- [ ] **Step 1: Replace `README_CN.md` with the Chinese mirror of Task 8**

Same structure and section order as `README.md`, translated to Chinese, but referencing the **`-zh` assets** (`hero-zh.png`, `demo-zh.gif`, `step1-zh.png`…`step4-zh.png`). The banner is shared (`banner.svg`). Keep the language-switch line as `[English](README.md) · **中文**`. Write `README_CN.md`:

```markdown
<div align="center">

<img src="docs/screenshots/banner.svg" alt="PmuSim" width="100%">

[![Release](https://img.shields.io/github/v/release/Karl-Dai/PmuSim?label=release&color=2ea043)](https://github.com/Karl-Dai/PmuSim/releases)
[![Downloads](https://img.shields.io/github/downloads/Karl-Dai/PmuSim/total?color=1f6feb)](https://github.com/Karl-Dai/PmuSim/releases)
[![Stars](https://img.shields.io/github/stars/Karl-Dai/PmuSim?color=e3b341)](https://github.com/Karl-Dai/PmuSim/stargazers)
[![License: MIT](https://img.shields.io/badge/License-MIT-lightgrey.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20·%20macOS%20·%20Linux-informational)]()

**跨平台 PMU 主站模拟器 — 一个桌面工具同时跑 Q/GDW 131-2006 (V2) 与 GB/T 26865.2-2011 (V3)。**

基于 **Rust** · **Tauri 2** · **Vue 3** — [English](README.md) · **中文**

<img src="docs/screenshots/hero-zh.png" alt="PmuSim 正在接收实时 PMU 数据" width="100%">

</div>

---

## 运行效果

![PmuSim 从握手到数据流的演示](docs/screenshots/demo-zh.gif)

空闲 → **连接中** → **已连接** → 实时数据流。TCP 已连上但子站还没回 CFG-1 时,状态保持琥珀色「连接中」;只有真正收到 PMU 帧后才转绿「已连接」。

## 这是个什么项目

测试 PMU 主站通常两种痛苦:借一台真实子站,或跑一段半坏的脚本、还常常只支持一个版本的规约。PmuSim 把完整的主站装进桌面端:

- 📡 **两个规约版本共用一个二进制** — Q/GDW 131-2006 (V2) 与 GB/T 26865.2-2011 (V3),帧格式与端口差异都已对齐。
- 🤝 **TCP 角色按规约正确** — 管理通道主站作 client;V3 数据通道主站作 client,V2 数据通道主站作 server。
- ⚡ **一键握手** — `CFG-1 → CFG-2 命令 → CFG-2 → 召唤 CFG-2 → 启动数据`,带 ACK/NACK 等待,全流程自动化。
- 🔄 **应用内自动更新** — ed25519 签名安装包,4 路 endpoint 回退(国内 3 个镜像 + GitHub)。
- 🪶 **小体积原生** — Rust + Tauri 2;没 JVM、没 Python runtime、没 Electron。

## 快速上手

1. **选规约 (V2 / V3)。** 默认目标 `10.15.48.12 : 8000` 可改。
   <br><img src="docs/screenshots/step1-zh.png" alt="第一步 — 选规约" width="100%">
2. **填子站地址与端口。** 数据端口自动跟随命令端口(可改)。
   <br><img src="docs/screenshots/step2-zh.png" alt="第二步 — 地址与端口" width="100%">
3. **点 开始,再点 连接。** 子站回应后状态从 连接中 → 已连接,IDCODE 落入只读字段。
   <br><img src="docs/screenshots/step3-zh.png" alt="第三步 — 已连接" width="100%">
4. **看数据表填充** — CFG-2 通道名、模拟量比例系数、开关量标签;上传速率显示实时帧率。
   <br><img src="docs/screenshots/step4-zh.png" alt="第四步 — 数据流" width="100%">

## 下载

预编译安装包在 **[Releases 页面](https://github.com/Karl-Dai/PmuSim/releases)**,每个文件都做了 minisign 签名,应用内更新器验签后才安装。

| 平台    | 安装包 |
|---------|--------|
| Windows | x64: `PmuSim_<ver>_x64-setup.exe` (NSIS) · `PmuSim_<ver>_x64_en-US.msi` — ARM64: `PmuSim_<ver>_arm64-setup.exe` (NSIS) |
| macOS   | `PmuSim_<ver>_aarch64.dmg` (Apple Silicon) · `PmuSim_<ver>_x64.dmg` (Intel) |
| Linux   | `PmuSim_<ver>_amd64.AppImage` · `PmuSim_<ver>_amd64.deb` · `PmuSim-<ver>-1.x86_64.rpm` |

v0.3.0 起支持应用内自动更新;旧版本先手动装一次 v0.3.0+,之后更新器接管。macOS 首次启动需要[一步操作](#macos-首次启动)。

**国内镜像**(GitHub 访问可能不稳):<https://ghfast.top/https://github.com/Karl-Dai/PmuSim/releases/latest>。v0.3.0 起更新器自动在多个 proxy 间回退;但从无更新器的旧版**首次升级**,需先通过镜像装一次。

## 从源码构建

前置:[Rust](https://rustup.rs/) 1.77+、[Node.js](https://nodejs.org/) 18+、Tauri CLI(`cargo install tauri-cli --version '^2'`)、[Tauri 2 系统依赖](https://v2.tauri.app/start/prerequisites/)。

```bash
cd frontend && npm install          # 一次性
cd ../crates/pmusim-app && cargo tauri dev   # dev
cargo tauri build                    # 生产构建
```

`cargo test --workspace` 跑核心协议测试(帧解析、CRC、时间工具 round-trip)。

## 规约支持

<details>
<summary><b>帧类型、命令、通道方向、V2 vs V3</b></summary>

### 帧类型

| SYNC   | 帧类型 | 方向                    |
|--------|--------|-------------------------|
| 0xAA0x | 数据帧 | 子站 → 主站 (数据通道)  |
| 0xAA2x | CFG-1  | 子站 → 主站 (管理通道)  |
| 0xAA3x | CFG-2  | 双向 (管理通道)         |
| 0xAA4x | 命令帧 | 主站 → 子站 (管理通道)  |

### 命令

| 代码   | 命令          | 说明                    |
|--------|---------------|-------------------------|
| 0x0001 | 关数据        | 停止实时数据流          |
| 0x0002 | 开数据        | 启动实时数据流          |
| 0x0004 | 请求 CFG-1    | 请求配置帧 1            |
| 0x0005 | 请求 CFG-2    | 请求配置帧 2            |
| 0x4000 | 心跳          | 保活心跳                |
| 0x8000 | 发 CFG-2 通知 | 通知子站即将下发 CFG-2  |

### 通道方向

| 通道 | 主站角色 (V2) | 主站角色 (V3) | V2 端口 | V3 端口 |
|------|---------------|---------------|---------|---------|
| 管理 | client        | client        | 7000    | 8000    |
| 数据 | server        | client (外联) | 7001    | 8001    |

### V2 vs V3 差异

| 特性             | V2 (2006)            | V3 (2011)            |
|------------------|----------------------|----------------------|
| 管理端口         | 7000                 | 8000                 |
| 数据端口         | 7001                 | 8001                 |
| IDCODE 长度      | 2 字节               | 8 字节 (ASCII)       |
| 帧头字段顺序     | SYNC-SIZE-SOC-IDCODE | SYNC-SIZE-IDCODE-SOC |
| 数据帧带 IDCODE  | 否                   | 是                   |
| 时间质量         | 4-bit                | 8-bit                |
| 数据通道主站角色 | server               | client               |

</details>

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

| 层     | 技术栈                                                           |
|--------|------------------------------------------------------------------|
| 后端   | Rust, [tokio](https://tokio.rs/) (async TCP), `encoding_rs` (GBK) |
| 前端   | Vue 3, TypeScript, Vite                                          |
| 桌面层 | [Tauri 2](https://tauri.app/) + `tauri-plugin-updater`           |

## 常见问题 / 故障排查

<details>
<summary><b>状态显示「已连接」但数据表一直空 / 上传速率为 0</b></summary>

状态分两种:**连接中**=TCP 已连上但子站还没回 CFG-1;**已连接**=已收到真实 PMU 帧。若看到琥珀色「连接中」加事件日志 `CFG-1 not received after request`,说明命令端口的 TCP 能连上、但后面没有真正说 PMU 协议的服务——检查子站的命令服务是否真的起在那个端口上。
</details>

<details>
<summary><b>macOS:「PmuSim 无法打开 — Apple 无法验证…」</b></summary>

安装包是 ad-hoc 签名(未公证)。一次性放行步骤见 [macOS 首次启动](#macos-首次启动)。
</details>

<details>
<summary><b>国内下载 GitHub 慢或被墙</b></summary>

用[下载](#下载)里的国内镜像;v0.3.0 起更新器也会自动走 proxy 回退。
</details>

<details>
<summary><b>数据端口由哪一端 bind?</b></summary>

V2:主站是数据通道**服务端**,在本地 bind 侦听端口(默认命令端口+1)。V3:主站是**客户端**,外连子站的远程数据端口。UI 会按规约命名该字段并隐藏无关项。
</details>

## 路线图

`docs/TODO.md` 里的 V3 规约符合性条目已全部完成:FORMAT 标志位解码(浮点/直角坐标)、多 PMU 配置帧、CFG-2 ACK/NACK 等待、心跳超时加固、STAT bit10 配置变更重握手、ANUNIT 类型字节掩码、GPS 时间质量解码、IDCODE 原字节保留、OpenData 状态门控。剩余:lab 子站数据源(IEMP pipeline)目前发全 0 采样——属子站侧问题,记录在 `docs/TODO.md`。欢迎通过 issue 提新想法与现场反馈。

## 参与贡献

1. 大改动前先开 issue 说明 bug / 需求。
2. 从 `main` 拉分支,PR 保持聚焦。
3. 提 PR 前跑 `cargo test --workspace` 和 `cd frontend && npm run build`,两者都要通过。
4. **提交署名:** 所有 commit 作者必须为 `Karl-Dai Karl <kelsoprotein@gmail.com>`,**禁止** AI co-author 或任何生成署名行。

## 更新日志

完整历史见 [CHANGELOG.md](CHANGELOG.md),签名安装包和 updater manifest 见 [Releases 页面](https://github.com/Karl-Dai/PmuSim/releases)。

## macOS 首次启动

安装包**未经 Apple 公证**(没买 Developer Program)。首次启动 macOS 会弹「PmuSim 无法打开 — Apple 无法验证…」——应用**没有损坏**,这是对 ad-hoc 签名应用的常规拦截。

<details>
<summary><b>放行方法(二选一)</b></summary>

**1. GUI 路径** — 双击 `.app`,点「完成」;打开*系统设置 → 隐私与安全性*,滚到最下面,点「仍要打开」并输入密码;在下一个对话框点「打开」。之后启动不再被拦。

**2. 终端一行命令**

```bash
xattr -dr com.apple.quarantine "/Applications/PmuSim.app"
```

</details>

## 致谢

基于 [Tauri 2](https://tauri.app/)、[Vue 3](https://vuejs.org/)、[tokio](https://tokio.rs/)、[`encoding_rs`](https://github.com/hsivonen/encoding_rs) 与 `tauri-plugin-updater` 构建。协议行为遵循 GB/T 26865.2-2011 (V3) 与 Q/GDW 131-2006 (V2) 规约。

## 许可证

[MIT](LICENSE)
```

- [ ] **Step 2: Verify every referenced asset exists**

Run:
```bash
grep -oE 'docs/screenshots/[A-Za-z0-9._-]+' README_CN.md | sort -u | while read -r f; do
  [ -f "$f" ] && echo "OK $f" || echo "MISSING $f"
done
```
Expected: every line `OK`. No `MISSING`.

- [ ] **Step 3: Commit**

```bash
git add README_CN.md
git commit --author="Karl-Dai Karl <kelsoprotein@gmail.com>" -m "docs(readme): rewrite 中文 README — 共用 banner、中文 UI 截图/GIF、FAQ/路线图/贡献"
```

---

## Task 10: Final verification, push, and PR

- [ ] **Step 1: Confirm the final diff touches no app code**

Run: `git diff --name-only main...HEAD`
Expected: only `README.md`, `README_CN.md`, `docs/screenshots/*` (added/deleted), and the two `docs/superpowers/{specs,plans}` docs. **No** `frontend/` or `crates/` paths.

- [ ] **Step 2: Confirm working tree is clean**

Run: `git status --porcelain`
Expected: empty (or only `?? frontend/node_modules`, the local symlink).

- [ ] **Step 3: Confirm the frontend still builds (regression guard)**

Run: `cd frontend && npm run build`
Expected: `vue-tsc -b` + `vite build` succeed.

- [ ] **Step 4: Confirm every commit has the correct author**

Run: `git log main..HEAD --format='%an <%ae>' | sort -u`
Expected: exactly `Karl-Dai Karl <kelsoprotein@gmail.com>` and nothing else. No `Co-Authored-By` / generated trailers (`git log main..HEAD --format='%b' | grep -i 'co-authored\|claude'` → empty).

- [ ] **Step 5: Push and open the PR**

```bash
git push -u origin worktree-readme-redesign
gh pr create --base main --head worktree-readme-redesign \
  --title "docs(readme): 重新设计 README — banner / 截图 / GIF / 引导 + 经典板块" \
  --body "Lean redesign of both READMEs: shared SVG banner, fresh PmuSim screenshots (中/EN), state-transition demo GIFs, guided Quick Start, collapsed protocol tables, and new FAQ / Roadmap / Contributing / Acknowledgments. Stale simpmufep screenshot removed. Screenshots self-captured via a dev-only mock harness (removed before commit — no frontend/ changes in the diff). Spec: docs/superpowers/specs/2026-05-30-readme-redesign-design.md."
```

- [ ] **Step 6: Report the PR URL and asset sizes to the user.**

---

## Self-Review

**Spec coverage:** banner (T1), bilingual screenshot sets + hero + steps + GIF (T3–T5, T7), dev-only mock removed before commit (T2/T6, verified T10·S1), stale `main.png` deleted (T7), full structure with folded protocol tables + new FAQ/Roadmap/Contributing/Acknowledgments (T8/T9), size guard reported (T5·S4, T7·S2), author-signature requirement enforced (every commit + T10·S4), zero `frontend/` in final diff (T6·S3, T10·S1). All spec sections map to a task.

**Placeholder scan:** no TBD/TODO/"handle appropriately" — all SVG, mock code, ffmpeg commands, and full README bodies are inline.

**Type consistency:** mock uses `ConfigInfo` (camelCase: `cfgType, formatFlags, measRate, channelNames, anunit`) and `DataInfo` (snake_case: `format_flags, time_quality`) exactly as defined in `frontend/src/types/index.ts`; store calls `addSession/updateState/setConfig/addData/tick/push` match the composable signatures; `channelNames` length 23 = `phnmr 4 + annmr 3 + 16*dgnmr 1`, matching DataTablePanel's indexing.
