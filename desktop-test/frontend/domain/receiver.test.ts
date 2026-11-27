/**
 * @file receiver.test.ts
 * @description 驗證接收器設定驗證、名稱正規化與 AirPlay 引擎狀態提示計算的領域單元測試。
 */

import { describe, expect, it } from "vitest";
import {
  airplayButtonLabel,
  airplayHint,
  engineLabel,
  isAirplayActive,
  isAirplayPinValid,
  isReceiverNameValid,
  sanitizeReceiverSettings,
} from "../../../desktop/src/domain";
import { createReceiverSettings } from "../fixtures/builders";

/** 驗證接收器核心領域規則與輸入驗證邏輯。 */
describe("接收器領域規則", () => {
  /** 驗證運行中／停用中狀態判斷、按鈕文字與提示文字轉換是否正確。 */
  it("正確判斷 AirPlay 啟用狀態、按鈕文字與提示", () => {
    expect(isAirplayActive("stopped")).toBe(false);
    expect(isAirplayActive("starting")).toBe(true);
    expect(isAirplayActive("running")).toBe(true);
    expect(isAirplayActive("stopping")).toBe(true);

    expect(airplayButtonLabel("starting")).toBe("取消啟動");
    expect(airplayButtonLabel("stopping")).toBe("正在停止…");
    expect(airplayButtonLabel("running")).toBe("停止接收");

    expect(airplayHint("starting", "Studio")).toContain("Bonjour 廣播");
    expect(airplayHint("running", "Studio")).toContain("Studio");
    expect(airplayHint("running", "Studio", "randomPin")).toContain("一次性 PIN");
    expect(airplayHint("running", "Studio", "fixedPin")).toContain("4 位數字 PIN");
    expect(airplayHint("stopped", "Studio")).toContain("UxPlay");
    expect(engineLabel("faulted")).toBe("發生錯誤");
  });

  /** 驗證空名稱與過長名稱會回傳錯誤，合法名稱則會去除前後空白並執行安全性正規化。 */
  it("驗證並正規化接收器設定名稱與安全選項", () => {
    expect(isReceiverNameValid("   ")).toBe(false);
    expect(isReceiverNameValid("\u202E\u0007   ")).toBe(false);
    expect(isReceiverNameValid("  Studio  ")).toBe(true);
    expect(isReceiverNameValid("a".repeat(64))).toBe(false);
    expect(isReceiverNameValid("測試".repeat(11))).toBe(false);

    expect(isAirplayPinValid("none", "")).toBe(true);
    expect(isAirplayPinValid("randomPin", "")).toBe(true);
    expect(isAirplayPinValid("fixedPin", "123")).toBe(false);
    expect(isAirplayPinValid("fixedPin", "abcd")).toBe(false);
    expect(isAirplayPinValid("fixedPin", " 0429 ")).toBe(true);

    expect(
      sanitizeReceiverSettings(createReceiverSettings({ receiverName: "  \u202EStudio\u0000  " })).receiverName,
    ).toBe("Studio");
    expect(
      sanitizeReceiverSettings(createReceiverSettings({ receiverName: "   " })).receiverName,
    ).toBe("Neo AirPlay");

    const invalidPinNormalized = sanitizeReceiverSettings(
      createReceiverSettings({
        airplayAuthMode: "fixedPin",
        airplayPin: "12a",
      }),
    );
    expect(invalidPinNormalized.airplayAuthMode).toBe("randomPin");
    expect(invalidPinNormalized.airplayPin).toBe("");

    const validPinNormalized = sanitizeReceiverSettings(
      createReceiverSettings({
        airplayAuthMode: "fixedPin",
        airplayPin: " 1234 ",
      }),
    );
    expect(validPinNormalized.airplayAuthMode).toBe("fixedPin");
    expect(validPinNormalized.airplayPin).toBe("1234");
  });
});
