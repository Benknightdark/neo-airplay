<script setup lang="ts">
/**
 * @file 投影工具與相依套件管理頁面元件：依目前作業系統呈現工具來源、線上版本爬取／手動版本安裝、重裝確認與安裝日誌。
 */
import { computed, reactive, ref, watch } from "vue";
import { useDependencyState } from "../composables/useDependencyState";
import { useToolCatalog } from "../composables/useToolCatalog";
import {
  dependencyOperationLabel,
  dependencyStatusLabel,
  platformBadgeLabel,
  requiresUnverifiedConfirmation,
  validateOptionalSha256Input,
  validateToolVersionInput,
  type ManagedToolInfo,
  type ToolId,
} from "../domain";
import InstallLogViewer from "./InstallLogViewer.vue";

/** 單一工具卡片的表單互動狀態。 */
interface ToolSelectionDraft {
  mode: "select" | "manual";
  selectedVersion: string;
  manualVersion: string;
  sha256: string;
}

/** 控制清除並重新安裝確認對話框是否開啟。 */
const confirmingReinstall = ref(false);
/** 控制僅清除所有已安裝工具確認對話框是否開啟。 */
const confirmingClear = ref(false);
/** 暫存待使用者明確確認（未附 SHA-256 的非預設 UxPlay）安裝項目。 */
const pendingUnverifiedTool = ref<{ tool: ManagedToolInfo; version: string } | null>(null);

/** 取得目前依賴工具狀態、忙碌鎖定狀態及維護操作方法。 */
const { snapshot, error, installing, unavailable, retry, runAction, installToolVersion } =
  useDependencyState();

const dependencyStatus = computed(() => snapshot.value?.status);
const {
  catalog,
  fetchingRemote,
  catalogError,
  hasCustomVersions,
  loadCatalog,
  fetchRemoteVersions,
  resetToolVersions,
} = useToolCatalog({ dependencyStatus });

/** 依據目前背景作業類型轉換的繁體中文操作名稱。 */
const operationLabel = computed(() => dependencyOperationLabel(snapshot.value?.operation));
/** 依據目前依賴狀態轉換的繁體中文狀態標籤。 */
const statusLabel = computed(() => dependencyStatusLabel(snapshot.value?.status));

/** 各工具之版本選擇／手動輸入表單狀態。 */
const drafts = reactive<Record<ToolId, ToolSelectionDraft>>({
  uxplay: { mode: "select", selectedVersion: "", manualVersion: "", sha256: "" },
  scrcpy: { mode: "select", selectedVersion: "", manualVersion: "", sha256: "" },
  gstreamer: { mode: "select", selectedVersion: "", manualVersion: "", sha256: "" },
});

watch(
  catalog,
  (nextCatalog) => {
    if (!nextCatalog) return;
    for (const tool of nextCatalog.tools) {
      if (tool.versionControl !== "selectable") continue;
      const draft = drafts[tool.id];
      const desired = tool.selectedVersion || tool.defaultVersion;
      const hasInList = tool.availableVersions.some((option) => option.version === desired);
      if (!draft.selectedVersion) {
        if (hasInList) {
          draft.mode = "select";
          draft.selectedVersion = desired;
        } else {
          draft.mode = "manual";
          draft.selectedVersion = tool.availableVersions[0]?.version ?? tool.defaultVersion;
        }
      } else if (!tool.availableVersions.some((option) => option.version === draft.selectedVersion)) {
        draft.selectedVersion = tool.availableVersions[0]?.version ?? tool.defaultVersion;
      }
      if (!draft.manualVersion) {
        draft.manualVersion = desired;
      }
    }
  },
  { immediate: true },
);

/**
 * 取得指定工具表單目前選用或輸入的目標版本字串。
 */
function effectiveVersion(tool: ManagedToolInfo): string {
  const draft = drafts[tool.id];
  return draft.mode === "select" ? draft.selectedVersion.trim() : draft.manualVersion.trim();
}

/**
 * 檢查指定工具目前版本與 SHA-256 輸入欄位之驗證錯誤訊息。
 */
function versionErrorFor(tool: ManagedToolInfo): string | null {
  return validateToolVersionInput(tool.id, effectiveVersion(tool));
}

/**
 * 檢查指定工具目前選填 SHA-256 輸入欄位之格式錯誤訊息。
 */
