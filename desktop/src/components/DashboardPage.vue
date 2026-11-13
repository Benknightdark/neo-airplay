<script setup lang="ts">
/**
 * @file 接收器總覽儀表板頁面元件，提供 AirPlay 與 Android 雙投影引擎啟動、停止及裝置選擇控制。
 */
import { computed, ref, watch } from "vue";
import {
  airplayButtonLabel,
  airplayHint,
  diagnoseUnavailableDevices,
  engineLabel,
  filterReadyDevices,
  isAirplayActive,
  resolveSelectedSerial,
  type DashboardSnapshot,
} from "../domain";

/** 元件接收的唯讀 Props。 */
const props = defineProps<{
  /** 來自後端的儀表板完整狀態快照。 */
  dashboard: DashboardSnapshot;
  /** 指示全域是否正處於操作忙碌鎖定狀態。 */
  busy: boolean;
}>();

/** 元件對父層發出的控制動作事件。 */
const emit = defineEmits<{
  /** 使用者點擊「開始接收」以啟動 AirPlay 接收器。 */
  (event: "start-airplay"): void;
  /** 使用者點擊停止或取消啟動 AirPlay 接收器。 */
  (event: "stop-airplay"): void;
  /** 使用者點擊重新整理 Android 裝置清單。 */
  (event: "refresh-devices"): void;
  /** 使用者選定裝置並點擊「開始投影」時觸發，攜帶裝置 ADB 序號。 */
  (event: "start-android", serial: string): void;
  /** 使用者點擊「停止投影」以停止 Android 投影。 */
  (event: "stop-android"): void;
}>();

/** 目前下拉選單選取中的 Android 裝置序號。 */
const selectedSerial = ref("");
/** 僅包含狀態為 `ready` 的可用 Android 裝置清單。 */
const availableDevices = computed(() => filterReadyDevices(props.dashboard.devices));
/** 目前選取序號對應的可用裝置物件（若無效則為 `undefined`）。 */
const selectedDevice = computed(() =>
  availableDevices.value.find((device) => device.serial === selectedSerial.value),
);
/** AirPlay 目前是否處於啟動中、運行中或停止中。 */
const airplayActive = computed(() => isAirplayActive(props.dashboard.airplay.status));
/** 依據 AirPlay 作用中狀態產生的停止按鈕文字。 */
const buttonLabel = computed(() => airplayButtonLabel(props.dashboard.airplay.status));
/** 依據 AirPlay 狀態與接收器名稱產生的操作引導提示。 */
const airplayStatusHint = computed(() =>
  airplayHint(props.dashboard.airplay.status, props.dashboard.settings.receiverName),
);
/** 當沒有可用 Android 裝置時顯示的診斷與排障指引文字。 */
const unavailableDeviceHint = computed(() =>
  diagnoseUnavailableDevices(props.dashboard.devices),
);

watch(
  availableDevices,
  (devices) => {
    selectedSerial.value = resolveSelectedSerial(devices, selectedSerial.value);
  },
  { immediate: true },
);

/**
 * 依據目前 AirPlay 狀態決定發出啟動或停止事件（處於 `stopping` 時忽略點擊）。
 */
function toggleAirplay(): void {
  if (
    props.dashboard.airplay.status === "running" ||
    props.dashboard.airplay.status === "starting"
  ) {
    emit("stop-airplay");
  } else if (props.dashboard.airplay.status !== "stopping") {
    emit("start-airplay");
  }
}

/**
 * 依據目前 Android 投影狀態決定發出停止事件或以目前選取序號啟動投影。
 */
function toggleAndroid(): void {
  if (props.dashboard.android.status === "running") {
    emit("stop-android");
  } else if (selectedSerial.value) {
    emit("start-android", selectedSerial.value);
  }
}
</script>

