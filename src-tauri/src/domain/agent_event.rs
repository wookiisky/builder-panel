//! 归一后的 Agent 事件模型。

use serde::{Deserialize, Serialize};

use crate::domain::agent_interaction::{AnswerInteraction, ApprovalInteraction};
use crate::domain::agent_session::{JumpTarget, SessionCapabilities, SessionKey};
use crate::domain::app_error::AppError;
use crate::domain::usage::{UnixMillis, UsageSnapshot};

/// 新会话或恢复会话事件。
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SessionStartedEvent {
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
    /// 当前会话能力。
    pub capabilities: SessionCapabilities,
    /// 当前用量快照。
    pub usage: UsageSnapshot,
    /// 事件更新时间。
    pub updated_at: UnixMillis,
}

/// 活动摘要或运行状态更新事件。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActivityUpdatedEvent {
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// 已清洗活动摘要。
    pub summary: String,
    /// 事件更新时间。
    pub updated_at: UnixMillis,
}

/// 审批请求事件。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApprovalRequestedEvent {
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// 审批交互。
    pub interaction: ApprovalInteraction,
    /// 事件更新时间。
    pub updated_at: UnixMillis,
}

/// 回答请求事件。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnswerRequestedEvent {
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// 回答交互。
    pub interaction: AnswerInteraction,
    /// 事件更新时间。
    pub updated_at: UnixMillis,
}

/// 用户交互已回写事件。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InteractionCompletedEvent {
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// 可选回写摘要。
    pub summary: Option<String>,
    /// 事件更新时间。
    pub updated_at: UnixMillis,
}

/// 当前 turn 完成事件。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TurnCompletedEvent {
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// 可选完成摘要。
    pub summary: Option<String>,
    /// 事件更新时间。
    pub updated_at: UnixMillis,
}

/// 失败事件。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FailedEvent {
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// 归一后的应用错误。
    pub error: AppError,
    /// 事件更新时间。
    pub updated_at: UnixMillis,
}

/// 会话失联事件。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DetachedEvent {
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// 已清洗失联原因。
    pub reason: Option<String>,
    /// 事件更新时间。
    pub updated_at: UnixMillis,
}

/// 会话能力更新事件。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CapabilitiesUpdatedEvent {
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// 当前会话能力。
    pub capabilities: SessionCapabilities,
    /// 事件更新时间。
    pub updated_at: UnixMillis,
}

/// 用量更新事件。
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct UsageUpdatedEvent {
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// 当前用量快照。
    pub usage: UsageSnapshot,
    /// 事件更新时间。
    pub updated_at: UnixMillis,
}

/// 跳回目标更新事件。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct JumpTargetUpdatedEvent {
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// 可选跳回目标。
    pub jump_target: Option<JumpTarget>,
    /// 事件更新时间。
    pub updated_at: UnixMillis,
}

/// 归一后的 agent 事件。
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentEvent {
    /// 新会话或恢复会话。
    SessionStarted(SessionStartedEvent),
    /// 活动摘要或运行状态更新。
    ActivityUpdated(ActivityUpdatedEvent),
    /// 审批请求。
    ApprovalRequested(ApprovalRequestedEvent),
    /// 问题或选项请求。
    AnswerRequested(AnswerRequestedEvent),
    /// 用户交互已回写，turn 可能仍在继续。
    InteractionCompleted(InteractionCompletedEvent),
    /// 当前 turn 完成。
    TurnCompleted(TurnCompletedEvent),
    /// 失败。
    Failed(FailedEvent),
    /// 会话失联。
    Detached(DetachedEvent),
    /// 会话能力更新。
    CapabilitiesUpdated(CapabilitiesUpdatedEvent),
    /// 用量更新。
    UsageUpdated(UsageUpdatedEvent),
    /// 跳回目标更新。
    JumpTargetUpdated(JumpTargetUpdatedEvent),
}

impl AgentEvent {
    /// 返回事件所属会话键。
    pub fn session_key(&self) -> &SessionKey {
        match self {
            Self::SessionStarted(event) => &event.session_key,
            Self::ActivityUpdated(event) => &event.session_key,
            Self::ApprovalRequested(event) => &event.session_key,
            Self::AnswerRequested(event) => &event.session_key,
            Self::InteractionCompleted(event) => &event.session_key,
            Self::TurnCompleted(event) => &event.session_key,
            Self::Failed(event) => &event.session_key,
            Self::Detached(event) => &event.session_key,
            Self::CapabilitiesUpdated(event) => &event.session_key,
            Self::UsageUpdated(event) => &event.session_key,
            Self::JumpTargetUpdated(event) => &event.session_key,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentEvent, SessionStartedEvent};
    use crate::domain::agent_session::{
        AgentKind, ConversationId, ProjectId, SessionCapabilities, SessionKey,
    };
    use crate::domain::usage::{UnixMillis, UsageSnapshot};

    #[test]
    fn agent_event_serializes_without_third_party_payload() {
        let session_key = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("project"),
            ConversationId::new("conversation"),
        );
        let event = AgentEvent::SessionStarted(SessionStartedEvent {
            session_key: session_key.clone(),
            project_label: "builder-panel".to_string(),
            conversation_label: "conversation".to_string(),
            title: Some("实现阶段 1".to_string()),
            summary: Some("开始".to_string()),
            capabilities: SessionCapabilities::none(),
            usage: UsageSnapshot::unavailable(),
            updated_at: UnixMillis::new(10),
        });

        let text = serde_json::to_string(&event).expect("event should serialize");
        let decoded: AgentEvent = serde_json::from_str(&text).expect("event should deserialize");

        assert_eq!(decoded.session_key(), &session_key);
        assert!(!text.contains("raw_payload"));
        assert!(!text.contains("swift"));
    }
}
