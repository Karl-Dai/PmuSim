<script setup lang="ts">
import { computed, ref } from "vue";
import { useSessions } from "../composables/useSessions";
import { useCommLog } from "../composables/useCommLog";

const { selectedIdcode, configs } = useSessions();
const { latestData } = useCommLog();
// Stable key (e.g. "stat-0" / "an-3" / "dg-7") so reselection survives
// CFG-2 reload or analog/digital count changes.
const selectedKey = ref<string | null>(null);

const cfg = computed(() => configs.get(selectedIdcode.value));

const TRIGGER_REASONS: Record<number, string> = {
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

interface DisplayRow {
  key: string;
  num: string;
  name: string;
  value: string;
  extra: string;
}

// Single computed flattening STAT + analogs + digitals into ready-to-render
// strings. With 100 Hz data, function-style cells like analogValue(i) re-run
// per cell per frame; one computed lets Vue diff only the changed text nodes.
// STAT bit decoding per V3 §8.11 表 12; analog scale per §8.5 表 8 row 16
// (raw × ANUNIT × 1e-5); digital bits per §8.6.
const displayRows = computed<DisplayRow[]>(() => {
  const data = latestData.value?.data;
  const c = cfg.value;
  const rows: DisplayRow[] = [];

  const stat = data?.stat;
  const has = stat !== undefined;
  rows.push({ key: "stat-0", num: "01", name: "数据可用",
    value: !has ? "-" : (stat & 0x8000) === 0 ? "正常" : "异常", extra: "" });
  rows.push({ key: "stat-1", num: "02", name: "装置状态",
    value: !has ? "-" : (stat & 0x4000) === 0 ? "正常" : "异常", extra: "" });
  rows.push({ key: "stat-2", num: "03", name: "同步状态",
    value: !has ? "-" : (stat & 0x2000) === 0 ? "同步" : "失步", extra: "" });
  rows.push({ key: "stat-3", num: "04", name: "触发原因",
    value: !has ? "-" : (stat & 0x0800) === 0 ? "无" : (TRIGGER_REASONS[stat & 0xF] ?? "未知"),
    extra: "" });

  if (!c) return rows;

  const analogStart = c.phnmr;
  for (let i = 0; i < c.annmr; i++) {
    const v = data?.analog[i];
    const factor = c.anunit?.[i];
    const value =
      v === undefined ? "-" :
      !factor ? v.toString() :
      (v * factor * 0.00001).toFixed(3);
    rows.push({
      key: `an-${i}`,
      num: String(5 + i).padStart(2, "0"),
      name: c.channelNames[analogStart + i] || `AN_${i + 1}`,
      value,
      extra: String(factor ?? 0),
    });
  }

  const digitalStart = c.phnmr + c.annmr;
  const digitalTotal = c.dgnmr * 16;
  for (let i = 0; i < digitalTotal; i++) {
    const word = data?.digital[Math.floor(i / 16)];
    const value = word === undefined ? "-" : ((word >> (i % 16)) & 1 ? "合位" : "分位");
    rows.push({
      key: `dg-${i}`,
      num: String(5 + c.annmr + i).padStart(2, "0"),
      name: c.channelNames[digitalStart + i] || `DG_${i + 1}`,
      value,
      extra: "",
    });
  }

  return rows;
});
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
        <tr v-for="row in displayRows" :key="row.key"
            :class="{ selected: selectedKey === row.key }"
            @click="selectedKey = row.key">
          <td>{{ row.num }}</td>
          <td>{{ row.name }}</td>
          <td>{{ row.value }}</td>
          <td>{{ row.extra }}</td>
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
