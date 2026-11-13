/**
 * @file 向下相容的型別轉匯檔，統一從 `domain` 層重新匯出所有前端共享型別。
 */
export type {
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
  ReceiverSettings,
} from "./domain";
