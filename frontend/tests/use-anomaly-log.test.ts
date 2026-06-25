import { describe, it, expect, beforeEach } from "vitest";
import { useAnomalyLog } from "../src/composables/useAnomalyLog";
import type { PmuEvent } from "../src/types";

function ev(over: Partial<Extract<PmuEvent, { type: "TimestampAnomaly" }>> = {}) {
  return {
    type: "TimestampAnomaly",
    idcode: "PMU1",
    kind: "gap",
    expected_ms: 20,
    actual_ms: 40,
    soc: 1781,
    fracsec: 0x000d9490,
    frame_time: "2026-06-23 14:30:45",
    ...over,
  } as Extract<PmuEvent, { type: "TimestampAnomaly" }>;
}

beforeEach(() => {
  useAnomalyLog().clear();
});

describe("useAnomalyLog", () => {
  it("push 把 snake_case 事件转成 camelCase 条目，最新在前", () => {
    const { entries, push } = useAnomalyLog();
    push(ev({ soc: 1 }));
    push(ev({ soc: 2 }));
    expect(entries[0].soc).toBe(2);
    expect(entries[0].expectedMs).toBe(20);
    expect(entries[0].actualMs).toBe(40);
    expect(entries[0].frameTime).toBe("2026-06-23 14:30:45");
    expect(entries.length).toBe(2);
  });

  it("每条 id 唯一", () => {
    const { entries, push } = useAnomalyLog();
    push(ev());
    push(ev());
    expect(entries[0].id).not.toBe(entries[1].id);
  });

  it("counts 按 kind 统计，未知 code 仅计 total", () => {
    const { counts, push } = useAnomalyLog();
    push(ev({ kind: "gap" }));
    push(ev({ kind: "backward" }));
    push(ev({ kind: "stall" }));
    push(ev({ kind: "weird" }));
    expect(counts.value).toEqual({ backward: 1, gap: 1, stall: 1, total: 4 });
  });

  it("FIFO 截断到 500", () => {
    const { entries, push } = useAnomalyLog();
    for (let i = 0; i < 520; i++) push(ev({ soc: i }));
    expect(entries.length).toBe(500);
    // 最新（soc 519）在前，最旧的被丢
    expect(entries[0].soc).toBe(519);
  });

  it("clear 清空", () => {
    const { entries, push, clear } = useAnomalyLog();
    push(ev());
    clear();
    expect(entries.length).toBe(0);
  });
});
