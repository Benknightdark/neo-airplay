<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { getDependencies, installDependencies } from "../api";
import InstallLogViewer from "./InstallLogViewer.vue";
import type { DependencySnapshot } from "../types";

const emit = defineEmits<{ (event: "busy", value: boolean): void }>();
const snapshot = ref<DependencySnapshot | null>(null);
const error = ref("");
const requesting = ref(false);
const installing = computed(() => requesting.value || snapshot.value?.status === "checking" || snapshot.value?.status === "installing");
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
    emit("busy", installing.value);
    if (next.status === "disabled") stopPolling();
  } catch (reason) {
    if (!disposed && generation === requestGeneration) {
      error.value = reason instanceof Error ? reason.message : String(reason);
      emit("busy", installing.value);
    }
  } finally {
    polling = false;
  }
}

async function retry(): Promise<void> {
  if (disposed || installing.value) return;
  const generation = ++requestGeneration;
  requesting.value = true;
  error.value = "";
  emit("busy", true);
  try {
    const next = await installDependencies();
    if (disposed || generation !== requestGeneration) return;
    snapshot.value = next;
    if (next.status === "disabled") stopPolling();
  } catch (reason) {
    if (!disposed && generation === requestGeneration) {
      error.value = reason instanceof Error ? reason.message : String(reason);
    }
  } finally {
    if (!disposed && generation === requestGeneration) {
      requesting.value = false;
      emit("busy", installing.value);
    }
  }
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
  <section v-if="installing || error || (snapshot && snapshot.status !== 'disabled')" class="dependency-panel" :class="{ 'dependency-failed': error || snapshot?.status === 'failed' }" aria-label="投影工具準備" data-testid="dependency-panel">
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
