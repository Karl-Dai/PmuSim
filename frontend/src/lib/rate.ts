// 用户可见 Hz → CFG-2 PERIOD（单位=工频周波×100）。
// 0Hz 特判为 PERIOD=0：非法上送周期（子站应 NACK），同时绕开 1000/hz 除零。
// 其余档位 PERIOD = round((1000/hz)*100/20) = round(5000/hz)
// （100→50, 50→100, 25→200, 200→25）。
export function hzToPeriod(hz: number): number {
  if (hz === 0) return 0;
  return Math.round((1000 / hz) * 100 / 20);
}
