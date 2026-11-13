/**
 * @file android.test.ts
 * @description 驗證 Android 裝置清單篩選、預設序號保留與不可用狀態提示的領域單元測試。
 */

import { describe, expect, it } from "vitest";
import {
  diagnoseUnavailableDevices,
  filterReadyDevices,
  resolveSelectedSerial,
} from "../../../desktop/src/domain";
import { createAndroidDevice } from "../fixtures/builders";

/** 驗證 Android 裝置領域規則與裝置選擇邏輯。 */
describe("Android 裝置領域規則", () => {
  /** 驗證僅保留 `ready` 狀態裝置，且在原選取失效時改選首台可用裝置。 */
  it("只保留已授權裝置並維持有效的選取序號", () => {
    const readyA = createAndroidDevice({ serial: "a", status: "ready" });
    const readyB = createAndroidDevice({ serial: "b", status: "ready" });
    const unauth = createAndroidDevice({ serial: "c", status: "unauthorized" });

    const ready = filterReadyDevices([readyA, unauth, readyB]);
    expect(ready.map((d) => d.serial)).toEqual(["a", "b"]);
    expect(resolveSelectedSerial(ready, "b")).toBe("b");
    expect(resolveSelectedSerial(ready, "c")).toBe("a");
    expect(resolveSelectedSerial([], "a")).toBe("");
  });

  /** 驗證裝置處於 `unauthorized`、`offline` 等異常狀態時的提示訊息。 */
  it("依裝置不可用狀態產生準確的排除提示", () => {
    expect(diagnoseUnavailableDevices([])).toContain("尚未找到裝置");
    expect(
      diagnoseUnavailableDevices([createAndroidDevice({ status: "unauthorized" })]),
    ).toContain("未授權");
    expect(
      diagnoseUnavailableDevices([createAndroidDevice({ status: "offline" })]),
    ).toContain("離線");
    expect(
      diagnoseUnavailableDevices([createAndroidDevice({ status: "unknown" })]),
    ).toContain("無法辨識");
    expect(
      diagnoseUnavailableDevices([
        createAndroidDevice({ serial: "1", status: "offline" }),
        createAndroidDevice({ serial: "2", status: "unauthorized" }),
      ]),
    ).toContain("找到裝置，但目前沒有可用裝置");
  });
});
