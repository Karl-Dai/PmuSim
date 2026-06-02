import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SubEvent, SubDataInfo } from "../types";
import { useToast } from "./useToast";
import { useEventLog } from "./useEventLog";
import { useFrameRate } from "./useFrameRate";
import { t } from "../i18n";

const POLL_INTERVAL_MS = 100;

export const running = ref(false);
export const masterPeer = ref<string | null>(null);
export const listenPorts = ref<{ mgmt: number; data: number } | null>(null);
export const streaming = ref(false);
export const lastData = ref<SubDataInfo | null>(null);
export const sentCount = ref(0);

export function useSubEvents() {
  const toast = useToast();
  const eventLog = useEventLog();
  const frameRate = useFrameRate();

  const pushToast = (msg: string) => toast.push(msg, "error");
  const pushEvent = (msg: string, level: "info" | "error" = "info") => eventLog.push(msg, level);
  const tickRate = () => frameRate.tick();
  const resetRate = () => frameRate.reset();

  function handle(ev: SubEvent) {
    switch (ev.type) {
      case "Listening":
        listenPorts.value = { mgmt: ev.mgmt_port, data: ev.data_port };
        pushEvent(t("event.listening", { mgmt: ev.mgmt_port, data: ev.data_port }));
        break;
      case "MasterConnected":
        masterPeer.value = ev.peer_ip; pushEvent(t("event.masterConnected", { peer: ev.peer_ip })); break;
      case "MasterDisconnected":
        masterPeer.value = null; streaming.value = false; resetRate(); pushEvent(t("event.masterDisconnected", { peer: ev.peer_ip })); break;
      case "CommandReceived":
        pushEvent(t("event.commandReceived", { name: ev.name, cmd: ev.cmd.toString(16) })); break;
      case "Cfg1Sent": pushEvent(t("event.cfg1Sent")); break;
      case "Cfg2Sent": pushEvent(t("event.cfg2Sent")); break;
      case "Cfg2Received": pushEvent(t("event.cfg2Received")); break;
      case "Cfg2Rejected":
        pushEvent(t("event.cfg2Rejected", { reason: ev.reason }), "error");
        break;
      case "StreamingStarted": streaming.value = true; pushEvent(t("event.streamingStarted")); break;
      case "StreamingStopped": streaming.value = false; resetRate(); pushEvent(t("event.streamingStopped")); break;
      case "DataFrameSent": lastData.value = ev.data; sentCount.value++; tickRate(); break;
      case "RawFrame": break;
      case "Error": pushToast(ev.error); pushEvent(ev.error, "error"); break;
    }
  }

  function startListening() {
    const pollOnce = async () => {
      try {
        const events = await invoke<SubEvent[]>("poll_events");
        for (const ev of events) handle(ev);
      } catch (e) {
        console.warn("poll_events failed", e);
      } finally {
        setTimeout(pollOnce, POLL_INTERVAL_MS);
      }
    };
    pollOnce();
  }

  return { startListening };
}
