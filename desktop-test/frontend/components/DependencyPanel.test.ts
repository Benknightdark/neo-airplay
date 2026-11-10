import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../../../desktop/src/api";
import type { DependencySnapshot } from "../../../desktop/src/types";
import DependencyPanel from "../../../desktop/src/components/DependencyPanel.vue";

vi.mock("../../../desktop/src/api", () => ({
  getDependencies: vi.fn(),
  installDependencies: vi.fn(),
}));

const ready: DependencySnapshot = {
  status: "ready",
  message: "投影工具已就緒。",
  missing: [],
  logPath: "",
};

const failed: DependencySnapshot = {
  status: "failed",
  message: "部分投影工具尚未安裝。",
  missing: ["scrcpy"],
  logPath: "/tmp/neo-airplay-install.log",
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

describe("投影工具準備", () => {
  beforeEach(() => vi.resetAllMocks());
  afterEach(() => vi.useRealTimers());

  it("掛載後自動載入依賴狀態", async () => {
    vi.mocked(api.getDependencies).mockResolvedValue(ready);
    const wrapper = mount(DependencyPanel);

    await flushPromises();

    expect(api.getDependencies).toHaveBeenCalledOnce();
    expect(wrapper.text()).toContain("投影工具已就緒。");
    expect(wrapper.emitted("busy")).toEqual([[false]]);
    wrapper.unmount();
  });

  it("載入失敗時解除準備鎖定並允許重試", async () => {
    const installRequest = deferred<DependencySnapshot>();
    vi.mocked(api.getDependencies).mockRejectedValue(new Error("Tauri IPC unavailable"));
    vi.mocked(api.installDependencies).mockReturnValue(installRequest.promise);
    const wrapper = mount(DependencyPanel);

    await flushPromises();

    expect(wrapper.get('[data-testid="dependency-panel"]').text()).toContain("Tauri IPC unavailable");
    expect(wrapper.emitted("busy")).toEqual([[false]]);
    const retryClick = wrapper.get('[data-testid="dependency-retry"]').trigger("click");
    await flushPromises();

    expect(api.installDependencies).toHaveBeenCalledOnce();
    expect(wrapper.get('[data-testid="dependency-panel"]').text()).toContain("正在檢查並安裝投影工具");
    expect(wrapper.find('[data-testid="dependency-retry"]').exists()).toBe(false);
    expect(wrapper.emitted("busy")).toEqual([[false], [true]]);

    installRequest.resolve(ready);
    await retryClick;
    await flushPromises();
    expect(wrapper.text()).toContain("投影工具已就緒。");
    expect(wrapper.emitted("busy")).toEqual([[false], [true], [false]]);
  });

  it("失敗狀態提供記錄位置，重試成功後更新狀態", async () => {
    vi.mocked(api.getDependencies).mockResolvedValue(failed);
    vi.mocked(api.installDependencies).mockResolvedValue(ready);
    const wrapper = mount(DependencyPanel);

    await flushPromises();
    expect(wrapper.text()).toContain("/tmp/neo-airplay-install.log");
    await wrapper.get('[data-testid="dependency-retry"]').trigger("click");
    await flushPromises();

    expect(api.installDependencies).toHaveBeenCalledOnce();
    expect(wrapper.text()).toContain("投影工具已就緒。");
    expect(wrapper.emitted("busy")).toEqual([[false], [true], [false]]);
    wrapper.unmount();
  });

  it("快速重複點擊只送出一次安裝要求", async () => {
    const installRequest = deferred<DependencySnapshot>();
    vi.mocked(api.getDependencies).mockResolvedValue(failed);
    vi.mocked(api.installDependencies).mockReturnValue(installRequest.promise);
    const wrapper = mount(DependencyPanel);

    await flushPromises();
    const retryButton = wrapper.get('[data-testid="dependency-retry"]');
    const firstClick = retryButton.trigger("click");
    const secondClick = retryButton.trigger("click");
    await Promise.all([firstClick, secondClick]);

    expect(api.installDependencies).toHaveBeenCalledOnce();
    expect(wrapper.get('[data-testid="dependency-retry"]').attributes("disabled")).toBeDefined();
    installRequest.resolve(ready);
    await flushPromises();
    wrapper.unmount();
  });

  it("較早開始的輪詢結果不會覆蓋重試回傳的新狀態", async () => {
    vi.useFakeTimers();
    const pendingPoll = deferred<DependencySnapshot>();
    vi.mocked(api.getDependencies)
      .mockResolvedValueOnce(failed)
      .mockReturnValueOnce(pendingPoll.promise);
    vi.mocked(api.installDependencies).mockResolvedValue(ready);
    const wrapper = mount(DependencyPanel);

    await flushPromises();
    await vi.advanceTimersByTimeAsync(1500);
    expect(api.getDependencies).toHaveBeenCalledTimes(2);

    await wrapper.get('[data-testid="dependency-retry"]').trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("投影工具已就緒。");

    pendingPoll.resolve(failed);
    await flushPromises();
    expect(wrapper.text()).toContain("投影工具已就緒。");
    expect(wrapper.text()).not.toContain("部分投影工具尚未安裝。");
    wrapper.unmount();
  });

  it("停用時隱藏面板、解除準備鎖定並停止輪詢", async () => {
    vi.useFakeTimers();
    vi.mocked(api.getDependencies).mockResolvedValue({
      status: "disabled",
      message: "自動安裝已停用。",
      missing: [],
      logPath: "",
    });
    const wrapper = mount(DependencyPanel);

    await flushPromises();
    expect(wrapper.find('[data-testid="dependency-panel"]').exists()).toBe(false);
    expect(wrapper.emitted("busy")).toEqual([[false]]);

    await vi.advanceTimersByTimeAsync(3000);
    expect(api.getDependencies).toHaveBeenCalledOnce();
    wrapper.unmount();
  });

  it("卸載後會停止定期狀態輪詢", async () => {
    vi.useFakeTimers();
    vi.mocked(api.getDependencies).mockResolvedValue(ready);
    const wrapper = mount(DependencyPanel);

    await flushPromises();
    await vi.advanceTimersByTimeAsync(1500);
    expect(api.getDependencies).toHaveBeenCalledTimes(2);

    wrapper.unmount();
    await vi.advanceTimersByTimeAsync(3000);
    expect(api.getDependencies).toHaveBeenCalledTimes(2);
  });
});
