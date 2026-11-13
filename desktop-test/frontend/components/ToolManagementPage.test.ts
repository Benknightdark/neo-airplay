/**
 * @file ToolManagementPage.test.ts
 * @description 驗證 `ToolManagementPage` 元件在工具狀態顯示、升級操作、重裝二次確認與忙線停用上的單元測試。
 */

import { flushPromises, mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import type { DependencyGateway } from "../../../desktop/src/application";
import ToolManagementPage from "../../../desktop/src/components/ToolManagementPage.vue";
import { DEPENDENCY_GATEWAY_KEY } from "../../../desktop/src/presentation/di";
import { createDependencySnapshot } from "../fixtures/builders";
import { createFakeDependencyGateway } from "../fixtures/fake-gateways";

/** 預設已就緒的投影工具依賴快照。 */
const ready = createDependencySnapshot();

/**
 * 掛載注入模擬 `DependencyGateway` 的 `ToolManagementPage` 測試元件。
 *
 * @param gateway 模擬的投影工具依賴 Gateway。
 * @returns `@vue/test-utils` 的元件掛載包裝器。
 */
function mountToolsPage(gateway: DependencyGateway) {
  return mount(ToolManagementPage, {
    global: {
      provide: {
        [DEPENDENCY_GATEWAY_KEY as symbol]: gateway,
      },
    },
  });
}

/** 驗證 `ToolManagementPage` 工具管理操作流程。 */
describe("工具管理頁", () => {
  /** 驗證初次載入顯示狀態與記錄路徑，且點擊升級會帶入 `"upgrade"` 呼叫後端。 */
  it("顯示工具狀態並執行升級", async () => {
    const gateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue(ready),
      manageDependencies: vi.fn().mockResolvedValue({
        ...ready,
        operation: "upgrade",
        message: "投影工具升級完成。",
      }),
    });
    const wrapper = mountToolsPage(gateway);

    await flushPromises();
    expect(wrapper.get('[data-testid="tools-status"]').text()).toContain("已就緒");
    expect(wrapper.text()).toContain("/tmp/neo-airplay-install.log");
    expect(wrapper.get('[data-testid="install-log-open"]').text()).toBe("查看安裝記錄");

    await wrapper.get('[data-testid="tools-upgrade"]').trigger("click");
    await flushPromises();

    expect(gateway.manageDependencies).toHaveBeenCalledWith("upgrade");
    expect(wrapper.get('[data-testid="tools-message"]').text()).toBe("投影工具升級完成。");
    wrapper.unmount();
  });

  /** 驗證完整重裝須先經確認視窗核准；取消不呼叫後端，確認後送出 `"reinstall"`。 */
  it("重裝必須確認，取消時不呼叫後端", async () => {
    const gateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue(ready),
      manageDependencies: vi.fn().mockResolvedValue({
        ...ready,
        operation: "reinstall",
        message: "投影工具重新安裝完成。",
      }),
    });
    const wrapper = mountToolsPage(gateway);

    await flushPromises();
    await wrapper.get('[data-testid="tools-reinstall"]').trigger("click");
    expect(wrapper.find('[role="dialog"]').exists()).toBe(true);
    expect(gateway.manageDependencies).not.toHaveBeenCalled();

    await wrapper.get('[data-testid="tools-reinstall-cancel"]').trigger("click");
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    expect(gateway.manageDependencies).not.toHaveBeenCalled();

    await wrapper.get('[data-testid="tools-reinstall"]').trigger("click");
    await wrapper.get('[data-testid="tools-reinstall-confirm"]').trigger("click");
    await flushPromises();

    expect(gateway.manageDependencies).toHaveBeenCalledWith("reinstall");
    expect(wrapper.get('[data-testid="tools-message"]').text()).toBe("投影工具重新安裝完成。");
    wrapper.unmount();
  });

  /** 驗證後端正在進行維護作業時，升級與重裝按鈕皆應維持停用且不觸發操作。 */
  it("維護作業進行中時停用操作", async () => {
    const gateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue({
        ...ready,
        status: "installing",
        operation: "upgrade",
        message: "正在更新平台相依套件與 Neo AirPlay 工具…",
      }),
    });
    const wrapper = mountToolsPage(gateway);

    await flushPromises();
    expect(wrapper.get('[data-testid="tools-upgrade"]').attributes("disabled")).toBeDefined();
    expect(wrapper.get('[data-testid="tools-reinstall"]').attributes("disabled")).toBeDefined();
    await wrapper.get('[data-testid="tools-upgrade"]').trigger("click");
    expect(gateway.manageDependencies).not.toHaveBeenCalled();
    wrapper.unmount();
  });
});
