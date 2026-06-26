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
});
