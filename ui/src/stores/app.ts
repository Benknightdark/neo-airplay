import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { api, subscribeToAppEvents } from '../api/tauri'
import type {
  AndroidDevice,
  AppLogEntry,
  AppSnapshot,
  EngineSetupStatus,
  EngineStatus,
  Platform,
  ReceiverSettings,
} from '../bindings/types'

const defaultSettings: ReceiverSettings = {
  receiverName: 'Neo AirPlay',
  renderer: 'Auto',
  fullscreen: false,
  startOnLogin: false,
  androidHwDecode: false,
  uxplayPath: null,
}

const stoppedLike = (state: EngineStatus['state']) =>
  state === 'Stopped' || state === 'Faulted' || state === 'DependencyMissing'

export const useAppStore = defineStore('app', () => {
  const receiver = ref<EngineStatus>({ state: 'Stopped', message: '正在載入接收器狀態。' })
  const android = ref<EngineStatus>({ state: 'Stopped', message: '正在載入 Android 狀態。' })
  const settings = ref<ReceiverSettings>({ ...defaultSettings })
  const devices = ref<AndroidDevice[]>([])
  const logs = ref<AppLogEntry[]>([])
  const rendererOptions = ref([{ value: 'Auto', label: '自動' }])
  const platform = ref<Platform>('windows')
  const version = ref('')
  const engineSetup = ref<EngineSetupStatus>({ message: '', inProgress: true })
  const selectedSerial = ref<string | null>(null)
  const alert = ref('')
  const busy = ref(false)
  const applyPending = ref(false)
  const initialized = ref(false)
  let unlisten: (() => void) | undefined

  const selectedDevice = computed(
    () => devices.value.find((device) => device.serial === selectedSerial.value) ?? null,
  )
  const canStartReceiver = computed(
    () => !busy.value && !engineSetup.value.inProgress && stoppedLike(receiver.value.state),
  )
  const canStopReceiver = computed(
    () => !busy.value && ['Starting', 'Running'].includes(receiver.value.state),
  )
  const canStartAndroid = computed(
    () =>
      !busy.value &&
      !engineSetup.value.inProgress &&
      selectedDevice.value?.status === 'Ready' &&
      stoppedLike(android.value.state),
  )

  function acceptSnapshot(snapshot: AppSnapshot) {
    receiver.value = snapshot.receiver
    android.value = snapshot.android
    settings.value = snapshot.settings
    devices.value = snapshot.devices
    rendererOptions.value = snapshot.rendererOptions
    platform.value = snapshot.platform
    logs.value = snapshot.logs.slice(-300)
    engineSetup.value = snapshot.engineSetup
    version.value = snapshot.version
    selectedSerial.value =
      devices.value.find((device) => device.status === 'Ready')?.serial ??
      devices.value[0]?.serial ??
      null
  }

  async function initialize() {
    if (initialized.value) return
    initialized.value = true
    try {
      acceptSnapshot(await api.getSnapshot())
      unlisten = await subscribeToAppEvents({
        receiver: (status) => (receiver.value = status),
        android: (status) => (android.value = status),
        log: (entry) => {
          logs.value = [...logs.value.slice(-299), entry]
        },
        setup: (status) => (engineSetup.value = status),
        refreshLogs: async () => {
          logs.value = (await api.getSnapshot()).logs.slice(-300)
        },
      })
    } catch (error) {
      alert.value = errorMessage(error)
      engineSetup.value = { message: '無法連接 Neo AirPlay 後端。', inProgress: false }
    }
  }

  async function withBusy<T>(action: () => Promise<T>): Promise<T | undefined> {
    busy.value = true
    alert.value = ''
    try {
      return await action()
    } catch (error) {
      alert.value = errorMessage(error)
      return undefined
    } finally {
      busy.value = false
    }
  }

  async function startReceiver() {
    await withBusy(async () => {
      receiver.value = await api.startReceiver()
      if (receiver.value.state === 'Running') applyPending.value = false
      if (receiver.value.state === 'Faulted' || receiver.value.state === 'DependencyMissing') {
        alert.value = receiver.value.message
      }
    })
  }

  async function stopReceiver() {
    await withBusy(async () => {
      receiver.value = await api.stopReceiver()
      applyPending.value = false
    })
  }

  async function saveSettings(next: ReceiverSettings) {
    return withBusy(async () => {
      const result = await api.saveSettings(next)
      settings.value = result.settings
      applyPending.value = result.applyPending
      return result
    })
  }

  async function applySettings() {
    await withBusy(async () => {
      receiver.value = await api.applySettings()
      applyPending.value = false
      if (receiver.value.state !== 'Running') alert.value = receiver.value.message
    })
  }

  async function refreshDevices() {
    await withBusy(async () => {
      const previous = selectedSerial.value
      devices.value = await api.refreshAndroidDevices()
      selectedSerial.value =
        devices.value.find((device) => device.serial === previous)?.serial ??
        devices.value.find((device) => device.status === 'Ready')?.serial ??
        devices.value[0]?.serial ??
        null
    })
  }

  async function startAndroid() {
    const serial = selectedSerial.value
    if (!serial) return
    await withBusy(async () => {
      android.value = await api.startAndroidMirror(serial)
      if (android.value.state === 'Faulted' || android.value.state === 'DependencyMissing') {
        alert.value = android.value.message
      }
    })
  }

  async function stopAndroid() {
    await withBusy(async () => {
      android.value = await api.stopAndroidMirror()
    })
  }

  function dispose() {
    unlisten?.()
    unlisten = undefined
  }

  return {
    receiver,
    android,
    settings,
    devices,
    logs,
    rendererOptions,
    platform,
    version,
    engineSetup,
    selectedSerial,
    selectedDevice,
    alert,
    busy,
    applyPending,
    canStartReceiver,
    canStopReceiver,
    canStartAndroid,
    initialize,
    saveSettings,
    startReceiver,
    stopReceiver,
    applySettings,
    refreshDevices,
    startAndroid,
    stopAndroid,
    dispose,
  }
})

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}
