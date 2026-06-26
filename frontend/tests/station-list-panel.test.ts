import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import StationListPanel from "../src/components/StationListPanel.vue";
import { useSessions } from "../src/composables/useSessions";
import { setLocale } from "../src/i18n";

beforeEach(() => {
  setLocale("en");
  useSessions().clear();
});

describe("StationListPanel", () => {
  it("渲染所有会话行并按 state 映射 LED 类", () => {
    const { addSession, updateState } = useSessions();
    addSession("PMU_A", "1.1.1.1", "1.1.1.1:8000");
    updateState("PMU_A", "streaming");
    addSession("PMU_B", "2.2.2.2", "2.2.2.2:8000");
    updateState("PMU_B", "disconnected");
    const wrapper = mount(StationListPanel);
    const rows = wrapper.findAll(".station-row");
    expect(rows.length).toBe(2);
    expect(wrapper.find(".led-ok").exists()).toBe(true);
    expect(wrapper.find(".led-err").exists()).toBe(true);
  });

  it("点击行切换 selectedIdcode", async () => {
    const { addSession, selectedIdcode } = useSessions();
    addSession("PMU_A", "1.1.1.1", "1.1.1.1:8000");
    addSession("PMU_B", "2.2.2.2", "2.2.2.2:8000"); // 新增即选中 B
    const wrapper = mount(StationListPanel);
    await wrapper.findAll(".station-row")[0].trigger("click");
    expect(selectedIdcode.value).toBe("PMU_A");
  });

  it("无会话显示空态", () => {
    const wrapper = mount(StationListPanel);
    expect(wrapper.find(".station-empty").exists()).toBe(true);
  });
});
