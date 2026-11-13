/**
 * @file DependencyPanel.test.ts
 * @description 驗證 `DependencyPanel` 元件在自動載入、失敗重試、防重複送出、輪詢競態保護與卸載清理上的單元測試。
 */

import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DependencyGateway } from "../../../desktop/src/application";
import DependencyPanel from "../../../desktop/src/components/DependencyPanel.vue";
import type { DependencySnapshot } from "../../../desktop/src/domain";
import { DEPENDENCY_GATEWAY_KEY } from "../../../desktop/src/presentation/di";
import { deferred } from "../fixtures/async";
import { createDependencySnapshot } from "../fixtures/builders";
import { createFakeDependencyGateway } from "../fixtures/fake-gateways";

/** 已就緒但無安裝記錄檔路徑的測試快照。 */
const ready = createDependencySnapshot({ logPath: "" });
/** 缺少 `scrcpy` 的失敗測試快照。 */
const failed = createDependencySnapshot({
  status: "failed",
  message: "部分投影工具尚未安裝。",
  missing: ["scrcpy"],
});

/**
 * 掛載注入模擬 `DependencyGateway` 的 `DependencyPanel` 測試元件。
 *
 * @param gateway 模擬的投影工具依賴 Gateway。
 * @returns `@vue/test-utils` 的元件掛載包裝器。
 */
function mountPanel(gateway: DependencyGateway) {
  return mount(DependencyPanel, {
    global: {
      provide: {
        [DEPENDENCY_GATEWAY_KEY as symbol]: gateway,
      },
    },
  });
}

