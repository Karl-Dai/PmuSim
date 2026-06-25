import { describe, it, expect } from "vitest";
import { droppedFrames, buildCsv, kindI18nKey } from "../src/lib/anomaly";
import type { AnomalyEntry } from "../src/types";

function entry(over: Partial<AnomalyEntry> = {}): AnomalyEntry {
  return {
    id: 1,
    localTime: "14:30:45",
    idcode: "PMU1",
    kind: "gap",
    expectedMs: 20,
    actualMs: 40,
    soc: 1781,
    fracsec: 0x000d9490,
    frameTime: "2026-06-23 14:30:45",
    ...over,
  };
}

describe("droppedFrames", () => {
  it("丢一帧 40/20 → 1", () => {
    expect(droppedFrames(40, 20)).toBe(1);
  });
  it("丢两帧 60/20 → 2", () => {
    expect(droppedFrames(60, 20)).toBe(2);
  });
  it("结果至少为 1（轻微超界 31/20 也算丢 1 帧）", () => {
    expect(droppedFrames(31, 20)).toBe(1);
  });
  it("expected<=0 → 0（防除零）", () => {
    expect(droppedFrames(40, 0)).toBe(0);
    expect(droppedFrames(40, -1)).toBe(0);
  });
});

describe("kindI18nKey", () => {
  it("映射已知 code", () => {
    expect(kindI18nKey("gap")).toBe("anomaly.kind.gap");
    expect(kindI18nKey("backward")).toBe("anomaly.kind.backward");
    expect(kindI18nKey("stall")).toBe("anomaly.kind.stall");
  });
  it("未知 code 走 unknown key", () => {
    expect(kindI18nKey("weird")).toBe("anomaly.kind.unknown");
  });
});

describe("buildCsv", () => {
  it("首行是表头，gap 行带丢帧数，数值 1 位小数，FRACSEC 为 hex", () => {
    const csv = buildCsv([entry()]);
    const lines = csv.split("\r\n");
    expect(lines.filter(Boolean).length).toBe(2);
    expect(csv.endsWith("\r\n")).toBe(true);
    expect(lines[0]).toContain("FRACSEC");
    expect(lines[1]).toContain("14:30:45");
    expect(lines[1]).toContain("PMU1");
    expect(lines[1]).toContain("20.0");
    expect(lines[1]).toContain("40.0");
    expect(lines[1]).toContain("0x000d9490");
    // 丢帧列（索引 5）= 1
    const cells = lines[1].split(",");
    expect(cells[5]).toBe("1");
  });
  it("非 gap 行丢帧列为空", () => {
    const csv = buildCsv([entry({ kind: "stall", actualMs: 0 })]);
    const cells = csv.split("\r\n")[1].split(",");
    // 丢帧列（索引 5）为空字符串
    expect(cells[5]).toBe("");
  });
  it("含逗号的字段被双引号包裹转义", () => {
    const csv = buildCsv([entry({ idcode: "A,B" })]);
    expect(csv).toContain('"A,B"');
  });
});
