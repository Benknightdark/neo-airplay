import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type {
  AndroidDevice,
  AppSnapshot,
  EngineStatus,
  ReceiverSettings,
  SaveResult,
} from '../../bindings/types'
import { useAppStore } from '../app'

const mocks = vi.hoisted(() => ({
  api: {
    getSnapshot: vi.fn(),
    startReceiver: vi.fn(),
    stopReceiver: vi.fn(),
    saveSettings: vi.fn(),
    applySettings: vi.fn(),
    refreshAndroidDevices: vi.fn(),
    startAndroidMirror: vi.fn(),
    stopAndroidMirror: vi.fn(),
    openLogDirectory: vi.fn(),
    hideToTray: vi.fn(),
    exitApp: vi.fn(),
  },
  subscribeToAppEvents: vi.fn(),
}))

vi.mock('../../api/tauri', () => mocks)

const defaultSettings: ReceiverSettings = {
  receiverName: 'Neo AirPlay',
  renderer: 'Auto',
  fullscreen: false,
  startOnLogin: false,
  androidHwDecode: false,
  uxplayPath: null,
}

const readyDevice: AndroidDevice = {
  serial: 'usb-123',
  model: 'Pixel 9',
  status: 'Ready',
}

function snapshot(overrides: Partial<AppSnapshot> = {}): AppSnapshot {
  return {
    receiver: { state: 'Stopped', message: '等待啟動。' },
    android: { state: 'Stopped', message: '連接裝置後更新清單。' },
    devices: [readyDevice],
    settings: { ...defaultSettings },
    platform: 'macos',
    rendererOptions: [{ value: 'Auto', label: '自動' }],
    logs: [],
    engineSetup: { message: '引擎已準備。', inProgress: false },
    version: '0.1.0',
    ...overrides,
  }
}

beforeEach(() => {
  vi.clearAllMocks()
  setActivePinia(createPinia())
  mocks.api.getSnapshot.mockResolvedValue(snapshot())
  mocks.subscribeToAppEvents.mockResolvedValue(vi.fn())
})

describe('application store', () => {
  it('loads the backend snapshot and subscribes to app events', async () => {
    const store = useAppStore()

    await store.initialize()

    expect(store.receiver.state).toBe('Stopped')
    expect(store.selectedDevice).toEqual(readyDevice)
    expect(store.platform).toBe('macos')
    expect(store.version).toBe('0.1.0')
    expect(mocks.subscribeToAppEvents).toHaveBeenCalledOnce()
  })

  it('keeps the selected device when refreshing and falls back to a ready device', async () => {
    const anotherDevice: AndroidDevice = { ...readyDevice, serial: 'wifi-456', model: 'Tablet' }
    const store = useAppStore()
    await store.initialize()
    store.selectedSerial = 'wifi-456'
    mocks.api.refreshAndroidDevices.mockResolvedValue([readyDevice, anotherDevice])

    await store.refreshDevices()
    expect(store.selectedSerial).toBe('wifi-456')

    mocks.api.refreshAndroidDevices.mockResolvedValue([
      { ...readyDevice, status: 'Unauthorized' },
      anotherDevice,
    ])
    await store.refreshDevices()
    expect(store.selectedSerial).toBe('wifi-456')

    mocks.api.refreshAndroidDevices.mockResolvedValue([{ ...readyDevice, status: 'Unauthorized' }])
    await store.refreshDevices()
    expect(store.selectedSerial).toBe('usb-123')
  })

  it('saves settings and exposes whether the running receiver needs to restart', async () => {
    const store = useAppStore()
    await store.initialize()
    const running: EngineStatus = { state: 'Running', message: '接收中。' }
    store.receiver = running
    const updated = { ...defaultSettings, receiverName: 'Living Room' }
    const result: SaveResult = { settings: updated, applyPending: true, message: '需重新啟動。' }
    mocks.api.saveSettings.mockResolvedValue(result)

    await store.saveSettings(updated)

    expect(mocks.api.saveSettings).toHaveBeenCalledWith(updated)
    expect(store.settings.receiverName).toBe('Living Room')
    expect(store.applyPending).toBe(true)
  })

  it('turns backend failures into a visible alert and clears the busy state', async () => {
    const store = useAppStore()
    await store.initialize()
    mocks.api.startReceiver.mockRejectedValue(new Error('UxPlay could not start.'))

    await store.startReceiver()

    expect(store.alert).toBe('UxPlay could not start.')
    expect(store.busy).toBe(false)
  })
})
