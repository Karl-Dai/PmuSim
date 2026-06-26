<script setup lang="ts">
import { computed } from "vue";
import { useSessions } from "../composables/useSessions";
import { useFrameRate } from "../composables/useFrameRate";
import { useReconnect } from "../composables/useReconnect";
import { useI18n } from "../i18n";
import type { SessionInfo } from "../types";

const { t } = useI18n();
const { sessions, selectedIdcode } = useSessions();
const { fpsOf } = useFrameRate();
const { reconnectingOf } = useReconnect();

const rows = computed(() => [...sessions.values()]);

// LED 语义:重连中/握手中=琥珀,streaming=绿,disconnected=红,其余(connecting)=琥珀。
function ledClass(s: SessionInfo): string {
  if (s.dialKey && reconnectingOf(s.dialKey)) return "led-warn";
  if (s.state === "streaming") return "led-ok";
  if (s.state === "disconnected") return "led-err";
  if (s.state === "connecting") return "led-warn";
  return "led-warn";
}
function select(idcode: string) {
  selectedIdcode.value = idcode;
}
</script>

<template>
  <section class="station-panel">
    <div class="panel-hd">{{ t("station.title") }}</div>
    <div class="station-list">
      <div
        v-for="s in rows"
        :key="s.idcode"
        class="station-row"
        :class="{ selected: s.idcode === selectedIdcode }"
        @click="select(s.idcode)"
      >
        <span class="led" :class="ledClass(s)"></span>
        <span class="station-id">{{ s.idcode }}</span>
        <span class="station-fps">{{ fpsOf(s.idcode) }} {{ t("station.fpsUnit") }}</span>
      </div>
      <div v-if="rows.length === 0" class="station-empty">{{ t("station.empty") }}</div>
    </div>
  </section>
</template>

<style scoped>
.station-panel {
  width: 188px;
  min-width: 188px;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border);
  background: var(--bg-panel);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.55);
  overflow: hidden;
}
/* 复用 ConfigInfoPanel 的金属铭牌表头风格(简化) */
.panel-hd {
  padding: 6px 10px 6px 14px;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 1.5px;
  color: #44443d;
  background: linear-gradient(180deg, #e4e3d9, #bbbaae);
  border-bottom: 1px solid var(--border);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.75);
  text-shadow: 0 1px 0 rgba(255, 255, 255, 0.6);
  user-select: none;
}
.station-list {
  flex: 1;
  overflow: auto;
  background: var(--bg-content);
}
/* 机架插卡:每行一块卡,左侧 pilot LED */
.station-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-bottom: 1px solid var(--border-soft);
  cursor: pointer;
  font-size: 12px;
}
.station-row:hover { background: var(--accent-tint); }
.station-row.selected { background: var(--accent); color: #fff; }
.led {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  flex-shrink: 0;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
}
.led-ok { background: var(--ok); box-shadow: 0 0 4px var(--ok), inset 0 0 0 1px rgba(0,0,0,0.2); }
.led-warn { background: var(--warn); box-shadow: 0 0 4px var(--warn), inset 0 0 0 1px rgba(0,0,0,0.2); }
.led-err { background: var(--err); box-shadow: inset 0 0 0 1px rgba(0,0,0,0.2); }
.station-id {
  flex: 1;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.station-fps {
  font-size: 11px;
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.station-row.selected .station-fps { color: var(--accent-on-sel); }
.station-empty {
  padding: 16px 10px;
  text-align: center;
  color: var(--text-faint);
  font-style: italic;
  font-size: 12px;
}
</style>
