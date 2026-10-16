import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SettingsPage from "./SettingsPage.vue";
import type { ReceiverSettings } from "../types";

const settings: ReceiverSettings = {
  receiverName: "Neo AirPlay",
  renderer: "Auto",
  fullscreen: false,
  startOnLogin: false,
};

describe("接收器設定", () => {
  it("輪詢收到相同設定時保留尚未儲存的表單內容", async () => {
    const wrapper = mount(SettingsPage, {
      props: { settings, rendererOptions: ["Auto"], busy: false },
    });

    await wrapper.get('[data-testid="receiver-name"]').setValue("Unsaved Receiver");
    await wrapper.setProps({ settings: { ...settings } });

    expect(wrapper.get('[data-testid="receiver-name"]').element).toHaveProperty(
      "value",
      "Unsaved Receiver",
    );

    await wrapper.setProps({ settings: { ...settings, receiverName: "Saved Receiver" } });
    expect(wrapper.get('[data-testid="receiver-name"]').element).toHaveProperty(
      "value",
      "Saved Receiver",
    );
  });
});
