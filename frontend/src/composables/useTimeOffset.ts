import { reactive } from "vue";

// 报文时间与本机时钟偏差(ms)按 idcode 隔离的定长滑动均值。每帧偏差由后端
// 采样写入 DataInfo.local_offset_ms。正=报文滞后本地,负=超前。
const WINDOW = 50;
const windows = new Map<string, number[]>();
const offsetMap = reactive(new Map<string, number | null>());

export function useTimeOffset() {
  function tick(idcode: string, ms: number) {
    let samples = windows.get(idcode);
    if (!samples) {
      samples = [];
      windows.set(idcode, samples);
    }
    samples.push(ms);
    if (samples.length > WINDOW) samples.shift();
    const sum = samples.reduce((a, b) => a + b, 0);
    offsetMap.set(idcode, sum / samples.length);
  }
  function reset(idcode: string) {
    windows.delete(idcode);
    offsetMap.set(idcode, null);
  }
  function offsetOf(idcode: string): number | null {
    return offsetMap.get(idcode) ?? null;
  }
  return { tick, reset, offsetOf };
}
