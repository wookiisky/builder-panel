//! 阶段 3 mock agent adapter。

use crate::domain::agent_event::{
    ActivityUpdatedEvent, AgentEvent, AnswerRequestedEvent, ApprovalRequestedEvent, FailedEvent,
    SessionStartedEvent, TurnCompletedEvent, UsageUpdatedEvent,
};
use crate::domain::agent_interaction::{
    AnswerInteraction, ApprovalInteraction, ChoiceInteraction, ClipboardFallbackTarget,
    InteractionChoice, InteractionId, InteractionStatus, ReplyTarget, TextReplyInteraction,
};
use crate::domain::agent_session::{
    AgentKind, ConversationId, ProjectId, SessionCapabilities, SessionKey,
};
use crate::domain::app_error::{AppError, AppErrorCode, FallbackAction};
use crate::domain::session_state::SessionState;
use crate::domain::usage::{
    UnixMillis, UsageAmount, UsageSnapshot, UsageValue, VerifiedUsageValue,
};
use crate::domain::view_model::{
    session_detail_view_model, session_list_view_models, SessionDetailViewModel,
    SessionListItemViewModel,
};
use crate::ports::agent_adapter_port::{
    AgentEventSourcePort, AgentInteractionWriterPort, ApprovalDecision, ChoiceSubmission,
};
use crate::ports::process_timeline_port::{
    ProcessTimelineEventKind, ProcessTimelineItem, ProcessTimelineReaderPort,
    ProcessTimelineReleasePort,
};
use crate::ports::reply_sender_port::ReplySenderPort;

/// Mock agent 记录的 directive 类型。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MockAgentDirectiveKind {
    /// 审批允许。
    ApprovalAllow,
    /// 审批允许并记住。
    ApprovalAllowAndRemember,
    /// 审批拒绝。
    ApprovalDeny,
    /// 文本回复。
    TextReply,
    /// 选项回复。
    ChoiceReply,
}

/// Mock agent 收到的回写记录。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MockAgentRecordedDirective {
    /// 所属会话。
    pub session_key: SessionKey,
    /// 所属交互。
    pub interaction_id: InteractionId,
    /// 回写目标。
    pub reply_target: ReplyTarget,
    /// directive 类型。
    pub kind: MockAgentDirectiveKind,
    /// 可选文本内容。
    pub content: Option<String>,
    /// 记录时间。
    pub recorded_at: UnixMillis,
}

/// 阶段 3 mock agent runtime。
pub struct MockAgentRuntime {
    /// 当前折叠后的 session 状态。
    session_state: SessionState,
    /// Mock agent 已收到的 directive。
    recorded_directives: Vec<MockAgentRecordedDirective>,
    /// Mock 时间线数据源。
    timeline_items: Vec<ProcessTimelineItem>,
    /// 下一次审批回写是否失败。
    fail_next_approval: bool,
    /// 下一次文本回复回写是否失败。
    fail_next_reply: bool,
    /// 下一次选项回写是否失败。
    fail_next_choice: bool,
    /// 单调递增的 mock 时间。
    clock: u64,
}

impl MockAgentRuntime {
    /// 创建阶段 3 默认 mock runtime。
    pub fn stage3_default() -> Self {
        let adapter = MockAgentScenarioAdapter::stage3_default();
        let session_state = adapter
            .load_initial_events()
            .into_iter()
            .fold(SessionState::empty(), |state, event| {
                state.apply_event(event)
            });

        Self {
            session_state,
            recorded_directives: Vec::new(),
            timeline_items: adapter.timeline_items,
            fail_next_approval: false,
            fail_next_reply: false,
            fail_next_choice: false,
            clock: 10_000,
        }
    }

    /// 创建阶段 5 默认 mock runtime。
    pub fn stage5_default() -> Self {
        Self::stage3_default()
    }

    /// 返回 session 列表 view model。
    pub fn session_list(&self) -> Vec<SessionListItemViewModel> {
        session_list_view_models(&self.session_state)
    }

