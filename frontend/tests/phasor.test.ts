import { describe, it, expect } from "vitest";
import { phasorMagAngle, computeVectors } from "../src/lib/phasor";

describe("phasorMagAngle", () => {
  it("直角坐标 → 幅值/相角(度)", () => {
    const r = phasorMagAngle([3, 4], false);
    expect(r.mag).toBeCloseTo(5, 6);
    expect(r.angleDeg).toBeCloseTo(53.1301, 3);
  });

  it("极坐标(弧度)→ 角度转度", () => {
    const r = phasorMagAngle([10, Math.PI / 2], true);
    expect(r.mag).toBe(10);
    expect(r.angleDeg).toBeCloseTo(90, 6);
  });

  it("角度规整到 (-180,180]", () => {
    const r = phasorMagAngle([-1, 0], false); // atan2(0,-1)=π → 180
    expect(r.angleDeg).toBeCloseTo(180, 6);
  });
});

describe("computeVectors", () => {
  it("按最大幅值归一化", () => {
    const v = computeVectors([[3, 4], [6, 8]], false); // mag 5, 10
    expect(v[0].normLen).toBeCloseTo(0.5, 6);
    expect(v[1].normLen).toBeCloseTo(1, 6);
  });
  it("全零相量 normLen=0 不除零", () => {
    const v = computeVectors([[0, 0]], false);
    expect(v[0].normLen).toBe(0);
  });
});
