<script setup lang="ts">
/**
 * @file 投影工具與相依套件管理頁面元件，提供工具升級、危險操作的雙重確認重裝及安裝日誌檢視。
 */
import { computed, ref } from "vue";
import { useDependencyState } from "../composables/useDependencyState";
import { dependencyOperationLabel, dependencyStatusLabel } from "../domain";
import InstallLogViewer from "./InstallLogViewer.vue";

/** 控制清除並重新安裝確認對話框是否開啟。 */
const confirmingReinstall = ref(false);
/** 取得目前依賴工具狀態、忙碌鎖定狀態及維護操作方法。 */
const { snapshot, error, installing, unavailable, runAction } = useDependencyState();

/** 依據目前背景作業類型轉換的繁體中文操作名稱。 */
const operationLabel = computed(() => dependencyOperationLabel(snapshot.value?.operation));
/** 依據目前依賴狀態轉換的繁體中文狀態標籤。 */
const statusLabel = computed(() => dependencyStatusLabel(snapshot.value?.status));

/**
 * 僅在工具未忙碌且功能可用時開啟「清除並重新安裝」二次確認對話框。
 */
function requestReinstall(): void {
  if (!installing.value && !unavailable.value) confirmingReinstall.value = true;
}

/**
 * 關閉確認對話框並觸發清除重新安裝維護流程。
 *
 * @returns 維護請求提交完成後解析的 `Promise<void>`。
 */
async function confirmReinstall(): Promise<void> {
  confirmingReinstall.value = false;
  await runAction("reinstall");
}
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
        <button class="button-primary" type="button" :disabled="installing || unavailable" data-testid="tools-upgrade" @click="runAction('upgrade')">
          {{ installing && snapshot?.operation === 'upgrade' ? '正在升級…' : '升級工具' }}
        </button>
      </article>

      <article class="tool-action-card tool-action-card-reinstall">
        <div class="tool-action-icon tool-action-icon-reinstall" aria-hidden="true">⟳</div>
        <h3>清除並重新安裝</h3>
        <p>先停止投影，再清除 Neo AirPlay 專用工具與快取並重新安裝。設定、診斷記錄及系統套件會保留。</p>
        <button class="button-secondary" type="button" :disabled="installing || unavailable" data-testid="tools-reinstall" @click="requestReinstall">
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
  display: inline-flex; align-items: center; gap: 7px; padding: 7px 10px; border: 1px solid var(--border-rust); border-radius: 2px;
  color: var(--text-muted); background: var(--bg-bunker-elevated); font-size: 11px; white-space: nowrap;
}
.tool-status .status-indicator { background: currentColor; }
.tool-status-installing, .tool-status-checking { color: var(--hazard-amber); background: var(--hazard-amber-dim); }
.tool-status-ready { color: var(--phosphor-cyan); background: var(--phosphor-cyan-dim); }
.tool-status-failed { color: var(--crimson-alert); background: var(--crimson-alert-dim); }
.tool-status-card {
  margin-bottom: 20px; padding: 18px 20px; border: 1px solid var(--border-rust); border-left: 3px solid var(--phosphor-cyan); border-radius: 2px;
  background: var(--bg-bunker); box-shadow: inset 0 1px rgba(255, 255, 255, .025);
}
.tool-status-card-failed { border-left-color: var(--crimson-alert); background: linear-gradient(90deg, var(--crimson-alert-dim), transparent 75%), var(--bg-bunker); }
.tool-status-card-heading { display: flex; justify-content: space-between; gap: 12px; }
.tool-operation-label { color: var(--text-muted); font-family: var(--font-mono); font-size: 11px; }
.tool-status-card p { margin: 8px 0 0; color: var(--text-secondary); font-size: 12px; line-height: 1.7; overflow-wrap: anywhere; }
.tool-status-card :deep(.install-log-control) { margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--border-rust); }
.tool-missing-list { margin: 8px 0 0; padding-left: 19px; color: var(--crimson-alert); font-size: 12px; line-height: 1.7; }
.tool-status-card .tool-disabled-hint { color: var(--text-muted); font-size: 11px; }
.tool-action-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 17px; }
.tool-action-card {
  position: relative; padding: 22px; border: 1px solid var(--border-rust); border-top: 2px solid var(--hazard-amber); border-radius: 2px;
  background: var(--bg-bunker); box-shadow: inset 0 1px rgba(255, 255, 255, .025);
}
.tool-action-card::after { content: "+"; position: absolute; top: 9px; right: 12px; color: var(--text-muted); font-family: var(--font-mono); font-size: 12px; pointer-events: none; }
.tool-action-card-reinstall { border-top-color: var(--crimson-alert); }
.tool-action-card h3 { margin: 16px 0 8px; color: var(--text-primary); font-size: 15px; letter-spacing: .03em; }
.tool-action-card p { min-height: 62px; margin: 0 0 20px; color: var(--text-secondary); font-size: 12px; line-height: 1.7; }
.tool-action-icon { display: grid; width: 38px; height: 38px; place-items: center; border: 1px solid currentColor; border-radius: 2px; font-size: 23px; }
.tool-action-icon-upgrade { color: var(--hazard-amber); background: var(--hazard-amber-dim); }
.tool-action-icon-reinstall { color: var(--crimson-alert); background: var(--crimson-alert-dim); }
.tool-package-warning { margin: 16px 0 0; padding: 10px 12px; border-left: 2px solid var(--hazard-amber); color: var(--text-secondary); background: var(--hazard-amber-dim); font-size: 11px; line-height: 1.7; }
.tool-confirmation-backdrop { position: fixed; inset: 0; z-index: 5; display: grid; place-items: center; padding: 20px; background: rgba(3, 5, 7, .85); }
.tool-confirmation { width: min(100%, 470px); padding: 24px; border: 1px solid var(--border-rust); border-top: 2px solid var(--crimson-alert); border-radius: 3px; background: var(--bg-bunker); box-shadow: 0 24px 80px rgba(0, 0, 0, .65); }
.tool-confirmation h3 { margin: 0; color: var(--text-primary); font-size: 16px; }
.tool-confirmation p { margin: 12px 0 22px; color: var(--text-secondary); font-size: 12px; line-height: 1.8; }
.tool-confirmation-actions { display: flex; justify-content: flex-end; gap: 10px; }

@media (max-width: 700px) {
  .tool-action-grid { grid-template-columns: 1fr; }
  .tool-action-card p { min-height: 0; }
}
</style>
