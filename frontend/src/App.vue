<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { useToast } from "./composables/useToast";
import ConfigInfoPanel from "./components/ConfigInfoPanel.vue";
import DataTablePanel from "./components/DataTablePanel.vue";
import UpdateDialog from "./components/UpdateDialog.vue";
import AnomalyPanel from "./components/AnomalyPanel.vue";
import { useI18n } from "./i18n";

// PMU event listener is attached in main.ts BEFORE this component mounts
// (see comment there) so we don't race the first connect_substation call.
const { toasts, dismiss, push } = useToast();
const { t, locale, setLocale } = useI18n();

type UpdateMeta = { version: string; notes: string; pub_date?: string | null };
const updateMeta = ref<UpdateMeta | null>(null);
const updateVisible = ref(false);
const checking = ref(false);
const appVersion = ref("");

async function checkUpdate(force = false) {
  if (checking.value) return;
  checking.value = true;
  try {
    const meta = await invoke<UpdateMeta | null>("check_for_update", { force });
    if (meta) {
      updateMeta.value = meta;
      updateVisible.value = true;
    } else if (force) {
      push(t("app.upToDate"), "info");
    }
  } catch (e) {
    if (force) push(t("app.checkFailed", { error: String(e) }), "error");
    else console.warn("auto update check failed", e);
  } finally {
    checking.value = false;
  }
}

function openGithub() {
  invoke("open_url", { url: "https://github.com/Karl-Dai/PmuSim" }).catch((e) => {
    console.warn("open github failed", e);
  });
}

function onSnooze() {
  if (updateMeta.value) {
    invoke("snooze_update", { version: updateMeta.value.version }).catch(() => {});
  }
}

onMounted(async () => {
  // Show the running version in the title bar — getVersion() reads
  // tauri.conf.json's version at runtime, so it always matches the actual build.
  try {
    appVersion.value = await getVersion();
  } catch (e) {
    console.warn("getVersion failed", e);
  }
  // Auto-check on startup; silent on failure / already-latest.
  checkUpdate(false);
});
</script>

<template>
  <div class="app">
    <div class="title-bar">
      <span>{{ t("app.title") }}<span v-if="appVersion" class="app-version">v{{ appVersion }}</span></span>
      <div class="title-actions">
        <div class="lang-toggle" role="group" aria-label="Language">
          <button :class="{ on: locale === 'zh' }" @click="setLocale('zh')">中</button>
          <button :class="{ on: locale === 'en' }" @click="setLocale('en')">EN</button>
        </div>
        <button class="icon-btn" :title="t('app.github')" aria-label="GitHub" @click="openGithub">
          <svg viewBox="0 0 16 16" width="15" height="15" fill="currentColor" aria-hidden="true">
            <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0016 8c0-4.42-3.58-8-8-8z"/>
          </svg>
        </button>
        <button class="check-btn" :disabled="checking" @click="checkUpdate(true)">
          {{ checking ? t("app.checking") : t("app.checkUpdate") }}
        </button>
      </div>
    </div>
    <div class="content">
      <ConfigInfoPanel />
      <DataTablePanel />
    </div>

    <AnomalyPanel />

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
/* ── 统一调色板 ──────────────────────────────────────────────────────
   保留老式 SCADA 暖灰机箱气质，但：去掉内容区的黄味、把三种蓝收敛成
   一个主蓝、拉开背景层级、并补上状态语义色（绿/红/琥珀）。 */
:root {
  /* 背景层级：机箱(暗) → 面板 → 内容(近白) */
  --bg-chrome: #cdccc0;
  --bg-panel: #f4f3ec;
  --bg-content: #fcfcfa;
  --bg-input: #ffffff;
  --bg-disabled: #e7e6de;
  /* 边框 */
  --border: #8a8a82;
  --border-soft: #d8d6cc;
  --border-dash: #b5b5ad;
  /* 统一主蓝 + 派生 */
  --accent: #2563a8;
  --accent-dark: #1d4f88;
  --accent-tint: #eaf1fb;        /* hover / 浅高亮 */
  --accent-on-sel: #cfe0f5;      /* 选中行内的次要文字 */
  /* 文字 */
  --text: #1a1a1a;
  --text-dim: #555;
  --text-faint: #888;
  /* 状态语义色 */
  --ok: #1d7a3e;
  --warn: #b06a00;
  --err: #c02626;
}
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
  font-family: -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", "Segoe UI", sans-serif;
  font-size: 13px;
  color: var(--text);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--bg-chrome);
}
.title-bar {
  background: linear-gradient(var(--accent), var(--accent-dark));
  color: #fff;
  padding: 5px 12px;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.3px;
  text-shadow: 0 1px 0 rgba(0,0,0,0.25);
  border-bottom: 1px solid var(--accent-dark);
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.app-version {
  margin-left: 8px;
  font-weight: 400;
  font-size: 11px;
  opacity: 0.72;
  letter-spacing: 0;
  text-shadow: none;
}
.title-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}
.lang-toggle {
  display: flex;
  border: 1px solid rgba(255, 255, 255, 0.3);
  border-radius: 3px;
  overflow: hidden;
}
.lang-toggle button {
  background: rgba(255, 255, 255, 0.16);
  border: none;
  color: #fff;
  font-size: 11px;
  font-weight: 600;
  padding: 2px 8px;
  cursor: pointer;
  text-shadow: none;
}
.lang-toggle button + button { border-left: 1px solid rgba(255, 255, 255, 0.3); }
.lang-toggle button:hover { background: rgba(255, 255, 255, 0.28); }
.lang-toggle button.on { background: #fff; color: var(--accent-dark); }
.icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 22px;
  padding: 0;
  background: rgba(255, 255, 255, 0.16);
  border: 1px solid rgba(255, 255, 255, 0.3);
  border-radius: 3px;
  color: #fff;
  cursor: pointer;
  opacity: 0.9;
}
.icon-btn:hover { background: rgba(255, 255, 255, 0.28); opacity: 1; }
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
