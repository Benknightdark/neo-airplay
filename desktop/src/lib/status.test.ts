import { describe, expect, it } from "vitest";
import { engineLabel, localTime, logLabel } from "./status";

describe("狀態顯示文字", () => {
  it("將引擎狀態轉成介面文字", () => {
    expect(engineLabel("dependencyMissing")).toBe("缺少元件");
    expect(engineLabel("running")).toBe("執行中");
  });

  it("將記錄層級轉成介面文字", () => {
    expect(logLabel("warning")).toBe("注意");
  });

  it("對無效時間提供穩定的空值文字", () => {
    expect(localTime("invalid")).toBe("--:--:--");
  });
});
