import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { usePmuEvents } from "../src/composables/usePmuEvents";
import { useAnomalyLog } from "../src/composables/useAnomalyLog";
import { useEventLog } from "../src/composables/useEventLog";
import { useToast } from "../src/composables/useToast";

const anomalyEvent = {
  type: "TimestampAnomaly",
  idcode: "PMU1",
  kind: "gap",
  expected_ms: 20,
  actual_ms: 40,
  soc: 1781,
  fracsec: 0x000d9490,
  frame_time: "2026-06-23 14:30:45",
};

beforeEach(() => {
  useAnomalyLog().clear();
  useEventLog().clear();
  // 清空 toasts（reactive 数组，直接 splice）
  const { toasts } = useToast();
  toasts.splice(0);
  invoke.mockReset();
  vi.useFakeTimers();
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("usePmuEvents 处理 TimestampAnomaly", () => {
  it("异常进入 anomaly log，不进生命周期事件日志，并弹 toast", async () => {
    const toast = useToast();
    // useToast 返回模块级 push 函数引用，解构后 spy 无法拦截内部调用；
    // 改为直接断言 toasts 数组，语义等效且更稳定。
    invoke.mockResolvedValueOnce([anomalyEvent]).mockResolvedValue([]);

    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);

    const { entries } = useAnomalyLog();
    expect(entries.length).toBe(1);
    expect(entries[0].idcode).toBe("PMU1");
    expect(entries[0].kind).toBe("gap");

    // 不混入生命周期日志
    expect(useEventLog().events.length).toBe(0);

    // 弹了一次错误 toast
    expect(toast.toasts.length).toBe(1);
    expect(toast.toasts[0].kind).toBe("error");
  });
});
