import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { usePmuEvents } from "../src/composables/usePmuEvents";
import { useReconnect } from "../src/composables/useReconnect";
import { useSessions } from "../src/composables/useSessions";
import { useCommLog } from "../src/composables/useCommLog";

const reconnect = useReconnect();

beforeEach(() => {
  const { sessions, selectedIdcode } = useSessions();
  sessions.clear();
  selectedIdcode.value = "";
  reconnect._resetForTest();
  invoke.mockReset();
  vi.useFakeTimers();
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function seedSession(idcode: string, state: string) {
  const { sessions } = useSessions();
  sessions.set(idcode, { idcode, peerIp: "1.1.1.1", state: state as never });
}

describe("usePmuEvents 断线触发自动重连", () => {
  it("真实会话 SessionDisconnected(streaming) → onDisconnect(true)", async () => {
    const spy = vi.spyOn(reconnect, "onDisconnect");
    seedSession("PMU1", "streaming");
    invoke.mockResolvedValueOnce([{ type: "SessionDisconnected", idcode: "PMU1" }]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);
    expect(spy).toHaveBeenCalledWith(true);
  });

  it("HeartbeatTimeout(非 streaming) → onDisconnect(false)", async () => {
    const spy = vi.spyOn(reconnect, "onDisconnect");
    seedSession("PMU1", "cfg2_sent");
    invoke.mockResolvedValueOnce([{ type: "HeartbeatTimeout", idcode: "PMU1" }]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);
    expect(spy).toHaveBeenCalledWith(false);
  });

  it("初始连接的 placeholder 断开不启动未授权的自动重连", async () => {
    invoke.mockResolvedValueOnce([{ type: "SessionDisconnected", idcode: "10.0.0.1:8000" }]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);
    expect(reconnect.reconnecting.value).toBe(false);
  });

  it("重连中的 placeholder 断开会进入下一档退避", async () => {
    const failedSpy = vi.spyOn(reconnect, "onAttemptFailed");
    reconnect.arm({ host: "10.0.0.1", mgmtPort: 8000, dataPort: 8001, protocol: "V3", period: 50, mode: "normal" });
    reconnect.onDisconnect(true);
    invoke.mockResolvedValueOnce([{ type: "SessionDisconnected", idcode: "10.0.0.1:8000" }]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);

    expect(failedSpy).toHaveBeenCalledTimes(1);
    invoke.mockClear();
    await vi.advanceTimersByTimeAsync(2000);
    const connectCalls = invoke.mock.calls.filter(([command]) => command === "connect_substation");
    expect(connectCalls).toContainEqual(["connect_substation", {
      host: "10.0.0.1", port: 8000, dataPort: 8001,
    }]);
  });

  it("真实会话断开时清除最后一帧", async () => {
    const comm = useCommLog();
    seedSession("PMU1", "streaming");
    comm.addData("PMU1", {
      soc: 100, fracsec: 0, stat: 0, format_flags: 0, time_quality: 0,
      freq: 50, dfreq: 0, analog: [], digital: [], phasors: [],
    });
    invoke.mockResolvedValueOnce([{ type: "SessionDisconnected", idcode: "PMU1" }]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);

    expect(comm.latestData.value).toBeNull();
  });
});
