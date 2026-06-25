import type { AnomalyEntry } from "../types";

/** Gap 估算丢了几帧：四舍五入间隔倍数减 1，至少 1。expected<=0 返回 0。 */
export function droppedFrames(actualMs: number, expectedMs: number): number {
  if (expectedMs <= 0) return 0;
  return Math.max(1, Math.round(actualMs / expectedMs) - 1);
}

const KNOWN = new Set(["backward", "gap", "stall"]);

/** 异常 code → i18n key，未知 code 归到 unknown。 */
export function kindI18nKey(kind: string): string {
  return KNOWN.has(kind) ? `anomaly.kind.${kind}` : "anomaly.kind.unknown";
}

const CSV_HEADER = [
  "时刻",
  "子站",
  "类型",
  "预期ms",
  "实际ms",
  "丢帧",
  "SOC",
  "帧时间",
  "FRACSEC",
];

function csvCell(s: string): string {
  return /[",\r\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

function fracHex(fracsec: number): string {
  return "0x" + (fracsec >>> 0).toString(16).padStart(8, "0");
}

/** 生成 CSV 文本（CRLF 行尾，表头固定中文，与 UI 列序一致）。 */
export function buildCsv(entries: AnomalyEntry[]): string {
  const rows = entries.map((e) => [
    e.localTime,
    e.idcode,
    e.kind,
    e.expectedMs.toFixed(1),
    e.actualMs.toFixed(1),
    e.kind === "gap" ? String(droppedFrames(e.actualMs, e.expectedMs)) : "",
    String(e.soc),
    e.frameTime,
    fracHex(e.fracsec),
  ]);
  return [CSV_HEADER, ...rows].map((r) => r.map(csvCell).join(",")).join("\r\n");
}