<template>
  <section class="page-content" aria-label="接收器總覽" data-testid="dashboard-page">
    <div class="welcome-row">
      <div>
        <p class="eyebrow">投影服務</p>
        <h2>讓每個裝置都能登上大螢幕</h2>
        <p class="muted-copy">管理 Apple AirPlay 與 Android 投影，可同時接收兩種裝置。</p>
      </div>
      <div class="network-indicator"><span class="connection-dot"></span>等待區域網路裝置</div>
    </div>

    <div class="engine-grid">
      <article class="engine-card airplay-card">
        <div class="card-heading">
          <div class="service-icon airplay-icon" aria-hidden="true">⌁</div>
          <div class="service-title">
            <p class="eyebrow">APPLE 裝置</p>
            <h3>AirPlay 接收器</h3>
          </div>
          <span class="status-pill" :class="`status-${dashboard.airplay.status}`" data-testid="airplay-status">
            <span class="status-indicator"></span>{{ engineLabel(dashboard.airplay.status) }}
          </span>
        </div>
        <p class="service-description">接收 iPhone、iPad 與 Mac 的螢幕鏡像或音訊串流。</p>
        <div class="service-detail" :class="{ 'detail-error': dashboard.airplay.status === 'faulted' || dashboard.airplay.status === 'dependencyMissing' }">
          <span class="detail-symbol" aria-hidden="true">ⓘ</span>
          <span>{{ dashboard.airplay.message }}</span>
        </div>
        <div class="card-actions">
          <button
            v-if="!airplayActive"
            class="button-primary"
            type="button"
            :disabled="busy"
            data-testid="airplay-start"
            @click="toggleAirplay"
          >開始接收</button>
          <button v-else class="button-secondary" type="button" :disabled="busy || dashboard.airplay.status === 'stopping'" data-testid="airplay-stop" @click="toggleAirplay">{{ buttonLabel }}</button>
          <span class="action-hint">{{ airplayStatusHint }}</span>
        </div>
      </article>

      <article class="engine-card android-card">
        <div class="card-heading">
          <div class="service-icon android-icon" aria-hidden="true">▣</div>
          <div class="service-title">
            <p class="eyebrow">ANDROID 裝置</p>
            <h3>Android 投影</h3>
          </div>
          <span class="status-pill" :class="`status-${dashboard.android.status}`" data-testid="android-status">
            <span class="status-indicator"></span>{{ engineLabel(dashboard.android.status) }}
          </span>
        </div>
        <p class="service-description">透過 USB 或已配對的網路偵錯裝置開啟獨立投影視窗。</p>
        <div class="device-picker">
          <label for="android-device">已連線裝置</label>
          <div class="picker-row">
            <select id="android-device" v-model="selectedSerial" data-testid="android-device-select">
              <option value="" disabled>請選擇裝置</option>
              <option v-for="device in availableDevices" :key="device.serial" :value="device.serial">
                {{ device.model }} · {{ device.serial }}
              </option>
            </select>
            <button class="button-icon" type="button" aria-label="重新整理 Android 裝置" data-testid="android-refresh" :disabled="busy" @click="emit('refresh-devices')">⟳</button>
          </div>
          <p v-if="dashboard.devices.length === 0" class="field-hint">尚未找到裝置。請連接手機並開啟 USB 偵錯。</p>
          <p v-else-if="availableDevices.length === 0" class="field-hint" data-testid="android-device-hint">{{ unavailableDeviceHint }}</p>
        </div>
        <div class="card-actions android-actions">
          <button
            v-if="dashboard.android.status !== 'running'"
            class="button-primary"
            type="button"
            :disabled="busy || !selectedDevice"
            data-testid="android-start"
            @click="toggleAndroid"
          >開始投影</button>
          <button v-else class="button-secondary" type="button" :disabled="busy" data-testid="android-stop" @click="toggleAndroid">停止投影</button>
          <span class="action-hint">需要安裝 scrcpy 與 Android Platform Tools</span>
        </div>
      </article>
    </div>
  </section>
</template>
