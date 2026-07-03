//! 会话状态 reducer。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::agent_event::AgentEvent;
use crate::domain::agent_interaction::AgentInteraction;
use crate::domain::agent_session::{AgentSession, SessionKey, SessionStatus};
use crate::domain::usage::UnixMillis;

/// 所有会话状态。
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SessionState {
    /// 按唯一键存储的会话。
    pub sessions: BTreeMap<SessionKey, AgentSession>,
    /// 下一个首次捕捉序号。
    #[serde(default)]
    next_capture_sequence: u64,
}

impl SessionState {
    /// 创建空会话状态。
    pub fn empty() -> Self {
        Self {
            sessions: BTreeMap::new(),
            next_capture_sequence: 0,
        }
    }

    /// 应用 agent 事件并返回新的状态。
    pub fn apply_event(&self, event: AgentEvent) -> Self {
        let mut next_state = self.clone();
        next_state.apply_event_in_place(event);
        next_state
    }

    /// 按首次捕捉顺序返回 session key，新捕捉到的 session 在前。
    pub fn sorted_session_keys(&self) -> Vec<SessionKey> {
        let mut sessions = self.sessions.values().collect::<Vec<_>>();

        sessions.sort_by(|left, right| compare_sessions_by_capture_order(left, right));
        sessions
            .into_iter()
            .map(|session| session.session_key.clone())
            .collect()
    }

