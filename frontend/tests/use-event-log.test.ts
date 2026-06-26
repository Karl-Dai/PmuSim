import { describe, it, expect, beforeEach } from "vitest";
import { useEventLog } from "../src/composables/useEventLog";

describe("useEventLog 按 idcode 过滤", () => {
  beforeEach(() => useEventLog().clear());

  it("entriesFor 只返回该子站 + 广播(空 idcode)条目", () => {
    const { push, entriesFor } = useEventLog();
    push("A", "a1");
    push("B", "b1");
    push("", "broadcast", "error");
    const a = entriesFor("A").map((e) => e.message);
    expect(a).toContain("a1");
    expect(a).toContain("broadcast");
    expect(a).not.toContain("b1");
  });

  it("kind 默认 info", () => {
    const { push, entriesFor } = useEventLog();
    push("A", "x");
    expect(entriesFor("A")[0].kind).toBe("info");
  });
});
