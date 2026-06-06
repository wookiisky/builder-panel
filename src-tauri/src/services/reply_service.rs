//! 回复应用服务。

use serde::{Deserialize, Serialize};

use crate::adapters::mock_agent::MockAgentRuntime;
use crate::domain::agent_interaction::{AgentInteraction, InteractionId};
use crate::domain::agent_session::{SessionKey, SessionStatus};
use crate::domain::app_error::{AppError, AppErrorCode, FallbackAction};
use crate::ports::reply_sender_port::ReplySenderPort;

/// 回复最大字符数。
pub const MAX_REPLY_CHARS: usize = 1000;

/// 文本回复提交请求。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SendReplyRequest {
    /// 所属会话。
    pub session_key: SessionKey,
    /// 所属交互。
    pub interaction_id: InteractionId,
    /// 文本内容。
    pub content: String,
    /// 是否注入一次 mock 回写失败。
    #[serde(default)]
    pub inject_failure: bool,
}

/// 回复应用服务。
pub struct ReplyService<'a> {
    /// Mock agent runtime。
    runtime: &'a mut MockAgentRuntime,
}

impl<'a> ReplyService<'a> {
    /// 创建回复应用服务。
    pub fn new(runtime: &'a mut MockAgentRuntime) -> Self {
        Self { runtime }
    }

    /// 发送开放性文本回复。
    pub fn send_reply(&mut self, request: SendReplyRequest) -> Result<(), AppError> {
        let content = request.content.trim();
        if content.is_empty() {
            return Err(validation_error("回复内容不能为空"));
        }

        if content.chars().count() > MAX_REPLY_CHARS {
            return Err(validation_error("回复内容超过最大长度"));
        }

        let interaction = self
            .pending_text_reply(&request.session_key, &request.interaction_id)?
            .clone();

        if request.inject_failure {
            self.runtime.fail_next_reply();
        }

        self.runtime.send_reply(
            &request.session_key,
            &request.interaction_id,
            interaction.reply_target(),
            content,
        )
    }

    /// 获取当前待处理文本回复。
    fn pending_text_reply(
        &self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
    ) -> Result<AgentInteraction, AppError> {
        let Some(session) = self.runtime.session_state().sessions.get(session_key) else {
            return Err(validation_error("会话不存在"));
        };

        if session.status != SessionStatus::WaitingForAnswer {
            return Err(validation_error("当前会话不等待回复"));
        }

        let Some(interaction) = &session.pending_interaction else {
            return Err(validation_error("当前会话没有待处理交互"));
        };

        match interaction {
            AgentInteraction::TextReply(text_reply)
                if text_reply.interaction_id == *interaction_id =>
            {
                Ok(interaction.clone())
            }
            AgentInteraction::TextReply(_) => Err(validation_error("回复交互已变化")),
            AgentInteraction::Approval(_) | AgentInteraction::Choice(_) => {
                Err(validation_error("当前交互不是文本回复"))
            }
        }
    }
}

/// 创建回复校验错误。
fn validation_error(message: &str) -> AppError {
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
    use super::{ReplyService, SendReplyRequest, MAX_REPLY_CHARS};
    use crate::adapters::mock_agent::{MockAgentDirectiveKind, MockAgentRuntime};
    use crate::domain::agent_interaction::AgentInteraction;
    use crate::domain::agent_session::SessionStatus;

    #[test]
    fn non_empty_reply_records_directive_and_clears_pending() {
        let mut runtime = MockAgentRuntime::stage3_default();
        let request = reply_request(&runtime, "第一行\n第二行", false);
        let key = request.session_key.clone();
        let mut service = ReplyService::new(&mut runtime);

        service.send_reply(request).expect("reply should send");

        assert_eq!(
            runtime.recorded_directives()[0].kind,
            MockAgentDirectiveKind::TextReply
        );
        assert_eq!(
            runtime.recorded_directives()[0].content,
            Some("第一行\n第二行".to_string())
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
    fn empty_reply_is_rejected() {
        let mut runtime = MockAgentRuntime::stage3_default();
        let request = reply_request(&runtime, "  \n ", false);
        let mut service = ReplyService::new(&mut runtime);

        let result = service.send_reply(request);

        assert!(result.is_err());
        assert!(runtime.recorded_directives().is_empty());
    }

    #[test]
    fn oversized_reply_is_rejected() {
        let mut runtime = MockAgentRuntime::stage3_default();
        let content = "a".repeat(MAX_REPLY_CHARS + 1);
        let request = reply_request(&runtime, &content, false);
        let mut service = ReplyService::new(&mut runtime);

        let result = service.send_reply(request);

        assert!(result.is_err());
        assert!(runtime.recorded_directives().is_empty());
    }

    #[test]
    fn writeback_failure_keeps_draft_target_pending() {
        let mut runtime = MockAgentRuntime::stage3_default();
        let request = reply_request(&runtime, "保留草稿", true);
        let key = request.session_key.clone();
        let mut service = ReplyService::new(&mut runtime);

        let result = service.send_reply(request);

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
        let mut invalid_request = reply_request(&runtime, "无效提交", true);
        invalid_request.interaction_id =
            crate::domain::agent_interaction::InteractionId::new("stale-reply");
        let valid_request = reply_request(&runtime, "有效提交", false);
        let mut service = ReplyService::new(&mut runtime);

        let invalid_result = service.send_reply(invalid_request);
        let valid_result = service.send_reply(valid_request);

        assert!(invalid_result.is_err());
        assert!(valid_result.is_ok());
        assert_eq!(
            runtime.recorded_directives()[0].kind,
            MockAgentDirectiveKind::TextReply
        );
    }

    fn reply_request(
        runtime: &MockAgentRuntime,
        content: &str,
        inject_failure: bool,
    ) -> SendReplyRequest {
        let session = runtime
            .session_state()
            .sessions
            .values()
            .find(|session| {
                session.status == SessionStatus::WaitingForAnswer
                    && matches!(
                        &session.pending_interaction,
                        Some(AgentInteraction::TextReply(_))
                    )
            })
            .expect("reply session should exist");
        let Some(AgentInteraction::TextReply(interaction)) = &session.pending_interaction else {
            panic!("text reply interaction should exist");
        };

        SendReplyRequest {
            session_key: session.session_key.clone(),
            interaction_id: interaction.interaction_id.clone(),
            content: content.to_string(),
            inject_failure,
        }
    }
}
