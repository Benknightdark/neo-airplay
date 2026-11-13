/**
 * @file DiagnosticsPage.test.ts
 * @description 驗證 `DiagnosticsPage` 元件在系統記錄倒序渲染、層級篩選與空狀態顯示上的單元測試。
 */

import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import DiagnosticsPage from "../../../desktop/src/components/DiagnosticsPage.vue";
import { createLogEntry } from "../fixtures/builders";

/** 驗證 `DiagnosticsPage` 診斷記錄頁行為。 */
describe("診斷記錄頁", () => {
  /** 驗證記錄依時間由新到舊顯示，並可切換層級過濾條件或呈現無資料提示。 */
  it("以最新優先顯示記錄並支援依層級篩選", async () => {
    const logs = [
      createLogEntry({ timestamp: "2026-11-13T10:00:00Z", level: "information", message: "啟動完成" }),
      createLogEntry({ timestamp: "2026-11-13T10:05:00Z", level: "error", message: "ADB 連線逾時" }),
    ];
    const wrapper = mount(DiagnosticsPage, { props: { logs } });

    const items = wrapper.findAll(".log-entry");
    expect(items).toHaveLength(2);
    expect(items[0].text()).toContain("ADB 連線逾時");

    await wrapper.get('[data-testid="log-filter"]').setValue("information");
    const filtered = wrapper.findAll(".log-entry");
    expect(filtered).toHaveLength(1);
    expect(filtered[0].text()).toContain("啟動完成");

    await wrapper.get('[data-testid="log-filter"]').setValue("warning");
    expect(wrapper.find(".empty-state").text()).toContain("目前沒有符合條件的記錄。");
  });
});
