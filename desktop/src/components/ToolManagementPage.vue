<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { getDependencies, manageDependencies } from "../api";
import InstallLogViewer from "./InstallLogViewer.vue";
import type { DependencyAction, DependencySnapshot } from "../types";

const snapshot = ref<DependencySnapshot | null>(null);
const error = ref("");
const requesting = ref(false);
const confirmingReinstall = ref(false);
const busy = computed(() => requesting.value || snapshot.value?.status === "checking" || snapshot.value?.status === "installing");
const unavailable = computed(() => !snapshot.value || snapshot.value.status === "disabled");
const operationLabel = computed(() => {
  switch (snapshot.value?.operation) {
    case "upgrade": return "升級工具";
    case "reinstall": return "清除並重新安裝";
    case "install": return "準備工具";
    default: return "工具狀態";
  }
});
const statusLabel = computed(() => {
  switch (snapshot.value?.status) {
    case "checking": return "檢查中";
    case "installing": return "處理中";
    case "ready": return "已就緒";
    case "failed": return "需要處理";
    case "disabled": return "已停用";
    default: return "讀取中";
  }
});

let timer: number | undefined;
let disposed = false;
let polling = false;
let requestGeneration = 0;

function stopPolling(): void {
  if (timer !== undefined) {
    window.clearInterval(timer);
    timer = undefined;
  }
}

async function refresh(): Promise<void> {
  if (disposed || polling || requesting.value) return;
  polling = true;
  const generation = requestGeneration;
  try {
    const next = await getDependencies();
    if (disposed || generation !== requestGeneration) return;
    snapshot.value = next;
    error.value = "";
    if (next.status === "disabled") stopPolling();
  } catch (reason) {
    if (!disposed && generation === requestGeneration) {
      error.value = reason instanceof Error ? reason.message : String(reason);
    }
  } finally {
    polling = false;
  }
}

async function runAction(action: DependencyAction): Promise<void> {
  if (disposed || busy.value || unavailable.value) return;
  const generation = ++requestGeneration;
  requesting.value = true;
  error.value = "";
  try {
    snapshot.value = await manageDependencies(action);
  } catch (reason) {
    if (!disposed && generation === requestGeneration) {
      error.value = reason instanceof Error ? reason.message : String(reason);
    }
  } finally {
    if (!disposed && generation === requestGeneration) requesting.value = false;
  }
}

function requestReinstall(): void {
  if (!busy.value && !unavailable.value) confirmingReinstall.value = true;
}

async function confirmReinstall(): Promise<void> {
  confirmingReinstall.value = false;
  await runAction("reinstall");
}

onMounted(() => {
  void refresh();
  timer = window.setInterval(() => void refresh(), 1500);
});

onUnmounted(() => {
  disposed = true;
  stopPolling();
});
</script>

<template>
  <section class="page-content tools-layout" aria-label="工具管理" data-testid="tools-page">
    <div class="section-intro">
      <div>
        <p class="eyebrow">投影工具</p>
        <h2>管理工具與相依套件</h2>
        <p class="muted-copy">升級至 Neo AirPlay 支援的工具版本，或清除 App 專用工具後重新安裝。</p>
      </div>
      <span class="tool-status" :class="`tool-status-${snapshot?.status ?? 'checking'}`" role="status" data-testid="tools-status">
        <span class="status-indicator" aria-hidden="true"></span>{{ statusLabel }}
      </span>
    </div>

    <div class="tool-status-card" :class="{ 'tool-status-card-failed': snapshot?.status === 'failed' || error }" aria-live="polite">
      <div class="tool-status-card-heading">
        <span class="eyebrow">{{ operationLabel }}</span>
        <span v-if="snapshot?.operation" class="tool-operation-label">{{ snapshot.operation }}</span>
      </div>
      <p data-testid="tools-message">{{ error || snapshot?.message || "正在讀取工具狀態…" }}</p>
      <ul v-if="snapshot?.missing.length" class="tool-missing-list" data-testid="tools-missing">
        <li v-for="dependency in snapshot.missing" :key="dependency">{{ dependency }}</li>
      </ul>
      <InstallLogViewer v-if="snapshot?.logPath" :log-path="snapshot.logPath" />
      <p v-if="snapshot?.status === 'disabled'" class="tool-disabled-hint">此工作階段已停用工具管理。</p>
    </div>

    <div class="tool-action-grid">
      <article class="tool-action-card">
        <div class="tool-action-icon tool-action-icon-upgrade" aria-hidden="true">↻</div>
        <h3>升級工具</h3>
        <p>更新 UxPlay、GStreamer、ADB 與 scrcpy，並更新平台套件管理器中的相依套件。</p>
        <button class="button-primary" type="button" :disabled="busy || unavailable" data-testid="tools-upgrade" @click="runAction('upgrade')">
          {{ busy && snapshot?.operation === 'upgrade' ? '正在升級…' : '升級工具' }}
        </button>
      </article>

      <article class="tool-action-card tool-action-card-reinstall">
        <div class="tool-action-icon tool-action-icon-reinstall" aria-hidden="true">⟳</div>
        <h3>清除並重新安裝</h3>
        <p>先停止投影，再清除 Neo AirPlay 專用工具與快取並重新安裝。設定、診斷記錄及系統套件會保留。</p>
        <button class="button-secondary" type="button" :disabled="busy || unavailable" data-testid="tools-reinstall" @click="requestReinstall">
          清除並重新安裝
        </button>
      </article>
    </div>

    <p class="tool-package-warning">套件更新可能會要求系統授權；Arch Linux 的 pacman 更新可能同時更新整個系統。</p>

    <div v-if="confirmingReinstall" class="tool-confirmation-backdrop" data-testid="tools-confirmation">
      <section class="tool-confirmation" role="dialog" aria-modal="true" aria-labelledby="tools-confirmation-title">
        <h3 id="tools-confirmation-title">清除並重新安裝工具？</h3>
        <p>此操作會移除 Neo AirPlay 專用工具與快取，接著重新下載、安裝並驗證。接收器設定、診斷記錄及系統套件會保留，投影服務會先停止。</p>
        <div class="tool-confirmation-actions">
          <button class="button-secondary" type="button" data-testid="tools-reinstall-cancel" @click="confirmingReinstall = false">取消</button>
          <button class="button-primary" type="button" data-testid="tools-reinstall-confirm" @click="confirmReinstall">清除並重新安裝</button>
        </div>
      </section>
    </div>
  </section>
