/**
 * @file receiver-use-cases.test.ts
 * @description 驗證接收器應用層用例在輸入驗證、正規化、Gateway 委派與例外轉換上的單元測試。
 */

import { describe, expect, it, vi } from "vitest";
import { createReceiverUseCases } from "../../../desktop/src/application";
import { createReceiverSettings } from "../fixtures/builders";
import { createFakeReceiverGateway } from "../fixtures/fake-gateways";

/** 驗證接收器應用層用例（ReceiverUseCases）行為。 */
describe("ReceiverUseCases 應用層用例", () => {
  /** 驗證儲存設定失敗時不會誤觸 Gateway，合法時則先修剪空白再送出。 */
  it("儲存設定前驗證名稱並執行正規化，失敗時不呼叫 Gateway", async () => {
    const gateway = createFakeReceiverGateway();
    const useCases = createReceiverUseCases(gateway);

    const invalid = await useCases.saveSettings(
      createReceiverSettings({ receiverName: "   " }),
    );
    expect(invalid.isSuccess).toBe(false);
    expect(invalid.errorType).toBe("Validation");
    expect(gateway.saveSettings).not.toHaveBeenCalled();

    const valid = await useCases.saveSettings(
      createReceiverSettings({ receiverName: "  Living Room  " }),
    );
    expect(valid.isSuccess).toBe(true);
    expect(valid.value?.receiverName).toBe("Living Room");
    expect(gateway.saveSettings).toHaveBeenCalledWith(
      expect.objectContaining({ receiverName: "Living Room" }),
    );
  });

  /** 驗證空序號會直接回傳驗證失敗，且 Gateway 非同步錯誤會被包成 `Result.failure`。 */
  it("啟動 Android 投影前驗證序號，並於 Gateway 拋出錯誤時回傳 Result.failure", async () => {
    const gateway = createFakeReceiverGateway({
      startAirplay: vi.fn().mockRejectedValue(new Error("uxplay missing")),
    });
    const useCases = createReceiverUseCases(gateway);

    const invalidSerial = await useCases.startAndroidMirror("   ");
    expect(invalidSerial.isSuccess).toBe(false);
    expect(invalidSerial.errorType).toBe("Validation");
    expect(gateway.startAndroidMirror).not.toHaveBeenCalled();

    const validSerial = await useCases.startAndroidMirror(" phone-1 ");
    expect(validSerial.isSuccess).toBe(true);
    expect(gateway.startAndroidMirror).toHaveBeenCalledWith("phone-1");

    const failedAirplay = await useCases.startAirplay();
    expect(failedAirplay.isSuccess).toBe(false);
    expect(failedAirplay.error).toBe("uxplay missing");
  });
});
