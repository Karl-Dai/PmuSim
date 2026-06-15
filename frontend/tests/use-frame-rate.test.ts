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

describe("useFrameRate 基于报文时间戳反推帧率", () => {
  beforeEach(() => useFrameRate().reset());

  it("以最近 1 秒(报文时间)内的帧数为 fps", () => {
    const { tick, fps } = useFrameRate();
    // 100Hz：每帧报文时间间隔 10ms，0..1490ms 共 150 帧。
    // 末帧 1490ms → cutoff=490ms，窗口保留 [490,1490] = 101 帧。
    for (let i = 0; i < 150; i++) tick(i * 10);
    expect(fps.value).toBe(101);
  });

  it("超出 1000ms 窗口的旧帧被剔除", () => {
    const { tick, fps } = useFrameRate();
    tick(0);
    tick(500);
    tick(1500); // cutoff=500 → 剔除 0ms 帧，保留 500/1500
    expect(fps.value).toBe(2);
  });

  it("报文时间倒退(子站重启/校时/SOC回绕)时重置窗口，避免虚高", () => {
    const { tick, fps } = useFrameRate();
    tick(100_000);
    tick(100_010);
    tick(100_020); // fps=3
    tick(5); // 时间大幅倒退 → 重置，仅保留当前帧
    expect(fps.value).toBe(1);
  });

  it("reset() 清零", () => {
    const { tick, fps, reset } = useFrameRate();
    tick(0);
    tick(10);
    reset();
    expect(fps.value).toBe(0);
  });
});
