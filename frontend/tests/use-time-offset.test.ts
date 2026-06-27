import { describe, it, expect, beforeEach } from "vitest";
import { useTimeOffset } from "../src/composables/useTimeOffset";

describe("useTimeOffset 报文-本机时偏滑动均值(按 idcode)", () => {
  beforeEach(() => {
    const { reset } = useTimeOffset();
    reset("A");
    reset("B");
  });

  it("均值随样本更新,正负保留", () => {
    const { tick, offsetOf } = useTimeOffset();
    tick("A", 10);
    tick("A", -10);
    tick("A", 30);
    expect(offsetOf("A")).toBeCloseTo(10, 5);
  });

  it("两个子站各自独立,互不串台", () => {
    const { tick, offsetOf } = useTimeOffset();
    tick("A", 100);
    tick("B", -50);
    expect(offsetOf("A")).toBe(100);
    expect(offsetOf("B")).toBe(-50);
  });

  it("无样本 / 未知 idcode 读数为 null", () => {
    const { offsetOf, reset } = useTimeOffset();
    reset("A");
    expect(offsetOf("A")).toBeNull();
    expect(offsetOf("nope")).toBeNull();
  });

  it("max/min 随样本更新,正负保留", () => {
    const { tick, offsetOf, maxOf, minOf } = useTimeOffset();
    tick("A", 10);
    tick("A", -10);
    tick("A", 30);
    expect(maxOf("A")).toBe(30);
    expect(minOf("A")).toBe(-10);
    expect(offsetOf("A")).toBeCloseTo(10, 5);
  });

  it("max 滑出窗口后重扫回落", () => {
    const { tick, maxOf, minOf } = useTimeOffset();
    // 窗口 50 帧:第一帧是最大值 100,其余 49 帧为 1。
    tick("A", 100);
    for (let i = 0; i < 49; i++) tick("A", 1);
    expect(maxOf("A")).toBe(100);
    expect(minOf("A")).toBe(1);
    // 第 51 帧=1,滑出 100(当前 max)→ 重扫,max 回落到 1。
    tick("A", 1);
    expect(maxOf("A")).toBe(1);
    expect(minOf("A")).toBe(1);
  });

  it("min 滑出窗口后重扫回落", () => {
    const { tick, maxOf, minOf } = useTimeOffset();
    // 第一帧是最小值 -100,其余 49 帧为 0。
    tick("A", -100);
    for (let i = 0; i < 49; i++) tick("A", 0);
    expect(minOf("A")).toBe(-100);
    // 第 51 帧=0,滑出 -100(当前 min)→ 重扫,min 回落到 0。
    tick("A", 0);
    expect(minOf("A")).toBe(0);
    expect(maxOf("A")).toBe(0);
  });

  it("max/min 两子站隔离", () => {
    const { tick, maxOf, minOf } = useTimeOffset();
    tick("A", 100);
    tick("B", -50);
    tick("A", 5);
    tick("B", 200);
    expect(maxOf("A")).toBe(100);
    expect(minOf("A")).toBe(5);
    expect(maxOf("B")).toBe(200);
    expect(minOf("B")).toBe(-50);
  });

  it("reset 后 max/min 归 null,未知 idcode 也为 null", () => {
    const { tick, maxOf, minOf, reset } = useTimeOffset();
    tick("A", 10);
    tick("A", -5);
    expect(maxOf("A")).toBe(10);
    reset("A");
    expect(maxOf("A")).toBeNull();
    expect(minOf("A")).toBeNull();
    expect(maxOf("nope")).toBeNull();
    expect(minOf("nope")).toBeNull();
  });
});
