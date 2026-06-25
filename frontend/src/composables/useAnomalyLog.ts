import { reactive, computed } from "vue";
import type { AnomalyEntry, PmuEvent } from "../types";

// 模块级共享状态，对齐 useEventLog.ts 风格（无 Pinia）。
const entries = reactive<AnomalyEntry[]>([]);
const MAX_ENTRIES = 500;
let nextId = 1;

type AnomalyEvent = Extract<PmuEvent, { type: "TimestampAnomaly" }>;

const counts = computed(() => {
  let backward = 0;
  let gap = 0;
  let stall = 0;
  for (const e of entries) {
    if (e.kind === "backward") backward++;
    else if (e.kind === "gap") gap++;
    else if (e.kind === "stall") stall++;
  }
  return { backward, gap, stall, total: entries.length };
});

function localNow(): string {
  const d = new Date();
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function useAnomalyLog() {
  function push(ev: AnomalyEvent) {
    entries.unshift({
      id: nextId++,
      localTime: localNow(),
      idcode: ev.idcode,
      kind: ev.kind,
      expectedMs: ev.expected_ms,
      actualMs: ev.actual_ms,
      soc: ev.soc,
      fracsec: ev.fracsec,
      frameTime: ev.frame_time,
    });
    if (entries.length > MAX_ENTRIES) entries.splice(MAX_ENTRIES);
  }

  function clear() {
    entries.splice(0);
  }

  return { entries, push, clear, counts };
}
