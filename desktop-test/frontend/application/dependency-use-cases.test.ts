/**
 * @file dependency-use-cases.test.ts
 * @description 驗證投影工具依賴應用層用例與 Gateway 協作及錯誤封裝的單元測試。
 */

import { describe, expect, it, vi } from "vitest";
import { createDependencyUseCases } from "../../../desktop/src/application";
import { createFakeDependencyGateway } from "../fixtures/fake-gateways";

/** 驗證投影工具依賴應用層用例（DependencyUseCases）行為。 */
describe("DependencyUseCases 應用層用例", () => {
  /** 驗證查詢、安裝、升級修復與讀取安裝記錄都會正確委派給 Gateway。 */
  it("協調工具狀態查詢、安裝、升級管理與安裝記錄讀取", async () => {
    const gateway = createFakeDependencyGateway();
    const useCases = createDependencyUseCases(gateway);

    const status = await useCases.getDependencies();
    expect(status.isSuccess).toBe(true);

    const installed = await useCases.installDependencies();
    expect(installed.isSuccess).toBe(true);

    const upgraded = await useCases.manageDependencies("upgrade");
    expect(upgraded.isSuccess).toBe(true);
    expect(gateway.manageDependencies).toHaveBeenCalledWith("upgrade");

    const log = await useCases.readInstallLog();
    expect(log.isSuccess).toBe(true);
    expect(log.value).toContain("install started");
  });

  /** 驗證依賴 Gateway 拋出例外時會安全封裝為 `Result.failure`。 */
  it("將 Gateway 拒絕錯誤轉為 Result.failure", async () => {
    const gateway = createFakeDependencyGateway({
      readInstallLog: vi.fn().mockRejectedValue(new Error("尚無安裝記錄。")),
    });
    const useCases = createDependencyUseCases(gateway);

    const log = await useCases.readInstallLog();
    expect(log.isSuccess).toBe(false);
    expect(log.error).toBe("尚無安裝記錄。");
  });
});
