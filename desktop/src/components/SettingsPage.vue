<script setup lang="ts">
import { reactive, watch } from "vue";
import type { ReceiverSettings } from "../types";

const props = defineProps<{
  settings: ReceiverSettings;
  rendererOptions: string[];
  busy: boolean;
}>();

const emit = defineEmits<{
  (event: "save", settings: ReceiverSettings): void;
}>();

const form = reactive<ReceiverSettings>({ ...props.settings });
watch(
  [
    () => props.settings.receiverName,
    () => props.settings.renderer,
    () => props.settings.fullscreen,
    () => props.settings.startOnLogin,
  ],
  ([receiverName, renderer, fullscreen, startOnLogin]) => {
    Object.assign(form, { receiverName, renderer, fullscreen, startOnLogin });
  },
);

function submit(): void {
  emit("save", { ...form, receiverName: form.receiverName.trim() });
}
</script>

<template>
  <section class="page-content settings-layout" aria-label="接收器設定" data-testid="settings-page">
    <div class="section-intro">
      <p class="eyebrow">個人化</p>
      <h2>接收器設定</h2>
      <p class="muted-copy">設定會保存在目前使用者的應用程式資料目錄。</p>
    </div>

    <form class="settings-card" @submit.prevent="submit">
      <div class="settings-section">
        <h3>AirPlay</h3>
        <p class="settings-description">這些選項會套用至下一次啟動的接收器。</p>
        <label class="field-label" for="receiver-name">接收器名稱</label>
        <input id="receiver-name" v-model="form.receiverName" class="text-input" maxlength="64" required data-testid="receiver-name" />
        <p class="field-hint">這個名稱會顯示在 Apple 裝置的螢幕鏡像清單中。</p>

        <label class="field-label" for="renderer">影片輸出方式</label>
        <select id="renderer" v-model="form.renderer" class="text-input" data-testid="renderer-select">
          <option v-for="renderer in rendererOptions" :key="renderer" :value="renderer">{{ renderer }}</option>
        </select>

        <label class="switch-row">
          <span><strong>全螢幕播放</strong><small>啟動 AirPlay 時使用全螢幕視窗</small></span>
          <input v-model="form.fullscreen" type="checkbox" data-testid="fullscreen-toggle" />
        </label>
      </div>

      <div class="settings-section settings-section-last">
        <h3>啟動方式</h3>
        <label class="switch-row">
          <span><strong>登入後自動啟動</strong><small>使用目前作業系統的登入啟動設定</small></span>
          <input v-model="form.startOnLogin" type="checkbox" data-testid="autostart-toggle" />
        </label>
      </div>

      <div class="form-footer">
        <span class="field-hint">新設定會立即儲存。</span>
        <button class="button-primary" type="submit" :disabled="busy || !form.receiverName.trim()" data-testid="settings-save">儲存設定</button>
      </div>
    </form>
  </section>
</template>
