# Changelog

All notable changes to PmuSim are documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.10.0] - 2026-06-25

### Highlights / 亮点

- 🕵️ 主站接收侧新增「数据帧时间戳错乱检测」:逐帧校验数据帧 SOC+FRACSEC 换算的绝对毫秒是否按当前数据速率间隔正常递增,回退 / 跳变(丢帧) / 停滞三类异常即时曝光,无需肉眼盯流 / New "data-frame timestamp anomaly detection" on the master's receive side: every data frame's absolute milliseconds (from SOC+FRACSEC) is checked against the current reporting interval, and three anomaly classes — backward / gap (frame loss) / stall — are surfaced the instant they happen, no eyeballing the stream.
- 🔔 复用既有 Error 事件通道,前端零改动:异常报文带「类型 / 预期·实际间隔 / SOC 北京时间 / FRACSEC」直接进前端 toast + 事件日志 / Reuses the existing Error event channel with zero frontend changes: each anomaly carries its type, expected vs. actual interval, SOC (Beijing time) and FRACSEC straight into the frontend toast + event log.
- 🧪 新增 `pmusim-core::ts_monitor` 纯检测模块 + 16 个单测,挂载 V2/V3 全部数据帧路径 / Added a pure `pmusim-core::ts_monitor` detection module with 16 unit tests, wired into every V2/V3 data-frame path.

### Added 新增

- 新增 `pmusim-core::ts_monitor::TimestampMonitor`:纯检测逻辑,逐帧 `feed` 当前帧绝对毫秒与速率间隔 `period_ms`,返回 `TsReport`。异常分三类——`Backward` 回退(delta<0)/ `Gap` 跳变(间隔过大,丢帧)/ `Stall` 停滞(时标不前进)/ New `pmusim-core::ts_monitor::TimestampMonitor`: pure detection logic that is `feed` each frame's absolute milliseconds and the rate interval `period_ms`, returning a `TsReport`. Anomalies fall into three kinds — `Backward` (delta<0), `Gap` (interval too large → frame loss) and `Stall` (timestamp not advancing).
- 检测挂载主站 V2 `handle_data_connection` 与 V3 `data_read_loop_outbound` 的全部数据帧路径,逐帧校验后复用 Error 事件曝光异常报文 / The detector is mounted on every data-frame path — V2 `handle_data_connection` and V3 `data_read_loop_outbound` — checking frame-by-frame and reusing the Error event to expose the offending frame.

### Changed 改进

- `fracsec_to_ms` 无条件屏蔽 FRACSEC 高 8 位时标质量码(对齐前端 `rate.ts`),修复 V2 质量位翻转造成的误报 / `fracsec_to_ms` now unconditionally masks the high 8 FRACSEC bits (time-quality), aligning with the frontend `rate.ts` and fixing false positives from V2 quality-bit flips.
- 容差改为纯比例 `expected*0.5`(修高数据率盲区);速率切换过渡帧跳过;V3 改为每帧实时取当前间隔(修实时改速率后持续误报);小幅回退也判定为 `Backward` / Tolerance is now a pure ratio `expected*0.5` (fixing the high-rate blind spot); rate-switch transition frames are skipped; V3 reads the current interval live per frame (fixing sustained false positives after a live rate change); small backward jumps are also classified as `Backward`.

### Tests 测试

- `crates/pmusim-core/src/ts_monitor.rs` 新增 16 个单测,覆盖回退 / 跳变 / 停滞三类判定、容差边界、速率切换过渡、FRACSEC 质量位屏蔽等场景 / Added 16 unit tests in `crates/pmusim-core/src/ts_monitor.rs`, covering the backward / gap / stall classifications, tolerance boundaries, rate-switch transitions and FRACSEC quality-bit masking.

## [0.9.0] - 2026-06-16

### Highlights / 亮点

- 🎛️ 主站速率下拉新增「10 Hz」合法档位:补齐 IEEE C37.118 对 50Hz 系统的标准上送率,从此下拉不再从 25Hz 直接跳到 200Hz,常用的低速率一键可选 / New "10 Hz" legal entry in the master's rate dropdown: it fills in the standard IEEE C37.118 reporting rate for 50 Hz systems, so the dropdown no longer jumps straight from 25 Hz to 200 Hz — the common low rate is now one pick away.
- ⚙️ 与 25/50/100/200 同等走正常路径:选中即下发 CFG-2(PERIOD=500),无确认框、无「异常」标签,readback 回显 `(10.0Hz)`;后端/watch/readback/i18n/默认档位零改动,纯靠既有链路 / It flows through the normal path exactly like 25/50/100/200: picking it pushes a CFG-2 (PERIOD=500) with no confirm dialog and no "abnormal" tag, and the readback shows `(10.0Hz)`. Backend, watcher, readback, i18n and the default rate are all unchanged — it rides the existing path.
- 🧪 新增对称无头组件回归测试,全量前端测试 12/12 通过 / Added a symmetric headless component regression test; the full frontend suite is 12/12 green.

