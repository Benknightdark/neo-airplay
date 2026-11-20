/**
 * @file dependency.test.ts
 * @description 驗證投影工具依賴狀態判定與操作標籤映射的領域單元測試。
 */

import { describe, expect, it } from "vitest";
import {
  dependencyOperationLabel,
  dependencyStatusLabel,
  isDependencyInstalling,
  isDependencyUnavailable,
  requiresUnverifiedConfirmation,
  shouldDisplayDependencyPanel,
  validateOptionalSha256Input,
  validateToolVersionInput,
} from "../../../desktop/src/domain";
import {
  createDependencySnapshot,
  createManagedToolInfo,
} from "../fixtures/builders";

/** 驗證投影工具依賴管理相關的領域規則。 */
describe("投影工具依賴領域規則", () => {
  /** 驗證檢查／安裝／升級忙線旗標與工具列按鈕停用條件。 */
  it("正確判斷安裝忙線狀態與停用狀態", () => {
    const ready = createDependencySnapshot({ status: "ready" });
    const checking = createDependencySnapshot({ status: "checking" });
    const disabled = createDependencySnapshot({ status: "disabled" });

    expect(isDependencyInstalling(ready, false)).toBe(false);
    expect(isDependencyInstalling(ready, true)).toBe(true);
    expect(isDependencyInstalling(checking, false)).toBe(true);

    expect(isDependencyUnavailable(null)).toBe(true);
    expect(isDependencyUnavailable(disabled)).toBe(true);
    expect(isDependencyUnavailable(ready)).toBe(false);

    expect(shouldDisplayDependencyPanel(disabled, false, "")).toBe(false);
    expect(shouldDisplayDependencyPanel(disabled, false, "IPC 失敗")).toBe(true);
    expect(shouldDisplayDependencyPanel(ready, false, "")).toBe(true);
  });

  /** 驗證不同依賴狀態與操作模式對應的繁體中文標籤文字。 */
  it("回傳正確的操作與狀態標籤", () => {
    expect(dependencyOperationLabel("upgrade")).toBe("升級工具");
    expect(dependencyOperationLabel("reinstall")).toBe("清除並重新安裝");
    expect(dependencyOperationLabel("installTool")).toBe("安裝指定工具版本");
    expect(dependencyOperationLabel("install")).toBe("準備工具");
    expect(dependencyOperationLabel(null)).toBe("工具狀態");

    expect(dependencyStatusLabel("ready")).toBe("已就緒");
    expect(dependencyStatusLabel("failed")).toBe("需要處理");
    expect(dependencyStatusLabel(undefined)).toBe("讀取中");
  });

  /** 驗證手動版本輸入格式檢核、SHA-256 驗證與非預設 UxPlay 確認視窗條件（Option 1A）。 */
  it("驗證版本格式、SHA-256 格式與 UxPlay 未附雜湊確認條件", () => {
    expect(validateToolVersionInput("scrcpy", "v5.0.1")).toBeNull();
    expect(validateToolVersionInput("scrcpy", "4.1")).toBeNull();
    expect(validateToolVersionInput("scrcpy", "5.0.1;id")).not.toBeNull();

    expect(validateToolVersionInput("uxplay", "1.73.6")).toBeNull();
    expect(validateToolVersionInput("uxplay", "a".repeat(40))).toBeNull();
    expect(validateToolVersionInput("uxplay", "../bad")).not.toBeNull();

    expect(validateOptionalSha256Input("")).toBeNull();
    expect(validateOptionalSha256Input("f".repeat(64))).toBeNull();
    expect(validateOptionalSha256Input("deadbeef")).not.toBeNull();

    const uxplayTool = createManagedToolInfo({
      id: "uxplay",
      defaultVersion: "1.73.7",
      defaultReference: "df67c212a433cf6dda3676dd40c097900d24e645",
      availableVersions: [],
    });
    expect(requiresUnverifiedConfirmation(uxplayTool, "1.73.7", "")).toBe(false);
    expect(requiresUnverifiedConfirmation(uxplayTool, "1.73.6", "")).toBe(true);
    expect(requiresUnverifiedConfirmation(uxplayTool, "1.73.6", "f".repeat(64))).toBe(false);
  });
});
