//! Domain 到 UI view model 的纯转换。

use serde::{Deserialize, Serialize};

use crate::domain::agent_interaction::{AgentInteraction, InteractionChoice, InteractionId};
use crate::domain::agent_session::{AgentSession, SessionCapabilities, SessionKey, SessionStatus};
use crate::domain::session_state::SessionState;
use crate::domain::usage::{UnixMillis, UsageScope, UsageValue};

/// UI 动作。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiAction {
    /// 跳回 agent。
    Jump,
    /// 发送开放性回复。
    SendReply,
    /// 处理审批。
    ResolveApproval,
    /// 创建后续 turn。
    CreateFollowupTurn,
}

/// 文本截断策略。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TextDisplay {
    /// 截断后的展示文本。
    pub text: String,
    /// 当前 view model 可用的完整清洗文本。
    pub full_text: String,
    /// 是否已经截断。
    pub truncated: bool,
    /// 截断上限。
    pub max_chars: usize,
}

/// 用量展示 view model。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UsageValueViewModel {
    /// 展示标签。
    pub value_label: String,
    /// 已验证数值标签。
    pub amount_label: Option<String>,
    /// 可选单位。
    pub unit: Option<String>,
    /// 稳定来源键。
    pub source_key: Option<String>,
    /// 可选来源标签。
    pub source_label: Option<String>,
    /// 用量作用域。
    pub scope: Option<UsageScope>,
    /// 来源更新时间。
    pub updated_at: Option<UnixMillis>,
}

/// 会话列表项 view model。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionListItemViewModel {
    /// 会话唯一键。
    pub session_key: SessionKey,
    /// Agent 展示标签。
    pub agent_label: String,
    /// 项目展示标签。
    pub project_label: String,
    /// Thread 展示标签。
    pub thread_label: String,
    /// 对话展示标签。
    pub conversation_label: String,
    /// 状态展示标签。
    pub status_label: String,
    /// 状态类型。
    pub status_kind: SessionStatus,
    /// 摘要展示。
    pub summary: TextDisplay,
    /// 更新时间展示标签。
    pub updated_at_label: String,
    /// 当前 turn 开始时间。
    pub started_at: UnixMillis,
    /// 当前 turn 结束时间。
    pub completed_at: Option<UnixMillis>,
    /// 5 小时用量展示。
    pub usage_5h: UsageValueViewModel,
    /// 周用量展示。
    pub usage_weekly: UsageValueViewModel,
    /// 可执行动作。
    pub actions: Vec<UiAction>,
    /// 行内交互展示。
    pub inline_interaction: InlineInteractionViewModel,
}

/// 行内交互 view model。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InlineInteractionViewModel {
    /// 等待处理的交互摘要。
    pub summary: Option<String>,
    /// 等待处理的交互 ID。
    pub interaction_id: Option<InteractionId>,
    /// 等待处理的交互类型。
    pub kind: Option<PendingInteractionKind>,
    /// 是否可跳回。
    pub can_jump: bool,
    /// 是否可回复。
    pub can_send_reply: bool,
    /// 是否可审批。
    pub can_resolve_approval: bool,
    /// 是否可创建后续 turn。
    pub can_create_followup_turn: bool,
    /// 选项框状态。
    pub choice_box: ChoiceBoxViewModel,
}

/// 会话详情 view model。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionDetailViewModel {
    /// 详情头部。
    pub header: String,
    /// 身份摘要。
    pub identity: String,
    /// 用量摘要。
    pub usage: String,
    /// 活动摘要。
    pub summary: TextDisplay,
    /// 执行信息。
    pub execution_info: String,
    /// 等待处理的交互摘要。
    pub pending_interaction: Option<String>,
    /// 等待处理的交互 ID。
    pub pending_interaction_id: Option<InteractionId>,
    /// 等待处理的交互类型。
    pub pending_interaction_kind: Option<PendingInteractionKind>,
    /// 回复框状态。
    pub reply_box: ReplyBoxViewModel,
    /// 选项框状态。
    pub choice_box: ChoiceBoxViewModel,
    /// 工具栏动作。
    pub toolbar_actions: Vec<UiAction>,
}

