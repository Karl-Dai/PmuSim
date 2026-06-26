import { reactive, computed, type ComputedRef } from "vue";
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

interface RState {
  desired: ReconnectTarget;
  intentional: boolean;
  attempt: number;
  pendingStreaming: boolean;
  timer: ReturnType<typeof setTimeout> | null;
}

// dialKey(= `${host}:${mgmtPort}`,与连接占位 idcode 一致) → 该目标的重连 FSM。
const states = new Map<string, RState>();
// 正在重连的 dialKey 集合(reactive,供 LED / 状态读数响应)。
const reconnectingKeys = reactive(new Set<string>());

function delayFor(a: number): number {
  return Math.min(BASE_DELAY_MS * 2 ** a, MAX_DELAY_MS);
}

function clearTimer(s: RState): void {
  if (s.timer !== null) {
    clearTimeout(s.timer);
    s.timer = null;
  }
}

async function attemptReconnect(dialKey: string): Promise<void> {
  const s = states.get(dialKey);
  if (!s) return;
  s.timer = null;
  const t = s.desired;
  try {
    await invoke("connect_substation", {
      host: t.host,
      port: t.mgmtPort,
      dataPort: t.protocol === "V3" ? t.dataPort : undefined,
    });
    if (s.pendingStreaming) {
      if (t.mode === "skipCfg2") {
        await invoke("skip_cfg2_open", { idcode: dialKey });
      } else {
        await invoke("auto_handshake", { idcode: dialKey, period: t.period });
      }
    }
    s.attempt = 0;
    reconnectingKeys.delete(dialKey);
  } catch {
    s.attempt += 1;
    scheduleRetry(dialKey);
  }
}

function scheduleRetry(dialKey: string): void {
  const s = states.get(dialKey);
  if (!s) return;
  clearTimer(s);
  s.timer = setTimeout(() => void attemptReconnect(dialKey), delayFor(s.attempt));
}

function arm(dialKey: string, t: ReconnectTarget): void {
  const existing = states.get(dialKey);
  if (existing) clearTimer(existing);
  states.set(dialKey, {
    desired: t,
    intentional: false,
    attempt: 0,
    pendingStreaming: false,
    timer: null,
  });
}
function onDisconnect(dialKey: string, wasStreaming: boolean): void {
  const s = states.get(dialKey);
  if (!s || s.intentional) return;
  s.pendingStreaming = wasStreaming;
  reconnectingKeys.add(dialKey);
  scheduleRetry(dialKey);
}
function cancel(dialKey: string): void {
  const s = states.get(dialKey);
  if (!s) return;
  s.intentional = true;
  clearTimer(s);
  s.attempt = 0;
  reconnectingKeys.delete(dialKey);
}
function cancelAll(): void {
  for (const key of [...states.keys()]) cancel(key);
}
function reconnectingOf(dialKey: string): boolean {
  return reconnectingKeys.has(dialKey);
}
function _resetForTest(): void {
  for (const s of states.values()) clearTimer(s);
  states.clear();
  reconnectingKeys.clear();
}
const reconnecting: ComputedRef<boolean> = computed(() => reconnectingKeys.size > 0);

const api = { arm, onDisconnect, cancel, cancelAll, reconnectingOf, reconnecting, _resetForTest };

export function useReconnect() {
  return api;
}
