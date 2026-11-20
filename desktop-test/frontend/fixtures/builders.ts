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
  ManagedToolInfo,
  ReceiverSettings,
  ToolCatalogSnapshot,
  ToolReleaseOption,
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

/**
 * 建立工具發行版選項之測試資料物件。
 */
export function createToolReleaseOption(
  overrides: Partial<ToolReleaseOption> = {},
): ToolReleaseOption {
  return {
    version: "5.0.1",
    reference: "v5.0.1",
    label: "v5.0.1（內建推薦驗證版）",
    publishedAt: "2026-10-08",
    prerelease: false,
    recommended: true,
    sha256: "9f969d30cc574816077edecda65719c1c70b0aee1df1ba5fa00779ac07b8fd6e",
    ...overrides,
  };
}

/**
 * 建立單一受管理投影工具資訊之測試資料物件。
 */
export function createManagedToolInfo(
  overrides: Partial<ManagedToolInfo> = {},
): ManagedToolInfo {
  return {
    id: "scrcpy",
    name: "scrcpy（內含 ADB）",
    summary: "Android USB 畫面投影、ADB 偵測與壓縮包同步部署。",
    sourceLabel: "GitHub 官方發行（macOS Apple Silicon 官方壓縮包）",
    sourceUrl: "https://github.com/Genymobile/scrcpy/releases",
    versionControl: "selectable",
    installedVersion: "5.0.1",
    bundledComponents: ["ADB 1.0.41"],
    defaultVersion: "5.0.1",
    defaultReference: "v5.0.1",
    selectedVersion: null,
    latestRemoteVersion: "5.0.1",
    availableVersions: [createToolReleaseOption()],
    supportsManualSha256: true,
    autoVerifiesChecksum: true,
    note: "切換版本時會自動從該發行版官方 SHA256SUMS.txt 驗證壓縮檔雜湊。",
    ...overrides,
  };
}

/**
 * 建立跨作業系統工具目錄快照之測試資料物件。
 */
export function createToolCatalogSnapshot(
  overrides: Partial<ToolCatalogSnapshot> = {},
): ToolCatalogSnapshot {
  const defaultTools: ManagedToolInfo[] = [
    createManagedToolInfo({
      id: "uxplay",
      name: "UxPlay",
      summary: "AirPlay 鏡像接收引擎。",
      sourceLabel: "GitHub 原始碼（Apple Clang / CMake 編譯）",
      sourceUrl: "https://github.com/FDH2/UxPlay",
      installedVersion: "1.73.7",
      bundledComponents: [],
      defaultVersion: "1.73.7",
      defaultReference: "df67c212a433cf6dda3676dd40c097900d24e645",
      latestRemoteVersion: "1.73.7",
      availableVersions: [
        createToolReleaseOption({
          version: "1.73.7",
          reference: "df67c212a433cf6dda3676dd40c097900d24e645",
          label: "v1.73.7（內建推薦驗證版）",
          sha256: "168de53ed688f87c4022d79357ce91f9608ad5c53236bff95134d19b88efeb3f",
        }),
      ],
      autoVerifiesChecksum: false,
    }),
    createManagedToolInfo(),
    createManagedToolInfo({
      id: "gstreamer",
      name: "GStreamer 媒體管線",
      summary: "UxPlay AirPlay 視訊／音訊解碼與渲染核心函式庫。",
      sourceLabel: "Homebrew Formulae（gstreamer）",
      sourceUrl: "https://formulae.brew.sh/formula/gstreamer",
      versionControl: "packageManager",
      installedVersion: "1.28.9",
      bundledComponents: [],
      defaultVersion: "1.28.9",
      defaultReference: "package-manager",
      latestRemoteVersion: "1.28.9",
      availableVersions: [],
      supportsManualSha256: false,
      autoVerifiesChecksum: false,
    }),
  ];

  return {
    os: "macos",
    arch: "aarch64",
    platformLabel: "macOS (aarch64)",
    packageManager: "Homebrew + GitHub 官方來源",
    platformNotice: "macOS 會透過 Homebrew 維護 GStreamer 與編譯相依。",
    platformWarning: null,
    managementEnabled: true,
    remoteFetchedAt: "2026-11-19T07:00:00Z",
    remoteError: null,
    tools: defaultTools,
    ...overrides,
  };
}
