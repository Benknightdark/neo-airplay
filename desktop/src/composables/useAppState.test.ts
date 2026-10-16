import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../api";
import type { DashboardSnapshot, EngineSnapshot } from "../types";
import { useAppState } from "./useAppState";

vi.mock("../api", () => ({
  getDependencies: vi.fn(),
  installDependencies: vi.fn(),
  getDashboard: vi.fn(),
  saveSettings: vi.fn(),
  startAirplay: vi.fn(),
  stopAirplay: vi.fn(),
  refreshAndroidDevices: vi.fn(),
  startAndroidMirror: vi.fn(),
  stopAndroidMirror: vi.fn(),
}));

const dashboard: DashboardSnapshot = {
  settings: { receiverName: "Neo AirPlay", renderer: "Auto", fullscreen: false, startOnLogin: false },
  rendererOptions: ["Auto"],
  airplay: { status: "stopped", message: "AirPlay 接收器尚未啟動。", processId: null },
  android: { status: "stopped", message: "Android 投影尚未啟動。", processId: null },
  devices: [],
  logs: [],
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function mountState() {
  const stateRef: { current?: ReturnType<typeof useAppState> } = {};
  const wrapper = mount(defineComponent({
    setup() {
      stateRef.current = useAppState();
      return () => h("div");
    },
  }));
  const state = stateRef.current;
  if (!state) throw new Error("找不到應用程式狀態。");
  return { state, wrapper };
}

describe("應用程式狀態", () => {
  beforeEach(() => vi.resetAllMocks());
  afterEach(() => vi.useRealTimers());

  it("動作失敗後保留錯誤，直到使用者重新整理或執行其他動作", async () => {
    vi.mocked(api.getDashboard).mockResolvedValue(dashboard);
    vi.mocked(api.startAirplay).mockRejectedValue(new Error("engine failed"));

    const { state, wrapper } = mountState();

    await state.startAirplay();
    expect(state.error.value).toBe("engine failed");

    await state.refresh();
    expect(state.error.value).toBe("engine failed");

    await state.refresh(true);
    expect(state.error.value).toBeNull();
    wrapper.unmount();
  });

  it("較新的狀態刷新完成後不會被較早的回應覆蓋", async () => {
    const oldRequest = deferred<DashboardSnapshot>();
    const newRequest = deferred<DashboardSnapshot>();
    vi.mocked(api.getDashboard)
      .mockReturnValueOnce(oldRequest.promise)
      .mockReturnValueOnce(newRequest.promise);
    const { state, wrapper } = mountState();

    const oldRefresh = state.refresh();
    const newRefresh = state.refresh(true);
    newRequest.resolve({ ...dashboard, settings: { ...dashboard.settings, receiverName: "Fresh" } });
    await newRefresh;
    oldRequest.resolve({ ...dashboard, settings: { ...dashboard.settings, receiverName: "Stale" } });
    await oldRefresh;

    expect(state.dashboard.value?.settings.receiverName).toBe("Fresh");
    wrapper.unmount();
  });

  it("操作完成後的狀態不會被較早開始的手動刷新覆蓋", async () => {
    const manualRefreshRequest = deferred<DashboardSnapshot>();
    const postActionRequest = deferred<DashboardSnapshot>();
    const startRequest = deferred<EngineSnapshot>();
    vi.mocked(api.getDashboard)
      .mockReturnValueOnce(manualRefreshRequest.promise)
      .mockReturnValueOnce(postActionRequest.promise);
    vi.mocked(api.startAirplay).mockReturnValue(startRequest.promise);
    const { state, wrapper } = mountState();

    const manualRefresh = state.refresh();
    const startAirplay = state.startAirplay();
    startRequest.resolve(dashboard.airplay);
    await flushPromises();
    expect(api.getDashboard).toHaveBeenCalledTimes(2);

    postActionRequest.resolve({ ...dashboard, airplay: { ...dashboard.airplay, status: "running", message: "執行中", processId: 42 } });
    await startAirplay;
    manualRefreshRequest.resolve(dashboard);
    await manualRefresh;

    expect(state.dashboard.value?.airplay.status).toBe("running");
    wrapper.unmount();
  });

  it("動作進行中忽略重複或重疊的操作", async () => {
    const startRequest = deferred<EngineSnapshot>();
    vi.mocked(api.startAirplay).mockReturnValue(startRequest.promise);
    vi.mocked(api.getDashboard).mockResolvedValue(dashboard);
    const { state, wrapper } = mountState();

    const firstStart = state.startAirplay();
    const duplicateStart = state.startAirplay();
    expect(api.startAirplay).toHaveBeenCalledOnce();
    expect(state.busy.value).toBe(true);

    startRequest.resolve(dashboard.airplay);
    await Promise.all([firstStart, duplicateStart]);
    expect(api.getDashboard).toHaveBeenCalledOnce();
    expect(state.busy.value).toBe(false);
    wrapper.unmount();
  });

  it("卸載後停止狀態輪詢", async () => {
    vi.useFakeTimers();
    vi.mocked(api.getDashboard).mockResolvedValue(dashboard);
    const { wrapper } = mountState();

    await vi.advanceTimersByTimeAsync(1500);
    await flushPromises();
    expect(api.getDashboard).toHaveBeenCalledOnce();

    wrapper.unmount();
    await vi.advanceTimersByTimeAsync(3000);
    expect(api.getDashboard).toHaveBeenCalledOnce();
  });
});
