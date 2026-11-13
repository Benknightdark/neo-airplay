/**
 * @file DashboardPage.test.ts
 * @description 驗證 `DashboardPage` 元件在 Android 裝置篩選、事件派發、AirPlay 狀態提示與錯誤說明上的單元測試。
 */

import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import DashboardPage from "../../../desktop/src/components/DashboardPage.vue";
import type { AndroidDeviceStatus } from "../../../desktop/src/domain";
import { createAndroidDevice, createDashboardSnapshot, createEngineSnapshot } from "../fixtures/builders";

/** 預設包含已授權與未授權手機的測試儀表板快照。 */
const dashboard = createDashboardSnapshot({
  devices: [
    createAndroidDevice({ serial: "phone-1", model: "Pixel 8", status: "ready" }),
    createAndroidDevice({ serial: "phone-2", model: "Pixel 7", status: "unauthorized" }),
  ],
});

/**
 * 依指定的裝置狀態清單產生對應的測試儀表板快照。
 *
 * @param statuses 欲模擬的 Android 裝置狀態陣列。
 * @returns 內含對應數量模擬裝置的 {@link DashboardSnapshot}。
 */
function dashboardWithDeviceStatuses(statuses: AndroidDeviceStatus[]) {
  return createDashboardSnapshot({
    devices: statuses.map((status, index) =>
      createAndroidDevice({
        serial: `phone-${index + 1}`,
        model: `Pixel ${8 - index}`,
        status,
      }),
    ),
  });
}

/** 驗證 `DashboardPage` 投影總覽介面互動與提示文字。 */
describe("投影總覽", () => {
  /** 驗證下拉選單僅列出已授權裝置，且啟動投影時帶入選取的裝置序號。 */
  it("只允許選擇已授權裝置，並送出選取序號", async () => {
    const wrapper = mount(DashboardPage, { props: { dashboard, busy: false } });
    const choices = wrapper.findAll("#android-device option");
    expect(choices).toHaveLength(2);
    expect(choices[1].text()).toContain("Pixel 8");
    await wrapper.get('[data-testid="android-start"]').trigger("click");
    expect(wrapper.emitted("start-android")).toEqual([["phone-1"]]);
  });

  /** 驗證點擊啟動 AirPlay 與重新整理裝置按鈕時分別派發對應事件。 */
  it("會分別送出 AirPlay 與裝置刷新操作", async () => {
    const wrapper = mount(DashboardPage, { props: { dashboard, busy: false } });
    await wrapper.get('[data-testid="airplay-start"]').trigger("click");
    await wrapper.get('[data-testid="android-refresh"]').trigger("click");
    expect(wrapper.emitted("start-airplay")).toHaveLength(1);
    expect(wrapper.emitted("refresh-devices")).toHaveLength(1);
  });

  /** 驗證引擎處於 `starting` 狀態時顯示取消按鈕並允許派發停止事件。 */
  it("AirPlay 等待 Bonjour 時會顯示啟動中並允許取消", async () => {
    const startingDashboard = createDashboardSnapshot({
      airplay: createEngineSnapshot({
        status: "starting",
        message: "正在等待 Bonjour 廣播。",
        processId: 42,
      }),
    });
    const wrapper = mount(DashboardPage, { props: { dashboard: startingDashboard, busy: false } });

    expect(wrapper.get('[data-testid="airplay-status"]').text()).toContain("啟動中");
    expect(wrapper.get('[data-testid="airplay-stop"]').text()).toBe("取消啟動");
    expect(wrapper.get(".airplay-card .action-hint").text()).toContain("等待 Bonjour 廣播");
    await wrapper.get('[data-testid="airplay-stop"]').trigger("click");
    expect(wrapper.emitted("stop-airplay")).toHaveLength(1);
  });

  /** 驗證單一不可用狀態裝置會呈現精確的疑難排解說明。 */
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

  /** 驗證多種不可用裝置狀態並存時顯示通用提示，避免誤報單一原因。 */
  it("混合多種不可用狀態時不會誤稱裝置未授權", () => {
    const wrapper = mount(DashboardPage, {
      props: { dashboard: dashboardWithDeviceStatuses(["offline", "unauthorized"]), busy: false },
    });

    const hint = wrapper.get('[data-testid="android-device-hint"]').text();
    expect(hint).toContain("找到裝置，但目前沒有可用裝置。");
    expect(hint).not.toContain("未授權");
  });
});
