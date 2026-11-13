/**
 * @file fake-gateways.ts
 * @description 提供應用層 Gateway 介面的模擬實作工廠（Fake Gateways），供用例、Composable 與元件測試注入。
 */

import { vi } from "vitest";
import type { DependencyGateway, ReceiverGateway } from "../../../desktop/src/application";
import { createDashboardSnapshot, createDependencySnapshot } from "./builders";

/**
 * 建立接收器 Gateway 的 Vitest 模擬物件。
 *
 * @param overrides 欲替換的特定 Gateway 方法模擬函式。
 * @returns 具備預設非同步回傳值的 {@link ReceiverGateway} 測試替身。
 */
export function createFakeReceiverGateway(
  overrides: Partial<ReceiverGateway> = {},
): ReceiverGateway {
  const defaultDashboard = createDashboardSnapshot();
  return {
    getDashboard: vi.fn().mockResolvedValue(defaultDashboard),
    saveSettings: vi.fn().mockImplementation(async (settings) => settings),
    startAirplay: vi.fn().mockResolvedValue(defaultDashboard.airplay),
    stopAirplay: vi.fn().mockResolvedValue(defaultDashboard.airplay),
    refreshAndroidDevices: vi.fn().mockResolvedValue(defaultDashboard.devices),
    startAndroidMirror: vi.fn().mockResolvedValue(defaultDashboard.android),
    stopAndroidMirror: vi.fn().mockResolvedValue(defaultDashboard.android),
    ...overrides,
  };
}

/**
 * 建立投影工具依賴 Gateway 的 Vitest 模擬物件。
 *
 * @param overrides 欲替換的特定依賴 Gateway 方法模擬函式。
 * @returns 具備預設非同步回傳值的 {@link DependencyGateway} 測試替身。
 */
export function createFakeDependencyGateway(
  overrides: Partial<DependencyGateway> = {},
): DependencyGateway {
  const defaultSnapshot = createDependencySnapshot();
  return {
    getDependencies: vi.fn().mockResolvedValue(defaultSnapshot),
    installDependencies: vi.fn().mockResolvedValue(defaultSnapshot),
    manageDependencies: vi.fn().mockResolvedValue(defaultSnapshot),
    readInstallLog: vi.fn().mockResolvedValue("install started\ninstall finished\n"),
    ...overrides,
  };
}
