import { reactive } from "vue";

// Lifecycle event log displayed in the bottom of the left config panel —
// mirrors the parametric reference UI's "数据管道建立 / 断开" entries. Kept
// separate from useCommLog (which is the per-frame RawFrame stream) so the
// UI shows ~5 meaningful entries instead of thousands of hex lines per second.

export interface EventLogEntry {
  time: string; // "YYYY/MM/DD HH:MM:SS"
  message: string;
  kind: "info" | "error";
}

const events = reactive<EventLogEntry[]>([]);
const MAX_ENTRIES = 200;

function now(): string {
  const d = new Date();
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function useEventLog() {
  function push(message: string, kind: EventLogEntry["kind"] = "info") {
    events.unshift({ time: now(), message, kind });
    if (events.length > MAX_ENTRIES) events.splice(MAX_ENTRIES);
  }
  function clear() {
    events.splice(0);
  }
  return { events, push, clear };
}
