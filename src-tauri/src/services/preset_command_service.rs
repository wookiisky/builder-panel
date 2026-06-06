//! 预设命令应用服务。

use serde::{Deserialize, Serialize};

use crate::domain::agent_session::AgentKind;

/// 预设命令唯一标识。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PresetCommandId {
    /// 配置内稳定 ID。
    pub value: String,
}

/// 预设命令配置。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PresetCommand {
    /// 预设命令唯一标识。
    pub id: PresetCommandId,
    /// 展示名称。
    pub label: String,
    /// 目标 agent 类型。
    pub agent_kind: AgentKind,
    /// 工作目录。
    pub working_directory: String,
    /// 启动命令。
    pub launch_command: Vec<String>,
    /// 初始 prompt。
    pub initial_prompt: Option<String>,
    /// 是否自动发送回车。
    pub auto_submit: bool,
    /// 默认快捷回复组 ID。
    pub default_shortcut_group_id: Option<String>,
}

/// 创建新对话能力。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CreateConversationCapabilities {
    /// 是否支持结构化创建。
    pub can_create_structured: bool,
    /// 是否支持启动托管进程。
    pub can_start_managed_process: bool,
}

/// 创建新对话计划类型。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresetCommandPlanKind {
    /// 使用结构化 API 创建。
    Structured,
    /// 启动托管进程。
    ManagedProcess,
    /// 只能复制命令。
    ClipboardFallback,
}

/// 创建新对话计划。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PresetCommandPlan {
    /// 计划类型。
    pub kind: PresetCommandPlanKind,
    /// 工作目录。
    pub working_directory: String,
    /// 最终命令行。
    pub command_line: String,
    /// 是否自动发送回车。
    pub auto_submit: bool,
    /// 可选复制降级原因。
    pub fallback_reason: Option<String>,
}

/// 预设命令应用服务。
pub struct PresetCommandService;

impl PresetCommandService {
    /// 生成创建新对话计划。
    pub fn build_plan(
        preset: &PresetCommand,
        capabilities: &CreateConversationCapabilities,
    ) -> PresetCommandPlan {
        let kind = if capabilities.can_create_structured {
            PresetCommandPlanKind::Structured
        } else if capabilities.can_start_managed_process {
            PresetCommandPlanKind::ManagedProcess
        } else {
            PresetCommandPlanKind::ClipboardFallback
        };
        let fallback_reason = match kind {
            PresetCommandPlanKind::ClipboardFallback => {
                Some("当前环境不支持可靠创建，只能复制命令".to_string())
            }
            PresetCommandPlanKind::Structured | PresetCommandPlanKind::ManagedProcess => None,
        };

        PresetCommandPlan {
            kind,
            working_directory: preset.working_directory.clone(),
            command_line: command_line(preset),
            auto_submit: preset.auto_submit,
            fallback_reason,
        }
    }
}

/// 生成最终命令行。
fn command_line(preset: &PresetCommand) -> String {
    let mut parts = preset.launch_command.clone();
    if let Some(prompt) = &preset.initial_prompt {
        if !prompt.trim().is_empty() {
            parts.push(prompt.clone());
        }
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::{
        CreateConversationCapabilities, PresetCommand, PresetCommandId, PresetCommandPlanKind,
        PresetCommandService,
    };
    use crate::domain::agent_session::AgentKind;

    #[test]
    fn structured_create_is_preferred() {
        let plan = PresetCommandService::build_plan(
            &preset(true),
            &CreateConversationCapabilities {
                can_create_structured: true,
                can_start_managed_process: true,
            },
        );

        assert_eq!(plan.kind, PresetCommandPlanKind::Structured);
        assert_eq!(plan.command_line, "codex 请分析当前项目");
        assert!(plan.auto_submit);
        assert_eq!(plan.fallback_reason, None);
    }

    #[test]
    fn managed_process_is_used_when_structured_create_is_missing() {
        let plan = PresetCommandService::build_plan(
            &preset(false),
            &CreateConversationCapabilities {
                can_create_structured: false,
                can_start_managed_process: true,
            },
        );

        assert_eq!(plan.kind, PresetCommandPlanKind::ManagedProcess);
        assert!(!plan.auto_submit);
    }

    #[test]
    fn clipboard_fallback_is_explicit() {
        let plan = PresetCommandService::build_plan(
            &preset(false),
            &CreateConversationCapabilities {
                can_create_structured: false,
                can_start_managed_process: false,
            },
        );

        assert_eq!(plan.kind, PresetCommandPlanKind::ClipboardFallback);
        assert_eq!(
            plan.fallback_reason,
            Some("当前环境不支持可靠创建，只能复制命令".to_string())
        );
    }

    fn preset(auto_submit: bool) -> PresetCommand {
        PresetCommand {
            id: PresetCommandId {
                value: "codex-default".to_string(),
            },
            label: "Codex 默认".to_string(),
            agent_kind: AgentKind::CodexCli,
            working_directory: "/tmp/project".to_string(),
            launch_command: vec!["codex".to_string()],
            initial_prompt: Some("请分析当前项目".to_string()),
            auto_submit,
            default_shortcut_group_id: Some("default".to_string()),
        }
    }
}