### Added 新增

- 主站速率下拉新增 `10 Hz` 合法档位,置于最前(顺序 `10 / 25 / 50 / 100 / 200 / 0(异常)`),映射 CFG-2 `PERIOD=500`(`hzToPeriod(10)=round(5000/10)=500`)。10Hz 落入既有正常档位的 `applyNormalRate` 防抖路径:streaming 时实时下发 CFG-2,未连接时由后续握手带下去,不触发 0Hz 的异常确认框 / New `10 Hz` legal entry in the master rate dropdown, placed first (order `10 / 25 / 50 / 100 / 200 / 0(abnormal)`), mapping to CFG-2 `PERIOD=500` (`hzToPeriod(10)=round(5000/10)=500`). 10 Hz falls into the existing normal-rate `applyNormalRate` debounce path: it pushes a CFG-2 live while streaming and is carried into the next handshake while disconnected, never triggering the 0 Hz abnormal confirm dialog.

### Tests 测试

- 在既有 `frontend/tests/config-info-panel.0hz.test.ts` 追加一条 vitest 用例(复用其 `invoke`/`ask` mock 与脚手架,DRY):streaming 时选 10Hz → 不弹确认框,且下发 `send_cfg2_cmd(period:null)` + `send_cfg2(period:500)`;TDD 先红后绿,`vue-tsc` 通过,全量前端测试 12/12 通过 / Added one vitest case to the existing `frontend/tests/config-info-panel.0hz.test.ts` (reusing its `invoke`/`ask` mocks and scaffolding, DRY): selecting 10 Hz while streaming fires no confirm dialog and issues `send_cfg2_cmd(period:null)` + `send_cfg2(period:500)`; written test-first (red→green), `vue-tsc` passes, full frontend suite 12/12 green.

## [0.8.1] - 2026-06-15

### Highlights / 亮点

- 📊 「上传速率」改为按数据帧 SOC/FRACSEC 报文时间戳反推,而非浏览器墙钟到达时间:消除 webview 事件循环抖动与网络挤帧导致的虚高(100Hz 流不再显示 102),读数反映子站给帧打的真实时标速率 / The master's "upload rate" is now derived from each data frame's own SOC/FRACSEC timestamp instead of browser wall-clock arrival time, eliminating the inflation from webview event-loop jitter and network bunching (a 100 Hz stream no longer reads 102) — the figure reflects the rate the substation actually time-stamped its frames at.
- 🛡️ 报文时间倒退(子站重启 / GPS 校时 / SOC 回绕)自动重置滑窗,避免旧时间戳全落入新窗造成的瞬时虚高 / A backward jump in the frame timestamp (substation restart / GPS resync / SOC wrap) now resets the sliding window, preventing the transient spike stale timestamps would otherwise cause.
- 🧪 新增 7 例 vitest 覆盖报文时间换算与帧率滑窗 / Added 7 vitest cases covering the timestamp-to-ms conversion and the frame-rate sliding window.

### Added 新增

- `frontend/src/lib/rate.ts` 新增纯函数 `frameTimeMs(soc, fracsec, measRate)`:按 V3 §8.11 将 SOC/FRACSEC 换算为绝对毫秒,屏蔽 FRACSEC 高 8 位时标质量码,`measRate<=0` 退化整秒防除零 / New pure helper `frameTimeMs(soc, fracsec, measRate)` in `frontend/src/lib/rate.ts`: converts SOC/FRACSEC to absolute milliseconds per V3 §8.11, masking the high 8 FRACSEC bits (time-quality) and degrading to whole seconds when `measRate<=0` to avoid divide-by-zero.

### Changed 改进

- `useFrameRate.tick` 改为接收数据帧报文时间(ms)而非内部读 `performance.now()`;`usePmuEvents` 在 `DataFrame` 分支用 `frameTimeMs(soc, fracsec, measRate)` 计算时标并传入,`measRate` 取该 IDCODE 的 CFG `TIME_BASE`(缺省 1e6);滑窗逻辑增加报文时间倒退检测 → 重置,停流/断链归零路径不变 / `useFrameRate.tick` now takes the frame's timestamp (ms) instead of reading `performance.now()` itself; `usePmuEvents` computes it via `frameTimeMs(soc, fracsec, measRate)` in the `DataFrame` branch (`measRate` from that IDCODE's CFG `TIME_BASE`, default 1e6); the sliding window adds backward-timestamp detection → reset, while the stop/disconnect zeroing paths are unchanged.
- `ConfigInfoPanel` 最新时间戳显示复用同一 `frameTimeMs`,消除与帧率换算两份 fracsec→ms 逻辑漂移的风险 / `ConfigInfoPanel`'s latest-timestamp display now reuses the same `frameTimeMs`, removing the risk of the two fracsec→ms conversions drifting apart.

