<script setup lang="ts">
/**
 * @file 工具安裝記錄檢視元件，提供目前日誌檔路徑顯示與彈出式純文字閱讀視窗。
 */
import { useInstallLogViewer } from "../composables/useInstallLogViewer";

/** 元件接收的唯讀 Props。 */
defineProps<{
  /** 工具安裝日誌檔在本機磁碟的完整路徑，顯示於按鈕旁供使用者定位。 */
  logPath: string;
}>();

/** 取得安裝記錄彈窗的開關、載入狀態、文字內容與控制函式。 */
const { isOpen, loading, content, error, openViewer, closeViewer } = useInstallLogViewer();
</script>

<template>
  <div class="install-log-control">
    <p class="install-log-path">安裝記錄：{{ logPath }}</p>
    <button class="button-secondary install-log-open" type="button" :disabled="loading" data-testid="install-log-open" @click="openViewer">
      {{ loading ? "正在讀取…" : "查看安裝記錄" }}
    </button>
  </div>

  <div v-if="isOpen" class="install-log-backdrop" data-testid="install-log-dialog-backdrop">
    <section class="install-log-dialog" role="dialog" aria-modal="true" aria-labelledby="install-log-title">
      <div class="install-log-heading">
        <h2 id="install-log-title">安裝記錄</h2>
        <button class="button-secondary" type="button" aria-label="關閉安裝記錄" data-testid="install-log-close" @click="closeViewer">關閉</button>
      </div>
      <p v-if="loading" class="install-log-message" role="status">正在載入安裝記錄…</p>
      <p v-else-if="error" class="install-log-error" role="alert">{{ error }}</p>
      <p v-else-if="!content" class="install-log-message">安裝記錄目前沒有內容。</p>
      <textarea
        v-else
        class="install-log-content"
        aria-label="安裝記錄內容"
        data-testid="install-log-content"
        :value="content"
        readonly
        spellcheck="false"
      ></textarea>
    </section>
  </div>
</template>

<style scoped>
.install-log-control { display: flex; align-items: center; flex-wrap: wrap; gap: 8px 12px; }
.install-log-path { margin: 0; overflow-wrap: anywhere; color: var(--text-muted); font-family: var(--font-mono); font-size: 11px; line-height: 1.7; }
.install-log-open { min-width: auto; padding: 7px 11px; border-color: var(--phosphor-cyan); color: var(--phosphor-cyan); background: var(--phosphor-cyan-dim); font-size: 11px; }
.install-log-open:hover { border-color: var(--phosphor-cyan); color: var(--bg-ash); background: var(--phosphor-cyan); }
.install-log-backdrop { position: fixed; inset: 0; z-index: 10; display: grid; place-items: center; padding: 20px; background: rgba(3, 5, 7, .85); }
.install-log-dialog { display: flex; flex-direction: column; width: min(100%, 900px); max-height: 85vh; padding: 22px; border: 1px solid var(--border-rust); border-top: 2px solid var(--phosphor-cyan); border-radius: 3px; background: var(--bg-bunker); box-shadow: 0 24px 80px rgba(0, 0, 0, .65); }
.install-log-heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 14px; }
.install-log-heading h2 { margin: 0; color: var(--text-primary); font-size: 16px; letter-spacing: .05em; }
.install-log-message, .install-log-error { margin: 12px 0; color: var(--text-secondary); font-size: 12px; line-height: 1.7; }
.install-log-error { color: var(--crimson-alert); }
.install-log-content { width: 100%; height: min(62vh, 620px); min-height: 0; box-sizing: border-box; padding: 14px; resize: vertical; overflow: auto; border: 1px solid var(--border-rust); border-radius: 2px; background: var(--bg-ash); color: var(--text-primary); font-family: var(--font-mono); font-size: 12px; line-height: 1.65; white-space: pre; -webkit-user-select: text; user-select: text; }
.install-log-content:focus-visible { outline: 1px solid var(--phosphor-cyan); outline-offset: 2px; }
</style>
