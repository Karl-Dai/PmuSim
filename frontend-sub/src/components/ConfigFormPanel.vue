<script setup lang="ts">
import { reactive, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { ConfigInput } from "../types";
import { running, listenPorts } from "../composables/useSubEvents";

const form = reactive<ConfigInput>({
  protocol: "V3", idcode: "0GX00GP1", stn: "测试子站",
  mgmt_port: 8000, data_port: 0, data_rate_fps: 50,
  phasors: [{ magnitude: 1000, phase_deg: 0 }],
  analogs: [300, 3000], digitals: [10],
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
  if (form.protocol === "V2" && form.mgmt_port === 8000) form.mgmt_port = 7000;
  if (form.protocol === "V3" && form.mgmt_port === 7000) form.mgmt_port = 8000;
  await invoke("start_substation", { config: { ...form } });
  running.value = true;
}
async function stop() { await invoke("stop_substation"); running.value = false; }
async function apply() { await invoke("update_config", { config: { ...form } }); }
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
