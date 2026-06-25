<script setup lang="ts">
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { useAnomalyLog } from "../composables/useAnomalyLog";
import { useToast } from "../composables/useToast";
import { useI18n } from "../i18n";
import { buildCsv, droppedFrames, kindI18nKey } from "../lib/anomaly";
import type { AnomalyEntry } from "../types";

const { entries, clear, counts } = useAnomalyLog();
const { push: pushToast } = useToast();
const { t } = useI18n();

const collapsed = ref(true);
const filterKind = ref<string>("all");
const filterStation = ref<string>("all");
const expandedId = ref<number | null>(null);

// 拖拽高度（展开态）。
const panelHeight = ref(220);
let dragStartY = 0;
let dragStartH = 0;
function onDragStart(e: PointerEvent) {
  dragStartY = e.clientY;
  dragStartH = panelHeight.value;
  window.addEventListener("pointermove", onDragMove);
  window.addEventListener("pointerup", onDragEnd);
}
function onDragMove(e: PointerEvent) {
  // 顶边手柄上拖增高。
  const delta = dragStartY - e.clientY;
  panelHeight.value = Math.min(560, Math.max(120, dragStartH + delta));
}
function onDragEnd() {
  window.removeEventListener("pointermove", onDragMove);
  window.removeEventListener("pointerup", onDragEnd);
}

const stations = computed(() => {
  const set = new Set<string>();
  for (const e of entries) set.add(e.idcode);
  return [...set];
});

const filtered = computed(() =>
  entries.filter(
    (e) =>
      (filterKind.value === "all" || e.kind === filterKind.value) &&
      (filterStation.value === "all" || e.idcode === filterStation.value),
  ),
);

function kindLabel(kind: string): string {
  return t(kindI18nKey(kind));
}
function fracHex(f: number): string {
  return "0x" + (f >>> 0).toString(16).padStart(8, "0");
}
function droppedText(e: AnomalyEntry): string {
  return e.kind === "gap" ? "≈" + droppedFrames(e.actualMs, e.expectedMs) : "";
}
function toggleRow(id: number) {
  expandedId.value = expandedId.value === id ? null : id;
}
function rowDetail(e: AnomalyEntry): string {
  return [
    `${t("anomaly.colStation")}=${e.idcode}`,
    `${t("anomaly.colKind")}=${kindLabel(e.kind)}`,
    `${t("anomaly.colExpected")}=${e.expectedMs.toFixed(1)}`,
    `${t("anomaly.colActual")}=${e.actualMs.toFixed(1)}`,
    `SOC=${e.soc}`,
    `${t("anomaly.colFrameTime")}=${e.frameTime}`,
    `FRACSEC=${fracHex(e.fracsec)}`,
  ].join("  ");
}
async function copyDetail(e: AnomalyEntry) {
  try {
    await navigator.clipboard.writeText(rowDetail(e));
    pushToast(t("anomaly.copied"), "success");
  } catch {
    /* 剪贴板不可用时静默 */
  }
}

async function onExport() {
  if (!entries.length) {
    pushToast(t("anomaly.exportEmpty"), "info");
    return;
  }
  try {
    const path = await save({ defaultPath: t("anomaly.csvName") });
    if (!path) return; // 用户取消
    await invoke("save_text_file", { path, content: buildCsv(entries) });
    pushToast(t("anomaly.exportDone", { n: entries.length }), "success");
  } catch (e) {
    pushToast(t("anomaly.exportFailed", { error: String(e) }), "error");
  }
}
</script>

