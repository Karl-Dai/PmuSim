import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { useReconnect, type ReconnectTarget } from "../src/composables/useReconnect";

const r = useReconnect();

function target(host: string, mode: "normal" | "skipCfg2" = "normal"): ReconnectTarget {
  return { host, mgmtPort: 8000, dataPort: 8001, protocol: "V3", period: 50, mode };
}

beforeEach(() => {
  r._resetForTest();
  invoke.mockReset();
  invoke.mockResolvedValue(undefined);
  vi.useFakeTimers();
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("useReconnect 按 dialKey 多目标重连", () => {
  it("arm 后断线(streaming) → 退避后 connect + auto_handshake", async () => {
    r.arm("h1:8000", target("h1"));
    r.onDisconnect("h1:8000", true);
    expect(r.reconnectingOf("h1:8000")).toBe(true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledWith("connect_substation", { host: "h1", port: 8000, dataPort: 8001 });
    expect(invoke).toHaveBeenCalledWith("auto_handshake", { idcode: "h1:8000", period: 50 });
    expect(r.reconnectingOf("h1:8000")).toBe(false);
  });

  it("两个子站重连互不影响", async () => {
    r.arm("h1:8000", target("h1"));
    r.arm("h2:8000", target("h2"));
    r.onDisconnect("h1:8000", false);
    expect(r.reconnectingOf("h1:8000")).toBe(true);
    expect(r.reconnectingOf("h2:8000")).toBe(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledWith("connect_substation", { host: "h1", port: 8000, dataPort: 8001 });
    expect(invoke).not.toHaveBeenCalledWith("connect_substation", { host: "h2", port: 8000, dataPort: 8001 });
  });

  it("skipCfg2 模式重连走 skip_cfg2_open", async () => {
    r.arm("h1:8000", target("h1", "skipCfg2"));
    r.onDisconnect("h1:8000", true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledWith("skip_cfg2_open", { idcode: "h1:8000" });
  });

  it("cancel(dialKey) 停止该目标重连", async () => {
    r.arm("h1:8000", target("h1"));
    r.onDisconnect("h1:8000", true);
    r.cancel("h1:8000");
    expect(r.reconnectingOf("h1:8000")).toBe(false);
    await vi.advanceTimersByTimeAsync(5000);
    expect(invoke).not.toHaveBeenCalled();
  });

  it("connect 失败则指数退避重试", async () => {
    invoke.mockRejectedValueOnce(new Error("down")).mockResolvedValue(undefined);
    r.arm("h1:8000", target("h1"));
    r.onDisconnect("h1:8000", false);
    await vi.advanceTimersByTimeAsync(1000); // 第1次失败
    expect(r.reconnectingOf("h1:8000")).toBe(true);
    await vi.advanceTimersByTimeAsync(2000); // 退避到 2s 第2次成功
    expect(r.reconnectingOf("h1:8000")).toBe(false);
  });

  it("未 arm 的 dialKey onDisconnect 无副作用", async () => {
    r.onDisconnect("ghost:8000", true);
    await vi.advanceTimersByTimeAsync(5000);
    expect(invoke).not.toHaveBeenCalled();
  });
});