    /// 就地应用 agent 事件。
    fn apply_event_in_place(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::SessionStarted(event) => {
                let mut session = match self.sessions.remove(&event.session_key) {
                    Some(session) => session,
                    None => self.new_running_session(
                        event.session_key.clone(),
                        event.project_label.clone(),
                        event.conversation_label.clone(),
                        event.updated_at,
                    ),
                };
                let pending_interaction = session.pending_interaction.take();
                let previous_status = session.status;
                session.project_label = event.project_label;
                session.conversation_label = event.conversation_label;
                if event.title.is_some() {
                    session.title = event.title;
                }
                if let Some(summary) = event.summary {
                    session.summary = Some(summary);
                }
                let next_status = preserve_waiting_status(pending_interaction.as_ref());
                if starts_new_turn(previous_status, next_status) {
                    session.started_at = event.updated_at;
                }
                if next_status == SessionStatus::Running {
                    session.completed_at = None;
                }
                session.status = next_status;
                session.capabilities = event.capabilities;
                session.usage = event.usage;
                session.pending_interaction = pending_interaction;
                session.updated_at = event.updated_at;
                self.sessions.insert(session.session_key.clone(), session);
            }
            AgentEvent::ActivityUpdated(event) => {
                let session = self.ensure_session(event.session_key, event.updated_at);
                session.summary = Some(event.summary);
                if session.pending_interaction.is_none() {
                    if resumes_terminal_session(session.status) {
                        session.started_at = event.updated_at;
                    }
                    session.status = SessionStatus::Running;
                    session.completed_at = None;
                }
                session.updated_at = event.updated_at;
            }
            AgentEvent::UserMessageUpdated(event) => {
                let session = self.ensure_session(event.session_key, event.updated_at);
                session.summary = Some(event.summary);
                if session.pending_interaction.is_none() {
                    if resumes_terminal_session(session.status) {
                        session.started_at = event.updated_at;
                    }
                    session.status = SessionStatus::Running;
                    session.completed_at = None;
                }
                session.updated_at = event.updated_at;
            }
            AgentEvent::TitleUpdated(event) => {
                let session = self.ensure_session(event.session_key, event.updated_at);
                session.title = Some(event.title);
                session.updated_at = event.updated_at;
            }
            AgentEvent::ApprovalRequested(event) => {
                let interaction = AgentInteraction::Approval(event.interaction)
                    .aligned_to_session_key(&event.session_key);
                let session = self.ensure_session(event.session_key, event.updated_at);
                session.status = SessionStatus::WaitingForApproval;
                session.pending_interaction = Some(interaction);
                session.updated_at = event.updated_at;
            }
            AgentEvent::AnswerRequested(event) => {
                let interaction = AgentInteraction::from(event.interaction)
                    .aligned_to_session_key(&event.session_key);
                let session = self.ensure_session(event.session_key, event.updated_at);
                session.status = SessionStatus::WaitingForAnswer;
                session.pending_interaction = Some(interaction);
                session.updated_at = event.updated_at;
            }
            AgentEvent::InteractionCompleted(event) => {
                let session = self.ensure_session(event.session_key, event.updated_at);
                session.status = SessionStatus::Running;
                if let Some(summary) = event.summary {
                    session.summary = Some(summary);
                }
                session.pending_interaction = None;
                session.completed_at = None;
                session.updated_at = event.updated_at;
            }
            AgentEvent::TurnCompleted(event) => {
                let session = self.ensure_session(event.session_key, event.updated_at);
                session.status = SessionStatus::Completed;
                if let Some(summary) = event.summary {
                    session.summary = Some(summary);
                }
                session.pending_interaction = None;
                session.completed_at = Some(event.updated_at);
                session.updated_at = event.updated_at;
            }
            AgentEvent::Failed(event) => {
                let session = self.ensure_session(event.session_key, event.updated_at);
                session.status = SessionStatus::Failed;
                session.summary = Some(event.error.user_message.clone());
                session.last_error = Some(event.error);
                session.pending_interaction = None;
                session.completed_at = Some(event.updated_at);
                session.updated_at = event.updated_at;
            }
            AgentEvent::Detached(event) => {
                let session = self.ensure_session(event.session_key, event.updated_at);
                session.status = SessionStatus::Detached;
                if let Some(reason) = event.reason {
                    session.summary = Some(reason);
                }
                session.pending_interaction = None;
                session.updated_at = event.updated_at;
            }
            AgentEvent::CapabilitiesUpdated(event) => {
                let session = self.ensure_session(event.session_key, event.updated_at);
                session.capabilities = event.capabilities;
                session.updated_at = event.updated_at;
            }
            AgentEvent::UsageUpdated(event) => {
                let session = self.ensure_session(event.session_key, event.updated_at);
                session.usage = event.usage;
                session.updated_at = event.updated_at;
            }
            AgentEvent::JumpTargetUpdated(event) => {
                let session = self.ensure_session(event.session_key, event.updated_at);
                session.jump_target = event.jump_target;
                session.updated_at = event.updated_at;
            }
        }
    }

    /// 获取已有 session 或创建占位 session。
    fn ensure_session(
        &mut self,
        session_key: SessionKey,
        updated_at: UnixMillis,
    ) -> &mut AgentSession {
        if !self.sessions.contains_key(&session_key) {
            let session =
                self.new_running_session(session_key.clone(), "未知项目", "未知对话", updated_at);
            self.sessions.insert(session_key.clone(), session);
        }

        self.sessions
            .get_mut(&session_key)
            .expect("session should exist after insertion")
    }

    /// 创建带当前捕捉序号的运行中 session。
    fn new_running_session(
        &mut self,
        session_key: SessionKey,
        project_label: impl Into<String>,
        conversation_label: impl Into<String>,
        updated_at: UnixMillis,
    ) -> AgentSession {
        let mut session =
            AgentSession::new_running(session_key, project_label, conversation_label, updated_at);
        session.capture_sequence = self.next_capture_sequence;
        self.next_capture_sequence = self.next_capture_sequence.saturating_add(1);
        session
    }
}

/// 保留有 pending interaction 的等待状态。
fn preserve_waiting_status(pending_interaction: Option<&AgentInteraction>) -> SessionStatus {
    match pending_interaction {
        Some(AgentInteraction::Approval(_)) => SessionStatus::WaitingForApproval,
        Some(AgentInteraction::Choice(_)) | Some(AgentInteraction::TextReply(_)) => {
            SessionStatus::WaitingForAnswer
        }
        None => SessionStatus::Running,
    }
}

/// 判断事件是否从终态开启了新 turn。
fn starts_new_turn(previous_status: SessionStatus, next_status: SessionStatus) -> bool {
    next_status == SessionStatus::Running && resumes_terminal_session(previous_status)
}

/// 判断状态是否是需要重新计算开始时间的终态。
fn resumes_terminal_session(status: SessionStatus) -> bool {
    matches!(
        status,
        SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Detached
    )
}