    /// 返回 session 详情 view model。
    pub fn session_detail(&self, session_key: &SessionKey) -> Option<SessionDetailViewModel> {
        self.session_state
            .sessions
            .get(session_key)
            .map(session_detail_view_model)
    }

    /// 返回当前 session 状态。
    pub fn session_state(&self) -> &SessionState {
        &self.session_state
    }

    /// 返回 mock agent 已收到的 directive。
    pub fn recorded_directives(&self) -> &[MockAgentRecordedDirective] {
        &self.recorded_directives
    }

    /// 设置下一次审批回写失败。
    pub fn fail_next_approval(&mut self) {
        self.fail_next_approval = true;
    }

    /// 设置下一次回复回写失败。
    pub fn fail_next_reply(&mut self) {
        self.fail_next_reply = true;
    }

    /// 设置下一次选项回写失败。
    pub fn fail_next_choice(&mut self) {
        self.fail_next_choice = true;
    }

    /// 应用已清洗事件。
    pub fn apply_event(&mut self, event: AgentEvent) {
        self.session_state = self.session_state.apply_event(event);
    }

    /// 返回下一个 mock 时间戳。
    fn next_time(&mut self) -> UnixMillis {
        self.clock += 1;
        UnixMillis::new(self.clock)
    }
}

impl AgentInteractionWriterPort for MockAgentRuntime {
    fn resolve_approval(
        &mut self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
        reply_target: &ReplyTarget,
        decision: ApprovalDecision,
    ) -> Result<(), AppError> {
        if self.fail_next_approval {
            self.fail_next_approval = false;
            return Err(reply_send_failed("mock 审批回写失败"));
        }

        let recorded_at = self.next_time();
        let kind = match decision {
            ApprovalDecision::Allow => MockAgentDirectiveKind::ApprovalAllow,
            ApprovalDecision::AllowAndRemember => MockAgentDirectiveKind::ApprovalAllowAndRemember,
            ApprovalDecision::Deny => MockAgentDirectiveKind::ApprovalDeny,
        };
        self.recorded_directives.push(MockAgentRecordedDirective {
            session_key: session_key.clone(),
            interaction_id: interaction_id.clone(),
            reply_target: reply_target.clone(),
            kind,
            content: None,
            recorded_at,
        });
        self.apply_event(AgentEvent::TurnCompleted(TurnCompletedEvent {
            session_key: session_key.clone(),
            summary: Some(match decision {
                ApprovalDecision::Allow => "审批已允许".to_string(),
                ApprovalDecision::AllowAndRemember => "审批已允许并记住".to_string(),
                ApprovalDecision::Deny => "审批已拒绝".to_string(),
            }),
            updated_at: recorded_at,
        }));

        Ok(())
    }

    fn submit_choice(
        &mut self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
        reply_target: &ReplyTarget,
        submission: ChoiceSubmission,
    ) -> Result<(), AppError> {
        if self.fail_next_choice {
            self.fail_next_choice = false;
            return Err(reply_send_failed("mock 选项回写失败"));
        }

        let recorded_at = self.next_time();
        self.recorded_directives.push(MockAgentRecordedDirective {
            session_key: session_key.clone(),
            interaction_id: interaction_id.clone(),
            reply_target: reply_target.clone(),
            kind: MockAgentDirectiveKind::ChoiceReply,
            content: Some(submission.selected_values.join(",")),
            recorded_at,
        });
        self.apply_event(AgentEvent::TurnCompleted(TurnCompletedEvent {
            session_key: session_key.clone(),
            summary: Some("选项已提交".to_string()),
            updated_at: recorded_at,
        }));

        Ok(())
    }
}

