import { reactive } from "vue";
import type { DataInfo } from "../types";

export interface LogEntry {
  time: string;
  idcode: string;
  direction: string;
  summary: string;
  hex?: string;
}

const logs = reactive<LogEntry[]>([]);
const latestByIdcode = reactive(new Map<string, DataInfo>());
const MAX_LOGS = 1000;

export function useCommLog() {
  function addLog(idcode: string, direction: string, summary: string, hex?: string) {
    const now = new Date();
    const time = `${now.getHours().toString().padStart(2, "0")}:${now.getMinutes().toString().padStart(2, "0")}:${now.getSeconds().toString().padStart(2, "0")}`;
    logs.unshift({ time, idcode, direction, summary, hex });
    if (logs.length > MAX_LOGS) logs.splice(MAX_LOGS);
  }

  function addData(idcode: string, data: DataInfo) {
    latestByIdcode.set(idcode, data);
  }

  function latestOf(idcode: string): DataInfo | undefined {
    return latestByIdcode.get(idcode);
  }

  function clear() {
    logs.splice(0);
    latestByIdcode.clear();
  }

  return { logs, addLog, addData, latestOf, clear };
}
