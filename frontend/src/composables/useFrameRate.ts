import { ref } from "vue";

// Sliding-window frame rate for the 实时数据 stream, derived from the data
// frames' own SOC/FRACSEC timestamps (报文时间) rather than wall-clock arrival
// time. This reports the rate the substation actually stamped its frames at —
// immune to webview event-loop jitter and network bunching that made the
// wall-clock count read e.g. 102 for a 100Hz stream. Each `tick(tsMs)` carries
// the frame's 报文时间 in ms (see frameTimeMs); fps = frames whose timestamp
// falls within the trailing 1s window.

const WINDOW_MS = 1000;
const recent: number[] = [];
const fps = ref(0);

export function useFrameRate() {
  function tick(tsMs: number) {
    // 报文时间倒退（子站重启 / GPS 校时 / SOC 回绕）会让旧时间戳全部落在
    // 新窗口内造成虚高 → 检测到回退即重置窗口，从当前帧重新计。
    if (recent.length > 0 && tsMs < recent[recent.length - 1]) {
      recent.length = 0;
    }
    recent.push(tsMs);
    const cutoff = tsMs - WINDOW_MS;
    while (recent.length > 0 && recent[0] < cutoff) recent.shift();
    fps.value = recent.length;
  }
  function reset() {
    recent.length = 0;
    fps.value = 0;
  }
  return { fps, tick, reset };
}
