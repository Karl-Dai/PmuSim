import { describe, it, expect, beforeEach } from "vitest";
import { useFrameRate } from "../src/composables/useFrameRate";
import { frameTimeMs } from "../src/lib/rate";

describe("frameTimeMs (报文 SOC/FRACSEC → 毫秒)", () => {
  it("soc 秒 + fracsec 小数秒按 measRate(TIME_BASE) 换算", () => {
    // measRate=1e6: msOffset = count / (1e6/1000) = count/1000
    expect(frameTimeMs(10, 500_000, 1_000_000)).toBe(10_500);
  });

  it("屏蔽 fracsec 高 8 位时标质量码，只取低 24 位计数", () => {
    const fracsec = (0x0f << 24) | 500_000; // 高位 = §8.11 表4 GPS 时标质量
    expect(frameTimeMs(10, fracsec, 1_000_000)).toBe(10_500);
  });

  it("measRate<=0 时退化为整秒，避免除零", () => {
    expect(frameTimeMs(10, 500_000, 0)).toBe(10_000);
  });
});

describe("useFrameRate 基于报文时间戳反推帧率(按 idcode)", () => {
  beforeEach(() => {
    const { reset } = useFrameRate();
    reset("A");
    reset("B");
  });

  it("以最近 1 秒(报文时间)内的帧数为 fps", () => {
    const { tick, fpsOf } = useFrameRate();
    for (let i = 0; i < 150; i++) tick("A", i * 10);
    expect(fpsOf("A")).toBe(101);
  });

  it("两个子站各自独立计数,互不串台", () => {
    const { tick, fpsOf } = useFrameRate();
    for (let i = 0; i < 150; i++) tick("A", i * 10); // 101
    tick("B", 0);
    tick("B", 500);
    expect(fpsOf("A")).toBe(101);
    expect(fpsOf("B")).toBe(2);
  });

  it("报文时间倒退时重置该 idcode 窗口", () => {
    const { tick, fpsOf } = useFrameRate();
    tick("A", 100_000);
    tick("A", 100_010);
    tick("A", 5); // 倒退 → 仅保留当前帧
    expect(fpsOf("A")).toBe(1);
  });

  it("reset(idcode) 清零且不影响其他 idcode", () => {
    const { tick, fpsOf, reset } = useFrameRate();
    tick("A", 0);
    tick("B", 0);
    reset("A");
    expect(fpsOf("A")).toBe(0);
    expect(fpsOf("B")).toBe(1);
  });

  it("未知 idcode 读数为 0", () => {
    expect(useFrameRate().fpsOf("nope")).toBe(0);
  });
});
