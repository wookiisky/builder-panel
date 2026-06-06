import { invoke } from "@tauri-apps/api/core";

import type {
  BuilderPanelSettings,
  SettingsViewModel,
} from "./settingsContract";

/// 浏览器 fallback 使用的 localStorage 键。
const FALLBACK_SETTINGS_KEY = "builder-panel-settings";

/// 默认设置。
export const defaultSettings = (): BuilderPanelSettings => ({
  general: {
    keep_panel_on_top: true,
    notify_on_completion: true,
    notify_on_waiting: true,
  },
  display: {
    show_usage: true,
    density: "comfortable",
    animation_level: "full",
  },
  panel: {
    collapsed: false,
    window_position: null,
    window_size: null,
  },
  agents: {
    mock_agent_enabled: true,
    codex_cli_enabled: true,
    codex_app_enabled: false,
    claude_cli_enabled: false,
    claude_app_enabled: false,
  },
  replies: {
    enter_to_send: true,
    shortcut_replies_enabled: true,
  },
  presets: {
    prefer_structured_create: true,
  },
  terminal: {
    jump_enabled: true,
    copy_fallback_enabled: true,
  },
  advanced: {
    developer_diagnostics: false,
  },
});

/// 读取设置。
export const fetchPanelSettings = async (): Promise<SettingsViewModel> => {
  try {
    return await invoke<SettingsViewModel>("get_panel_settings");
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "读取设置失败");
    }
    return readFallbackSettings();
  }
};

/// 保存设置。
export const savePanelSettings = async (
  settings: BuilderPanelSettings,
): Promise<SettingsViewModel> => {
  try {
    return await invoke<SettingsViewModel>("save_panel_settings", {
      settings,
    });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "保存设置失败");
    }
    window.localStorage.setItem(
      FALLBACK_SETTINGS_KEY,
      JSON.stringify(settings),
    );
    return {
      settings,
      status_message: "设置已保存",
    };
  }
};

/// 浏览器开发环境读取 fallback 设置。
const readFallbackSettings = (): SettingsViewModel => {
  const raw = window.localStorage.getItem(FALLBACK_SETTINGS_KEY);
  if (raw === null) {
    return {
      settings: defaultSettings(),
      status_message: null,
    };
  }

  try {
    const parsed: unknown = JSON.parse(raw);
    if (isBuilderPanelSettings(parsed)) {
      return {
        settings: parsed,
        status_message: null,
      };
    }
  } catch {
    // fallback 统一在下方返回默认设置和提示。
  }

  return {
    settings: defaultSettings(),
    status_message: "配置损坏，已使用默认值",
  };
};

/// 校验浏览器 fallback 设置结构。
const isBuilderPanelSettings = (
  value: unknown,
): value is BuilderPanelSettings => {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Partial<BuilderPanelSettings>;
  return (
    isGeneralSettings(candidate.general) &&
    isDisplaySettings(candidate.display) &&
    isPanelSettings(candidate.panel) &&
    isAgentSettings(candidate.agents) &&
    isReplySettings(candidate.replies) &&
    isPresetSettings(candidate.presets) &&
    isTerminalSettings(candidate.terminal) &&
    isAdvancedSettings(candidate.advanced)
  );
};

/// 校验通用设置。
const isGeneralSettings = (
  value: unknown,
): value is BuilderPanelSettings["general"] => {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Partial<BuilderPanelSettings["general"]>;
  return (
    typeof candidate.keep_panel_on_top === "boolean" &&
    typeof candidate.notify_on_completion === "boolean" &&
    typeof candidate.notify_on_waiting === "boolean"
  );
};

/// 校验展示设置。
const isDisplaySettings = (
  value: unknown,
): value is BuilderPanelSettings["display"] => {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Partial<BuilderPanelSettings["display"]>;
  return (
    typeof candidate.show_usage === "boolean" &&
    (candidate.density === "comfortable" || candidate.density === "compact") &&
    (candidate.animation_level === "full" ||
      candidate.animation_level === "reduced")
  );
};

/// 校验 panel 设置。
const isPanelSettings = (
  value: unknown,
): value is BuilderPanelSettings["panel"] => {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Partial<BuilderPanelSettings["panel"]>;
  return (
    typeof candidate.collapsed === "boolean" &&
    isPanelWindowPosition(candidate.window_position) &&
    isPanelWindowSize(candidate.window_size)
  );
};

/// 校验 panel 位置。
const isPanelWindowPosition = (
  value: unknown,
): value is BuilderPanelSettings["panel"]["window_position"] => {
  if (value === null) {
    return true;
  }
  if (typeof value !== "object") {
    return false;
  }

  const candidate = value as Partial<
    NonNullable<BuilderPanelSettings["panel"]["window_position"]>
  >;
  return Number.isInteger(candidate.x) && Number.isInteger(candidate.y);
};

/// 校验 panel 尺寸。
const isPanelWindowSize = (
  value: unknown,
): value is BuilderPanelSettings["panel"]["window_size"] => {
  if (value === null) {
    return true;
  }
  if (typeof value !== "object") {
    return false;
  }

  const candidate = value as Partial<
    NonNullable<BuilderPanelSettings["panel"]["window_size"]>
  >;
  const width = candidate.width;
  const height = candidate.height;
  return (
    Number.isInteger(width) &&
    Number.isInteger(height) &&
    width !== undefined &&
    height !== undefined &&
    width > 0 &&
    height > 0
  );
};

/// 校验 Agent 设置。
const isAgentSettings = (
  value: unknown,
): value is BuilderPanelSettings["agents"] => {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Partial<BuilderPanelSettings["agents"]>;
  return (
    typeof candidate.mock_agent_enabled === "boolean" &&
    typeof candidate.codex_cli_enabled === "boolean" &&
    typeof candidate.codex_app_enabled === "boolean" &&
    typeof candidate.claude_cli_enabled === "boolean" &&
    typeof candidate.claude_app_enabled === "boolean"
  );
};

/// 校验回复设置。
const isReplySettings = (
  value: unknown,
): value is BuilderPanelSettings["replies"] => {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Partial<BuilderPanelSettings["replies"]>;
  return (
    typeof candidate.enter_to_send === "boolean" &&
    typeof candidate.shortcut_replies_enabled === "boolean"
  );
};

/// 校验预设设置。
const isPresetSettings = (
  value: unknown,
): value is BuilderPanelSettings["presets"] => {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Partial<BuilderPanelSettings["presets"]>;
  return typeof candidate.prefer_structured_create === "boolean";
};

/// 校验终端设置。
const isTerminalSettings = (
  value: unknown,
): value is BuilderPanelSettings["terminal"] => {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Partial<BuilderPanelSettings["terminal"]>;
  return (
    typeof candidate.jump_enabled === "boolean" &&
    typeof candidate.copy_fallback_enabled === "boolean"
  );
};

/// 校验高级设置。
const isAdvancedSettings = (
  value: unknown,
): value is BuilderPanelSettings["advanced"] => {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as Partial<BuilderPanelSettings["advanced"]>;
  return typeof candidate.developer_diagnostics === "boolean";
};

/// 创建保留 cause 的错误。
const errorWithCause = (error: unknown, fallback: string): Error => {
  if (error instanceof Error && error.message.length > 0) {
    return new Error(error.message, { cause: error });
  }
  if (typeof error === "string" && error.length > 0) {
    return new Error(error, { cause: error });
  }

  return new Error(fallback, { cause: error });
};

/// 判断当前是否运行在 Tauri 环境。
const isTauriRuntime = (): boolean => {
  return "__TAURI_INTERNALS__" in window;
};
