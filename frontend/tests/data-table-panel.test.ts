import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import DataTablePanel from "../src/components/DataTablePanel.vue";
import { useSessions } from "../src/composables/useSessions";
import { useCommLog } from "../src/composables/useCommLog";
import { setLocale } from "../src/i18n";
import type { ConfigInfo, DataInfo } from "../src/types";

function cfg(over: Partial<ConfigInfo> = {}): ConfigInfo {
  return { cfgType: 2, version: 3, stn: "S", idcode: "X", formatFlags: 1, period: 100, measRate: 1_000_000, phnmr: 0, annmr: 1, dgnmr: 0, channelNames: ["AN1"], anunit: [0], ...over };
}
function data(stat: number, analog: number[]): DataInfo {
  return { soc: 0, fracsec: 0, stat, format_flags: 1, time_quality: 0, freq: 0, dfreq: 0, analog, digital: [], phasors: [], local_offset_ms: 0 };
}

beforeEach(() => {
  setLocale("en");
  useSessions().clear();
  useCommLog().clear();
});

describe("DataTablePanel 跟随选中子站", () => {
  it("显示选中子站的数据与其 cfg,不串台", async () => {
    const { addSession, setConfig, selectedIdcode } = useSessions();
    const { addData } = useCommLog();
    addSession("A", "1.1.1.1", "1.1.1.1:8000");
    setConfig("A", cfg({ channelNames: ["A_AN"] }));
    addData("A", data(0, [111]));
    addSession("B", "2.2.2.2", "2.2.2.2:8000");
    setConfig("B", cfg({ channelNames: ["B_AN"] }));
    addData("B", data(0, [222]));
    selectedIdcode.value = "A";
    const wrapper = mount(DataTablePanel);
    expect(wrapper.text()).toContain("A_AN");
    expect(wrapper.text()).toContain("111");
    expect(wrapper.text()).not.toContain("222");
  });

  it("渲染相量行(直角坐标→幅值/相角)", () => {
    const { addSession, setConfig, selectedIdcode } = useSessions();
    const { addData } = useCommLog();
    addSession("A", "1.1.1.1", "1.1.1.1:8000");
    setConfig("A", cfg({ phnmr: 1, annmr: 0, channelNames: ["Ua"] }));
    const d = data(0, []);
    d.format_flags = 0; // 直角
    d.phasors = [[3, 4]]; // mag 5, angle 53.13°
    addData("A", d);
    selectedIdcode.value = "A";
    const wrapper = mount(DataTablePanel);
    expect(wrapper.text()).toContain("Ua");
    expect(wrapper.text()).toContain("5.000");
    expect(wrapper.text()).toContain("53.13");
  });
});
