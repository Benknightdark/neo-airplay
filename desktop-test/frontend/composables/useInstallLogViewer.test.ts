/**
 * @file useInstallLogViewer.test.ts
 * @description 驗證 `useInstallLogViewer` 在非同步讀取與關閉競態處理上的單元測試。
 */

import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { describe, expect, it, vi } from "vitest";
import type { DependencyGateway } from "../../../desktop/src/application";
import { useInstallLogViewer } from "../../../desktop/src/composables/useInstallLogViewer";
import { deferred } from "../fixtures/async";
import { createFakeDependencyGateway } from "../fixtures/fake-gateways";

/**
 * 在暫時掛載的 Vue 測試元件中建立並回傳 `useInstallLogViewer` 實例。
 *
 * @param gateway 模擬的投影工具依賴 Gateway。
 * @returns 包含 `viewer` 狀態物件與 `wrapper` 測試元件包裝器的物件。
 */
function mountViewerState(gateway: DependencyGateway) {
  const viewerRef: { current?: ReturnType<typeof useInstallLogViewer> } = {};
  const wrapper = mount(
    defineComponent({
      setup() {
        viewerRef.current = useInstallLogViewer(gateway);
        return () => h("div");
      },
    }),
  );
  const viewer = viewerRef.current;
  if (!viewer) throw new Error("找不到安裝記錄狀態。");
  return { viewer, wrapper };
}

/** 驗證 `useInstallLogViewer` 的開啟、關閉與競態忽略行為。 */
describe("useInstallLogViewer", () => {
  /** 驗證檢視器已關閉後，先前尚未完成的讀取回應不會回填內容。 */
  it("關閉檢視器後忽略逾期完成的讀取結果", async () => {
    const request = deferred<string>();
    const gateway = createFakeDependencyGateway({
      readInstallLog: vi.fn().mockReturnValue(request.promise),
    });
    const { viewer, wrapper } = mountViewerState(gateway);

    const openPromise = viewer.openViewer();
    expect(viewer.isOpen.value).toBe(true);
    expect(viewer.loading.value).toBe(true);

    viewer.closeViewer();
    expect(viewer.isOpen.value).toBe(false);
    expect(viewer.loading.value).toBe(false);

    request.resolve("stale log");
    await openPromise;
    await flushPromises();

    expect(viewer.content.value).toBe("");
    wrapper.unmount();
  });
});
