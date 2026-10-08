<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import DashboardPage from "./components/DashboardPage.vue";
import DiagnosticsPage from "./components/DiagnosticsPage.vue";
import SettingsPage from "./components/SettingsPage.vue";
import { useAppState } from "./composables/useAppState";

type PageKey = "overview" | "settings" | "diagnostics";

const pages: { key: PageKey; label: string; icon: string }[] = [
  { key: "overview", label: "總覽", icon: "◫" },
  { key: "settings", label: "設定", icon: "⚙" },
  { key: "diagnostics", label: "診斷記錄", icon: "≡" },
];

const activePage = ref<PageKey>("overview");
const { dashboard, busy, error, refresh, startAirplay, stopAirplay, refreshDevices, startAndroid, stopAndroid, saveSettings } = useAppState();
const pageTitle = computed(() => pages.find((page) => page.key === activePage.value)?.label ?? "總覽");

onMounted(() => {
  void refresh(true);
});
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand-lockup">
        <div class="brand-mark" aria-hidden="true">N</div>
        <div>
          <p class="brand-name">Neo AirPlay</p>
          <p class="brand-caption">跨裝置投影接收器</p>
        </div>
      </div>

      <nav class="primary-nav" aria-label="主要導覽">
        <button
          v-for="page in pages"
          :key="page.key"
          class="nav-item"
          :class="{ 'nav-item-active': activePage === page.key }"
          :aria-current="activePage === page.key ? 'page' : undefined"
          :data-testid="`nav-${page.key}`"
          @click="activePage = page.key"
        >
          <span class="nav-icon" aria-hidden="true">{{ page.icon }}</span>
          <span>{{ page.label }}</span>
        </button>
      </nav>

      <div class="sidebar-footer">
        <span class="connection-dot" aria-hidden="true"></span>
        <span>本機接收器</span>
        <span class="version-label">0.1.0</span>
      </div>
    </aside>

    <main class="main-panel">
      <header class="topbar">
        <div>
          <p class="eyebrow">控制中心</p>
          <h1>{{ pageTitle }}</h1>
        </div>
        <button class="icon-button" type="button" aria-label="重新整理狀態" data-testid="refresh-dashboard" @click="refresh(true)">
          <span aria-hidden="true">⟳</span>
        </button>
      </header>

      <div v-if="error" class="alert-banner" role="alert" data-testid="app-error">{{ error }}</div>
      <div v-if="!dashboard" class="loading-state" role="status">正在載入接收器狀態…</div>

      <DashboardPage
        v-else-if="activePage === 'overview'"
        :dashboard="dashboard"
        :busy="busy"
        @start-airplay="startAirplay"
        @stop-airplay="stopAirplay"
        @refresh-devices="refreshDevices"
        @start-android="startAndroid"
        @stop-android="stopAndroid"
      />
      <SettingsPage
        v-else-if="activePage === 'settings' && dashboard"
        :settings="dashboard.settings"
        :renderer-options="dashboard.rendererOptions"
        :busy="busy"
        @save="saveSettings"
      />
      <DiagnosticsPage v-else-if="activePage === 'diagnostics' && dashboard" :logs="dashboard.logs" />
    </main>
  </div>
</template>
