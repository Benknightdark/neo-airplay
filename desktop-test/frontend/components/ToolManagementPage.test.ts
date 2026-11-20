/**
 * @file ToolManagementPage.test.ts
 * @description 驗證 `ToolManagementPage` 元件在工具狀態顯示、升級操作、重裝二次確認與忙線停用上的單元測試。
 */

import { flushPromises, mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import type { DependencyGateway } from "../../../desktop/src/application";
import ToolManagementPage from "../../../desktop/src/components/ToolManagementPage.vue";
import { DEPENDENCY_GATEWAY_KEY } from "../../../desktop/src/presentation/di";
import {
  createDependencySnapshot,
  createManagedToolInfo,
  createToolCatalogSnapshot,
  createToolReleaseOption,
} from "../fixtures/builders";
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

  /** 驗證清除工具須先經二次確認；取消不呼叫後端，確認後送出 `"clear"`、更新目錄顯示為未偵測並提供重裝入口。 */
  it("清除工具必須確認，確認後清除狀態、重載目錄並可重新檢查安裝", async () => {
    const clearedSnapshot = createDependencySnapshot({
      status: "failed",
      operation: "clear",
      message: "已清除所有 Neo AirPlay 專用工具與快取。",
      missing: ["uxplay 1.73.7", "scrcpy"],
    });
    const clearedCatalog = createToolCatalogSnapshot({
      tools: [
        createManagedToolInfo({ id: "uxplay", name: "UxPlay", installedVersion: null, selectedVersion: null }),
        createManagedToolInfo({ id: "scrcpy", installedVersion: null, selectedVersion: null }),
      ],
    });
    const initialCatalog = createToolCatalogSnapshot({
      tools: [
        createManagedToolInfo({ id: "uxplay", name: "UxPlay", installedVersion: "1.73.7", selectedVersion: "1.73.6" }),
        createManagedToolInfo({ id: "scrcpy", installedVersion: "5.0.1", selectedVersion: "4.1" }),
      ],
    });

    const getToolCatalog = vi
      .fn()
      .mockResolvedValueOnce(initialCatalog)
      .mockResolvedValueOnce(clearedCatalog);
    const gateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue(ready),
      getToolCatalog,
      manageDependencies: vi.fn().mockResolvedValue(clearedSnapshot),
      installDependencies: vi.fn().mockResolvedValue(ready),
    });
    const wrapper = mountToolsPage(gateway);

    await flushPromises();
    expect(wrapper.find('[data-testid="tools-reset-versions"]').exists()).toBe(true);

    await wrapper.get('[data-testid="tools-clear"]').trigger("click");
    expect(wrapper.find('[data-testid="tools-clear-confirmation"]').exists()).toBe(true);
    expect(gateway.manageDependencies).not.toHaveBeenCalled();

    await wrapper.get('[data-testid="tools-clear-cancel"]').trigger("click");
    expect(wrapper.find('[data-testid="tools-clear-confirmation"]').exists()).toBe(false);
    expect(gateway.manageDependencies).not.toHaveBeenCalled();

    await wrapper.get('[data-testid="tools-clear"]').trigger("click");
    await wrapper.get('[data-testid="tools-clear-confirm"]').trigger("click");
    await flushPromises();

    expect(gateway.manageDependencies).toHaveBeenCalledWith("clear");
    expect(getToolCatalog).toHaveBeenCalledTimes(2);
    expect(wrapper.get('[data-testid="tools-status"]').text()).toContain("需要處理");
    expect(wrapper.get('[data-testid="tools-message"]').text()).toBe("已清除所有 Neo AirPlay 專用工具與快取。");
    expect(wrapper.get('[data-testid="tools-missing"]').text()).toContain("uxplay 1.73.7");
    expect(wrapper.get('[data-testid="tool-card-uxplay"]').text()).toContain("尚未偵測");
    expect(wrapper.find('[data-testid="tools-reset-versions"]').exists()).toBe(false);

    await wrapper.get('[data-testid="tools-retry-install"]').trigger("click");
    await flushPromises();
    expect(gateway.installDependencies).toHaveBeenCalledOnce();
    wrapper.unmount();
  });

  /** 驗證後端正在進行維護作業時，升級、重裝與清除按鈕皆應維持停用且不觸發操作。 */
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
    expect(wrapper.get('[data-testid="tools-clear"]').attributes("disabled")).toBeDefined();
    await wrapper.get('[data-testid="tools-upgrade"]').trigger("click");
    await wrapper.get('[data-testid="tools-clear"]').trigger("click");
    expect(wrapper.find('[data-testid="tools-clear-confirmation"]').exists()).toBe(false);
    expect(gateway.manageDependencies).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  /** 驗證作業系統差異化渲染：僅在 Arch pacman 時顯示 pacman 警告，macOS 與 Windows 不顯示。 */
  it("依目前作業系統區別顯示來源與平台專屬警告", async () => {
    const macGateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue(ready),
      getToolCatalog: vi.fn().mockResolvedValue(createToolCatalogSnapshot({ os: "macos", platformWarning: null })),
    });
    const macWrapper = mountToolsPage(macGateway);
    await flushPromises();
    expect(macWrapper.get('[data-testid="tools-platform-os"]').text()).toContain("macOS");
    expect(macWrapper.find('[data-testid="tools-platform-warning"]').exists()).toBe(false);
    macWrapper.unmount();

    const winGateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue(ready),
      getToolCatalog: vi.fn().mockResolvedValue(
        createToolCatalogSnapshot({
          os: "windows",
          arch: "x86_64",
          platformLabel: "Windows (x86_64)",
          packageManager: "WinGet + MSYS2 UCRT64",
          platformNotice: "Windows 僅於補裝系統前置項目時要求 UAC 授權。",
          platformWarning: null,
        }),
      ),
    });
    const winWrapper = mountToolsPage(winGateway);
    await flushPromises();
    expect(winWrapper.get('[data-testid="tools-platform-os"]').text()).toContain("Windows");
    expect(winWrapper.text()).toContain("WinGet + MSYS2 UCRT64");
    expect(winWrapper.find('[data-testid="tools-platform-warning"]').exists()).toBe(false);
    winWrapper.unmount();

    const pacmanGateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue(ready),
      getToolCatalog: vi.fn().mockResolvedValue(
        createToolCatalogSnapshot({
          os: "linux",
          arch: "aarch64",
          platformLabel: "Linux (aarch64)",
          packageManager: "Pacman（Arch Linux）",
          platformWarning: "偵測到 Arch Linux pacman：升級可能會執行 pacman -Syu。",
          tools: [
            createManagedToolInfo({ id: "uxplay", name: "UxPlay", versionControl: "selectable" }),
            createManagedToolInfo({
              id: "scrcpy",
              versionControl: "packageManager",
              availableVersions: [],
              supportsManualSha256: false,
            }),
          ],
        }),
      ),
    });
    const pacmanWrapper = mountToolsPage(pacmanGateway);
    await flushPromises();
    expect(pacmanWrapper.get('[data-testid="tools-platform-warning"]').text()).toContain("pacman -Syu");
    expect(pacmanWrapper.find('[data-testid="tool-install-scrcpy"]').exists()).toBe(false);
    pacmanWrapper.unmount();
  });

  /** 驗證透過按鈕爬取線上版本後可選擇特定 scrcpy 版本並送出 installToolVersion。 */
  it("支援重新爬取線上版本並安裝選取的 scrcpy 版本", async () => {
    const crawledCatalog = createToolCatalogSnapshot({
      tools: [
        createManagedToolInfo({ id: "uxplay", name: "UxPlay" }),
        createManagedToolInfo({
          id: "scrcpy",
          availableVersions: [
            createToolReleaseOption({ version: "5.0.1", label: "v5.0.1（內建推薦驗證版）", recommended: true }),
            createToolReleaseOption({ version: "4.1", reference: "v4.1", label: "v4.1", recommended: false, sha256: null }),
          ],
        }),
      ],
    });
    const gateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue(ready),
      getToolCatalog: vi.fn().mockResolvedValue(createToolCatalogSnapshot({ remoteFetchedAt: null })),
      fetchToolVersions: vi.fn().mockResolvedValue(crawledCatalog),
    });

    const wrapper = mountToolsPage(gateway);
    await flushPromises();
    expect(gateway.fetchToolVersions).toHaveBeenCalledOnce();

    await wrapper.get('[data-testid="tools-fetch-versions"]').trigger("click");
    await flushPromises();
    expect(gateway.fetchToolVersions).toHaveBeenCalledTimes(2);

    await wrapper.get('[data-testid="tool-version-select-scrcpy"]').setValue("4.1");
    await wrapper.get('[data-testid="tool-install-scrcpy"]').trigger("click");
    await flushPromises();

    expect(gateway.installToolVersion).toHaveBeenCalledWith({
      tool: "scrcpy",
      version: "4.1",
      sha256: null,
      allowUnverified: false,
    });
    wrapper.unmount();
  });

  /** 驗證手動輸入非法版本時阻擋提交，以及安裝未附 SHA-256 的非預設 UxPlay 時顯示確認對話框（Option 1A）。 */
  it("手動輸入會即時驗證格式並在未附 SHA-256 的 UxPlay 時跳出確認視窗", async () => {
    const gateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue(ready),
    });
    const wrapper = mountToolsPage(gateway);
    await flushPromises();

    await wrapper.get('[data-testid="tool-mode-manual-uxplay"]').trigger("click");
    const input = wrapper.get('[data-testid="tool-version-input-uxplay"]');
    await input.setValue("bad version;id");
    expect(wrapper.find('[data-testid="tool-version-error-uxplay"]').exists()).toBe(true);
    expect(wrapper.get('[data-testid="tool-install-uxplay"]').attributes("disabled")).toBeDefined();

    await input.setValue("1.73.6");
    expect(wrapper.find('[data-testid="tool-version-error-uxplay"]').exists()).toBe(false);
    await wrapper.get('[data-testid="tool-install-uxplay"]').trigger("click");

    expect(wrapper.find('[data-testid="tools-unverified-confirmation"]').exists()).toBe(true);
    expect(gateway.installToolVersion).not.toHaveBeenCalled();

    await wrapper.get('[data-testid="tools-unverified-confirm"]').trigger("click");
    await flushPromises();

    expect(gateway.installToolVersion).toHaveBeenCalledWith({
      tool: "uxplay",
      version: "1.73.6",
      sha256: null,
      allowUnverified: true,
    });
    wrapper.unmount();
  });
});
