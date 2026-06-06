import { describe, expect, it } from "vitest";

import { defaultSettings, normalizeFallbackSettings } from "./settingsApi";

describe("settingsApi", () => {
  it("creates stage seven default settings without auto update option", () => {
    const settings = defaultSettings();

    expect(settings.display.show_usage).toBe(true);
    expect(settings.display.theme).toBe("light");
    expect(settings.panel.collapsed).toBe(false);
    expect(settings.panel.window_position).toBeNull();
    expect(settings.panel.window_size).toBeNull();
    expect(settings.agents.mock_agent_enabled).toBe(true);
    expect(settings.agents.codex_cli_enabled).toBe(true);
    expect("auto_update" in settings).toBe(false);
  });

  it("normalizes legacy fallback settings with missing display theme", () => {
    const legacySettings = {
      ...defaultSettings(),
      display: {
        show_usage: false,
        density: "compact",
        animation_level: "reduced",
      },
      panel: {
        collapsed: true,
        window_position: { x: 10, y: 20 },
        window_size: { width: 700, height: 500 },
      },
      agents: {
        mock_agent_enabled: false,
        codex_cli_enabled: true,
        codex_app_enabled: false,
        claude_cli_enabled: false,
        claude_app_enabled: false,
      },
    };

    const normalizedSettings = normalizeFallbackSettings(legacySettings);

    expect(normalizedSettings?.display.theme).toBe("light");
    expect(normalizedSettings?.display.show_usage).toBe(false);
    expect(normalizedSettings?.display.density).toBe("compact");
    expect(normalizedSettings?.panel.collapsed).toBe(true);
    expect(normalizedSettings?.agents.mock_agent_enabled).toBe(false);
  });

  it("rejects fallback settings with invalid provided fields", () => {
    const invalidSettings = {
      ...defaultSettings(),
      display: {
        ...defaultSettings().display,
        theme: "neon",
      },
    };

    expect(normalizeFallbackSettings(invalidSettings)).toBeNull();
  });

  it("rejects fallback settings when object sections are arrays", () => {
    expect(normalizeFallbackSettings([])).toBeNull();
    expect(
      normalizeFallbackSettings({
        ...defaultSettings(),
        display: [],
      }),
    ).toBeNull();
  });
});