impl ReplySenderPort for MockAgentRuntime {
    fn send_reply(
        &mut self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
        reply_target: &ReplyTarget,
        content: &str,
    ) -> Result<(), AppError> {
        if self.fail_next_reply {
            self.fail_next_reply = false;
            return Err(reply_send_failed("mock 文本回复回写失败"));
        }

        let recorded_at = self.next_time();
        self.recorded_directives.push(MockAgentRecordedDirective {
            session_key: session_key.clone(),
            interaction_id: interaction_id.clone(),
            reply_target: reply_target.clone(),
            kind: MockAgentDirectiveKind::TextReply,
            content: Some(content.to_string()),
            recorded_at,
        });
        self.apply_event(AgentEvent::TurnCompleted(TurnCompletedEvent {
            session_key: session_key.clone(),
            summary: Some("回复已发送".to_string()),
            updated_at: recorded_at,
        }));

        Ok(())
    }
}

impl ProcessTimelineReaderPort for MockAgentRuntime {
    fn read_timeline(
        &self,
        session_key: &SessionKey,
    ) -> Result<Vec<ProcessTimelineItem>, AppError> {
        Ok(self
            .timeline_items
            .iter()
            .filter(|item| &item.session_key == session_key)
            .cloned()
            .collect())
    }
}

impl ProcessTimelineReleasePort for MockAgentRuntime {
    fn release_large_texts(&mut self, session_key: &SessionKey) -> Result<usize, AppError> {
        let mut released_count = 0;
        for item in &mut self.timeline_items {
            if &item.session_key != session_key || item.body.chars().count() <= 512 {
                continue;
            }

            item.body = "长正文缓存已释放，重新打开后仅保留标题和类型。".to_string();
            released_count += 1;
        }

        Ok(released_count)
    }
}

/// 阶段 3 mock agent 场景 adapter。
pub struct MockAgentScenarioAdapter {
    /// 初始事件。
    initial_events: Vec<AgentEvent>,
    /// 时间线条目。
    timeline_items: Vec<ProcessTimelineItem>,
}

