/// Agent 来源类型。
export type AgentKind =
  | "codex_app"
  | "codex_cli"
  | "claude_code_app"
  | "claude_code_cli";

/// Session 运行状态。
export type SessionStatus =
  | "running"
  | "waiting_for_approval"
  | "waiting_for_answer"
  | "completed"
  | "failed"
  | "detached";

/// UI 动作。
export type UiAction =
  | "jump"
  | "send_reply"
  | "resolve_approval"
  | "create_followup_turn";

/// Pending 交互类型。
export type PendingInteractionKind = "approval" | "choice" | "text_reply";

/// 用量作用域。
export type UsageScope = "session" | "account_window";

/// 审批决策。
export type ApprovalDecision = "allow" | "allow_and_remember" | "deny";

/// 项目稳定标识。
export interface ProjectId {
  /// adapter 清洗后的项目 ID。
  readonly value: string;
}

/// 对话稳定标识。
export interface ConversationId {
  /// adapter 清洗后的对话 ID。
  readonly value: string;
}

/// 会话唯一键。
export interface SessionKey {
  /// Agent 来源类型。
  readonly agent_kind: AgentKind;
  /// 项目稳定标识。
  readonly project_id: ProjectId;
  /// 对话稳定标识。
  readonly conversation_id: ConversationId;
}

/// 交互唯一标识。
export interface InteractionId {
  /// adapter 清洗后的交互 ID。
  readonly value: string;
}

/// 文本截断展示。
export interface TextDisplay {
  /// 截断后的展示文本。
  readonly text: string;
  /// 当前 view model 可用的完整清洗文本。
  readonly full_text: string;
  /// 是否已经截断。
  readonly truncated: boolean;
  /// 截断上限。
  readonly max_chars: number;
}

/// 用量展示。
export interface UsageValueViewModel {
  /// 展示标签。
  readonly value_label: string;
  /// 已验证数值标签。
  readonly amount_label: string | null;
  /// 可选单位。
  readonly unit: string | null;
  /// 稳定来源键。
  readonly source_key: string | null;
  /// 可选来源标签。
  readonly source_label: string | null;
  /// 用量作用域。
  readonly scope: UsageScope | null;
  /// 来源更新时间。
  readonly updated_at: { readonly value: number } | null;
}

/// 会话列表项。
export interface SessionListItemViewModel {
  /// 会话唯一键。
  readonly session_key: SessionKey;
  /// Agent 展示标签。
  readonly agent_label: string;
  /// 项目展示标签。
  readonly project_label: string;
  /// Thread 展示标签。
  readonly thread_label: string;
  /// 对话展示标签。
  readonly conversation_label: string;
  /// 状态展示标签。
  readonly status_label: string;
  /// 状态类型。
  readonly status_kind: SessionStatus;
  /// 摘要展示。
  readonly summary: TextDisplay;
  /// 更新时间展示标签。
  readonly updated_at_label: string;
  /// 当前 turn 开始时间。
  readonly started_at: { readonly value: number };
  /// 当前 turn 结束时间。
  readonly completed_at: { readonly value: number } | null;
  /// 5 小时用量展示。
  readonly usage_5h: UsageValueViewModel;
  /// 周用量展示。
  readonly usage_weekly: UsageValueViewModel;
  /// 可执行动作。
  readonly actions: readonly UiAction[];
  /// 行内交互展示。
  readonly inline_interaction: InlineInteractionViewModel;
}

/// 行内交互展示。
export interface InlineInteractionViewModel {
  /// 等待处理的交互摘要。
  readonly summary: string | null;
  /// 等待处理的交互 ID。
  readonly interaction_id: InteractionId | null;
  /// 等待处理的交互类型。
  readonly kind: PendingInteractionKind | null;
  /// 是否可跳回。
  readonly can_jump: boolean;
  /// 是否可回复。
  readonly can_send_reply: boolean;
  /// 是否可审批。
  readonly can_resolve_approval: boolean;
  /// 是否可创建后续 turn。
  readonly can_create_followup_turn: boolean;
  /// 选项框状态。
  readonly choice_box: ChoiceBoxViewModel;
}

/// 回复框状态。
export interface ReplyBoxViewModel {
  /// 是否可编辑。
  readonly enabled: boolean;
  /// 不可编辑原因。
  readonly disabled_reason: string | null;
}

/// 选项项。
export interface InteractionChoiceViewModel {
  /// 选项稳定值。
  readonly value: string;
  /// 展示标签。
  readonly label: string;
  /// 可选悬停说明。
  readonly tooltip: string | null;
}

/// 选项框状态。
export interface ChoiceBoxViewModel {
  /// 是否可提交选项。
  readonly enabled: boolean;
  /// 是否允许多选。
  readonly allows_multiple: boolean;
  /// 可选项。
  readonly choices: readonly InteractionChoiceViewModel[];
  /// 不可提交原因。
  readonly disabled_reason: string | null;
}

/// 会话详情。
export interface SessionDetailViewModel {
  /// 详情头部。
  readonly header: string;
  /// 身份摘要。
  readonly identity: string;
  /// 用量摘要。
  readonly usage: string;
  /// 活动摘要。
  readonly summary: TextDisplay;
  /// 执行信息。
  readonly execution_info: string;
  /// 等待处理的交互摘要。
  readonly pending_interaction: string | null;
  /// 等待处理的交互 ID。
  readonly pending_interaction_id: InteractionId | null;
  /// 等待处理的交互类型。
  readonly pending_interaction_kind: PendingInteractionKind | null;
  /// 回复框状态。
  readonly reply_box: ReplyBoxViewModel;
  /// 选项框状态。
  readonly choice_box: ChoiceBoxViewModel;
  /// 工具栏动作。
  readonly toolbar_actions: readonly UiAction[];
}
