/**
 * @file 投影依賴工具管理輸出埠介面，隔離 Tauri IPC 與工具安裝／維護實作。
 */
import type {
  DependencyAction,
  DependencySnapshot,
  ToolCatalogSnapshot,
  ToolInstallRequest,
} from "../../domain";

/** 抽象化投影依賴工具檢查、安裝、維護、版本目錄爬取與安裝日誌讀取的閘道契約。 */
export interface DependencyGateway {
  /**
   * 查詢目前依賴工具狀態快照。
   *
   * @returns 解析為 `DependencySnapshot` 的非同步 Promise。
   */
  getDependencies(): Promise<DependencySnapshot>;
  /**
   * 檢查並啟動缺失投影工具的背景自動安裝流程。
   *
   * @returns 解析為最新 `DependencySnapshot` 的非同步 Promise。
   */
  installDependencies(): Promise<DependencySnapshot>;
  /**
   * 執行升級或重新安裝維護動作。
   *
   * @param action 使用者觸發的維護動作（`upgrade` 或 `reinstall`）。
   * @returns 解析為維護開始後 `DependencySnapshot` 的非同步 Promise。
   */
  manageDependencies(action: DependencyAction): Promise<DependencySnapshot>;
  /**
   * 讀取依賴工具安裝或維護腳本產生的文字日誌內容。
   *
   * @returns 解析為日誌文字內容的非同步 Promise。
   */
  readInstallLog(): Promise<string>;
  /**
   * 取得依作業系統組裝的工具目錄與本機已安裝版本快照。
   */
  getToolCatalog(): Promise<ToolCatalogSnapshot>;
  /**
   * 從白名單官方來源爬取工具最新版本清單並更新目錄快照。
   */
  fetchToolVersions(): Promise<ToolCatalogSnapshot>;
  /**
   * 安裝或切換指定工具與版本。
   */
  installToolVersion(request: ToolInstallRequest): Promise<DependencySnapshot>;
  /**
   * 清除自訂工具版本偏好設定並恢復推薦預設版。
   */
  resetToolVersions(): Promise<ToolCatalogSnapshot>;
}
