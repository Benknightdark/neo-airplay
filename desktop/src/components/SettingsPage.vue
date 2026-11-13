<script setup lang="ts">
/**
 * @file 接收器設定頁面元件，管理接收器名稱、視訊輸出方式、全螢幕與開機自啟表單與送出驗證。
 */
import { computed, ref, watch } from "vue";
import { isReceiverNameValid, type ReceiverSettings } from "../domain";

/** 元件接收的唯讀 Props。 */
const props = defineProps<{
  /** 目前生效的接收器設定快照，作為表單欄位初始與同步來源。 */
  settings: ReceiverSettings;
  /** 目前作業系統可用的視訊輸出渲染器選項清單。 */
  rendererOptions: string[];
  /** 全域是否正處於忙碌鎖定狀態（防止重複送出）。 */
  busy: boolean;
}>();

/** 元件對父層發出的表單送出事件。 */
const emit = defineEmits<{
  /** 使用者送出已驗證的表單設定時觸發，攜帶整理後的 `ReceiverSettings`。 */
  (event: "save", settings: ReceiverSettings): void;
}>();

/** 表單欄位：AirPlay 接收器名稱雙向綁定值。 */
const receiverName = ref(props.settings.receiverName);
/** 表單欄位：選用視訊渲染器雙向綁定值。 */
const renderer = ref(props.settings.renderer);
/** 表單欄位：是否全螢幕播放雙向綁定值。 */
const fullscreen = ref(props.settings.fullscreen);
/** 表單欄位：是否於登入時自動啟動雙向綁定值。 */
const startOnLogin = ref(props.settings.startOnLogin);
/** 判斷是否非忙碌中且接收器名稱有效，用以決定送出按鈕是否可點擊。 */
const canSubmit = computed(() => !props.busy && isReceiverNameValid(receiverName.value));

watch(
  [
    () => props.settings.receiverName,
    () => props.settings.renderer,
    () => props.settings.fullscreen,
    () => props.settings.startOnLogin,
  ],
  ([nextReceiverName, nextRenderer, nextFullscreen, nextStartOnLogin]) => {
    receiverName.value = nextReceiverName;
    renderer.value = nextRenderer;
    fullscreen.value = nextFullscreen;
    startOnLogin.value = nextStartOnLogin;
  },
);

/**
 * 修剪接收器名稱前後空白並向父層發出 `save` 事件提交新設定。
 */
function submit(): void {
  emit("save", {
    receiverName: receiverName.value.trim(),
    renderer: renderer.value,
    fullscreen: fullscreen.value,
    startOnLogin: startOnLogin.value,
  });
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
        <input id="receiver-name" v-model="receiverName" class="text-input" maxlength="64" required data-testid="receiver-name" />
        <p class="field-hint">這個名稱會顯示在 Apple 裝置的螢幕鏡像清單中。</p>

        <label class="field-label" for="renderer">影片輸出方式</label>
        <select id="renderer" v-model="renderer" class="text-input" data-testid="renderer-select">
          <option v-for="option in rendererOptions" :key="option" :value="option">{{ option }}</option>
        </select>

        <label class="switch-row">
          <span><strong>全螢幕播放</strong><small>啟動 AirPlay 時使用全螢幕視窗</small></span>
          <input v-model="fullscreen" type="checkbox" data-testid="fullscreen-toggle" />
        </label>
      </div>

      <div class="settings-section settings-section-last">
        <h3>啟動方式</h3>
        <label class="switch-row">
          <span><strong>登入後自動啟動</strong><small>使用目前作業系統的登入啟動設定</small></span>
          <input v-model="startOnLogin" type="checkbox" data-testid="autostart-toggle" />
        </label>
      </div>

      <div class="form-footer">
        <span class="field-hint">新設定會立即儲存。</span>
        <button class="button-primary" type="submit" :disabled="!canSubmit" data-testid="settings-save">儲存設定</button>
      </div>
    </form>
  </section>
</template>
