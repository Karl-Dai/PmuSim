import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";

const { invoke, ask } = vi.hoisted(() => ({ invoke: vi.fn(), ask: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ ask }));

import ConfigInfoPanel from "../src/components/ConfigInfoPanel.vue";
import { useSessions } from "../src/composables/useSessions";
import { useReconnect } from "../src/composables/useReconnect";
import { usePmuEvents } from "../src/composables/usePmuEvents";
import { setLocale } from "../src/i18n";

const reconnect = useReconnect();

beforeEach(() => {
  setLocale("en");
  const { sessions, selectedIdcode } = useSessions();
  sessions.clear();
  selectedIdcode.value = "";
  reconnect._resetForTest();
  invoke.mockReset();
  invoke.mockResolvedValue(undefined);
  ask.mockReset();
  // startEverything 第一步 await listenerReady;启动轮询使其 resolve。
  usePmuEvents().startListening();
});

function findButtonByText(wrapper: ReturnType<typeof mount>, text: string) {
  const btn = wrapper.findAll("button").find((b) => b.text().includes(text));
  if (!btn) throw new Error(`button "${text}" not found`);
  return btn;
}

describe("ConfigInfoPanel 重连接线", () => {
  it("连接成功后 arm(mode=normal) 携带表单快照", async () => {
    const armSpy = vi.spyOn(reconnect, "arm");
    const wrapper = mount(ConfigInfoPanel);
    await findButtonByText(wrapper, "Start").trigger("click");
    await flushPromises();
    expect(armSpy).toHaveBeenCalledTimes(1);
    const arg = armSpy.mock.calls[0][0];
    expect(arg).toMatchObject({ host: "10.15.48.12", mgmtPort: 8000, dataPort: 8001, protocol: "V3", mode: "normal" });
    wrapper.unmount();
  });

  it("停止后 cancel 被调用", async () => {
    const cancelSpy = vi.spyOn(reconnect, "cancel");
    const { sessions, selectedIdcode } = useSessions();
    sessions.set("PMU1", { idcode: "PMU1", peerIp: "1.1.1.1", state: "streaming" });
    selectedIdcode.value = "PMU1";
    const wrapper = mount(ConfigInfoPanel);
    await findButtonByText(wrapper, "Stop").trigger("click");
    await flushPromises();
    expect(cancelSpy).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });
});
