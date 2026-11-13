/**
 * @file 工具安裝記錄檢視器 Composable，管理彈窗開關、非同步日誌載入與關閉時放棄過期回應。
 */
import { ref } from "vue";
import type { DependencyGateway } from "../application";
import { resolveDependencyUseCases } from "../presentation/di";

/**
 * 提供安裝日誌檢視彈窗的狀態管理與開啟／關閉操作。
 *
 * @param gateway 選擇性傳入的自訂 `DependencyGateway`（供測試注入使用）。
 * @returns 包含 `isOpen`、`loading`、`content`、`error` 與 `openViewer`/`closeViewer` 的物件。
 */
export function useInstallLogViewer(gateway?: DependencyGateway) {
  /** 解析後的依賴工具應用層使用案例集合。 */
  const useCases = resolveDependencyUseCases(gateway);
  /** 日誌檢視對話框是否處於開啟狀態。 */
  const isOpen = ref(false);
  /** 是否正在從後端讀取安裝日誌檔案內容。 */
  const loading = ref(false);
  /** 已載入的安裝日誌純文字內容。 */
  const content = ref("");
  /** 讀取安裝日誌失敗時顯示的錯誤訊息。 */
  const error = ref("");
  /** 遞增請求世代編號，確保關閉後不讓延遲回應覆寫狀態。 */
  let requestGeneration = 0;

  /**
   * 開啟日誌檢視對話框並向後端非同步載入最新 `install.log` 內容；若正在載入則忽略重複觸發。
   *
   * @returns 讀取並更新狀態完成後解析的 `Promise<void>`。
   */
  async function openViewer(): Promise<void> {
    if (loading.value) return;

    const generation = ++requestGeneration;
    isOpen.value = true;
    loading.value = true;
    content.value = "";
    error.value = "";

    try {
      const result = await useCases.readInstallLog();
      if (generation !== requestGeneration) return;
      if (result.isSuccess) {
        content.value = result.value ?? "";
      } else {
        error.value = result.error ?? "無法讀取安裝記錄。";
      }
    } finally {
      if (generation === requestGeneration) loading.value = false;
    }
  }

  /**
   * 關閉日誌檢視對話框並遞增請求世代，立即作廢進行中的讀取作業。
   */
  function closeViewer(): void {
    requestGeneration += 1;
    isOpen.value = false;
    loading.value = false;
  }

  return {
    isOpen,
    loading,
    content,
    error,
    openViewer,
    closeViewer,
  };
}
