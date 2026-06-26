import { describe, it, expect, beforeEach } from "vitest";
import { useCommLog } from "../src/composables/useCommLog";
import type { DataInfo } from "../src/types";

function mkData(stat: number): DataInfo {
  return {
    soc: 0, fracsec: 0, stat, format_flags: 1, time_quality: 0,
    freq: 0, dfreq: 0, analog: [], digital: [], phasors: [], local_offset_ms: 0,
  };
}

describe("useCommLog 按 idcode 存最新数据帧", () => {
  beforeEach(() => useCommLog().clear());

  it("不同子站的数据帧互不覆盖", () => {
    const { addData, latestOf } = useCommLog();
    addData("A", mkData(1));
    addData("B", mkData(2));
    addData("A", mkData(3));
    expect(latestOf("A")?.stat).toBe(3);
    expect(latestOf("B")?.stat).toBe(2);
  });

  it("未知 idcode 返回 undefined", () => {
    expect(useCommLog().latestOf("nope")).toBeUndefined();
  });

  it("clear() 清空", () => {
    const { addData, latestOf, clear } = useCommLog();
    addData("A", mkData(1));
    clear();
    expect(latestOf("A")).toBeUndefined();
  });
});
