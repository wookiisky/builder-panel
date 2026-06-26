import { describe, expect, it } from "vitest";

import { defaultSettings, normalizeFallbackSettings } from "./settingsApi";

describe("settingsApi", () => {
  it("creates default real runtime settings without auto update option", () => {
    const settings = defaultSettings();

    expect(settings.display.show_usage).toBe(true);
    expect(settings.display.theme).toBe("light");
    expect(settings.display.summary_tooltip_paragraphs).toBe(5);
    expect(settings.panel.collapsed).toBe(false);
    expect(settings.panel.window_position).toBeNull();
    expect(settings.panel.window_size).toBeNull();
    expect(settings.agents.codex_cli_enabled).toBe(true);
    expect(settings.agents.codex_app_enabled).toBe(true);
    expect("mock_agent_enabled" in settings.agents).toBe(false);
    expect("auto_update" in settings).toBe(false);
  });

  it("defaults codex internal prompt patterns to the suggestion task", () => {
    const settings = defaultSettings();

    expect(settings.agents.codex_internal_prompt_patterns).toEqual([
      "hyperpersonalized suggestions",
    ]);
  });

  it("normalizes codex internal prompt patterns by trimming and dropping blanks", () => {
    const settings = {
      ...defaultSettings(),
      agents: {
        ...defaultSettings().agents,
        codex_internal_prompt_patterns: ["  custom task  ", "", "   "],
      },
    };

    const normalized = normalizeFallbackSettings(settings);

    expect(normalized?.agents.codex_internal_prompt_patterns).toEqual([
      "custom task",
    ]);
  });

  it("normalizes legacy fallback settings and drops mock agent flag", () => {
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
        codex_app_enabled: true,
        claude_cli_enabled: false,
        claude_app_enabled: false,
      },
    };

    const normalizedSettings = normalizeFallbackSettings(legacySettings);

    expect(normalizedSettings?.display.theme).toBe("light");
    expect(normalizedSettings?.display.show_usage).toBe(false);
    expect(normalizedSettings?.display.density).toBe("compact");
    expect(normalizedSettings?.display.summary_tooltip_paragraphs).toBe(5);
    expect(normalizedSettings?.panel.collapsed).toBe(false);
    expect(normalizedSettings?.agents.codex_app_enabled).toBe(true);
    expect("mock_agent_enabled" in (normalizedSettings?.agents ?? {})).toBe(
      false,
    );
  });

  it("normalizes summary tooltip paragraphs by flooring and rejecting invalid counts", () => {
    const base = defaultSettings();
    const cases: Array<[unknown, number]> = [
      [3, 3],
      [2.9, 2],
      [0, 5],
      [-4, 5],
      ["7", 5],
      [Number.NaN, 5],
    ];

    for (const [input, expected] of cases) {
      const normalized = normalizeFallbackSettings({
        ...base,
        display: {
          ...base.display,
          summary_tooltip_paragraphs: input,
        },
      });
      expect(normalized?.display.summary_tooltip_paragraphs).toBe(expected);
    }
  });

  it("normalizes custom shortcuts by dropping invalid rows and duplicate ids", () => {
    const settings = {
      ...defaultSettings(),
      replies: {
        enter_to_send: true,
        shortcut_replies_enabled: true,
        custom_shortcuts: [
          {
            id: "same",
            label: " B ",
            content: " 内容 ",
            enabled: true,
            order: 20,
          },
          {
            id: "same",
            label: "重复",
            content: "重复内容",
            enabled: true,
            order: 10,
          },
          {
            id: "",
            label: "空 ID",
            content: "无效",
            enabled: true,
            order: 30,
          },
          {
            id: "first",
            label: "A",
            content: "A 内容",
            enabled: false,
            order: 5,
          },
        ],
      },
    };

    const normalizedSettings = normalizeFallbackSettings(settings);

    expect(normalizedSettings?.replies.custom_shortcuts).toEqual([
      {
        id: "first",
        label: "A",
        content: "A 内容",
        enabled: false,
        order: 5,
      },
      {
        id: "same",
        label: "B",
        content: "内容",
        enabled: true,
        order: 20,
      },
    ]);
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
