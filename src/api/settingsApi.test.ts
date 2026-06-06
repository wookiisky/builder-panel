import { describe, expect, it } from "vitest";

import { defaultSettings } from "./settingsApi";

describe("settingsApi", () => {
  it("creates stage seven default settings without auto update option", () => {
    const settings = defaultSettings();

    expect(settings.display.show_usage).toBe(true);
    expect(settings.panel.collapsed).toBe(false);
    expect(settings.panel.window_position).toBeNull();
    expect(settings.panel.window_size).toBeNull();
    expect(settings.agents.mock_agent_enabled).toBe(true);
    expect(settings.agents.codex_cli_enabled).toBe(true);
    expect("auto_update" in settings).toBe(false);
  });
});