/// 比较两个 session 的捕捉展示顺序。
fn compare_sessions_by_capture_order(
    left: &AgentSession,
    right: &AgentSession,
) -> std::cmp::Ordering {
    right
        .capture_sequence
        .cmp(&left.capture_sequence)
        .then_with(|| left.session_key.cmp(&right.session_key))
}

#[cfg(test)]
mod tests {
    use super::SessionState;
    use crate::domain::agent_event::{
        ActivityUpdatedEvent, AgentEvent, AnswerRequestedEvent, ApprovalRequestedEvent,
        CapabilitiesUpdatedEvent, DetachedEvent, FailedEvent, InteractionCompletedEvent,
        SessionStartedEvent, TurnCompletedEvent, UsageUpdatedEvent, UserMessageUpdatedEvent,
    };
    use crate::domain::agent_interaction::{
        AnswerInteraction, ApprovalInteraction, ChoiceInteraction, ClipboardFallbackTarget,
        InteractionId, InteractionStatus, ReplyTarget, TextReplyInteraction,
    };
    use crate::domain::agent_session::{
        AgentKind, ConversationId, ProjectId, SessionCapabilities, SessionKey, SessionStatus,
    };
    use crate::domain::app_error::{AppError, AppErrorCode};
    use crate::domain::usage::{
        UnixMillis, UsageAmount, UsageSnapshot, UsageValue, VerifiedUsageValue,
    };

    #[test]
    fn session_started_creates_session() {
        let key = session_key("project-a", "conversation-a");
        let state = SessionState::empty().apply_event(started_event(key.clone(), 1));
        let session = state.sessions.get(&key).expect("session should exist");

        assert_eq!(session.status, SessionStatus::Running);
        assert_eq!(session.project_label, "project-a");
        assert_eq!(session.started_at, UnixMillis::new(1));
        assert_eq!(session.completed_at, None);
    }

    #[test]
    fn session_started_without_title_preserves_existing_title() {
        let key = session_key("project-a", "conversation-a");
        let state = SessionState::empty()
            .apply_event(AgentEvent::SessionStarted(SessionStartedEvent {
                project_label: "project-a".to_string(),
                conversation_label: "conversation-a".to_string(),
                session_key: key.clone(),
                title: Some("真实 thread 名".to_string()),
                summary: None,
                capabilities: SessionCapabilities::none(),
                usage: UsageSnapshot::unavailable(),
                updated_at: UnixMillis::new(1),
            }))
            .apply_event(started_event(key.clone(), 2));
        let session = state.sessions.get(&key).expect("session should exist");

        assert_eq!(session.title.as_deref(), Some("真实 thread 名"));
    }

