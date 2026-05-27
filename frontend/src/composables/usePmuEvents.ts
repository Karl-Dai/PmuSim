import { listen } from "@tauri-apps/api/event";
import type { PmuEvent } from "../types";
import { useSessions } from "./useSessions";
import { useCommLog } from "./useCommLog";
import { useToast } from "./useToast";

export function usePmuEvents() {
  const { addSession, updateState, removeSession, setConfig } = useSessions();
  const { addLog, addData } = useCommLog();
  const { push: pushToast } = useToast();

  async function startListening() {
    await listen<PmuEvent>("pmu-event", ({ payload }) => {
      switch (payload.type) {
        case "SessionCreated":
          addSession(payload.idcode, payload.peer_ip);
          break;
        case "SessionDisconnected":
          // Drop the row entirely so heartbeat-timeout, mgmt-EOF and re-key
          // events all leave the station list clean. Without this the
          // sessions Map and status-bar count drift up monotonically across
          // every transient disconnect.
          removeSession(payload.idcode);
          break;
        case "Cfg1Received":
          updateState(payload.idcode, "cfg1_received");
          setConfig(payload.idcode, payload.cfg);
          break;
        case "Cfg2Sent":
          updateState(payload.idcode, "cfg2_sent");
          break;
        case "Cfg2Received":
          setConfig(payload.idcode, payload.cfg);
          break;
        case "StreamingStarted":
          updateState(payload.idcode, "streaming");
          break;
        case "StreamingStopped":
          updateState(payload.idcode, "cfg2_sent");
          break;
        case "DataFrame":
          addData(payload.idcode, payload.data);
          break;
        case "RawFrame":
          addLog(payload.idcode, payload.direction, payload.hex);
          break;
        case "HeartbeatTimeout":
          pushToast(`${payload.idcode}: 心跳超时,已断开`, "error");
          removeSession(payload.idcode);
          break;
        case "Error":
          addLog(payload.idcode, "!", payload.error);
          pushToast(payload.idcode ? `${payload.idcode}: ${payload.error}` : payload.error, "error");
          break;
      }
    });
  }

  return { startListening };
}
