//! Agent 会话身份、状态和能力模型。

use serde::{Deserialize, Serialize};

use crate::domain::agent_interaction::AgentInteraction;
use crate::domain::app_error::AppError;
use crate::domain::usage::{UnixMillis, UsageSnapshot};

/// Agent 来源类型。
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    /// Codex 桌面 APP。
    CodexApp,
    /// Codex CLI。
    CodexCli,
    /// Claude Code 桌面 APP。
    ClaudeCodeApp,
    /// Claude Code CLI。
    ClaudeCodeCli,
}

impl AgentKind {
    /// 返回 UI 可读的 agent 标签。
    pub fn label(&self) -> &'static str {
        match self {
            Self::CodexApp => "Codex APP",
            Self::CodexCli => "Codex CLI",
            Self::ClaudeCodeApp => "Claude Code APP",
            Self::ClaudeCodeCli => "Claude Code CLI",
        }
    }
}

/// 项目稳定标识。
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProjectId {
    /// adapter 清洗后的项目 ID。
    pub value: String,
}

impl ProjectId {
    /// 创建项目稳定标识。
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

/// 对话稳定标识。
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ConversationId {
    /// adapter 清洗后的对话 ID。
    pub value: String,
}

impl ConversationId {
    /// 创建对话稳定标识。
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

/// 会话唯一键，由 agent、项目和对话共同确定。
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct SessionKey {
    /// Agent 来源类型。
    pub agent_kind: AgentKind,
    /// 项目稳定标识。
    pub project_id: ProjectId,
    /// 对话稳定标识。
    pub conversation_id: ConversationId,
}

impl SessionKey {
    /// 创建会话唯一键。
    pub fn new(
        agent_kind: AgentKind,
        project_id: ProjectId,
        conversation_id: ConversationId,
    ) -> Self {
        Self {
            agent_kind,
            project_id,
            conversation_id,
        }
    }
}

/// 会话运行状态。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Agent 正在工作。
    Running,
    /// 等待用户审批。
    WaitingForApproval,
    /// 等待用户选择或文本回复。
    WaitingForAnswer,
    /// 当前 turn 完成。
    Completed,
    /// Agent 或 bridge 出错。
    Failed,
    /// 会话失联或本地进程不可见。
    Detached,
}

impl SessionStatus {
    /// 返回 UI 可读的状态标签。
    pub fn label(&self) -> &'static str {
        match self {
            Self::Running => "运行中",
            Self::WaitingForApproval => "等待审批",
            Self::WaitingForAnswer => "等待回复",
            Self::Completed => "已完成",
            Self::Failed => "失败",
            Self::Detached => "失联",
        }
    }
}

/// 当前会话可执行能力。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionCapabilities {
    /// 是否可跳回 agent 所在 APP 或终端。
    pub can_jump: bool,
    /// 是否可发送开放性回复。
    pub can_send_reply: bool,
    /// 是否可处理审批。
    pub can_resolve_approval: bool,
    /// 是否可创建后续 turn。
    pub can_create_followup_turn: bool,
}

impl SessionCapabilities {
    /// 创建全能力关闭的默认能力。
    pub fn none() -> Self {
        Self {
            can_jump: false,
            can_send_reply: false,
            can_resolve_approval: false,
            can_create_followup_turn: false,
        }
    }
}

/// 跳回目标。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct JumpTarget {
    /// 跳回目标标签。
    pub label: String,
    /// adapter 清洗后的目标定位信息。
    pub location: String,
}

/// Agent 会话状态。
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentSession {
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// 项目展示标签。
    pub project_label: String,
    /// 对话展示标签。
    pub conversation_label: String,
    /// 可选对话标题。
    pub title: Option<String>,
    /// 已清洗活动摘要。
    pub summary: Option<String>,
    /// 当前运行状态。
    pub status: SessionStatus,
    /// 当前会话能力。
    pub capabilities: SessionCapabilities,
    /// 当前用量快照。
    pub usage: UsageSnapshot,
    /// 当前等待用户处理的交互。
    pub pending_interaction: Option<AgentInteraction>,
    /// 最近一次错误。
    pub last_error: Option<AppError>,
    /// 可选跳回目标。
    pub jump_target: Option<JumpTarget>,
    /// 首次被当前状态捕捉到的稳定顺序。
    #[serde(default)]
    pub capture_sequence: u64,
    /// 当前 turn 开始时间。
    pub started_at: UnixMillis,
    /// 当前 turn 结束时间。
    pub completed_at: Option<UnixMillis>,
    /// 最近更新时间。
    pub updated_at: UnixMillis,
}

impl AgentSession {
    /// 创建运行中的基础会话。
    pub fn new_running(
        session_key: SessionKey,
        project_label: impl Into<String>,
        conversation_label: impl Into<String>,
        updated_at: UnixMillis,
    ) -> Self {
        Self {
            session_key,
            project_label: project_label.into(),
            conversation_label: conversation_label.into(),
            title: None,
            summary: None,
            status: SessionStatus::Running,
            capabilities: SessionCapabilities::none(),
            usage: UsageSnapshot::unavailable(),
            pending_interaction: None,
            last_error: None,
            jump_target: None,
            capture_sequence: 0,
            started_at: updated_at,
            completed_at: None,
            updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentKind, ConversationId, ProjectId, SessionCapabilities, SessionKey};

    #[test]
    fn session_key_distinguishes_project_and_conversation() {
        let first = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("project-a"),
            ConversationId::new("conversation-a"),
        );
        let same = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("project-a"),
            ConversationId::new("conversation-a"),
        );
        let other_project = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("project-b"),
            ConversationId::new("conversation-a"),
        );
        let other_conversation = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("project-a"),
            ConversationId::new("conversation-b"),
        );

        assert_eq!(first, same);
        assert_ne!(first, other_project);
        assert_ne!(first, other_conversation);
        assert!(first < other_project);
    }

    #[test]
    fn session_capabilities_cover_all_ui_actions() {
        let capabilities = SessionCapabilities {
            can_jump: true,
            can_send_reply: true,
            can_resolve_approval: true,
            can_create_followup_turn: true,
        };

        assert!(capabilities.can_jump);
        assert!(capabilities.can_send_reply);
        assert!(capabilities.can_resolve_approval);
        assert!(capabilities.can_create_followup_turn);
    }
}
