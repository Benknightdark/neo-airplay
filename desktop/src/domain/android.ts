/**
 * @file Android 裝置模型、就緒裝置篩選與異常狀態診斷的領域純函式。
 */

/** 透過 ADB 偵測到的 Android 裝置授權與連線狀態。 */
export type AndroidDeviceStatus = "ready" | "unauthorized" | "offline" | "unknown";

/** 單一偵測到的 Android 裝置資料模型。 */
export interface AndroidDevice {
  /** ADB 裝置序號或連線位址。 */
  serial: string;
  /** 解析後的裝置型號顯示名稱。 */
  model: string;
  /** 裝置目前的 ADB 授權／連線狀態。 */
  status: AndroidDeviceStatus;
}

/**
 * 從全部 Android 裝置清單中篩選出狀態為 `ready` 的可用裝置。
 *
 * @param devices 後端回報的完整裝置清單。
 * @returns 所有可供啟動 `scrcpy` 投影的裝置陣列。
 */
export function filterReadyDevices(devices: readonly AndroidDevice[]): AndroidDevice[] {
  return devices.filter((device) => device.status === "ready");
}

/**
 * 在已就緒裝置清單中維持原選取序號，若原序號已離線則自動切換為第一台可用裝置。
 *
 * @param readyDevices 已授權且可投影的裝置清單。
 * @param currentSerial 目前介面選取中的裝置序號。
 * @returns 應選取的裝置序號；若無可用裝置則回傳空字串。
 */
export function resolveSelectedSerial(
  readyDevices: readonly AndroidDevice[],
  currentSerial: string,
): string {
  if (readyDevices.some((device) => device.serial === currentSerial)) {
    return currentSerial;
  }
  return readyDevices[0]?.serial ?? "";
}

/**
 * 當畫面中沒有任何 `ready` 裝置時，根據裝置清單狀態組合具體的排障指引訊息。
 *
 * @param devices 後端回報的全部偵測裝置列表（可能為空或全部未授權／離線）。
 * @returns 顯示於 Android 投影卡片的繁體中文指引文字。
 */
export function diagnoseUnavailableDevices(devices: readonly AndroidDevice[]): string {
  if (devices.length === 0) {
    return "尚未找到裝置。請連接手機並開啟 USB 偵錯。";
  }

  if (devices.every((device) => device.status === "unauthorized")) {
    return "找到未授權的裝置。請在 Android 裝置上確認並允許此電腦的偵錯連線。";
  }

  if (devices.every((device) => device.status === "offline")) {
    return "裝置目前離線，請確認手機已連線並重新整理裝置清單。";
  }

  if (devices.every((device) => device.status === "unknown")) {
    return "無法辨識裝置狀態，請重新整理裝置清單或檢查 ADB 連線。";
  }

  return "找到裝置，但目前沒有可用裝置。請確認裝置連線或偵錯授權後重新整理。";
}
