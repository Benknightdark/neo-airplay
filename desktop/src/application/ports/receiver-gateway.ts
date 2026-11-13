/**
 * @file 接收器執行期存取輸出埠介面，定義前端應用層對後端接收器控制與設定的抽象依賴。
 */
import type {
  AndroidDevice,
  DashboardSnapshot,
  EngineSnapshot,
  ReceiverSettings,
} from "../../domain";

/** 抽象化後端接收器與雙投影引擎操作通訊的閘道契約。 */
export interface ReceiverGateway {
  /**
   * 取得儀表板完整狀態快照。
   *
   * @returns 解析為 `DashboardSnapshot` 的非同步 Promise。
   */
  getDashboard(): Promise<DashboardSnapshot>;
  /**
   * 驗證並儲存接收器設定。
   *
   * @param settings 欲儲存的接收器設定物件。
   * @returns 解析為已正規化 `ReceiverSettings` 的非同步 Promise。
   */
  saveSettings(settings: ReceiverSettings): Promise<ReceiverSettings>;
  /**
   * 啟動 AirPlay（UxPlay）接收器。
   *
   * @returns 解析為 AirPlay 最新 `EngineSnapshot` 的非同步 Promise。
   */
  startAirplay(): Promise<EngineSnapshot>;
  /**
   * 停止 AirPlay（UxPlay）接收器。
   *
   * @returns 解析為停止後 `EngineSnapshot` 的非同步 Promise。
   */
  stopAirplay(): Promise<EngineSnapshot>;
  /**
   * 重新掃描透過 ADB 連線的 Android 裝置列表。
   *
   * @returns 解析為 `AndroidDevice` 陣列的非同步 Promise。
   */
  refreshAndroidDevices(): Promise<AndroidDevice[]>;
  /**
   * 啟動指定 Android 裝置的畫面投影。
   *
   * @param serial 目標裝置的 ADB 序號。
   * @returns 解析為 Android 引擎最新 `EngineSnapshot` 的非同步 Promise。
   */
  startAndroidMirror(serial: string): Promise<EngineSnapshot>;
  /**
   * 停止目前的 Android 畫面投影。
   *
   * @returns 解析為停止後 `EngineSnapshot` 的非同步 Promise。
   */
  stopAndroidMirror(): Promise<EngineSnapshot>;
}
