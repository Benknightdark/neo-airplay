<script setup lang="ts">
import { useAppStore } from '../stores/app'
import type { AndroidDeviceStatus, ReceiverState } from '../bindings/types'

const store = useAppStore()
const stateLabel = (state: ReceiverState) =>
  ({
    Stopped: '未啟動',
    Starting: '正在啟動',
    Running: '接收中',
    Stopping: '正在停止',
    Faulted: '發生錯誤',
    DependencyMissing: '缺少元件',
  })[state]
const deviceStatusLabel = (status: AndroidDeviceStatus) =>
  ({ Ready: '可投影', Unauthorized: '尚未授權', Offline: '離線', Unknown: '狀態未知' })[status]
</script>

<template>
  <section class="page-heading">
    <div>
      <p class="eyebrow">控制台</p>
      <h2 data-testid="current-page-heading">投影接收器</h2>
      <p class="muted">管理 AirPlay 與 Android 鏡像。兩種投影可同時運作。</p>
    </div>
    <span class="local-badge"><span class="online-dot"></span> 僅在此裝置執行</span>
  </section>

  <section class="engine-grid" aria-label="投影引擎">
    <article class="engine-card airplay-card">
      <div class="card-topline">
        <div class="engine-symbol airplay-symbol" aria-hidden="true">⌁</div>
        <span :class="['state-pill', `pill-${store.receiver.state.toLowerCase()}`]">{{ stateLabel(store.receiver.state) }}</span>
      </div>
      <div class="engine-copy">
        <p class="eyebrow">IOS 與 MACOS</p>
        <h3>AirPlay 接收器</h3>
        <p class="muted">{{ store.receiver.message || '等待啟動。' }}</p>
      </div>
      <div class="card-actions">
        <button v-if="store.receiver.state === 'Running' || store.receiver.state === 'Starting'" class="button button-danger" data-testid="receiver-stop" :disabled="!store.canStopReceiver" @click="store.stopReceiver">停止接收器</button>
        <button v-else class="button button-primary" data-testid="receiver-start" :disabled="!store.canStartReceiver" @click="store.startReceiver">啟動接收器</button>
        <span class="shortcut-note">{{ store.settings.receiverName }}</span>
      </div>
    </article>

    <article class="engine-card android-card">
      <div class="card-topline">
        <div class="engine-symbol android-symbol" aria-hidden="true">▣</div>
        <span :class="['state-pill', `pill-${store.android.state.toLowerCase()}`]">{{ stateLabel(store.android.state) }}</span>
      </div>
      <div class="engine-copy">
        <p class="eyebrow">USB 或無線偵錯</p>
        <h3>Android 鏡像</h3>
        <p class="muted">{{ store.android.message || '連接裝置後更新清單。' }}</p>
      </div>
      <div class="device-picker-row">
        <label class="sr-only" for="android-device">選擇 Android 裝置</label>
        <select id="android-device" v-model="store.selectedSerial" data-testid="android-device-select">
          <option v-if="store.devices.length === 0" :value="null">尚未找到裝置</option>
          <option v-for="device in store.devices" :key="device.serial" :value="device.serial">{{ device.model }} · {{ deviceStatusLabel(device.status) }}</option>
        </select>
        <button class="button button-secondary button-square" data-testid="android-refresh" :disabled="store.busy" aria-label="重新整理裝置" @click="store.refreshDevices">↻</button>
      </div>
      <div class="card-actions">
        <button v-if="store.android.state === 'Running' || store.android.state === 'Starting'" class="button button-danger" data-testid="android-stop" :disabled="store.busy" @click="store.stopAndroid">停止鏡像</button>
        <button v-else class="button button-primary" data-testid="android-start" :disabled="!store.canStartAndroid" @click="store.startAndroid">開始鏡像</button>
        <span class="shortcut-note">scrcpy</span>
      </div>
    </article>
  </section>

  <section class="privacy-callout">
    <div class="privacy-icon" aria-hidden="true">⌂</div>
    <div><strong>畫面在本機處理</strong><p>鏡像連線由獨立引擎處理，不會經由 Neo AirPlay 雲端轉送。</p></div>
    <span class="privacy-status">本機連線</span>
  </section>
</template>
