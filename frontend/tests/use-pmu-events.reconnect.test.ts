import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { usePmuEvents } from "../src/composables/usePmuEvents";
import { useReconnect } from "../src/composables/useReconnect";
import { useSessions } from "../src/composables/useSessions";

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

  it("placeholder(host:port)SessionDisconnected 不触发重连", async () => {
    const spy = vi.spyOn(reconnect, "onDisconnect");
    invoke.mockResolvedValueOnce([{ type: "SessionDisconnected", idcode: "10.0.0.1:8000" }]).mockResolvedValue([]);
    usePmuEvents().startListening();
    await vi.advanceTimersByTimeAsync(120);
    expect(spy).not.toHaveBeenCalled();
  });
});
