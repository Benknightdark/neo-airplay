import { spawn } from "node:child_process";
import assert from "node:assert/strict";
import { access, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { $, expect } from "@wdio/globals";

describe("Neo AirPlay 桌面流程", () => {
  it("啟動後顯示兩種投影服務", async () => {
    await expect($("[data-testid='dashboard-page']")).toBeDisplayed();
    await expect($("[data-testid='dependency-panel']")).not.toBeDisplayed();
    await expect($("[data-testid='airplay-start']")).toBeDisplayed();
    await expect($("[data-testid='airplay-start']")).toBeEnabled();
    await expect($("[data-testid='android-refresh']")).toBeDisplayed();
    await expect($("[data-testid='android-refresh']")).toBeEnabled();
    await expect($("[data-testid='airplay-status']")).toHaveText(expect.stringContaining("未啟動"));
  });

  it("再次啟動時會使用既有執行個體", async () => {
    const appBinaryPath = process.env.NEO_AIRPLAY_E2E_APP_BINARY;
    const profilePath = process.env.NEO_AIRPLAY_E2E_PROFILE;
    assert.ok(appBinaryPath, "缺少 E2E 應用程式路徑。");
    assert.ok(profilePath, "缺少 E2E 使用者資料路徑。");

    const secondInstance = spawn(appBinaryPath, ["--e2e-profile", profilePath], {
      stdio: "ignore",
      windowsHide: true,
    });
    let timeout: ReturnType<typeof setTimeout> | undefined;
    try {
      const result = await Promise.race([
        new Promise<{ code: number | null; signal: NodeJS.Signals | null }>((resolveResult, reject) => {
          secondInstance.once("error", reject);
          secondInstance.once("exit", (code, signal) => resolveResult({ code, signal }));
        }),
        new Promise<null>((resolveTimeout) => {
          timeout = setTimeout(() => resolveTimeout(null), 10_000);
        }),
      ]);
      assert.ok(result, "第二個執行個體沒有在期限內結束。");
      assert.equal(result.code, 0, `第二個執行個體意外結束：${result.signal ?? result.code}`);
    } finally {
      if (timeout !== undefined) clearTimeout(timeout);
      if (secondInstance.exitCode === null && secondInstance.signalCode === null) {
        secondInstance.kill("SIGKILL");
      }
    }
  });

  it("ADB 查詢等待期間仍可操作視窗", async () => {
    const profilePath = process.env.NEO_AIRPLAY_E2E_PROFILE;
    assert.ok(profilePath, "缺少 E2E 使用者資料路徑。");
    const blockPath = join(profilePath, "adb.block");
    const startedPath = join(profilePath, "adb.started");

    await writeFile(blockPath, "wait");
    try {
      await $(`[data-testid='nav-overview']`).click();
      await $(`[data-testid='android-refresh']`).click();
      try {
        await waitForFile(startedPath, 5_000);
      } catch (error) {
        const status = await $(`[data-testid='android-status']`).getText().catch(() => "未知");
        const device = await $(`[data-testid='android-device-select']`).getValue().catch(() => "未知");
        const appError = await $(`[data-testid='app-error']`).getText().catch(() => "");
        throw new Error(`ADB 測試程序未進入等待狀態（${status}，裝置 ${device}）：${appError || String(error)}`);
      }

      const navigation = $(`[data-testid='nav-diagnostics']`).click();
      const responsive = await Promise.race([
        navigation.then(() => true),
        delay(2_000).then(() => false),
      ]);
      assert.ok(responsive, "ADB 查詢期間視窗無法處理導覽操作。");
      await expect($(`[data-testid='diagnostics-page']`)).toBeDisplayed();
    } finally {
      await rm(blockPath, { force: true });
      await rm(startedPath, { force: true });
    }
  });

  it("可在總覽、設定與診斷頁間切換", async () => {
    await $(`[data-testid='nav-settings']`).click();
    await expect($("[data-testid='settings-page']")).toBeDisplayed();
    const receiverName = $("[data-testid='receiver-name']");
    await expect(receiverName).toHaveValue("Neo AirPlay");

    await receiverName.setValue("Meeting Room Receiver");
    await expect(receiverName).toHaveValue("Meeting Room Receiver");
    await $(`[data-testid='settings-save']`).click();
    await expect($(`[data-testid='settings-save']`)).toBeEnabled();
    await expect(receiverName).toHaveValue("Meeting Room Receiver");

    await $(`[data-testid='nav-diagnostics']`).click();
    await expect($("[data-testid='diagnostics-page']")).toBeDisplayed();

    await $(`[data-testid='nav-overview']`).click();
    await expect($("[data-testid='dashboard-page']")).toBeDisplayed();
    await $(`[data-testid='nav-settings']`).click();
    await expect($("[data-testid='receiver-name']")).toHaveValue("Meeting Room Receiver");
  });

  it("可同時執行兩種投影，並分別停止引擎", async () => {
    const airplayStatus = $("[data-testid='airplay-status']");
    const androidStatus = $("[data-testid='android-status']");
    const deviceSelector = $("[data-testid='android-device-select']");

    try {
      await $("[data-testid='nav-overview']").click();
      await $("[data-testid='airplay-start']").click();
      await expect(airplayStatus).toHaveText(expect.stringContaining("執行中"));

      await $("[data-testid='android-refresh']").click();
      await expect(deviceSelector).toHaveValue("neo-e2e-device");
      await expect($("[data-testid='android-start']")).toBeEnabled();
      await $("[data-testid='android-start']").click();
      await expect(androidStatus).toHaveText(expect.stringContaining("執行中"));
      await expect(airplayStatus).toHaveText(expect.stringContaining("執行中"));

      await $("[data-testid='android-stop']").click();
      await expect(androidStatus).toHaveText(expect.stringContaining("未啟動"));
      await expect(airplayStatus).toHaveText(expect.stringContaining("執行中"));

      await $("[data-testid='airplay-stop']").click();
      await expect(airplayStatus).toHaveText(expect.stringContaining("未啟動"));
    } finally {
      const androidStop = $("[data-testid='android-stop']");
      if (await androidStop.isExisting()) await androidStop.click();
      const airplayStop = $("[data-testid='airplay-stop']");
      if (await airplayStop.isExisting()) await airplayStop.click();
    }
  });
});

async function waitForFile(path: string, timeoutMilliseconds: number): Promise<void> {
  const expiresAt = Date.now() + timeoutMilliseconds;
  while (Date.now() < expiresAt) {
    try {
      await access(path);
      return;
    } catch {
      await delay(25);
    }
  }
  throw new Error(`等待測試檔案逾時：${path}`);
}
