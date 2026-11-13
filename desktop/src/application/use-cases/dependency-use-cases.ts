/**
 * @file 投影依賴工具管理應用層使用案例集合，統一將閘道操作包裝為 `Result`。
 */
import {
  Result,
  type DependencyAction,
  type DependencySnapshot,
} from "../../domain";
import type { DependencyGateway } from "../ports/dependency-gateway";
import { executeSafely } from "./execute-safely";

/** 投影依賴工具狀態查詢、安裝、維護與日誌檢視的前端使用案例集合介面。 */
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
}

/**
 * 透過注入 `DependencyGateway` 建立對應的依賴工具管理使用案例物件。
 *
 * @param gateway 實作依賴工具通訊能力的 `DependencyGateway`。
 * @returns 可供 Composables 與元件使用的 `DependencyUseCases` 物件。
 */
export function createDependencyUseCases(gateway: DependencyGateway): DependencyUseCases {
  return {
    /**
     * 取得目前依賴工具快照。
     *
     * @returns 包裝 `DependencySnapshot` 的 `Result`。
     */
    getDependencies: () => executeSafely(() => gateway.getDependencies()),
    /**
     * 觸發自動安裝缺少工具。
     *
     * @returns 包裝 `DependencySnapshot` 的 `Result`。
     */
    installDependencies: () => executeSafely(() => gateway.installDependencies()),
    /**
     * 執行升級或重新安裝維護作業。
     *
     * @param action 維護動作（`upgrade` 或 `reinstall`）。
     * @returns 包裝 `DependencySnapshot` 的 `Result`。
     */
    manageDependencies: (action: DependencyAction) =>
      executeSafely(() => gateway.manageDependencies(action)),
    /**
     * 讀取完整安裝日誌內容。
     *
     * @returns 包裝日誌字串的 `Result`。
     */
    readInstallLog: () => executeSafely(() => gateway.readInstallLog()),
  };
}
