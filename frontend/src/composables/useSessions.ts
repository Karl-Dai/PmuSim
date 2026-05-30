import { reactive, ref } from "vue";
import type { SessionInfo, ConfigInfo } from "../types";

const sessions = reactive(new Map<string, SessionInfo>());
const configs = reactive(new Map<string, ConfigInfo>());
const selectedIdcode = ref<string>("");

export function useSessions() {
  function addSession(idcode: string, peerIp: string) {
    // A placeholder session keyed by "host:port" only means a TCP connect
    // attempt is in flight / the PMU handshake hasn't started — it must NOT
    // read as 已连接 (otherwise a peer that accepts TCP but never replies with
    // CFG-1 shows green "已连接" forever, see do_connect's "connecting…" emit).
    // Only a re-keyed session (real IDCODE, no ":") — which exists *because*
    // the substation actually replied with a frame — is a real connection.
    const state: SessionInfo["state"] = idcode.includes(":") ? "connecting" : "connected";
    sessions.set(idcode, { idcode, peerIp, state });
    // Auto-select the first/newest session so the user doesn't have to
    // remember to click the row before using the 操作 buttons. Re-key
    // (placeholder → real IDCODE) takes the removeSession path which
    // clears selectedIdcode, so the next addSession picks the real one.
    if (!selectedIdcode.value) selectedIdcode.value = idcode;
  }
  function updateState(idcode: string, state: SessionInfo["state"]) {
    const s = sessions.get(idcode);
    if (s) s.state = state;
  }
  function removeSession(idcode: string) {
    sessions.delete(idcode);
    configs.delete(idcode);
    if (selectedIdcode.value === idcode) selectedIdcode.value = "";
  }
  function setConfig(idcode: string, cfg: ConfigInfo) {
    configs.set(idcode, cfg);
  }
  function clear() {
    sessions.clear();
    configs.clear();
    selectedIdcode.value = "";
  }
  return { sessions, configs, selectedIdcode, addSession, updateState, removeSession, setConfig, clear };
}
