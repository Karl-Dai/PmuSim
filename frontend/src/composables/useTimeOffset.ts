import { ref } from "vue";

// 报文时间与本机时钟的偏差(ms)滑动均值。每帧偏差由后端在接收时刻采样
// (now − 报文时间戳)写入 DataInfo.local_offset_ms；这里保留最近 N 帧定长
// 计数窗求均值，抹平逐帧网络抖动。正=报文滞后本地，负=报文超前本地。
// 模块级单例：usePmuEvents 逐帧 tick、ConfigInfoPanel 读 offsetMs，共享同窗。

const WINDOW = 50;
const samples: number[] = [];
const offsetMs = ref<number | null>(null);

export function useTimeOffset() {
  function tick(ms: number) {
    samples.push(ms);
    if (samples.length > WINDOW) samples.shift();
    const sum = samples.reduce((a, b) => a + b, 0);
    offsetMs.value = sum / samples.length;
  }
  function reset() {
    samples.length = 0;
    offsetMs.value = null;
  }
  return { offsetMs, tick, reset };
}