function shaErrorFor(tool: ManagedToolInfo): string | null {
  return validateOptionalSha256Input(drafts[tool.id].sha256);
}

/**
 * 觸發安裝指定工具與版本；若屬於未附 SHA-256 的非預設 UxPlay，先行顯示確認視窗。
 */
async function submitToolInstall(tool: ManagedToolInfo): Promise<void> {
  if (installing.value || unavailable.value) return;
  const version = effectiveVersion(tool);
  if (validateToolVersionInput(tool.id, version) || shaErrorFor(tool)) return;

  const sha256 = drafts[tool.id].sha256.trim();
  if (requiresUnverifiedConfirmation(tool, version, sha256)) {
    pendingUnverifiedTool.value = { tool, version };
    return;
  }

  await installToolVersion({
    tool: tool.id,
    version,
    sha256: sha256.length > 0 ? sha256 : null,
    allowUnverified: false,
  });
}

/**
 * 使用者確認後，送出未附帶 SHA-256 的自訂 UxPlay HTTPS 安裝請求。
 */
async function confirmUnverifiedInstall(): Promise<void> {
  const pending = pendingUnverifiedTool.value;
  pendingUnverifiedTool.value = null;
  if (!pending) return;

  await installToolVersion({
    tool: pending.tool.id,
    version: pending.version,
    sha256: null,
    allowUnverified: true,
  });
}

/**
 * 僅在工具未忙碌且功能可用時開啟「清除並重新安裝」二次確認對話框。
 */
function requestReinstall(): void {
  if (!installing.value && !unavailable.value) confirmingReinstall.value = true;
}

/**
 * 關閉確認對話框並觸發清除重新安裝維護流程。
 */
async function confirmReinstall(): Promise<void> {
  confirmingReinstall.value = false;
  await runAction("reinstall");
}

/**
 * 僅在工具未忙碌且功能可用時開啟「清除工具」二次確認對話框。
 */
function requestClear(): void {
  if (!installing.value && !unavailable.value) confirmingClear.value = true;
}

/**
 * 關閉確認對話框並觸發僅清除已安裝工具與快取流程。
 */
async function confirmClear(): Promise<void> {
  confirmingClear.value = false;
  const previousStatus = snapshot.value?.status;
  await runAction("clear");
  if (
    snapshot.value?.status !== "installing" &&
    snapshot.value?.status === previousStatus
  ) {
    await loadCatalog();
  }
}

/**
 * 格式化爬取時間戳記為台灣 24 小時制簡短時間顯示。
 */
function formatFetchedAt(timestamp: string | null | undefined): string {
  if (!timestamp) return "尚未線上爬取（使用內建清單）";
  const date = new Date(timestamp);
  if (Number.isNaN(date.valueOf())) return timestamp;
  return new Intl.DateTimeFormat("zh-TW", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);
}
</script>

