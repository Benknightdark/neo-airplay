import { onMounted, onUnmounted, ref } from "vue";
import * as api from "../api";
import type { DashboardSnapshot, ReceiverSettings } from "../types";

export function useAppState() {
  const dashboard = ref<DashboardSnapshot | null>(null);
  const busy = ref(false);
  const error = ref<string | null>(null);
  let dashboardPollingTimer: number | undefined;
  let latestDashboardRequest = 0;
  let dashboardRefreshRequestsInFlight = 0;

  async function refresh(clearError = false): Promise<void> {
    if (clearError) error.value = null;
    const requestId = ++latestDashboardRequest;
    dashboardRefreshRequestsInFlight += 1;
    try {
      const next = await api.getDashboard();
      if (requestId === latestDashboardRequest) dashboard.value = next;
    } catch (reason) {
      if (requestId === latestDashboardRequest) error.value = errorMessage(reason);
    } finally {
      dashboardRefreshRequestsInFlight -= 1;
    }
  }

  async function runAction(action: () => Promise<unknown>): Promise<void> {
    if (busy.value) return;
    busy.value = true;
    error.value = null;
    try {
      await action();
      await refresh();
    } catch (reason) {
      error.value = errorMessage(reason);
      await refresh();
      error.value = errorMessage(reason);
    } finally {
      busy.value = false;
    }
  }

  onMounted(() => {
    dashboardPollingTimer = window.setInterval(() => {
      if (!busy.value && dashboardRefreshRequestsInFlight === 0) void refresh();
    }, 1500);
  });

  onUnmounted(() => {
    if (dashboardPollingTimer !== undefined) window.clearInterval(dashboardPollingTimer);
  });

  return {
    dashboard,
    busy,
    error,
    refresh,
    startAirplay: (): Promise<void> => runAction(api.startAirplay),
    stopAirplay: (): Promise<void> => runAction(api.stopAirplay),
    refreshDevices: (): Promise<void> => runAction(api.refreshAndroidDevices),
    startAndroid: (serial: string): Promise<void> => runAction(() => api.startAndroidMirror(serial)),
    stopAndroid: (): Promise<void> => runAction(api.stopAndroidMirror),
    saveSettings: (settings: ReceiverSettings): Promise<void> => runAction(() => api.saveSettings(settings)),
  };
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