impl MockAgentScenarioAdapter {
    /// 创建阶段 3 默认 mock 场景。
    pub fn stage3_default() -> Self {
        let approval_key = session_key("mock-project-alpha", "approval-turn");
        let reply_key = session_key("mock-project-beta", "reply-turn");
        let choice_key = session_key("mock-project-beta", "choice-turn");
        let completed_key = session_key("mock-project-alpha", "completed-turn");
        let failed_key = session_key("mock-project-gamma", "failed-turn");
        let events = vec![
            AgentEvent::SessionStarted(SessionStartedEvent {
                session_key: approval_key.clone(),
                project_label: "Mock Alpha".to_string(),
                conversation_label: "审批闭环".to_string(),
                title: Some("检查文件写入权限".to_string()),
                summary: Some("准备修改配置文件，需要用户审批".to_string()),
                capabilities: capabilities(true, false, true, false, true),
                usage: verified_usage(41.0, 64.0),
                updated_at: UnixMillis::new(1000),
            }),
            AgentEvent::ApprovalRequested(ApprovalRequestedEvent {
                session_key: approval_key.clone(),
                interaction: ApprovalInteraction {
                    interaction_id: InteractionId::new("approval-1"),
                    session_key: approval_key.clone(),
                    created_at: UnixMillis::new(1001),
                    expires_at: None,
                    reply_target: ReplyTarget::ClipboardOnly(ClipboardFallbackTarget {
                        reason: "mock agent 使用内存 directive 记录".to_string(),
                    }),
                    status: InteractionStatus::Pending,
                    request_summary: "允许 mock agent 写入本地配置样例".to_string(),
                },
                updated_at: UnixMillis::new(1001),
            }),
            AgentEvent::SessionStarted(SessionStartedEvent {
                session_key: reply_key.clone(),
                project_label: "Mock Beta".to_string(),
                conversation_label: "回复闭环".to_string(),
                title: Some("补充需求细节".to_string()),
                summary: Some("等待用户补充执行边界".to_string()),
                capabilities: capabilities(true, true, false, false, true),
                usage: UsageSnapshot::unavailable(),
                updated_at: UnixMillis::new(1002),
            }),
            AgentEvent::AnswerRequested(AnswerRequestedEvent {
                session_key: reply_key.clone(),
                interaction: AnswerInteraction::TextReply(TextReplyInteraction {
                    interaction_id: InteractionId::new("reply-1"),
                    session_key: reply_key.clone(),
                    created_at: UnixMillis::new(1003),
                    expires_at: None,
                    reply_target: ReplyTarget::ClipboardOnly(ClipboardFallbackTarget {
                        reason: "mock agent 使用内存 directive 记录".to_string(),
                    }),
                    status: InteractionStatus::Pending,
                    request_summary: "请输入阶段 3 的验收补充说明".to_string(),
                    prompt: "输入回复内容".to_string(),
                }),
                updated_at: UnixMillis::new(1003),
            }),
            AgentEvent::SessionStarted(SessionStartedEvent {
                session_key: choice_key.clone(),
                project_label: "Mock Beta".to_string(),
                conversation_label: "选项闭环".to_string(),
                title: Some("选择执行方案".to_string()),
                summary: Some("等待用户选择一个执行方案".to_string()),
                capabilities: capabilities(true, true, false, false, true),
                usage: UsageSnapshot::unavailable(),
                updated_at: UnixMillis::new(1004),
            }),
            AgentEvent::AnswerRequested(AnswerRequestedEvent {
                session_key: choice_key.clone(),
                interaction: AnswerInteraction::Choice(ChoiceInteraction {
                    interaction_id: InteractionId::new("choice-1"),
                    session_key: choice_key.clone(),
                    created_at: UnixMillis::new(1005),
                    expires_at: None,
                    reply_target: ReplyTarget::ClipboardOnly(ClipboardFallbackTarget {
                        reason: "mock agent 使用内存 directive 记录".to_string(),
                    }),
                    status: InteractionStatus::Pending,
                    request_summary: "请选择阶段 5 的执行方案".to_string(),
                    choices: vec![
                        InteractionChoice {
                            value: "plan-a".to_string(),
                            label: "先完成 mock 闭环".to_string(),
                        },
                        InteractionChoice {
                            value: "plan-b".to_string(),
                            label: "先补真实终端 adapter".to_string(),
                        },
                    ],
                    allows_multiple: false,
                }),
                updated_at: UnixMillis::new(1005),
            }),
            AgentEvent::SessionStarted(SessionStartedEvent {
                session_key: completed_key.clone(),
                project_label: "Mock Alpha".to_string(),
                conversation_label: "完成闭环".to_string(),
                title: Some("完成 mock 用量同步".to_string()),
                summary: Some("mock 用量已同步".to_string()),
                capabilities: capabilities(true, false, false, true, true),
                usage: verified_usage(18.0, 27.0),
                updated_at: UnixMillis::new(1006),
            }),
            AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
                session_key: completed_key.clone(),
                summary: "生成完成事件前的运行摘要".to_string(),
                updated_at: UnixMillis::new(1007),
            }),
            AgentEvent::UsageUpdated(UsageUpdatedEvent {
                session_key: completed_key.clone(),
                usage: verified_usage(22.0, 31.0),
                updated_at: UnixMillis::new(1008),
            }),
            AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key: completed_key.clone(),
                summary: Some("mock turn 已完成".to_string()),
                updated_at: UnixMillis::new(1009),
            }),
            AgentEvent::SessionStarted(SessionStartedEvent {
                session_key: failed_key.clone(),
                project_label: "Mock Gamma".to_string(),
                conversation_label: "失败闭环".to_string(),
                title: Some("模拟 agent 失败".to_string()),
                summary: Some("即将写入失败状态".to_string()),
                capabilities: capabilities(false, false, false, false, false),
                usage: UsageSnapshot::unavailable(),
                updated_at: UnixMillis::new(1010),
            }),
            AgentEvent::Failed(FailedEvent {
                session_key: failed_key.clone(),
                error: AppError::new(
                    AppErrorCode::AgentProtocolUnsupported,
                    "mock agent 模拟失败",
                    Some("stage3 fixture".to_string()),
                    false,
                    Some(FallbackAction::ViewReadOnly),
                ),
                updated_at: UnixMillis::new(1011),
            }),
        ];
        let timeline_items = vec![
            timeline_item(
                &approval_key,
                "a-1",
                ProcessTimelineEventKind::Activity,
                "读取任务",
                "mock agent 读取阶段 3 任务",
                2001,
            ),
            timeline_item(
                &approval_key,
                "a-2",
                ProcessTimelineEventKind::Tool,
                "准备写入",
                "检测到需要写入配置样例",
                2002,
            ),
            timeline_item(
                &approval_key,
                "a-3",
                ProcessTimelineEventKind::Approval,
                "等待审批",
                "等待用户允许或拒绝",
                2003,
            ),
            timeline_item(
                &reply_key,
                "r-1",
                ProcessTimelineEventKind::Activity,
                "分析输入",
                "mock agent 等待补充说明",
                2004,
            ),
            timeline_item(
                &reply_key,
                "r-2",
                ProcessTimelineEventKind::Reply,
                "请求回复",
                "用户需要输入单行或多行回复",
                2005,
            ),
            timeline_item(
                &choice_key,
                "ch-1",
                ProcessTimelineEventKind::Reply,
                "请求选择",
                "mock agent 等待用户选择执行方案",
                2006,
            ),
            timeline_item(
                &completed_key,
                "c-1",
                ProcessTimelineEventKind::System,
                "同步用量",
                "用量数据已经清洗为可展示数字",
                2007,
            ),
            timeline_item(
                &completed_key,
                "c-2",
                ProcessTimelineEventKind::Activity,
                "完成 turn",
                "mock turn 已完成",
                2008,
            ),
        ];

        Self {
            initial_events: events,
            timeline_items,
        }
    }
}

