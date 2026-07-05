//! 会话状态 reducer。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::domain::agent_event::AgentEvent;
use crate::domain::agent_interaction::AgentInteraction;
use crate::domain::agent_session::{AgentSession, SessionKey, SessionStatus};
use crate::domain::usage::UnixMillis;

const MAX_HIERARCHY_DEPTH: u8 = 8;

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

    /// 按展示分组、块级捕捉锚点和父子相邻规则返回 session key。
    pub fn sorted_session_keys(&self) -> Vec<SessionKey> {
        self.sorted_session_entries()
            .into_iter()
            .map(|entry| entry.session_key)
            .collect()
    }

    /// 返回当前有效展示缩进层级。
    pub fn effective_session_indent_levels(&self) -> BTreeMap<SessionKey, u8> {
        self.sorted_session_entries()
            .into_iter()
            .map(|entry| (entry.session_key, entry.indent_level))
            .collect()
    }

    /// 返回超过保留窗口的已完成 session key。
    pub fn expired_completed_session_keys(
        &self,
        now: UnixMillis,
        retention_millis: u64,
    ) -> Vec<SessionKey> {
        let cutoff = now.value.saturating_sub(retention_millis);
        self.sessions
            .iter()
            .filter_map(|(session_key, session)| {
                if session.status != SessionStatus::Completed {
                    return None;
                }
                let completed_at = session.completed_at?;
                if completed_at.value < cutoff {
                    return Some(session_key.clone());
                }

                None
            })
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
            AgentEvent::HierarchyUpdated(event) => {
                let Some(session) = self.sessions.get_mut(&event.session_key) else {
                    return;
                };
                let parent_session_key = event
                    .parent_session_key
                    .filter(|parent_key| parent_key != &event.session_key);
                session.hierarchy_depth =
                    normalized_hierarchy_depth(parent_session_key.as_ref(), event.hierarchy_depth);
                session.parent_session_key = parent_session_key;
                session.updated_at = event.updated_at;
            }
        }
    }

    /// 按父子展示顺序返回 session key 与有效缩进。
    fn sorted_session_entries(&self) -> Vec<SortedSessionEntry> {
        let mut children_by_parent: BTreeMap<SessionKey, Vec<SessionKey>> = BTreeMap::new();
        let mut attached_children = BTreeSet::new();

        for (session_key, session) in &self.sessions {
            let Some(parent_key) = self.valid_parent_key(session_key, session) else {
                continue;
            };
            attached_children.insert(session_key.clone());
            children_by_parent
                .entry(parent_key)
                .or_default()
                .push(session_key.clone());
        }
        for children in children_by_parent.values_mut() {
            self.sort_session_keys_by_capture_order(children);
        }

        let mut roots = self
            .sessions
            .keys()
            .filter(|session_key| !attached_children.contains(*session_key))
            .cloned()
            .collect::<Vec<_>>();
        self.sort_root_session_keys_by_display_order(&mut roots, &children_by_parent);

        let mut output = Vec::new();
        for root in roots {
            self.push_sorted_session_entries(&root, 0, &children_by_parent, &mut output);
        }

        output
    }

    /// 返回当前 session 的有效父级；无父级、父级缺失或存在环时返回空。
    fn valid_parent_key(
        &self,
        session_key: &SessionKey,
        session: &AgentSession,
    ) -> Option<SessionKey> {
        let parent_key = session.parent_session_key.as_ref()?;
        if parent_key == session_key || !self.sessions.contains_key(parent_key) {
            return None;
        }
        if self.has_parent_cycle(session_key) {
            return None;
        }

        Some(parent_key.clone())
    }

    /// 判断父级链路是否成环。
    fn has_parent_cycle(&self, session_key: &SessionKey) -> bool {
        let mut seen = BTreeSet::new();
        let mut current_key = session_key;
        loop {
            if !seen.insert(current_key.clone()) {
                return true;
            }
            let Some(session) = self.sessions.get(current_key) else {
                return false;
            };
            let Some(parent_key) = session.parent_session_key.as_ref() else {
                return false;
            };
            current_key = parent_key;
        }
    }

    /// 递归输出父子相邻的排序结果。
    fn push_sorted_session_entries(
        &self,
        session_key: &SessionKey,
        indent_level: u8,
        children_by_parent: &BTreeMap<SessionKey, Vec<SessionKey>>,
        output: &mut Vec<SortedSessionEntry>,
    ) {
        output.push(SortedSessionEntry {
            session_key: session_key.clone(),
            indent_level,
        });
        let Some(children) = children_by_parent.get(session_key) else {
            return;
        };
        let child_indent = indent_level.saturating_add(1);
        for child_key in children {
            self.push_sorted_session_entries(child_key, child_indent, children_by_parent, output);
        }
    }

    /// 按捕捉顺序排序 key。
    fn sort_session_keys_by_capture_order(&self, session_keys: &mut [SessionKey]) {
        session_keys.sort_by(|left_key, right_key| {
            let left = self
                .sessions
                .get(left_key)
                .expect("left session key should exist");
            let right = self
                .sessions
                .get(right_key)
                .expect("right session key should exist");
            compare_sessions_by_capture_order(left, right)
        });
    }

    /// 按展示分组和块级捕捉锚点排序顶层 key。
    fn sort_root_session_keys_by_display_order(
        &self,
        session_keys: &mut [SessionKey],
        children_by_parent: &BTreeMap<SessionKey, Vec<SessionKey>>,
    ) {
        let sort_keys = session_keys
            .iter()
            .map(|session_key| {
                (
                    session_key.clone(),
                    self.session_block_sort_key(session_key, children_by_parent),
                )
            })
            .collect::<BTreeMap<_, _>>();

        session_keys.sort_by(|left_key, right_key| {
            let left = sort_keys
                .get(left_key)
                .expect("left root sort key should exist");
            let right = sort_keys
                .get(right_key)
                .expect("right root sort key should exist");
            compare_session_blocks(left, right).then_with(|| left_key.cmp(right_key))
        });
    }

    /// 计算一个展示块的排序键。
    fn session_block_sort_key(
        &self,
        root_key: &SessionKey,
        children_by_parent: &BTreeMap<SessionKey, Vec<SessionKey>>,
    ) -> SessionBlockSortKey {
        let mut sort_key = SessionBlockSortKey::empty();
        self.collect_session_block_sort_key(root_key, children_by_parent, &mut sort_key);
        sort_key
    }

    /// 递归收集展示块内所有有效 session 的排序事实。
    fn collect_session_block_sort_key(
        &self,
        session_key: &SessionKey,
        children_by_parent: &BTreeMap<SessionKey, Vec<SessionKey>>,
        sort_key: &mut SessionBlockSortKey,
    ) {
        let Some(session) = self.sessions.get(session_key) else {
            return;
        };
        sort_key.include_session(session);

        let Some(children) = children_by_parent.get(session_key) else {
            return;
        };
        for child_key in children {
            self.collect_session_block_sort_key(child_key, children_by_parent, sort_key);
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

/// 排序后的 session 条目。
struct SortedSessionEntry {
    /// 会话唯一键。
    session_key: SessionKey,
    /// 当前有效缩进层级。
    indent_level: u8,
}

/// 展示块排序键。
#[derive(Clone, Debug, Eq, PartialEq)]
struct SessionBlockSortKey {
    /// 块内是否存在未完成 session。
    unfinished: bool,
    /// 块内最新未完成 session 的捕捉序号。
    unfinished_capture_sequence: Option<u64>,
    /// 块内最新 session 的捕捉序号。
    latest_capture_sequence: u64,
}

impl SessionBlockSortKey {
    /// 创建空排序键。
    fn empty() -> Self {
        Self {
            unfinished: false,
            unfinished_capture_sequence: None,
            latest_capture_sequence: 0,
        }
    }

    /// 纳入一个 session 的排序事实。
    fn include_session(&mut self, session: &AgentSession) {
        self.latest_capture_sequence = self.latest_capture_sequence.max(session.capture_sequence);
        if session_is_unfinished_for_display(session.status) {
            self.unfinished = true;
            self.unfinished_capture_sequence = Some(
                self.unfinished_capture_sequence
                    .unwrap_or(0)
                    .max(session.capture_sequence),
            );
        }
    }

    /// 返回块级捕捉锚点。
    fn anchor_capture_sequence(&self) -> u64 {
        self.unfinished_capture_sequence
            .unwrap_or(self.latest_capture_sequence)
    }
}

/// 归一化存储层级。
fn normalized_hierarchy_depth(parent_key: Option<&SessionKey>, hierarchy_depth: u8) -> u8 {
    if parent_key.is_none() {
        return 0;
    }

    hierarchy_depth.clamp(1, MAX_HIERARCHY_DEPTH)
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

/// 判断 session 是否属于展示上的未完成分组。
fn session_is_unfinished_for_display(status: SessionStatus) -> bool {
    matches!(
        status,
        SessionStatus::Running
            | SessionStatus::WaitingForApproval
            | SessionStatus::WaitingForAnswer
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

/// 比较两个展示块的展示顺序。
fn compare_session_blocks(
    left: &SessionBlockSortKey,
    right: &SessionBlockSortKey,
) -> std::cmp::Ordering {
    right.unfinished.cmp(&left.unfinished).then_with(|| {
        right
            .anchor_capture_sequence()
            .cmp(&left.anchor_capture_sequence())
    })
}

#[cfg(test)]
mod tests {
    use super::SessionState;
    use crate::domain::agent_event::{
        ActivityUpdatedEvent, AgentEvent, AnswerRequestedEvent, ApprovalRequestedEvent,
        CapabilitiesUpdatedEvent, DetachedEvent, FailedEvent, HierarchyUpdatedEvent,
        InteractionCompletedEvent, SessionStartedEvent, TurnCompletedEvent, UsageUpdatedEvent,
        UserMessageUpdatedEvent,
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
    fn sorted_sessions_keep_capture_order_within_unfinished_group() {
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

    #[test]
    fn sorted_sessions_keep_unfinished_group_above_finished_group() {
        let running = session_key("project-a", "running");
        let waiting_approval = session_key("project-a", "waiting-approval");
        let waiting_answer = session_key("project-a", "waiting-answer");
        let completed = session_key("project-a", "completed");
        let failed = session_key("project-a", "failed");
        let detached = session_key("project-a", "detached");
        let state = SessionState::empty()
            .apply_event(started_event(running.clone(), 1))
            .apply_event(started_event(completed.clone(), 2))
            .apply_event(completed_event(completed.clone(), 3))
            .apply_event(started_event(failed.clone(), 4))
            .apply_event(failed_event(failed.clone(), 5))
            .apply_event(started_event(detached.clone(), 6))
            .apply_event(detached_event(detached.clone(), 7))
            .apply_event(started_event(waiting_approval.clone(), 8))
            .apply_event(approval_event(waiting_approval.clone(), 9))
            .apply_event(started_event(waiting_answer.clone(), 10))
            .apply_event(text_answer_event(waiting_answer.clone(), 11));

        assert_eq!(
            state.sorted_session_keys(),
            vec![
                waiting_answer,
                waiting_approval,
                running,
                detached,
                failed,
                completed
            ]
        );
    }

    #[test]
    fn expired_completed_session_keys_only_returns_completed_after_retention() {
        let expired = session_key("project-a", "expired");
        let exact = session_key("project-a", "exact");
        let fresh = session_key("project-a", "fresh");
        let failed = session_key("project-a", "failed");
        let detached = session_key("project-a", "detached");
        let running = session_key("project-a", "running");
        let state = SessionState::empty()
            .apply_event(started_event(expired.clone(), 1))
            .apply_event(completed_event(expired.clone(), 99))
            .apply_event(started_event(exact.clone(), 2))
            .apply_event(completed_event(exact, 100))
            .apply_event(started_event(fresh.clone(), 3))
            .apply_event(completed_event(fresh, 101))
            .apply_event(started_event(failed.clone(), 4))
            .apply_event(failed_event(failed, 50))
            .apply_event(started_event(detached.clone(), 5))
            .apply_event(detached_event(detached, 50))
            .apply_event(started_event(running, 6));

        let expired_keys = state.expired_completed_session_keys(UnixMillis::new(200), 100);

        assert_eq!(expired_keys, vec![expired]);
    }

    #[test]
    fn expired_completed_session_keys_keeps_future_completed_time() {
        let future = session_key("project-a", "future");
        let state = SessionState::empty()
            .apply_event(started_event(future.clone(), 1))
            .apply_event(completed_event(future, 300));

        assert!(state
            .expired_completed_session_keys(UnixMillis::new(200), 100)
            .is_empty());
    }

    #[test]
    fn sorted_sessions_move_resumed_session_to_unfinished_group_without_new_capture() {
        let first = session_key("project-a", "first");
        let second = session_key("project-a", "second");
        let state = SessionState::empty()
            .apply_event(started_event(first.clone(), 1))
            .apply_event(completed_event(first.clone(), 2))
            .apply_event(started_event(second.clone(), 3))
            .apply_event(completed_event(second.clone(), 4))
            .apply_event(AgentEvent::UserMessageUpdated(UserMessageUpdatedEvent {
                session_key: first.clone(),
                summary: "继续处理".to_string(),
                updated_at: UnixMillis::new(5),
            }));
        let resumed = state.sessions.get(&first).expect("session should exist");

        assert_eq!(resumed.status, SessionStatus::Running);
        assert_eq!(resumed.capture_sequence, 0);
        assert_eq!(state.sorted_session_keys(), vec![first, second]);
    }

    #[test]
    fn sorted_sessions_keep_running_parent_block_anchor_when_child_completed() {
        let other_running = session_key("project-a", "other-running");
        let parent = session_key("project-a", "parent");
        let child = session_key("project-a", "child");
        let state = SessionState::empty()
            .apply_event(started_event(parent.clone(), 1))
            .apply_event(started_event(child.clone(), 2))
            .apply_event(completed_event(child.clone(), 3))
            .apply_event(started_event(other_running.clone(), 4))
            .apply_event(hierarchy_event(child.clone(), Some(parent.clone()), 1, 5));

        assert_eq!(
            state.sorted_session_keys(),
            vec![other_running, parent, child]
        );
    }

    #[test]
    fn sorted_sessions_use_newest_unfinished_child_as_block_anchor() {
        let other_running = session_key("project-a", "other-running");
        let parent = session_key("project-a", "parent");
        let child = session_key("project-a", "child");
        let state = SessionState::empty()
            .apply_event(started_event(other_running.clone(), 1))
            .apply_event(started_event(parent.clone(), 2))
            .apply_event(started_event(child.clone(), 3))
            .apply_event(hierarchy_event(child.clone(), Some(parent.clone()), 1, 4));

        assert_eq!(
            state.sorted_session_keys(),
            vec![parent.clone(), child.clone(), other_running]
        );
        assert_eq!(
            state.effective_session_indent_levels().get(&child),
            Some(&1)
        );
    }

    #[test]
    fn sorted_sessions_put_block_with_running_child_in_unfinished_group() {
        let completed_root = session_key("project-a", "completed-root");
        let parent = session_key("project-a", "parent");
        let child = session_key("project-a", "child");
        let state = SessionState::empty()
            .apply_event(started_event(parent.clone(), 1))
            .apply_event(completed_event(parent.clone(), 2))
            .apply_event(started_event(completed_root.clone(), 3))
            .apply_event(completed_event(completed_root.clone(), 4))
            .apply_event(started_event(child.clone(), 5))
            .apply_event(hierarchy_event(child.clone(), Some(parent.clone()), 1, 6));

        assert_eq!(
            state.sorted_session_keys(),
            vec![parent, child, completed_root]
        );
    }

    #[test]
    fn hierarchy_update_does_not_create_unknown_child_session() {
        let parent = session_key("project-a", "parent");
        let child = session_key("project-a", "child");
        let state = SessionState::empty()
            .apply_event(started_event(parent, 1))
            .apply_event(hierarchy_event(child.clone(), None, 1, 2));

        assert!(!state.sessions.contains_key(&child));
    }

    #[test]
    fn sorted_sessions_place_child_after_existing_parent() {
        let older_parent = session_key("project-a", "older-parent");
        let child = session_key("project-a", "child");
        let newer_root = session_key("project-a", "newer-root");
        let state = SessionState::empty()
            .apply_event(started_event(older_parent.clone(), 1))
            .apply_event(started_event(child.clone(), 2))
            .apply_event(started_event(newer_root.clone(), 3))
            .apply_event(hierarchy_event(
                child.clone(),
                Some(older_parent.clone()),
                1,
                4,
            ));

        assert_eq!(
            state.sorted_session_keys(),
            vec![newer_root, older_parent.clone(), child.clone()]
        );
        assert_eq!(
            state.effective_session_indent_levels().get(&child),
            Some(&1)
        );
    }

    #[test]
    fn hierarchy_update_keeps_session_behavior_state() {
        let parent = session_key("project-a", "parent");
        let child = session_key("project-a", "child");
        let state = SessionState::empty()
            .apply_event(started_event(parent.clone(), 1))
            .apply_event(started_event(child.clone(), 2))
            .apply_event(approval_event(child.clone(), 3))
            .apply_event(hierarchy_event(child.clone(), Some(parent), 4, 4));
        let session = state.sessions.get(&child).expect("child should exist");

        assert_eq!(session.status, SessionStatus::WaitingForApproval);
        assert!(session.pending_interaction.is_some());
        assert_eq!(session.capture_sequence, 1);
        assert_eq!(session.hierarchy_depth, 4);
        assert_eq!(session.updated_at, UnixMillis::new(4));
    }

    #[test]
    fn missing_parent_session_makes_child_top_level() {
        let parent = session_key("project-a", "missing-parent");
        let child = session_key("project-a", "child");
        let state = SessionState::empty()
            .apply_event(started_event(child.clone(), 1))
            .apply_event(hierarchy_event(child.clone(), Some(parent), 1, 2));

        assert_eq!(state.sorted_session_keys(), vec![child.clone()]);
        assert_eq!(
            state.effective_session_indent_levels().get(&child),
            Some(&0)
        );
    }

    #[test]
    fn hierarchy_cycle_makes_sessions_top_level() {
        let first = session_key("project-a", "first");
        let second = session_key("project-a", "second");
        let state = SessionState::empty()
            .apply_event(started_event(first.clone(), 1))
            .apply_event(started_event(second.clone(), 2))
            .apply_event(hierarchy_event(first.clone(), Some(second.clone()), 1, 3))
            .apply_event(hierarchy_event(second.clone(), Some(first.clone()), 1, 4));

        assert_eq!(
            state.sorted_session_keys(),
            vec![second.clone(), first.clone()]
        );
        let indent_levels = state.effective_session_indent_levels();
        assert_eq!(indent_levels.get(&first), Some(&0));
        assert_eq!(indent_levels.get(&second), Some(&0));
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

    fn hierarchy_event(
        session_key: SessionKey,
        parent_session_key: Option<SessionKey>,
        hierarchy_depth: u8,
        updated_at: u64,
    ) -> AgentEvent {
        AgentEvent::HierarchyUpdated(HierarchyUpdatedEvent {
            session_key,
            parent_session_key,
            hierarchy_depth,
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

    fn completed_event(session_key: SessionKey, updated_at: u64) -> AgentEvent {
        AgentEvent::TurnCompleted(TurnCompletedEvent {
            session_key,
            summary: None,
            updated_at: UnixMillis::new(updated_at),
        })
    }

    fn failed_event(session_key: SessionKey, updated_at: u64) -> AgentEvent {
        AgentEvent::Failed(FailedEvent {
            session_key,
            error: AppError::new(
                AppErrorCode::BridgeUnavailable,
                "bridge 不可用",
                None,
                false,
                None,
            ),
            updated_at: UnixMillis::new(updated_at),
        })
    }

    fn detached_event(session_key: SessionKey, updated_at: u64) -> AgentEvent {
        AgentEvent::Detached(DetachedEvent {
            session_key,
            reason: Some("进程不可见".to_string()),
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
