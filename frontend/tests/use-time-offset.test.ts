import { describe, it, expect, beforeEach } from "vitest";
import { useTimeOffset } from "../src/composables/useTimeOffset";

describe("useTimeOffset 最近 50 帧偏差滑动均值", () => {
  beforeEach(() => useTimeOffset().reset());

  it("样本数 < 窗口时取全部样本均值", () => {
    const { tick, offsetMs } = useTimeOffset();
    tick(100);
    tick(200);
    expect(offsetMs.value).toBe(150);
  });

  it("超过 50 帧只保留最近 50 帧求均值", () => {
    const { tick, offsetMs } = useTimeOffset();
    // 推 60 帧 0..59 → 窗口保留 10..59，均值 (10+59)/2 = 34.5。
    for (let i = 0; i < 60; i++) tick(i);
    expect(offsetMs.value).toBe(34.5);
  });

  it("负偏差(报文超前本地)如实平均", () => {
    const { tick, offsetMs } = useTimeOffset();
    tick(-20);
    tick(-40);
    expect(offsetMs.value).toBe(-30);
  });

  it("reset() 后回到 null（显示 —）", () => {
    const { tick, offsetMs, reset } = useTimeOffset();
    tick(10);
    reset();
    expect(offsetMs.value).toBeNull();
  });
});
