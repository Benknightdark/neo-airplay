<script setup lang="ts">
import { onBeforeUnmount, onMounted } from 'vue'
import { isTauri } from '@tauri-apps/api/core'
import { RouterLink, RouterView } from 'vue-router'
import { useAppStore } from './stores/app'
import type { ReceiverState } from './bindings/types'

const store = useAppStore()
const statusLabel = (state: ReceiverState) =>
  ({
    Stopped: '未啟動',
    Starting: '正在啟動',
    Running: '接收中',
    Stopping: '正在停止',
    Faulted: '發生錯誤',
    DependencyMissing: '缺少元件',
  })[state]

onMounted(() => {
  if (isTauri()) void store.initialize()
})

onBeforeUnmount(store.dispose)
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand-lockup">
        <span class="brand-mark" aria-hidden="true">N</span>
        <div>
          <strong>Neo AirPlay</strong>
          <small>{{ store.platform.toUpperCase() }} RECEIVER</small>
        </div>
      </div>
      <nav class="primary-nav" aria-label="主導覽">
        <RouterLink to="/" data-testid="nav-dashboard"><span>◈</span> 控制台</RouterLink>
        <RouterLink to="/settings" data-testid="nav-settings"><span>⚙</span> 設定</RouterLink>
        <RouterLink to="/diagnostics" data-testid="nav-diagnostics"><span>≡</span> 診斷記錄</RouterLink>
      </nav>
      <div class="sidebar-foot">
        <span class="online-dot"></span><span>本機服務</span>
        <span class="version">{{ store.version || '0.1.0' }}</span>
      </div>
    </aside>

    <div class="workspace">
      <header class="topbar">
        <div><p class="eyebrow">NEO AIRPLAY RECEIVER</p><h1>Neo AirPlay</h1></div>
        <div class="topbar-status" data-testid="receiver-summary">
          <span :class="['status-dot', `status-${store.receiver.state.toLowerCase()}`]"></span>
          <span>{{ statusLabel(store.receiver.state) }}</span>
        </div>
      </header>
      <div v-if="store.engineSetup.message" class="setup-notice" data-testid="engine-setup-status">
        <span class="setup-icon">i</span><span>{{ store.engineSetup.message }}</span>
        <span v-if="store.engineSetup.inProgress" class="loading-indicator"></span>
      </div>
      <div v-if="store.alert" class="alert-banner" role="alert" data-testid="alert-message">
        <span>{{ store.alert }}</span>
        <button class="icon-button" aria-label="關閉訊息" @click="store.alert = ''">×</button>
      </div>
      <main class="page-content"><RouterView /></main>
    </div>
  </div>
</template>
