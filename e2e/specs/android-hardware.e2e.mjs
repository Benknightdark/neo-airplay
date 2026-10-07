import assert from 'node:assert/strict'

const hardwareEnabled = process.env.NEO_HARDWARE_E2E === '1'

;(hardwareEnabled ? describe : describe.skip)('Neo AirPlay hardware projection', () => {
  it('runs AirPlay and Android together, then stops Android independently', async () => {
    const readSnapshot = () => browser.tauri.execute(({ core }) => core.invoke('get_snapshot'))
    const invoke = (command, args) =>
      browser.tauri.execute(({ core }, name, payload) => core.invoke(name, payload), command, args)

    try {
      await $('[data-testid="receiver-start"]').click()
      await browser.waitUntil(async () => (await readSnapshot()).receiver.state === 'Running', {
        timeout: 90000,
        timeoutMsg: 'UxPlay did not enter Running state. Check the AirPlay engine and mDNS service.',
      })

      await $('[data-testid="android-refresh"]').click()
      await browser.waitUntil(async () => (await readSnapshot()).devices.some((device) => device.status === 'Ready'), {
        timeout: 30000,
        timeoutMsg: 'No authorized Android device was found by ADB.',
      })

      const readySerial = (await readSnapshot()).devices.find((device) => device.status === 'Ready').serial
      await $('[data-testid="android-device-select"]').selectByAttribute('value', readySerial)
      await $('[data-testid="android-start"]').click()
      await browser.waitUntil(async () => {
        const current = await readSnapshot()
        return current.receiver.state === 'Running' && current.android.state === 'Running'
      }, {
        timeout: 45000,
        timeoutMsg: 'Android mirror did not start while the AirPlay receiver was running.',
      })

      await $('[data-testid="android-stop"]').click()
      await browser.waitUntil(async () => {
        const current = await readSnapshot()
        return current.android.state === 'Stopped' && current.receiver.state === 'Running'
      }, {
        timeout: 15000,
        timeoutMsg: 'Stopping Android also stopped or failed to preserve AirPlay.',
      })
    } finally {
      await invoke('stop_android_mirror').catch(() => undefined)
      await invoke('stop_receiver').catch(() => undefined)
    }

    const finalState = await readSnapshot()
    assert.equal(finalState.android.state, 'Stopped')
    assert.equal(finalState.receiver.state, 'Stopped')
  }, 180000)
})
