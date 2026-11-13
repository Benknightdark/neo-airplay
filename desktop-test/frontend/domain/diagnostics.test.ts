/**
 * @file diagnostics.test.ts
 * @description 驗證診斷系統記錄篩選、倒序排列、時間格式化與層級標籤轉換的領域單元測試。
 */

import { describe, expect, it } from "vitest";
import { filterAndReverseLogs, localTime, logLabel } from "../../../desktop/src/domain";
import { createLogEntry } from "../fixtures/builders";

/** 驗證診斷記錄領域規則。 */
describe("診斷記錄領域規則", () => {
  /** 驗證可依記錄層級篩選並以最新時間優先倒序輸出。 */
  it("支援依層級篩選並以最新優先倒序排列", () => {
    const logs = [
      createLogEntry({ timestamp: "2026-11-13T10:00:00Z", level: "information", message: "first" }),
      createLogEntry({ timestamp: "2026-11-13T10:01:00Z", level: "warning", message: "second" }),
      createLogEntry({ timestamp: "2026-11-13T10:02:00Z", level: "error", message: "third" }),
    ];

    expect(filterAndReverseLogs(logs, "all").map((entry) => entry.message)).toEqual([
      "third",
      "second",
      "first",
    ]);
    expect(filterAndReverseLogs(logs, "warning").map((entry) => entry.message)).toEqual([
      "second",
    ]);
  });

  /** 驗證記錄層級代碼轉短標籤與無效時間字串的容錯回傳。 */
  it("轉換記錄層級與無效時間字串", () => {
    expect(logLabel("information")).toBe("資訊");
    expect(logLabel("warning")).toBe("注意");
    expect(logLabel("error")).toBe("錯誤");
    expect(localTime("invalid")).toBe("--:--:--");
  });
});
