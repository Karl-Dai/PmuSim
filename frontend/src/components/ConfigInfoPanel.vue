<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useProtocol } from "../composables/useProtocol";
import { useServerStatus } from "../composables/useServerStatus";
import { useSessions } from "../composables/useSessions";
import { useCommLog } from "../composables/useCommLog";
import { useEventLog } from "../composables/useEventLog";
import { useFrameRate } from "../composables/useFrameRate";
import { useToast, toastError } from "../composables/useToast";
import { listenerReady } from "../composables/usePmuEvents";
import { useI18n } from "../i18n";

const { t } = useI18n();
const { protocol } = useProtocol();
const { running } = useServerStatus();
const { sessions, configs, selectedIdcode } = useSessions();
const { latestData } = useCommLog();
const { events } = useEventLog();
const { fps } = useFrameRate();
const { push: pushToast } = useToast();

// Debounce select changes so holding ↑/↓ doesn't fire one invoke per tick
// (rateHz path issues two CFG-2 commands; back-to-back could land out of
// order on the substation).
function debounced<T>(ms: number, fn: (v: T) => void | Promise<void>) {
  let t: ReturnType<typeof setTimeout> | null = null;
  return (v: T) => {
    if (t) clearTimeout(t);
    t = setTimeout(() => fn(v), ms);
  };
}

// Connection form (single substation — reference UI is single-target).
const connIp = ref("10.15.48.12");
const connMgmtPort = ref("8000");
const connDataPort = ref("8001");
const connDataPortDirty = ref(false);

// V3 default 8000/8001, V2 default 7000/7001. Reset data-port dirty flag on
// protocol switch so the user doesn't get stuck on a stale value.
watch(protocol, (p) => {
  connMgmtPort.value = p === "V2" ? "7000" : "8000";
  connDataPortDirty.value = false;
  connDataPort.value = p === "V2" ? "7001" : "8001";
});
watch(connMgmtPort, (v) => {
  if (!connDataPortDirty.value) {
    const m = parseInt(v);
    if (Number.isFinite(m)) connDataPort.value = String(m + 1);
  }
});
function onDataPortInput(e: Event) {
  connDataPortDirty.value = true;
  connDataPort.value = (e.target as HTMLInputElement).value;
}

// V2 (2006): this is the master's OWN local listening port — the master is the
// data-pipe server and binds it (mgmt+1 by default) for the substation to push
// to. V3 (2011): it's the REMOTE substation data port the master dials out to.
const dataPortLabel = computed(() =>
  protocol.value === "V2" ? t("config.localListenPort") : t("config.dataPort")
);

// Editable runtime params.
const rateHz = ref("100"); // PERIOD inverse → user-visible Hz
const heartbeatSecs = ref("5");

// 异常注入：勾选后用原始 PERIOD 值直发 CFG-2（允许 0），绕过 Hz→PERIOD 换算，
// 用于受控注入规约未定义的非法上送周期，验证子站 NACK 应对。
const injectAbnormal = ref(false);
const rawPeriod = ref("0");

async function injectPeriod() {
  const s = session.value;
  if (!s) return; // 按钮 disabled 已兜底
  if (s.state !== "streaming" && s.state !== "cfg2_sent") return;
  const p = parseInt(rawPeriod.value);
  if (!Number.isFinite(p) || p < 0 || p > 65535) {
    pushToast(t("config.injectBadValue"), "error");
    return;
  }
  try {
    await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2_cmd", period: null });
    await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2", period: p });
    pushToast(t("config.injectSent", { period: String(p) }), "info");
  } catch (e) {
    pushToast(t("config.injectFailed", { error: toastError(e) }), "error");
  }
}

// Selected session for IDCODE / status display.
const session = computed(() => sessions.get(selectedIdcode.value));
const cfg = computed(() => configs.get(selectedIdcode.value));

const idcodeDisplay = computed(() => session.value?.idcode ?? "");
const stateLabel = computed(() => {
  const s = session.value?.state;
  if (!s) return "";
  return t(`state.${s}`);
});
// 状态语义色：在线类绿、断开红、无会话不着色
const stateClass = computed(() => {
  const s = session.value?.state;
  if (!s) return "";
  if (s === "disconnected") return "st-err";
  if (s === "connecting") return "st-warn";
  return "st-ok";
});

