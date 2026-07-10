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
    attempt = 0;
    reconnecting.value = false;
  } catch {
    attempt += 1;
    scheduleRetry();
  }
}

function scheduleRetry(): void {
  clearTimer();
  timer = setTimeout(() => {
    void attemptReconnect();
  }, delayFor(attempt));
}

function arm(t: ReconnectTarget): void {
  desired = t;
  intentional = false;
  attempt = 0;
}

function onDisconnect(wasStreaming: boolean): void {
  if (intentional || !desired) return;
  pendingStreaming = wasStreaming;
  reconnecting.value = true;
  scheduleRetry();
}

function cancel(): void {
  intentional = true;
  clearTimer();
  attempt = 0;
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

const api = { reconnecting, arm, onDisconnect, cancel, _resetForTest };

export function useReconnect() {
  return api;
}
