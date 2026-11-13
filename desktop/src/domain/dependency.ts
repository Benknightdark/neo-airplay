/**
 * @file 投影依賴工具的狀態型別定義、忙碌／展示判斷與中文標籤轉換邏輯。
 */

/** 使用者可主動觸發的工具維護動作。 */
export type DependencyAction = "upgrade" | "reinstall";
/** 工具管理背景作業的操作種類（含自動安裝與手動維護）。 */
export type DependencyOperation = "install" | DependencyAction;
/** 工具環境檢查或安裝作業目前的狀態機值。 */
export type DependencyStatus = "checking" | "installing" | "ready" | "failed" | "disabled";

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
