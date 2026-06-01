import { ref } from "vue";

// Sliding-window frame rate for the 实时数据 stream. We just track timestamps
// of recent ticks and report count/window-secs. Updating on each frame keeps
// the displayed fps responsive to short-term stalls (heartbeat hiccup, etc.)
// without amplifying jitter.

const WINDOW_MS = 1000;
const recent: number[] = [];
const fps = ref(0);

export function useFrameRate() {
  function tick() {
    const now = performance.now();
    recent.push(now);
    const cutoff = now - WINDOW_MS;
    while (recent.length > 0 && recent[0] < cutoff) recent.shift();
    fps.value = recent.length;
  }
  function reset() {
    recent.length = 0;
    fps.value = 0;
  }
  return { fps, tick, reset };
}