/// 等待处理的交互类型 view model。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingInteractionKind {
    /// 审批请求。
    Approval,
    /// 选项请求。
    Choice,
    /// 文本回复请求。
    TextReply,
}

/// 回复框 view model。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReplyBoxViewModel {
    /// 是否可编辑。
    pub enabled: bool,
    /// 不可编辑原因。
    pub disabled_reason: Option<String>,
}

/// 选项项 view model。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InteractionChoiceViewModel {
    /// 选项稳定值。
    pub value: String,
    /// 展示标签。
    pub label: String,
    /// 可选悬停说明。
    pub tooltip: Option<String>,
}

/// 选项框 view model。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChoiceBoxViewModel {
    /// 是否可提交选项。
    pub enabled: bool,
    /// 是否允许多选。
    pub allows_multiple: bool,
    /// 可选项。
    pub choices: Vec<InteractionChoiceViewModel>,
    /// 不可提交原因。
    pub disabled_reason: Option<String>,
}

/// 将 session state 转为列表 view model。
pub fn session_list_view_models(state: &SessionState) -> Vec<SessionListItemViewModel> {
    state
        .sorted_session_keys()
        .into_iter()
        .filter_map(|key| state.sessions.get(&key))
        .map(session_list_item_view_model)
        .collect()
}

/// 将 session 转为列表项 view model。
pub fn session_list_item_view_model(session: &AgentSession) -> SessionListItemViewModel {
    SessionListItemViewModel {
        session_key: session.session_key.clone(),
        agent_label: session.session_key.agent_kind.label().to_string(),
        project_label: session.project_label.clone(),
        thread_label: thread_label(session),
        conversation_label: session.conversation_label.clone(),
        status_label: session.status.label().to_string(),
        status_kind: session.status,
        summary: text_display(session.summary.as_deref().unwrap_or(""), 96),
        updated_at_label: session.updated_at.value.to_string(),
        started_at: session.started_at,
        completed_at: session.completed_at,
        usage_5h: usage_value_view_model(&session.usage.usage_5h),
        usage_weekly: usage_value_view_model(&session.usage.usage_weekly),
        actions: actions_for_session(session),
        inline_interaction: inline_interaction_view_model(session),
    }
}

/// 将 session 转为详情 view model。
pub fn session_detail_view_model(session: &AgentSession) -> SessionDetailViewModel {
    let pending_interaction = session
        .pending_interaction
        .as_ref()
        .map(pending_interaction_label);
    let pending_interaction_id = session
        .pending_interaction
        .as_ref()
        .map(pending_interaction_id);
    let pending_interaction_kind = session
        .pending_interaction
        .as_ref()
        .map(pending_interaction_kind);
    let actions = actions_for_session(session);
    let reply_enabled = actions.contains(&UiAction::SendReply);
    let choice_box = choice_box_view_model(session, &actions);
    let thread_label = thread_label(session);

    SessionDetailViewModel {
        header: thread_label.clone(),
        identity: format!("{} / {}", session.project_label, thread_label),
        usage: format!(
            "5H {}，本周 {}",
            session.usage.usage_5h.display_label(),
            session.usage.usage_weekly.display_label()
        ),
        summary: text_display(session.summary.as_deref().unwrap_or(""), 240),
        execution_info: session.status.label().to_string(),
        pending_interaction,
        pending_interaction_id,
        pending_interaction_kind,
        reply_box: ReplyBoxViewModel {
            enabled: reply_enabled,
            disabled_reason: if reply_enabled {
                None
            } else {
                Some("当前会话不支持回复".to_string())
            },
        },
        choice_box,
        toolbar_actions: actions,
    }
}

/// 将 capability 映射为 UI 动作。
pub fn actions_from_capabilities(capabilities: &SessionCapabilities) -> Vec<UiAction> {
    let mut actions = Vec::new();

    if capabilities.can_jump {
        actions.push(UiAction::Jump);
    }
    if capabilities.can_send_reply {
        actions.push(UiAction::SendReply);
    }
    if capabilities.can_resolve_approval {
        actions.push(UiAction::ResolveApproval);
    }
    if capabilities.can_create_followup_turn {
        actions.push(UiAction::CreateFollowupTurn);
    }

    actions
}

