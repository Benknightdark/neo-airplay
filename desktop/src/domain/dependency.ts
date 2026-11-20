/**
 * @file 投影依賴工具的狀態型別定義、忙碌／展示判斷與中文標籤轉換邏輯。
 */

/** 使用者可主動觸發的工具維護動作。 */
export type DependencyAction = "upgrade" | "reinstall" | "clear";
/** 工具管理背景作業的操作種類（含自動安裝、單工具換版與手動維護）。 */
export type DependencyOperation = "install" | DependencyAction | "installTool";
/** 工具環境檢查或安裝作業目前的狀態機值。 */
export type DependencyStatus = "checking" | "installing" | "ready" | "failed" | "disabled";
/** 作業系統平台識別。 */
export type PlatformOs = "macos" | "windows" | "linux";
/** 受管理或參考之投影工具識別碼。 */
export type ToolId = "uxplay" | "scrcpy" | "gstreamer";
/** 工具版本控制權限模式。 */
export type VersionControlMode = "selectable" | "packageManager";

/** 單一可選／線上參考版本項目。 */
export interface ToolReleaseOption {
  /** 標準顯示用版本號（或 commit SHA）。 */
  version: string;
  /** 傳遞給安裝來源之實際 ref（tag 或 commit SHA）。 */
  reference: string;
  /** 顯示給使用者的版本標籤。 */
  label: string;
  /** 上游發佈日期（`YYYY-MM-DD`，若有）。 */
  publishedAt?: string | null;
  /** 是否為上游預覽版本。 */
  prerelease: boolean;
  /** 是否為 Neo AirPlay 內建推薦驗證版。 */
  recommended: boolean;
  /** 已知之 SHA-256 校驗碼（若有）。 */
  sha256?: string | null;
}

/** 單一工具之作業系統來源、本機狀態與可選版本資訊。 */
export interface ManagedToolInfo {
  /** 工具識別碼。 */
  id: ToolId;
  /** 工具顯示名稱。 */
  name: string;
  /** 工具用途說明。 */
  summary: string;
  /** 目前 OS 下的安裝來源描述。 */
  sourceLabel: string;
  /** 上游專案網址。 */
  sourceUrl: string;
  /** 版本控制模式。 */
  versionControl: VersionControlMode;
  /** 本機 `engine/` 實測已安裝版本（若有）。 */
  installedVersion?: string | null;
  /** 隨附子工具版本資訊（如隨 scrcpy 同包之 ADB）。 */
  bundledComponents: string[];
  /** 目前 OS 內建推薦版本。 */
  defaultVersion: string;
  /** 目前 OS 內建推薦參照（commit SHA 或 tag）。 */
  defaultReference: string;
  /** 使用者已鎖定之自訂版本覆寫（無則為 `null`/`undefined`）。 */
  selectedVersion?: string | null;
  /** 最近一次線上爬取到之最新上游版本。 */
  latestRemoteVersion?: string | null;
  /** 可供下拉選取的版本清單。 */
  availableVersions: ToolReleaseOption[];
  /** 是否支援手動輸入 SHA-256。 */
  supportsManualSha256: boolean;
  /** 是否會自動透過官方 `SHA256SUMS.txt` 驗證雜湊。 */
  autoVerifiesChecksum: boolean;
  /** 平台／架構專屬補充說明。 */
  note?: string | null;
}

/** 作業系統感知之工具安裝目錄與版本爬取快照。 */
export interface ToolCatalogSnapshot {
  /** 目前執行作業系統。 */
  os: PlatformOs;
  /** CPU 架構名稱。 */
  arch: string;
  /** 人類可讀之平台與架構名稱。 */
  platformLabel: string;
  /** 主要套件管理器描述。 */
  packageManager: string;
  /** 目前 OS 之授權／安裝位置說明。 */
  platformNotice: string;
  /** 僅於高風險套件管理器（如 Linux `pacman`）顯示之警告文字。 */
  platformWarning?: string | null;
  /** 本工作階段是否啟用工具管理。 */
  managementEnabled: boolean;
  /** 最近一次線上爬取時間（RFC 3339）。 */
  remoteFetchedAt?: string | null;
  /** 最近一次爬取失敗或部分失敗訊息。 */
  remoteError?: string | null;
  /** 目前平台對應之工具卡片清單。 */
  tools: ManagedToolInfo[];
}

/** 前端提交安裝／切換單一工具版本之請求資料。 */
export interface ToolInstallRequest {
  /** 目標工具。 */
  tool: ToolId;
  /** 使用者選擇或輸入的版本／tag／commit SHA。 */
  version: string;
  /** 選填 SHA-256 校驗碼。 */
  sha256?: string | null;
  /** 安裝非預設 UxPlay 且未提供 SHA-256 時是否已明確確認。 */
  allowUnverified?: boolean;
}

/** 從後端取得的依賴工具狀態完整快照。 */
export interface DependencySnapshot {
  /** 目前狀態機值。 */
  status: DependencyStatus;
  /** 正在執行或最近完成的作業種類，無則為 `null`。 */
  operation: DependencyOperation | null;
  /** 顯示給使用者的狀態說明訊息。 */
  message: string;
  /** 偵測後仍缺少的工具名稱陣列。 */
  missing: string[];
  /** 安裝記錄檔在磁碟上的完整路徑。 */
  logPath: string;
}

/**
 * 判斷依賴工具目前是否正在請求作業、檢查中或背景安裝中（用於鎖定操作按鈕）。
 *
 * @param snapshot 目前的依賴狀態快照。
 * @param requesting 前端是否剛送出請求且尚未收到回應。
 * @returns 處於任一忙碌狀態時回傳 `true`。
 */
