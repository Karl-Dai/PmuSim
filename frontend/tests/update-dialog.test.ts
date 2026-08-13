import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import UpdateDialog from "../src/components/UpdateDialog.vue";
import { setLocale } from "../src/i18n";

function button(label: string): HTMLButtonElement {
  const match = Array.from(document.body.querySelectorAll("button")).find(
    (element) => element.textContent?.trim() === label,
  );
  if (!(match instanceof HTMLButtonElement)) {
    throw new Error(`button "${label}" not found`);
  }
  return match;
}

beforeEach(() => {
  setLocale("en");
  invoke.mockReset();
  invoke.mockResolvedValue(undefined);
});

afterEach(() => {
  document.body.innerHTML = "";
});

describe("UpdateDialog", () => {
  it("shows only after the package is ready and maps all three choices", async () => {
    const wrapper = mount(UpdateDialog, {
      props: {
        visible: true,
        version: "0.13.2",
        notes: "- Background update",
      },
      attachTo: document.body,
    });

    expect(document.body.textContent).toContain("downloaded and verified");

    button("Skip This Version").click();
    await flushPromises();
    expect(invoke).toHaveBeenLastCalledWith("skip_update", { version: "0.13.2" });

    button("Update on Next Launch").click();
    await flushPromises();
    expect(invoke).toHaveBeenLastCalledWith("schedule_update_on_next_launch", {
      version: "0.13.2",
    });

    button("Update Now").click();
    await flushPromises();
    expect(invoke).toHaveBeenLastCalledWith("install_update", undefined);

    wrapper.unmount();
  });
});
