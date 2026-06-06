//! 设置应用服务。

use serde::{Deserialize, Serialize};

use crate::domain::app_error::AppError;
use crate::ports::config_store_port::SettingsStorePort;

/// UI 密度。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDensity {
    /// 标准密度。
    Comfortable,
    /// 紧凑密度。
    Compact,
}

impl Default for UiDensity {
    /// 返回默认 UI 密度。
    fn default() -> Self {
        Self::Comfortable
    }
}

/// UI 主题。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTheme {
    /// 浅色主题。
    Light,
    /// 深色主题。
    Dark,
}

impl Default for UiTheme {
    /// 返回默认 UI 主题。
    fn default() -> Self {
        Self::Light
    }
}

/// 动画等级。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationLevel {
    /// 完整动画。
    Full,
    /// 降级动画。
    Reduced,
}

impl Default for AnimationLevel {
    /// 返回默认动画等级。
    fn default() -> Self {
        Self::Full
    }
}

/// 通用设置。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct GeneralSettings {
    /// 是否启动后保持面板置顶。
    pub keep_panel_on_top: bool,
    /// 是否启用完成通知。
    pub notify_on_completion: bool,
    /// 是否启用等待用户操作通知。
    pub notify_on_waiting: bool,
}

impl Default for GeneralSettings {
    /// 返回通用设置默认值。
    fn default() -> Self {
        Self {
            keep_panel_on_top: true,
            notify_on_completion: true,
            notify_on_waiting: true,
        }
    }
}

/// 展示设置。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct DisplaySettings {
    /// 是否展示用量信息。
    pub show_usage: bool,
    /// UI 主题。
    pub theme: UiTheme,
    /// UI 密度。
    pub density: UiDensity,
    /// 动画等级。
    pub animation_level: AnimationLevel,
}

/// panel 窗口位置。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PanelWindowPosition {
    /// 物理像素横坐标。
    pub x: i32,
    /// 物理像素纵坐标。
    pub y: i32,
}

/// panel 窗口尺寸。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PanelWindowSize {
    /// 物理像素宽度。
    pub width: u32,
    /// 物理像素高度。
    pub height: u32,
}

/// panel 展示状态。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct PanelSettings {
    /// 是否处于收缩状态。
    pub collapsed: bool,
    /// 上次窗口位置。
    pub window_position: Option<PanelWindowPosition>,
    /// 上次窗口尺寸。
    pub window_size: Option<PanelWindowSize>,
}

impl Default for PanelSettings {
    /// 返回 panel 状态默认值。
    fn default() -> Self {
        Self {
            collapsed: false,
            window_position: None,
            window_size: None,
        }
    }
}

impl Default for DisplaySettings {
    /// 返回展示设置默认值。
    fn default() -> Self {
        Self {
            show_usage: true,
            theme: UiTheme::Light,
            density: UiDensity::Comfortable,
            animation_level: AnimationLevel::Full,
        }
    }
}

/// Agent 接入设置。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct AgentSettings {
    /// 是否启用 mock agent。
    pub mock_agent_enabled: bool,
    /// 是否启用 Codex CLI。
    pub codex_cli_enabled: bool,
    /// 是否启用 Codex APP 探针。
    pub codex_app_enabled: bool,
    /// 是否启用 Claude Code CLI。
    pub claude_cli_enabled: bool,
    /// 是否启用 Claude Code APP。
    pub claude_app_enabled: bool,
}

impl Default for AgentSettings {
    /// 返回 Agent 接入默认值。
    fn default() -> Self {
        Self {
            mock_agent_enabled: true,
            codex_cli_enabled: true,
            codex_app_enabled: false,
            claude_cli_enabled: false,
            claude_app_enabled: false,
        }
    }
}

/// 回复设置。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct ReplySettings {
    /// 是否启用 Enter 发送。
    pub enter_to_send: bool,
    /// 是否启用快捷回复。
    pub shortcut_replies_enabled: bool,
}

impl Default for ReplySettings {
    /// 返回回复设置默认值。
    fn default() -> Self {
        Self {
            enter_to_send: true,
            shortcut_replies_enabled: true,
        }
    }
}

/// 预设命令设置。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct PresetSettings {
    /// 是否优先使用结构化创建。
    pub prefer_structured_create: bool,
}

impl Default for PresetSettings {
    /// 返回预设命令默认值。
    fn default() -> Self {
        Self {
            prefer_structured_create: true,
        }
    }
}

/// 终端设置。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct TerminalSettings {
    /// 是否展示跳回能力。
    pub jump_enabled: bool,
    /// 是否允许复制降级。
    pub copy_fallback_enabled: bool,
}

impl Default for TerminalSettings {
    /// 返回终端设置默认值。
    fn default() -> Self {
        Self {
            jump_enabled: true,
            copy_fallback_enabled: true,
        }
    }
}

/// 高级设置。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct AdvancedSettings {
    /// 是否展示开发诊断信息。
    pub developer_diagnostics: bool,
}

impl Default for AdvancedSettings {
    /// 返回高级设置默认值。
    fn default() -> Self {
        Self {
            developer_diagnostics: false,
        }
    }
}