### Tests 测试

- 新增 `frontend/tests/use-frame-rate.test.ts`(vitest)7 场景(换算 / 高位屏蔽 / 除零 / 滑窗 / 旧帧剔除 / 倒退重置 / 清零),TDD 先红后绿;`vue-tsc` 类型检查通过,全量前端测试 11/11 通过 / Added `frontend/tests/use-frame-rate.test.ts` (vitest), 7 scenarios (conversion / high-bit masking / divide-by-zero / window / eviction / backward-reset / clear), written test-first (red→green); `vue-tsc` passes and the full frontend suite is 11/11 green.

## [0.8.0] - 2026-06-03

### Highlights / 亮点

- 🎛️ 主站速率下拉新增「0 Hz (异常场景)」一键档位:选中即注入非法上送周期 PERIOD=0(子站应 NACK),先弹原生确认框、取消自动回退,把最常用的异常场景从「勾选+手填」降为一次点选 / New "0 Hz (abnormal)" entry in the master's rate dropdown: picking it injects an illegal reporting period PERIOD=0 (the substation should NACK), guarded by a native confirm dialog with auto-revert on cancel — turning the most common abnormal case from "check a box + type a value" into a single pick.
- 🐛 修复 V2 双 IDCODE 子站握手中途丢会话致永不开流:真实网关 CFG-1 报站名标签、命令帧报真实通信 IDCODE,主站对两者都重命名会话致握手第 3 步找不到会话而中止;现每步按对端地址重解析,真机验证稳定开流 / Fixed a V2 dual-IDCODE substation losing its session mid-handshake and never streaming: a real gateway reports a name label in CFG-1 but its true comms IDCODE in command frames; the master re-keyed on both and the handshake aborted at step 3. The session is now re-resolved by peer address at every step — verified streaming stably on real hardware.
- 🧪 新增 0Hz 无头组件测试(vitest + happy-dom)4 场景 + V2 双 IDCODE 握手回归测试 / Added a 4-scenario headless component test for the 0 Hz flow (vitest + happy-dom) plus a V2 dual-IDCODE handshake regression test.

### Added 新增

- 主站速率下拉新增 `0 Hz (异常场景)` 档位,作为注入 PERIOD=0 的一键快捷入口:streaming 时选中并确认即实时下发 CFG-2(PERIOD=0),未连接时由后续「连接/启动」握手带下去;取消则回退上一档且不下发(`suppressRateWatch` 防回退误发);现有「异常注入」勾选区保留不动,后端零改动(复用既有 `send_cfg2` + 子站 NACK 链路)/ New `0 Hz (abnormal)` entry in the master rate dropdown as a one-click PERIOD=0 injector: while streaming, confirming it pushes a CFG-2 (PERIOD=0) live; while disconnected it's carried into the next connect/start handshake; cancelling reverts to the prior rate with no send (`suppressRateWatch` guards a stray resend). The existing "abnormal injection" area is untouched and the backend is unchanged (reuses the existing `send_cfg2` + substation-NACK path).
- 抽出纯函数 `frontend/src/lib/rate.ts` 的 `hzToPeriod`(0Hz 特判为 PERIOD=0,绕开 1000/hz 除零)+ 单测;新增 i18n 文案(`rateAbnormalTag`/`inject0Title`/`inject0Confirm`)与 `@tauri-apps/plugin-dialog` 依赖 / Extracted a pure `hzToPeriod` helper in `frontend/src/lib/rate.ts` (0 Hz special-cased to PERIOD=0, avoiding the 1000/hz division) with a unit test; added i18n strings (`rateAbnormalTag`/`inject0Title`/`inject0Confirm`) and the `@tauri-apps/plugin-dialog` dependency.
- 主站 `resolve_peer_idcode`:握手步骤 2~5 与跳过-CFG-2 的 OpenData 前,均按 `(peer_host, peer_port)` 重解析当前会话 IDCODE,扛任意次中途重命名;无头复现工具 `crates/pmusim-app/examples/v2_master_repro.rs` / Master `resolve_peer_idcode`: re-resolves the current session IDCODE by `(peer_host, peer_port)` before handshake steps 2–5 and the skip-CFG-2 OpenData, tolerating any number of mid-flight re-keys; plus a headless repro tool `crates/pmusim-app/examples/v2_master_repro.rs`.

