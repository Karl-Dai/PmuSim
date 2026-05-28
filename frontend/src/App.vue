<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useToast } from "./composables/useToast";
import ConfigInfoPanel from "./components/ConfigInfoPanel.vue";
import DataTablePanel from "./components/DataTablePanel.vue";
import UpdateDialog from "./components/UpdateDialog.vue";

// PMU event listener is attached in main.ts BEFORE this component mounts
// (see comment there) so we don't race the first connect_substation call.
const { toasts, dismiss, push } = useToast();

type UpdateMeta = { version: string; notes: string; pub_date?: string | null };
const updateMeta = ref<UpdateMeta | null>(null);
const updateVisible = ref(false);
const checking = ref(false);

async function checkUpdate(force = false) {
  if (checking.value) return;
  checking.value = true;
  try {
    const meta = await invoke<UpdateMeta | null>("check_for_update", { force });
    if (meta) {
      updateMeta.value = meta;
      updateVisible.value = true;
    } else if (force) {
      push("当前已是最新版本", "info");
    }
  } catch (e) {
    if (force) push(`检查更新失败: ${e}`, "error");
    else console.warn("auto update check failed", e);
  } finally {
    checking.value = false;
  }
}

function onSnooze() {
  if (updateMeta.value) {
    invoke("snooze_update", { version: updateMeta.value.version }).catch(() => {});
  }
}

onMounted(() => {
  // Auto-check on startup; silent on failure / already-latest.
  checkUpdate(false);
});
</script>

<template>
  <div class="app">
    <div class="title-bar">
      <span>simpmufep — PMU 主站模拟器</span>
      <button class="check-btn" :disabled="checking" @click="checkUpdate(true)">
        {{ checking ? "检查中…" : "检查更新" }}
      </button>
    </div>
    <div class="content">
      <ConfigInfoPanel />
      <DataTablePanel />
    </div>

    <div class="toasts" aria-live="polite">
      <div v-for="t in toasts" :key="t.id" :class="['toast', `toast-${t.kind}`]" @click="dismiss(t.id)">
        {{ t.message }}
      </div>
    </div>

    <UpdateDialog
      :visible="updateVisible"
      :version="updateMeta?.version ?? ''"
      :notes="updateMeta?.notes ?? ''"
      @close="updateVisible = false"
      @snooze="onSnooze"
    />
  </div>
</template>

<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
  font-family: -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", "Segoe UI", sans-serif;
  font-size: 13px;
  color: #1a1a1a;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #d4d2c5;
}
.title-bar {
  background: linear-gradient(#5a8ccc, #2c5a99);
  color: #fff;
  padding: 5px 12px;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.3px;
  text-shadow: 0 1px 0 rgba(0,0,0,0.25);
  border-bottom: 1px solid #1a467a;
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.check-btn {
  background: rgba(255, 255, 255, 0.16);
  border: 1px solid rgba(255, 255, 255, 0.3);
  color: #fff;
  font-size: 11px;
  padding: 2px 10px;
  border-radius: 3px;
  cursor: pointer;
  font-weight: 500;
  text-shadow: none;
}
.check-btn:hover:not(:disabled) { background: rgba(255, 255, 255, 0.28); }
.check-btn:disabled { opacity: 0.6; cursor: default; }
.content {
  flex: 1;
  display: flex;
  gap: 8px;
  padding: 8px;
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
