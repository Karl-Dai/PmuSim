import { invoke } from "@tauri-apps/api/core";
import type { PmuEvent } from "../types";
import { useSessions } from "./useSessions";
import { useCommLog } from "./useCommLog";
import { useToast } from "./useToast";
import { useEventLog } from "./useEventLog";
import { useFrameRate } from "./useFrameRate";

// We poll `poll_events` instead of using Tauri's listen()/emit() pair.
// On macOS WebKit, `listen()` IPC reliably deadlocks until the webview
// emits an internal ready event that Vue's mount alone does NOT trigger,
// so any event the master emits before listen() resolves is lost. The
// backend now buffers events in AppState (VecDeque, capped) and drains
// them on each poll, decoupling event delivery from webview lifecycle.

const POLL_INTERVAL_MS = 100;

// Resolves once the polling loop has started — kept for backwards
// compatibility with any caller that wants to wait before issuing the
// first command. With polling, "ready" just means the timer is running.
let resolveReady: () => void = () => {};
export const listenerReady: Promise<void> = new Promise((res) => {
  resolveReady = res;
});

export function usePmuEvents() {
  const { addSession, updateState, removeSession, setConfig } = useSessions();
  const { addLog, addData } = useCommLog();
  const { push: pushToast } = useToast();
  const { push: pushEvent } = useEventLog();
  const { tick: tickFrameRate, reset: resetFrameRate } = useFrameRate();

  function handle(payload: PmuEvent) {
    switch (payload.type) {
      case "SessionCreated":
        addSession(payload.idcode, payload.peer_ip);
        if (!payload.idcode.includes(":")) {
          pushEvent(`管理管道建立: ${payload.idcode}@${payload.peer_ip}`);
        }
        break;
      case "SessionDisconnected":
        removeSession(payload.idcode);
        if (!payload.idcode.includes(":")) {
          pushEvent(`管道断开: ${payload.idcode}`);
        }
        resetFrameRate();
        break;
      case "Cfg1Received":
        updateState(payload.idcode, "cfg1_received");
        setConfig(payload.idcode, payload.cfg);
        pushEvent(`收到 CFG-1 (${payload.cfg.annmr} 模拟量 / ${payload.cfg.dgnmr} 开关量组)`);
        break;
      case "Cfg2Sent":
        updateState(payload.idcode, "cfg2_sent");
        pushEvent("已下传 CFG-2");
        break;
      case "Cfg2Received":
        setConfig(payload.idcode, payload.cfg);
        break;
      case "StreamingStarted":
        updateState(payload.idcode, "streaming");
        pushEvent("数据管道建立");
        break;
      case "StreamingStopped":
        updateState(payload.idcode, "cfg2_sent");
        pushEvent("数据管道暂停");
        resetFrameRate();
        break;
      case "DataFrame":
        addData(payload.idcode, payload.data);
        tickFrameRate();
        break;
      case "RawFrame":
        addLog(payload.idcode, payload.direction, payload.hex);
        break;
      case "HeartbeatTimeout":
        pushToast(`${payload.idcode}: 心跳超时,已断开`, "error");
        pushEvent(`心跳超时: ${payload.idcode}`, "error");
        removeSession(payload.idcode);
        resetFrameRate();
        break;
      case "Error":
        addLog(payload.idcode, "!", payload.error);
        pushToast(payload.idcode ? `${payload.idcode}: ${payload.error}` : payload.error, "error");
        pushEvent(payload.error, "error");
        break;
    }
  }

  function startListening() {
    // Kick off polling. We don't await anything — the backend buffer is
    // already accumulating from the moment AppState is constructed, so
    // even events emitted before this point are not lost (they wait in
    // VecDeque until our first poll).
    setInterval(async () => {
      try {
        const events = await invoke<PmuEvent[]>("poll_events");
        for (const ev of events) handle(ev);
      } catch (e) {
        // First-call failures are expected during webview boot; log only.
        // eslint-disable-next-line no-console
        console.warn("poll_events failed", e);
      }
    }, POLL_INTERVAL_MS);
    resolveReady();
  }

  return { startListening };
}
