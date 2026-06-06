//! Agent 等待用户处理的交互模型。

use serde::{Deserialize, Serialize};

use crate::domain::agent_session::SessionKey;
use crate::domain::usage::UnixMillis;

/// 交互唯一标识。
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct InteractionId {
    /// adapter 清洗后的交互 ID。
    pub value: String,
}

impl InteractionId {
    /// 创建交互唯一标识。
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

/// 交互处理状态。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionStatus {
    /// 等待用户处理。
    Pending,
    /// 已由用户处理。
    Resolved,
    /// 已过期。
    Expired,
}

/// 结构化 RPC 回复目标。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StructuredRpcTarget {
    /// RPC 目标 ID。
    pub target_id: String,
}

/// hook stdout directive 回复目标。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HookDirectiveTarget {
    /// hook 请求 ID。
    pub request_id: String,
}

/// 托管进程 stdin 回复目标。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ManagedProcessTarget {
    /// 托管进程 ID。
    pub process_id: String,
}

/// 受控终端回复目标。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ControlledTerminalTarget {
    /// 终端目标 ID。
    pub terminal_id: String,
}

/// 剪贴板降级目标。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClipboardFallbackTarget {
    /// 降级原因。
    pub reason: String,
}

/// 回复目标。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyTarget {
    /// 通过结构化 RPC 回写。
    StructuredRpc(StructuredRpcTarget),
    /// 通过 hook stdout directive 回写。
    HookDirective(HookDirectiveTarget),
    /// 通过托管进程 stdin 回写。
    ManagedProcessStdin(ManagedProcessTarget),
    /// 通过受控终端输入。
    ControlledTerminal(ControlledTerminalTarget),
    /// 不支持自动回写，只能复制。
    ClipboardOnly(ClipboardFallbackTarget),
}

/// 审批请求交互。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApprovalInteraction {
    /// 交互唯一标识。
    pub interaction_id: InteractionId,
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// 创建时间。
    pub created_at: UnixMillis,
    /// 可选过期时间。
    pub expires_at: Option<UnixMillis>,
    /// 回复目标。
    pub reply_target: ReplyTarget,
    /// 当前处理状态。
    pub status: InteractionStatus,
    /// 已清洗的请求摘要。
    pub request_summary: String,
}

/// 选项项。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InteractionChoice {
    /// 选项稳定值。
    pub value: String,
    /// 选项展示标签。
    pub label: String,
    /// 可选悬停说明。
    pub tooltip: Option<String>,
}

/// 选择请求交互。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChoiceInteraction {
    /// 交互唯一标识。
    pub interaction_id: InteractionId,
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// 创建时间。
    pub created_at: UnixMillis,
    /// 可选过期时间。
    pub expires_at: Option<UnixMillis>,
    /// 回复目标。
    pub reply_target: ReplyTarget,
    /// 当前处理状态。
    pub status: InteractionStatus,
    /// 已清洗的请求摘要。
    pub request_summary: String,
    /// 可选项。
    pub choices: Vec<InteractionChoice>,
    /// 是否允许多选。
    pub allows_multiple: bool,
}

/// 文本回复请求交互。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextReplyInteraction {
    /// 交互唯一标识。
    pub interaction_id: InteractionId,
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// 创建时间。
    pub created_at: UnixMillis,
    /// 可选过期时间。
    pub expires_at: Option<UnixMillis>,
    /// 回复目标。
    pub reply_target: ReplyTarget,
    /// 当前处理状态。
    pub status: InteractionStatus,
    /// 已清洗的请求摘要。
    pub request_summary: String,
    /// 输入提示。
    pub prompt: String,
}

/// Agent 正在等待用户处理的交互。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentInteraction {
    /// 审批请求。
    Approval(ApprovalInteraction),
    /// 单选或多选问题。
    Choice(ChoiceInteraction),
    /// 开放性文本回复。
    TextReply(TextReplyInteraction),
}

impl AgentInteraction {
    /// 返回交互所属会话键。
    pub fn session_key(&self) -> &SessionKey {
        match self {
            Self::Approval(interaction) => &interaction.session_key,
            Self::Choice(interaction) => &interaction.session_key,
            Self::TextReply(interaction) => &interaction.session_key,
        }
    }

    /// 返回交互回复目标。
    pub fn reply_target(&self) -> &ReplyTarget {
        match self {
            Self::Approval(interaction) => &interaction.reply_target,
            Self::Choice(interaction) => &interaction.reply_target,
            Self::TextReply(interaction) => &interaction.reply_target,
        }
    }

    /// 返回交互处理状态。
    pub fn status(&self) -> InteractionStatus {
        match self {
            Self::Approval(interaction) => interaction.status,
            Self::Choice(interaction) => interaction.status,
            Self::TextReply(interaction) => interaction.status,
        }
    }

    /// 返回会话键对齐后的交互对象。
    pub fn aligned_to_session_key(mut self, session_key: &SessionKey) -> Self {
        match &mut self {
            Self::Approval(interaction) => {
                interaction.session_key = session_key.clone();
            }
            Self::Choice(interaction) => {
                interaction.session_key = session_key.clone();
            }
            Self::TextReply(interaction) => {
                interaction.session_key = session_key.clone();
            }
        }

        self
    }
}

/// 回答请求交互。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerInteraction {
    /// 选择请求。
    Choice(ChoiceInteraction),
    /// 文本回复请求。
    TextReply(TextReplyInteraction),
}

impl From<AnswerInteraction> for AgentInteraction {
    fn from(value: AnswerInteraction) -> Self {
        match value {
            AnswerInteraction::Choice(interaction) => Self::Choice(interaction),
            AnswerInteraction::TextReply(interaction) => Self::TextReply(interaction),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AgentInteraction, ApprovalInteraction, ClipboardFallbackTarget, InteractionId,
        InteractionStatus, ReplyTarget,
    };
    use crate::domain::agent_session::{AgentKind, ConversationId, ProjectId, SessionKey};
    use crate::domain::usage::UnixMillis;

    #[test]
    fn interaction_exposes_session_key_status_and_reply_target() {
        let session_key = SessionKey::new(
            AgentKind::ClaudeCodeCli,
            ProjectId::new("project"),
            ConversationId::new("conversation"),
        );
        let reply_target = ReplyTarget::ClipboardOnly(ClipboardFallbackTarget {
            reason: "readonly".to_string(),
        });
        let interaction = AgentInteraction::Approval(ApprovalInteraction {
            interaction_id: InteractionId::new("approval-1"),
            session_key: session_key.clone(),
            created_at: UnixMillis::new(1),
            expires_at: None,
            reply_target: reply_target.clone(),
            status: InteractionStatus::Pending,
            request_summary: "需要审批".to_string(),
        });

        assert_eq!(interaction.session_key(), &session_key);
        assert_eq!(interaction.reply_target(), &reply_target);
        assert_eq!(interaction.status(), InteractionStatus::Pending);
    }
}
