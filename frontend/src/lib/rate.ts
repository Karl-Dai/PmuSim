// 用户可见 Hz → CFG-2 PERIOD（单位=工频周波×100）。
// 0Hz 特判为 PERIOD=0：非法上送周期（子站应 NACK），同时绕开 1000/hz 除零。
// 其余档位 PERIOD = round((1000/hz)*100/20) = round(5000/hz)
// （100→50, 50→100, 25→200, 200→25）。
export function hzToPeriod(hz: number): number {
  if (hz === 0) return 0;
  return Math.round((1000 / hz) * 100 / 20);
}

// 数据帧 SOC/FRACSEC → 绝对毫秒（V3 §8.11）。
//   ms = SOC*1000 + FRACSEC_count / (MEAS_RATE / 1000)
// FRACSEC 低 24 位为亚秒计数；高 8 位是时标质量码，须屏蔽。
// measRate(TIME_BASE) 缺省 1_000_000(微秒)；<=0 时退化为整秒避免除零。
export function frameTimeMs(soc: number, fracsec: number, measRate: number): number {
  const count = fracsec & 0xffffff;
  const msOffset = measRate > 0 ? count / (measRate / 1000) : 0;
  return soc * 1000 + msOffset;
}
