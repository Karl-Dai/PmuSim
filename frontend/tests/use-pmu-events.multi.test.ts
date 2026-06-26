import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { usePmuEvents } from "../src/composables/usePmuEvents";
import { useSessions } from "../src/composables/useSessions";
import { useCommLog } from "../src/composables/useCommLog";
import { useReconnect } from "../src/composables/useReconnect";

const reconnect = useReconnect();

function mkData(stat: number) {
  return { soc: 0, fracsec: 0, stat, format_flags: 1, time_quality: 0, freq: 0, dfreq: 0, analog: [], digital: [], phasors: [], local_offset_ms: 0 };
}

beforeEach(() => {
  const { clear } = useSessions();
  clear();
  useCommLog().clear();
  reconnect._resetForTest();
  invoke.mockReset();
  vi.useFakeTimers();
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("usePmuEvents 多子站", () => {
  it("占位→真实 re-key 后,真实会话继承 dialKey", async () => {
    const { sessions } = useSessions();
    invoke.mockResolvedValueOnce([
      { type: "SessionCreated", idcode: "10.0.0.1:8000", peer_ip: "10.0.0.1" },
      { type: "SessionDisconnected", idcode: "10.0.0.1:8000" },
      { type: "SessionCreated", idcode: "PMU_A", peer_ip: "10.0.0.1" },
    ]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);
    expect(sessions.get("PMU_A")?.dialKey).toBe("10.0.0.1:8000");
  });

  it("两个子站的数据帧分别落到各自 idcode", async () => {
    const { latestOf } = useCommLog();
    invoke.mockResolvedValueOnce([
      { type: "DataFrame", idcode: "PMU_A", data: mkData(1) },
      { type: "DataFrame", idcode: "PMU_B", data: mkData(2) },
    ]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);
    expect(latestOf("PMU_A")?.stat).toBe(1);
    expect(latestOf("PMU_B")?.stat).toBe(2);
  });
});
