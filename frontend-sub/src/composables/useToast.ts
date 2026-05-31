import { reactive } from "vue";

export type ToastKind = "error" | "info" | "success";

export interface Toast {
  id: number;
  message: string;
  kind: ToastKind;
}

const toasts = reactive<Toast[]>([]);
let nextId = 1;

function push(message: string, kind: ToastKind = "info", ttlMs = 4000) {
  const id = nextId++;
  toasts.push({ id, message, kind });
  if (ttlMs > 0) {
    window.setTimeout(() => {
      const idx = toasts.findIndex((t) => t.id === id);
      if (idx >= 0) toasts.splice(idx, 1);
    }, ttlMs);
  }
  return id;
}

function dismiss(id: number) {
  const idx = toasts.findIndex((t) => t.id === id);
  if (idx >= 0) toasts.splice(idx, 1);
}

/** Stringify an error from any source (Tauri invoke rejects with string or Error). */
export function toastError(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

export function useToast() {
  return { toasts, push, dismiss };
}
