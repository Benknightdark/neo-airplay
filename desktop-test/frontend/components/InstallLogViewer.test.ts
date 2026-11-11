import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "../../../desktop/src/api";
import InstallLogViewer from "../../../desktop/src/components/InstallLogViewer.vue";

vi.mock("../../../desktop/src/api", () => ({
  readInstallLog: vi.fn(),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

describe("安裝記錄檢視器", () => {
  beforeEach(() => vi.resetAllMocks());

  it("開啟並顯示可選取的安裝記錄，關閉後可以重新開啟", async () => {
    const firstRequest = deferred<string>();
    vi.mocked(api.readInstallLog)
      .mockReturnValueOnce(firstRequest.promise)
      .mockResolvedValueOnce("install started\ninstall finished\n");
    const wrapper = mount(InstallLogViewer, {
      props: { logPath: "/tmp/neo-airplay-install.log" },
    });

    expect(wrapper.text()).toContain("/tmp/neo-airplay-install.log");
    await wrapper.get('[data-testid="install-log-open"]').trigger("click");
    expect(wrapper.get('[role="dialog"]').text()).toContain("正在載入安裝記錄");
    firstRequest.resolve("install started\ninstall finished\n");
    await flushPromises();

    const content = wrapper.get<HTMLTextAreaElement>('[data-testid="install-log-content"]');
    expect(content.element.value).toBe("install started\ninstall finished\n");
    expect(content.element.readOnly).toBe(true);
    expect(api.readInstallLog).toHaveBeenCalledOnce();

    await wrapper.get('[data-testid="install-log-close"]').trigger("click");
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    await wrapper.get('[data-testid="install-log-open"]').trigger("click");
    await flushPromises();
    expect(api.readInstallLog).toHaveBeenCalledTimes(2);
    wrapper.unmount();
  });

  it("讀取失敗時在視窗內顯示錯誤", async () => {
    vi.mocked(api.readInstallLog).mockRejectedValue(new Error("尚無安裝記錄。"));
    const wrapper = mount(InstallLogViewer, {
      props: { logPath: "/tmp/neo-airplay-install.log" },
    });

    await wrapper.get('[data-testid="install-log-open"]').trigger("click");
    await flushPromises();

    expect(wrapper.get('[role="alert"]').text()).toBe("尚無安裝記錄。");
    expect(wrapper.find('[data-testid="install-log-close"]').exists()).toBe(true);
    wrapper.unmount();
  });
});