</template>

<style scoped>
.tools-layout { max-width: 1080px; }
.tool-status {
  display: inline-flex; align-items: center; gap: 7px; padding: 7px 10px; border-radius: 18px;
  color: #788497; background: #f1f3f6; font-size: 10px; white-space: nowrap;
}
.tool-status .status-indicator { background: currentColor; }
.tool-status-installing, .tool-status-checking { color: #a97519; background: #fff5e2; }
.tool-status-ready { color: #158264; background: #e8f7f1; }
.tool-status-failed { color: #c44953; background: #fff0f1; }
.tool-status-card {
  margin-bottom: 18px; padding: 17px 19px; border: 1px solid #e5e9ef; border-radius: 12px;
  background: #fff; box-shadow: 0 5px 20px #1e2c4508;
}
.tool-status-card-failed { border-color: #f2ced0; background: #fff9f9; }
.tool-status-card-heading { display: flex; justify-content: space-between; gap: 12px; }
.tool-operation-label { color: #9aa3b0; font-size: 10px; }
.tool-status-card p { margin: 8px 0 0; color: #59667b; font-size: 12px; line-height: 1.6; overflow-wrap: anywhere; }
.tool-missing-list { margin: 8px 0 0; padding-left: 19px; color: #a3444a; font-size: 11px; line-height: 1.7; }
.tool-status-card .tool-disabled-hint { color: #8a95a5; font-size: 10px; }
.tool-action-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 17px; }
.tool-action-card {
  padding: 20px 22px; border: 1px solid #e5e9ef; border-radius: 13px; background: #fff;
  box-shadow: 0 5px 20px #1e2c4508;
}
.tool-action-card h3 { margin: 15px 0 7px; color: #2e394d; font-size: 14px; }
.tool-action-card p { min-height: 54px; margin: 0 0 17px; color: #737f90; font-size: 11px; line-height: 1.7; }
.tool-action-icon { display: grid; width: 38px; height: 38px; place-items: center; border-radius: 11px; font-size: 23px; }
.tool-action-icon-upgrade { color: #456be8; background: #edf1ff; }
.tool-action-icon-reinstall { color: #a04d52; background: #fff0f1; }
.tool-package-warning { margin: 13px 0 0; color: #8a95a5; font-size: 10px; line-height: 1.6; }
.tool-confirmation-backdrop { position: fixed; inset: 0; z-index: 5; display: grid; place-items: center; padding: 20px; background: #11182780; }
.tool-confirmation { width: min(100%, 470px); padding: 23px; border-radius: 13px; background: #fff; box-shadow: 0 20px 55px #10182740; }
.tool-confirmation h3 { margin: 0; color: #2e394d; font-size: 16px; }
.tool-confirmation p { margin: 11px 0 20px; color: #667387; font-size: 12px; line-height: 1.7; }
.tool-confirmation-actions { display: flex; justify-content: flex-end; gap: 10px; }

@media (max-width: 700px) {
  .tool-action-grid { grid-template-columns: 1fr; }
  .tool-action-card p { min-height: 0; }
}
</style>
