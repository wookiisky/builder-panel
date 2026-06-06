//! 用户交互应用服务。

use serde::{Deserialize, Serialize};

use crate::adapters::mock_agent::MockAgentRuntime;
use crate::domain::agent_interaction::{AgentInteraction, InteractionId};
use crate::domain::agent_session::{SessionKey, SessionStatus};
use crate::domain::app_error::{AppError, AppErrorCode, FallbackAction};
use crate::ports::agent_adapter_port::{
    AgentInteractionWriterPort, ApprovalDecision, ChoiceSubmission,
};

/// 审批提交请求。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolveApprovalRequest {
    /// 所属会话。
    pub session_key: SessionKey,
    /// 所属交互。
    pub interaction_id: InteractionId,
    /// 审批决策。
    pub decision: ApprovalDecision,
    /// 是否注入一次 mock 回写失败。
    #[serde(default)]
    pub inject_failure: bool,
}

/// 选项提交请求。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SubmitChoiceRequest {
    /// 所属会话。
    pub session_key: SessionKey,
    /// 所属交互。
    pub interaction_id: InteractionId,
    /// 用户选择的选项值。
    pub selected_values: Vec<String>,
    /// 是否注入一次 mock 回写失败。
    #[serde(default)]
    pub inject_failure: bool,
}

/// 用户交互应用服务。
pub struct InteractionService<'a> {
    /// Mock agent runtime。
    runtime: &'a mut MockAgentRuntime,
}

impl<'a> InteractionService<'a> {
    /// 创建用户交互应用服务。
    pub fn new(runtime: &'a mut MockAgentRuntime) -> Self {
        Self { runtime }
    }

    /// 提交审批决策。
    pub fn resolve_approval(&mut self, request: ResolveApprovalRequest) -> Result<(), AppError> {
        let interaction = self
            .pending_approval(&request.session_key, &request.interaction_id)?
            .clone();

        if request.inject_failure {
            self.runtime.fail_next_approval();
        }

        self.runtime.resolve_approval(
            &request.session_key,
            &request.interaction_id,
            interaction.reply_target(),
            request.decision,
        )
    }

    /// 提交单选或多选结果。
    pub fn submit_choice(&mut self, request: SubmitChoiceRequest) -> Result<(), AppError> {
        let interaction = self
            .pending_choice(&request.session_key, &request.interaction_id)?
            .clone();
        let AgentInteraction::Choice(choice) = &interaction else {
            return Err(invalid_interaction("当前交互不是选项"));
        };
        let selected_values = validated_choice_values(choice, &request.selected_values)?;

        if request.inject_failure {
            self.runtime.fail_next_choice();
        }

        self.runtime.submit_choice(
            &request.session_key,
            &request.interaction_id,
            interaction.reply_target(),
            ChoiceSubmission { selected_values },
        )
    }

    /// 获取当前待处理审批。
    fn pending_approval(
        &self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
    ) -> Result<AgentInteraction, AppError> {
        let Some(session) = self.runtime.session_state().sessions.get(session_key) else {
            return Err(invalid_interaction("会话不存在"));
        };

        if session.status != SessionStatus::WaitingForApproval {
            return Err(invalid_interaction("当前会话不等待审批"));
        }

        let Some(interaction) = &session.pending_interaction else {
            return Err(invalid_interaction("当前会话没有待处理交互"));
        };

        match interaction {
            AgentInteraction::Approval(approval) if approval.interaction_id == *interaction_id => {
                Ok(interaction.clone())
            }
            AgentInteraction::Approval(_) => Err(invalid_interaction("审批交互已变化")),
            AgentInteraction::Choice(_) | AgentInteraction::TextReply(_) => {
                Err(invalid_interaction("当前交互不是审批"))
            }
        }
    }

    /// 获取当前待处理选项。
    fn pending_choice(
        &self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
    ) -> Result<AgentInteraction, AppError> {
        let Some(session) = self.runtime.session_state().sessions.get(session_key) else {
            return Err(invalid_interaction("会话不存在"));
        };

        if session.status != SessionStatus::WaitingForAnswer {
            return Err(invalid_interaction("当前会话不等待回复"));
        }

        let Some(interaction) = &session.pending_interaction else {
            return Err(invalid_interaction("当前会话没有待处理交互"));
        };

        match interaction {
            AgentInteraction::Choice(choice) if choice.interaction_id == *interaction_id => {
                Ok(interaction.clone())
            }
            AgentInteraction::Choice(_) => Err(invalid_interaction("选项交互已变化")),
            AgentInteraction::Approval(_) | AgentInteraction::TextReply(_) => {
                Err(invalid_interaction("当前交互不是选项"))
            }
        }
    }
}

