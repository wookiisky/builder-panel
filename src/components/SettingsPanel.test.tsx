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
          saving={false}
          settings={defaultSettings()}
          statusMessage={null}
          onChange={() => undefined}
          onInstallHook={onInstall}
          onUninstallHook={onUninstall}
        />,
      );
    });

    const rows = container.querySelectorAll(".hook-install-row");
    const codexButtons = rows[0]?.querySelectorAll("button");
    const claudeButtons = rows[1]?.querySelectorAll("button");

    expect(container.textContent).toContain("codex & codex cli");
    expect(container.textContent).toContain("已安装");
    expect((codexButtons?.[0] as HTMLButtonElement | undefined)?.disabled).toBe(
      true,
    );
    expect((codexButtons?.[1] as HTMLButtonElement | undefined)?.disabled).toBe(
      false,
    );
    expect((claudeButtons?.[0] as HTMLButtonElement | undefined)?.disabled).toBe(
      false,
    );
    expect((claudeButtons?.[1] as HTMLButtonElement | undefined)?.disabled).toBe(
      true,
    );

    await act(async () => {
      claudeButtons?.[0]?.click();
      codexButtons?.[1]?.click();
    });

    expect(onInstall).toHaveBeenCalledWith("claude");
    expect(onUninstall).toHaveBeenCalledWith("codex");

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
