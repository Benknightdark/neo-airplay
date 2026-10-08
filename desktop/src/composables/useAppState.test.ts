import { mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { describe, expect, it, vi } from "vitest";
import * as api from "../api";
import type { DashboardSnapshot } from "../types";
import { useAppState } from "./useAppState";

vi.mock("../api", () => ({
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

describe("應用程式狀態", () => {
  it("動作失敗後保留錯誤，直到使用者重新整理或執行其他動作", async () => {
    vi.mocked(api.getDashboard).mockResolvedValue(dashboard);
    vi.mocked(api.startAirplay).mockRejectedValue(new Error("engine failed"));

    const stateRef: { current?: ReturnType<typeof useAppState> } = {};
    const wrapper = mount(defineComponent({
      setup() {
        stateRef.current = useAppState();
        return () => h("div");
      },
    }));
    const state = stateRef.current;
    if (!state) throw new Error("找不到應用程式狀態。");

    await state.startAirplay();
    expect(state.error.value).toBe("engine failed");

    await state.refresh();
    expect(state.error.value).toBe("engine failed");

    await state.refresh(true);
    expect(state.error.value).toBeNull();
    wrapper.unmount();
  });
});
