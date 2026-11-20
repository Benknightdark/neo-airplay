import { invoke } from "@tauri-apps/api/core";
import type {
  AndroidDevice,
  DashboardSnapshot,
  DependencyAction,
  DependencySnapshot,
  EngineSnapshot,
  ReceiverSettings,
  ToolCatalogSnapshot,
  ToolInstallRequest,
} from "./types";

export const getDependencies = (): Promise<DependencySnapshot> => invoke("get_dependencies");
export const installDependencies = (): Promise<DependencySnapshot> => invoke("install_dependencies");
export const manageDependencies = (action: DependencyAction): Promise<DependencySnapshot> => invoke("manage_dependencies", { action });
export const readInstallLog = (): Promise<string> => invoke("read_install_log");
export const getToolCatalog = (): Promise<ToolCatalogSnapshot> => invoke("get_tool_catalog");
export const fetchToolVersions = (): Promise<ToolCatalogSnapshot> => invoke("fetch_tool_versions");
export const installToolVersion = (request: ToolInstallRequest): Promise<DependencySnapshot> => invoke("install_tool_version", { request });
export const resetToolVersions = (): Promise<ToolCatalogSnapshot> => invoke("reset_tool_versions");

export const getDashboard = (): Promise<DashboardSnapshot> => invoke("get_dashboard");
export const saveSettings = (settings: ReceiverSettings): Promise<ReceiverSettings> => invoke("save_settings", { settings });
export const startAirplay = (): Promise<EngineSnapshot> => invoke("start_airplay");
export const stopAirplay = (): Promise<EngineSnapshot> => invoke("stop_airplay");
export const refreshAndroidDevices = (): Promise<AndroidDevice[]> => invoke("refresh_android_devices");
export const startAndroidMirror = (serial: string): Promise<EngineSnapshot> => invoke("start_android_mirror", { serial });
export const stopAndroidMirror = (): Promise<EngineSnapshot> => invoke("stop_android_mirror");
