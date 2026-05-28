<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useSessions } from "../composables/useSessions";
import { useToast, toastError } from "../composables/useToast";
import { useProtocol } from "../composables/useProtocol";
import { useServerStatus } from "../composables/useServerStatus";

const { sessions, selectedIdcode, removeSession } = useSessions();
const { push: pushToast } = useToast();
const { protocol } = useProtocol();
const { running } = useServerStatus();
const connIp = ref("127.0.0.1");
const connMgmtPort = ref("8000");
const connDataPort = ref("8001");
const connDataPortDirty = ref(false);
const period = ref("");
const busy = ref(false);

const stationList = computed(() => Array.from(sessions.values()));

// 协议切换时联动 mgmt + data 默认值 — 切协议视为新的配置上下文,
// 复位 dirty 让数据端口重新跟随 mgmt+1
watch(protocol, (p) => {
  connMgmtPort.value = p === "V2" ? "7000" : "8000";
  connDataPortDirty.value = false;
  connDataPort.value = p === "V2" ? "7001" : "8001";
});

// 命令端口手动改时,数据端口自动跟随 mgmt+1(除非用户编辑过 data)
watch(connMgmtPort, (newMgmt) => {
  if (!connDataPortDirty.value) {
    const m = parseInt(newMgmt);
    if (Number.isFinite(m)) connDataPort.value = String(m + 1);
  }
});

function onDataPortInput(e: Event) {
  // 用户手动编辑 → 锁定为用户值
  connDataPortDirty.value = true;
  connDataPort.value = (e.target as HTMLInputElement).value;
}

function selectStation(idcode: string) {
  selectedIdcode.value = idcode;
}

async function connect() {
  if (busy.value) return;
  if (!running.value) {
    // Without this guard, clicking 连接 before 启动 fires a connect_substation
    // command that the Rust side rejects with "Server not running" — once per
    // click. Easy to misread as a network problem.
    pushToast("请先点击工具栏「启动」", "error");
    return;
  }
  busy.value = true;
  const host = connIp.value;
  const mgmt = parseInt(connMgmtPort.value);
  const data = protocol.value === "V3" ? parseInt(connDataPort.value) : undefined;
  const target = `${host}:${mgmt}`;
  const p = period.value ? parseInt(period.value) : null;
  try {
    await invoke("connect_substation", { host, port: mgmt, dataPort: data });
    await invoke("auto_handshake", { idcode: target, period: p });
  } catch (e) {
    pushToast(`连接失败: ${toastError(e)}`, "error");
  } finally {
    busy.value = false;
  }
}

async function disconnect() {
  if (!selectedIdcode.value) return;
  const id = selectedIdcode.value;
  try {
    await invoke("disconnect_substation", { idcode: id });
    removeSession(id);
    pushToast(`已断开 ${id}`, "info");
  } catch (e) {
    pushToast(`断开失败: ${toastError(e)}`, "error");
  }
}

async function sendCmd(cmd: string) {
  if (!selectedIdcode.value) {
    pushToast("请先选择一个子站", "error");
    return;
  }
  const p = period.value ? parseInt(period.value) : null;
  try {
    if (cmd === "auto_handshake") {
      await invoke("auto_handshake", { idcode: selectedIdcode.value, period: p });
    } else {
      await invoke("send_command", { idcode: selectedIdcode.value, cmd, period: p });
    }
  } catch (e) {
    pushToast(`命令 ${cmd} 失败: ${toastError(e)}`, "error");
  }
}
</script>

<template>
  <div class="station-panel">
    <div class="section-title">子站列表</div>
    <div class="station-list">
      <div v-for="s in stationList" :key="s.idcode"
           :class="['station-item', { selected: s.idcode === selectedIdcode }]"
           @click="selectStation(s.idcode)">
        {{ s.idcode }} <span class="badge">{{ s.state }}</span>
      </div>
      <div v-if="stationList.length === 0" class="empty">无子站</div>
    </div>

    <fieldset>
      <legend>连接子站</legend>
      <div class="form-row"><label>IP:</label><input v-model="connIp" style="width:110px" /></div>
      <div class="form-row"><label>命令端口:</label><input v-model="connMgmtPort" style="width:60px" /></div>
      <div class="form-row" v-if="protocol === 'V3'">
        <label>数据端口:</label>
        <input :value="connDataPort" @input="onDataPortInput" style="width:60px" :placeholder="String(parseInt(connMgmtPort) + 1)" />
      </div>
      <button class="full-btn" :disabled="busy || !running" @click="connect" :title="!running ? '请先点击工具栏「启动」' : ''">
        {{ busy ? '连接中…' : (running ? '连接' : '连接 (未启动)') }}
      </button>
      <button class="full-btn" :disabled="!selectedIdcode" @click="disconnect">断开所选</button>
    </fieldset>

    <fieldset>
      <legend>操作</legend>
      <button class="full-btn" @click="sendCmd('request_cfg1')">召唤CFG-1</button>
      <button class="full-btn" @click="sendCmd('send_cfg2_cmd')">下传CFG-2命令</button>
      <button class="full-btn" @click="sendCmd('send_cfg2')">下传CFG-2</button>
      <button class="full-btn" @click="sendCmd('request_cfg2')">召唤CFG-2</button>
      <button class="full-btn" @click="sendCmd('open_data')">开启数据</button>
      <button class="full-btn" @click="sendCmd('close_data')">关闭数据</button>
      <hr />
      <div class="form-row"><label>PERIOD:</label><input v-model="period" style="width:50px" /><span style="color:#999;font-size:11px">空=沿用</span></div>
      <button class="full-btn" @click="sendCmd('auto_handshake')">一键握手</button>
    </fieldset>
  </div>
</template>

<style scoped>
.station-panel { width: 220px; min-width: 220px; display: flex; flex-direction: column; gap: 4px; overflow-y: auto; }
.section-title { font-weight: 600; padding: 4px 0; text-align: center; }
.station-list { border: 1px solid #ccc; border-radius: 3px; background: white; min-height: 120px; overflow-y: auto; }
.station-item { padding: 4px 8px; cursor: pointer; border-bottom: 1px solid #eee; }
.station-item.selected { background: #0078d7; color: white; }
.badge { font-size: 11px; color: #888; }
.selected .badge { color: #cce; }
.empty { padding: 8px; color: #999; text-align: center; }
fieldset { border: 1px solid #ccc; border-radius: 3px; padding: 6px; }
legend { font-size: 12px; color: #555; padding: 0 4px; }
.form-row { display: flex; align-items: center; gap: 4px; margin: 2px 0; }
.form-row label { min-width: 40px; color: #555; }
.form-row input { padding: 2px 4px; border: 1px solid #bbb; border-radius: 3px; }
.full-btn { width: 100%; padding: 4px; margin: 2px 0; border: 1px solid #bbb; border-radius: 3px; background: #e8e8e8; cursor: pointer; }
.full-btn:hover:not(:disabled) { background: #ddd; }
.full-btn:disabled { opacity: 0.5; cursor: not-allowed; }
hr { border: none; border-top: 1px solid #ddd; margin: 4px 0; }
</style>
