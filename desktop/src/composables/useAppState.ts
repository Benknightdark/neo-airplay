/**
 * @file 接收器儀表板與雙投影引擎共用狀態 Composable，管理定期輪詢、互斥請求與錯誤訊息同步。
 */
import { onMounted, onUnmounted, ref } from "vue";
import type { ReceiverGateway } from "../application";
import type { DashboardSnapshot, ReceiverSettings, Result } from "../domain";
import { resolveReceiverUseCases } from "../presentation/di";

/**
 * 提供接收器狀態快照、操作鎖與控制方法給根介面與各子頁面使用。
 *
 * @param gateway 選擇性傳入的自訂 `ReceiverGateway`（預設透過 Vue DI 或 Tauri 閘道解析）。
 * @returns 包含 `dashboard`、`busy`、`error` 響應式狀態與各控制動作的 Composable 回傳物件。
 */
export function useAppState(gateway?: ReceiverGateway) {
  /** 解析後的接收器應用層使用案例集合。 */
  const useCases = resolveReceiverUseCases(gateway);
  /** 最新一次取得的儀表板狀態快照。 */
  const dashboard = ref<DashboardSnapshot | null>(null);
  /** 指示目前是否有變更型操作（如啟動、停止、儲存設定）正在執行中。 */
  const busy = ref(false);
  /** 最近一次操作或狀態更新失敗的錯誤提示文字。 */
  const error = ref<string | null>(null);
  /** 定期輪詢儀表板快照的計時器句柄。 */
  let dashboardPollingTimer: number | undefined;
  /** 用於丟棄過期非同步請求回應的遞增世代編號。 */
  let latestDashboardRequest = 0;
  /** 目前尚未完成之輪詢或刷新請求數，用於避免重疊輪詢。 */
  let dashboardRefreshRequestsInFlight = 0;

  /**
   * 重新向後端讀取最新儀表板狀態快照；僅採用最後發出的請求結果以防競態覆寫。
   *
   * @param clearError 是否在開始重新整理前先清空現有的錯誤訊息（預設為 `false`）。
   * @returns 在請求完成後解析的非同步 `Promise<void>`。
   */
  async function refresh(clearError = false): Promise<void> {
    if (clearError) error.value = null;
    const requestId = ++latestDashboardRequest;
    dashboardRefreshRequestsInFlight += 1;
    try {
      const result = await useCases.getDashboard();
      if (requestId === latestDashboardRequest) {
        if (result.isSuccess) {
          dashboard.value = result.value;
        } else {
          error.value = result.error ?? "無法讀取接收器狀態。";
        }
      }
    } finally {
      dashboardRefreshRequestsInFlight -= 1;
    }
  }

  /**
   * 序列執行變更型動作並於結束後刷新儀表板；失敗時保留操作本身的錯誤訊息不被 `refresh` 蓋掉。
   *
   * @typeParam T 使用案例回傳值型別。
   * @param action 回傳 `Promise<Result<T>>` 的使用案例呼叫函式。
   * @returns 動作執行與狀態刷新完畢後解析的 `Promise<void>`。
   */
  async function runAction<T>(action: () => Promise<Result<T>>): Promise<void> {
    if (busy.value) return;
    busy.value = true;
    error.value = null;
    try {
      const result = await action();
      if (result.isSuccess) {
        await refresh();
      } else {
        const failureMessage = result.error ?? "操作失敗。";
        error.value = failureMessage;
        await refresh();
        error.value = failureMessage;
      }
    } finally {
      busy.value = false;
    }
  }

  onMounted(() => {
    dashboardPollingTimer = window.setInterval(() => {
      if (!busy.value && dashboardRefreshRequestsInFlight === 0) void refresh();
    }, 1500);
  });

  onUnmounted(() => {
    if (dashboardPollingTimer !== undefined) window.clearInterval(dashboardPollingTimer);
  });

  return {
    dashboard,
    busy,
    error,
    refresh,
    /** 啟動 AirPlay 接收器並更新儀表板。 */
    startAirplay: (): Promise<void> => runAction(useCases.startAirplay),
    /** 停止 AirPlay 接收器並更新儀表板。 */
    stopAirplay: (): Promise<void> => runAction(useCases.stopAirplay),
    /** 重新掃描 Android 裝置列表並更新儀表板。 */
    refreshDevices: (): Promise<void> => runAction(useCases.refreshAndroidDevices),
    /**
     * 啟動指定 Android 裝置的畫面投影。
     *
     * @param serial 目標裝置的 ADB 序號。
     * @returns 操作與快照刷新完成後解析的 Promise。
     */
    startAndroid: (serial: string): Promise<void> =>
      runAction(() => useCases.startAndroidMirror(serial)),
    /** 停止運作中的 Android 畫面投影並更新儀表板。 */
    stopAndroid: (): Promise<void> => runAction(useCases.stopAndroidMirror),
    /**
     * 驗證並儲存接收器設定後更新儀表板。
     *
     * @param settings 使用者編輯後的設定物件。
     * @returns 設定儲存與快照刷新完成後解析的 Promise。
     */
    saveSettings: (settings: ReceiverSettings): Promise<void> =>
      runAction(() => useCases.saveSettings(settings)),
  };
}