/// 将 session 状态、pending 和 capability 映射为 UI 动作。
pub fn actions_for_session(session: &AgentSession) -> Vec<UiAction> {
    let mut actions = Vec::new();

    if session.capabilities.can_jump
        && session.jump_target.is_some()
        && session.status != SessionStatus::Detached
    {
        actions.push(UiAction::Jump);
    }

    if session.capabilities.can_send_reply && session.status == SessionStatus::WaitingForAnswer {
        if matches!(
            session.pending_interaction,
            Some(AgentInteraction::TextReply(_) | AgentInteraction::Choice(_))
        ) {
            actions.push(UiAction::SendReply);
        }
    }

    if session.capabilities.can_resolve_approval
        && session.status == SessionStatus::WaitingForApproval
        && matches!(
            session.pending_interaction,
            Some(AgentInteraction::Approval(_))
        )
    {
        actions.push(UiAction::ResolveApproval);
    }

    if session.capabilities.can_create_followup_turn
        && matches!(
            session.status,
            SessionStatus::Completed | SessionStatus::Failed
        )
    {
        actions.push(UiAction::CreateFollowupTurn);
    }

    actions
}

/// 创建 thread 展示标签。
fn thread_label(session: &AgentSession) -> String {
    session
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| "未命名".to_string())
}

/// 创建用量 view model。
fn usage_value_view_model(value: &UsageValue) -> UsageValueViewModel {
    UsageValueViewModel {
        value_label: value.display_label(),
        amount_label: value.amount_label(),
        unit: value.unit().map(ToString::to_string),
        source_key: value.source_key().map(ToString::to_string),
        source_label: value.source_label().map(ToString::to_string),
        scope: value.scope().cloned(),
        updated_at: value.updated_at(),
    }
}

/// 创建行内交互 view model。
fn inline_interaction_view_model(session: &AgentSession) -> InlineInteractionViewModel {
    let actions = actions_for_session(session);
    let choice_box = choice_box_view_model(session, &actions);

    InlineInteractionViewModel {
        summary: session
            .pending_interaction
            .as_ref()
            .map(pending_interaction_label),
        interaction_id: session
            .pending_interaction
            .as_ref()
            .map(pending_interaction_id),
        kind: session
            .pending_interaction
            .as_ref()
            .map(pending_interaction_kind),
        can_jump: actions.contains(&UiAction::Jump),
        can_send_reply: actions.contains(&UiAction::SendReply),
        can_resolve_approval: actions.contains(&UiAction::ResolveApproval),
        can_create_followup_turn: actions.contains(&UiAction::CreateFollowupTurn),
        choice_box,
    }
}

/// 创建截断文本展示。
pub fn text_display(value: &str, max_chars: usize) -> TextDisplay {
    let char_count = value.chars().count();

    if char_count <= max_chars {
        return TextDisplay {
            text: value.to_string(),
            full_text: value.to_string(),
            truncated: false,
            max_chars,
        };
    }

    TextDisplay {
        text: value.chars().take(max_chars).collect(),
        full_text: value.to_string(),
        truncated: true,
        max_chars,
    }
}

/// 返回 pending interaction 标签。
fn pending_interaction_label(interaction: &AgentInteraction) -> String {
    match interaction {
        AgentInteraction::Approval(interaction) => interaction.request_summary.clone(),
        AgentInteraction::Choice(interaction) => interaction.request_summary.clone(),
        AgentInteraction::TextReply(interaction) => interaction.request_summary.clone(),
    }
}

/// 返回 pending interaction ID。
fn pending_interaction_id(interaction: &AgentInteraction) -> InteractionId {
    match interaction {
        AgentInteraction::Approval(interaction) => interaction.interaction_id.clone(),
        AgentInteraction::Choice(interaction) => interaction.interaction_id.clone(),
        AgentInteraction::TextReply(interaction) => interaction.interaction_id.clone(),
    }
}

/// 返回 pending interaction 类型。
fn pending_interaction_kind(interaction: &AgentInteraction) -> PendingInteractionKind {
    match interaction {
        AgentInteraction::Approval(_) => PendingInteractionKind::Approval,
        AgentInteraction::Choice(_) => PendingInteractionKind::Choice,
        AgentInteraction::TextReply(_) => PendingInteractionKind::TextReply,
    }
}