/** 驗證 `DependencyPanel` 依賴準備面板行為。 */
describe("投影工具準備", () => {
  afterEach(() => vi.useRealTimers());

  /** 驗證元件掛載後會主動查詢依賴狀態並於完成後解除忙線事件。 */
  it("掛載後自動載入依賴狀態", async () => {
    const gateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue(ready),
    });
    const wrapper = mountPanel(gateway);

    await flushPromises();

    expect(gateway.getDependencies).toHaveBeenCalledOnce();
    expect(wrapper.text()).toContain("投影工具已就緒。");
    expect(wrapper.emitted("busy")).toEqual([[false]]);
    wrapper.unmount();
  });

  /** 驗證初始查詢例外會解除準備鎖定，並於點擊重試時進入安裝流程。 */
  it("載入失敗時解除準備鎖定並允許重試", async () => {
    const installRequest = deferred<DependencySnapshot>();
    const gateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockRejectedValue(new Error("Tauri IPC unavailable")),
      installDependencies: vi.fn().mockReturnValue(installRequest.promise),
    });
    const wrapper = mountPanel(gateway);

    await flushPromises();

    expect(wrapper.get('[data-testid="dependency-panel"]').text()).toContain("Tauri IPC unavailable");
    expect(wrapper.emitted("busy")).toEqual([[false]]);
    const retryClick = wrapper.get('[data-testid="dependency-retry"]').trigger("click");
    await flushPromises();

    expect(gateway.installDependencies).toHaveBeenCalledOnce();
    expect(wrapper.get('[data-testid="dependency-panel"]').text()).toContain("正在檢查並安裝投影工具");
    expect(wrapper.find('[data-testid="dependency-retry"]').exists()).toBe(false);
    expect(wrapper.emitted("busy")).toEqual([[false], [true]]);

    installRequest.resolve(ready);
    await retryClick;
    await flushPromises();
    expect(wrapper.text()).toContain("投影工具已就緒。");
    expect(wrapper.emitted("busy")).toEqual([[false], [true], [false]]);
  });

  /** 驗證失敗畫面會顯示安裝記錄檢視按鈕，且重試成功後同步更新 UI 與忙線狀態。 */
  it("失敗狀態提供記錄位置，重試成功後更新狀態", async () => {
    const gateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue(failed),
      installDependencies: vi.fn().mockResolvedValue(ready),
    });
    const wrapper = mountPanel(gateway);

    await flushPromises();
    expect(wrapper.text()).toContain("/tmp/neo-airplay-install.log");
    expect(wrapper.get('[data-testid="install-log-open"]').text()).toBe("查看安裝記錄");
    await wrapper.get('[data-testid="dependency-retry"]').trigger("click");
    await flushPromises();

    expect(gateway.installDependencies).toHaveBeenCalledOnce();
    expect(wrapper.text()).toContain("投影工具已就緒。");
    expect(wrapper.emitted("busy")).toEqual([[false], [true], [false]]);
    wrapper.unmount();
  });

  /** 驗證連點重試按鈕時只會觸發一次安裝請求並停用按鈕。 */
  it("快速重複點擊只送出一次安裝要求", async () => {
    const installRequest = deferred<DependencySnapshot>();
    const gateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue(failed),
      installDependencies: vi.fn().mockReturnValue(installRequest.promise),
    });
    const wrapper = mountPanel(gateway);

    await flushPromises();
    const retryButton = wrapper.get('[data-testid="dependency-retry"]');
    const firstClick = retryButton.trigger("click");
    const secondClick = retryButton.trigger("click");
    await Promise.all([firstClick, secondClick]);

    expect(gateway.installDependencies).toHaveBeenCalledOnce();
    expect(wrapper.get('[data-testid="dependency-retry"]').attributes("disabled")).toBeDefined();
    installRequest.resolve(ready);
    await flushPromises();
    wrapper.unmount();
  });

  /** 驗證過期輪詢回應抵達時不會蓋過重試安裝後取得的最新狀態。 */
  it("較早開始的輪詢結果不會覆蓋重試回傳的新狀態", async () => {
    vi.useFakeTimers();
    const pendingPoll = deferred<DependencySnapshot>();
    const gateway = createFakeDependencyGateway({
      getDependencies: vi
        .fn()
        .mockResolvedValueOnce(failed)
        .mockReturnValueOnce(pendingPoll.promise),
      installDependencies: vi.fn().mockResolvedValue(ready),
    });
    const wrapper = mountPanel(gateway);

    await flushPromises();
    await vi.advanceTimersByTimeAsync(1500);
    expect(gateway.getDependencies).toHaveBeenCalledTimes(2);

    await wrapper.get('[data-testid="dependency-retry"]').trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("投影工具已就緒。");

    pendingPoll.resolve(failed);
    await flushPromises();
    expect(wrapper.text()).toContain("投影工具已就緒。");
    expect(wrapper.text()).not.toContain("部分投影工具尚未安裝。");
    wrapper.unmount();
  });

  /** 驗證自動安裝停用時會隱藏面板、釋放鎖定並停止定時輪詢。 */
  it("停用時隱藏面板、解除準備鎖定並停止輪詢", async () => {
    vi.useFakeTimers();
    const gateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue(
        createDependencySnapshot({
          status: "disabled",
          message: "自動安裝已停用。",
          logPath: "",
        }),
      ),
    });
    const wrapper = mountPanel(gateway);

    await flushPromises();
    expect(wrapper.find('[data-testid="dependency-panel"]').exists()).toBe(false);
    expect(wrapper.emitted("busy")).toEqual([[false]]);

    await vi.advanceTimersByTimeAsync(3000);
    expect(gateway.getDependencies).toHaveBeenCalledOnce();
    wrapper.unmount();
  });

  /** 驗證元件卸載後會中止背景輪詢計時器，避免繼續發送查詢。 */
  it("卸載後會停止定期狀態輪詢", async () => {
    vi.useFakeTimers();
    const gateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue(ready),
    });
    const wrapper = mountPanel(gateway);

    await flushPromises();
    await vi.advanceTimersByTimeAsync(1500);
    expect(gateway.getDependencies).toHaveBeenCalledTimes(2);

    wrapper.unmount();
    await vi.advanceTimersByTimeAsync(3000);
    expect(gateway.getDependencies).toHaveBeenCalledTimes(2);
  });
});
