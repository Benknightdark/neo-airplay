/**
 * @file SettingsPage.test.ts
 * @description 驗證 `SettingsPage` 元件在背景輪詢更新相同設定時保留未儲存表單內容的單元測試。
 */

import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SettingsPage from "../../../desktop/src/components/SettingsPage.vue";
import { createReceiverSettings } from "../fixtures/builders";

/** 基準接收器設定測試資料。 */
const settings = createReceiverSettings();

/** 驗證 `SettingsPage` 設定表單與外部屬性同步策略。 */
describe("接收器設定", () => {
  /** 驗證相同設定快照重新傳入時保留使用者編輯中欄位，實質變更時才同步更新。 */
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
