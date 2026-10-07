import assert from 'node:assert/strict'

describe('Neo AirPlay desktop navigation', () => {
  it('opens each page and saves settings in the isolated E2E profile', async () => {
    const waitForHeading = async (expected) => {
      await browser.waitUntil(async () => {
        const heading = await $('[data-testid="current-page-heading"]')
        if (!(await heading.isDisplayed().catch(() => false))) return false
        return (await heading.getText().catch(() => '')) === expected
      }, { timeout: 10000, timeoutMsg: `Page heading did not change to ${expected}.` })
    }

    await waitForHeading('投影接收器')

    await $('[data-testid="nav-settings"]').click()
    await waitForHeading('設定')

    const receiverName = $('[data-testid="receiver-name-input"]')
    await receiverName.setValue('Neo AirPlay E2E')
    const saveButton = $('[data-testid="save-settings"]')
    await saveButton.click()
    await browser.waitUntil(async () => {
      const snapshot = await browser.tauri.execute(({ core }) => core.invoke('get_snapshot'))
      return snapshot.settings.receiverName === 'Neo AirPlay E2E'
    }, { timeout: 15000 })
    assert.equal(await receiverName.getValue(), 'Neo AirPlay E2E')

    await $('[data-testid="nav-diagnostics"]').click()
    await waitForHeading('診斷記錄')
  })
})
