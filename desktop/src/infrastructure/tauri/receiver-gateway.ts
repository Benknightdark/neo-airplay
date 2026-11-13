/**
 * @file 透過 Tauri API 模組實作 `ReceiverGateway` 應用層輸出埠。
 */
import * as api from "../../api";
import type { ReceiverGateway } from "../../application";

/**
 * 建立使用預設 Tauri IPC `api` 模組的 `ReceiverGateway` 實例。
 *
 * @returns 委派呼叫 Tauri 後端接收器命令的 `ReceiverGateway` 物件。
 */
export function createTauriReceiverGateway(): ReceiverGateway {
  return {
    /**
     * 呼叫後端 `get_dashboard` 命令取得儀表板快照。
     *
     * @returns 儀表板快照 Promise。
     */
    getDashboard: () => api.getDashboard(),
    /**
     * 呼叫後端 `save_settings` 命令保存接收器設定。
     *
     * @param settings 欲儲存的設定內容。
     * @returns 已儲存設定 Promise。
     */
    saveSettings: (settings) => api.saveSettings(settings),
    /**
     * 呼叫後端 `start_airplay` 命令啟動 AirPlay。
     *
     * @returns AirPlay 引擎快照 Promise。
     */
    startAirplay: () => api.startAirplay(),
    /**
     * 呼叫後端 `stop_airplay` 命令停止 AirPlay。
     *
     * @returns AirPlay 引擎快照 Promise。
     */
    stopAirplay: () => api.stopAirplay(),
    /**
     * 呼叫後端 `refresh_android_devices` 命令重新掃描裝置。
     *
     * @returns Android 裝置清單 Promise。
     */
    refreshAndroidDevices: () => api.refreshAndroidDevices(),
    /**
     * 呼叫後端 `start_android_mirror` 命令啟動指定裝置投影。
     *
     * @param serial 目標裝置的 ADB 序號。
     * @returns Android 引擎快照 Promise。
     */
    startAndroidMirror: (serial) => api.startAndroidMirror(serial),
    /**
     * 呼叫後端 `stop_android_mirror` 命令停止 Android 投影。
     *
     * @returns Android 引擎快照 Promise。
     */
    stopAndroidMirror: () => api.stopAndroidMirror(),
  };
}
