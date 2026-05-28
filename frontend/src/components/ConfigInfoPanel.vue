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

const { protocol } = useProtocol();
const { running } = useServerStatus();
const { sessions, configs, selectedIdcode } = useSessions();
const { latestData } = useCommLog();
const { events } = useEventLog();
const { fps } = useFrameRate();
const { push: pushToast } = useToast();

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

// Editable runtime params.
const rateHz = ref("100"); // PERIOD inverse → user-visible Hz
const heartbeatSecs = ref("5");

// Selected session for IDCODE / status display.
const session = computed(() => sessions.get(selectedIdcode.value));
const cfg = computed(() => configs.get(selectedIdcode.value));

const idcodeDisplay = computed(() => session.value?.idcode ?? "");
const stateLabel = computed(() => {
  const s = session.value?.state;
  return {
    connected: "已连接",
    cfg1_received: "已收 CFG-1",
    cfg2_sent: "已下传 CFG-2",
    streaming: "正在接收",
    disconnected: "已断开",
  }[s ?? "disconnected"] ?? "";
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
    pushToast(`启动失败: ${toastError(e)}`, "error");
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
    pushToast(`停止失败: ${toastError(e)}`, "error");
  } finally {
    busy.value = false;
  }
}

async function pauseData() {
  if (!session.value) return;
  try {
    await invoke("send_command", { idcode: session.value.idcode, cmd: "close_data", period: null });
  } catch (e) {
    pushToast(`暂停失败: ${toastError(e)}`, "error");
  }
}

async function triggerCmd() {
  if (!session.value) return;
  try {
    await invoke("send_command", { idcode: session.value.idcode, cmd: "trigger", period: null });
  } catch (e) {
    pushToast(`触发失败: ${toastError(e)}`, "error");
  }
}

// Heartbeat live update.
watch(heartbeatSecs, async (v) => {
  if (!running.value) return;
  const hb = parseFloat(v);
  if (Number.isFinite(hb) && hb > 0) {
    try {
      await invoke("set_heartbeat_interval", { seconds: hb });
    } catch (e) {
      pushToast(`心跳间隔修改失败: ${toastError(e)}`, "error");
    }
  }
});

// Rate live update — push fresh CFG-2 to substation. Only valid once
// CFG-1 has been received (cfg2_sent / streaming); before then the
// initial auto_handshake will carry the chosen rate.
watch(rateHz, async (v) => {
  const s = session.value;
  if (!s) return;
  if (s.state !== "streaming" && s.state !== "cfg2_sent") return;
  const hz = parseFloat(v);
  if (!Number.isFinite(hz) || hz <= 0) return;
  const periodVal = Math.round((1000 / hz) * 100 / 20);
  try {
    await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2_cmd", period: null });
    await invoke("send_command", { idcode: s.idcode, cmd: "send_cfg2", period: periodVal });
    pushToast(`已下发新速率 ${hz}Hz`, "info");
  } catch (e) {
    pushToast(`修改速率失败: ${toastError(e)}`, "error");
  }
});
</script>

<template>
  <div class="config-panel">
    <fieldset>
      <legend>配置及运行信息</legend>

      <div class="row"><label>子站地址</label><input v-model="connIp" /></div>
      <div class="row"><label>命令端口</label><input v-model="connMgmtPort" inputmode="numeric" /></div>
      <div class="row" v-if="protocol === 'V3'">
        <label>数据端口</label><input :value="connDataPort" @input="onDataPortInput" inputmode="numeric" />
      </div>
      <div class="row"><label>IDCODE</label><input :value="idcodeDisplay" readonly placeholder="待握手" /></div>
      <div class="row">
        <label>速率</label>
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
        <label>心跳间隔</label>
        <select v-model="heartbeatSecs">
          <option value="1">1 秒</option>
          <option value="5">5 秒</option>
          <option value="10">10 秒</option>
          <option value="30">30 秒</option>
        </select>
      </div>
      <div class="row"><label>通讯协议</label><input value="TCP" disabled /></div>
      <div class="row">
        <label>规约协议</label>
        <select v-model="protocol" :disabled="running">
          <option value="V2">2006 (V2)</option>
          <option value="V3">2011 (V3)</option>
        </select>
      </div>

      <div class="btn-grid">
        <button class="btn" @click="startEverything" :disabled="busy || running"><span>开始</span></button>
        <button class="btn" @click="stopEverything" :disabled="busy || !running"><span>停止</span></button>
        <button class="btn" @click="pauseData" :disabled="!session || session.state !== 'streaming'"><span>暂停</span></button>
        <button class="btn" @click="triggerCmd" :disabled="!session"><span>触发</span></button>
      </div>

      <div class="readout">
        <div class="rd-row"><label>状态</label><span class="rd-val">{{ stateLabel || "—" }}</span></div>
        <div class="rd-row"><label>最新时间</label><span class="rd-val mono">{{ latestTime }}</span></div>
        <div class="rd-row"><label>上传速率</label><span class="rd-val mono">{{ fps }} <span class="unit">帧/秒</span></span></div>
      </div>
    </fieldset>

    <fieldset class="log-fs">
      <legend>事件日志</legend>
      <div class="log-list">
        <div v-for="(e, i) in events" :key="i" :class="['log-line', e.kind]">
          <span class="log-time">{{ e.time }}</span>
          <span class="log-msg">{{ e.message }}</span>
        </div>
        <div v-if="events.length === 0" class="log-empty">无事件</div>
      </div>
    </fieldset>
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
fieldset {
  border: 1px solid #8a8a82;
  border-radius: 0;
  padding: 10px 12px 12px;
  background: #f2f1ea;
  box-shadow: inset 0 1px 0 rgba(255,255,255,0.6);
}
fieldset legend {
  padding: 0 6px;
  font-weight: 600;
  color: #333;
  letter-spacing: 0.5px;
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
  color: #444;
  font-size: 13px;
  line-height: 22px;
  user-select: none;
}
/* Render the colon via ::after so spacing is consistent regardless of
   label length (CJK vs ASCII have different intrinsic widths). */
.row > label::after {
  content: ":";
  margin-left: 2px;
  color: #777;
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
  border: 1px solid #8a8a82;
  background: #fff;
  font-size: 13px;
  line-height: 20px;
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  color: #1a1a1a;
  border-radius: 0;
  box-shadow: inset 1px 1px 0 rgba(0,0,0,0.04);
  outline: none;
}
.row input:focus,
.row select:focus {
  border-color: #4178c7;
  box-shadow: inset 0 0 0 1px rgba(65,120,199,0.35);
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
  background-color: #e6e5dd;
  color: #666;
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
  border-color: #4178c7;
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
  border-top: 1px dashed #b5b5ad;
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
  color: #555;
  font-size: 13px;
}
.rd-row > label::after {
  content: ":";
  margin-left: 2px;
  color: #888;
}
.rd-val {
  font-size: 13px;
  color: #1a1a1a;
  font-variant-numeric: tabular-nums;
}
.rd-val .unit {
  color: #777;
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
.log-list {
  flex: 1;
  overflow: auto;
  background: #fffef8;
  border: 1px solid #b5b5ad;
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
.log-line.error { color: #b91c1c; }
.log-time { color: #888; margin-right: 8px; font-variant-numeric: tabular-nums; }
.log-empty {
  color: #aaa;
  text-align: center;
  padding: 12px;
  font-style: italic;
}
</style>
