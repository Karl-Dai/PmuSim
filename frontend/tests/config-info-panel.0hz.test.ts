import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount, flushPromises, type VueWrapper, type DOMWrapper } from "@vue/test-utils";

// Mock the only two Tauri touchpoints the rate watcher uses. vi.hoisted so the
// fns exist before the (hoisted) vi.mock factories run.
const { invoke, ask } = vi.hoisted(() => ({ invoke: vi.fn(), ask: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ ask }));

import ConfigInfoPanel from "../src/components/ConfigInfoPanel.vue";
import { useSessions } from "../src/composables/useSessions";

// The rate <select> is the only one carrying a value="0" option.
function rateSelect(wrapper: VueWrapper): DOMWrapper<Element> {
  const sel = wrapper
    .findAll("select")
    .find((s) => s.findAll("option").some((o) => (o.element as HTMLOptionElement).value === "0"));
  if (!sel) throw new Error("rate <select> (with a 0 Hz option) not found");
  return sel;
}
const rateValue = (wrapper: VueWrapper) => (rateSelect(wrapper).element as HTMLSelectElement).value;

function setStreaming() {
  const { sessions, selectedIdcode } = useSessions();
  sessions.set("PMU1", { idcode: "PMU1", peerIp: "10.0.0.1", state: "streaming" });
  selectedIdcode.value = "PMU1";
}

beforeEach(() => {
  const { sessions, selectedIdcode } = useSessions();
  sessions.clear();
  selectedIdcode.value = "";
  invoke.mockReset();
  invoke.mockResolvedValue(undefined);
  ask.mockReset();
});

describe("ConfigInfoPanel 速率下拉 0 Hz (异常场景)", () => {
  it("streaming 时选 0Hz 并确认 → 下发 CFG-2 PERIOD=0", async () => {
    setStreaming();
    ask.mockResolvedValue(true);
    const wrapper = mount(ConfigInfoPanel);

    await rateSelect(wrapper).setValue("0");
    await flushPromises();

    expect(ask).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("send_command", { idcode: "PMU1", cmd: "send_cfg2_cmd", period: null });
    expect(invoke).toHaveBeenCalledWith("send_command", { idcode: "PMU1", cmd: "send_cfg2", period: 0 });
    wrapper.unmount();
  });

  it("选 0Hz 但取消 → 回退到上一档且不下发任何 CFG-2", async () => {
    setStreaming();
    ask.mockResolvedValue(false);
    const wrapper = mount(ConfigInfoPanel);
    expect(rateValue(wrapper)).toBe("100"); // 默认档位

    await rateSelect(wrapper).setValue("0");
    await flushPromises();

    expect(ask).toHaveBeenCalledTimes(1);
    expect(invoke).not.toHaveBeenCalled(); // 取消：不下发，且 suppress 阻止回退误发
    expect(rateValue(wrapper)).toBe("100"); // 已回退
    wrapper.unmount();
  });

  it("未连接时选 0Hz 并确认 → 不立即下发，保持 0Hz 选中（由启动带下去）", async () => {
    // 无 session
    ask.mockResolvedValue(true);
    const wrapper = mount(ConfigInfoPanel);

    await rateSelect(wrapper).setValue("0");
    await flushPromises();

    expect(ask).toHaveBeenCalledTimes(1);
    expect(invoke).not.toHaveBeenCalled();
    expect(rateValue(wrapper)).toBe("0");
    wrapper.unmount();
  });

  it("streaming 时选正常档位 50Hz → 防抖后下发 PERIOD=100，且无确认框", async () => {
    setStreaming();
    const wrapper = mount(ConfigInfoPanel);

    await rateSelect(wrapper).setValue("50");
    await new Promise((r) => setTimeout(r, 300)); // 等 250ms 防抖
    await flushPromises();

    expect(ask).not.toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith("send_command", { idcode: "PMU1", cmd: "send_cfg2_cmd", period: null });
    expect(invoke).toHaveBeenCalledWith("send_command", { idcode: "PMU1", cmd: "send_cfg2", period: 100 }); // hzToPeriod(50)
    wrapper.unmount();
  });

  it("streaming 时选 10Hz → 防抖后下发 PERIOD=500，且无确认框", async () => {
    setStreaming();
    const wrapper = mount(ConfigInfoPanel);

    await rateSelect(wrapper).setValue("10");
    await new Promise((r) => setTimeout(r, 300)); // 等 250ms 防抖
    await flushPromises();

    expect(ask).not.toHaveBeenCalled(); // 正常档位：不弹确认框
    expect(invoke).toHaveBeenCalledWith("send_command", { idcode: "PMU1", cmd: "send_cfg2_cmd", period: null });
    expect(invoke).toHaveBeenCalledWith("send_command", { idcode: "PMU1", cmd: "send_cfg2", period: 500 }); // hzToPeriod(10)
    wrapper.unmount();
  });
});
