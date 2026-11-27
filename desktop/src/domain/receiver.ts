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

/** AirPlay 區域網路連線 PIN 認證模式。 */
export type AirplayAuthMode = "none" | "randomPin" | "fixedPin";

/** 單一 AirPlay 廣播名稱允許的最大 UTF-8 位元組數（Bonjour / DNS-SD 實例名稱上限）。 */
export const MAX_RECEIVER_NAME_UTF8_BYTES = 63;

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
  /** AirPlay 區網連線 PIN 認證模式。 */
  airplayAuthMode: AirplayAuthMode;
  /** 固定 PIN 模式下的 4 位數字密碼（非固定模式時為空字串）。 */
  airplayPin: string;
  /** 啟動 Android 投影時是否停用電腦與裝置間的自動剪貼簿同步。 */
  androidNoClipboardSync: boolean;
  /** 啟動 Android 投影時是否限制為唯讀模式（禁止操控與檔案拖放安裝）。 */
  androidReadOnly: boolean;
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

/** 控制字元與 Unicode 雙向（Bidi）方向覆寫字元正規運算式。 */
const CONTROL_OR_BIDI_PATTERN =
  // eslint-disable-next-line no-control-regex
  /[\u0000-\u001F\u007F-\u009F\u061C\u200E\u200F\u202A-\u202E\u2066-\u2069]/g;

/**
 * 依 UTF-8 字元邊界截斷字串，確保不超過指定位元組上限。
 */
function truncateUtf8Bytes(value: string, maxBytes: number): string {
  const encoder = new TextEncoder();
  if (encoder.encode(value).length <= maxBytes) {
    return value;
  }
  let result = "";
  let usedBytes = 0;
  for (const ch of value) {
    const chBytes = encoder.encode(ch).length;
    if (usedBytes + chBytes > maxBytes) {
      break;
    }
    result += ch;
    usedBytes += chBytes;
  }
  return result.trimEnd();
}

/**
 * 移除控制與 Bidi 字元並修剪前後空白，作為接收器名稱正規化前處理。
 */
export function normalizeReceiverNameInput(name: string): string {
  return name.replace(CONTROL_OR_BIDI_PATTERN, "").trim();
}

/**
 * 計算正規化後接收器名稱的 UTF-8 位元組數。
 */
export function receiverNameUtf8Bytes(name: string): number {
  return new TextEncoder().encode(normalizeReceiverNameInput(name)).length;
}

/**
 * 驗證 AirPlay PIN 設定於固定 PIN 模式下是否為精確 4 位 ASCII 數字。
 */
export function isAirplayPinValid(mode: AirplayAuthMode, pin: string): boolean {
  if (mode !== "fixedPin") {
    return true;
  }
  return /^\d{4}$/.test(pin.trim());
}

/**
 * 依據 AirPlay 狀態、接收器廣播名稱與 PIN 驗證模式組合儀表板提示說明文字。
 *
 * @param status AirPlay 引擎目前狀態。
 * @param receiverName 目前設定的接收器名稱。
 * @param airplayAuthMode 目前設定的 AirPlay 認證模式。
 * @returns 對應狀態的使用指引或環境需求提示。
 */
export function airplayHint(
  status: EngineStatus,
  receiverName: string,
  airplayAuthMode: AirplayAuthMode = "none",
): string {
  if (status === "starting") {
    return "正在等待 Bonjour 廣播；接收器出現在 iPhone 清單後即可連線。";
  }
  if (status === "running") {
    const base = `請從 iPhone 的螢幕鏡像清單選擇「${receiverName}」。`;
    if (airplayAuthMode === "randomPin") {
      return `${base} 連線時請輸入診斷記錄顯示的一次性 PIN。`;
    }
    if (airplayAuthMode === "fixedPin") {
      return `${base} 連線時請輸入設定的 4 位數字 PIN。`;
    }
    return base;
  }
  return "需要安裝 UxPlay 與影音執行環境";
}

/**
 * 驗證使用者輸入的接收器名稱在清理控制／Bidi 字元並修剪前後空白後非空且不超過 63 UTF-8 位元組。
 *
 * @param name 待驗證的接收器名稱字串。
 * @returns 若符合長度與內容要求則回傳 `true`。
 */
export function isReceiverNameValid(name: string): boolean {
  const normalized = normalizeReceiverNameInput(name);
  if (normalized.length === 0) {
    return false;
  }
  return receiverNameUtf8Bytes(normalized) <= MAX_RECEIVER_NAME_UTF8_BYTES;
}

/**
 * 清理接收器設定物件，確保 `receiverName` 安全且不超過 63 UTF-8 位元組，並正規化 AirPlay PIN 與 Android 安全選項。
 *
 * @param settings 原始接收器設定物件。
 * @returns 正規化後的新 `ReceiverSettings` 物件（不突變輸入參數）。
 */
export function sanitizeReceiverSettings(settings: ReceiverSettings): ReceiverSettings {
  const normalizedName = normalizeReceiverNameInput(settings.receiverName);
  const boundedName = truncateUtf8Bytes(normalizedName, MAX_RECEIVER_NAME_UTF8_BYTES);
  const trimmedPin = settings.airplayPin.trim();
  const mode = settings.airplayAuthMode ?? "none";
  let nextMode: AirplayAuthMode = "none";
  let nextPin = "";
  if (mode === "randomPin") {
    nextMode = "randomPin";
  } else if (mode === "fixedPin") {
    if (/^\d{4}$/.test(trimmedPin)) {
      nextMode = "fixedPin";
      nextPin = trimmedPin;
    } else {
      nextMode = "randomPin";
    }
  }
  return {
    ...settings,
    receiverName: boundedName.length > 0 ? boundedName : DEFAULT_RECEIVER_NAME,
    airplayAuthMode: nextMode,
    airplayPin: nextPin,
    androidNoClipboardSync: settings.androidNoClipboardSync ?? true,
    androidReadOnly: settings.androidReadOnly ?? false,
  };
}
