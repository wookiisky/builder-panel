//! 过程事件时间线 adapter。

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use crate::domain::agent_event::AgentEvent;
use crate::domain::agent_session::SessionKey;
use crate::domain::app_error::{AppError, AppErrorCode, FallbackAction};
use crate::ports::process_timeline_port::{
    ProcessTimelineEventKind, ProcessTimelineItem, ProcessTimelineReaderPort,
    ProcessTimelineReleasePort, ProcessTimelineWriterPort,
};

/// 默认单 session 最大 timeline 条目数。
pub const DEFAULT_SESSION_TIMELINE_LIMIT: usize = 500;
/// 默认全局最大 timeline 条目数。
pub const DEFAULT_GLOBAL_TIMELINE_LIMIT: usize = 5_000;
/// 关闭弹层后可释放正文的字符阈值。
pub const LARGE_TEXT_RELEASE_THRESHOLD_CHARS: usize = 512;

/// 内存过程事件时间线缓存。
#[derive(Clone, Debug)]
pub struct InMemoryProcessTimelineCache {
    /// 单 session 最大条目数。
    session_limit: usize,
    /// 全局最大条目数。
    global_limit: usize,
    /// 按 session 分片保存的条目。
    shards: BTreeMap<SessionKey, Vec<CachedTimelineItem>>,
    /// 单调递增写入顺序。
    next_order: u64,
}

impl InMemoryProcessTimelineCache {
    /// 使用默认上限创建缓存。
    pub fn new() -> Self {
        Self::with_limits(
            DEFAULT_SESSION_TIMELINE_LIMIT,
            DEFAULT_GLOBAL_TIMELINE_LIMIT,
        )
    }

    /// 使用指定上限创建缓存。
    pub fn with_limits(session_limit: usize, global_limit: usize) -> Self {
        Self {
            session_limit: session_limit.max(1),
            global_limit: global_limit.max(1),
            shards: BTreeMap::new(),
            next_order: 0,
        }
    }

    /// 从归一领域事件接收一条时间线事件。
    pub fn record_agent_event(&mut self, event: &AgentEvent) -> Result<bool, AppError> {
        let Some(item) = timeline_item_from_agent_event(event) else {
            return Ok(false);
        };

        self.record_timeline_item(item)
    }

    /// 返回当前缓存条目总数。
    pub fn total_item_count(&self) -> usize {
        self.shards.values().map(Vec::len).sum()
    }

    /// 返回当前缓存正文字符数。
    #[cfg(test)]
    pub fn total_body_chars(&self) -> usize {
        self.shards
            .values()
            .flat_map(|items| items.iter())
            .map(|item| item.item.body.chars().count())
            .sum()
    }

    fn contains_item(&self, session_key: &SessionKey, item_id: &str) -> bool {
        self.shards
            .get(session_key)
            .is_some_and(|items| items.iter().any(|item| item.item.item_id == item_id))
    }

    fn enforce_limits(&mut self) {
        let session_keys = self.shards.keys().cloned().collect::<Vec<_>>();
        for session_key in session_keys {
            while self
                .shards
                .get(&session_key)
                .is_some_and(|items| items.len() > self.session_limit)
            {
                self.evict_one_from_session(&session_key);
            }
        }

        while self.total_item_count() > self.global_limit {
            self.evict_one_global();
        }
    }

    fn evict_one_from_session(&mut self, session_key: &SessionKey) {
        let Some(items) = self.shards.get_mut(session_key) else {
            return;
        };
        if items.is_empty() {
            return;
        }

        let index = oldest_low_priority_index(items).unwrap_or(0);
        items.remove(index);
        if items.is_empty() {
            self.shards.remove(session_key);
        }
    }

    fn evict_one_global(&mut self) {
        let candidate = self
            .shards
            .iter()
            .filter_map(|(session_key, items)| {
                oldest_low_priority_index(items)
                    .or_else(|| oldest_index(items))
                    .map(|index| (session_key.clone(), index, items[index].order))
            })
            .min_by_key(|(_, _, order)| *order);

        let Some((session_key, index, _)) = candidate else {
            return;
        };
        let Some(items) = self.shards.get_mut(&session_key) else {
            return;
        };
        items.remove(index);
        if items.is_empty() {
            self.shards.remove(&session_key);
        }
    }
}

impl Default for InMemoryProcessTimelineCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessTimelineWriterPort for InMemoryProcessTimelineCache {
    fn record_timeline_item(&mut self, item: ProcessTimelineItem) -> Result<bool, AppError> {
        if self.contains_item(&item.session_key, &item.item_id) {
            return Ok(false);
        }

        self.next_order = self.next_order.saturating_add(1);
        let session_items = self.shards.entry(item.session_key.clone()).or_default();
        session_items.push(CachedTimelineItem {
            priority: TimelineRetentionPriority::from_item(&item),
            order: self.next_order,
            item,
        });
        self.enforce_limits();

        Ok(true)
    }
}

impl ProcessTimelineReaderPort for InMemoryProcessTimelineCache {
    fn read_timeline(
        &self,
        session_key: &SessionKey,
    ) -> Result<Vec<ProcessTimelineItem>, AppError> {
        Ok(self
            .shards
            .get(session_key)
            .map(|items| items.iter().map(|item| item.item.clone()).collect())
            .unwrap_or_default())
    }
}

impl ProcessTimelineReleasePort for InMemoryProcessTimelineCache {
    fn release_large_texts(&mut self, session_key: &SessionKey) -> Result<usize, AppError> {
        let Some(items) = self.shards.get_mut(session_key) else {
            return Ok(0);
        };

        let mut released_count = 0;
        for item in items {
            if item.item.body.chars().count() <= LARGE_TEXT_RELEASE_THRESHOLD_CHARS {
                continue;
            }

            item.item.body = "长正文缓存已释放，重新打开后仅保留标题和类型。".to_string();
            released_count += 1;
        }

        Ok(released_count)
    }
}

/// 内部缓存条目。
#[derive(Clone, Debug)]
struct CachedTimelineItem {
    /// 已清洗条目。
    item: ProcessTimelineItem,
    /// 写入顺序。
    order: u64,
    /// 保留优先级。
    priority: TimelineRetentionPriority,
}

/// 时间线淘汰优先级。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TimelineRetentionPriority {
    /// 普通活动，可优先淘汰。
    Low,
    /// 审批、回复、失败等关键事件。
    High,
}

impl TimelineRetentionPriority {
    /// 根据条目类型和内容决定保留优先级。
    fn from_item(item: &ProcessTimelineItem) -> Self {
        if matches!(
            item.kind,
            ProcessTimelineEventKind::Approval | ProcessTimelineEventKind::Reply
        ) || item.title.contains("失败")
            || item.body.contains("失败")
            || item.title.contains("错误")
            || item.body.contains("错误")
        {
            return Self::High;
        }

        Self::Low
    }
}

fn oldest_low_priority_index(items: &[CachedTimelineItem]) -> Option<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.priority == TimelineRetentionPriority::Low)
        .min_by_key(|(_, item)| item.order)
        .map(|(index, _)| index)
}

fn oldest_index(items: &[CachedTimelineItem]) -> Option<usize> {
    items
        .iter()
        .enumerate()
        .min_by_key(|(_, item)| item.order)
        .map(|(index, _)| index)
}

