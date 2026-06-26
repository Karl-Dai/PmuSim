import { describe, it, expect, beforeEach } from "vitest";
import { useSessions } from "../src/composables/useSessions";

describe("useSessions 多子站选中管理", () => {
  beforeEach(() => useSessions().clear());

  it("新增会话即成为选中项", () => {
    const { addSession, selectedIdcode } = useSessions();
    addSession("A", "1.1.1.1");
    expect(selectedIdcode.value).toBe("A");
    addSession("B", "2.2.2.2");
    expect(selectedIdcode.value).toBe("B");
  });

  it("移除选中项回退到剩余第一个", () => {
    const { addSession, removeSession, selectedIdcode } = useSessions();
    addSession("A", "1.1.1.1");
    addSession("B", "2.2.2.2");
    selectedIdcode.value = "A";
    removeSession("A");
    expect(selectedIdcode.value).toBe("B");
  });

  it("移除最后一个会话置空选中", () => {
    const { addSession, removeSession, selectedIdcode } = useSessions();
    addSession("A", "1.1.1.1");
    removeSession("A");
    expect(selectedIdcode.value).toBe("");
  });

  it("setDialKey 写入会话", () => {
    const { addSession, setDialKey, sessions } = useSessions();
    addSession("A", "1.1.1.1");
    setDialKey("A", "1.1.1.1:8000");
    expect(sessions.get("A")?.dialKey).toBe("1.1.1.1:8000");
  });
});
