/**
 * @file 投影依賴工具管理應用層使用案例集合，統一將閘道操作包裝為 `Result`。
 */
import {
  Result,
  type DependencyAction,
  type DependencySnapshot,
  type ToolCatalogSnapshot,
  type ToolInstallRequest,
} from "../../domain";
import type { DependencyGateway } from "../ports/dependency-gateway";
import { executeSafely } from "./execute-safely";

/** 投影依賴工具狀態查詢、安裝、維護、版本管理與日誌檢視的前端使用案例集合介面。 */
export interface DependencyUseCases {
  /**
   * 查詢目前依賴工具狀態快照。
   *
   * @returns 包含 `DependencySnapshot` 的非同步 `Result`。
   */
  getDependencies(): Promise<Result<DependencySnapshot>>;
  /**
   * 觸發背景檢查並安裝缺少的投影工具。
   *
   * @returns 包含啟動檢查後 `DependencySnapshot` 的非同步 `Result`。
   */
  installDependencies(): Promise<Result<DependencySnapshot>>;
  /**
   * 執行工具升級或清除重新安裝作業。
   *
   * @param action 使用者觸發的維護動作。
   * @returns 包含更新後 `DependencySnapshot` 的非同步 `Result`。
   */
  manageDependencies(action: DependencyAction): Promise<Result<DependencySnapshot>>;
  /**
   * 讀取工具安裝／維護腳本的文字日誌。
   *
   * @returns 包含日誌全文字串的非同步 `Result`。
   */
  readInstallLog(): Promise<Result<string>>;
  /**
   * 查詢依目前作業系統組裝之工具與本機版本目錄。
   */
  getToolCatalog(): Promise<Result<ToolCatalogSnapshot>>;
  /**
   * 爬取官方線上版本清單並回傳最新目錄快照。
   */
  fetchToolVersions(): Promise<Result<ToolCatalogSnapshot>>;
  /**
   * 驗證並啟動指定單一工具版本之安裝／切換作業。
   */
  installToolVersion(request: ToolInstallRequest): Promise<Result<DependencySnapshot>>;
  /**
   * 清除自訂版本覆寫並恢復推薦版。
   */
  resetToolVersions(): Promise<Result<ToolCatalogSnapshot>>;
}

/**
 * 透過注入 `DependencyGateway` 建立對應的依賴工具管理使用案例物件。
 *
 * @param gateway 實作依賴工具通訊能力的 `DependencyGateway`。
 * @returns 可供 Composables 與元件使用的 `DependencyUseCases` 物件。
 */
export function createDependencyUseCases(gateway: DependencyGateway): DependencyUseCases {
  return {
    getDependencies: () => executeSafely(() => gateway.getDependencies()),
    installDependencies: () => executeSafely(() => gateway.installDependencies()),
    manageDependencies: (action: DependencyAction) =>
      executeSafely(() => gateway.manageDependencies(action)),
    readInstallLog: () => executeSafely(() => gateway.readInstallLog()),
    getToolCatalog: () => executeSafely(() => gateway.getToolCatalog()),
    fetchToolVersions: () => executeSafely(() => gateway.fetchToolVersions()),
    installToolVersion: (request: ToolInstallRequest) =>
      executeSafely(() => gateway.installToolVersion(request)),
    resetToolVersions: () => executeSafely(() => gateway.resetToolVersions()),
  };
}