<template>
  <section class="anomaly-panel" :class="{ collapsed }" :style="!collapsed ? { height: panelHeight + 'px' } : undefined">
    <div v-if="!collapsed" class="drag-handle" @pointerdown="onDragStart"></div>
    <div class="anomaly-header" @click="collapsed = !collapsed">
      <span class="caret">{{ collapsed ? '▸' : '▾' }}</span>
      <span class="title">{{ t("anomaly.title") }}</span>
      <span class="badges" @click.stop>
        <span class="badge b-backward">{{ t("anomaly.count.backward", { n: counts.backward }) }}</span>
        <span class="badge b-gap">{{ t("anomaly.count.gap", { n: counts.gap }) }}</span>
        <span class="badge b-stall">{{ t("anomaly.count.stall", { n: counts.stall }) }}</span>
        <span class="badge b-total" :class="{ alert: counts.total > 0 }">{{ t("anomaly.count.total", { n: counts.total }) }}</span>
      </span>
      <span class="spacer"></span>
      <span v-if="!collapsed" class="tools" @click.stop>
        <select class="filter-kind" v-model="filterKind">
          <option value="all">{{ t("anomaly.filterKindAll") }}</option>
          <option value="backward">{{ t("anomaly.kind.backward") }}</option>
          <option value="gap">{{ t("anomaly.kind.gap") }}</option>
          <option value="stall">{{ t("anomaly.kind.stall") }}</option>
        </select>
        <select class="filter-station" v-model="filterStation">
          <option value="all">{{ t("anomaly.filterStationAll") }}</option>
          <option v-for="s in stations" :key="s" :value="s">{{ s }}</option>
        </select>
        <button class="btn-clear" @click="clear">{{ t("anomaly.clear") }}</button>
        <button class="btn-export" :disabled="!entries.length" @click="onExport">{{ t("anomaly.export") }}</button>
      </span>
    </div>

    <div v-if="!collapsed" class="anomaly-body">
      <table class="anomaly-table">
        <thead>
          <tr>
            <th>{{ t("anomaly.colTime") }}</th>
            <th>{{ t("anomaly.colStation") }}</th>
            <th>{{ t("anomaly.colKind") }}</th>
            <th>{{ t("anomaly.colExpected") }}</th>
            <th>{{ t("anomaly.colActual") }}</th>
            <th>{{ t("anomaly.colDropped") }}</th>
            <th>SOC</th>
            <th>{{ t("anomaly.colFrameTime") }}</th>
            <th>FRACSEC</th>
          </tr>
        </thead>
        <tbody>
          <template v-for="e in filtered" :key="e.id">
            <tr class="anomaly-row" :class="['k-' + e.kind, { selected: expandedId === e.id }]" @click="toggleRow(e.id)">
              <td>{{ e.localTime }}</td>
              <td>{{ e.idcode }}</td>
              <td class="col-kind">{{ kindLabel(e.kind) }}</td>
              <td class="num">{{ e.expectedMs.toFixed(1) }}</td>
              <td class="num">{{ e.actualMs.toFixed(1) }}</td>
              <td class="num col-dropped">{{ droppedText(e) }}</td>
              <td class="num">{{ e.soc }}</td>
              <td>{{ e.frameTime }}</td>
              <td class="mono">{{ fracHex(e.fracsec) }}</td>
            </tr>
            <tr v-if="expandedId === e.id" class="anomaly-detail">
              <td colspan="9">
                <span class="detail-text">{{ rowDetail(e) }}</span>
                <button class="btn-copy" @click.stop="copyDetail(e)">{{ t("anomaly.copy") }}</button>
              </td>
            </tr>
          </template>
          <tr v-if="!filtered.length" class="anomaly-empty">
            <td colspan="9">{{ t("anomaly.empty") }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>

<style scoped>
.anomaly-panel {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg-panel);
  border-top: 1px solid var(--border);
  margin: 0 8px 8px;
  border-radius: 4px;
  overflow: hidden;
  position: relative;
}
.anomaly-panel.collapsed { height: auto; }
.drag-handle {
  height: 5px;
  cursor: ns-resize;
  background: var(--border-soft);
  flex-shrink: 0;
}
.anomaly-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px;
  cursor: pointer;
  user-select: none;
  background: var(--bg-content);
  border-bottom: 1px solid var(--border-soft);
  flex-wrap: wrap;
}
.caret { width: 12px; color: var(--text-dim); }
.title { font-weight: 600; }
.spacer { flex: 1; }
.badges { display: flex; gap: 6px; }
.badge {
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 8px;
  background: var(--bg-disabled);
  color: var(--text-dim);
}
.badge.b-backward { color: var(--warn); }
.badge.b-gap { color: var(--err); }
.badge.b-stall { color: var(--text-dim); }
.badge.b-total.alert { background: var(--err); color: #fff; }
.tools { display: flex; gap: 6px; align-items: center; }
.tools select, .tools button {
  font-size: 11px;
  padding: 2px 6px;
  border: 1px solid var(--border-soft);
  border-radius: 3px;
  background: var(--bg-input);
  cursor: pointer;
}
.tools button:disabled { opacity: 0.5; cursor: default; }
.anomaly-body { flex: 1; overflow: auto; }
.anomaly-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
  font-family: "SF Mono", Menlo, Consolas, monospace;
}
.anomaly-table th {
  position: sticky;
  top: 0;
  background: var(--bg-content);
  text-align: left;
  padding: 3px 8px;
  border-bottom: 1px solid var(--border-soft);
  font-weight: 600;
  white-space: nowrap;
}
.anomaly-table td { padding: 2px 8px; border-bottom: 1px solid var(--border-soft); white-space: nowrap; }
.anomaly-table .num { text-align: right; }
.anomaly-row { cursor: pointer; }
.anomaly-row:hover { background: var(--accent-tint); }
.anomaly-row.selected { background: var(--accent-tint); }
.anomaly-row.k-backward .col-kind { color: var(--warn); }
.anomaly-row.k-gap .col-kind { color: var(--err); font-weight: 600; }
.anomaly-row.k-stall .col-kind { color: var(--text-dim); }
.anomaly-detail td { background: var(--bg-content); color: var(--text-dim); }
.detail-text { margin-right: 8px; }
.btn-copy {
  font-size: 11px;
  padding: 1px 6px;
  border: 1px solid var(--border-soft);
  border-radius: 3px;
  background: var(--bg-input);
  cursor: pointer;
}
.anomaly-empty td { text-align: center; color: var(--text-faint); padding: 12px; }
</style>
