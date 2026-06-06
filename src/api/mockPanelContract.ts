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
  | "create_followup_turn"
  | "view_process_timeline";

/// Pending 交互类型。
export type PendingInteractionKind = "approval" | "choice" | "text_reply";

/// 审批决策。
export type ApprovalDecision = "allow" | "allow_and_remember" | "deny";

/// 时间线事件类型。
export type TimelineEventKind =
  | "activity"
  | "tool"
  | "approval"
  | "reply"
  | "system";

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
  /// 是否已经截断。
  readonly truncated: boolean;
  /// 截断上限。
  readonly max_chars: number;
}

/// 用量展示。
export interface UsageValueViewModel {
  /// 展示标签。
  readonly value_label: string;
  /// 可选来源标签。
  readonly source_label: string | null;
}

/// 会话列表项。
export interface SessionListItemViewModel {
  /// 会话唯一键。
  readonly session_key: SessionKey;
  /// Agent 展示标签。
  readonly agent_label: string;
  /// 项目展示标签。
  readonly project_label: string;
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
  /// 5 小时用量展示。
  readonly usage_5h: UsageValueViewModel;
  /// 周用量展示。
  readonly usage_weekly: UsageValueViewModel;
  /// 可执行动作。
  readonly actions: readonly UiAction[];
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

/// 审批提交请求。
export interface ResolveApprovalRequest {
  /// 所属会话。
  readonly session_key: SessionKey;
  /// 所属交互。
  readonly interaction_id: InteractionId;
  /// 审批决策。
  readonly decision: ApprovalDecision;
  /// 是否注入一次 mock 回写失败。
  readonly inject_failure: boolean;
}

/// 文本回复请求。
export interface SendReplyRequest {
  /// 所属会话。
  readonly session_key: SessionKey;
  /// 所属交互。
  readonly interaction_id: InteractionId;
  /// 文本内容。
  readonly content: string;
  /// 是否注入一次 mock 回写失败。
  readonly inject_failure: boolean;
}

/// 选项提交请求。
export interface SubmitChoiceRequest {
  /// 所属会话。
  readonly session_key: SessionKey;
  /// 所属交互。
  readonly interaction_id: InteractionId;
  /// 用户选择的选项值。
  readonly selected_values: readonly string[];
  /// 是否注入一次 mock 回写失败。
  readonly inject_failure: boolean;
}

/// 时间线条目。
export interface ProcessTimelineItem {
  /// 条目唯一标识。
  readonly item_id: string;
  /// 所属会话。
  readonly session_key: SessionKey;
  /// 事件类型。
  readonly kind: TimelineEventKind;
  /// 条目标题。
  readonly title: string;
  /// 已清洗正文。
  readonly body: string;
  /// 创建时间。
  readonly created_at: { readonly value: number };
}

/// 时间线查询请求。
export interface TimelineQuery {
  /// 所属会话。
  readonly session_key: SessionKey;
  /// 页码，从 0 开始。
  readonly page: number;
  /// 每页条目数。
  readonly page_size: number;
  /// 搜索关键词。
  readonly search: string | null;
  /// 类型筛选。
  readonly kind: TimelineEventKind | null;
}

/// 时间线分页结果。
export interface TimelinePage {
  /// 当前页条目。
  readonly items: readonly ProcessTimelineItem[];
  /// 页码，从 0 开始。
  readonly page: number;
  /// 每页条目数。
  readonly page_size: number;
  /// 过滤后的总条目数。
  readonly total: number;
  /// 是否还有下一页。
  readonly has_next: boolean;
  /// 当前启用过滤器数量。
  readonly filter_count: number;
}