/// 将归一领域事件转换成过程时间线条目。
pub fn timeline_item_from_agent_event(event: &AgentEvent) -> Option<ProcessTimelineItem> {
    match event {
        AgentEvent::SessionStarted(event) => Some(process_item(
            &event.session_key,
            ProcessTimelineEventKind::System,
            "会话开始",
            event.summary.as_deref().unwrap_or("agent 会话已开始"),
            event.updated_at,
        )),
        AgentEvent::ActivityUpdated(event) => Some(process_item(
            &event.session_key,
            ProcessTimelineEventKind::Activity,
            "活动更新",
            &event.summary,
            event.updated_at,
        )),
        AgentEvent::ApprovalRequested(event) => Some(process_item(
            &event.session_key,
            ProcessTimelineEventKind::Approval,
            "等待审批",
            &event.interaction.request_summary,
            event.updated_at,
        )),
        AgentEvent::AnswerRequested(event) => Some(process_item(
            &event.session_key,
            ProcessTimelineEventKind::Reply,
            "等待回复",
            answer_summary(&event.interaction),
            event.updated_at,
        )),
        AgentEvent::InteractionCompleted(event) => Some(process_item(
            &event.session_key,
            ProcessTimelineEventKind::Activity,
            "交互已回写",
            event.summary.as_deref().unwrap_or("用户交互已回写"),
            event.updated_at,
        )),
        AgentEvent::TurnCompleted(event) => Some(process_item(
            &event.session_key,
            ProcessTimelineEventKind::System,
            "Turn 完成",
            event.summary.as_deref().unwrap_or("agent turn 已完成"),
            event.updated_at,
        )),
        AgentEvent::Failed(event) => Some(process_item(
            &event.session_key,
            ProcessTimelineEventKind::System,
            "Agent 失败",
            &event.error.user_message,
            event.updated_at,
        )),
        AgentEvent::Detached(event) => Some(process_item(
            &event.session_key,
            ProcessTimelineEventKind::System,
            "会话失联",
            event.reason.as_deref().unwrap_or("agent 会话已失联"),
            event.updated_at,
        )),
        AgentEvent::CapabilitiesUpdated(_)
        | AgentEvent::UsageUpdated(_)
        | AgentEvent::JumpTargetUpdated(_) => None,
    }
}

fn answer_summary(interaction: &crate::domain::agent_interaction::AnswerInteraction) -> &str {
    match interaction {
        crate::domain::agent_interaction::AnswerInteraction::Choice(interaction) => {
            &interaction.request_summary
        }
        crate::domain::agent_interaction::AnswerInteraction::TextReply(interaction) => {
            &interaction.request_summary
        }
    }
}

fn process_item(
    session_key: &SessionKey,
    kind: ProcessTimelineEventKind,
    title: &str,
    body: &str,
    created_at: crate::domain::usage::UnixMillis,
) -> ProcessTimelineItem {
    ProcessTimelineItem {
        item_id: item_id(session_key, &kind, title, body, created_at.value),
        session_key: session_key.clone(),
        kind,
        title: title.to_string(),
        body: body.to_string(),
        created_at,
    }
}

fn item_id(
    session_key: &SessionKey,
    kind: &ProcessTimelineEventKind,
    title: &str,
    body: &str,
    created_at: u64,
) -> String {
    let mut hasher = DefaultHasher::new();
    format!("{:?}", session_key.agent_kind).hash(&mut hasher);
    session_key.project_id.value.hash(&mut hasher);
    session_key.conversation_id.value.hash(&mut hasher);
    format!("{kind:?}").hash(&mut hasher);
    title.hash(&mut hasher);
    body.hash(&mut hasher);
    created_at.hash(&mut hasher);

    format!("timeline-{:x}", hasher.finish())
}

