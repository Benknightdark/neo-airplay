/**
 * @file 接收器引擎狀態、設定契約與 AirPlay 介面顯示規則的純領域邏輯。
 */
import type { AndroidDevice } from "./android";
import type { AppLogEntry } from "./diagnostics";

/** 單一投影引擎的生命週期狀態識別字串。 */
export type EngineStatus =
  | "stopped"
  | "starting"
  | "running"
  | "stopping"
  | "faulted"
  | "dependencyMissing";

/** 投影引擎目前狀態與子程序 PID 快照。 */
export interface EngineSnapshot {
  /** 目前引擎生命週期狀態。 */
  status: EngineStatus;
  /** 對應此狀態的說明或錯誤訊息。 */
  message: string;
  /** 運作中的外部子程序 PID，未啟動或已停止時為 `null`。 */
  processId: number | null;
}

/** 投影接收器的可持久化設定結構。 */
export interface ReceiverSettings {
  /** AirPlay 區網廣播名稱。 */
  receiverName: string;
  /** 選用的視訊渲染器識別名稱。 */
  renderer: string;
  /** 投影啟動時是否使用全螢幕模式。 */
  fullscreen: boolean;
  /** 是否於作業系統登入時自動啟動應用程式。 */
  startOnLogin: boolean;
}

/** 儀表板畫面一次讀取所需的完整後端狀態快照。 */
export interface DashboardSnapshot {
  /** 目前生效的接收器設定。 */
  settings: ReceiverSettings;
  /** 目前作業系統可用的視訊渲染器選項清單。 */
  rendererOptions: string[];
  /** AirPlay 接收器引擎最新狀態。 */
  airplay: EngineSnapshot;
  /** Android 畫面投影引擎最新狀態。 */
  android: EngineSnapshot;
  /** 最近一次掃描取得的 Android 裝置列表。 */
  devices: AndroidDevice[];
  /** 近期診斷日誌清單。 */
  logs: AppLogEntry[];
}

/** 接收器名稱為空時退回使用的預設廣播名稱。 */
const DEFAULT_RECEIVER_NAME = "Neo AirPlay";

/** 引擎狀態與繁體中文介面顯示標籤的對應表。 */
const engineLabels: Record<EngineStatus, string> = {
  stopped: "未啟動",
  starting: "啟動中",
  running: "執行中",
  stopping: "停止中",
  faulted: "發生錯誤",
  dependencyMissing: "缺少元件",
};

/**
 * 將英文引擎狀態碼轉譯為繁體中文顯示標籤。
 *
 * @param status 引擎目前狀態。
 * @returns 對應的中文狀態標籤文字。
 */
export function engineLabel(status: EngineStatus): string {
  return engineLabels[status];
}

/**
 * 判斷 AirPlay 引擎是否正處於啟動、運行或停止中（視為作用中狀態）。
 *
 * @param status 引擎目前狀態。
 * @returns 處於 `starting`、`running` 或 `stopping` 時回傳 `true`。
 */
export function isAirplayActive(status: EngineStatus): boolean {
  return status === "starting" || status === "running" || status === "stopping";
}

/**
 * 依據 AirPlay 作用中狀態決定切換按鈕上的停止提示文字。
 *
 * @param status AirPlay 引擎目前狀態。
 * @returns 按鈕顯示文字（例如 `"取消啟動"`、`"正在停止…"` 或 `"停止接收"`）。
 */
export function airplayButtonLabel(status: EngineStatus): string {
  if (status === "starting") return "取消啟動";
  if (status === "stopping") return "正在停止…";
  return "停止接收";
}

/**
 * 依據 AirPlay 狀態與接收器廣播名稱組合儀表板提示說明文字。
 *
 * @param status AirPlay 引擎目前狀態。
 * @param receiverName 目前設定的接收器名稱。
 * @returns 對應狀態的使用指引或環境需求提示。
 */
export function airplayHint(status: EngineStatus, receiverName: string): string {
  if (status === "starting") {
    return "正在等待 Bonjour 廣播；接收器出現在 iPhone 清單後即可連線。";
  }
  if (status === "running") {
    return `請從 iPhone 的螢幕鏡像清單選擇「${receiverName}」。`;
  }
  return "需要安裝 UxPlay 與影音執行環境";
}

/**
 * 驗證使用者輸入的接收器名稱在修剪前後空白後是否非空。
 *
 * @param name 待驗證的接收器名稱字串。
 * @returns 若包含至少一個非空白字元則回傳 `true`。
 */
export function isReceiverNameValid(name: string): boolean {
  return name.trim().length > 0;
}

/**
 * 清理接收器設定物件，確保 `receiverName` 若為全空白時退回 `"Neo AirPlay"`。
 *
 * @param settings 原始接收器設定物件。
 * @returns 正規化後的新 `ReceiverSettings` 物件（不突變輸入參數）。
 */
export function sanitizeReceiverSettings(settings: ReceiverSettings): ReceiverSettings {
  const trimmedName = settings.receiverName.trim();
  return {
    ...settings,
    receiverName: trimmedName.length > 0 ? trimmedName : DEFAULT_RECEIVER_NAME,
  };
}