### Fixed 修复

- V2 双 IDCODE 子站(CFG-1 报站名标签 `pmuTag`、命令帧报通信 IDCODE `q1234567`)握手中途被二次重命名,致 `install_ack_waiter` 找不到会话、报「CFG-2 帧: session 已消失」而中止、永不发 OpenData(连上无数据);修复后握手每步重解析会话,且 `mgmt_read_loop` 仅允许 config 帧给占位会话定身份、不覆盖命令帧已确立的真实 IDCODE,消除 label↔comms-id 横跳 / A V2 dual-IDCODE substation (name label `pmuTag` in CFG-1, comms IDCODE `q1234567` in command frames) was re-keyed twice mid-handshake, so `install_ack_waiter` lost the session, aborted with "CFG-2 frame: session gone", and never sent OpenData (connected but no data). The handshake now re-resolves the session at every step, and `mgmt_read_loop` only lets a config frame name a placeholder session (never overriding the real IDCODE established by a command frame) — eliminating the label↔comms-id flapping.

### Tests 测试

- 新增 `frontend/tests/config-info-panel.0hz.test.ts`(vitest + happy-dom + @vue/test-utils):真实挂载 `ConfigInfoPanel.vue`、mock `invoke`/`ask`,驱动速率下拉验证 0Hz watcher 四条路径(确认下发 PERIOD=0 / 取消回退不下发 / 未连接保持选中 / 正常档位防抖下发),4/4 通过 / Added `frontend/tests/config-info-panel.0hz.test.ts` (vitest + happy-dom + @vue/test-utils): mounts `ConfigInfoPanel.vue`, mocks `invoke`/`ask`, and drives the rate dropdown to verify the four 0 Hz watcher paths (confirm → PERIOD=0 sent / cancel → revert with no send / disconnected → stays selected / normal rate → debounced send) — 4/4 passing.
- 新增 V2 双 IDCODE 握手回归测试 `v2_dual_idcode_handshake_reaches_streaming`(修复前红、修复后绿)/ Added the V2 dual-IDCODE handshake regression test `v2_dual_idcode_handshake_reaches_streaming` (red before the fix, green after).

## [0.7.0] - 2026-06-02

### Highlights / 亮点

- 🧪 受控注入 V4:主站可「跳过 CFG-2」仅凭 CFG-1 直接开流,演示规约层面 CFG-2 并非开流的硬前置 / Controlled injection (V4): the master can **skip CFG-2** and start streaming using only CFG-1 — showing CFG-2 is not a hard prerequisite for data flow at the protocol level.
- 🛰️ 印证「子站无 CFG-2 也推流、主站凭 CFG-1 即可解码」:子站收到 OpenData 即推流(本无 CFG 门控),主站用 CFG-1 的维度成功解出相量/模拟量/数字量 / Demonstrates "substation streams without CFG-2, master decodes via CFG-1": the substation streams the moment it gets OpenData (no CFG gate), and the master decodes phasors/analogs/digitals from CFG-1 dimensions.
- 🖱️ 主站「异常注入」区新增「连接(跳过 CFG-2)」入口,作为正常「连接」之外的另一启动方式;事件区标注「跳过 CFG-2,仅凭 CFG-1 开流」 / New "Connect, skip CFG-2" entry in the master's injection area as an alternative to the normal connect; the event log marks "skipped CFG-2, streaming via CFG-1 only".

### Added 新增

- 主站新增 `skip_cfg2_open` 命令与握手 `do_skip_cfg2_open`:召唤 CFG-1 → 跳过下传/召唤 CFG-2 → 直接 OpenData(走内部路径,天然绕过手动 OpenData 的握手门控,不改门控、不加状态) / Master gains the `skip_cfg2_open` command and `do_skip_cfg2_open` handshake: request CFG-1 → skip all CFG-2 → OpenData directly (via the internal path, naturally bypassing the manual-OpenData gate; gate and session states unchanged).
- 新事件 `Cfg2Skipped`(后端 + 前端 types/事件展示/中英 i18n)作为跳过 CFG-2 的可见注入标记 / New `Cfg2Skipped` event (backend + frontend types / event-log display / 中英 i18n) as a visible injection marker for the skip.
- 集成测试 `v3_master_skips_cfg2_streams_via_cfg1`:断言全程无 CFG-2 交换、有 `Cfg2Skipped`,且凭 CFG-1 维度解出 `DataFrame` / Integration test `v3_master_skips_cfg2_streams_via_cfg1`: asserts no CFG-2 exchange occurs, `Cfg2Skipped` is emitted, and a `DataFrame` is decoded from CFG-1 dimensions.

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
