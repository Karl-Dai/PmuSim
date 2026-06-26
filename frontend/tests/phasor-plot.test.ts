import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import PhasorPlot from "../src/components/PhasorPlot.vue";
import { useSessions } from "../src/composables/useSessions";
import { useCommLog } from "../src/composables/useCommLog";
import { setLocale } from "../src/i18n";
import type { ConfigInfo, DataInfo } from "../src/types";

function cfg(phnmr: number): ConfigInfo {
  return { cfgType: 2, version: 3, stn: "S", idcode: "X", formatFlags: 1, period: 100, measRate: 1_000_000, phnmr, annmr: 0, dgnmr: 0, channelNames: Array.from({ length: phnmr }, (_, i) => `PH${i}`), anunit: [] };
}
function data(phasors: [number, number][]): DataInfo {
  return { soc: 0, fracsec: 0, stat: 0, format_flags: 0, time_quality: 0, freq: 0, dfreq: 0, analog: [], digital: [], phasors, local_offset_ms: 0 };
}

beforeEach(() => {
  setLocale("en");
  useSessions().clear();
  useCommLog().clear();
});

describe("PhasorPlot", () => {
  it("无相量数据显示占位", () => {
    const wrapper = mount(PhasorPlot);
    expect(wrapper.find(".phasor-empty").exists()).toBe(true);
  });

  it("有相量数据渲染 canvas", async () => {
    const { addSession, setConfig, selectedIdcode } = useSessions();
    const { addData } = useCommLog();
    addSession("A", "1.1.1.1", "1.1.1.1:8000");
    setConfig("A", cfg(2));
    addData("A", data([[3, 4], [1, 0]]));
    selectedIdcode.value = "A";
    const wrapper = mount(PhasorPlot);
    expect(wrapper.find("canvas").exists()).toBe(true);
  });
});
