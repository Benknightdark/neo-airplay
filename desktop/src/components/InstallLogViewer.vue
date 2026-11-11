<script setup lang="ts">
import { ref } from "vue";
import { readInstallLog } from "../api";

defineProps<{ logPath: string }>();

const isOpen = ref(false);
const loading = ref(false);
const content = ref("");
const error = ref("");
let requestGeneration = 0;

async function openViewer(): Promise<void> {
  if (loading.value) return;

  const generation = ++requestGeneration;
  isOpen.value = true;
  loading.value = true;
  content.value = "";
  error.value = "";

  try {
    const log = await readInstallLog();
    if (generation !== requestGeneration) return;
    content.value = log;
  } catch (reason) {
    if (generation !== requestGeneration) return;
    error.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    if (generation === requestGeneration) loading.value = false;
  }
}

function closeViewer(): void {
  requestGeneration += 1;
  isOpen.value = false;
  loading.value = false;
}
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
.install-log-path { margin: 0; overflow-wrap: anywhere; font-size: 12px; }
.install-log-open { padding: 6px 10px; font-size: 11px; }
.install-log-backdrop { position: fixed; inset: 0; z-index: 10; display: grid; place-items: center; padding: 20px; background: #11182780; }
.install-log-dialog { display: flex; flex-direction: column; width: min(100%, 900px); max-height: 85vh; padding: 20px; border-radius: 13px; background: #fff; box-shadow: 0 20px 55px #10182740; }
.install-log-heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 14px; }
.install-log-heading h2 { margin: 0; color: #2e394d; font-size: 16px; }
.install-log-message, .install-log-error { margin: 12px 0; color: #667387; font-size: 12px; }
.install-log-error { color: #a3444a; }
.install-log-content { width: 100%; min-height: min(62vh, 620px); box-sizing: border-box; padding: 12px; resize: vertical; overflow: auto; border: 1px solid #dfe4eb; border-radius: 8px; background: #f8fafc; color: #263245; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; line-height: 1.55; white-space: pre; user-select: text; }
</style>
