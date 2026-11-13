<script setup lang="ts">
/**
 * @file 診斷記錄檢視頁面元件，提供依日誌嚴重等級篩選並以最新在前的順序呈現系統活動紀錄。
 */
import { computed, ref } from "vue";
import {
  filterAndReverseLogs,
  localTime,
  logLabel,
  type AppLogEntry,
  type LogFilter,
} from "../domain";

/** 元件接收的唯讀 Props。 */
const props = defineProps<{
  /** 後端回傳的近期應用程式診斷日誌清單。 */
  logs: AppLogEntry[];
}>();

/** 使用者選擇的日誌等級篩選條件（預設顯示全部）。 */
const filter = ref<LogFilter>("all");
/** 依目前篩選條件過濾並反轉為新紀錄置頂的日誌列表。 */
const filteredLogs = computed(() => filterAndReverseLogs(props.logs, filter.value));
</script>

<template>
  <section class="page-content diagnostics-layout" aria-label="診斷記錄" data-testid="diagnostics-page">
    <div class="section-intro diagnostics-intro">
      <div>
        <p class="eyebrow">系統活動</p>
        <h2>診斷記錄</h2>
        <p class="muted-copy">最近 {{ logs.length }} 筆記錄會顯示於此，完整記錄保存在本機。</p>
      </div>
      <label class="filter-control">
        <span>篩選</span>
        <select v-model="filter" aria-label="篩選記錄" data-testid="log-filter">
          <option value="all">全部記錄</option>
          <option value="information">資訊</option>
          <option value="warning">注意</option>
          <option value="error">錯誤</option>
        </select>
      </label>
    </div>

    <div class="log-card" data-testid="log-list">
      <div v-if="filteredLogs.length === 0" class="empty-state">目前沒有符合條件的記錄。</div>
      <div v-for="(entry, index) in filteredLogs" :key="`${entry.timestamp}-${index}`" class="log-entry" :class="`log-${entry.level}`">
        <time>{{ localTime(entry.timestamp) }}</time>
        <span class="log-level">{{ logLabel(entry.level) }}</span>
        <p>{{ entry.message }}</p>
      </div>
    </div>
  </section>
</template>
