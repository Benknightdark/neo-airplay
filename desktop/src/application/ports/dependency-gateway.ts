/**
 * @file 投影依賴工具管理輸出埠介面，隔離 Tauri IPC 與工具安裝／維護實作。
 */
import type { DependencyAction, DependencySnapshot } from "../../domain";

/** 抽象化投影依賴工具檢查、安裝、維護與安裝日誌讀取的閘道契約。 */
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
}
