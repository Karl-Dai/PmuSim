import { listen } from "@tauri-apps/api/event";
import type { PmuEvent } from "../types";
import { useSessions } from "./useSessions";
import { useCommLog } from "./useCommLog";
import { useToast } from "./useToast";
import { useEventLog } from "./useEventLog";
import { useFrameRate } from "./useFrameRate";

export function usePmuEvents() {
  const { addSession, updateState, removeSession, setConfig } = useSessions();
  const { addLog, addData } = useCommLog();
  const { push: pushToast } = useToast();
  const { push: pushEvent } = useEventLog();
  const { tick: tickFrameRate, reset: resetFrameRate } = useFrameRate();

  async function startListening() {
    await listen<PmuEvent>("pmu-event", ({ payload }) => {
      switch (payload.type) {
        case "SessionCreated":
          addSession(payload.idcode, payload.peer_ip);
          // The placeholder "host:port" → real-idcode re-key fires
          // SessionDisconnected + SessionCreated; only emit the human-facing
          // log line for the real one, to match the reference UI's terse
          // "管道建立" semantics.
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
    });
  }

  return { startListening };
}