// Latest data SOC → wall time string. Per V3 §8.11:
//   ms = FRACSEC_count / (MEAS_RATE / 1000)
// FRACSEC low 24 bits = sub-second count; high 8 bits = time quality
// (currently dropped here — TODO #9 in docs/TODO.md will expose them).
const latestTime = computed(() => {
  const d = latestData.value?.data;
  if (!d) return "—";
  const measRate = cfg.value?.measRate ?? 1_000_000;
  const fracsecCount = d.fracsec & 0xFFFFFF;
  const msOffset = measRate > 0 ? fracsecCount / (measRate / 1000) : 0;
  const ms = d.soc * 1000 + msOffset;
  const date = new Date(ms);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${date.getFullYear()}/${pad(date.getMonth() + 1)}/${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${pad(date.getMilliseconds()).padStart(3, "0")}`;
});

// Inverted PERIOD → Hz for display (read-only when CFG-2 has populated period).
const ratePeriodReadback = computed(() => {
  if (!cfg.value || !cfg.value.period) return "";
  // period_ms = (period/100) * (1000/fnom_base); we don't have fnom here,
  // assume 50Hz base. ms→Hz = 1000/ms.
  const periodMs = (cfg.value.period / 100) * (1000 / 50);
  if (periodMs <= 0) return "";
  return `${(1000 / periodMs).toFixed(1)}Hz`;
});

const busy = ref(false);

async function startEverything() {
  if (busy.value) return;
  busy.value = true;
  try {
    // Block until the Tauri event listener is attached. If start_server +
    // connect_substation fire before listen() round-trips,every handshake
    // event drops on the floor and the UI never updates (the original
    // 已断开/无事件 bug).
    await listenerReady;
    // 1. start server if not running
    if (!running.value) {
      const dataPort = protocol.value === "V3" ? 0 : parseInt(connDataPort.value);
      await invoke("start_server", { dataPort, protocol: protocol.value });
      running.value = true;
    }
    // 2. push heartbeat interval
    const hb = parseFloat(heartbeatSecs.value);
    if (Number.isFinite(hb) && hb > 0) {
      await invoke("set_heartbeat_interval", { seconds: hb });
    }
    // 3. connect substation if not already
    if (!session.value) {
      const mgmt = parseInt(connMgmtPort.value);
      const data = protocol.value === "V3" ? parseInt(connDataPort.value) : undefined;
      await invoke("connect_substation", { host: connIp.value.trim(), port: mgmt, dataPort: data });
    }
    // 4. auto handshake with the chosen PERIOD (PERIOD value = (cycles*100); Hz→cycles = 50/Hz)
    const hz = parseFloat(rateHz.value);
    let periodVal: number | null = null;
    if (Number.isFinite(hz) && hz > 0) {
      // period_ms = 1000/Hz; cycles = period_ms * 50/1000 = period_ms/20; PERIOD = cycles*100
      periodVal = Math.round((1000 / hz) * 100 / 20);
    }
    // We don't yet know the real idcode; auto_handshake resolves it from
    // peer (host:port) so the placeholder works.
    const target = session.value?.idcode ?? `${connIp.value.trim()}:${connMgmtPort.value}`;
    await invoke("auto_handshake", { idcode: target, period: periodVal });
  } catch (e) {
    pushToast(t("config.startFailed", { error: toastError(e) }), "error");
  } finally {
    busy.value = false;
  }
}

async function stopEverything() {
  if (busy.value) return;
  busy.value = true;
  try {
    await invoke("stop_server");
    running.value = false;
  } catch (e) {
    pushToast(t("config.stopFailed", { error: toastError(e) }), "error");
  } finally {
    busy.value = false;
  }
}

async function pauseData() {
  if (!session.value) return;
  try {
    await invoke("send_command", { idcode: session.value.idcode, cmd: "close_data", period: null });
  } catch (e) {
    pushToast(t("config.pauseFailed", { error: toastError(e) }), "error");
  }
}

// 触发 (CMD=0xA000 "联网触发") 已从 UI 移除 — 规约 §8 表 3 只给了命令
// 编码,没说子站收到后做什么,实测子站也不会返回任何明确状态(STAT
// bit11/bit3-0 应该翻转但 lab 子站不响应)。后端 send_command 的
// "trigger" 分支保留以备调试 / 未来重新启用。

// Heartbeat live update.
watch(heartbeatSecs, debounced<string>(250, async (v) => {
  if (!running.value) return;
  const hb = parseFloat(v);
  if (Number.isFinite(hb) && hb > 0) {
    try {
      await invoke("set_heartbeat_interval", { seconds: hb });
    } catch (e) {
      pushToast(t("config.heartbeatFailed", { error: toastError(e) }), "error");
    }
  }
}));

// Rate live update — push fresh CFG-2 to substation. Only valid once
// CFG-1 has been received (cfg2_sent / streaming); before then the
// initial auto_handshake will carry the chosen rate.
watch(rateHz, debounced<string>(250, async (v) => {
  const s = session.value;
  if (!s) return;
  if (s.state !== "streaming" && s.state !== "cfg2_sent") return;
  const hz = parseFloat(v);
  if (!Number.isFinite(hz) || hz <= 0) return;
  const periodVal = Math.round((1000 / hz) * 100 / 20);
  try {
    await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2_cmd", period: null });
    await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2", period: periodVal });
    pushToast(t("config.rateApplied", { hz: String(hz) }), "info");
  } catch (e) {
    pushToast(t("config.rateFailed", { error: toastError(e) }), "error");
  }
}));
</script>

<template>
  <div class="config-panel">
    <section class="panel">
      <div class="panel-hd">{{ t("config.title") }}</div>
      <div class="panel-bd">

      <div class="row"><label>{{ t("config.substationAddr") }}</label><input v-model="connIp" /></div>
      <div class="row"><label>{{ t("config.cmdPort") }}</label><input v-model="connMgmtPort" inputmode="numeric" /></div>
      <div class="row">
        <label>{{ dataPortLabel }}</label><input :value="connDataPort" @input="onDataPortInput" inputmode="numeric" />
      </div>
      <div class="row"><label>{{ t("config.idcode") }}</label><input :value="idcodeDisplay" readonly :placeholder="t('config.idcodePlaceholder')" /></div>
      <div class="row">
        <label>{{ t("config.rate") }}</label>
        <div class="ctl-with-suffix">
          <select v-model="rateHz">
            <option value="25">25 Hz</option>
            <option value="50">50 Hz</option>
            <option value="100">100 Hz</option>
            <option value="200">200 Hz</option>
          </select>
          <span class="readback">{{ ratePeriodReadback ? `(${ratePeriodReadback})` : "" }}</span>
        </div>
      </div>
      <div class="row">
        <label>{{ t("config.abnormalInject") }}</label>
        <input type="checkbox" v-model="injectAbnormal" />
      </div>
      <div class="row" v-if="injectAbnormal">
        <label>{{ t("config.rawPeriod") }}</label>
        <div class="ctl-with-suffix">
          <input v-model="rawPeriod" inputmode="numeric" style="width: 80px" />
          <button
            class="btn"
            @click="injectPeriod"
            :disabled="!session || (session.state !== 'streaming' && session.state !== 'cfg2_sent')"
          >{{ t("config.inject") }}</button>
        </div>
      </div>
      <div class="row">
        <label>{{ t("config.heartbeat") }}</label>
        <select v-model="heartbeatSecs">
          <option value="1">{{ t("config.seconds", { n: 1 }) }}</option>
          <option value="5">{{ t("config.seconds", { n: 5 }) }}</option>
          <option value="10">{{ t("config.seconds", { n: 10 }) }}</option>
          <option value="30">{{ t("config.seconds", { n: 30 }) }}</option>
        </select>
      </div>
      <div class="row"><label>{{ t("config.commProtocol") }}</label><input value="TCP" disabled /></div>
      <div class="row">
        <label>{{ t("config.protocol") }}</label>
        <select v-model="protocol" :disabled="running">
          <option value="V2">2006 (V2)</option>
          <option value="V3">2011 (V3)</option>
        </select>
      </div>

      <div class="btn-grid">
        <button class="btn" @click="startEverything" :disabled="busy || running"><span>{{ t("config.start") }}</span></button>
        <button class="btn" @click="stopEverything" :disabled="busy || !running"><span>{{ t("config.stop") }}</span></button>
        <button class="btn btn-wide" @click="pauseData" :disabled="!session || session.state !== 'streaming'"><span>{{ t("config.pause") }}</span></button>
      </div>

      <div class="readout">
        <div class="rd-row"><label>{{ t("config.status") }}</label><span class="rd-val" :class="stateClass">{{ stateLabel || "—" }}</span></div>
        <div class="rd-row"><label>{{ t("config.latestTime") }}</label><span class="rd-val mono">{{ latestTime }}</span></div>
        <div class="rd-row"><label>{{ t("config.uploadRate") }}</label><span class="rd-val mono">{{ fps }} <span class="unit">{{ t("config.fpsUnit") }}</span></span></div>
      </div>
      </div>
    </section>

    <section class="panel log-fs">
      <div class="panel-hd">{{ t("config.eventLog") }}</div>
      <div class="panel-bd log-bd">
        <div class="log-list">
          <div v-for="(e, i) in events" :key="i" :class="['log-line', e.kind]">
            <span class="log-time">{{ e.time }}</span>
            <span class="log-msg">{{ e.message }}</span>
          </div>
          <div v-if="events.length === 0" class="log-empty">{{ t("config.noEvents") }}</div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
/* Layout shell --------------------------------------------------------- */
.config-panel {
  width: 392px;
  min-width: 392px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px;
  overflow: hidden;
}
/* Panel = framed group with a label-plate header band. Replaces the old
   fieldset/legend (whose notched border + double top edge read as an
   un-styled browser default). Frame is now continuous; the title lives in
   its own tinted strip with a brand-blue nameplate tick on the left. */
.panel {
  border: 1px solid var(--border);
  background: var(--bg-panel);
  /* 机箱受光唇边 —— 顶部一道极细高光，下沉的实体感 */
  box-shadow: inset 0 1px 0 rgba(255,255,255,0.55);
}
/* 表头 = 拉丝金属铭牌：镜面渐变(中段高光带) + 顶部受光唇边 +
   底部机加工接缝(暗线 + .panel-bd 顶部亮线 = 双线浮雕)，标题蚀刻。 */
.panel-hd {
  position: relative;
  padding: 6px 10px 6px 14px;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 1.5px;
  color: #44443d;
  /* 顶层极细竖向拉丝纹理(2px 周期，低对比) 叠在压暗半档的阳极氧化
     深灰镜面渐变上 —— 两者不同轴，纹理不被宏观高光糊掉。 */
  background:
    repeating-linear-gradient(
      90deg,
      rgba(255,255,255,0.05) 0px,
      rgba(255,255,255,0.05) 1px,
      rgba(0,0,0,0.028) 1px,
      rgba(0,0,0,0.028) 2px
    ),
    linear-gradient(
      180deg,
      #e4e3d9 0%,
      #d4d3c8 44%,
      #cbcabf 56%,
      #bbbaae 100%
    );
  border-bottom: 1px solid var(--border);
  box-shadow:
    inset 0 1px 0 rgba(255,255,255,0.75),
    inset 0 -1px 1px rgba(0,0,0,0.07);
  text-shadow: 0 1px 0 rgba(255,255,255,0.6);
  user-select: none;
}
/* 品牌蓝竖纹 —— 嵌进金属面的铭牌指示纹，紧贴左边框 */
.panel-hd::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  background: linear-gradient(180deg, #3a78bd, var(--accent) 50%, var(--accent-dark));
  box-shadow: inset -1px 0 0 rgba(0,0,0,0.18), 1px 0 0 rgba(255,255,255,0.5);
}
.panel-bd {
  padding: 10px 12px 12px;
  /* 接缝下方的受光亮线，与表头底部暗线合成浮雕 */
  box-shadow: inset 0 1px 0 rgba(255,255,255,0.7);
}

/* Form row — pixel-aligned label : control ---------------------------- */
.row {
  display: grid;
  grid-template-columns: 84px 1fr;
  align-items: center;
  column-gap: 8px;
  margin: 5px 0;
  min-height: 24px;
}
/* When a row needs a trailing readback (e.g. 速率 → (100.0Hz)),
   nest the control + readback in this wrapper so they share the same
   1fr column and the box widths still align with other rows. */
.ctl-with-suffix {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
.ctl-with-suffix > select,
.ctl-with-suffix > input { flex: 1; min-width: 0; }
.row > label {
  text-align: right;
  color: var(--text-dim);
  font-size: 13px;
  line-height: 22px;
  user-select: none;
  white-space: nowrap;
}
/* Render the colon via ::after so spacing is consistent regardless of
   label length (CJK vs ASCII have different intrinsic widths). */
.row > label::after {
  content: ":";
  margin-left: 2px;
  color: var(--text-faint);
}

/* Inputs & selects share identical box metrics so vertical edges line
   up across every row. appearance:none kills the macOS native select
   chrome (rounded corners + chevron) that otherwise sits 1–2px taller
   than the input boxes. */
.row input,
.row select {
  width: 100%;
  height: 22px;
  padding: 0 6px;
  border: 1px solid var(--border);
  background: var(--bg-input);
  font-size: 13px;
  line-height: 20px;
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  color: var(--text);
  border-radius: 0;
  box-shadow: inset 1px 1px 0 rgba(0,0,0,0.04);
  outline: none;
}
.row input:focus,
.row select:focus {
  border-color: var(--accent);
  box-shadow: inset 0 0 0 1px rgba(37,99,168,0.35);
}
.row input::placeholder { color: #aaa; font-style: italic; }

/* Custom chevron — single SVG so disabled / enabled selects render
   identically across platforms. */
.row select {
  -webkit-appearance: none;
  -moz-appearance: none;
  appearance: none;
  padding-right: 22px;
  background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='10' height='6' viewBox='0 0 10 6'><path d='M0 0l5 6 5-6z' fill='%23555'/></svg>");
  background-repeat: no-repeat;
  background-position: right 7px center;
  background-size: 8px 5px;
  cursor: pointer;
}
.row input[readonly],
.row input:disabled,
.row select:disabled {
  background-color: var(--bg-disabled);
  color: var(--text-dim);
  cursor: not-allowed;
}
.row select:disabled {
  background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='10' height='6' viewBox='0 0 10 6'><path d='M0 0l5 6 5-6z' fill='%23999'/></svg>");
}

.readback {
  font-size: 11px;
  color: #7a7a72;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  min-width: 56px;
  text-align: right;
  white-space: nowrap;
}

/* Buttons — even CJK letter-spacing without dangling whitespace ------ */
.btn-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  margin-top: 14px;
}
/* 暂停按钮独占第二行,平衡 3 按钮布局(原 2x2 删去触发后) */
.btn-wide { grid-column: 1 / -1; }
.btn {
  height: 30px;
  border: 1px solid #7a7a72;
  background: linear-gradient(#f6f5ee, #d6d4c5);
  font-size: 14px;
  font-weight: 600;
  color: #2a2a2a;
  cursor: pointer;
  border-radius: 0;
  box-shadow: inset 0 1px 0 rgba(255,255,255,0.7),
              0 1px 0 rgba(0,0,0,0.05);
  display: flex;
  align-items: center;
  justify-content: center;
}
/* CJK 字距：在 span 上写 letter-spacing 并用 padding-left 抵消尾部空白，
   保证字符在按钮内视觉居中（letter-spacing 在最后一个字符后仍会加空）。 */
.btn > span {
  letter-spacing: 6px;
  padding-left: 6px;
}
.btn:hover:not(:disabled) {
  background: linear-gradient(#fffefb, #dedccd);
  border-color: var(--accent);
}
.btn:active:not(:disabled) {
  background: linear-gradient(#c4c2b3, #aaa89a);
  box-shadow: inset 0 1px 2px rgba(0,0,0,0.2);
}
.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  color: #888;
}

/* Readout — same label grid as form rows so columns line up ---------- */
.readout {
  margin-top: 12px;
  padding-top: 10px;
  border-top: 1px dashed var(--border-dash);
}
.rd-row {
  display: grid;
  grid-template-columns: 84px 1fr;
  column-gap: 8px;
  align-items: baseline;
  margin: 4px 0;
}
.rd-row > label {
  text-align: right;
  color: var(--text-dim);
  font-size: 13px;
}
.rd-row > label::after {
  content: ":";
  margin-left: 2px;
  color: var(--text-faint);
}
.rd-val {
  font-size: 13px;
  color: var(--text);
  font-variant-numeric: tabular-nums;
}
/* 状态读数语义色：在线绿 / 断开红 */
.rd-val.st-ok { color: var(--ok); font-weight: 600; }
.rd-val.st-warn { color: var(--warn); font-weight: 600; }
.rd-val.st-err { color: var(--err); font-weight: 600; }
.rd-val .unit {
  color: var(--text-faint);
  font-family: -apple-system, "PingFang SC", "Microsoft YaHei", sans-serif;
  margin-left: 2px;
  font-size: 12px;
}
.mono { font-family: ui-monospace, "SF Mono", Menlo, monospace; }

/* Event log ---------------------------------------------------------- */
.log-fs {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  min-height: 120px;
}
/* header band is fixed-height; body flexes to fill the panel so the log
   list can scroll within it */
.log-bd {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.log-list {
  flex: 1;
  overflow: auto;
  background: var(--bg-content);
  border: 1px solid var(--border-dash);
  box-shadow: inset 1px 1px 0 rgba(0,0,0,0.04);
  padding: 4px 6px;
  font-size: 12px;
  line-height: 1.5;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
}
.log-line {
  padding: 1px 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: #333;
}
.log-line.error { color: var(--err); }
.log-time { color: var(--text-faint); margin-right: 8px; font-variant-numeric: tabular-nums; }
.log-empty {
  color: #aaa;
  text-align: center;
  padding: 12px;
  font-style: italic;
}
</style>
