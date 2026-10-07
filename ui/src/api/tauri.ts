import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type {
  AndroidDevice,
  AppLogEntry,
  AppSnapshot,
  EngineSetupStatus,
  EngineStatus,
  ReceiverSettings,
  SaveResult,
} from '../bindings/types'

export const api = {
  getSnapshot: () => invoke<AppSnapshot>('get_snapshot'),
  startReceiver: () => invoke<EngineStatus>('start_receiver'),
  stopReceiver: () => invoke<EngineStatus>('stop_receiver'),
  saveSettings: (settings: ReceiverSettings) =>
    invoke<SaveResult>('save_settings', { settings }),
  applySettings: () => invoke<EngineStatus>('apply_settings'),
  refreshAndroidDevices: () => invoke<AndroidDevice[]>('refresh_android_devices'),
  startAndroidMirror: (serial: string) =>
    invoke<EngineStatus>('start_android_mirror', { serial }),
  stopAndroidMirror: () => invoke<EngineStatus>('stop_android_mirror'),
  openLogDirectory: () => invoke<void>('open_log_directory'),
  hideToTray: () => invoke<void>('hide_to_tray'),
  exitApp: () => invoke<void>('exit_app'),
}

export async function subscribeToAppEvents(handlers: {
  receiver: (status: EngineStatus) => void
  android: (status: EngineStatus) => void
  log: (entry: AppLogEntry) => void
  setup: (status: EngineSetupStatus) => void
  refreshLogs: () => void
}) {
  const unlisten = await Promise.all([
    listen<EngineStatus>('receiver-status', (event) => handlers.receiver(event.payload)),
    listen<EngineStatus>('android-status', (event) => handlers.android(event.payload)),
    listen<AppLogEntry>('log-entry', (event) => handlers.log(event.payload)),
    listen<EngineSetupStatus>('engine-setup-status', (event) => handlers.setup(event.payload)),
    listen('log-entry-refresh-required', handlers.refreshLogs),
  ])
  return () => unlisten.forEach((stop) => stop())
}