    #[test]
    fn session_started_restores_completed_session_to_running() {
        let key = session_key("project-a", "conversation-a");
        let state = SessionState::empty()
            .apply_event(started_event(key.clone(), 1))
            .apply_event(AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key: key.clone(),
                summary: None,
                updated_at: UnixMillis::new(2),
            }))
            .apply_event(started_event(key.clone(), 3));
        let session = state.sessions.get(&key).expect("session should exist");

        assert_eq!(session.status, SessionStatus::Running);
        assert_eq!(session.started_at, UnixMillis::new(3));
        assert_eq!(session.completed_at, None);
    }

    #[test]
    fn session_started_restores_failed_session_to_running() {
        let key = session_key("project-a", "conversation-a");
        let state = SessionState::empty()
            .apply_event(started_event(key.clone(), 1))
            .apply_event(AgentEvent::Failed(FailedEvent {
                session_key: key.clone(),
                error: AppError::new(
                    AppErrorCode::BridgeUnavailable,
                    "bridge 不可用",
                    None,
                    false,
                    None,
                ),
                updated_at: UnixMillis::new(2),
            }))
            .apply_event(started_event(key.clone(), 3));
        let session = state.sessions.get(&key).expect("session should exist");

        assert_eq!(session.status, SessionStatus::Running);
    }

    #[test]
    fn activity_update_does_not_overwrite_pending_approval() {
        let key = session_key("project-a", "conversation-a");
        let state = SessionState::empty()
            .apply_event(started_event(key.clone(), 1))
            .apply_event(approval_event(key.clone(), 2))
            .apply_event(AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
                session_key: key.clone(),
                summary: "仍在运行".to_string(),
                updated_at: UnixMillis::new(3),
            }));
        let session = state.sessions.get(&key).expect("session should exist");

        assert_eq!(session.status, SessionStatus::WaitingForApproval);
        assert!(matches!(
            session.pending_interaction,
            Some(crate::domain::agent_interaction::AgentInteraction::Approval(_))
        ));
    }

    #[test]
    fn session_started_without_summary_preserves_existing_summary() {
        let key = session_key("project-a", "conversation-a");
        let state = SessionState::empty()
            .apply_event(started_event(key.clone(), 1))
            .apply_event(AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
                session_key: key.clone(),
                summary: "原始输出".to_string(),
                updated_at: UnixMillis::new(2),
            }))
            .apply_event(started_event(key.clone(), 3));
        let session = state.sessions.get(&key).expect("session should exist");

        assert_eq!(session.summary.as_deref(), Some("原始输出"));
    }

    #[test]
    fn user_message_update_uses_raw_text_and_running_status() {
        let key = session_key("project-a", "conversation-a");
        let state = SessionState::empty()
            .apply_event(started_event(key.clone(), 1))
            .apply_event(AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key: key.clone(),
                summary: Some("上一轮输出".to_string()),
                updated_at: UnixMillis::new(2),
            }))
            .apply_event(AgentEvent::UserMessageUpdated(UserMessageUpdatedEvent {
                session_key: key.clone(),
                summary: "继续处理".to_string(),
                updated_at: UnixMillis::new(3),
            }));
        let session = state.sessions.get(&key).expect("session should exist");

        assert_eq!(session.summary.as_deref(), Some("继续处理"));
        assert_eq!(session.status, SessionStatus::Running);
    }

    #[test]
    fn activity_update_does_not_overwrite_pending_answer() {
        let key = session_key("project-a", "conversation-a");
        let state = SessionState::empty()
            .apply_event(started_event(key.clone(), 1))
            .apply_event(text_answer_event(key.clone(), 2))
            .apply_event(AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
                session_key: key.clone(),
                summary: "仍在运行".to_string(),
                updated_at: UnixMillis::new(3),
            }));
        let session = state.sessions.get(&key).expect("session should exist");

        assert_eq!(session.status, SessionStatus::WaitingForAnswer);
        assert!(matches!(
            session.pending_interaction,
            Some(crate::domain::agent_interaction::AgentInteraction::TextReply(_))
        ));
    }

    #[test]
    fn approval_and_answer_events_replace_pending_kind() {
        let key = session_key("project-a", "conversation-a");
        let state = SessionState::empty()
            .apply_event(started_event(key.clone(), 1))
            .apply_event(approval_event(key.clone(), 2))
            .apply_event(choice_answer_event(key.clone(), 3));
        let session = state.sessions.get(&key).expect("session should exist");

        assert_eq!(session.status, SessionStatus::WaitingForAnswer);
        assert!(matches!(
            session.pending_interaction,
            Some(crate::domain::agent_interaction::AgentInteraction::Choice(
                _
            ))
        ));
    }

    #[test]
    fn approval_event_aligns_inner_interaction_session_key() {
        let outer_key = session_key("project-a", "conversation-a");
        let inner_key = session_key("project-b", "conversation-b");
        let state = SessionState::empty()
            .apply_event(started_event(outer_key.clone(), 1))
            .apply_event(AgentEvent::ApprovalRequested(ApprovalRequestedEvent {
                session_key: outer_key.clone(),
                interaction: ApprovalInteraction {
                    interaction_id: InteractionId::new("approval"),
                    session_key: inner_key,
                    created_at: UnixMillis::new(2),
                    expires_at: None,
                    reply_target: clipboard_target(),
                    status: InteractionStatus::Pending,
                    request_summary: "需要审批".to_string(),
                },
                updated_at: UnixMillis::new(2),
            }));
        let session = state
            .sessions
            .get(&outer_key)
            .expect("session should exist");
        let pending_session_key = session
            .pending_interaction
            .as_ref()
            .expect("pending should exist")
            .session_key();

        assert_eq!(pending_session_key, &outer_key);
    }

    #[test]
    fn turn_completed_clears_pending() {
        let key = session_key("project-a", "conversation-a");
        let state = SessionState::empty()
            .apply_event(started_event(key.clone(), 1))
            .apply_event(approval_event(key.clone(), 2))
            .apply_event(AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key: key.clone(),
                summary: Some("完成".to_string()),
                updated_at: UnixMillis::new(3),
            }));
        let session = state.sessions.get(&key).expect("session should exist");

        assert_eq!(session.status, SessionStatus::Completed);
        assert_eq!(session.pending_interaction, None);
        assert_eq!(session.summary, Some("完成".to_string()));
        assert_eq!(session.completed_at, Some(UnixMillis::new(3)));
    }

    #[test]
    fn interaction_completed_clears_pending_and_keeps_running() {
        let key = session_key("project-a", "conversation-a");
        let state = SessionState::empty()
            .apply_event(started_event(key.clone(), 1))
            .apply_event(approval_event(key.clone(), 2))
            .apply_event(AgentEvent::InteractionCompleted(
                InteractionCompletedEvent {
                    session_key: key.clone(),
                    summary: Some("审批已允许".to_string()),
                    updated_at: UnixMillis::new(3),
                },
            ));
        let session = state.sessions.get(&key).expect("session should exist");

        assert_eq!(session.status, SessionStatus::Running);
        assert_eq!(session.pending_interaction, None);
        assert_eq!(session.summary, Some("审批已允许".to_string()));
        assert_eq!(session.started_at, UnixMillis::new(1));
        assert_eq!(session.completed_at, None);
    }

    #[test]
    fn failed_clears_pending_and_keeps_error() {
        let key = session_key("project-a", "conversation-a");
        let error = AppError::new(AppErrorCode::ReplySendFailed, "发送失败", None, true, None);
        let state = SessionState::empty()
            .apply_event(started_event(key.clone(), 1))
            .apply_event(approval_event(key.clone(), 2))
            .apply_event(AgentEvent::Failed(FailedEvent {
                session_key: key.clone(),
                error,
                updated_at: UnixMillis::new(3),
            }));
        let session = state.sessions.get(&key).expect("session should exist");

        assert_eq!(session.status, SessionStatus::Failed);
        assert!(session.pending_interaction.is_none());
        assert!(session.last_error.is_some());
        assert_eq!(session.completed_at, Some(UnixMillis::new(3)));
    }

    #[test]
    fn detached_keeps_session_history() {
        let key = session_key("project-a", "conversation-a");
        let state = SessionState::empty()
            .apply_event(started_event(key.clone(), 1))
            .apply_event(approval_event(key.clone(), 2))
            .apply_event(AgentEvent::Detached(DetachedEvent {
                session_key: key.clone(),
                reason: Some("进程不可见".to_string()),
                updated_at: UnixMillis::new(3),
            }));
        let session = state.sessions.get(&key).expect("session should exist");

        assert!(state.sessions.contains_key(&key));
        assert_eq!(session.status, SessionStatus::Detached);
        assert!(session.pending_interaction.is_none());
    }

    #[test]
    fn usage_and_capabilities_do_not_change_status_or_pending() {
        let key = session_key("project-a", "conversation-a");
        let capabilities = SessionCapabilities {
            can_jump: true,
            can_send_reply: true,
            can_resolve_approval: true,
            can_create_followup_turn: true,
        };
        let usage = UsageSnapshot {
            usage_5h: UsageValue::Verified(VerifiedUsageValue {
                value: UsageAmount::new(7.0).expect("valid usage amount"),
                unit: None,
                source_key: "mock".to_string(),
                source_label: "mock".to_string(),
                scope: crate::domain::usage::UsageScope::Session,
                updated_at: None,
            }),
            usage_weekly: UsageValue::Unavailable,
        };
        let state = SessionState::empty()
            .apply_event(started_event(key.clone(), 1))
            .apply_event(approval_event(key.clone(), 2))
            .apply_event(AgentEvent::CapabilitiesUpdated(CapabilitiesUpdatedEvent {
                session_key: key.clone(),
                capabilities,
                updated_at: UnixMillis::new(3),
            }))
            .apply_event(AgentEvent::UsageUpdated(UsageUpdatedEvent {
                session_key: key.clone(),
                usage,
                updated_at: UnixMillis::new(4),
            }));
        let session = state.sessions.get(&key).expect("session should exist");

        assert_eq!(session.status, SessionStatus::WaitingForApproval);
        assert!(session.pending_interaction.is_some());
        assert!(session.capabilities.can_jump);
        assert_eq!(session.usage.usage_5h.display_label(), "7");
    }

    #[test]
    fn multiple_projects_and_conversations_do_not_merge() {
        let first = session_key("project-a", "conversation-a");
        let second = session_key("project-b", "conversation-a");
        let third = session_key("project-a", "conversation-b");
        let state = SessionState::empty()
            .apply_event(started_event(first, 1))
            .apply_event(started_event(second, 2))
            .apply_event(started_event(third, 3));

        assert_eq!(state.sessions.len(), 3);
    }

    #[test]
    fn sorted_sessions_keep_capture_order_and_put_new_captures_first() {
        let first = session_key("project-a", "first");
        let second = session_key("project-a", "second");
        let third = session_key("project-a", "third");
        let state = SessionState::empty()
            .apply_event(started_event(first.clone(), 100))
            .apply_event(started_event(second.clone(), 1))
            .apply_event(approval_event(first.clone(), 999))
            .apply_event(started_event(third.clone(), 50));

        assert_eq!(state.sorted_session_keys(), vec![third, second, first],);
    }

    fn session_key(project: &str, conversation: &str) -> SessionKey {
        SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new(project),
            ConversationId::new(conversation),
        )
    }

    fn started_event(session_key: SessionKey, updated_at: u64) -> AgentEvent {
        AgentEvent::SessionStarted(SessionStartedEvent {
            project_label: session_key.project_id.value.clone(),
            conversation_label: session_key.conversation_id.value.clone(),
            session_key,
            title: None,
            summary: None,
            capabilities: SessionCapabilities::none(),
            usage: UsageSnapshot::unavailable(),
            updated_at: UnixMillis::new(updated_at),
        })
    }

    fn approval_event(session_key: SessionKey, updated_at: u64) -> AgentEvent {
        AgentEvent::ApprovalRequested(ApprovalRequestedEvent {
            session_key: session_key.clone(),
            interaction: ApprovalInteraction {
                interaction_id: InteractionId::new("approval"),
                session_key,
                created_at: UnixMillis::new(updated_at),
                expires_at: None,
                reply_target: clipboard_target(),
                status: InteractionStatus::Pending,
                request_summary: "需要审批".to_string(),
            },
            updated_at: UnixMillis::new(updated_at),
        })
    }

    fn choice_answer_event(session_key: SessionKey, updated_at: u64) -> AgentEvent {
        AgentEvent::AnswerRequested(AnswerRequestedEvent {
            session_key: session_key.clone(),
            interaction: AnswerInteraction::Choice(ChoiceInteraction {
                interaction_id: InteractionId::new("choice"),
                session_key,
                created_at: UnixMillis::new(updated_at),
                expires_at: None,
                reply_target: clipboard_target(),
                status: InteractionStatus::Pending,
                request_summary: "选择一个选项".to_string(),
                choices: Vec::new(),
                allows_multiple: false,
            }),
            updated_at: UnixMillis::new(updated_at),
        })
    }

    fn text_answer_event(session_key: SessionKey, updated_at: u64) -> AgentEvent {
        AgentEvent::AnswerRequested(AnswerRequestedEvent {
            session_key: session_key.clone(),
            interaction: AnswerInteraction::TextReply(TextReplyInteraction {
                interaction_id: InteractionId::new("reply"),
                session_key,
                created_at: UnixMillis::new(updated_at),
                expires_at: None,
                reply_target: clipboard_target(),
                status: InteractionStatus::Pending,
                request_summary: "需要回复".to_string(),
                prompt: "请输入".to_string(),
            }),
            updated_at: UnixMillis::new(updated_at),
        })
    }

    fn clipboard_target() -> ReplyTarget {
        ReplyTarget::ClipboardOnly(ClipboardFallbackTarget {
            reason: "测试降级".to_string(),
        })
    }
}
