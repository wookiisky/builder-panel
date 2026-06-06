//! 快捷回复应用服务。

use serde::{Deserialize, Serialize};

use crate::domain::agent_session::{AgentKind, SessionKey};

/// 快捷回复唯一标识。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ShortcutReplyId {
    /// 配置内稳定 ID。
    pub value: String,
}

/// 快捷回复配置。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ShortcutReply {
    /// 快捷回复唯一标识。
    pub id: ShortcutReplyId,
    /// 展示标签。
    pub label: String,
    /// 回复正文。
    pub content: String,
    /// 是否启用。
    pub enabled: bool,
    /// 排序值，数值越小越靠前。
    pub order: u32,
    /// 绑定的 agent 类型，空列表表示全部 agent。
    pub agent_kinds: Vec<AgentKind>,
    /// 绑定的项目 ID，空列表表示全部项目。
    pub project_ids: Vec<String>,
}

/// 快捷回复应用服务。
pub struct ShortcutReplyService {
    /// 当前配置内的快捷回复。
    shortcuts: Vec<ShortcutReply>,
}

impl ShortcutReplyService {
    /// 创建快捷回复应用服务。
    pub fn new(shortcuts: Vec<ShortcutReply>) -> Self {
        Self { shortcuts }
    }

    /// 返回当前 session 可用快捷回复。
    pub fn available_for_session(&self, session_key: &SessionKey) -> Vec<ShortcutReply> {
        let mut matched = self
            .shortcuts
            .iter()
            .filter(|shortcut| shortcut.enabled)
            .filter(|shortcut| agent_matches(shortcut, session_key))
            .filter(|shortcut| project_matches(shortcut, session_key))
            .cloned()
            .collect::<Vec<_>>();

        matched.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then(left.label.cmp(&right.label))
                .then(left.id.value.cmp(&right.id.value))
        });
        matched
    }
}

/// 判断快捷回复是否匹配 agent。
fn agent_matches(shortcut: &ShortcutReply, session_key: &SessionKey) -> bool {
    shortcut.agent_kinds.is_empty() || shortcut.agent_kinds.contains(&session_key.agent_kind)
}

/// 判断快捷回复是否匹配项目。
fn project_matches(shortcut: &ShortcutReply, session_key: &SessionKey) -> bool {
    shortcut.project_ids.is_empty()
        || shortcut
            .project_ids
            .iter()
            .any(|project_id| project_id == &session_key.project_id.value)
}

#[cfg(test)]
mod tests {
    use super::{ShortcutReply, ShortcutReplyId, ShortcutReplyService};
    use crate::domain::agent_session::{AgentKind, ConversationId, ProjectId, SessionKey};

    #[test]
    fn filters_disabled_agent_and_project_mismatch() {
        let service = ShortcutReplyService::new(vec![
            shortcut("global", "全局", true, 20, Vec::new(), Vec::new()),
            shortcut(
                "codex",
                "Codex",
                true,
                10,
                vec![AgentKind::CodexCli],
                Vec::new(),
            ),
            shortcut(
                "other-project",
                "其他项目",
                true,
                5,
                Vec::new(),
                vec!["other".to_string()],
            ),
            shortcut("disabled", "禁用", false, 1, Vec::new(), Vec::new()),
        ]);
        let session_key = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("project"),
            ConversationId::new("conversation"),
        );

        let shortcuts = service.available_for_session(&session_key);

        assert_eq!(
            shortcuts
                .iter()
                .map(|shortcut| shortcut.id.value.as_str())
                .collect::<Vec<_>>(),
            vec!["codex", "global"]
        );
    }

    #[test]
    fn sorts_by_order_label_and_id() {
        let service = ShortcutReplyService::new(vec![
            shortcut("b", "同名", true, 1, Vec::new(), Vec::new()),
            shortcut("a", "同名", true, 1, Vec::new(), Vec::new()),
            shortcut("c", "靠后", true, 2, Vec::new(), Vec::new()),
        ]);
        let session_key = SessionKey::new(
            AgentKind::ClaudeCodeCli,
            ProjectId::new("project"),
            ConversationId::new("conversation"),
        );

        let shortcuts = service.available_for_session(&session_key);

        assert_eq!(
            shortcuts
                .iter()
                .map(|shortcut| shortcut.id.value.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
    }

    fn shortcut(
        id: &str,
        label: &str,
        enabled: bool,
        order: u32,
        agent_kinds: Vec<AgentKind>,
        project_ids: Vec<String>,
    ) -> ShortcutReply {
        ShortcutReply {
            id: ShortcutReplyId {
                value: id.to_string(),
            },
            label: label.to_string(),
            content: format!("{label} 内容"),
            enabled,
            order,
            agent_kinds,
            project_ids,
        }
    }
}