impl AgentEventSourcePort for MockAgentScenarioAdapter {
    fn load_initial_events(&self) -> Vec<AgentEvent> {
        self.initial_events.clone()
    }
}

/// 创建 mock session key。
fn session_key(project_id: &str, conversation_id: &str) -> SessionKey {
    SessionKey::new(
        AgentKind::CodexCli,
        ProjectId::new(project_id),
        ConversationId::new(conversation_id),
    )
}

/// 创建 mock capability。
fn capabilities(
    can_jump: bool,
    can_send_reply: bool,
    can_resolve_approval: bool,
    can_create_followup_turn: bool,
    can_view_process_timeline: bool,
) -> SessionCapabilities {
    SessionCapabilities {
        can_jump,
        can_send_reply,
        can_resolve_approval,
        can_create_followup_turn,
        can_view_process_timeline,
    }
}

/// 创建已验证 mock 用量。
fn verified_usage(usage_5h: f64, usage_weekly: f64) -> UsageSnapshot {
    UsageSnapshot {
        usage_5h: UsageValue::Verified(VerifiedUsageValue {
            value: UsageAmount::new(usage_5h).expect("mock usage must be valid"),
            unit: Some("percent".to_string()),
            source_label: "Mock /status".to_string(),
            updated_at: Some(UnixMillis::new(1000)),
        }),
        usage_weekly: UsageValue::Verified(VerifiedUsageValue {
            value: UsageAmount::new(usage_weekly).expect("mock usage must be valid"),
            unit: Some("percent".to_string()),
            source_label: "Mock /status".to_string(),
            updated_at: Some(UnixMillis::new(1000)),
        }),
    }
}

/// 创建 mock timeline 条目。
fn timeline_item(
    session_key: &SessionKey,
    item_id: &str,
    kind: ProcessTimelineEventKind,
    title: &str,
    body: &str,
    created_at: u64,
) -> ProcessTimelineItem {
    ProcessTimelineItem {
        item_id: item_id.to_string(),
        session_key: session_key.clone(),
        kind,
        title: title.to_string(),
        body: body.to_string(),
        created_at: UnixMillis::new(created_at),
    }
}

