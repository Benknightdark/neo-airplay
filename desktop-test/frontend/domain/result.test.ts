/**
 * @file result.test.ts
 * @description 驗證前端 `Result` 模式與未知例外正規化工具函式的單元測試。
 */

import { describe, expect, it } from "vitest";
import { normalizeErrorMessage, Result } from "../../../desktop/src/domain";

/** 驗證 Result 物件建構與錯誤訊息正規化行為。 */
describe("Result 模式", () => {
  /** 驗證成功與失敗工廠方法會保留正確的判定旗標、資料與錯誤字串。 */
  it("建立成功與失敗結果時保留對應的狀態與欄位", () => {
    const ok = Result.success(42);
    expect(ok.isSuccess).toBe(true);
    expect(ok.value).toBe(42);
    expect(ok.error).toBeNull();
    expect(ok.errorType).toBe("None");

    const validation = Result.validation("名稱不可為空");
    expect(validation.isSuccess).toBe(false);
    expect(validation.value).toBeNull();
    expect(validation.error).toBe("名稱不可為空");
    expect(validation.errorType).toBe("Validation");
  });

  /** 驗證 Error 實例、純字串與非標準拋出值皆可正規化為可讀訊息。 */
  it("將未知例外統一正規化為訊息字串", () => {
    expect(normalizeErrorMessage(new Error("boom"))).toBe("boom");
    expect(normalizeErrorMessage("raw error")).toBe("raw error");
  });
});
