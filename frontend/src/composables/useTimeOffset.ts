import { reactive } from "vue";

// 报文时间与本机时钟偏差(ms)按 idcode 隔离的定长滑动窗口。每帧偏差由后端
// 采样写入 DataInfo.local_offset_ms。正=报文滞后本地,负=超前。
// 窗口内同时维护均值、最大值、最小值;极值样本被滑出时重扫窗口重算。
const WINDOW = 50;
const windows = new Map<string, number[]>();
const offsetMap = reactive(new Map<string, number | null>());
const maxMap = reactive(new Map<string, number | null>());
const minMap = reactive(new Map<string, number | null>());

export function useTimeOffset() {
  function tick(idcode: string, ms: number) {
    let samples = windows.get(idcode);
    if (!samples) {
      samples = [];
      windows.set(idcode, samples);
    }
    samples.push(ms);
    let rescanMax = false;
    let rescanMin = false;
    if (samples.length > WINDOW) {
      const evicted = samples.shift() as number;
      if (evicted === maxMap.get(idcode)) rescanMax = true;
      if (evicted === minMap.get(idcode)) rescanMin = true;
    }
    const curMax = maxMap.get(idcode);
    if (curMax == null) {
      // 首帧:max=min=新样本。
      maxMap.set(idcode, ms);
      minMap.set(idcode, ms);
    } else {
      if (rescanMax) maxMap.set(idcode, Math.max(...samples));
      else if (ms > curMax) maxMap.set(idcode, ms);
      if (rescanMin) minMap.set(idcode, Math.min(...samples));
      else if (ms < (minMap.get(idcode) as number)) minMap.set(idcode, ms);
    }
    const sum = samples.reduce((a, b) => a + b, 0);
    offsetMap.set(idcode, sum / samples.length);
  }
  function reset(idcode: string) {
    windows.delete(idcode);
    offsetMap.set(idcode, null);
    maxMap.set(idcode, null);
    minMap.set(idcode, null);
  }
  function offsetOf(idcode: string): number | null {
    return offsetMap.get(idcode) ?? null;
  }
  function maxOf(idcode: string): number | null {
    return maxMap.get(idcode) ?? null;
  }
  function minOf(idcode: string): number | null {
    return minMap.get(idcode) ?? null;
  }
  return { tick, reset, offsetOf, maxOf, minOf };
}
