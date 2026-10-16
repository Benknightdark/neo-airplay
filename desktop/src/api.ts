import { invoke } from "@tauri-apps/api/core";
import type { AndroidDevice, DashboardSnapshot, DependencySnapshot, EngineSnapshot, ReceiverSettings } from "./types";

export const getDependencies = (): Promise<DependencySnapshot> => invoke("get_dependencies");
export const installDependencies = (): Promise<DependencySnapshot> => invoke("install_dependencies");

export const getDashboard = (): Promise<DashboardSnapshot> => invoke("get_dashboard");
export const saveSettings = (settings: ReceiverSettings): Promise<ReceiverSettings> => invoke("save_settings", { settings });
export const startAirplay = (): Promise<EngineSnapshot> => invoke("start_airplay");
export const stopAirplay = (): Promise<EngineSnapshot> => invoke("stop_airplay");
export const refreshAndroidDevices = (): Promise<AndroidDevice[]> => invoke("refresh_android_devices");
export const startAndroidMirror = (serial: string): Promise<EngineSnapshot> => invoke("start_android_mirror", { serial });
export const stopAndroidMirror = (): Promise<EngineSnapshot> => invoke("stop_android_mirror");
