# README Redesign — Design Spec

**Date:** 2026-05-30
**Branch:** `readme-redesign` (worktree, based on `origin/main` @ `165900e`, the merged 连接中 fix)
**Author:** Karl-Dai Karl

## Goal

Rewrite both `README.md` (English, primary) and `README_CN.md` (Chinese) into a
visually guided, "classic good GitHub README" while keeping the prose lean. Add
self-captured screenshots, a demo GIF, an SVG banner, and the standard README
blocks the project currently lacks (FAQ, Roadmap, Contributing,
Acknowledgments). Replace the stale screenshot that still shows the old
`simpmufep` title bar.

## Non-Goals

- No changes to application code, protocol behavior, or UI. (A temporary
  dev-only screenshot mock harness is added during capture and **removed before
  the final commit** — the merged PR ships zero mock code, zero `frontend/`
  changes.)
- No new docs site / wiki. Everything stays in the two README files plus
  `docs/screenshots/` assets.
- No translation of the changelog or in-repo specs.

## Final README Structure (both files, identical structure)

Top to bottom:

1. **SVG banner** (`docs/screenshots/banner.svg`) — replaces the bare 📡 emoji
   title. Radar / phasor-vector theme, "PmuSim" wordmark + one-line role.
2. **Badges** — keep existing (release, downloads, stars, license, platform) +
   the "Built with Rust · Tauri 2 · Vue 3" line. Language-switch link.
3. **Hero screenshot** — single clean *streaming* state: connected, data table
   full of CFG-2 channel names, fps ticking.
4. **Demo GIF** — the dynamic walkthrough: idle → 连接中 (amber) → 已连接 (green)
   → streaming with moving fps. Primary "guided" element.
5. **Why this project** — tightened from the current version (keep the 5
   bullets, trim wording).
6. **Quick Start (guided gallery)** — numbered steps, each paired with a step
   screenshot: pick protocol → fill address/port → 开始 + 连接 → data flows.
   Steps reference what the reader sees in each shot.
7. **Download** — keep platform installer table + China mirror section
   (unchanged content, tightened).
8. **Build from Source** — keep, compact.
9. **Protocol Support** — folded into `<details>` (frame types, commands,
   connection model, V2-vs-V3 tables). Keeps first screen clean.
10. **Architecture** — keep, compact (tree + layer table).
11. **FAQ / Troubleshooting** — new (see Content Sources).
12. **Roadmap** — new (from `docs/TODO.md`).
13. **Contributing** — new (see Content Sources).
14. **Changelog** — keep (link to CHANGELOG.md + Releases).
15. **macOS First Launch** — keep (the `<details>` allow-it block).
16. **Acknowledgments** — new.
17. **License** — keep (MIT).

## Visual Assets

All assets live under `docs/screenshots/`. **Bilingual: each README gets its own
UI-language set** — `README.md` embeds English-UI shots, `README_CN.md` embeds
Chinese-UI shots. `banner.svg` is language-neutral and shared.

| File | Content |
|------|---------|
| `banner.svg` | Wordmark + radar/phasor motif, hand-authored SVG (shared) |
| `hero-en.png` / `hero-zh.png` | Streaming state, 1200×800 |
| `demo-en.gif` / `demo-zh.gif` | idle→连接中→已连接→streaming loop, width-capped ~900px, ~6–8s |
| `step1-en.png`…`step4-en.png` | Quick Start steps, EN UI |
| `step1-zh.png`…`step4-zh.png` | Quick Start steps, ZH UI |

Total: 1 SVG + 10 PNG + 2 GIF. The stale `docs/screenshots/main.png` (old
`simpmufep` title bar) is **deleted**; the READMEs reference the new files.

### Capture Mechanism

The blue title bar in screenshots is the app's own HTML `.title-bar` (App.vue),
not macOS window chrome — so the Vite dev server rendered in a headless browser
reproduces the exact existing look without running the Tauri backend.

1. Run `npm run dev` (frontend only).
2. Add a **temporary, dev-only** seed module gated behind a `?mock=<state>` query
   param and `import.meta.env.DEV`, wired in `main.ts`. It populates the Vue
   store singletons (`useSessions`, `useCommLog`, `useFrameRate`,
   `useEventLog`) so the data table shows filled CFG-2 channels, a connected/
   streaming status, and a ticking fps — without a real substation. States
   seeded: `idle`, `connecting`, `connected`, `streaming`.
3. Drive with Playwright (browser tool) at **1200×800** viewport (matches the
   current `main.png`). Capture each set twice — once with UI in English, once
   in Chinese (title-bar 中 / EN toggle).
4. Capture stills for hero + 4 Quick Start steps per language. For each GIF,
   capture an ordered frame sequence across the states and assemble with
   **ffmpeg** (`palettegen` + `paletteuse`, looping, width-capped). gifski is
   not installed; ffmpeg is the chosen tool.
5. **Delete the mock seed module and its `main.ts` wiring** before committing.
   Verify the final diff shows no `frontend/src` changes.

### Repo-Size Guard

PNGs optimized; GIF width-capped (~900px) and frame-count-limited. Before
committing assets, report total added bytes. Target: each GIF under ~1.5 MB and
the whole asset set under ~3 MB.

## New Content Blocks (grounded — no invented facts)

- **FAQ / Troubleshooting:**
  - "Shows 已连接 but no data / 0 fps" → explains 连接中 (TCP up, no CFG-1 yet) vs
    已连接 (substation replied), referencing the merged fix behavior.
  - macOS "cannot be opened" → points to the macOS First Launch section.
  - Slow / blocked GitHub download in mainland China → China mirror.
  - "Which side binds the data port?" → V2 master = server, V3 master = client.
- **Roadmap:** distilled from `docs/TODO.md`, split ✅ done / 🚧 planned. Only
  items actually present in TODO.md.
- **Contributing:** issue/PR flow; branch convention; **author-signature
  requirement** — all commits authored `Karl-Dai Karl <kelsoprotein@gmail.com>`,
  no AI co-author / generated-signature lines; run `cargo test --workspace` and
  the frontend build (`npm run build`) before opening a PR.
- **Acknowledgments:** Tauri 2, Vue 3, tokio, encoding_rs, `tauri-plugin-updater`,
  and the GB/T 26865.2-2011 / Q/GDW 131-2006 specifications.

## Bilingual Parity Rules

- CN and EN README are structurally identical (same sections, same order).
- All English headings have a matching Chinese heading; the language-switch link
  at the top stays.
- Each README embeds its own UI-language screenshot set; `banner.svg` is shared.

## Process

- All work in the `readme-redesign` worktree → single PR to `main`.
- This spec is committed first, then assets, then README rewrites.
- **Verification before "done":**
  - Final diff shows only `README.md`, `README_CN.md`, `docs/screenshots/*`, and
    this spec changed — **no `frontend/` or `crates/` changes** (mock removed).
  - Both READMEs' relative image links resolve (every referenced
    `docs/screenshots/<file>` exists).
  - Report total bytes of added assets.
- PR authored `Karl-Dai Karl <kelsoprotein@gmail.com>`, no generated signature.

## Risks

- **Mock realism:** seeded state must look plausible (sane channel names, fps,
  SOC/time). Mitigation: seed from the real CFG-2 shape — STAT rows plus a few
  analog/phasor channels with believable names.
- **GIF size:** mitigated by width cap + frame limit + size report before commit.
- **iCloud read latency** (known environment quirk): tolerate delayed tool
  output; avoid speculative parallel polling; re-read via local temp if needed.
