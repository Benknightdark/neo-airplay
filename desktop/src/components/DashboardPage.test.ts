import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import DashboardPage from "./DashboardPage.vue";
import type { AndroidDeviceStatus, DashboardSnapshot } from "../types";

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

function dashboardWithDeviceStatuses(statuses: AndroidDeviceStatus[]): DashboardSnapshot {
  return {
    ...dashboard,
    devices: statuses.map((status, index) => ({
      serial: `phone-${index + 1}`,
      model: `Pixel ${8 - index}`,
      status,
    })),
  };
}

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

  it("AirPlay 等待 Bonjour 時會顯示啟動中並允許取消", async () => {
    const startingDashboard: DashboardSnapshot = {
      ...dashboard,
      airplay: { status: "starting", message: "正在等待 Bonjour 廣播。", processId: 42 },
    };
    const wrapper = mount(DashboardPage, { props: { dashboard: startingDashboard, busy: false } });

    expect(wrapper.get('[data-testid="airplay-status"]').text()).toContain("啟動中");
    expect(wrapper.get('[data-testid="airplay-stop"]').text()).toBe("取消啟動");
    expect(wrapper.get(".airplay-card .action-hint").text()).toContain("等待 Bonjour 廣播");
    await wrapper.get('[data-testid="airplay-stop"]').trigger("click");
    expect(wrapper.emitted("stop-airplay")).toHaveLength(1);
  });

  it.each([
    ["unauthorized", "找到未授權的裝置。請在 Android 裝置上確認並允許此電腦的偵錯連線。"],
    ["offline", "裝置目前離線，請確認手機已連線並重新整理裝置清單。"],
    ["unknown", "無法辨識裝置狀態，請重新整理裝置清單或檢查 ADB 連線。"],
  ] as const)("會為 %s 裝置顯示相符的排除提示", (status, hint) => {
    const wrapper = mount(DashboardPage, {
      props: { dashboard: dashboardWithDeviceStatuses([status]), busy: false },
    });

    expect(wrapper.get('[data-testid="android-device-hint"]').text()).toBe(hint);
  });

  it("混合多種不可用狀態時不會誤稱裝置未授權", () => {
    const wrapper = mount(DashboardPage, {
      props: { dashboard: dashboardWithDeviceStatuses(["offline", "unauthorized"]), busy: false },
    });

    const hint = wrapper.get('[data-testid="android-device-hint"]').text();
    expect(hint).toContain("找到裝置，但目前沒有可用裝置。");
    expect(hint).not.toContain("未授權");
  });
});
