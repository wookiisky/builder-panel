//! 通知应用服务。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::agent_session::SessionKey;
use crate::domain::app_error::AppError;
use crate::ports::notification_port::NotificationPort;

/// 通知合并窗口。
pub const NOTIFICATION_MERGE_WINDOW_MS: i64 = 3000;

/// 通知类型。
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    /// Turn 完成通知。
    TurnCompleted,
    /// 等待审批通知。
    WaitingApproval,
    /// 等待选择通知。
    WaitingChoice,
    /// 失败通知。
    Failed,
}

/// 通知输入事件。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NotificationRequest {
    /// 所属会话。
    pub session_key: SessionKey,
    /// 通知类型。
    pub kind: NotificationKind,
    /// 通知标题。
    pub title: String,
    /// 通知正文。
    pub body: String,
}

/// 通知计划。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NotificationPlan {
    /// 所属会话。
    pub session_key: SessionKey,
    /// 通知类型。
    pub kind: NotificationKind,
    /// 通知标题。
    pub title: String,
    /// 通知正文。
    pub body: String,
    /// 合并后的事件数量。
    pub merged_count: u32,
}

/// 通知点击动作。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NotificationClickAction {
    /// 应定位到的 session。
    pub session_key: SessionKey,
    /// 是否聚焦 panel。
    pub focus_panel: bool,
    /// 是否展开 panel。
    pub expand_panel: bool,
    /// 是否打开过程时间线。
    pub open_timeline: bool,
}

/// 通知应用服务。
pub struct NotificationService<Port>
where
    Port: NotificationPort,
{
    /// 通知发送端口。
    port: Port,
    /// 最近已发送通知。
    recent: BTreeMap<NotificationDedupKey, RecentNotification>,
}

impl<Port> NotificationService<Port>
where
    Port: NotificationPort,
{
    /// 创建通知应用服务。
    pub fn new(port: Port) -> Self {
        Self {
            port,
            recent: BTreeMap::new(),
        }
    }

    /// 生成并发送通知计划。
    pub fn notify(
        &mut self,
        request: NotificationRequest,
        current_session: Option<&SessionKey>,
        now_ms: i64,
    ) -> Result<Option<NotificationPlan>, AppError> {
        if current_session == Some(&request.session_key) {
            return Ok(None);
        }

        let key = NotificationDedupKey {
            session_key: request.session_key.clone(),
            kind: request.kind,
        };
        let recent = self.recent.get(&key);
        let merged_count = match recent {
            Some(recent) if now_ms - recent.sent_at_ms <= NOTIFICATION_MERGE_WINDOW_MS => {
                recent.merged_count.saturating_add(1)
            }
            _ => 1,
        };
        let title = if merged_count > 1 {
            format!("{}（{}）", request.title, merged_count)
        } else {
            request.title
        };
        let plan = NotificationPlan {
            session_key: request.session_key,
            kind: request.kind,
            title,
            body: request.body,
            merged_count,
        };

        self.port.show_notification(&plan)?;
        self.recent.insert(
            key,
            RecentNotification {
                sent_at_ms: now_ms,
                merged_count,
            },
        );

        Ok(Some(plan))
    }

    /// 将通知点击转换为 UI 定位动作。
    pub fn click_action(&self, session_key: SessionKey) -> NotificationClickAction {
        NotificationClickAction {
            session_key,
            focus_panel: true,
            expand_panel: true,
            open_timeline: false,
        }
    }
}

/// 通知去重键。
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct NotificationDedupKey {
    /// 所属会话。
    session_key: SessionKey,
    /// 通知类型。
    kind: NotificationKind,
}

/// 最近通知记录。
#[derive(Clone, Debug, Eq, PartialEq)]
struct RecentNotification {
    /// 发送时间。
    sent_at_ms: i64,
    /// 合并后的事件数量。
    merged_count: u32,
}

#[cfg(test)]
mod tests {
    use super::{NotificationKind, NotificationRequest, NotificationService};
    use crate::adapters::notification::RecordingNotificationAdapter;
    use crate::domain::agent_session::{AgentKind, ConversationId, ProjectId, SessionKey};

    #[test]
    fn suppresses_notification_for_current_session() {
        let adapter = RecordingNotificationAdapter::new();
        let mut service = NotificationService::new(adapter);
        let session_key = session_key("project-a", "conversation-a");
        let request = request(session_key.clone(), NotificationKind::WaitingApproval);

        let plan = service
            .notify(request, Some(&session_key), 1000)
            .expect("notification should evaluate");

        assert_eq!(plan, None);
    }

    #[test]
    fn merges_repeated_notifications_for_same_session_and_kind() {
        let adapter = RecordingNotificationAdapter::new();
        let mut service = NotificationService::new(adapter);
        let session_key = session_key("project-a", "conversation-a");
        let first = request(session_key.clone(), NotificationKind::WaitingApproval);
        let second = request(session_key.clone(), NotificationKind::WaitingApproval);

        let first_plan = service
            .notify(first, None, 1000)
            .expect("first notification should evaluate")
            .expect("first notification should send");
        let second_plan = service
            .notify(second, None, 2000)
            .expect("second notification should evaluate")
            .expect("second notification should send");

        assert_eq!(first_plan.merged_count, 1);
        assert_eq!(second_plan.merged_count, 2);
        assert!(second_plan.title.contains("2"));
    }

    #[test]
    fn click_action_focuses_panel_without_opening_timeline() {
        let adapter = RecordingNotificationAdapter::new();
        let service = NotificationService::new(adapter);
        let session_key = session_key("project-a", "conversation-a");

        let action = service.click_action(session_key.clone());

        assert_eq!(action.session_key, session_key);
        assert!(action.focus_panel);
        assert!(action.expand_panel);
        assert!(!action.open_timeline);
    }

    fn request(session_key: SessionKey, kind: NotificationKind) -> NotificationRequest {
        NotificationRequest {
            session_key,
            kind,
            title: "等待处理".to_string(),
            body: "需要用户确认".to_string(),
        }
    }

    fn session_key(project_id: &str, conversation_id: &str) -> SessionKey {
        SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new(project_id),
            ConversationId::new(conversation_id),
        )
    }
}
