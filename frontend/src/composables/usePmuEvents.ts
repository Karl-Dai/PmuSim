import { invoke } from "@tauri-apps/api/core";
import type { PmuEvent } from "../types";
import { useSessions } from "./useSessions";
import { useCommLog } from "./useCommLog";
import { useToast } from "./useToast";
import { useEventLog } from "./useEventLog";
import { useFrameRate } from "./useFrameRate";
import { t } from "../i18n";

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
  const { addData } = useCommLog();
  const { push: pushToast } = useToast();
  const { push: pushEvent } = useEventLog();
  const { tick: tickFrameRate, reset: resetFrameRate } = useFrameRate();

  function handle(payload: PmuEvent) {
    switch (payload.type) {
      case "SessionCreated":
        addSession(payload.idcode, payload.peer_ip);
        if (!payload.idcode.includes(":")) {
          pushEvent(t("event.mgmtEstablished", { idcode: payload.idcode, ip: payload.peer_ip }));
        }
        break;
      case "SessionDisconnected":
        removeSession(payload.idcode);
        if (!payload.idcode.includes(":")) {
          pushEvent(t("event.pipeDisconnected", { idcode: payload.idcode }));
        }
        resetFrameRate();
        break;
      case "Cfg1Received":
        updateState(payload.idcode, "cfg1_received");
        setConfig(payload.idcode, payload.cfg);
        pushEvent(t("event.cfg1Received", { analog: payload.cfg.annmr, digital: payload.cfg.dgnmr }));
        break;
      case "Cfg2Sent":
        updateState(payload.idcode, "cfg2_sent");
        pushEvent(t("event.cfg2Sent"));
        break;
      case "Cfg2Received":
        setConfig(payload.idcode, payload.cfg);
        break;
      case "StreamingStarted":
        updateState(payload.idcode, "streaming");
        pushEvent(t("event.dataEstablished"));
        break;
      case "StreamingStopped":
        updateState(payload.idcode, "cfg2_sent");
        pushEvent(t("event.dataPaused"));
        resetFrameRate();
        break;
      case "DataFrame":
        addData(payload.idcode, payload.data);
        tickFrameRate();
        break;
      case "RawFrame":
        // The new UI does not render the raw-frame stream (a future hex
        // viewer can re-attach to useCommLog). Until then, silently drop
        // — buffering ~100 frames/s into a 1000-cap ring is just a 10s
        // sliding window of hex strings nobody reads.
        break;
      case "HeartbeatTimeout":
        pushToast(t("event.heartbeatTimeoutToast", { idcode: payload.idcode }), "error");
        pushEvent(t("event.heartbeatTimeout", { idcode: payload.idcode }), "error");
        removeSession(payload.idcode);
        resetFrameRate();
        break;
      case "Error":
        pushToast(payload.idcode ? `${payload.idcode}: ${payload.error}` : payload.error, "error");
        pushEvent(payload.error, "error");
        break;
    }
  }

  function startListening() {
    // setTimeout chain instead of setInterval: each poll must complete
    // before the next starts. If two polls race (setInterval allows this
    // when invoke takes > POLL_INTERVAL_MS), they call drain() in
    // arbitrary order and handle() across drains can re-order events
    // (SessionCreated arriving after the SessionDisconnected that closed
    // it). Sequential chain preserves emit-order end-to-end.
    const pollOnce = async () => {
      try {
        const events = await invoke<PmuEvent[]>("poll_events");
        for (const ev of events) handle(ev);
      } catch (e) {
        // First-call failures are expected during webview boot; log only.
        // eslint-disable-next-line no-console
        console.warn("poll_events failed", e);
      } finally {
        setTimeout(pollOnce, POLL_INTERVAL_MS);
      }
    };
    pollOnce();
    resolveReady();
  }

  return { startListening };
}
