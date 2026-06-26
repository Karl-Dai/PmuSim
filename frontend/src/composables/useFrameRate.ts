import { reactive } from "vue";

// 按 idcode 维护滑动窗的帧率。窗口数组(plain)按 idcode 隔离;只把派生的
// fps 标量放进 reactive Map,供面板按选中子站读取。语义同单子站版:基于
// 报文 SOC/FRACSEC 时间戳(frameTimeMs)反推,报文时间倒退即重置该窗口。
const WINDOW_MS = 1000;
const windows = new Map<string, number[]>();
const fpsMap = reactive(new Map<string, number>());

export function useFrameRate() {
  function tick(idcode: string, tsMs: number) {
    let recent = windows.get(idcode);
    if (!recent) {
      recent = [];
      windows.set(idcode, recent);
    }
    if (recent.length > 0 && tsMs < recent[recent.length - 1]) recent.length = 0;
    recent.push(tsMs);
    const cutoff = tsMs - WINDOW_MS;
    while (recent.length > 0 && recent[0] < cutoff) recent.shift();
    fpsMap.set(idcode, recent.length);
  }
  function reset(idcode: string) {
    windows.delete(idcode);
    fpsMap.set(idcode, 0);
  }
  function fpsOf(idcode: string): number {
    return fpsMap.get(idcode) ?? 0;
  }
  return { tick, reset, fpsOf };
}
