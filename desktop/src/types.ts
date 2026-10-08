export type EngineStatus = "stopped" | "starting" | "running" | "stopping" | "faulted" | "dependencyMissing";
export type AndroidDeviceStatus = "ready" | "unauthorized" | "offline" | "unknown";
export type LogLevel = "information" | "warning" | "error";

export interface EngineSnapshot {
  status: EngineStatus;
  message: string;
  processId: number | null;
}

export interface AndroidDevice {
  serial: string;
  model: string;
  status: AndroidDeviceStatus;
}

export interface ReceiverSettings {
  receiverName: string;
  renderer: string;
  fullscreen: boolean;
  startOnLogin: boolean;
}

export interface AppLogEntry {
  timestamp: string;
  level: LogLevel;
  message: string;
}

export interface DashboardSnapshot {
  settings: ReceiverSettings;
  rendererOptions: string[];
  airplay: EngineSnapshot;
  android: EngineSnapshot;
  devices: AndroidDevice[];
  logs: AppLogEntry[];
}
