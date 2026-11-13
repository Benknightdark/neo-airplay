/**
 * @file 前端基礎設施層匯出進入點，提供 Tauri IPC 閘道轉接器工廠。
 */
export { createTauriDependencyGateway } from "./tauri/dependency-gateway";
export { createTauriReceiverGateway } from "./tauri/receiver-gateway";