/// 创建选项框 view model。
fn choice_box_view_model(session: &AgentSession, actions: &[UiAction]) -> ChoiceBoxViewModel {
    let Some(AgentInteraction::Choice(interaction)) = &session.pending_interaction else {
        return ChoiceBoxViewModel {
            enabled: false,
            allows_multiple: false,
            choices: Vec::new(),
            disabled_reason: Some("当前会话没有选项交互".to_string()),
        };
    };
    let enabled = actions.contains(&UiAction::SendReply);

    ChoiceBoxViewModel {
        enabled,
        allows_multiple: interaction.allows_multiple,
        choices: interaction.choices.iter().map(choice_view_model).collect(),
        disabled_reason: if enabled {
            None
        } else {
            Some("当前会话不支持选项回写".to_string())
        },
    }
}

/// 创建单个选项 view model。
fn choice_view_model(choice: &InteractionChoice) -> InteractionChoiceViewModel {
    InteractionChoiceViewModel {
        value: choice.value.clone(),
        label: choice.label.clone(),
        tooltip: choice.tooltip.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        actions_from_capabilities, session_detail_view_model, session_list_item_view_model,
        text_display, UiAction,
    };
    use crate::domain::agent_interaction::{
        AgentInteraction, ChoiceInteraction, ClipboardFallbackTarget, InteractionChoice,
        InteractionId, InteractionStatus, ReplyTarget, TextReplyInteraction,
    };
    use crate::domain::agent_session::{
        AgentKind, AgentSession, ConversationId, JumpTarget, ProjectId, SessionCapabilities,
        SessionKey, SessionStatus,
    };
    use crate::domain::usage::{
        UnixMillis, UsageAmount, UsageSnapshot, UsageValue, VerifiedUsageValue,
    };

    #[test]
    fn unavailable_usage_maps_to_placeholder() {
        let session = base_session(SessionCapabilities::none(), UsageSnapshot::unavailable());
        let view_model = session_list_item_view_model(&session);

        assert_eq!(view_model.usage_5h.value_label, "--");
        assert_eq!(view_model.usage_weekly.value_label, "--");
    }

    #[test]
    fn unsupported_capability_does_not_create_action() {
        let actions = actions_from_capabilities(&SessionCapabilities::none());

        assert_eq!(actions, Vec::<UiAction>::new());
    }

    #[test]
    fn supported_capabilities_create_actions() {
        let actions = actions_from_capabilities(&SessionCapabilities {
            can_jump: true,
            can_send_reply: true,
            can_resolve_approval: true,
            can_create_followup_turn: true,
        });

        assert_eq!(
            actions,
            vec![
                UiAction::Jump,
                UiAction::SendReply,
                UiAction::ResolveApproval,
                UiAction::CreateFollowupTurn,
            ]
        );
    }

    #[test]
    fn verified_usage_maps_number_and_source_label() {
        let usage = UsageSnapshot {
            usage_5h: UsageValue::Verified(VerifiedUsageValue {
                value: UsageAmount::new(64.0).expect("valid usage amount"),
                unit: Some("percent".to_string()),
                source_key: "codex-status".to_string(),
                source_label: "Codex /status".to_string(),
                scope: crate::domain::usage::UsageScope::AccountWindow,
                updated_at: None,
            }),
            usage_weekly: UsageValue::Unavailable,
        };
        let session = base_session(SessionCapabilities::none(), usage);
        let view_model = session_list_item_view_model(&session);

        assert_eq!(view_model.usage_5h.value_label, "64 percent");
        assert_eq!(
            view_model.usage_5h.source_label,
            Some("Codex /status".to_string())
        );
    }

    #[test]
    fn view_models_expose_full_thread_label_from_session_title() {
        let mut session = base_session(SessionCapabilities::none(), UsageSnapshot::unavailable());
        session.title = Some("一二三四五六七八九十十一".to_string());
        let list_view_model = session_list_item_view_model(&session);
        let detail_view_model = session_detail_view_model(&session);

        assert_eq!(list_view_model.thread_label, "一二三四五六七八九十十一");
        assert_eq!(detail_view_model.header, "一二三四五六七八九十十一");
        assert_eq!(
            detail_view_model.identity,
            "project / 一二三四五六七八九十十一"
        );
    }

    #[test]
    fn list_view_model_exposes_turn_timestamps() {
        let mut session = base_session(SessionCapabilities::none(), UsageSnapshot::unavailable());
        session.started_at = UnixMillis::new(10);
        session.completed_at = Some(UnixMillis::new(20));
        let list_view_model = session_list_item_view_model(&session);

        assert_eq!(list_view_model.started_at, UnixMillis::new(10));
        assert_eq!(list_view_model.completed_at, Some(UnixMillis::new(20)));
    }

    #[test]
    fn list_view_model_hides_conversation_id_when_thread_title_is_missing() {
        let session = base_session(SessionCapabilities::none(), UsageSnapshot::unavailable());
        let list_view_model = session_list_item_view_model(&session);
        let detail_view_model = session_detail_view_model(&session);

        assert_eq!(list_view_model.thread_label, "未命名");
        assert_eq!(detail_view_model.header, "未命名");
        assert_eq!(detail_view_model.identity, "project / 未命名");
    }

    #[test]
    fn list_view_model_filters_stale_reply_action_after_completion() {
        let mut session = base_session(
            SessionCapabilities {
                can_jump: true,
                can_send_reply: true,
                can_resolve_approval: true,
                can_create_followup_turn: true,
            },
            UsageSnapshot::unavailable(),
        );
        session.status = SessionStatus::Completed;
        session.pending_interaction = None;
        session.jump_target = Some(JumpTarget {
            label: "Codex".to_string(),
            location: "codex://threads/conversation".to_string(),
        });
        let view_model = session_list_item_view_model(&session);

        assert_eq!(
            view_model.actions,
            vec![UiAction::Jump, UiAction::CreateFollowupTurn]
        );
    }

    #[test]
    fn list_view_model_only_creates_jump_when_target_exists() {
        let mut session = base_session(
            SessionCapabilities {
                can_jump: true,
                can_send_reply: false,
                can_resolve_approval: false,
                can_create_followup_turn: false,
            },
            UsageSnapshot::unavailable(),
        );

        let without_target = session_list_item_view_model(&session);
        session.jump_target = Some(JumpTarget {
            label: "Codex".to_string(),
            location: "codex://threads/conversation".to_string(),
        });
        let with_target = session_list_item_view_model(&session);

        assert_eq!(without_target.actions, Vec::<UiAction>::new());
        assert_eq!(with_target.actions, vec![UiAction::Jump]);
        assert!(!without_target.inline_interaction.can_jump);
        assert!(with_target.inline_interaction.can_jump);
    }

    #[test]
    fn list_view_model_creates_send_reply_only_for_pending_text_reply() {
        let mut session = base_session(
            SessionCapabilities {
                can_jump: false,
                can_send_reply: true,
                can_resolve_approval: false,
                can_create_followup_turn: false,
            },
            UsageSnapshot::unavailable(),
        );
        session.status = SessionStatus::WaitingForAnswer;
        session.pending_interaction = Some(AgentInteraction::TextReply(TextReplyInteraction {
            interaction_id: InteractionId::new("reply"),
            session_key: session.session_key.clone(),
            created_at: UnixMillis::new(100),
            expires_at: None,
            reply_target: ReplyTarget::ClipboardOnly(ClipboardFallbackTarget {
                reason: "测试".to_string(),
            }),
            status: InteractionStatus::Pending,
            request_summary: "需要回复".to_string(),
            prompt: "请输入".to_string(),
        }));
        let view_model = session_list_item_view_model(&session);

        assert_eq!(view_model.actions, vec![UiAction::SendReply]);
    }

    #[test]
    fn list_view_model_creates_send_reply_for_pending_choice() {
        let mut session = base_session(
            SessionCapabilities {
                can_jump: false,
                can_send_reply: true,
                can_resolve_approval: false,
                can_create_followup_turn: false,
            },
            UsageSnapshot::unavailable(),
        );
        session.status = SessionStatus::WaitingForAnswer;
        session.pending_interaction = Some(AgentInteraction::Choice(choice_interaction(
            &session.session_key,
            false,
        )));
        let view_model = session_list_item_view_model(&session);

        assert_eq!(view_model.actions, vec![UiAction::SendReply]);
    }

    #[test]
    fn long_summary_has_truncation_policy() {
        let display = text_display("abcdefghijklmnopqrstuvwxyz", 8);

        assert_eq!(display.text, "abcdefgh");
        assert_eq!(display.full_text, "abcdefghijklmnopqrstuvwxyz");
        assert!(display.truncated);
        assert_eq!(display.max_chars, 8);
    }

    #[test]
    fn detail_reply_box_reflects_reply_capability() {
        let session = base_session(SessionCapabilities::none(), UsageSnapshot::unavailable());
        let view_model = session_detail_view_model(&session);

        assert!(!view_model.reply_box.enabled);
        assert_eq!(
            view_model.reply_box.disabled_reason,
            Some("当前会话不支持回复".to_string())
        );
    }

    #[test]
    fn detail_exposes_pending_interaction_id_and_kind() {
        let mut session = base_session(
            SessionCapabilities {
                can_jump: false,
                can_send_reply: true,
                can_resolve_approval: false,
                can_create_followup_turn: false,
            },
            UsageSnapshot::unavailable(),
        );
        session.status = SessionStatus::WaitingForAnswer;
        session.pending_interaction = Some(AgentInteraction::TextReply(TextReplyInteraction {
            interaction_id: InteractionId::new("reply"),
            session_key: session.session_key.clone(),
            created_at: UnixMillis::new(100),
            expires_at: None,
            reply_target: ReplyTarget::ClipboardOnly(ClipboardFallbackTarget {
                reason: "测试".to_string(),
            }),
            status: InteractionStatus::Pending,
            request_summary: "需要回复".to_string(),
            prompt: "请输入".to_string(),
        }));

        let view_model = session_detail_view_model(&session);

        assert_eq!(
            view_model.pending_interaction_id,
            Some(InteractionId::new("reply"))
        );
        assert_eq!(
            view_model.pending_interaction_kind,
            Some(super::PendingInteractionKind::TextReply)
        );
    }

    #[test]
    fn detail_exposes_choice_box_options() {
        let mut session = base_session(
            SessionCapabilities {
                can_jump: false,
                can_send_reply: true,
                can_resolve_approval: false,
                can_create_followup_turn: false,
            },
            UsageSnapshot::unavailable(),
        );
        session.status = SessionStatus::WaitingForAnswer;
        session.pending_interaction = Some(AgentInteraction::Choice(choice_interaction(
            &session.session_key,
            true,
        )));

        let view_model = session_detail_view_model(&session);

        assert!(view_model.choice_box.enabled);
        assert!(view_model.choice_box.allows_multiple);
        assert_eq!(view_model.choice_box.choices[0].value, "first");
        assert_eq!(
            view_model.choice_box.choices[0].tooltip,
            Some("第一个选项说明".to_string())
        );
        assert_eq!(
            view_model.pending_interaction_kind,
            Some(super::PendingInteractionKind::Choice)
        );
    }

    fn base_session(capabilities: SessionCapabilities, usage: UsageSnapshot) -> AgentSession {
        let mut session = AgentSession::new_running(
            SessionKey::new(
                AgentKind::CodexCli,
                ProjectId::new("project"),
                ConversationId::new("conversation"),
            ),
            "project",
            "conversation",
            UnixMillis::new(100),
        );
        session.capabilities = capabilities;
        session.usage = usage;
        session.summary = Some("摘要".to_string());
        session
    }

    fn choice_interaction(session_key: &SessionKey, allows_multiple: bool) -> ChoiceInteraction {
        ChoiceInteraction {
            interaction_id: InteractionId::new("choice"),
            session_key: session_key.clone(),
            created_at: UnixMillis::new(100),
            expires_at: None,
            reply_target: ReplyTarget::ClipboardOnly(ClipboardFallbackTarget {
                reason: "测试".to_string(),
            }),
            status: InteractionStatus::Pending,
            request_summary: "请选择".to_string(),
            choices: vec![
                InteractionChoice {
                    value: "first".to_string(),
                    label: "第一个选项".to_string(),
                    tooltip: Some("第一个选项说明".to_string()),
                },
                InteractionChoice {
                    value: "second".to_string(),
                    label: "第二个选项".to_string(),
                    tooltip: None,
                },
            ],
            allows_multiple,
        }
    }
}
