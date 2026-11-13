/**
 * @file 診斷日誌資料模型、本地時間格式化與日誌篩選／反轉排序的領域函式。
 */

/** 單筆應用程式日誌的重要程度等級。 */
export type LogLevel = "information" | "warning" | "error";
/** 診斷檢視器使用的日誌過濾條件（全部或單一等級）。 */
export type LogFilter = "all" | LogLevel;

/** 單筆後端寫入的診斷日誌紀錄。 */
export interface AppLogEntry {
  /** ISO 8601 格式的時間戳記字串。 */
  timestamp: string;
  /** 日誌等級。 */
  level: LogLevel;
  /** 日誌訊息文字。 */
  message: string;
}

/** 日誌等級對應的繁體中文標籤對照表。 */
const logLabels: Record<LogLevel, string> = {
  information: "資訊",
  warning: "注意",
  error: "錯誤",
};

/**
 * 將英文日誌等級轉換為繁體中文顯示名稱。
 *
 * @param level 日誌等級。
 * @returns 對應的中文等級名稱（資訊／注意／錯誤）。
 */
export function logLabel(level: LogLevel): string {
  return logLabels[level];
}

/**
 * 將 ISO 時間戳記格式化為台灣 24 小時制本地時分秒字串（`HH:mm:ss`）。
 *
 * @param timestamp 原始時間戳記字串。
 * @returns 格式化時間字串；無法解析時回傳 `"--:--:--"`。
 */
export function localTime(timestamp: string): string {
  const date = new Date(timestamp);
  return Number.isNaN(date.valueOf())
    ? "--:--:--"
    : new Intl.DateTimeFormat("zh-TW", {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
        hour12: false,
      }).format(date);
}

/**
 * 依指定過濾條件篩選日誌，並將順序反轉為最新紀錄在最上方顯示。
 *
 * @param logs 依時間順序遞增排列的原始日誌陣列。
 * @param filter 目前選用的等級過濾條件。
 * @returns 過濾並反轉後的新 `AppLogEntry` 陣列（不修改原陣列）。
 */
export function filterAndReverseLogs(
  logs: readonly AppLogEntry[],
  filter: LogFilter,
): AppLogEntry[] {
  const entries = filter === "all" ? logs : logs.filter((entry) => entry.level === filter);
  return [...entries].reverse();
}
