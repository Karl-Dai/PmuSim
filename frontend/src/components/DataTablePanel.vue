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
  tone?: "ok" | "err" | "warn"; // 状态值语义色，仅 STAT 行用
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
  const ok = (good: boolean): DisplayRow["tone"] => !has ? undefined : good ? "ok" : "err";
  rows.push({ key: "stat-0", num: "01", name: "数据可用",
    value: !has ? "-" : (stat & 0x8000) === 0 ? "正常" : "异常", extra: "",
    tone: ok((stat! & 0x8000) === 0) });
  rows.push({ key: "stat-1", num: "02", name: "装置状态",
    value: !has ? "-" : (stat & 0x4000) === 0 ? "正常" : "异常", extra: "",
    tone: ok((stat! & 0x4000) === 0) });
  rows.push({ key: "stat-2", num: "03", name: "同步状态",
    value: !has ? "-" : (stat & 0x2000) === 0 ? "同步" : "失步", extra: "",
    tone: ok((stat! & 0x2000) === 0) });
  rows.push({ key: "stat-3", num: "04", name: "触发原因",
    value: !has ? "-" : (stat & 0x0800) === 0 ? "无" : (TRIGGER_REASONS[stat & 0xF] ?? "未知"),
    extra: "",
    tone: !has || (stat! & 0x0800) === 0 ? undefined : "warn" });

  if (!c) return rows;

  const analogStart = c.phnmr;
  for (let i = 0; i < c.annmr; i++) {
    const v = data?.analog[i];
    const raw = c.anunit?.[i];
    // ANUNIT high byte = IEEE C37.118 analog-type tag (0=single, 1=rms,
    // 2=peak); low 24 bits = signed multiplier × 0.00001. Without masking
    // the tag, a substation that reports ANUNIT=0x01000064 (rms, factor
    // 100) shows up as 16_777_316 × 0.00001 ≈ 167.77 and blows up the
    // displayed value ~1.6e4×.
    const factor = raw === undefined ? 0 : (() => {
      const low24 = raw & 0xFFFFFF;
      const signed = low24 & 0x800000 ? low24 - 0x1000000 : low24;
      return signed * 0.00001;
    })();
    const value =
      v === undefined ? "-" :
      factor === 0 ? v.toString() :
      (v * factor).toFixed(3);
    rows.push({
      key: `an-${i}`,
      num: String(5 + i).padStart(2, "0"),
      name: c.channelNames[analogStart + i] || `AN_${i + 1}`,
      value,
      extra: String(raw ?? 0),
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
          <td :class="row.tone ? `tone-${row.tone}` : ''">{{ row.value }}</td>
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
  background: var(--bg-content);
  border: 1px solid var(--border);
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
  background: linear-gradient(#eceadf, #dcdacc);
  font-weight: 600;
  color: #2a2a2a;
  padding: 7px 10px;
  text-align: left;
  border-right: 1px solid #b6b5a8;
  border-bottom: 1px solid var(--border);
  letter-spacing: 0.3px;
  white-space: nowrap;
}
.data-table thead th:first-child { text-align: center; }
.data-table thead th:last-child { border-right: none; }

.data-table tbody td {
  padding: 5px 10px;
  border-right: 1px solid var(--border-soft);
  border-bottom: 1px solid var(--border-soft);
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-variant-numeric: tabular-nums;
  color: var(--text);
  height: 24px;
}
/* 序号列：等宽数字 + 居中 */
.data-table tbody td:first-child {
  text-align: center;
  color: var(--text-faint);
}
/* 状态值语义色 */
.data-table tbody td.tone-ok { color: var(--ok); font-weight: 600; }
.data-table tbody td.tone-err { color: var(--err); font-weight: 600; }
.data-table tbody td.tone-warn { color: var(--warn); font-weight: 600; }
/* 名称列：用中文 sans，避免 monospace 把中文撑很宽 */
.data-table tbody td:nth-child(2) {
  font-family: -apple-system, "PingFang SC", "Microsoft YaHei", sans-serif;
}
.data-table tbody td:last-child { border-right: none; }

.data-table tbody tr { cursor: pointer; }
.data-table tbody tr:nth-child(even) { background: rgba(0,0,0,0.015); }
.data-table tbody tr:hover { background: var(--accent-tint); }
.data-table tbody tr.selected,
.data-table tbody tr.selected:hover {
  background: var(--accent);
  color: #fff;
}
/* 选中行：语义色让位于白字，保证蓝底可读 */
.data-table tbody tr.selected td,
.data-table tbody tr.selected td.tone-ok,
.data-table tbody tr.selected td.tone-err,
.data-table tbody tr.selected td.tone-warn {
  color: #fff;
  border-right-color: var(--accent-dark);
  border-bottom-color: var(--accent-dark);
}
.data-table tbody tr.selected td:first-child { color: var(--accent-on-sel); }

.empty-row td,
.empty-row td:first-child {
  color: var(--text-faint);
  text-align: center;
  font-style: italic;
  padding: 24px;
  background: transparent;
  border-right: none;
}
</style>
