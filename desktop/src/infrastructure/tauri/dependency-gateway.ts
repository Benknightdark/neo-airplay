/**
 * @file 透過 Tauri API 模組實作 `DependencyGateway` 應用層輸出埠。
 */
import * as api from "../../api";
import type { DependencyGateway } from "../../application";

/**
 * 建立使用預設 Tauri IPC `api` 模組的 `DependencyGateway` 實例。
 *
 * @returns 委派呼叫 Tauri 後端工具管理命令的 `DependencyGateway` 物件。
 */
export function createTauriDependencyGateway(): DependencyGateway {
  return {
    /**
     * 呼叫後端 `get_dependencies` 命令取得目前依賴工具快照。
     *
     * @returns 依賴工具狀態快照 Promise。
     */
    getDependencies: () => api.getDependencies(),
    /**
     * 呼叫後端 `install_dependencies` 命令啟動檢查與安裝。
     *
     * @returns 依賴工具狀態快照 Promise。
     */
    installDependencies: () => api.installDependencies(),
    /**
     * 呼叫後端 `manage_dependencies` 命令執行升級或重裝。
     *
     * @param action 使用者觸發的維護動作。
     * @returns 依賴工具狀態快照 Promise。
     */
    manageDependencies: (action) => api.manageDependencies(action),
    /**
     * 呼叫後端 `read_install_log` 命令讀取安裝腳本文字日誌。
     *
     * @returns 安裝日誌純文字內容 Promise。
     */
    readInstallLog: () => api.readInstallLog(),
  };
}
