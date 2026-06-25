import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { useReconnect, type ReconnectTarget } from "../src/composables/useReconnect";

const api = useReconnect();

const target = (over: Partial<ReconnectTarget> = {}): ReconnectTarget => ({
  host: "10.0.0.1",
  mgmtPort: 8000,
  dataPort: 8001,
  protocol: "V3",
  period: 100,
  mode: "normal",
  ...over,
});

beforeEach(() => {
  api._resetForTest();
  invoke.mockReset();
  invoke.mockResolvedValue(undefined);
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

describe("useReconnect", () => {
  it("指数退避序列 1/2/4/8/16/30/30s,connect 持续失败", async () => {
    invoke.mockRejectedValue(new Error("connect failed"));
    api.arm(target());
    api.onDisconnect(false); // wasStreaming=false → 每次只调 connect 一次

    const delays = [1000, 2000, 4000, 8000, 16000, 30000, 30000];
    let calls = 0;
    for (const d of delays) {
      await vi.advanceTimersByTimeAsync(d - 1);
      expect(invoke).toHaveBeenCalledTimes(calls); // 还没到点
      await vi.advanceTimersByTimeAsync(1);
      calls += 1;
      expect(invoke).toHaveBeenCalledTimes(calls); // 第 calls 次 connect 尝试
    }
  });

  it("连上后重置退避:成功一次 → reconnecting=false,再断从 1s 重新开始", async () => {
    invoke.mockRejectedValueOnce(new Error("fail")).mockResolvedValue(undefined);
    api.arm(target());
    api.onDisconnect(false);

    await vi.advanceTimersByTimeAsync(1000); // 第1次失败 → 排 2s
    await vi.advanceTimersByTimeAsync(2000); // 第2次成功
    expect(api.reconnecting.value).toBe(false);

    invoke.mockClear();
    api.onDisconnect(false); // 再次断开
    await vi.advanceTimersByTimeAsync(999);
    expect(invoke).toHaveBeenCalledTimes(0);
    await vi.advanceTimersByTimeAsync(1); // 1s 后(从 attempt=0 重新开始)
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("主动断开(cancel)后 onDisconnect 不重连", async () => {
    api.arm(target());
    api.cancel();
    api.onDisconnect(true);
    await vi.advanceTimersByTimeAsync(60000);
    expect(invoke).not.toHaveBeenCalled();
    expect(api.reconnecting.value).toBe(false);
  });

  it("未 arm 时 onDisconnect 忽略", async () => {
    api.onDisconnect(true);
    await vi.advanceTimersByTimeAsync(60000);
    expect(invoke).not.toHaveBeenCalled();
  });

  it("忠实恢复:wasStreaming=true & mode=normal → connect + auto_handshake(period)", async () => {
    api.arm(target({ mode: "normal", period: 500 }));
    api.onDisconnect(true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledWith("connect_substation", { host: "10.0.0.1", port: 8000, dataPort: 8001 });
    expect(invoke).toHaveBeenCalledWith("auto_handshake", { idcode: "10.0.0.1:8000", period: 500 });
  });

  it("忠实恢复:mode=skipCfg2 → connect + skip_cfg2_open", async () => {
    api.arm(target({ mode: "skipCfg2" }));
    api.onDisconnect(true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledWith("skip_cfg2_open", { idcode: "10.0.0.1:8000" });
  });

  it("忠实恢复:wasStreaming=false → 只 connect,不握手/开流", async () => {
    api.arm(target());
    api.onDisconnect(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("connect_substation", { host: "10.0.0.1", port: 8000, dataPort: 8001 });
  });

  it("V2 不传 dataPort", async () => {
    api.arm(target({ protocol: "V2" }));
    api.onDisconnect(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(invoke).toHaveBeenCalledWith("connect_substation", { host: "10.0.0.1", port: 8000, dataPort: undefined });
  });

  it("cancel 清挂起 timer", async () => {
    invoke.mockRejectedValue(new Error("fail"));
    api.arm(target());
    api.onDisconnect(false);
    await vi.advanceTimersByTimeAsync(1000); // 第1次失败,排了 2s
    invoke.mockClear();
    api.cancel();
    await vi.advanceTimersByTimeAsync(60000);
    expect(invoke).not.toHaveBeenCalled();
  });
});
