import type { EngineStatus, LogLevel } from "../types";

const engineLabels: Record<EngineStatus, string> = {
  stopped: "未啟動",
  starting: "啟動中",
  running: "執行中",
  stopping: "停止中",
  faulted: "發生錯誤",
  dependencyMissing: "缺少元件",
};

const logLabels: Record<LogLevel, string> = {
  information: "資訊",
  warning: "注意",
  error: "錯誤",
};

export function engineLabel(status: EngineStatus): string {
  return engineLabels[status];
}

export function logLabel(level: LogLevel): string {
  return logLabels[level];
}

export function localTime(timestamp: string): string {
  const date = new Date(timestamp);
  return Number.isNaN(date.valueOf())
    ? "--:--:--"
    : new Intl.DateTimeFormat("zh-TW", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false }).format(date);
}
