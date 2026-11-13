/**
 * @file 投影依賴工具檢查與背景維護狀態 Composable，管理定期輪詢、請求序列化與忙碌狀態回呼。
 */
import { computed, onMounted, onUnmounted, ref } from "vue";
import type { DependencyGateway } from "../application";
import {
  isDependencyInstalling,
  isDependencyUnavailable,
  type DependencyAction,
  type DependencySnapshot,
  type Result,
} from "../domain";
import { resolveDependencyUseCases } from "../presentation/di";

/** `useDependencyState` 的可選設定參數。 */
export interface UseDependencyStateOptions {
  /** 選擇性覆寫預設注入來源的 `DependencyGateway`（供測試替身使用）。 */
  gateway?: DependencyGateway;
  /** 當檢查或維護作業進入或離開忙碌狀態時觸發的回呼函式。 */
  onBusyChange?: (busy: boolean) => void;
}

/**
 * 管理依賴工具狀態查詢、自動重試安裝與升級／重裝動作的響應式 Composable。
 *
 * @param options 設定閘道注入與忙碌狀態通知回呼的可選參數物件。
 * @returns 包含狀態快照、計算旗標與操作方法的 Composable 回傳物件。
 */
export function useDependencyState(options: UseDependencyStateOptions = {}) {
  /** 解析後的依賴工具應用層使用案例集合。 */
  const useCases = resolveDependencyUseCases(options.gateway);
  /** 最新一次取得的依賴工具狀態快照。 */
  const snapshot = ref<DependencySnapshot | null>(null);
  /** 查詢或維護操作失敗時的錯誤提示字串。 */
  const error = ref("");
  /** 指示前端是否已送出主動請求且仍在等待後端初步回應。 */
  const requesting = ref(false);
  /** 是否處於檢查、安裝或主動請求中的衍生狀態。 */
  const installing = computed(() => isDependencyInstalling(snapshot.value, requesting.value));
  /** 目前依賴工具管理是否尚未載入或處於停用狀態的衍生旗標。 */
  const unavailable = computed(() => isDependencyUnavailable(snapshot.value));

  /** 定期輪詢依賴狀態的計時器句柄。 */
  let timer: number | undefined;
  /** 元件卸載旗標，防止卸載後仍覆寫狀態或排程輪詢。 */
  let disposed = false;
  /** 防止同時發起多個 `getDependencies` 輪詢請求的鎖定旗標。 */
  let polling = false;
  /** 用於區分手動操作世代、忽略陳舊輪詢回應的遞增計數器。 */
  let requestGeneration = 0;

  /**
   * 停止定期狀態輪詢計時器（例如在組件卸載或狀態回報 `disabled` 時呼叫）。
   */
  function stopPolling(): void {
    if (timer !== undefined) {
      window.clearInterval(timer);
      timer = undefined;
    }
  }

  /**
   * 將最新的 `installing` 忙碌旗標通知給外部監聽者。
   */
  function notifyBusy(): void {
    options.onBusyChange?.(installing.value);
  }

  /**
   * 重新向後端查詢依賴工具狀態；若工具管理已停用則自動停止後續輪詢。
   *
   * @returns 查詢完成並更新響應式狀態後解析的 `Promise<void>`。
   */
  async function refresh(): Promise<void> {
    if (disposed || polling || requesting.value) return;
    polling = true;
    const generation = requestGeneration;
    try {
      const result = await useCases.getDependencies();
      if (disposed || generation !== requestGeneration) return;
      if (result.isSuccess && result.value) {
        snapshot.value = result.value;
        error.value = "";
        notifyBusy();
        if (result.value.status === "disabled") stopPolling();
      } else {
        error.value = result.error ?? "無法取得工具狀態。";
        notifyBusy();
      }
    } finally {
      polling = false;
    }
  }

  /**
   * 執行主動變更依賴狀態的請求（安裝／升級／重裝），妥善切換 `requesting` 與世代保護。
   *
   * @param operation 回傳 `Promise<Result<DependencySnapshot>>` 的應用層作業閉包。
   * @param stopWhenDisabled 當回傳狀態為 `disabled` 時是否立刻停止背景輪詢（預設 `false`）。
   * @returns 操作完成並同步最新狀態後解析的 `Promise<void>`。
   */
  async function executeRequest(
    operation: () => Promise<Result<DependencySnapshot>>,
    stopWhenDisabled = false,
  ): Promise<void> {
    const generation = ++requestGeneration;
    requesting.value = true;
    error.value = "";
    notifyBusy();
    try {
      const result = await operation();
      if (disposed || generation !== requestGeneration) return;
      if (result.isSuccess && result.value) {
        snapshot.value = result.value;
        if (stopWhenDisabled && result.value.status === "disabled") stopPolling();
      } else {
        error.value = result.error ?? "工具作業失敗。";
      }
    } finally {
      if (!disposed && generation === requestGeneration) {
        requesting.value = false;
        notifyBusy();
      }
    }
  }

  /**
   * 重新觸發投影工具檢查與缺失項目安裝（若目前未處於安裝或檢查中）。
   *
   * @returns 請求完成後解析的 `Promise<void>`。
   */
  async function retry(): Promise<void> {
    if (disposed || installing.value) return;
    await executeRequest(() => useCases.installDependencies(), true);
  }

  /**
   * 觸發升級或清除重裝維護動作（僅在非安裝中且依賴管理可用時生效）。
   *
   * @param action 使用者點選的維護動作（`upgrade` 或 `reinstall`）。
   * @returns 請求完成後解析的 `Promise<void>`。
   */
  async function runAction(action: DependencyAction): Promise<void> {
    if (disposed || installing.value || unavailable.value) return;
    await executeRequest(() => useCases.manageDependencies(action));
  }

  onMounted(() => {
    void refresh();
    timer = window.setInterval(() => void refresh(), 1500);
  });

  onUnmounted(() => {
    disposed = true;
    stopPolling();
  });

  return {
    snapshot,
    error,
    requesting,
    installing,
    unavailable,
    refresh,
    retry,
    runAction,
  };
}
