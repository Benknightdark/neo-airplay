<script setup lang="ts">
/**
 * @file 投影依賴準備狀態提示橫幅元件，負責呈現自動安裝進度、失敗日誌檢視入口與手動重試按鈕。
 */
import { computed } from "vue";
import { useDependencyState } from "../composables/useDependencyState";
import { shouldDisplayDependencyPanel } from "../domain";
import InstallLogViewer from "./InstallLogViewer.vue";

/** 對父層發出的忙碌狀態變更事件，用於於背景安裝期間停用投影啟動控制。 */
const emit = defineEmits<{
  /** 當檢查或背景安裝切換為忙碌／閒置時發出布林值。 */
  (event: "busy", value: boolean): void;
}>();

/** 訂閱依賴工具狀態並將忙碌變化轉發至父元件。 */
const { snapshot, error, installing, retry } = useDependencyState({
  onBusyChange: (value) => emit("busy", value),
});

/** 判斷目前是否應在介面上呈現此依賴狀態橫幅。 */
const visible = computed(() =>
  shouldDisplayDependencyPanel(snapshot.value, installing.value, error.value),
);
</script>

<template>
  <section
    v-if="visible"
    class="dependency-panel"
    :class="{ 'dependency-failed': error || snapshot?.status === 'failed' }"
    aria-label="投影工具準備"
    data-testid="dependency-panel"
  >
    <p role="status">{{ error || snapshot?.message || "正在檢查並安裝投影工具…" }}</p>
    <template v-if="snapshot?.status === 'failed' || error">
      <InstallLogViewer v-if="snapshot?.logPath" :log-path="snapshot.logPath" />
      <button class="button-secondary" type="button" :disabled="installing" data-testid="dependency-retry" @click="retry">重新檢查並安裝</button>
    </template>
  </section>
</template>

<style scoped>
.dependency-panel {
  margin: 16px 0 0; padding: 14px 18px; border: 1px solid var(--border-rust); border-left: 3px solid var(--phosphor-cyan);
  border-radius: 2px; background: linear-gradient(90deg, var(--phosphor-cyan-dim), transparent 75%), var(--bg-bunker); color: var(--text-primary);
}
.dependency-panel p { margin: 0 0 8px; font-size: 12px; line-height: 1.7; }
.dependency-panel > .button-secondary { margin-top: 12px; }
.dependency-failed { border-left-color: var(--hazard-amber); background: linear-gradient(90deg, var(--hazard-amber-dim), transparent 75%), var(--bg-bunker); }
</style>