<template>
  <section class="page-content tools-layout" aria-label="工具管理" data-testid="tools-page">
    <div class="section-intro">
      <div>
        <p class="eyebrow">投影工具</p>
        <h2>管理工具與相依套件</h2>
        <p class="muted-copy">依目前作業系統自動區分來源，並支援爬取線上版本或手動輸入版本安裝。</p>
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
      <div v-if="snapshot?.status === 'failed' && !unavailable" class="tool-status-actions">
        <button
          class="button-secondary"
          type="button"
          :disabled="installing"
          data-testid="tools-retry-install"
          @click="retry"
        >
          重新檢查並安裝
        </button>
      </div>
      <p v-if="snapshot?.status === 'disabled'" class="tool-disabled-hint">此工作階段已停用工具管理。</p>
    </div>

    <section v-if="catalog" class="tool-platform-banner" data-testid="tools-platform-banner" aria-label="作業系統環境與線上版本">
      <div class="tool-platform-summary">
        <div class="tool-platform-badges">
          <span class="os-badge" :class="`os-badge-${catalog.os}`" data-testid="tools-platform-os">
            {{ platformBadgeLabel(catalog.os) }} · {{ catalog.platformLabel }}
          </span>
          <span class="pkg-badge">{{ catalog.packageManager }}</span>
        </div>
        <p class="tool-platform-notice">{{ catalog.platformNotice }}</p>
        <p class="tool-remote-meta">
          線上版本清單更新：{{ formatFetchedAt(catalog.remoteFetchedAt) }}
          <span v-if="catalog.remoteError || catalogError" class="tool-remote-error" data-testid="tools-remote-error">
            （部分來源暫時無法連線：{{ catalogError || catalog.remoteError }}）
          </span>
        </p>
      </div>
      <div class="tool-platform-actions">
        <button
          class="button-secondary"
          type="button"
          :disabled="fetchingRemote"
          data-testid="tools-fetch-versions"
          @click="fetchRemoteVersions"
        >
          {{ fetchingRemote ? "正在爬取版本…" : "重新抓取線上版本" }}
        </button>
        <button
          v-if="hasCustomVersions"
          class="button-secondary"
          type="button"
          :disabled="installing || unavailable"
          data-testid="tools-reset-versions"
          @click="resetToolVersions"
        >
          還原推薦版本
        </button>
      </div>
    </section>

    <div v-if="catalog" class="tool-catalog-grid" data-testid="tools-catalog-list">
      <article
        v-for="tool in catalog.tools"
        :key="tool.id"
        class="tool-version-card"
        :class="{ 'tool-version-card-readonly': tool.versionControl === 'packageManager' }"
        :data-testid="`tool-card-${tool.id}`"
      >
        <div class="tool-version-header">
          <div>
            <h3>{{ tool.name }}</h3>
            <span class="tool-source-label">{{ tool.sourceLabel }}</span>
          </div>
          <div class="tool-version-pills">
            <span class="version-pill" :class="{ 'version-pill-active': Boolean(tool.installedVersion) }">
              已安裝：{{ tool.installedVersion ? `v${tool.installedVersion}` : "尚未偵測" }}
            </span>
            <span v-if="tool.latestRemoteVersion" class="version-pill version-pill-remote">
              線上最新：v{{ tool.latestRemoteVersion }}
            </span>
            <span v-if="tool.selectedVersion" class="version-pill version-pill-pinned">
              已鎖定：{{ tool.selectedVersion }}
            </span>
          </div>
        </div>

        <p class="tool-version-summary">{{ tool.summary }}</p>

        <div v-if="tool.bundledComponents.length" class="tool-bundled-row">
          <span v-for="component in tool.bundledComponents" :key="component" class="bundled-chip">
            {{ component }}
          </span>
        </div>

        <template v-if="tool.versionControl === 'selectable'">
          <div class="tool-mode-tabs" role="group" :aria-label="`${tool.name} 版本輸入方式`">
            <button
              type="button"
              class="mode-tab"
              :class="{ 'mode-tab-active': drafts[tool.id].mode === 'select' }"
              :data-testid="`tool-mode-select-${tool.id}`"
              @click="drafts[tool.id].mode = 'select'"
            >
              線上版本清單
            </button>
            <button
              type="button"
              class="mode-tab"
              :class="{ 'mode-tab-active': drafts[tool.id].mode === 'manual' }"
              :data-testid="`tool-mode-manual-${tool.id}`"
              @click="drafts[tool.id].mode = 'manual'"
            >
              手動輸入版本
            </button>
          </div>

          <div class="tool-version-controls">
            <label v-if="drafts[tool.id].mode === 'select'" class="field-block">
              <span class="field-label">選擇發行版本</span>
              <select
                v-model="drafts[tool.id].selectedVersion"
                class="tool-input"
                :disabled="installing || unavailable"
                :data-testid="`tool-version-select-${tool.id}`"
              >
                <option v-for="option in tool.availableVersions" :key="`${option.version}-${option.reference}`" :value="option.version">
                  {{ option.label }}{{ option.publishedAt ? ` · ${option.publishedAt}` : "" }}{{ option.prerelease ? "（預覽）" : "" }}
                </option>
              </select>
            </label>

            <label v-else class="field-block">
              <span class="field-label">輸入版本號{{ tool.id === "uxplay" ? "或 40 碼 Commit SHA" : "" }}</span>
              <input
                v-model="drafts[tool.id].manualVersion"
                type="text"
                class="tool-input"
                :disabled="installing || unavailable"
                :placeholder="tool.id === 'uxplay' ? '例如 1.73.7 或 40 碼 commit SHA' : '例如 5.0.1 或 v4.1'"
                :data-testid="`tool-version-input-${tool.id}`"
              />
            </label>

            <button
              class="button-primary"
              type="button"
              :disabled="installing || unavailable || Boolean(versionErrorFor(tool)) || Boolean(shaErrorFor(tool))"
              :data-testid="`tool-install-${tool.id}`"
              @click="submitToolInstall(tool)"
            >
              安裝此版本
            </button>
          </div>

          <p v-if="versionErrorFor(tool)" class="tool-input-error" :data-testid="`tool-version-error-${tool.id}`">
            {{ versionErrorFor(tool) }}
          </p>

          <details v-if="tool.supportsManualSha256" class="tool-advanced-details">
            <summary>進階校驗選項（選填 SHA-256）</summary>
            <label class="field-block">
              <span class="field-label">
                SHA-256 校驗碼（{{ tool.autoVerifiesChecksum ? "留空將自動比對官方 SHA256SUMS.txt" : "選填 64 碼十六進位" }}）
              </span>
              <input
                v-model="drafts[tool.id].sha256"
                type="text"
                class="tool-input"
                :disabled="installing || unavailable"
                placeholder="選填 64 碼十六進位 SHA-256"
                :data-testid="`tool-sha256-input-${tool.id}`"
              />
            </label>
            <p v-if="shaErrorFor(tool)" class="tool-input-error">{{ shaErrorFor(tool) }}</p>
          </details>
        </template>

        <p v-if="tool.note" class="tool-version-note">{{ tool.note }}</p>
      </article>
    </div>

    <div class="tool-action-grid">
      <article class="tool-action-card">
        <div class="tool-action-icon tool-action-icon-upgrade" aria-hidden="true">↻</div>
        <h3>升級工具</h3>
        <p>更新平台套件管理器中的相依套件，並依目前設定版本重建／部署 UxPlay、scrcpy 與 ADB。</p>
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

      <article class="tool-action-card tool-action-card-clear">
        <div class="tool-action-icon tool-action-icon-clear" aria-hidden="true">✕</div>
        <h3>清除工具</h3>
        <p>先停止投影，再移除所有已安裝的 Neo AirPlay 專用工具、GStreamer 快取與自訂版本設定（不重新安裝）。系統套件、設定與記錄檔會保留。</p>
        <button class="button-secondary button-danger-outline" type="button" :disabled="installing || unavailable" data-testid="tools-clear" @click="requestClear">
          {{ installing && snapshot?.operation === 'clear' ? '正在清除…' : '清除工具' }}
        </button>
      </article>
    </div>

    <p v-if="catalog?.platformWarning" class="tool-package-warning" data-testid="tools-platform-warning">
      {{ catalog.platformWarning }}
    </p>

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

    <div v-if="confirmingClear" class="tool-confirmation-backdrop" data-testid="tools-clear-confirmation">
      <section class="tool-confirmation" role="dialog" aria-modal="true" aria-labelledby="tools-clear-confirmation-title">
        <h3 id="tools-clear-confirmation-title">清除所有已安裝工具？</h3>
        <p>
          此操作會先停止投影服務，並移除 <code>engine/</code> 內的 Neo AirPlay 專用工具、GStreamer 快取、過渡性安裝標記及自訂版本偏好，且不會自動重新安裝。系統套件、接收器設定與安裝／診斷記錄會保留。
        </p>
        <div class="tool-confirmation-actions">
          <button class="button-secondary" type="button" data-testid="tools-clear-cancel" @click="confirmingClear = false">取消</button>
          <button class="button-primary button-danger" type="button" data-testid="tools-clear-confirm" @click="confirmClear">確認清除工具</button>
        </div>
      </section>
    </div>

    <div v-if="pendingUnverifiedTool" class="tool-confirmation-backdrop" data-testid="tools-unverified-confirmation">
      <section class="tool-confirmation tool-confirmation-amber" role="dialog" aria-modal="true" aria-labelledby="tools-unverified-title">
        <h3 id="tools-unverified-title">安裝未附帶雜湊值的 UxPlay {{ pendingUnverifiedTool.version }}？</h3>
        <p>
          您所選的非內建推薦 UxPlay 版本未填入 SHA-256 校驗碼。確認後將透過嚴格 HTTPS（TLS 1.2+）自官方 GitHub 原始碼倉庫（<code>https://github.com/FDH2/UxPlay</code>）下載並本機編譯。
        </p>
        <div class="tool-confirmation-actions">
          <button class="button-secondary" type="button" data-testid="tools-unverified-cancel" @click="pendingUnverifiedTool = null">取消</button>
          <button class="button-primary" type="button" data-testid="tools-unverified-confirm" @click="confirmUnverifiedInstall">確認並透過 HTTPS 安裝</button>
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
.tool-status-actions { margin-top: 12px; }
.tool-missing-list { margin: 8px 0 0; padding-left: 19px; color: var(--crimson-alert); font-size: 12px; line-height: 1.7; }
.tool-status-card .tool-disabled-hint { color: var(--text-muted); font-size: 11px; }