/// Builder Panel 设置。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct BuilderPanelSettings {
    /// 通用设置。
    #[serde(default)]
    pub general: GeneralSettings,
    /// 展示设置。
    #[serde(default)]
    pub display: DisplaySettings,
    /// panel 展示状态。
    #[serde(default)]
    pub panel: PanelSettings,
    /// Agent 接入设置。
    #[serde(default)]
    pub agents: AgentSettings,
    /// 回复设置。
    #[serde(default)]
    pub replies: ReplySettings,
    /// 预设命令设置。
    #[serde(default)]
    pub presets: PresetSettings,
    /// 终端设置。
    #[serde(default)]
    pub terminal: TerminalSettings,
    /// 高级设置。
    #[serde(default)]
    pub advanced: AdvancedSettings,
}

impl BuilderPanelSettings {
    /// 创建默认设置。
    pub fn defaults() -> Self {
        Self::default()
    }
}

impl Default for BuilderPanelSettings {
    /// 返回 Builder Panel 设置默认值。
    fn default() -> Self {
        Self {
            general: GeneralSettings::default(),
            display: DisplaySettings::default(),
            panel: PanelSettings::default(),
            agents: AgentSettings::default(),
            replies: ReplySettings::default(),
            presets: PresetSettings::default(),
            terminal: TerminalSettings::default(),
            advanced: AdvancedSettings::default(),
        }
    }
}

/// 设置读取结果。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SettingsViewModel {
    /// 当前有效设置。
    pub settings: BuilderPanelSettings,
    /// 配置读取提示。
    pub status_message: Option<String>,
}

/// 设置应用服务。
pub struct SettingsService<'a, Store>
where
    Store: SettingsStorePort,
{
    /// 设置存储端口。
    store: &'a Store,
}

impl<'a, Store> SettingsService<'a, Store>
where
    Store: SettingsStorePort,
{
    /// 创建设置应用服务。
    pub fn new(store: &'a Store) -> Self {
        Self { store }
    }

    /// 读取设置；配置损坏时使用默认设置并返回提示。
    pub fn read_settings(&self) -> SettingsViewModel {
        match self.store.load_settings() {
            Ok(Some(settings)) => SettingsViewModel {
                settings,
                status_message: None,
            },
            Ok(None) => SettingsViewModel {
                settings: BuilderPanelSettings::defaults(),
                status_message: None,
            },
            Err(_) => SettingsViewModel {
                settings: BuilderPanelSettings::defaults(),
                status_message: Some("配置损坏，已使用默认值".to_string()),
            },
        }
    }

    /// 保存设置并返回保存后的结果。
    pub fn save_settings(
        &self,
        settings: BuilderPanelSettings,
    ) -> Result<SettingsViewModel, AppError> {
        self.store.save_settings(&settings)?;

        Ok(SettingsViewModel {
            settings,
            status_message: Some("设置已保存".to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{BuilderPanelSettings, SettingsService};
    use crate::domain::app_error::{AppError, AppErrorCode, FallbackAction};
    use crate::ports::config_store_port::SettingsStorePort;
    use std::cell::RefCell;

    #[test]
    fn missing_settings_uses_defaults_without_warning() {
        let store = FakeSettingsStore::missing();
        let service = SettingsService::new(&store);
        let view = service.read_settings();

        assert_eq!(view.settings, BuilderPanelSettings::defaults());
        assert_eq!(view.status_message, None);
    }

    #[test]
    fn corrupted_settings_uses_defaults_with_warning() {
        let store = FakeSettingsStore::corrupted();
        let service = SettingsService::new(&store);
        let view = service.read_settings();

        assert_eq!(view.settings, BuilderPanelSettings::defaults());
        assert_eq!(
            view.status_message,
            Some("配置损坏，已使用默认值".to_string())
        );
    }

    #[test]
    fn save_settings_persists_explicit_model() {
        let store = FakeSettingsStore::missing();
        let service = SettingsService::new(&store);
        let mut settings = BuilderPanelSettings::defaults();
        settings.display.show_usage = false;
        let view = service
            .save_settings(settings.clone())
            .expect("settings should save");

        assert_eq!(view.settings.display.show_usage, false);
        assert_eq!(store.saved.borrow().as_ref(), Some(&settings));
    }

    #[test]
    fn defaults_include_expanded_panel_without_geometry() {
        let settings = BuilderPanelSettings::defaults();

        assert_eq!(settings.display.theme, Default::default());
        assert_eq!(settings.panel.collapsed, false);
        assert_eq!(settings.panel.window_position, None);
        assert_eq!(settings.panel.window_size, None);
    }

    struct FakeSettingsStore {
        value: Result<Option<BuilderPanelSettings>, AppError>,
        saved: RefCell<Option<BuilderPanelSettings>>,
    }

    impl FakeSettingsStore {
        fn missing() -> Self {
            Self {
                value: Ok(None),
                saved: RefCell::new(None),
            }
        }

        fn corrupted() -> Self {
            Self {
                value: Err(AppError::new(
                    AppErrorCode::ConfigLoadFailed,
                    "配置读取失败",
                    Some("invalid json".to_string()),
                    true,
                    Some(FallbackAction::OpenSettings),
                )),
                saved: RefCell::new(None),
            }
        }
    }

    impl SettingsStorePort for FakeSettingsStore {
        fn load_settings(&self) -> Result<Option<BuilderPanelSettings>, AppError> {
            self.value.clone()
        }

        fn save_settings(&self, settings: &BuilderPanelSettings) -> Result<(), AppError> {
            *self.saved.borrow_mut() = Some(settings.clone());
            Ok(())
        }
    }
}
