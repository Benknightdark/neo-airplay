<script setup lang="ts">
import { reactive, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import type { ReceiverSettings } from '../bindings/types'
import { useAppStore } from '../stores/app'

const store = useAppStore()
const draft = reactive<ReceiverSettings>({ ...store.settings })
watch(store.settings, (settings) => Object.assign(draft, settings))

async function chooseUxPlay() {
  const selected = await open({
    multiple: false,
    directory: false,
    title: '選擇 UxPlay 執行檔',
    filters: [{ name: 'UxPlay', extensions: ['exe', 'app', '*'] }],
  })
  if (typeof selected === 'string') draft.uxplayPath = selected
}

async function copyInstallCommand() {
  const scriptName = 'install-uxplay-' + store.platform + '.sh'
  const releaseUrl = 'https://github.com/Benknightdark/neo-airplay/releases/download/v' + store.version
  const command = [
    'curl --fail --location --output ' + scriptName + ' ' + releaseUrl + '/' + scriptName,
    'curl --fail --location --output uxplay-release.json ' + releaseUrl + '/uxplay-release.json',
    'bash ./' + scriptName,
  ].join(' && ')
  await writeText(command)
  store.alert = '安裝指令已複製。'
}

async function save() {
  await store.saveSettings({ ...draft, uxplayPath: draft.uxplayPath?.trim() || null })
}
</script>

<template>
  <section class="page-heading">
    <div><p class="eyebrow">偏好設定</p><h2 data-testid="current-page-heading">設定</h2><p class="muted">調整接收器名稱、畫面與啟動方式。</p></div>
  </section>

  <section class="settings-panel">
    <div class="settings-section">
      <div class="section-title"><div class="section-icon">A</div><div><h3>AirPlay 接收器</h3><p>設定 iPhone 或 iPad 顯示的接收器資訊。</p></div></div>
      <label class="field-label" for="receiver-name">接收器名稱</label>
      <input id="receiver-name" v-model="draft.receiverName" maxlength="80" data-testid="receiver-name-input" />
      <label class="field-label" for="renderer">畫面輸出</label>
      <select id="renderer" v-model="draft.renderer" data-testid="renderer-select"><option v-for="option in store.rendererOptions" :key="option.value" :value="option.value">{{ option.label }}</option></select>
      <label class="toggle-row"><span><strong>全螢幕顯示</strong><small>啟動 AirPlay 時使用全螢幕視窗。</small></span><input v-model="draft.fullscreen" type="checkbox" data-testid="fullscreen-toggle" /></label>
      <div v-if="store.platform !== 'windows'" class="uxplay-path-block">
        <label class="field-label" for="uxplay-path">UxPlay 執行檔</label>
        <div class="input-action-row"><input id="uxplay-path" v-model="draft.uxplayPath" placeholder="自動搜尋系統路徑" /><button class="button button-secondary" @click="chooseUxPlay">選擇檔案</button></div>
        <button class="text-button" @click="copyInstallCommand">複製安裝指令</button>
      </div>
    </div>

    <div class="settings-section">
      <div class="section-title"><div class="section-icon android-section-icon">D</div><div><h3>Android 鏡像</h3><p>調整 scrcpy 的影像解碼方式。</p></div></div>
      <label class="toggle-row"><span><strong>使用硬體解碼</strong><small>若出現黑畫面，可關閉後重新開始鏡像。</small></span><input v-model="draft.androidHwDecode" type="checkbox" data-testid="android-hw-decode-toggle" /></label>
    </div>

    <div class="settings-section last-section">
      <div class="section-title"><div class="section-icon login-section-icon">↗</div><div><h3>啟動方式</h3><p>選擇是否在登入桌面時開啟應用程式。</p></div></div>
      <label class="toggle-row"><span><strong>登入後自動啟動</strong><small>在背景啟動並開始 AirPlay 接收器。</small></span><input v-model="draft.startOnLogin" type="checkbox" data-testid="autostart-toggle" /></label>
    </div>

    <div class="settings-footer"><span v-if="store.applyPending" class="pending-note">AirPlay 接收器需要重新啟動以套用設定。</span><button class="button button-primary" data-testid="save-settings" :disabled="store.busy" @click="save">儲存設定</button></div>
  </section>
</template>
