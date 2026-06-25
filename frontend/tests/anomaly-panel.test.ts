import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount } from "@vue/test-utils";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
const { save } = vi.hoisted(() => ({ save: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save }));

import AnomalyPanel from "../src/components/AnomalyPanel.vue";
import { useAnomalyLog } from "../src/composables/useAnomalyLog";
import type { PmuEvent } from "../src/types";

function ev(over: Partial<Extract<PmuEvent, { type: "TimestampAnomaly" }>> = {}) {
  return {
    type: "TimestampAnomaly",
    idcode: "PMU1",
    kind: "gap",
    expected_ms: 20,
    actual_ms: 40,
    soc: 1781,
    fracsec: 0x000d9490,
    frame_time: "2026-06-23 14:30:45",
    ...over,
  } as Extract<PmuEvent, { type: "TimestampAnomaly" }>;
}

beforeEach(() => {
  useAnomalyLog().clear();
  invoke.mockReset();
  save.mockReset();
});

describe("AnomalyPanel", () => {
  it("展开后渲染异常行，行数随数据增长", async () => {
    const { push } = useAnomalyLog();
    push(ev({ kind: "gap" }));
    push(ev({ kind: "backward", idcode: "PMU2" }));
    const wrapper = mount(AnomalyPanel);
    // 默认折叠，先展开
    await wrapper.find(".anomaly-header").trigger("click");
    expect(wrapper.findAll(".anomaly-row").length).toBe(2);
  });

  it("按类型筛选只显示该类型", async () => {
    const { push } = useAnomalyLog();
    push(ev({ kind: "gap" }));
    push(ev({ kind: "stall" }));
    const wrapper = mount(AnomalyPanel);
    await wrapper.find(".anomaly-header").trigger("click");
    await wrapper.find("select.filter-kind").setValue("gap");
    expect(wrapper.findAll(".anomaly-row").length).toBe(1);
  });

  it("清空后无行", async () => {
    const { push } = useAnomalyLog();
    push(ev());
    const wrapper = mount(AnomalyPanel);
    await wrapper.find(".anomaly-header").trigger("click");
    await wrapper.find("button.btn-clear").trigger("click");
    expect(wrapper.findAll(".anomaly-row").length).toBe(0);
  });

  it("空态时导出按钮禁用", async () => {
    const wrapper = mount(AnomalyPanel);
    await wrapper.find(".anomaly-header").trigger("click");
    expect(wrapper.find("button.btn-export").attributes("disabled")).toBeDefined();
  });

  it("有数据时导出调用 save 与 save_text_file", async () => {
    const { push } = useAnomalyLog();
    push(ev());
    save.mockResolvedValueOnce("/tmp/out.csv");
    invoke.mockResolvedValueOnce(undefined);
    const wrapper = mount(AnomalyPanel);
    await wrapper.find(".anomaly-header").trigger("click");
    await wrapper.find("button.btn-export").trigger("click");
    await Promise.resolve();
    await Promise.resolve();
    expect(save).toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith("save_text_file", expect.objectContaining({ path: "/tmp/out.csv" }));
  });

  it("gap 行显示丢帧估算", async () => {
    const { push } = useAnomalyLog();
    push(ev({ kind: "gap", expected_ms: 20, actual_ms: 60 }));
    const wrapper = mount(AnomalyPanel);
    await wrapper.find(".anomaly-header").trigger("click");
    expect(wrapper.find(".anomaly-row .col-dropped").text()).toContain("2");
  });
});
