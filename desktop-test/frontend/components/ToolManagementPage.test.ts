import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../../../desktop/src/api";
import type { DependencySnapshot } from "../../../desktop/src/types";
import ToolManagementPage from "../../../desktop/src/components/ToolManagementPage.vue";

vi.mock("../../../desktop/src/api", () => ({
  getDependencies: vi.fn(),
  manageDependencies: vi.fn(),
}));

const ready: DependencySnapshot = {
  status: "ready",
  operation: null,
  message: "投影工具已就緒。",
  missing: [],
  logPath: "/tmp/neo-airplay-install.log",
};

describe("工具管理頁", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("顯示工具狀態並執行升級", async () => {
    vi.mocked(api.getDependencies).mockResolvedValue(ready);
    vi.mocked(api.manageDependencies).mockResolvedValue({
      ...ready,
      operation: "upgrade",
      message: "投影工具升級完成。",
    });
    const wrapper = mount(ToolManagementPage);

    await flushPromises();
    expect(wrapper.get('[data-testid="tools-status"]').text()).toContain("已就緒");
    expect(wrapper.text()).toContain("/tmp/neo-airplay-install.log");

    await wrapper.get('[data-testid="tools-upgrade"]').trigger("click");
    await flushPromises();

    expect(api.manageDependencies).toHaveBeenCalledWith("upgrade");
    expect(wrapper.get('[data-testid="tools-message"]').text()).toBe("投影工具升級完成。");
    wrapper.unmount();
  });

  it("重裝必須確認，取消時不呼叫後端", async () => {
    vi.mocked(api.getDependencies).mockResolvedValue(ready);
    vi.mocked(api.manageDependencies).mockResolvedValue({
      ...ready,
      operation: "reinstall",
      message: "投影工具重新安裝完成。",
    });
    const wrapper = mount(ToolManagementPage);

    await flushPromises();
    await wrapper.get('[data-testid="tools-reinstall"]').trigger("click");
    expect(wrapper.find('[role="dialog"]').exists()).toBe(true);
    expect(api.manageDependencies).not.toHaveBeenCalled();

    await wrapper.get('[data-testid="tools-reinstall-cancel"]').trigger("click");
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    expect(api.manageDependencies).not.toHaveBeenCalled();

    await wrapper.get('[data-testid="tools-reinstall"]').trigger("click");
    await wrapper.get('[data-testid="tools-reinstall-confirm"]').trigger("click");
    await flushPromises();

    expect(api.manageDependencies).toHaveBeenCalledWith("reinstall");
    expect(wrapper.get('[data-testid="tools-message"]').text()).toBe("投影工具重新安裝完成。");
    wrapper.unmount();
  });

  it("維護作業進行中時停用操作", async () => {
    vi.mocked(api.getDependencies).mockResolvedValue({
      ...ready,
      status: "installing",
      operation: "upgrade",
      message: "正在更新平台相依套件與 Neo AirPlay 工具…",
    });
    const wrapper = mount(ToolManagementPage);

    await flushPromises();
    expect(wrapper.get('[data-testid="tools-upgrade"]').attributes("disabled")).toBeDefined();
    expect(wrapper.get('[data-testid="tools-reinstall"]').attributes("disabled")).toBeDefined();
    await wrapper.get('[data-testid="tools-upgrade"]').trigger("click");
    expect(api.manageDependencies).not.toHaveBeenCalled();
    wrapper.unmount();
  });
});
