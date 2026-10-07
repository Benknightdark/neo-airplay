<script setup lang="ts">
import { useAppStore } from '../stores/app'
import { api } from '../api/tauri'

const store = useAppStore()
const time = (timestamp: string) => new Date(timestamp).toLocaleTimeString()
</script>

<template>
  <section class="page-heading">
    <div><p class="eyebrow">問題排查</p><h2 data-testid="current-page-heading">診斷記錄</h2><p class="muted">顯示最近 300 筆應用程式事件。</p></div>
    <button class="button button-secondary" data-testid="open-log-directory" @click="api.openLogDirectory">開啟記錄資料夾</button>
  </section>
  <section class="log-panel">
    <div class="log-toolbar"><div><strong>應用程式記錄</strong><span>{{ store.logs.length }} 筆</span></div><span class="live-indicator"><i></i> 即時</span></div>
    <div v-if="store.logs.length === 0" class="empty-state" data-testid="logs-empty"><span class="empty-symbol">≡</span><strong>目前沒有記錄</strong><p>啟動接收器或連接 Android 裝置後，事件會顯示在這裡。</p></div>
    <ol v-else class="log-list" data-testid="diagnostics-log-list">
      <li v-for="(entry, index) in store.logs" :key="`${entry.timestamp}-${index}`" :class="`log-${entry.level.toLowerCase()}`"><time>{{ time(entry.timestamp) }}</time><span class="log-level">{{ entry.level }}</span><span class="log-message">{{ entry.message }}</span></li>
    </ol>
  </section>
</template>
