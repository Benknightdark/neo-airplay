<script setup lang="ts">
/**
 * @file 接收器設定頁面元件，管理接收器名稱、視訊輸出方式、全螢幕與開機自啟表單與送出驗證。
 */
import { computed, ref, watch } from "vue";
import {
  isAirplayPinValid,
  isReceiverNameValid,
  MAX_RECEIVER_NAME_UTF8_BYTES,
  receiverNameUtf8Bytes,
  type AirplayAuthMode,
  type ReceiverSettings,
} from "../domain";

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
/** 表單欄位：AirPlay 區域網路連線 PIN 認證模式。 */
const airplayAuthMode = ref<AirplayAuthMode>(props.settings.airplayAuthMode);
/** 表單欄位：固定 PIN 模式下的 4 位數字密碼。 */
const airplayPin = ref(props.settings.airplayPin);
/** 表單欄位：是否停用 Android 與電腦之間的自動剪貼簿同步。 */
const androidNoClipboardSync = ref(props.settings.androidNoClipboardSync);
/** 表單欄位：是否啟用 Android 唯讀投影模式。 */
const androidReadOnly = ref(props.settings.androidReadOnly);

/** 目前輸入的接收器名稱 UTF-8 位元組長度。 */
const receiverNameBytes = computed(() => receiverNameUtf8Bytes(receiverName.value));
/** 判斷是否非忙碌中且接收器名稱與 AirPlay PIN 設定皆有效，用以決定送出按鈕是否可點擊。 */
const canSubmit = computed(
  () =>
    !props.busy &&
    isReceiverNameValid(receiverName.value) &&
    isAirplayPinValid(airplayAuthMode.value, airplayPin.value),
);

watch(
  [
    () => props.settings.receiverName,
    () => props.settings.renderer,
    () => props.settings.fullscreen,
    () => props.settings.startOnLogin,
    () => props.settings.airplayAuthMode,
    () => props.settings.airplayPin,
    () => props.settings.androidNoClipboardSync,
    () => props.settings.androidReadOnly,
  ],
  ([
    nextReceiverName,
    nextRenderer,
    nextFullscreen,
    nextStartOnLogin,
    nextAirplayAuthMode,
    nextAirplayPin,
    nextAndroidNoClipboardSync,
    nextAndroidReadOnly,
  ]) => {
    receiverName.value = nextReceiverName;
    renderer.value = nextRenderer;
    fullscreen.value = nextFullscreen;
    startOnLogin.value = nextStartOnLogin;
    airplayAuthMode.value = nextAirplayAuthMode;
    airplayPin.value = nextAirplayPin;
    androidNoClipboardSync.value = nextAndroidNoClipboardSync;
    androidReadOnly.value = nextAndroidReadOnly;
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
    airplayAuthMode: airplayAuthMode.value,
    airplayPin: airplayAuthMode.value === "fixedPin" ? airplayPin.value.trim() : "",
    androidNoClipboardSync: androidNoClipboardSync.value,
    androidReadOnly: androidReadOnly.value,
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
        <input id="receiver-name" v-model="receiverName" class="text-input" maxlength="63" required data-testid="receiver-name" />
        <p class="field-hint">
          這個名稱會顯示在 Apple 裝置的螢幕鏡像清單中（目前 {{ receiverNameBytes }} / {{ MAX_RECEIVER_NAME_UTF8_BYTES }} UTF-8 位元組）。
        </p>

        <label class="field-label" for="renderer">影片輸出方式</label>
        <select id="renderer" v-model="renderer" class="text-input" data-testid="renderer-select">
          <option v-for="option in rendererOptions" :key="option" :value="option">{{ option }}</option>
        </select>

        <label class="switch-row">
          <span><strong>全螢幕播放</strong><small>啟動 AirPlay 時使用全螢幕視窗</small></span>
          <input v-model="fullscreen" type="checkbox" data-testid="fullscreen-toggle" />
        </label>

        <label class="field-label" for="airplay-auth-mode">區網連線驗證</label>
        <select id="airplay-auth-mode" v-model="airplayAuthMode" class="text-input" data-testid="airplay-auth-mode">
          <option value="none">不要求 PIN（快速連線）</option>
          <option value="randomPin">每次啟動產生隨機 PIN</option>
          <option value="fixedPin">固定 4 位數字 PIN</option>
        </select>
        <p class="field-hint">建議於共用區網環境啟用 PIN 驗證，避免鄰近裝置未經授權投放畫面。</p>

        <template v-if="airplayAuthMode === 'fixedPin'">
          <label class="field-label" for="airplay-pin">固定 4 位數字 PIN</label>
          <input
            id="airplay-pin"
            v-model="airplayPin"
            class="text-input"
            type="password"
            inputmode="numeric"
            autocomplete="off"
            maxlength="4"
            pattern="\d{4}"
            placeholder="請輸入 4 位數字，例如 1234"
            required
            data-testid="airplay-pin"
          />
          <p v-if="!isAirplayPinValid(airplayAuthMode, airplayPin)" class="field-hint">
            固定 PIN 模式必須為精確 4 位 ASCII 數字（0-9）。
          </p>
        </template>
      </div>

      <div class="settings-section">
        <h3>Android 鏡像安全性</h3>
        <p class="settings-description">控制電腦與 Android 裝置在 scrcpy 投影期間的互動權限。</p>
        <label class="switch-row">
          <span><strong>停用自動剪貼簿同步</strong><small>避免電腦剪貼簿中的密碼或機密文字自動傳送至手機</small></span>
          <input v-model="androidNoClipboardSync" type="checkbox" data-testid="android-no-clipboard-toggle" />
        </label>
        <label class="switch-row">
          <span><strong>唯讀投影模式</strong><small>禁止透過桌面視窗觸控操控手機與拖放安裝 APK</small></span>
          <input v-model="androidReadOnly" type="checkbox" data-testid="android-read-only-toggle" />
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
