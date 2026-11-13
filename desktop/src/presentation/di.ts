/**
 * @file 表現層相依注入（DI）容器與 InjectionKey 定義，負責組裝並提供應用層使用案例。
 */
import { inject, type InjectionKey } from "vue";
import {
  createDependencyUseCases,
  createReceiverUseCases,
  type DependencyGateway,
  type DependencyUseCases,
  type ReceiverGateway,
  type ReceiverUseCases,
} from "../application";
import {
  createTauriDependencyGateway,
  createTauriReceiverGateway,
} from "../infrastructure";

/** 用於 Vue `provide`/`inject` 注入自訂 `ReceiverGateway`（特別是測試替身）的鍵值。 */
export const RECEIVER_GATEWAY_KEY: InjectionKey<ReceiverGateway> = Symbol("ReceiverGateway");
/** 用於 Vue `provide`/`inject` 注入自訂 `DependencyGateway`（特別是測試替身）的鍵值。 */
export const DEPENDENCY_GATEWAY_KEY: InjectionKey<DependencyGateway> = Symbol("DependencyGateway");

/**
 * 解析並建立接收器管理使用案例；優先採用顯式傳入參數，其次取用 Vue DI，預設退回 Tauri 閘道。
 *
 * @param gateway 選擇性覆寫預設注入來源的 `ReceiverGateway` 實例。
 * @returns 綁定對應閘道的 `ReceiverUseCases` 實例。
 */
export function resolveReceiverUseCases(gateway?: ReceiverGateway): ReceiverUseCases {
  const resolved = gateway ?? inject(RECEIVER_GATEWAY_KEY, undefined) ?? createTauriReceiverGateway();
  return createReceiverUseCases(resolved);
}

/**
 * 解析並建立依賴工具管理使用案例；優先採用顯式傳入參數，其次取用 Vue DI，預設退回 Tauri 閘道。
 *
 * @param gateway 選擇性覆寫預設注入來源的 `DependencyGateway` 實例。
 * @returns 綁定對應閘道的 `DependencyUseCases` 實例。
 */
export function resolveDependencyUseCases(gateway?: DependencyGateway): DependencyUseCases {
  const resolved =
    gateway ?? inject(DEPENDENCY_GATEWAY_KEY, undefined) ?? createTauriDependencyGateway();
  return createDependencyUseCases(resolved);
}
