<script setup lang="ts">
import { computed, ref, watch, onMounted, nextTick } from "vue";
import { useSessions } from "../composables/useSessions";
import { useCommLog } from "../composables/useCommLog";
import { computeVectors } from "../lib/phasor";

const { selectedIdcode, configs } = useSessions();
const { latestOf } = useCommLog();
const canvas = ref<HTMLCanvasElement | null>(null);
const SIZE = 160;
const COLORS = ["#2563a8", "#c02626", "#1d7a3e", "#b06a00", "#6b3fa0", "#0a7d8c"];

const vectors = computed(() => {
  const data = latestOf(selectedIdcode.value);
  const cfg = configs.get(selectedIdcode.value);
  if (!data || !cfg || cfg.phnmr === 0 || data.phasors.length === 0) return [];
  const polar = (data.format_flags & 1) === 1;
  return computeVectors(data.phasors.slice(0, cfg.phnmr), polar);
});

function draw() {
  const el = canvas.value;
  if (!el) return;
  const ctx = el.getContext("2d");
  if (!ctx) return;
  const c = SIZE / 2;
  const R = c - 12;
  ctx.clearRect(0, 0, SIZE, SIZE);
  // 刻度盘
  ctx.strokeStyle = "#d8d6cc";
  ctx.lineWidth = 1;
  for (const f of [0.5, 1]) {
    ctx.beginPath();
    ctx.arc(c, c, R * f, 0, Math.PI * 2);
    ctx.stroke();
  }
  ctx.beginPath();
  ctx.moveTo(c - R, c); ctx.lineTo(c + R, c);
  ctx.moveTo(c, c - R); ctx.lineTo(c, c + R);
  ctx.stroke();
  // 矢量(0° 指向右,逆时针为正,屏幕 y 向下故取负角)
  vectors.value.forEach((v, i) => {
    const rad = (-v.angleDeg * Math.PI) / 180;
    const x = c + R * v.normLen * Math.cos(rad);
    const y = c + R * v.normLen * Math.sin(rad);
    ctx.strokeStyle = COLORS[i % COLORS.length];
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(c, c); ctx.lineTo(x, y);
    ctx.stroke();
    ctx.fillStyle = COLORS[i % COLORS.length];
    ctx.beginPath();
    ctx.arc(x, y, 2.5, 0, Math.PI * 2);
    ctx.fill();
  });
}

watch(vectors, () => { void nextTick(draw); });
onMounted(draw);
</script>

<template>
  <div class="phasor-plot">
    <canvas v-if="vectors.length" ref="canvas" :width="SIZE" :height="SIZE"></canvas>
    <div v-else class="phasor-empty">—</div>
  </div>
</template>

<style scoped>
.phasor-plot {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 8px;
  background: var(--bg-content);
  border-bottom: 1px solid var(--border-soft);
}
.phasor-empty {
  height: 160px;
  display: flex;
  align-items: center;
  color: var(--text-faint);
}
</style>
