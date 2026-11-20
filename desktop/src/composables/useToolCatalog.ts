/**
 * @file 跨作業系統投影工具目錄與線上版本爬取狀態管理 Composable。
 */
import { computed, onMounted, onUnmounted, ref, watch, type Ref } from "vue";
import type { DependencyGateway } from "../application";
import type { DependencyStatus, ToolCatalogSnapshot } from "../domain";
import { resolveDependencyUseCases } from "../presentation/di";

/** `useToolCatalog` 的設定參數。 */
export interface UseToolCatalogOptions {
  /** 選擇性覆寫預設注入的 `DependencyGateway`（供測試替身使用）。 */
  gateway?: DependencyGateway;
  /** 關聯之依賴作業狀態，用於安裝／切換完成時自動刷新本機已安裝版本。 */
  dependencyStatus?: Ref<DependencyStatus | undefined>;
  /** 載入頁面後若工具管理已啟用且尚無快取，是否於背景自動爬取一次線上版本（預設 `true`）。 */
  autoFetchRemote?: boolean;
}

/**
 * 管理作業系統專屬工具清單、本機已安裝版本探測、線上版本爬取與自訂版本偏好重置。
 *
 * @param options 選用閘道與自動爬取設定。
 * @returns 工具目錄快照、載入／爬取狀態旗標與操作函式。
 */
export function useToolCatalog(options: UseToolCatalogOptions = {}) {
  const useCases = resolveDependencyUseCases(options.gateway);
  const autoFetchRemote = options.autoFetchRemote ?? true;

  const catalog = ref<ToolCatalogSnapshot | null>(null);
  const loadingCatalog = ref(false);
  const fetchingRemote = ref(false);
  const catalogError = ref("");

  /** 目前是否有任何工具已套用非預設版本鎖定覆寫。 */
  const hasCustomVersions = computed(
    () => catalog.value?.tools.some((tool) => Boolean(tool.selectedVersion)) ?? false,
  );

  let disposed = false;

  /**
   * 從後端讀取目前作業系統之本機工具目錄、已安裝版本與快取線上清單。
   */
  async function loadCatalog(): Promise<void> {
    if (disposed) return;
    loadingCatalog.value = true;
    try {
      const result = await useCases.getToolCatalog();
      if (disposed) return;
      if (result.isSuccess && result.value) {
        catalog.value = result.value;
        catalogError.value = "";
      } else {
        catalogError.value = result.error ?? "無法取得工具目錄。";
      }
    } finally {
      if (!disposed) loadingCatalog.value = false;
    }
  }

  /**
   * 透過後端白名單 HTTPS 爬蟲更新官方線上版本清單。
   */
  async function fetchRemoteVersions(): Promise<void> {
    if (disposed || fetchingRemote.value) return;
    fetchingRemote.value = true;
    catalogError.value = "";
    try {
      const result = await useCases.fetchToolVersions();
      if (disposed) return;
      if (result.isSuccess && result.value) {
        catalog.value = result.value;
      } else {
        catalogError.value = result.error ?? "無法爬取線上工具版本。";
      }
    } finally {
      if (!disposed) fetchingRemote.value = false;
    }
  }

  /**
   * 清除使用者自訂版本覆寫，還原各作業系統之內建推薦版。
   */
  async function resetToolVersions(): Promise<void> {
    if (disposed) return;
    catalogError.value = "";
    const result = await useCases.resetToolVersions();
    if (disposed) return;
    if (result.isSuccess && result.value) {
      catalog.value = result.value;
    } else {
      catalogError.value = result.error ?? "無法重設工具版本偏好。";
    }
  }

  if (options.dependencyStatus) {
    watch(options.dependencyStatus, (nextStatus, prevStatus) => {
      const wasBusy = prevStatus === "checking" || prevStatus === "installing";
      const isDone = nextStatus === "ready" || nextStatus === "failed";
      if (wasBusy && isDone) {
        void loadCatalog();
      }
    });
  }

  onMounted(async () => {
    await loadCatalog();
    if (
      !disposed &&
      autoFetchRemote &&
      catalog.value?.managementEnabled &&
      !catalog.value.remoteFetchedAt
    ) {
      await fetchRemoteVersions();
    }
  });

  onUnmounted(() => {
    disposed = true;
  });

  return {
    catalog,
    loadingCatalog,
    fetchingRemote,
    catalogError,
    hasCustomVersions,
    loadCatalog,
    fetchRemoteVersions,
    resetToolVersions,
  };
}
