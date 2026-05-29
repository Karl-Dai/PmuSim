# Frontend i18n (中/English switching) — Design

Date: 2026-05-29
Status: Approved (design phase)

## Goal

Add runtime Chinese/English language switching to the PmuSim desktop frontend.
The app currently hardcodes Chinese throughout the Vue UI. Users need a toggle
that flips the whole UI between 中文 and English, with their choice remembered.

## Scope

In scope (all frontend):

- All user-visible UI chrome in the 4 Vue components and the composables:
  buttons, panel titles, table headers, labels, toasts, dialog text.
- Frontend-mapped **semantic labels** decoded from numeric protocol codes:
  - time-quality (FRACSEC bit27-24)
  - STAT word decode (`数据可用`/`正常`/`异常`, `同步`/`失步`, `合位`/`分位`, …)
  - trigger reasons (`手动`/`幅值越下限`/`频率越限`/…)

Out of scope:

- The Rust backend is **not** modified.
  - `pmusim-core::time_utils::fracsec_time_quality` returns Chinese labels but is
    dead code (defined + unit-tested, never wired to the UI); the frontend maps
    the numeric `time_quality` itself. Left as-is.
  - Raw protocol / communication log lines (`useCommLog`, backend-emitted comm
    strings like `OpenData 拒绝…`, `联网触发`) stay **verbatim** — they are
    technical transcripts, not UI chrome.
- No pluralization / ICU message formatting (not needed here).

## Approach

Lightweight self-authored i18n composable — **no new dependency**. Chosen over
vue-i18n because the project keeps dependencies minimal, there are only ~180
static strings across 4 components, and there is no plural/ICU requirement. The
mechanism is ~40 lines and synchronous (no startup flash).

## Architecture

New directory `frontend/src/i18n/`:

### `detect.ts` — pure functions (no Vue import → node-testable)

```ts
export type Locale = 'zh' | 'en'

// First-run resolution: explicit stored choice wins; else OS/webview locale.
export function detectLocale(navLang: string | undefined, stored: string | null): Locale

// Look up a dot-path key in a messages object for a locale, interpolate {var}.
// Missing key → returns the key string and console.warn in dev.
export function translate(
  messages: Record<Locale, Record<string, unknown>>,
  locale: Locale,
  key: string,
  params?: Record<string, string | number>,
): string
```

`detectLocale` rule: `stored === 'zh' | 'en'` → use it; otherwise
`navLang` starts with `zh` → `'zh'`, else `'en'`.

### `messages.ts` — the catalogs

Two nested objects `zh` and `en`, grouped by area:
`app.* / config.* / data.* / stat.* / quality.* / trigger.* / update.*`.
zh and en MUST have identical key sets.

### `index.ts` — Vue-layer wrapper + composable

- On module load, synchronously initialise a reactive `locale` ref:
  `detectLocale(navigator.language, localStorage.getItem('pmusim.locale'))`.
- `setLocale(l: Locale)` updates the ref and writes `localStorage['pmusim.locale']`.
- `t(key, params?)` = `translate(messages, locale.value, key, params)` (reads the
  reactive ref, so any template using `t()` re-renders on switch).
- Export `useI18n() => { locale, setLocale, t }`.

## Usage pattern

Each component `<script setup>`: `const { t } = useI18n()`, template uses
`t('config.title')`. Semantic-label lookup tables (TRIGGER_REASONS, STAT decode,
quality) change from constant Chinese arrays to arrays/maps of **keys** resolved
through `t()` at render time, so they react to locale changes.

## Language toggle UI

In `App.vue` `.title-actions` (where the GitHub icon-btn and 检查更新 button
live), add a segmented `中 / EN` control highlighting the active locale; click
calls `setLocale`. The Tauri window title is already English — not switched.

## Files touched

New:
- `frontend/src/i18n/detect.ts`
- `frontend/src/i18n/messages.ts`
- `frontend/src/i18n/index.ts`

Edited (replace hardcoded Chinese with `t()` / key-based lookup):
- `frontend/src/App.vue`
- `frontend/src/components/ConfigInfoPanel.vue`
- `frontend/src/components/DataTablePanel.vue`
- `frontend/src/components/UpdateDialog.vue`
- `frontend/src/composables/usePmuEvents.ts`, `useEventLog.ts`, `useSessions.ts`,
  `useFrameRate.ts` — per-string judgement: translate user-facing text, leave
  raw protocol-log lines verbatim.

## Testing

- `detect.ts` unit test, run via `node --experimental-strip-types` (Node 22+,
  already the CI node version — no test-runner dependency added): `detectLocale`
  (zh-CN→zh, en-US→en, stored wins), `translate` (hit, `{var}` interpolation,
  missing-key fallback to key). `detect.ts` imports no Vue, so it runs standalone.
- Catalog-parity check: `zh` and `en` key sets are identical (guards against
  missed translations).
- `vue-tsc --noEmit` passes.
- Headless-browser verification (repo norm): start dev server, drive with
  Playwright, toggle language, assert title bar + ConfigInfoPanel + DataTablePanel
  key strings flip zh↔en and back, screenshot as evidence. Delete any temp
  harness, stop the dev server, confirm clean `git status` afterwards.

## Out-of-scope notes (not acted on)

- Dead `fracsec_time_quality` in `pmusim-core` could be removed later; left
  untouched to keep this change frontend-only.
