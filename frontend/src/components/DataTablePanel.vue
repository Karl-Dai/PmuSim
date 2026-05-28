<script setup lang="ts">
import { computed, ref } from "vue";
import { useSessions } from "../composables/useSessions";
import { useCommLog } from "../composables/useCommLog";

const { selectedIdcode, configs } = useSessions();
const { latestData } = useCommLog();
const selectedRow = ref(-1);

const cfg = computed(() => configs.get(selectedIdcode.value));

// STAT bit decoding per V3 §8.11 表 12. Returned as fixed 4 rows so the table
// row count is stable (and so the user sees row 01-04 even before any data
// arrives).
const statRows = computed(() => {
  const stat = latestData.value?.data.stat;
  const has = stat !== undefined;
  const triggerReasons: Record<number, string> = {
    0: "手动",
    1: "幅值越下限",
    2: "幅值越上限",
    3: "相角差",
    4: "频率越限",
    5: "频率变化率越限",
    6: "线性组合",
    7: "开关量",
    8: "低频振荡",
  };
  return [
    { name: "数据可用", value: !has ? "-" : (stat & 0x8000) === 0 ? "正常" : "异常" },
    { name: "装置状态", value: !has ? "-" : (stat & 0x4000) === 0 ? "正常" : "异常" },
    { name: "同步状态", value: !has ? "-" : (stat & 0x2000) === 0 ? "同步" : "失步" },
    { name: "触发原因", value: !has ? "-" : (stat! & 0x0800) === 0 ? "无" : (triggerReasons[stat! & 0xF] ?? "未知") },
  ];
});

// channel_names layout per spec §8.2/8.5: phasors..analogs..digitals,
// each digital group has 16 entries. Slice accordingly.
const analogNames = computed(() => {
  if (!cfg.value) return [];
  const start = cfg.value.phnmr;
  return cfg.value.channelNames.slice(start, start + cfg.value.annmr);
});

const digitalNames = computed(() => {
  if (!cfg.value) return [];
  const start = cfg.value.phnmr + cfg.value.annmr;
  return cfg.value.channelNames.slice(start, start + cfg.value.dgnmr * 16);
});

// Render the engineering value (raw_int × ANUNIT × 0.00001) per V3 §8.5
// 表 8 row 16 — the reference UI shows e.g. "2.000" for "风速_1" with
// ANUNIT=100000 (factor=1.0). Falling back to raw when factor=0 keeps
// the cell informative even if a malformed CFG-2 left anunit empty.
function analogValue(i: number): string {
  const v = latestData.value?.data.analog[i];
  if (v === undefined) return "-";
  const rawFactor = cfg.value?.anunit?.[i];
  if (!rawFactor) return v.toString();
  return (v * rawFactor * 0.00001).toFixed(3);
}

function digitalBit(i: number): string {
  const data = latestData.value?.data.digital;
  if (!data) return "-";
  const wordIdx = Math.floor(i / 16);
  const bitIdx = i % 16;
  const word = data[wordIdx];
  if (word === undefined) return "-";
  return (word >> bitIdx) & 1 ? "合位" : "分位";
}

function selectRow(idx: number) {
  selectedRow.value = idx;
}
</script>

<template>
  <div class="data-table-wrap">
    <table class="data-table">
      <thead>
        <tr>
          <th style="width: 50px">序号</th>
          <th>名称</th>
          <th style="width: 120px">状态/数值</th>
          <th style="width: 160px">比例系数/开关量状态</th>
        </tr>
      </thead>
      <tbody>
        <!-- STAT 4 rows -->
        <tr v-for="(r, i) in statRows" :key="`stat-${i}`"
            :class="{ selected: selectedRow === i }"
            @click="selectRow(i)">
          <td>{{ String(i + 1).padStart(2, '0') }}</td>
          <td>{{ r.name }}</td>
          <td>{{ r.value }}</td>
          <td></td>
        </tr>
        <!-- Analog rows -->
        <tr v-for="(name, i) in analogNames" :key="`an-${i}`"
            :class="{ selected: selectedRow === 4 + i }"
            @click="selectRow(4 + i)">
          <td>{{ String(5 + i).padStart(2, '0') }}</td>
          <td>{{ name || `AN_${i + 1}` }}</td>
          <td>{{ analogValue(i) }}</td>
          <td>{{ cfg?.anunit[i] ?? 0 }}</td>
        </tr>
        <!-- Digital rows -->
        <tr v-for="(name, i) in digitalNames" :key="`dg-${i}`"
            :class="{ selected: selectedRow === 4 + analogNames.length + i }"
            @click="selectRow(4 + analogNames.length + i)">
          <td>{{ String(5 + analogNames.length + i).padStart(2, '0') }}</td>
          <td>{{ name || `DG_${i + 1}` }}</td>
          <td>{{ digitalBit(i) }}</td>
          <td></td>
        </tr>
        <tr v-if="!cfg" class="empty-row">
          <td colspan="4">点击「连接」后,CFG-2 到达再显示通道列表</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.data-table-wrap {
  flex: 1;
  overflow: auto;
  background: #fefdf0;
  border: 1px solid #8a8a82;
  box-shadow: inset 1px 1px 0 rgba(0,0,0,0.03);
}
.data-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}
.data-table thead th {
  position: sticky;
  top: 0;
  background: linear-gradient(#dedccd, #cfcdbe);
  font-weight: 600;
  color: #2a2a2a;
  padding: 7px 10px;
  text-align: left;
  border-right: 1px solid #a8a89a;
  border-bottom: 1px solid #8a8a82;
  letter-spacing: 0.3px;
  white-space: nowrap;
}
.data-table thead th:first-child { text-align: center; }
.data-table thead th:last-child { border-right: none; }

.data-table tbody td {
  padding: 5px 10px;
  border-right: 1px solid #e6e2c5;
  border-bottom: 1px solid #ece8cf;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-variant-numeric: tabular-nums;
  color: #1a1a1a;
  height: 24px;
}
/* 序号列：等宽数字 + 居中 */
.data-table tbody td:first-child {
  text-align: center;
  color: #777;
}
/* 名称列：用中文 sans，避免 monospace 把中文撑很宽 */
.data-table tbody td:nth-child(2) {
  font-family: -apple-system, "PingFang SC", "Microsoft YaHei", sans-serif;
}
.data-table tbody td:last-child { border-right: none; }

.data-table tbody tr { cursor: pointer; }
.data-table tbody tr:nth-child(even) { background: rgba(0,0,0,0.015); }
.data-table tbody tr:hover { background: #f5f3d8; }
.data-table tbody tr.selected,
.data-table tbody tr.selected:hover {
  background: #2a6fc7;
  color: #fff;
}
.data-table tbody tr.selected td {
  color: #fff;
  border-right-color: #1c5aa8;
  border-bottom-color: #1c5aa8;
}
.data-table tbody tr.selected td:first-child { color: #cfe0f5; }

.empty-row td,
.empty-row td:first-child {
  color: #9a9a8a;
  text-align: center;
  font-style: italic;
  padding: 24px;
  background: transparent;
  border-right: none;
}
</style>
