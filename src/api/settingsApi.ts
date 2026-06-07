import { invoke } from "@tauri-apps/api/core";

import type {
  BuilderPanelSettings,
  CustomShortcutInput,
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
    theme: "light",
    density: "comfortable",
    animation_level: "full",
  },
  panel: {
    collapsed: false,
    window_position: null,
    window_size: null,
  },
  agents: {
    codex_cli_enabled: true,
    codex_app_enabled: true,
    claude_cli_enabled: false,
    claude_app_enabled: false,
  },
  replies: {
    enter_to_send: true,
    shortcut_replies_enabled: true,
    custom_shortcuts: defaultCustomShortcuts(),
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
  const normalizedSettings = normalizeFallbackSettings(settings) ?? settings;
  try {
    return await invoke<SettingsViewModel>("save_panel_settings", {
      settings: normalizedSettings,
    });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "保存设置失败");
    }
    window.localStorage.setItem(
      FALLBACK_SETTINGS_KEY,
      JSON.stringify(normalizedSettings),
    );
    return {
      settings: normalizedSettings,
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
    const normalizedSettings = normalizeFallbackSettings(parsed);
    if (normalizedSettings !== null) {
      return {
        settings: normalizedSettings,
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

/// 归一浏览器 fallback 设置结构。
export const normalizeFallbackSettings = (
  value: unknown,
): BuilderPanelSettings | null => {
  if (!isObjectRecord(value)) {
    return null;
  }

  const candidate = value as Partial<BuilderPanelSettings>;
  const defaults = defaultSettings();
  const general = normalizeGeneralSettings(candidate.general, defaults.general);
  const display = normalizeDisplaySettings(candidate.display, defaults.display);
  const panel = normalizePanelSettings(candidate.panel, defaults.panel);
  const agents = normalizeAgentSettings(candidate.agents, defaults.agents);
  const replies = normalizeReplySettings(candidate.replies, defaults.replies);
  const presets = normalizePresetSettings(candidate.presets, defaults.presets);
  const terminal = normalizeTerminalSettings(
    candidate.terminal,
    defaults.terminal,
  );
  const advanced = normalizeAdvancedSettings(
    candidate.advanced,
    defaults.advanced,
  );

  if (
    general === null ||
    display === null ||
    panel === null ||
    agents === null ||
    replies === null ||
    presets === null ||
    terminal === null ||
    advanced === null
  ) {
    return null;
  }

  return {
    general,
    display,
    panel,
    agents,
    replies,
    presets,
    terminal,
    advanced,
  };
};

/// 归一通用设置。
const normalizeGeneralSettings = (
  value: unknown,
  defaults: BuilderPanelSettings["general"],
): BuilderPanelSettings["general"] | null => {
  if (value === undefined) {
    return defaults;
  }
  if (!isObjectRecord(value)) {
    return null;
  }

  const candidate = value as Partial<BuilderPanelSettings["general"]>;
  const keepPanelOnTop = normalizeBoolean(
    candidate.keep_panel_on_top,
    defaults.keep_panel_on_top,
  );
  const notifyOnCompletion = normalizeBoolean(
    candidate.notify_on_completion,
    defaults.notify_on_completion,
  );
  const notifyOnWaiting = normalizeBoolean(
    candidate.notify_on_waiting,
    defaults.notify_on_waiting,
  );

  if (
    keepPanelOnTop === null ||
    notifyOnCompletion === null ||
    notifyOnWaiting === null
  ) {
    return null;
  }

  return {
    keep_panel_on_top: keepPanelOnTop,
    notify_on_completion: notifyOnCompletion,
    notify_on_waiting: notifyOnWaiting,
  };
};

/// 归一展示设置。
const normalizeDisplaySettings = (
  value: unknown,
  defaults: BuilderPanelSettings["display"],
): BuilderPanelSettings["display"] | null => {
  if (value === undefined) {
    return defaults;
  }
  if (!isObjectRecord(value)) {
    return null;
  }

  const candidate = value as Partial<BuilderPanelSettings["display"]>;
  const showUsage = normalizeBoolean(candidate.show_usage, defaults.show_usage);
  const theme = normalizeStringUnion(candidate.theme, defaults.theme, [
    "light",
    "dark",
  ]);
  const density = normalizeStringUnion(candidate.density, defaults.density, [
    "comfortable",
    "compact",
  ]);
  const animationLevel = normalizeStringUnion(
    candidate.animation_level,
    defaults.animation_level,
    ["full", "reduced"],
  );

  if (
    showUsage === null ||
    theme === null ||
    density === null ||
    animationLevel === null
  ) {
    return null;
  }

  return {
    show_usage: showUsage,
    theme,
    density,
    animation_level: animationLevel,
  };
};

/// 归一 panel 设置。
const normalizePanelSettings = (
  value: unknown,
  defaults: BuilderPanelSettings["panel"],
): BuilderPanelSettings["panel"] | null => {
  if (value === undefined) {
    return defaults;
  }
  if (!isObjectRecord(value)) {
    return null;
  }

  const candidate = value as Partial<BuilderPanelSettings["panel"]>;
  const collapsed = false;
  const windowPosition = normalizePanelWindowPosition(
    candidate.window_position,
    defaults.window_position,
  );
  const windowSize = normalizePanelWindowSize(
    candidate.window_size,
    defaults.window_size,
  );

  if (windowPosition === undefined || windowSize === undefined) {
    return null;
  }

  return {
    collapsed,
    window_position: windowPosition,
    window_size: windowSize,
  };
};

/// 归一 panel 位置。
const normalizePanelWindowPosition = (
  value: unknown,
  defaults: BuilderPanelSettings["panel"]["window_position"],
): BuilderPanelSettings["panel"]["window_position"] | undefined => {
  if (value === undefined) {
    return defaults;
  }
  if (value === null) {
    return null;
  }
  if (!isObjectRecord(value)) {
    return undefined;
  }

  const candidate = value as Partial<
    NonNullable<BuilderPanelSettings["panel"]["window_position"]>
  >;
  const x = candidate.x;
  const y = candidate.y;
  if (
    x === undefined ||
    y === undefined ||
    !Number.isInteger(x) ||
    !Number.isInteger(y)
  ) {
    return undefined;
  }

  return {
    x,
    y,
  };
};

/// 归一 panel 尺寸。
const normalizePanelWindowSize = (
  value: unknown,
  defaults: BuilderPanelSettings["panel"]["window_size"],
): BuilderPanelSettings["panel"]["window_size"] | undefined => {
  if (value === undefined) {
    return defaults;
  }
  if (value === null) {
    return null;
  }
  if (!isObjectRecord(value)) {
    return undefined;
  }

  const candidate = value as Partial<
    NonNullable<BuilderPanelSettings["panel"]["window_size"]>
  >;
  const width = candidate.width;
  const height = candidate.height;
  if (
    !Number.isInteger(width) ||
    !Number.isInteger(height) ||
    width === undefined ||
    height === undefined ||
    width <= 0 ||
    height <= 0
  ) {
    return undefined;
  }

  return {
    width,
    height,
  };
};

/// 归一 Agent 设置。
const normalizeAgentSettings = (
  value: unknown,
  defaults: BuilderPanelSettings["agents"],
): BuilderPanelSettings["agents"] | null => {
  if (value === undefined) {
    return defaults;
  }
  if (!isObjectRecord(value)) {
    return null;
  }

  const candidate = value as Partial<BuilderPanelSettings["agents"]>;
  const codexCliEnabled = normalizeBoolean(
    candidate.codex_cli_enabled,
    defaults.codex_cli_enabled,
  );
  const codexAppEnabled = normalizeBoolean(
    candidate.codex_app_enabled,
    defaults.codex_app_enabled,
  );
  const claudeCliEnabled = normalizeBoolean(
    candidate.claude_cli_enabled,
    defaults.claude_cli_enabled,
  );
  const claudeAppEnabled = normalizeBoolean(
    candidate.claude_app_enabled,
    defaults.claude_app_enabled,
  );

  if (
    codexCliEnabled === null ||
    codexAppEnabled === null ||
    claudeCliEnabled === null ||
    claudeAppEnabled === null
  ) {
    return null;
  }

  return {
    codex_cli_enabled: codexCliEnabled,
    codex_app_enabled: codexAppEnabled,
    claude_cli_enabled: claudeCliEnabled,
    claude_app_enabled: claudeAppEnabled,
  };
};

/// 归一回复设置。
const normalizeReplySettings = (
  value: unknown,
  defaults: BuilderPanelSettings["replies"],
): BuilderPanelSettings["replies"] | null => {
  if (value === undefined) {
    return defaults;
  }
  if (!isObjectRecord(value)) {
    return null;
  }

  const candidate = value as Partial<BuilderPanelSettings["replies"]>;
  const enterToSend = normalizeBoolean(
    candidate.enter_to_send,
    defaults.enter_to_send,
  );
  const shortcutRepliesEnabled = normalizeBoolean(
    candidate.shortcut_replies_enabled,
    defaults.shortcut_replies_enabled,
  );
  const customShortcuts = normalizeCustomShortcuts(
    candidate.custom_shortcuts,
    defaults.custom_shortcuts,
  );

  if (
    enterToSend === null ||
    shortcutRepliesEnabled === null ||
    customShortcuts === null
  ) {
    return null;
  }

  return {
    enter_to_send: enterToSend,
    shortcut_replies_enabled: shortcutRepliesEnabled,
    custom_shortcuts: customShortcuts,
  };
};

/// 默认自定义快捷输入。
export const defaultCustomShortcuts = (): readonly CustomShortcutInput[] => [
  {
    id: "continue",
    label: "继续",
    content: "继续按当前方案执行。",
    enabled: true,
    order: 10,
  },
  {
    id: "need-boundary",
    label: "补充边界",
    content: "请优先说明输入、输出、边界条件和失败处理。",
    enabled: true,
    order: 20,
  },
];

/// 归一自定义快捷输入。
export const normalizeCustomShortcuts = (
  value: unknown,
  defaults: readonly CustomShortcutInput[],
): readonly CustomShortcutInput[] | null => {
  if (value === undefined) {
    return defaults;
  }
  if (!Array.isArray(value)) {
    return null;
  }

  const seenIds = new Set<string>();
  const shortcuts: CustomShortcutInput[] = [];
  for (const item of value) {
    const shortcut = normalizeCustomShortcut(item);
    if (shortcut === null || seenIds.has(shortcut.id)) {
      continue;
    }
    seenIds.add(shortcut.id);
    shortcuts.push(shortcut);
  }

  return shortcuts.sort((left, right) => {
    if (left.order !== right.order) {
      return left.order - right.order;
    }
    if (left.label !== right.label) {
      return left.label.localeCompare(right.label);
    }
    return left.id.localeCompare(right.id);
  });
};

/// 归一单条自定义快捷输入。
const normalizeCustomShortcut = (
  value: unknown,
): CustomShortcutInput | null => {
  if (!isObjectRecord(value)) {
    return null;
  }

  const candidate = value as Partial<CustomShortcutInput>;
  const id = normalizeTrimmedText(candidate.id, 80);
  const label = normalizeTrimmedText(candidate.label, 80);
  const content = normalizeTrimmedText(candidate.content, 1000);
  const enabled = normalizeBoolean(candidate.enabled, true);
  const order = candidate.order;
  if (
    id === null ||
    label === null ||
    content === null ||
    enabled === null ||
    !Number.isInteger(order) ||
    order === undefined ||
    order < 0
  ) {
    return null;
  }

  return {
    id,
    label,
    content,
    enabled,
    order,
  };
};

/// 归一非空文本。
const normalizeTrimmedText = (
  value: unknown,
  maxChars: number,
): string | null => {
  if (typeof value !== "string") {
    return null;
  }

  const text = value.trim();
  if (text.length === 0 || [...text].length > maxChars) {
    return null;
  }

  return text;
};

/// 归一预设设置。
const normalizePresetSettings = (
  value: unknown,
  defaults: BuilderPanelSettings["presets"],
): BuilderPanelSettings["presets"] | null => {
  if (value === undefined) {
    return defaults;
  }
  if (!isObjectRecord(value)) {
    return null;
  }

  const candidate = value as Partial<BuilderPanelSettings["presets"]>;
  const preferStructuredCreate = normalizeBoolean(
    candidate.prefer_structured_create,
    defaults.prefer_structured_create,
  );
  if (preferStructuredCreate === null) {
    return null;
  }

  return {
    prefer_structured_create: preferStructuredCreate,
  };
};

/// 归一终端设置。
const normalizeTerminalSettings = (
  value: unknown,
  defaults: BuilderPanelSettings["terminal"],
): BuilderPanelSettings["terminal"] | null => {
  if (value === undefined) {
    return defaults;
  }
  if (!isObjectRecord(value)) {
    return null;
  }

  const candidate = value as Partial<BuilderPanelSettings["terminal"]>;
  const jumpEnabled = normalizeBoolean(
    candidate.jump_enabled,
    defaults.jump_enabled,
  );
  const copyFallbackEnabled = normalizeBoolean(
    candidate.copy_fallback_enabled,
    defaults.copy_fallback_enabled,
  );

  if (jumpEnabled === null || copyFallbackEnabled === null) {
    return null;
  }

  return {
    jump_enabled: jumpEnabled,
    copy_fallback_enabled: copyFallbackEnabled,
  };
};

/// 归一高级设置。
const normalizeAdvancedSettings = (
  value: unknown,
  defaults: BuilderPanelSettings["advanced"],
): BuilderPanelSettings["advanced"] | null => {
  if (value === undefined) {
    return defaults;
  }
  if (!isObjectRecord(value)) {
    return null;
  }

  const candidate = value as Partial<BuilderPanelSettings["advanced"]>;
  const developerDiagnostics = normalizeBoolean(
    candidate.developer_diagnostics,
    defaults.developer_diagnostics,
  );
  if (developerDiagnostics === null) {
    return null;
  }

  return {
    developer_diagnostics: developerDiagnostics,
  };
};

/// 归一布尔字段。
const normalizeBoolean = (
  value: unknown,
  defaults: boolean,
): boolean | null => {
  if (value === undefined) {
    return defaults;
  }
  if (typeof value === "boolean") {
    return value;
  }

  return null;
};

/// 归一字符串枚举字段。
const normalizeStringUnion = <Value extends string>(
  value: unknown,
  defaults: Value,
  allowedValues: readonly Value[],
): Value | null => {
  if (value === undefined) {
    return defaults;
  }
  if (typeof value === "string" && allowedValues.includes(value as Value)) {
    return value as Value;
  }

  return null;
};

/// 判断值是否为对象记录。
const isObjectRecord = (value: unknown): value is Record<string, unknown> => {
  return typeof value === "object" && value !== null && !Array.isArray(value);
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
