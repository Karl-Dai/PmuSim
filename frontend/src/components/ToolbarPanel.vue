<script setup lang="ts">
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useToast, toastError } from "../composables/useToast";
import { useProtocol } from "../composables/useProtocol";

const { push: pushToast } = useToast();
const { protocol } = useProtocol();

// V2 only: master 本地侦听端口(子站会主动连这里上送数据)
const localListenPort = ref("8001");
const running = ref(false);

watch(protocol, (p) => {
  // V2 默认 7001, V3 默认 8001(虽然 V3 不会用,但留个合理值)
  localListenPort.value = p === "V2" ? "7001" : "8001";
});

async function start() {
  try {
    // V3 模式后端会忽略 data_port,但 Tauri 命令签名仍要求 u16,传 0 不通过
    // 这里照旧传值即可 — 后端 start() 按 protocol 分支处理
    const dataPort = protocol.value === "V3" ? 0 : parseInt(localListenPort.value);
    await invoke("start_server", { dataPort, protocol: protocol.value });
    running.value = true;
    pushToast(
      protocol.value === "V3"
        ? `已启动 (V3, 数据走 master-outbound)`
        : `已启动 (V2, 本地侦听端口 ${localListenPort.value})`,
      "success",
    );
  } catch (e) {
    pushToast(`启动失败: ${toastError(e)}`, "error");
  }
}

async function stop() {
  try {
    await invoke("stop_server");
    running.value = false;
    pushToast("已停止", "info");
  } catch (e) {
    pushToast(`停止失败: ${toastError(e)}`, "error");
  }
}
</script>

<template>
  <div class="toolbar">
    <button @click="start" :disabled="running">&#9654; 启动</button>
    <button @click="stop" :disabled="!running">&#9632; 停止</button>
    <span class="sep"></span>
    <label>协议:</label>
    <select v-model="protocol" :disabled="running">
      <option>V2</option>
      <option>V3</option>
    </select>
    <template v-if="protocol === 'V2'">
      <span class="sep"></span>
      <label>本地侦听端口:</label>
      <input v-model="localListenPort" type="text" style="width: 70px" :disabled="running" />
    </template>
  </div>
</template>

<style scoped>
.toolbar { display: flex; align-items: center; gap: 6px; padding: 6px 8px; background: #e8e8e8; border-bottom: 1px solid #ccc; }
.toolbar button { padding: 4px 12px; border: 1px solid #bbb; border-radius: 3px; background: #ddd; cursor: pointer; }
.toolbar button:disabled { opacity: 0.5; cursor: default; }
.toolbar input, .toolbar select { padding: 2px 4px; border: 1px solid #bbb; border-radius: 3px; }
.toolbar select:disabled, .toolbar input:disabled { opacity: 0.6; background: #f5f5f5; }
.sep { width: 1px; height: 20px; background: #bbb; margin: 0 4px; }
label { color: #555; }
</style>
