<script setup lang="ts">
import { onMounted, ref } from "vue";
import { usePmuEvents } from "./composables/usePmuEvents";
import { useSessions } from "./composables/useSessions";
import { useToast } from "./composables/useToast";
import ToolbarPanel from "./components/ToolbarPanel.vue";
import StationListPanel from "./components/StationListPanel.vue";
import ConfigTab from "./components/ConfigTab.vue";
import DataTab from "./components/DataTab.vue";
import LogTab from "./components/LogTab.vue";

const { startListening } = usePmuEvents();
const { sessions } = useSessions();
const { toasts, dismiss } = useToast();
const activeTab = ref("config");

onMounted(() => {
  startListening();
});
</script>

<template>
  <div class="app">
    <ToolbarPanel />
    <div class="content">
      <StationListPanel />
      <div class="main-panel">
        <div class="tabs">
          <button :class="{ active: activeTab === 'config' }" @click="activeTab = 'config'">配置</button>
          <button :class="{ active: activeTab === 'data' }" @click="activeTab = 'data'">实时数据</button>
          <button :class="{ active: activeTab === 'log' }" @click="activeTab = 'log'">通信日志</button>
        </div>
        <div class="tab-content">
          <ConfigTab v-if="activeTab === 'config'" />
          <DataTab v-if="activeTab === 'data'" />
          <LogTab v-if="activeTab === 'log'" />
        </div>
      </div>
    </div>
    <div class="status-bar">已连接子站: {{ sessions.size }}</div>

    <div class="toasts" aria-live="polite">
      <div v-for="t in toasts" :key="t.id" :class="['toast', `toast-${t.kind}`]" @click="dismiss(t.id)">
        {{ t.message }}
      </div>
    </div>
  </div>
</template>

<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; font-size: 13px; }
.app { display: flex; flex-direction: column; height: 100vh; background: #f5f5f5; }
.content { display: flex; flex: 1; overflow: hidden; padding: 4px; gap: 4px; }
.main-panel { flex: 1; display: flex; flex-direction: column; background: white; border-radius: 4px; overflow: hidden; }
.tabs { display: flex; border-bottom: 1px solid #ddd; background: #fafafa; }
.tabs button { padding: 8px 16px; border: none; background: none; cursor: pointer; border-bottom: 2px solid transparent; }
.tabs button.active { border-bottom-color: #0078d7; color: #0078d7; font-weight: 600; }
.tab-content { flex: 1; overflow: auto; padding: 8px; }
.status-bar { padding: 4px 8px; background: #e8e8e8; border-top: 1px solid #ccc; font-size: 12px; color: #666; }

.toasts {
  position: fixed;
  right: 16px;
  bottom: 32px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  z-index: 1000;
  max-width: 360px;
  pointer-events: none;
}
.toast {
  pointer-events: auto;
  padding: 8px 12px;
  border-radius: 4px;
  box-shadow: 0 2px 8px rgba(0,0,0,0.18);
  font-size: 12px;
  cursor: pointer;
  word-break: break-all;
  line-height: 1.4;
  animation: toast-in 0.18s ease-out;
}
.toast-error { background: #fde8e8; color: #9b1c1c; border: 1px solid #f5b5b5; }
.toast-info { background: #e6f0fb; color: #1a4f8b; border: 1px solid #b8d2ec; }
.toast-success { background: #e6f6ea; color: #1d6638; border: 1px solid #b6e0c1; }
@keyframes toast-in {
  from { transform: translateY(8px); opacity: 0; }
  to { transform: translateY(0); opacity: 1; }
}
</style>
