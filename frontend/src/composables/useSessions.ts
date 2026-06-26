import { reactive, ref } from "vue";
import type { SessionInfo, ConfigInfo } from "../types";

const sessions = reactive(new Map<string, SessionInfo>());
const configs = reactive(new Map<string, ConfigInfo>());
const selectedIdcode = ref<string>("");

export function useSessions() {
  function addSession(idcode: string, peerIp: string, dialKey?: string) {
    // A placeholder session keyed by "host:port" only means a TCP connect
    // attempt is in flight / the PMU handshake hasn't started — it must NOT
    // read as 已连接 (otherwise a peer that accepts TCP but never replies with
    // CFG-1 shows green "已连接" forever, see do_connect's "connecting…" emit).
    // Only a re-keyed session (real IDCODE, no ":") — which exists *because*
    // the substation actually replied with a frame — is a real connection.
    const state: SessionInfo["state"] = idcode.includes(":") ? "connecting" : "connected";
    sessions.set(idcode, { idcode, peerIp, state, dialKey });
    // 新增即选中,让用户立刻看到最新连入的子站握手/推流。
    selectedIdcode.value = idcode;
  }
  function setDialKey(idcode: string, dialKey: string) {
    const s = sessions.get(idcode);
    if (s) s.dialKey = dialKey;
  }
  function updateState(idcode: string, state: SessionInfo["state"]) {
    const s = sessions.get(idcode);
    if (s) s.state = state;
  }
  function removeSession(idcode: string) {
    sessions.delete(idcode);
    configs.delete(idcode);
    if (selectedIdcode.value === idcode) {
      const next = sessions.keys().next();
      selectedIdcode.value = next.done ? "" : next.value;
    }
  }
  function setConfig(idcode: string, cfg: ConfigInfo) {
    configs.set(idcode, cfg);
  }
  function clear() {
    sessions.clear();
    configs.clear();
    selectedIdcode.value = "";
  }
  return { sessions, configs, selectedIdcode, addSession, setDialKey, updateState, removeSession, setConfig, clear };
}