.tool-platform-banner {
  display: flex; flex-wrap: wrap; align-items: flex-start; justify-content: space-between; gap: 16px;
  margin-bottom: 20px; padding: 16px 20px; border: 1px solid var(--border-rust); border-radius: 2px; background: var(--bg-bunker);
}
.tool-platform-summary { flex: 1 1 520px; min-width: 0; }
.tool-platform-badges { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
.os-badge, .pkg-badge {
  display: inline-flex; align-items: center; padding: 4px 9px; border: 1px solid var(--border-rust); border-radius: 2px;
  font-family: var(--font-mono); font-size: 11px; font-weight: 600;
}
.os-badge-macos { color: var(--phosphor-cyan); border-color: rgba(0, 229, 255, .35); background: var(--phosphor-cyan-dim); }
.os-badge-windows { color: var(--hazard-amber); border-color: rgba(255, 157, 0, .35); background: var(--hazard-amber-dim); }
.os-badge-linux { color: var(--text-primary); background: var(--bg-bunker-elevated); }
.pkg-badge { color: var(--text-secondary); background: var(--bg-ash); }
.tool-platform-notice { margin: 10px 0 4px; color: var(--text-secondary); font-size: 12px; line-height: 1.7; }
.tool-remote-meta { margin: 0; color: var(--text-muted); font-size: 11px; line-height: 1.6; }
.tool-remote-error { color: var(--hazard-amber); }
.tool-platform-actions { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; }

.tool-catalog-grid { display: grid; gap: 16px; margin-bottom: 20px; }
.tool-version-card {
  padding: 20px; border: 1px solid var(--border-rust); border-left: 3px solid var(--phosphor-cyan); border-radius: 2px;
  background: var(--bg-bunker);
}
.tool-version-card-readonly { border-left-color: var(--border-rust); }
.tool-version-header { display: flex; flex-wrap: wrap; align-items: flex-start; justify-content: space-between; gap: 12px; }
.tool-version-header h3 { margin: 0; color: var(--text-primary); font-size: 15px; }
.tool-source-label { display: inline-block; margin-top: 4px; color: var(--text-muted); font-family: var(--font-mono); font-size: 11px; }
.tool-version-pills { display: flex; flex-wrap: wrap; gap: 8px; }
.version-pill {
  padding: 4px 8px; border: 1px solid var(--border-rust); border-radius: 2px;
  color: var(--text-muted); background: var(--bg-ash); font-family: var(--font-mono); font-size: 11px;
}
.version-pill-active { color: var(--phosphor-cyan); border-color: rgba(0, 229, 255, .35); background: var(--phosphor-cyan-dim); }
.version-pill-remote { color: var(--text-secondary); }
.version-pill-pinned { color: var(--hazard-amber); border-color: rgba(255, 157, 0, .35); background: var(--hazard-amber-dim); }
.tool-version-summary { margin: 10px 0 0; color: var(--text-secondary); font-size: 12px; line-height: 1.65; }
.tool-bundled-row { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 10px; }
.bundled-chip {
  padding: 3px 8px; border: 1px dashed var(--border-rust); border-radius: 2px;
  color: var(--text-secondary); font-family: var(--font-mono); font-size: 11px;
}
.tool-mode-tabs { display: inline-flex; margin-top: 14px; border: 1px solid var(--border-rust); border-radius: 2px; overflow: hidden; }
.mode-tab {
  padding: 7px 12px; border: 0; color: var(--text-muted); background: var(--bg-ash); font-size: 11px; font-weight: 600;
}
.mode-tab-active { color: var(--phosphor-cyan); background: var(--phosphor-cyan-dim); }
.tool-version-controls { display: flex; flex-wrap: wrap; align-items: flex-end; gap: 12px; margin-top: 12px; }
.field-block { display: grid; gap: 6px; flex: 1 1 280px; min-width: 0; }
.field-label { color: var(--text-muted); font-size: 11px; }
.tool-input {
  width: 100%; padding: 10px 12px; border: 1px solid var(--border-rust); border-radius: 2px;
  color: var(--text-primary); background: var(--bg-ash); font-family: var(--font-mono); font-size: 12px;
}
.tool-input-error { margin: 8px 0 0; color: var(--crimson-alert); font-size: 11px; }
.tool-advanced-details { margin-top: 12px; color: var(--text-muted); font-size: 11px; }
.tool-advanced-details summary { cursor: pointer; user-select: none; }
.tool-advanced-details .field-block { margin-top: 10px; }
.tool-version-note { margin: 12px 0 0; color: var(--text-muted); font-size: 11px; line-height: 1.6; }

.tool-action-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 17px; }
.tool-action-card {
  position: relative; display: flex; flex-direction: column; align-items: flex-start; padding: 22px; border: 1px solid var(--border-rust); border-top: 2px solid var(--hazard-amber); border-radius: 2px;
  background: var(--bg-bunker); box-shadow: inset 0 1px rgba(255, 255, 255, .025);
}
.tool-action-card::after { content: "+"; position: absolute; top: 9px; right: 12px; color: var(--text-muted); font-family: var(--font-mono); font-size: 12px; pointer-events: none; }
.tool-action-card-reinstall, .tool-action-card-clear { border-top-color: var(--crimson-alert); }
.tool-action-card h3 { margin: 16px 0 8px; color: var(--text-primary); font-size: 15px; letter-spacing: .03em; }
.tool-action-card p { margin: 0 0 20px; color: var(--text-secondary); font-size: 12px; line-height: 1.7; }
.tool-action-card > button { margin-top: auto; }
.tool-action-icon { display: grid; width: 38px; height: 38px; place-items: center; border: 1px solid currentColor; border-radius: 2px; font-size: 23px; }
.tool-action-icon-upgrade { color: var(--hazard-amber); background: var(--hazard-amber-dim); }
.tool-action-icon-reinstall, .tool-action-icon-clear { color: var(--crimson-alert); background: var(--crimson-alert-dim); }
.button-danger-outline { border-color: rgba(255, 87, 87, .45); border-left-color: var(--crimson-alert); color: var(--crimson-alert); }
.button-danger-outline:hover:not(:disabled) { border-color: var(--crimson-alert); color: var(--bg-ash); background: var(--crimson-alert); }
.button-danger { border-color: var(--crimson-alert); border-left-color: var(--crimson-alert); color: var(--crimson-alert); background: var(--crimson-alert-dim); }
.button-danger:hover:not(:disabled) { border-color: var(--crimson-alert); color: var(--bg-ash); background: var(--crimson-alert); }
.tool-package-warning { margin: 16px 0 0; padding: 10px 12px; border-left: 2px solid var(--hazard-amber); color: var(--text-secondary); background: var(--hazard-amber-dim); font-size: 11px; line-height: 1.7; }
.tool-confirmation-backdrop { position: fixed; inset: 0; z-index: 5; display: grid; place-items: center; padding: 20px; background: rgba(3, 5, 7, .85); }
.tool-confirmation { width: min(100%, 470px); padding: 24px; border: 1px solid var(--border-rust); border-top: 2px solid var(--crimson-alert); border-radius: 3px; background: var(--bg-bunker); box-shadow: 0 24px 80px rgba(0, 0, 0, .65); }
.tool-confirmation-amber { border-top-color: var(--hazard-amber); }
.tool-confirmation h3 { margin: 0; color: var(--text-primary); font-size: 16px; }
.tool-confirmation p { margin: 12px 0 22px; color: var(--text-secondary); font-size: 12px; line-height: 1.8; }
.tool-confirmation code { font-family: var(--font-mono); color: var(--phosphor-cyan); }
.tool-confirmation-actions { display: flex; justify-content: flex-end; gap: 10px; }

@media (max-width: 960px) {
  .tool-action-grid { grid-template-columns: 1fr; }
}
</style>
