//! Agent adapter 抽象边界。

use serde::{Deserialize, Serialize};

use crate::domain::agent_event::AgentEvent;
use crate::domain::agent_interaction::{InteractionId, ReplyTarget};
use crate::domain::agent_session::SessionKey;
use crate::domain::app_error::AppError;

/// 审批处理结果。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// 允许本次请求。
    Allow,
    /// 允许并记住同类请求。
    AllowAndRemember,
    /// 拒绝本次请求。
    Deny,
}

/// 用户选择提交内容。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChoiceSubmission {
    /// 已校验的选项值。
    pub selected_values: Vec<String>,
}

/// Agent 事件来源端口。
pub trait AgentEventSourcePort {
    /// 拉取 adapter 已清洗的初始事件。
    fn load_initial_events(&self) -> Vec<AgentEvent>;
}

/// Agent 用户交互回写端口。
pub trait AgentInteractionWriterPort {
    /// 回写审批处理结果。
    fn resolve_approval(
        &mut self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
        reply_target: &ReplyTarget,
        decision: ApprovalDecision,
    ) -> Result<(), AppError>;

    /// 回写单选或多选结果。
    fn submit_choice(
        &mut self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
        reply_target: &ReplyTarget,
        submission: ChoiceSubmission,
    ) -> Result<(), AppError>;
}
