/**
 * @file useDependencyState.test.ts
 * @description 驗證 `useDependencyState` 在定時輪詢、世代競態保護、重試同步與停用停止輪詢上的單元測試。
 */

import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DependencyGateway } from "../../../desktop/src/application";
import { useDependencyState } from "../../../desktop/src/composables/useDependencyState";
import type { DependencySnapshot } from "../../../desktop/src/domain";
import { deferred } from "../fixtures/async";
import { createDependencySnapshot } from "../fixtures/builders";
import { createFakeDependencyGateway } from "../fixtures/fake-gateways";

/**
 * 在暫時掛載的 Vue 測試元件中建立並回傳 `useDependencyState` 實例。
 *
 * @param gateway 模擬的投影工具依賴 Gateway。
 * @param onBusyChange 忙線狀態變更回呼函式。
 * @returns 包含 `state` Composable 狀態與 `wrapper` 測試元件包裝器的物件。
 */
function mountDependencyState(gateway: DependencyGateway, onBusyChange?: (busy: boolean) => void) {
  const stateRef: { current?: ReturnType<typeof useDependencyState> } = {};
  const wrapper = mount(
    defineComponent({
      setup() {
        stateRef.current = useDependencyState({ gateway, onBusyChange });
        return () => h("div");
      },
    }),
  );
  const state = stateRef.current;
  if (!state) throw new Error("找不到依賴狀態。");
  return { state, wrapper };
}

/** 驗證 `useDependencyState` 狀態管理與輪詢控制。 */
describe("useDependencyState", () => {
  afterEach(() => vi.useRealTimers());

  /** 驗證逾期輪詢結果不會覆蓋重試後的最新快照，並正確觸發忙線回呼順序。 */
  it("支援輪詢、競態世代保護與重試後狀態同步", async () => {
    vi.useFakeTimers();
    const failed = createDependencySnapshot({ status: "failed", message: "安裝失敗" });
    const ready = createDependencySnapshot({ status: "ready", message: "投影工具已就緒。" });
    const pendingPoll = deferred<DependencySnapshot>();
    const gateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValueOnce(failed).mockReturnValueOnce(pendingPoll.promise),
      installDependencies: vi.fn().mockResolvedValue(ready),
    });
    const busyEvents: boolean[] = [];

    const { state, wrapper } = mountDependencyState(gateway, (busy) => busyEvents.push(busy));
    await flushPromises();
    expect(state.snapshot.value?.status).toBe("failed");

    await vi.advanceTimersByTimeAsync(1500);
    await state.retry();
    await flushPromises();
    expect(state.snapshot.value?.status).toBe("ready");

    pendingPoll.resolve(failed);
    await flushPromises();
    expect(state.snapshot.value?.status).toBe("ready");
    expect(busyEvents).toEqual([false, true, false]);
    wrapper.unmount();
  });

  /** 驗證後端回報功能已停用時不再排程後續定時輪詢。 */
  it("工具停用時會自動停止輪詢", async () => {
    vi.useFakeTimers();
    const gateway = createFakeDependencyGateway({
      getDependencies: vi.fn().mockResolvedValue(
        createDependencySnapshot({ status: "disabled", message: "已停用" }),
      ),
    });

    const { state, wrapper } = mountDependencyState(gateway);
    await flushPromises();
    expect(state.unavailable.value).toBe(true);

    await vi.advanceTimersByTimeAsync(3000);
    expect(gateway.getDependencies).toHaveBeenCalledOnce();
    wrapper.unmount();
  });
});