/// 创建 timeline 不可用错误。
pub fn timeline_unavailable(message: &str) -> AppError {
    AppError::new(
        AppErrorCode::ProcessTimelineUnavailable,
        message,
        None,
        false,
        Some(FallbackAction::ViewReadOnly),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        InMemoryProcessTimelineCache, TimelineRetentionPriority, LARGE_TEXT_RELEASE_THRESHOLD_CHARS,
    };
    use crate::domain::agent_session::{AgentKind, ConversationId, ProjectId, SessionKey};
    use crate::domain::usage::UnixMillis;
    use crate::ports::process_timeline_port::{
        ProcessTimelineEventKind, ProcessTimelineItem, ProcessTimelineReaderPort,
        ProcessTimelineReleasePort, ProcessTimelineWriterPort,
    };

    #[test]
    fn cache_deduplicates_by_session_and_item_id() {
        let key = session_key("project-a", "conversation-a");
        let mut cache = InMemoryProcessTimelineCache::with_limits(10, 10);
        let item = timeline_item(&key, "same", ProcessTimelineEventKind::Activity, "body", 1);

        assert!(cache
            .record_timeline_item(item.clone())
            .expect("first item should record"));
        assert!(!cache
            .record_timeline_item(item)
            .expect("duplicate item should skip"));

        assert_eq!(
            cache
                .read_timeline(&key)
                .expect("timeline should read")
                .len(),
            1
        );
    }

    #[test]
    fn session_limit_evicts_old_low_priority_before_approval() {
        let key = session_key("project-a", "conversation-a");
        let mut cache = InMemoryProcessTimelineCache::with_limits(2, 10);
        cache
            .record_timeline_item(timeline_item(
                &key,
                "activity-old",
                ProcessTimelineEventKind::Activity,
                "old",
                1,
            ))
            .expect("activity should record");
        cache
            .record_timeline_item(timeline_item(
                &key,
                "approval",
                ProcessTimelineEventKind::Approval,
                "approval",
                2,
            ))
            .expect("approval should record");
        cache
            .record_timeline_item(timeline_item(
                &key,
                "activity-new",
                ProcessTimelineEventKind::Activity,
                "new",
                3,
            ))
            .expect("new activity should record");

        let ids = cache
            .read_timeline(&key)
            .expect("timeline should read")
            .into_iter()
            .map(|item| item.item_id)
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec!["approval".to_string(), "activity-new".to_string()]
        );
    }

    #[test]
    fn global_limit_evicts_oldest_low_priority_across_sessions() {
        let first_key = session_key("project-a", "conversation-a");
        let second_key = session_key("project-b", "conversation-b");
        let mut cache = InMemoryProcessTimelineCache::with_limits(10, 2);
        cache
            .record_timeline_item(timeline_item(
                &first_key,
                "first",
                ProcessTimelineEventKind::Activity,
                "first",
                1,
            ))
            .expect("first should record");
        cache
            .record_timeline_item(timeline_item(
                &second_key,
                "approval",
                ProcessTimelineEventKind::Approval,
                "approval",
                2,
            ))
            .expect("approval should record");
        cache
            .record_timeline_item(timeline_item(
                &second_key,
                "latest",
                ProcessTimelineEventKind::Activity,
                "latest",
                3,
            ))
            .expect("latest should record");

        assert!(cache
            .read_timeline(&first_key)
            .expect("timeline should read")
            .is_empty());
        assert_eq!(cache.total_item_count(), 2);
    }

    #[test]
    fn release_large_texts_reduces_cached_body_size() {
        let key = session_key("project-a", "conversation-a");
        let body = "长".repeat(LARGE_TEXT_RELEASE_THRESHOLD_CHARS + 10);
        let mut cache = InMemoryProcessTimelineCache::with_limits(10, 10);
        cache
            .record_timeline_item(timeline_item(
                &key,
                "large",
                ProcessTimelineEventKind::Tool,
                &body,
                1,
            ))
            .expect("large item should record");
        let before = cache.total_body_chars();

        let released = cache
            .release_large_texts(&key)
            .expect("large texts should release");
        let after = cache.total_body_chars();

        assert_eq!(released, 1);
        assert!(after < before);
    }

    #[test]
    fn priority_marks_approval_reply_and_failures_as_high() {
        let key = session_key("project-a", "conversation-a");
        let approval = timeline_item(
            &key,
            "approval",
            ProcessTimelineEventKind::Approval,
            "等待",
            1,
        );
        let failure = timeline_item(
            &key,
            "failure",
            ProcessTimelineEventKind::System,
            "写入失败",
            2,
        );
        let activity = timeline_item(
            &key,
            "activity",
            ProcessTimelineEventKind::Activity,
            "普通",
            3,
        );

        assert_eq!(
            TimelineRetentionPriority::from_item(&approval),
            TimelineRetentionPriority::High
        );
        assert_eq!(
            TimelineRetentionPriority::from_item(&failure),
            TimelineRetentionPriority::High
        );
        assert_eq!(
            TimelineRetentionPriority::from_item(&activity),
            TimelineRetentionPriority::Low
        );
    }

    fn session_key(project_id: &str, conversation_id: &str) -> SessionKey {
        SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new(project_id),
            ConversationId::new(conversation_id),
        )
    }

    fn timeline_item(
        session_key: &SessionKey,
        item_id: &str,
        kind: ProcessTimelineEventKind,
        body: &str,
        created_at: u64,
    ) -> ProcessTimelineItem {
        ProcessTimelineItem {
            item_id: item_id.to_string(),
            session_key: session_key.clone(),
            kind,
            title: item_id.to_string(),
            body: body.to_string(),
            created_at: UnixMillis::new(created_at),
        }
    }
}