/// 校验选项值并保留用户选择顺序。
fn validated_choice_values(
    interaction: &crate::domain::agent_interaction::ChoiceInteraction,
    selected_values: &[String],
) -> Result<Vec<String>, AppError> {
    if selected_values.is_empty() {
        return Err(invalid_interaction("至少选择一项"));
    }

    if !interaction.allows_multiple && selected_values.len() != 1 {
        return Err(invalid_interaction("当前交互只允许单选"));
    }

    let allowed_values = interaction
        .choices
        .iter()
        .map(|choice| choice.value.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut unique_values = std::collections::BTreeSet::new();
    let mut validated = Vec::new();

    for value in selected_values {
        if !allowed_values.contains(value.as_str()) {
            return Err(invalid_interaction("选项值不在当前交互中"));
        }
        if !unique_values.insert(value.as_str()) {
            return Err(invalid_interaction("选项值重复"));
        }
        validated.push(value.clone());
    }

    Ok(validated)
}

/// 创建无效交互错误。
fn invalid_interaction(message: &str) -> AppError {
    AppError::new(
        AppErrorCode::UnsupportedReplyTarget,
        message,
        None,
        false,
        Some(FallbackAction::ViewReadOnly),
    )
}

#[cfg(test)]
mod tests {
    use super::{InteractionService, ResolveApprovalRequest, SubmitChoiceRequest};
    use crate::adapters::mock_agent::{MockAgentDirectiveKind, MockAgentRuntime};
    use crate::domain::agent_interaction::AgentInteraction;
    use crate::domain::agent_session::SessionStatus;
    use crate::ports::agent_adapter_port::ApprovalDecision;

    #[test]
    fn allow_records_directive_and_clears_pending() {
        let mut runtime = MockAgentRuntime::stage3_default();
        let request = approval_request(&runtime, ApprovalDecision::Allow, false);
        let key = request.session_key.clone();
        let mut service = InteractionService::new(&mut runtime);

        service
            .resolve_approval(request)
            .expect("approval should resolve");

        assert_eq!(
            runtime.recorded_directives()[0].kind,
            MockAgentDirectiveKind::ApprovalAllow
        );
        assert!(runtime
            .session_state()
            .sessions
            .get(&key)
            .expect("session should exist")
            .pending_interaction
            .is_none());
    }

    #[test]
    fn deny_records_directive() {
        let mut runtime = MockAgentRuntime::stage3_default();
        let request = approval_request(&runtime, ApprovalDecision::Deny, false);
        let mut service = InteractionService::new(&mut runtime);

        service
            .resolve_approval(request)
            .expect("approval should resolve");

        assert_eq!(
            runtime.recorded_directives()[0].kind,
            MockAgentDirectiveKind::ApprovalDeny
        );
    }

    #[test]
    fn allow_and_remember_records_directive() {
        let mut runtime = MockAgentRuntime::stage3_default();
        let request = approval_request(&runtime, ApprovalDecision::AllowAndRemember, false);
        let mut service = InteractionService::new(&mut runtime);

        service
            .resolve_approval(request)
            .expect("approval should resolve");

        assert_eq!(
            runtime.recorded_directives()[0].kind,
            MockAgentDirectiveKind::ApprovalAllowAndRemember
        );
    }

    #[test]
    fn writeback_failure_keeps_pending() {
        let mut runtime = MockAgentRuntime::stage3_default();
        let request = approval_request(&runtime, ApprovalDecision::Allow, true);
        let key = request.session_key.clone();
        let mut service = InteractionService::new(&mut runtime);

        let result = service.resolve_approval(request);

        assert!(result.is_err());
        assert!(runtime
            .session_state()
            .sessions
            .get(&key)
            .expect("session should exist")
            .pending_interaction
            .is_some());
    }

    #[test]
    fn single_choice_records_directive_and_clears_pending() {
        let mut runtime = MockAgentRuntime::stage5_default();
        let request = choice_request(&runtime, vec!["plan-a"], false);
        let key = request.session_key.clone();
        let mut service = InteractionService::new(&mut runtime);

        service
            .submit_choice(request)
            .expect("choice should submit");

        assert_eq!(
            runtime.recorded_directives()[0].kind,
            MockAgentDirectiveKind::ChoiceReply
        );
        assert_eq!(
            runtime.recorded_directives()[0].content,
            Some("plan-a".to_string())
        );
        assert!(runtime
            .session_state()
            .sessions
            .get(&key)
            .expect("session should exist")
            .pending_interaction
            .is_none());
    }

    #[test]
    fn multi_choice_requires_at_least_one_value() {
        let mut runtime = MockAgentRuntime::stage5_default();
        let request = choice_request(&runtime, Vec::new(), false);
        let mut service = InteractionService::new(&mut runtime);

        let result = service.submit_choice(request);

        assert!(result.is_err());
        assert!(runtime.recorded_directives().is_empty());
    }

    #[test]
    fn choice_rejects_unknown_value() {
        let mut runtime = MockAgentRuntime::stage5_default();
        let request = choice_request(&runtime, vec!["unknown"], false);
        let mut service = InteractionService::new(&mut runtime);

        let result = service.submit_choice(request);

        assert!(result.is_err());
        assert!(runtime.recorded_directives().is_empty());
    }

    #[test]
    fn choice_writeback_failure_keeps_pending() {
        let mut runtime = MockAgentRuntime::stage5_default();
        let request = choice_request(&runtime, vec!["plan-a"], true);
        let key = request.session_key.clone();
        let mut service = InteractionService::new(&mut runtime);

        let result = service.submit_choice(request);

        assert!(result.is_err());
        assert!(runtime
            .session_state()
            .sessions
            .get(&key)
            .expect("session should exist")
            .pending_interaction
            .is_some());
    }

    #[test]
    fn invalid_request_with_failure_injection_does_not_pollute_next_submit() {
        let mut runtime = MockAgentRuntime::stage3_default();
        let mut invalid_request = approval_request(&runtime, ApprovalDecision::Allow, true);
        invalid_request.interaction_id =
            crate::domain::agent_interaction::InteractionId::new("stale-approval");
        let valid_request = approval_request(&runtime, ApprovalDecision::Allow, false);
        let mut service = InteractionService::new(&mut runtime);

        let invalid_result = service.resolve_approval(invalid_request);
        let valid_result = service.resolve_approval(valid_request);

        assert!(invalid_result.is_err());
        assert!(valid_result.is_ok());
        assert_eq!(
            runtime.recorded_directives()[0].kind,
            MockAgentDirectiveKind::ApprovalAllow
        );
    }

    fn approval_request(
        runtime: &MockAgentRuntime,
        decision: ApprovalDecision,
        inject_failure: bool,
    ) -> ResolveApprovalRequest {
        let session = runtime
            .session_state()
            .sessions
            .values()
            .find(|session| session.status == SessionStatus::WaitingForApproval)
            .expect("approval session should exist");
        let Some(AgentInteraction::Approval(interaction)) = &session.pending_interaction else {
            panic!("approval interaction should exist");
        };

        ResolveApprovalRequest {
            session_key: session.session_key.clone(),
            interaction_id: interaction.interaction_id.clone(),
            decision,
            inject_failure,
        }
    }

    fn choice_request(
        runtime: &MockAgentRuntime,
        selected_values: Vec<&str>,
        inject_failure: bool,
    ) -> SubmitChoiceRequest {
        let session = runtime
            .session_state()
            .sessions
            .values()
            .find(|session| {
                matches!(
                    &session.pending_interaction,
                    Some(AgentInteraction::Choice(_))
                )
            })
            .expect("choice session should exist");
        let Some(AgentInteraction::Choice(interaction)) = &session.pending_interaction else {
            panic!("choice interaction should exist");
        };

        SubmitChoiceRequest {
            session_key: session.session_key.clone(),
            interaction_id: interaction.interaction_id.clone(),
            selected_values: selected_values
                .into_iter()
                .map(ToString::to_string)
                .collect(),
            inject_failure,
        }
    }
}
