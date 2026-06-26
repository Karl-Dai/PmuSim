// 相量换算:CFG-2 FORMAT bit0=1 → 数据帧相量为 (magnitude, angle);bit0=0 → (real, imag)。
// C37.118 极坐标相角定义为弧度,此处统一输出"度"并规整到 (-180,180]。
function normalizeDeg(deg: number): number {
  let d = deg % 360;
  if (d <= -180) d += 360;
  if (d > 180) d -= 360;
  return d;
}

export function phasorMagAngle(pair: [number, number], polar: boolean): { mag: number; angleDeg: number } {
  if (polar) {
    return { mag: pair[0], angleDeg: normalizeDeg((pair[1] * 180) / Math.PI) };
  }
  const [re, im] = pair;
  return { mag: Math.hypot(re, im), angleDeg: normalizeDeg((Math.atan2(im, re) * 180) / Math.PI) };
}

export interface PhasorVector {
  mag: number;
  angleDeg: number;
  normLen: number; // 0..1,按本帧最大幅值归一化
}

export function computeVectors(phasors: [number, number][], polar: boolean): PhasorVector[] {
  const polarVals = phasors.map((p) => phasorMagAngle(p, polar));
  const maxMag = polarVals.reduce((m, v) => Math.max(m, v.mag), 0);
  return polarVals.map((v) => ({ mag: v.mag, angleDeg: v.angleDeg, normLen: maxMag > 0 ? v.mag / maxMag : 0 }));
}
