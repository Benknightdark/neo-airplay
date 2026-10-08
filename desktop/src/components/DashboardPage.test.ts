import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import DashboardPage from "./DashboardPage.vue";
import type { DashboardSnapshot } from "../types";

const dashboard: DashboardSnapshot = {
  settings: { receiverName: "Neo AirPlay", renderer: "Auto", fullscreen: false, startOnLogin: false },
  rendererOptions: ["Auto"],
  airplay: { status: "stopped", message: "AirPlay 接收器尚未啟動。", processId: null },
  android: { status: "stopped", message: "Android 投影尚未啟動。", processId: null },
  devices: [
    { serial: "phone-1", model: "Pixel 8", status: "ready" },
    { serial: "phone-2", model: "Pixel 7", status: "unauthorized" },
  ],
  logs: [],
};

describe("投影總覽", () => {
  it("只允許選擇已授權裝置，並送出選取序號", async () => {
    const wrapper = mount(DashboardPage, { props: { dashboard, busy: false } });
    const choices = wrapper.findAll("#android-device option");
    expect(choices).toHaveLength(2);
    expect(choices[1].text()).toContain("Pixel 8");
    await wrapper.get('[data-testid="android-start"]').trigger("click");
    expect(wrapper.emitted("start-android")).toEqual([["phone-1"]]);
  });

  it("會分別送出 AirPlay 與裝置刷新操作", async () => {
    const wrapper = mount(DashboardPage, { props: { dashboard, busy: false } });
    await wrapper.get('[data-testid="airplay-start"]').trigger("click");
    await wrapper.get('[data-testid="android-refresh"]').trigger("click");
    expect(wrapper.emitted("start-airplay")).toHaveLength(1);
    expect(wrapper.emitted("refresh-devices")).toHaveLength(1);
  });
});
