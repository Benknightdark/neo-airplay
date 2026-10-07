export type ReceiverState =
  | 'Stopped'
  | 'Starting'
  | 'Running'
  | 'Stopping'
  | 'Faulted'
  | 'DependencyMissing'

export type AndroidDeviceStatus = 'Ready' | 'Unauthorized' | 'Offline' | 'Unknown'
export type Platform = 'windows' | 'macos' | 'linux'
export type LogLevel = 'Information' | 'Warning' | 'Error'

export interface ReceiverSettings {
  receiverName: string
  renderer: string
  fullscreen: boolean
  startOnLogin: boolean
  androidHwDecode: boolean
  uxplayPath: string | null
}

export interface EngineStatus {
  state: ReceiverState
  message: string
}

export interface AndroidDevice {
  serial: string
  model: string
  status: AndroidDeviceStatus
}

export interface RendererOption {
  value: string
  label: string
}

export interface AppLogEntry {
  timestamp: string
  level: LogLevel
  message: string
}

export interface EngineSetupStatus {
  message: string
  inProgress: boolean
}

export interface AppSnapshot {
  receiver: EngineStatus
  android: EngineStatus
  devices: AndroidDevice[]
  settings: ReceiverSettings
  platform: Platform
  rendererOptions: RendererOption[]
  logs: AppLogEntry[]
  engineSetup: EngineSetupStatus
  version: string
}

export interface SaveResult {
  settings: ReceiverSettings
  applyPending: boolean
  message: string
}
