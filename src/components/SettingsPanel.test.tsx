import { act } from "react";
import { createRoot } from "react-dom/client";
import { describe, expect, it, vi } from "vitest";

import type { HookInstallAgentStatus } from "../api/hookInstallApi";
import { defaultSettings } from "../api/settingsApi";
import { SettingsPanel, type HookInstallPanelState } from "./SettingsPanel";

describe("SettingsPanel hook install list", () => {
  it("shows hook status rows and disables duplicate actions", async () => {
    const onInstall = vi.fn();
    const onUninstall = vi.fn();
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        <SettingsPanel
          hookInstall={hookInstallState([
            {
              agent: "codex",
              state: "installed",
              message: "已安装",
              reasons: [],
              can_install: false,
              can_uninstall: true,
            },
            {
              agent: "claude",
              state: "not_installed",
              message: "未安装",
              reasons: [],
              can_install: true,
              can_uninstall: false,
            },
          ])}
          logPath={null}
          saving={false}
          settings={defaultSettings()}
          statusMessage={null}
          onChange={() => undefined}
          onInstallHook={onInstall}
          onOpenLogFolder={() => undefined}
          onUninstallHook={onUninstall}
        />,
      );
    });

    const codexInstallButton = labeledButton(container, "安装 codex hook");
    const codexUninstallButton = labeledButton(container, "卸载 codex hook");
    const claudeInstallButton = labeledButton(container, "安装 claude hook");
    const claudeUninstallButton = labeledButton(container, "卸载 claude hook");

    expect(container.textContent).toContain("codex & codex cli");
    expect(container.textContent).toContain("已安装");
    expect(codexInstallButton.disabled).toBe(true);
    expect(codexUninstallButton.disabled).toBe(false);
    expect(claudeInstallButton.disabled).toBe(false);
    expect(claudeUninstallButton.disabled).toBe(true);

    await act(async () => {
      claudeInstallButton.click();
      codexUninstallButton.click();
    });

    expect(onInstall).toHaveBeenCalledWith("claude");
    expect(onUninstall).toHaveBeenCalledWith("codex");

    await act(async () => {
      root.unmount();
    });
  });

  it("omits the old fixed settings note", async () => {
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        <SettingsPanel
          hookInstall={hookInstallState([])}
          logPath={null}
          saving={false}
          settings={defaultSettings()}
          statusMessage={null}
          onChange={() => undefined}
          onInstallHook={() => undefined}
          onOpenLogFolder={() => undefined}
          onUninstallHook={() => undefined}
        />,
      );
    });

    expect(container.textContent).not.toContain("本轮不提供自动更新配置项");

    await act(async () => {
      root.unmount();
    });
  });

  it("keeps shortcut icon actions accessible and wired", async () => {
    const onChange = vi.fn();
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        <SettingsPanel
          hookInstall={hookInstallState([])}
          logPath={null}
          saving={false}
          settings={defaultSettings()}
          statusMessage={null}
          onChange={onChange}
          onInstallHook={() => undefined}
          onOpenLogFolder={() => undefined}
          onUninstallHook={() => undefined}
        />,
      );
    });

    expect(labeledButton(container, "上移快捷输入：继续").disabled).toBe(true);

    await act(async () => {
      labeledButton(container, "上移快捷输入：补充边界").click();
      labeledButton(container, "下移快捷输入：继续").click();
      labeledButton(container, "删除快捷输入：继续").click();
      labeledButton(container, "新增快捷输入").click();
    });

    expect(onChange).toHaveBeenCalledTimes(4);

    await act(async () => {
      root.unmount();
    });
  });

  it("updates panel max window height from the Panel group", async () => {
    const onChange = vi.fn();
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        <SettingsPanel
          hookInstall={hookInstallState([])}
          logPath={null}
          saving={false}
          settings={defaultSettings()}
          statusMessage={null}
          onChange={onChange}
          onInstallHook={() => undefined}
          onOpenLogFolder={() => undefined}
          onUninstallHook={() => undefined}
        />,
      );
    });

    const input = labeledInput(container, "窗口最大高度");
    await act(async () => {
      setInputValue(input, "520");
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });

    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        panel: expect.objectContaining({
          max_window_height: 520,
        }),
      }),
    );

    await act(async () => {
      root.unmount();
    });
  });

  it("opens the log folder through the icon action", async () => {
    const onOpenLogFolder = vi.fn();
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        <SettingsPanel
          hookInstall={hookInstallState([])}
          logPath="/tmp/builder-panel.log"
          saving={false}
          settings={defaultSettings()}
          statusMessage={null}
          onChange={() => undefined}
          onInstallHook={() => undefined}
          onOpenLogFolder={onOpenLogFolder}
          onUninstallHook={() => undefined}
        />,
      );
    });

    await act(async () => {
      labeledButton(container, "打开日志目录").click();
    });

    expect(onOpenLogFolder).toHaveBeenCalledTimes(1);

    await act(async () => {
      root.unmount();
    });
  });
});

const hookInstallState = (
  agentStatuses: readonly HookInstallAgentStatus[],
): HookInstallPanelState => ({
  agentStatuses,
  statusMessage: null,
  workingAgent: null,
  refreshing: false,
});

const labeledButton = (
  container: HTMLElement,
  label: string,
): HTMLButtonElement => {
  const button = container.querySelector<HTMLButtonElement>(
    `button[aria-label="${label}"]`,
  );
  if (button === null) {
    throw new Error(`未找到按钮：${label}`);
  }
  return button;
};

const labeledInput = (
  container: HTMLElement,
  label: string,
): HTMLInputElement => {
  const labels = [...container.querySelectorAll("label")];
  const target = labels.find((item) => item.textContent?.includes(label));
  const input = target?.querySelector("input");
  if (input === undefined || input === null) {
    throw new Error(`未找到输入框：${label}`);
  }

  return input;
};

const setInputValue = (input: HTMLInputElement, value: string): void => {
  const descriptor = Object.getOwnPropertyDescriptor(
    HTMLInputElement.prototype,
    "value",
  );
  descriptor?.set?.call(input, value);
};
