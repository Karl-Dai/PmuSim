<script setup lang="ts">
import { onMounted } from "vue";
import { usePmuEvents } from "./composables/usePmuEvents";
import { useToast } from "./composables/useToast";
import ConfigInfoPanel from "./components/ConfigInfoPanel.vue";
import DataTablePanel from "./components/DataTablePanel.vue";

const { startListening } = usePmuEvents();
const { toasts, dismiss } = useToast();

onMounted(() => {
  startListening();
});
</script>

<template>
  <div class="app">
    <div class="title-bar">simpmufep — PMU 主站模拟器</div>
    <div class="content">
      <ConfigInfoPanel />
      <DataTablePanel />
    </div>

    <div class="toasts" aria-live="polite">
      <div v-for="t in toasts" :key="t.id" :class="['toast', `toast-${t.kind}`]" @click="dismiss(t.id)">
        {{ t.message }}
      </div>
    </div>
  </div>
</template>

<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", sans-serif; font-size: 13px; }
.app { display: flex; flex-direction: column; height: 100vh; background: #d0d0c8; }
.title-bar {
  background: linear-gradient(#5a8ccc, #2c5a99);
  color: white;
  padding: 4px 10px;
  font-size: 12px;
  font-weight: 600;
}
.content {
  flex: 1;
  display: flex;
  gap: 6px;
  padding: 6px;
  overflow: hidden;
}

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
