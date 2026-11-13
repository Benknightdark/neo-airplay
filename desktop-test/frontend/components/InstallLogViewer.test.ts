/**
 * @file InstallLogViewer.test.ts
 * @description 驗證 `InstallLogViewer` 元件在記錄載入、唯讀呈現、重新開啟與錯誤提示上的單元測試。
 */

import { flushPromises, mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import type { DependencyGateway } from "../../../desktop/src/application";
import InstallLogViewer from "../../../desktop/src/components/InstallLogViewer.vue";
import { DEPENDENCY_GATEWAY_KEY } from "../../../desktop/src/presentation/di";
import { deferred } from "../fixtures/async";
import { createFakeDependencyGateway } from "../fixtures/fake-gateways";

/**
 * 掛載注入模擬 `DependencyGateway` 的 `InstallLogViewer` 測試元件。
 *
 * @param gateway 模擬的投影工具依賴 Gateway。
 * @returns `@vue/test-utils` 的元件掛載包裝器。
 */
function mountViewer(gateway: DependencyGateway) {
  return mount(InstallLogViewer, {
    props: { logPath: "/tmp/neo-airplay-install.log" },
    global: {
      provide: {
        [DEPENDENCY_GATEWAY_KEY as symbol]: gateway,
      },
    },
  });
}

/** 驗證 `InstallLogViewer` 對話框互動與記錄載入流程。 */
describe("安裝記錄檢視器", () => {
  /** 驗證載入中狀態、唯讀文字區域內容呈現以及關閉後重開會再次向後端讀取。 */
  it("開啟並顯示可選取的安裝記錄，關閉後可以重新開啟", async () => {
    const firstRequest = deferred<string>();
    const gateway = createFakeDependencyGateway({
      readInstallLog: vi
        .fn()
        .mockReturnValueOnce(firstRequest.promise)
        .mockResolvedValueOnce("install started\ninstall finished\n"),
    });
    const wrapper = mountViewer(gateway);

    expect(wrapper.text()).toContain("/tmp/neo-airplay-install.log");
    await wrapper.get('[data-testid="install-log-open"]').trigger("click");
    expect(wrapper.get('[role="dialog"]').text()).toContain("正在載入安裝記錄");
    firstRequest.resolve("install started\ninstall finished\n");
    await flushPromises();

    const content = wrapper.get<HTMLTextAreaElement>('[data-testid="install-log-content"]');
    expect(content.element.value).toBe("install started\ninstall finished\n");
    expect(content.element.readOnly).toBe(true);
    expect(gateway.readInstallLog).toHaveBeenCalledOnce();

    await wrapper.get('[data-testid="install-log-close"]').trigger("click");
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    await wrapper.get('[data-testid="install-log-open"]').trigger("click");
    await flushPromises();
    expect(gateway.readInstallLog).toHaveBeenCalledTimes(2);
    wrapper.unmount();
  });

  /** 驗證讀取記錄失敗時於對話框內呈現 `role="alert"` 錯誤訊息並保留關閉按鈕。 */
  it("讀取失敗時在視窗內顯示錯誤", async () => {
    const gateway = createFakeDependencyGateway({
      readInstallLog: vi.fn().mockRejectedValue(new Error("尚無安裝記錄。")),
    });
    const wrapper = mountViewer(gateway);

    await wrapper.get('[data-testid="install-log-open"]').trigger("click");
    await flushPromises();

    expect(wrapper.get('[role="alert"]').text()).toBe("尚無安裝記錄。");
    expect(wrapper.find('[data-testid="install-log-close"]').exists()).toBe(true);
    wrapper.unmount();
  });
});
