/// UI 密度。
export type UiDensity = "comfortable" | "compact";

/// 动画等级。
export type AnimationLevel = "full" | "reduced";

/// 通用设置。
export interface GeneralSettings {
  /// 是否保持面板置顶。
  readonly keep_panel_on_top: boolean;
  /// 是否启用完成通知。
  readonly notify_on_completion: boolean;
  /// 是否启用等待用户操作通知。
  readonly notify_on_waiting: boolean;
}

/// 展示设置。
export interface DisplaySettings {
  /// 是否展示用量信息。
  readonly show_usage: boolean;
  /// UI 密度。
  readonly density: UiDensity;
  /// 动画等级。
  readonly animation_level: AnimationLevel;
}

/// panel 窗口位置。
export interface PanelWindowPosition {
  /// 物理像素横坐标。
  readonly x: number;
  /// 物理像素纵坐标。
  readonly y: number;
}

/// panel 窗口尺寸。
export interface PanelWindowSize {
  /// 物理像素宽度。
  readonly width: number;
  /// 物理像素高度。
  readonly height: number;
}

/// panel 展示状态。
export interface PanelSettings {
  /// 是否处于收缩状态。
  readonly collapsed: boolean;
  /// 上次窗口位置。
  readonly window_position: PanelWindowPosition | null;
  /// 上次窗口尺寸。
  readonly window_size: PanelWindowSize | null;
}

/// Agent 接入设置。
export interface AgentSettings {
  /// 是否启用 mock agent。
  readonly mock_agent_enabled: boolean;
  /// 是否启用 Codex CLI。
  readonly codex_cli_enabled: boolean;
  /// 是否启用 Codex APP。
  readonly codex_app_enabled: boolean;
  /// 是否启用 Claude Code CLI。
  readonly claude_cli_enabled: boolean;
  /// 是否启用 Claude Code APP。
  readonly claude_app_enabled: boolean;
}

/// 回复设置。
export interface ReplySettings {
  /// 是否启用 Enter 发送。
  readonly enter_to_send: boolean;
  /// 是否启用快捷回复。
  readonly shortcut_replies_enabled: boolean;
}

/// 预设命令设置。
export interface PresetSettings {
  /// 是否优先结构化创建。
  readonly prefer_structured_create: boolean;
}

/// 终端设置。
export interface TerminalSettings {
  /// 是否启用跳回入口。
  readonly jump_enabled: boolean;
  /// 是否启用复制降级。
  readonly copy_fallback_enabled: boolean;
}

/// 高级设置。
export interface AdvancedSettings {
  /// 是否展示开发诊断。
  readonly developer_diagnostics: boolean;
}

/// Builder Panel 设置。
export interface BuilderPanelSettings {
  /// 通用设置。
  readonly general: GeneralSettings;
  /// 展示设置。
  readonly display: DisplaySettings;
  /// panel 展示状态。
  readonly panel: PanelSettings;
  /// Agent 接入设置。
  readonly agents: AgentSettings;
  /// 回复设置。
  readonly replies: ReplySettings;
  /// 预设命令设置。
  readonly presets: PresetSettings;
  /// 终端设置。
  readonly terminal: TerminalSettings;
  /// 高级设置。
  readonly advanced: AdvancedSettings;
}

/// 设置读取结果。
export interface SettingsViewModel {
  /// 当前有效设置。
  readonly settings: BuilderPanelSettings;
  /// 配置读取或保存提示。
  readonly status_message: string | null;
}
