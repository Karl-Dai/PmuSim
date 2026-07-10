import { ref, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export type ReconnectMode = "normal" | "skipCfg2";

export interface ReconnectTarget {
  host: string;
  mgmtPort: number;
  dataPort: number;
  protocol: "V2" | "V3";
  period: number | null;
  mode: ReconnectMode;
}

const BASE_DELAY_MS = 1_000;
const MAX_DELAY_MS = 30_000;

// 模块级单例状态(与 useSessions / useProtocol 同风格)。
let desired: ReconnectTarget | null = null;
let intentional = false;
let attempt = 0;
let pendingStreaming = false;
let timer: ReturnType<typeof setTimeout> | null = null;
const reconnecting: Ref<boolean> = ref(false);

function delayFor(a: number): number {
  return Math.min(BASE_DELAY_MS * 2 ** a, MAX_DELAY_MS);
}

function clearTimer(): void {
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
}

async function attemptReconnect(): Promise<void> {
  timer = null;
  if (!desired) {
    reconnecting.value = false;
    return;
  }
  const t = desired;
  const idcode = `${t.host}:${t.mgmtPort}`;
  try {
    await invoke("connect_substation", {
      host: t.host,
      port: t.mgmtPort,
      dataPort: t.protocol === "V3" ? t.dataPort : undefined,
    });
    if (pendingStreaming) {
      if (t.mode === "skipCfg2") {
        await invoke("skip_cfg2_open", { idcode });
      } else {
        await invoke("auto_handshake", { idcode, period: t.period });
      }
    }
  } catch {
    onAttemptFailed();
  }
}

function scheduleRetry(): void {
  clearTimer();
  timer = setTimeout(() => {
    void attemptReconnect();
  }, delayFor(attempt));
}

function arm(t: ReconnectTarget): void {
  clearTimer();
  desired = t;
  intentional = false;
  attempt = 0;
  pendingStreaming = false;
  reconnecting.value = false;
}

function onDisconnect(wasStreaming: boolean): void {
  if (intentional || !desired) return;
  pendingStreaming = pendingStreaming || wasStreaming;
  reconnecting.value = true;
  scheduleRetry();
}

// Tauri invoke only confirms that the backend command was queued. The actual
// TCP result arrives later as SessionCreated/SessionDisconnected events.
function onAttemptFailed(): void {
  if (intentional || !desired || !reconnecting.value) return;
  attempt += 1;
  scheduleRetry();
}

function onConnected(streaming: boolean): void {
  if (!reconnecting.value) return;
  // Re-key emits placeholder-disconnected immediately before real-session-created.
  // Cancel the retry scheduled by the placeholder event while the handshake continues.
  clearTimer();
  if (pendingStreaming && !streaming) return;
  attempt = 0;
  pendingStreaming = false;
  reconnecting.value = false;
}

function cancel(): void {
  intentional = true;
  clearTimer();
  attempt = 0;
  pendingStreaming = false;
  reconnecting.value = false;
}

function _resetForTest(): void {
  desired = null;
  intentional = false;
  attempt = 0;
  pendingStreaming = false;
  clearTimer();
  reconnecting.value = false;
}

const api = { reconnecting, arm, onDisconnect, onAttemptFailed, onConnected, cancel, _resetForTest };

export function useReconnect() {
  return api;
}