/// 创建 mock 回写失败错误。
fn reply_send_failed(detail: &str) -> AppError {
    AppError::new(
        AppErrorCode::ReplySendFailed,
        "mock agent 回写失败",
        Some(detail.to_string()),
        true,
        Some(FallbackAction::RetryLater),
    )
}

#[cfg(test)]
mod tests {
    use super::{MockAgentDirectiveKind, MockAgentRuntime};
    use crate::domain::agent_session::SessionStatus;
    use crate::ports::agent_adapter_port::{AgentInteractionWriterPort, ApprovalDecision};
    use crate::ports::reply_sender_port::ReplySenderPort;

    #[test]
    fn mock_events_update_session_state_and_keep_sessions_separate() {
        let runtime = MockAgentRuntime::stage3_default();

        assert_eq!(runtime.session_state().sessions.len(), 5);
        assert!(runtime
            .session_list()
            .iter()
            .any(|item| item.project_label == "Mock Alpha"
                && item.conversation_label == "审批闭环"
                && item.status_kind == SessionStatus::WaitingForApproval));
        assert!(runtime
            .session_list()
            .iter()
            .any(|item| item.project_label == "Mock Alpha"
                && item.conversation_label == "完成闭环"
                && item.status_kind == SessionStatus::Completed));
    }

    #[test]
    fn mock_usage_available_and_unavailable_are_explicit() {
        let runtime = MockAgentRuntime::stage3_default();
        let items = runtime.session_list();

        assert!(items
            .iter()
            .any(|item| item.usage_5h.value_label == "41 percent"));
        assert!(items.iter().any(|item| item.usage_5h.value_label == "--"));
    }

    #[test]
    fn mock_records_approval_allow_and_clears_pending() {
        let mut runtime = MockAgentRuntime::stage3_default();
        let session = runtime
            .session_state()
            .sessions
            .values()
            .find(|session| session.status == SessionStatus::WaitingForApproval)
            .expect("approval session should exist")
            .clone();
        let interaction = session
            .pending_interaction
            .as_ref()
            .expect("pending approval should exist");

        runtime
            .resolve_approval(
                &session.session_key,
                interaction_id(interaction),
                interaction.reply_target(),
                ApprovalDecision::Allow,
            )
            .expect("approval should resolve");

        assert_eq!(
            runtime.recorded_directives()[0].kind,
            MockAgentDirectiveKind::ApprovalAllow
        );
        assert_eq!(
            runtime
                .session_state()
                .sessions
                .get(&session.session_key)
                .expect("session should exist")
                .pending_interaction,
            None
        );
    }

    #[test]
    fn mock_reply_failure_does_not_clear_pending() {
        let mut runtime = MockAgentRuntime::stage3_default();
        let session = runtime
            .session_state()
            .sessions
            .values()
            .find(|session| session.status == SessionStatus::WaitingForAnswer)
            .expect("reply session should exist")
            .clone();
        let interaction = session
            .pending_interaction
            .as_ref()
            .expect("pending reply should exist");
        runtime.fail_next_reply();

        let result = runtime.send_reply(
            &session.session_key,
            interaction_id(interaction),
            interaction.reply_target(),
            "补充说明",
        );

        assert!(result.is_err());
        assert!(runtime
            .session_state()
            .sessions
            .get(&session.session_key)
            .expect("session should exist")
            .pending_interaction
            .is_some());
    }

    fn interaction_id(
        interaction: &crate::domain::agent_interaction::AgentInteraction,
    ) -> &crate::domain::agent_interaction::InteractionId {
        match interaction {
            crate::domain::agent_interaction::AgentInteraction::Approval(interaction) => {
                &interaction.interaction_id
            }
            crate::domain::agent_interaction::AgentInteraction::Choice(interaction) => {
                &interaction.interaction_id
            }
            crate::domain::agent_interaction::AgentInteraction::TextReply(interaction) => {
                &interaction.interaction_id
            }
        }
    }
}
