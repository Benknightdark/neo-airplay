<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { DashboardSnapshot } from "../types";
import { engineLabel } from "../lib/status";

const props = defineProps<{
  dashboard: DashboardSnapshot;
  busy: boolean;
}>();

const emit = defineEmits<{
  (event: "start-airplay"): void;
  (event: "stop-airplay"): void;
  (event: "refresh-devices"): void;
  (event: "start-android", serial: string): void;
  (event: "stop-android"): void;
}>();

const selectedSerial = ref("");
const availableDevices = computed(() => props.dashboard.devices.filter((device) => device.status === "ready"));
const selectedDevice = computed(() => availableDevices.value.find((device) => device.serial === selectedSerial.value));

watch(availableDevices, (devices) => {
  if (!devices.some((device) => device.serial === selectedSerial.value)) {
    selectedSerial.value = devices[0]?.serial ?? "";
  }
}, { immediate: true });

function toggleAirplay(): void {
  if (props.dashboard.airplay.status === "running") {
    emit("stop-airplay");
  } else {
    emit("start-airplay");
  }
}

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
            v-if="dashboard.airplay.status !== 'running'"
            class="button-primary"
            type="button"
            :disabled="busy"
            data-testid="airplay-start"
            @click="toggleAirplay"
          >開始接收</button>
          <button v-else class="button-secondary" type="button" :disabled="busy" data-testid="airplay-stop" @click="toggleAirplay">停止接收</button>
          <span class="action-hint">需要安裝 UxPlay 與影音執行環境</span>
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
          <p v-else-if="availableDevices.length === 0" class="field-hint">已找到裝置，但尚未授權 USB 偵錯。</p>
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

    <div class="summary-strip">
      <div class="summary-item"><span class="summary-icon">⌘</span><span>Apple 裝置</span><strong>AirPlay</strong></div>
      <div class="summary-divider"></div>
      <div class="summary-item"><span class="summary-icon">▣</span><span>Android 裝置</span><strong>scrcpy</strong></div>
      <div class="summary-divider"></div>
      <div class="summary-item"><span class="summary-icon">◉</span><span>執行方式</span><strong>獨立程序</strong></div>
    </div>
  </section>
</template>