export function isDependencyInstalling(
  snapshot: DependencySnapshot | null,
  requesting: boolean,
): boolean {
  return requesting || snapshot?.status === "checking" || snapshot?.status === "installing";
}

/**
 * 判斷目前工作階段是否無法存取或已停用工具管理功能。
 *
 * @param snapshot 目前的依賴狀態快照。
 * @returns 快照尚未載入或狀態為 `"disabled"` 時回傳 `true`。
 */
export function isDependencyUnavailable(snapshot: DependencySnapshot | null): boolean {
  return !snapshot || snapshot.status === "disabled";
}

/**
 * 決定儀表板上方的依賴狀態提示橫幅（`DependencyPanel`）是否需要顯示。
 *
 * @param snapshot 目前的依賴狀態快照。
 * @param installing 目前是否正在執行安裝或檢查流程。
 * @param error 是否有尚未清除的錯誤訊息。
 * @returns 只要正在安裝、有錯誤或非停用狀態即回傳 `true`。
 */
export function shouldDisplayDependencyPanel(
  snapshot: DependencySnapshot | null,
  installing: boolean,
  error: string,
): boolean {
  return installing || error.length > 0 || (snapshot !== null && snapshot.status !== "disabled");
}

/**
 * 將操作類型轉換為繁體中文標題標籤。
 *
 * @param operation 工具作業類型（可為 `null` 或 `undefined`）。
 * @returns 對應的中文操作名稱。
 */
export function dependencyOperationLabel(operation: DependencyOperation | null | undefined): string {
  switch (operation) {
    case "upgrade":
      return "升級工具";
    case "reinstall":
      return "清除並重新安裝";
    case "clear":
      return "清除工具";
    case "installTool":
      return "安裝指定工具版本";
    case "install":
      return "準備工具";
    default:
      return "工具狀態";
  }
}

/**
 * 將依賴狀態碼轉換為繁體中文狀態標籤。
 *
 * @param status 目前狀態碼（可為 `null` 或 `undefined`）。
 * @returns 對應的中文狀態標籤文字。
 */
export function dependencyStatusLabel(status: DependencyStatus | null | undefined): string {
  switch (status) {
    case "checking":
      return "檢查中";
    case "installing":
      return "處理中";
    case "ready":
      return "已就緒";
    case "failed":
      return "需要處理";
    case "disabled":
      return "已停用";
    default:
      return "讀取中";
  }
}

/**
 * 將平台識別碼轉換為顯示用作業系統名稱。
 *
 * @param os 作業系統識別碼。
 * @returns 顯示名稱（`macOS`、`Windows` 或 `Linux`）。
 */
export function platformBadgeLabel(os: PlatformOs): string {
  switch (os) {
    case "macos":
      return "macOS";
    case "windows":
      return "Windows";
    case "linux":
      return "Linux";
  }
}

/**
 * 驗證使用者輸入的工具版本號（或 UxPlay 40 碼 Git commit SHA）是否符合白名單格式。
 *
 * @param tool 目標工具識別碼。
 * @param value 使用者輸入字串。
 * @returns 驗證失敗時回傳中文錯誤訊息；合法時回傳 `null`。
 */
export function validateToolVersionInput(tool: ToolId, value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return "請選擇或輸入欲安裝的版本號。";
  }
  if (tool === "gstreamer") {
    return "GStreamer 由作業系統套件管理器維護。";
  }
  if (tool === "scrcpy") {
    if (!/^v?[0-9]+\.[0-9]+(\.[0-9]+)?$/i.test(trimmed)) {
      return "scrcpy 版本格式無效，請輸入例如 5.0.1 或 v4.1。";
    }
    return null;
  }
  if (/^[0-9a-fA-F]{40}$/.test(trimmed) || /^v?[0-9]+\.[0-9]+(\.[0-9]+)*$/i.test(trimmed)) {
    return null;
  }
  return "UxPlay 版本格式無效，請輸入版本號（如 1.73.7）或完整 40 碼 commit SHA。";
}

/**
 * 驗證選填之 SHA-256 校驗碼輸入格式。
 *
 * @param value 使用者輸入之雜湊字串（可為空）。
 * @returns 留空或有效 64 碼十六進位回傳 `null`；否則回傳錯誤訊息。
 */
export function validateOptionalSha256Input(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  return /^[0-9a-fA-F]{64}$/.test(trimmed)
    ? null
    : "SHA-256 必須為精確 64 碼十六進位字元（或留空）。";
}

/**
 * 判斷安裝特定工具版本前是否須顯示未附 SHA-256 確認視窗（Option 1A）。
 *
 * @param tool 目標工具目錄資訊。
 * @param version 欲安裝之版本或 commit 字串。
 * @param sha256 使用者填入之 SHA-256 字串。
 * @returns 當為非內建推薦 UxPlay 且未提供雜湊值時回傳 `true`。
 */
export function requiresUnverifiedConfirmation(
  tool: ManagedToolInfo,
  version: string,
  sha256: string,
): boolean {
  if (tool.id !== "uxplay" || sha256.trim().length > 0) {
    return false;
  }
  const trimmed = version.trim();
  const normalized = trimmed.replace(/^[vV]/, "");
  const defaultNormalized = tool.defaultVersion.replace(/^[vV]/, "");
  if (
    normalized === defaultNormalized ||
    trimmed.toLowerCase() === tool.defaultReference.toLowerCase()
  ) {
    return false;
  }
  const matchedOption = tool.availableVersions.find(
    (option) =>
      option.version === normalized ||
      option.reference.toLowerCase() === trimmed.toLowerCase(),
  );
  return !matchedOption?.sha256;
}
