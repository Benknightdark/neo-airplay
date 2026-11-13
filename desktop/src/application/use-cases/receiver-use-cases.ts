/**
 * @file 接收器控制與設定儲存應用層使用案例集合，提供輸入驗證並以 `Result` 回傳結果。
 */
import {
  isReceiverNameValid,
  Result,
  sanitizeReceiverSettings,
  type AndroidDevice,
  type DashboardSnapshot,
  type EngineSnapshot,
  type ReceiverSettings,
} from "../../domain";
import type { ReceiverGateway } from "../ports/receiver-gateway";
import { executeSafely } from "./execute-safely";

/** 接收器、AirPlay 與 Android 投影相關的前端使用案例集合介面。 */
export interface ReceiverUseCases {
  /**
   * 載入儀表板所需的設定、引擎狀態、Android 裝置與診斷日誌快照。
   *
   * @returns 包裝 `DashboardSnapshot` 或錯誤訊息的 `Result`。
   */
  getDashboard(): Promise<Result<DashboardSnapshot>>;
  /**
   * 驗證接收器名稱、正規化設定並儲存至後端。
   *
   * @param settings 使用者編輯後的接收器設定物件。
   * @returns 成功時包含已儲存設定的 `Result`；驗證或儲存失敗時包含錯誤詳情。
   */
  saveSettings(settings: ReceiverSettings): Promise<Result<ReceiverSettings>>;
  /**
   * 啟動 AirPlay 接收器。
   *
   * @returns 包含 AirPlay 最新 `EngineSnapshot` 的 `Result`。
   */
  startAirplay(): Promise<Result<EngineSnapshot>>;
  /**
   * 停止運作中的 AirPlay 接收器。
   *
   * @returns 包含停止後 `EngineSnapshot` 的 `Result`。
   */
  stopAirplay(): Promise<Result<EngineSnapshot>>;
  /**
   * 觸發後端重新掃描已連線的 Android 裝置列表。
   *
   * @returns 包含 `AndroidDevice` 陣列的 `Result`。
   */
  refreshAndroidDevices(): Promise<Result<AndroidDevice[]>>;
  /**
   * 驗證裝置序號非空後啟動該 Android 裝置的畫面投影。
   *
   * @param serial 目標裝置的 ADB 序號字串。
   * @returns 包含 Android 引擎 `EngineSnapshot` 或驗證失敗的 `Result`。
   */
  startAndroidMirror(serial: string): Promise<Result<EngineSnapshot>>;
  /**
   * 停止目前的 Android 畫面投影。
   *
   * @returns 包含停止後 `EngineSnapshot` 的 `Result`。
   */
  stopAndroidMirror(): Promise<Result<EngineSnapshot>>;
}

/**
 * 透過注入 `ReceiverGateway` 建立對應的前端接收器使用案例實作物件。
 *
 * @param gateway 實作後端通訊能力的 `ReceiverGateway`。
 * @returns 可供 Composables 與元件使用的 `ReceiverUseCases` 物件。
 */
export function createReceiverUseCases(gateway: ReceiverGateway): ReceiverUseCases {
  return {
    /**
     * 取得儀表板完整狀態快照。
     *
     * @returns 包裝 `DashboardSnapshot` 的非同步 `Result`。
     */
    getDashboard: () => executeSafely(() => gateway.getDashboard()),

    /**
     * 驗證並正規化接收器設定後送交閘道儲存。
     *
     * @param settings 使用者提交的設定內容。
     * @returns 名稱無效時回傳 `Result.validation`，否則回傳儲存結果 `Result`。
     */
    saveSettings: async (settings: ReceiverSettings) => {
      if (!isReceiverNameValid(settings.receiverName)) {
        return Result.validation("請輸入有效的接收器名稱。");
      }
      const normalized = sanitizeReceiverSettings(settings);
      return executeSafely(() => gateway.saveSettings(normalized));
    },

    /**
     * 啟動 AirPlay 接收器。
     *
     * @returns 包裝 `EngineSnapshot` 的非同步 `Result`。
     */
    startAirplay: () => executeSafely(() => gateway.startAirplay()),

    /**
     * 停止 AirPlay 接收器。
     *
     * @returns 包裝 `EngineSnapshot` 的非同步 `Result`。
     */
    stopAirplay: () => executeSafely(() => gateway.stopAirplay()),

    /**
     * 重新掃描已連線的 Android 裝置。
     *
     * @returns 包裝 `AndroidDevice[]` 的非同步 `Result`。
     */
    refreshAndroidDevices: () => executeSafely(() => gateway.refreshAndroidDevices()),

    /**
     * 去除序號前後空白並驗證非空後啟動 Android 畫面投影。
     *
     * @param serial 目標裝置序號。
     * @returns 序號為空時回傳 `Result.validation`，否則回傳執行結果 `Result`。
     */
    startAndroidMirror: async (serial: string) => {
      const trimmedSerial = serial.trim();
      if (!trimmedSerial) {
        return Result.validation("請先選擇 Android 裝置。");
      }
      return executeSafely(() => gateway.startAndroidMirror(trimmedSerial));
    },

    /**
     * 停止 Android 畫面投影。
     *
     * @returns 包裝 `EngineSnapshot` 的非同步 `Result`。
     */
    stopAndroidMirror: () => executeSafely(() => gateway.stopAndroidMirror()),
  };
}
