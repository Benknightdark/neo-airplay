/**
 * @file builders.ts
 * @description 提供前端領域模型與快照資料的測試建構器（Test Data Builders），便於各測試案例依需求覆寫局部欄位。
 */

import type {
  AndroidDevice,
  AppLogEntry,
  DashboardSnapshot,
  DependencySnapshot,
  EngineSnapshot,
  ReceiverSettings,
} from "../../../desktop/src/domain";

/**
 * 建立接收器設定的測試資料物件。
 *
 * @param overrides 欲覆寫的接收器設定局部欄位。
 * @returns 合併預設值與覆寫欄位後的 {@link ReceiverSettings} 物件。
 */
export function createReceiverSettings(
  overrides: Partial<ReceiverSettings> = {},
): ReceiverSettings {
  return {
    receiverName: "Neo AirPlay",
    renderer: "Auto",
    fullscreen: false,
    startOnLogin: false,
    ...overrides,
  };
}

/**
 * 建立引擎執行狀態快照的測試資料物件。
 *
 * @param overrides 欲覆寫的引擎快照局部欄位。
 * @returns 合併預設值與覆寫欄位後的 {@link EngineSnapshot} 物件。
 */
export function createEngineSnapshot(
  overrides: Partial<EngineSnapshot> = {},
): EngineSnapshot {
  return {
    status: "stopped",
    message: "AirPlay 接收器尚未啟動。",
    processId: null,
    ...overrides,
  };
}

/**
 * 建立 Android 裝置資訊的測試資料物件。
 *
 * @param overrides 欲覆寫的 Android 裝置局部欄位。
 * @returns 合併預設值與覆寫欄位後的 {@link AndroidDevice} 物件。
 */
export function createAndroidDevice(
  overrides: Partial<AndroidDevice> = {},
): AndroidDevice {
  return {
    serial: "phone-1",
    model: "Pixel 8",
    status: "ready",
    ...overrides,
  };
}

/**
 * 建立單筆系統記錄項目的測試資料物件。
 *
 * @param overrides 欲覆寫的系統記錄局部欄位。
 * @returns 合併預設值與覆寫欄位後的 {@link AppLogEntry} 物件。
 */
export function createLogEntry(
  overrides: Partial<AppLogEntry> = {},
): AppLogEntry {
  return {
    timestamp: "2026-11-13T12:00:00+08:00",
    level: "information",
    message: "服務已就緒",
    ...overrides,
  };
}

/**
 * 建立儀表板完整狀態快照的測試資料物件。
 *
 * @param overrides 欲覆寫的儀表板快照局部欄位。
 * @returns 合併預設值與覆寫欄位後的 {@link DashboardSnapshot} 物件。
 */
export function createDashboardSnapshot(
  overrides: Partial<DashboardSnapshot> = {},
): DashboardSnapshot {
  return {
    settings: createReceiverSettings(),
    rendererOptions: ["Auto"],
    airplay: createEngineSnapshot(),
    android: createEngineSnapshot({ message: "Android 投影尚未啟動。" }),
    devices: [],
    logs: [],
    ...overrides,
  };
}

/**
 * 建立投影工具依賴狀態快照的測試資料物件。
 *
 * @param overrides 欲覆寫的依賴快照局部欄位。
 * @returns 合併預設值與覆寫欄位後的 {@link DependencySnapshot} 物件。
 */
export function createDependencySnapshot(
  overrides: Partial<DependencySnapshot> = {},
): DependencySnapshot {
  return {
    status: "ready",
    operation: null,
    message: "投影工具已就緒。",
    missing: [],
    logPath: "/tmp/neo-airplay-install.log",
    ...overrides,
  };
}
