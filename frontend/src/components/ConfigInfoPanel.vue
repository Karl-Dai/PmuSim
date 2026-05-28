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

      <div class="row"><label>子站地址:</label><input v-model="connIp" /></div>
      <div class="row"><label>命令端口:</label><input v-model="connMgmtPort" /></div>
      <div class="row" v-if="protocol === 'V3'">
        <label>数据端口:</label><input :value="connDataPort" @input="onDataPortInput" />
      </div>
      <div class="row"><label>IDCODE:</label><input :value="idcodeDisplay" readonly :class="{ readonly: true }" /></div>
      <div class="row">
        <label>速率:</label>
        <select v-model="rateHz">
          <option value="25">25 Hz</option>
          <option value="50">50 Hz</option>
          <option value="100">100 Hz</option>
          <option value="200">200 Hz</option>
        </select>
        <span class="readback" v-if="ratePeriodReadback">({{ ratePeriodReadback }})</span>
      </div>
      <div class="row">
        <label>心跳间隔:</label>
        <select v-model="heartbeatSecs">
          <option value="1">1 秒</option>
          <option value="5">5 秒</option>
          <option value="10">10 秒</option>
          <option value="30">30 秒</option>
        </select>
      </div>
      <div class="row"><label>通讯协议:</label><input value="TCP" disabled /></div>
      <div class="row">
        <label>规约协议:</label>
        <select v-model="protocol" :disabled="running">
          <option value="V2">2006 (V2)</option>
          <option value="V3">2011 (V3)</option>
        </select>
      </div>

      <div class="btn-grid">
        <button class="btn" @click="startEverything" :disabled="busy || running">开 始</button>
        <button class="btn" @click="stopEverything" :disabled="busy || !running">停 止</button>
        <button class="btn" @click="pauseData" :disabled="!session || session.state !== 'streaming'">暂 停</button>
        <button class="btn" @click="triggerCmd" :disabled="!session">触 发</button>
      </div>

      <div class="readout">
        <div class="rd-row"><label>状态:</label><span>{{ stateLabel || "—" }}</span></div>
        <div class="rd-row"><label>最新时间:</label><span class="mono">{{ latestTime }}</span></div>
        <div class="rd-row"><label>上传速率:</label><span class="mono">{{ fps }} 帧/秒</span></div>
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
.config-panel {
  width: 380px;
  min-width: 380px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 6px;
  overflow: hidden;
}
fieldset {
  border: 1px solid #888;
  border-radius: 0;
  padding: 8px 10px 10px 10px;
  background: #f0f0f0;
}
fieldset legend { padding: 0 6px; font-weight: 600; color: #333; }

.row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 4px 0;
}
.row label {
  width: 78px;
  text-align: right;
  color: #444;
  font-size: 13px;
}
.row input, .row select {
  flex: 1;
  padding: 3px 4px;
  border: 1px solid #888;
  background: white;
  font-size: 13px;
  font-family: ui-monospace, Menlo, monospace;
}
.row input.readonly, .row input[readonly], .row input:disabled, .row select:disabled {
  background: #e8e8e8;
  color: #555;
}
.row .readback {
  font-size: 11px;
  color: #777;
  margin-left: 4px;
  font-family: ui-monospace, Menlo, monospace;
}

.btn-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 6px;
  margin-top: 10px;
}
.btn {
  padding: 7px 0;
  border: 1px solid #888;
  background: linear-gradient(#f4f4f4, #d8d8d8);
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  letter-spacing: 4px;
}
.btn:hover:not(:disabled) { background: linear-gradient(#fff, #e0e0e0); }
.btn:active:not(:disabled) { background: linear-gradient(#d0d0d0, #b8b8b8); }
.btn:disabled { opacity: 0.45; cursor: not-allowed; }

.readout {
  margin-top: 10px;
  padding-top: 8px;
  border-top: 1px dashed #aaa;
}
.rd-row { display: flex; gap: 6px; margin: 3px 0; }
.rd-row label { width: 78px; text-align: right; color: #444; font-size: 13px; }
.rd-row span { font-size: 13px; color: #222; }
.mono { font-family: ui-monospace, Menlo, monospace; }

.log-fs { flex: 1; overflow: hidden; display: flex; flex-direction: column; }
.log-list {
  flex: 1;
  overflow: auto;
  background: white;
  border: 1px solid #ccc;
  padding: 4px 6px;
  font-size: 12px;
  font-family: ui-monospace, Menlo, monospace;
}
.log-line { padding: 1px 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.log-line.error { color: #b91c1c; }
.log-time { color: #666; margin-right: 6px; }
.log-empty { color: #999; text-align: center; padding: 8px; }
</style>
