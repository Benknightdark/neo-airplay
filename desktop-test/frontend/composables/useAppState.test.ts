/**
 * @file useAppState.test.ts
 * @description 驗證 `useAppState` 在錯誤狀態保留、請求世代競態防護、重複操作鎖定與定時輪詢清理上的單元測試。
 */

import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ReceiverGateway } from "../../../desktop/src/application";
import { useAppState } from "../../../desktop/src/composables/useAppState";
import type { DashboardSnapshot, EngineSnapshot } from "../../../desktop/src/domain";
import { deferred } from "../fixtures/async";
import { createDashboardSnapshot, createEngineSnapshot } from "../fixtures/builders";
import { createFakeReceiverGateway } from "../fixtures/fake-gateways";

/** 預設測試用儀表板快照。 */
const dashboard = createDashboardSnapshot();

/**
 * 在暫時掛載的 Vue 測試元件中建立並回傳 `useAppState` 實例。
 *
 * @param gateway 模擬的接收器 Gateway。
 * @returns 包含 `state` Composable 狀態與 `wrapper` 測試元件包裝器的物件。
 */
function mountState(gateway: ReceiverGateway) {
  const stateRef: { current?: ReturnType<typeof useAppState> } = {};
  const wrapper = mount(
    defineComponent({
      setup() {
        stateRef.current = useAppState(gateway);
        return () => h("div");
      },
    }),
  );
  const state = stateRef.current;
  if (!state) throw new Error("找不到應用程式狀態。");
  return { state, wrapper };
}

/** 驗證 `useAppState` 的競態保護、錯誤訊息管理與輪詢生命週期。 */
describe("應用程式狀態", () => {
  afterEach(() => vi.useRealTimers());

  /** 驗證背景靜默刷新會保留錯誤提示，僅明確清除旗標或下一個動作才清除。 */
  it("動作失敗後保留錯誤，直到使用者重新整理或執行其他動作", async () => {
    const gateway = createFakeReceiverGateway({
      startAirplay: vi.fn().mockRejectedValue(new Error("engine failed")),
    });
    const { state, wrapper } = mountState(gateway);

    await state.startAirplay();
    expect(state.error.value).toBe("engine failed");

    await state.refresh();
    expect(state.error.value).toBe("engine failed");

    await state.refresh(true);
    expect(state.error.value).toBeNull();
    wrapper.unmount();
  });

  /** 驗證較晚完成但屬於舊世代的刷新回應不會覆蓋較新的儀表板資料。 */
  it("較新的狀態刷新完成後不會被較早的回應覆蓋", async () => {
    const oldRequest = deferred<DashboardSnapshot>();
    const newRequest = deferred<DashboardSnapshot>();
    const gateway = createFakeReceiverGateway({
      getDashboard: vi
        .fn()
        .mockReturnValueOnce(oldRequest.promise)
        .mockReturnValueOnce(newRequest.promise),
    });
    const { state, wrapper } = mountState(gateway);

    const oldRefresh = state.refresh();
    const newRefresh = state.refresh(true);
    newRequest.resolve({
      ...dashboard,
      settings: { ...dashboard.settings, receiverName: "Fresh" },
    });
    await newRefresh;
    oldRequest.resolve({
      ...dashboard,
      settings: { ...dashboard.settings, receiverName: "Stale" },
    });
    await oldRefresh;

    expect(state.dashboard.value?.settings.receiverName).toBe("Fresh");
    wrapper.unmount();
  });

  /** 驗證在執行控制操作期間並行發起的舊手動刷新不會覆寫操作後最新快照。 */
  it("操作完成後的狀態不會被較早開始的手動刷新覆蓋", async () => {
    const manualRefreshRequest = deferred<DashboardSnapshot>();
    const postActionRequest = deferred<DashboardSnapshot>();
    const startRequest = deferred<EngineSnapshot>();
    const gateway = createFakeReceiverGateway({
      getDashboard: vi
        .fn()
        .mockReturnValueOnce(manualRefreshRequest.promise)
        .mockReturnValueOnce(postActionRequest.promise),
      startAirplay: vi.fn().mockReturnValue(startRequest.promise),
    });
    const { state, wrapper } = mountState(gateway);

    const manualRefresh = state.refresh();
    const startAirplay = state.startAirplay();
    startRequest.resolve(dashboard.airplay);
    await flushPromises();
    expect(gateway.getDashboard).toHaveBeenCalledTimes(2);

    postActionRequest.resolve({
      ...dashboard,
      airplay: createEngineSnapshot({ status: "running", message: "執行中", processId: 42 }),
    });
    await startAirplay;
    manualRefreshRequest.resolve(dashboard);
    await manualRefresh;

    expect(state.dashboard.value?.airplay.status).toBe("running");
    wrapper.unmount();
  });

  /** 驗證 `busy` 鎖定期間會忽略重複呼叫相同或重疊控制動作。 */
  it("動作進行中忽略重複或重疊的操作", async () => {
    const startRequest = deferred<EngineSnapshot>();
    const gateway = createFakeReceiverGateway({
      startAirplay: vi.fn().mockReturnValue(startRequest.promise),
    });
    const { state, wrapper } = mountState(gateway);

    const firstStart = state.startAirplay();
    const duplicateStart = state.startAirplay();
    expect(gateway.startAirplay).toHaveBeenCalledOnce();
    expect(state.busy.value).toBe(true);

    startRequest.resolve(dashboard.airplay);
    await Promise.all([firstStart, duplicateStart]);
    expect(gateway.getDashboard).toHaveBeenCalledOnce();
    expect(state.busy.value).toBe(false);
    wrapper.unmount();
  });

  /** 驗證宿主元件卸載後會停止背景狀態輪詢計時器。 */
  it("卸載後停止狀態輪詢", async () => {
    vi.useFakeTimers();
    const gateway = createFakeReceiverGateway();
    const { wrapper } = mountState(gateway);

    await vi.advanceTimersByTimeAsync(1500);
    await flushPromises();
    expect(gateway.getDashboard).toHaveBeenCalledOnce();

    wrapper.unmount();
    await vi.advanceTimersByTimeAsync(3000);
    expect(gateway.getDashboard).toHaveBeenCalledOnce();
  });
});
