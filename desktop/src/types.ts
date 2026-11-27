/**
 * @file 向下相容的型別轉匯檔，統一從 `domain` 層重新匯出所有前端共享型別。
 */
export type {
  AirplayAuthMode,
  AndroidDevice,
  AndroidDeviceStatus,
  AppLogEntry,
  DashboardSnapshot,
  DependencyAction,
  DependencyOperation,
  DependencySnapshot,
  DependencyStatus,
  EngineSnapshot,
  EngineStatus,
  LogFilter,
  LogLevel,
  ManagedToolInfo,
  PlatformOs,
  ReceiverSettings,
  ToolCatalogSnapshot,
  ToolId,
  ToolInstallRequest,
  ToolReleaseOption,
  VersionControlMode,
} from "./domain";
