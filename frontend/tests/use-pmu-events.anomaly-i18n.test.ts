import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { usePmuEvents } from "../src/composables/usePmuEvents";
import { useEventLog } from "../src/composables/useEventLog";
import { useToast } from "../src/composables/useToast";
import { setLocale } from "../src/i18n";

beforeEach(() => {
  vi.useFakeTimers();
  invoke.mockReset();
  useEventLog().clear();
  useToast().toasts.splice(0);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("TimestampAnomaly i18n", () => {
  it("英文界面由结构化字段生成英文 toast 和事件日志", async () => {
    setLocale("en");
    invoke.mockResolvedValueOnce([{
      type: "TimestampAnomaly",
      idcode: "PMU1",
      kind: "gap",
      expected_ms: 20,
      actual_ms: 60,
      soc: 100,
      fracsec: 123,
      frame_time: "1970/01/01 08:01:40",
    }]).mockResolvedValue([]);

    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);

    const expected = "PMU1: timestamp gap, expected 20.0ms, actual 60.0ms";
    expect(useToast().toasts[0]?.message).toBe(expected);
    expect(useEventLog().events[0]?.message).toBe(expected);
  });
});
