import { invoke } from "@tauri-apps/api/core";

import type {
  ProcessTimelineItem,
  ResolveApprovalRequest,
  SendReplyRequest,
  SessionDetailViewModel,
  SessionKey,
  SessionListItemViewModel,
  SubmitChoiceRequest,
  TimelineEventKind,
  TimelinePage,
  TimelineQuery,
} from "./mockPanelContract";

/// 读取 mock session 列表。
export const fetchMockSessions = async (): Promise<
  readonly SessionListItemViewModel[]
> => {
  try {
    return await invoke<SessionListItemViewModel[]>("get_mock_sessions");
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "读取 mock session 失败");
    }
    return fallbackSessions;
  }
};

/// 读取 mock session 详情。
export const fetchMockSessionDetail = async (
  sessionKey: SessionKey,
): Promise<SessionDetailViewModel | null> => {
  try {
    return await invoke<SessionDetailViewModel | null>(
      "get_mock_session_detail",
      { sessionKey },
    );
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "读取 mock session 详情失败");
    }
    return fallbackDetails.get(sessionKeyToFallbackId(sessionKey)) ?? null;
  }
};

/// 提交 mock 审批。
export const submitMockApproval = async (
  request: ResolveApprovalRequest,
): Promise<void> => {
  try {
    await invoke("resolve_mock_approval", { request });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "mock agent 回写失败");
    }
    fallbackResolveApproval(request);
    if (request.inject_failure) {
      throw errorWithCause(error, "mock agent 回写失败");
    }
  }
};

/// 提交 mock 选项。
export const submitMockChoice = async (
  request: SubmitChoiceRequest,
): Promise<void> => {
  try {
    await invoke("submit_mock_choice", { request });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "mock agent 选项回写失败");
    }
    fallbackSubmitChoice(request);
    if (request.inject_failure) {
      throw errorWithCause(error, "mock agent 选项回写失败");
    }
  }
};

/// 发送 mock 文本回复。
export const submitMockReply = async (
  request: SendReplyRequest,
): Promise<void> => {
  try {
    await invoke("send_mock_reply", { request });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "mock agent 回写失败");
    }
    fallbackSendReply(request);
    if (request.inject_failure) {
      throw errorWithCause(error, "mock agent 回写失败");
    }
  }
};

/// 查询 mock 时间线。
export const fetchMockTimeline = async (
  query: TimelineQuery,
): Promise<TimelinePage> => {
  try {
    return await invoke<TimelinePage>("query_mock_timeline", { query });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "读取 mock 时间线失败");
    }
    return fallbackQueryTimeline(query);
  }
};

/// 释放 mock 时间线大文本缓存。
export const releaseMockTimelineCache = async (
  sessionKey: SessionKey,
): Promise<void> => {
  try {
    await invoke<number>("release_mock_timeline_cache", { sessionKey });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "释放 mock 时间线缓存失败");
    }
  }
};

/// 生成 fallback session key。
const key = (projectId: string, conversationId: string): SessionKey => ({
  agent_kind: "codex_cli",
  project_id: { value: projectId },
  conversation_id: { value: conversationId },
});

/// 审批会话 key。
const approvalKey = key("mock-project-alpha", "approval-turn");

/// 回复会话 key。
const replyKey = key("mock-project-beta", "reply-turn");

/// 选项会话 key。
const choiceKey = key("mock-project-beta", "choice-turn");

/// 完成会话 key。
const completedKey = key("mock-project-alpha", "completed-turn");

/// 失败会话 key。
const failedKey = key("mock-project-gamma", "failed-turn");

/// fallback session 列表。
let fallbackSessions: SessionListItemViewModel[] = [
  sessionItem(
    approvalKey,
    "Mock Alpha",
    "审批闭环",
    "等待审批",
    "waiting_for_approval",
    "准备修改配置文件，需要用户审批",
    "41 percent",
    ["jump", "resolve_approval", "view_process_timeline"],
  ),
  sessionItem(
    replyKey,
    "Mock Beta",
    "回复闭环",
    "等待回复",
    "waiting_for_answer",
    "等待用户补充执行边界",
    "--",
    ["jump", "send_reply", "view_process_timeline"],
  ),
  sessionItem(
    choiceKey,
    "Mock Beta",
    "选项闭环",
    "等待回复",
    "waiting_for_answer",
    "等待用户选择一个执行方案",
    "--",
    ["jump", "send_reply", "view_process_timeline"],
  ),
  sessionItem(
    completedKey,
    "Mock Alpha",
    "完成闭环",
    "已完成",
    "completed",
    "mock turn 已完成",
    "22 percent",
    ["jump", "create_followup_turn", "view_process_timeline"],
  ),
  sessionItem(
    failedKey,
    "Mock Gamma",
    "失败闭环",
    "失败",
    "failed",
    "mock agent 模拟失败",
    "--",
    [],
  ),
];

/// fallback session 详情。
const fallbackDetails = new Map<string, SessionDetailViewModel>([
  [
    sessionKeyToFallbackId(approvalKey),
    detailItem(
      approvalKey,
      "检查文件写入权限",
      "Mock Alpha / 审批闭环",
      "允许 mock agent 写入本地配置样例",
      "approval-1",
      "approval",
      ["jump", "resolve_approval", "view_process_timeline"],
      false,
    ),
  ],
  [
    sessionKeyToFallbackId(replyKey),
    detailItem(
      replyKey,
      "补充需求细节",
      "Mock Beta / 回复闭环",
      "请输入阶段 3 的验收补充说明",
      "reply-1",
      "text_reply",
      ["jump", "send_reply", "view_process_timeline"],
      true,
    ),
  ],
  [
    sessionKeyToFallbackId(choiceKey),
    detailItem(
      choiceKey,
      "选择执行方案",
      "Mock Beta / 选项闭环",
      "请选择阶段 5 的执行方案",
      "choice-1",
      "choice",
      ["jump", "send_reply", "view_process_timeline"],
      false,
      {
        enabled: true,
        allows_multiple: false,
        choices: [
          {
            value: "plan-a",
            label: "先完成 mock 闭环",
            tooltip: "优先验证前端审批、回复和选项闭环",
          },
          {
            value: "plan-b",
            label: "先补真实终端 adapter",
            tooltip: "优先推进真实终端跳回能力",
          },
        ],
        disabled_reason: null,
      },
    ),
  ],
  [
    sessionKeyToFallbackId(completedKey),
    detailItem(
      completedKey,
      "完成 mock 用量同步",
      "Mock Alpha / 完成闭环",
      null,
      null,
      null,
      ["jump", "create_followup_turn", "view_process_timeline"],
      false,
    ),
  ],
]);

/// fallback 时间线。
const fallbackTimelineItems: readonly ProcessTimelineItem[] = [
  timelineItem(
    approvalKey,
    "a-1",
    "activity",
    "读取任务",
    "mock agent 读取阶段 3 任务",
    2001,
  ),
  timelineItem(
    approvalKey,
    "a-2",
    "tool",
    "准备写入",
    "检测到需要写入配置样例",
    2002,
  ),
  timelineItem(
    approvalKey,
    "a-3",
    "approval",
    "等待审批",
    "等待用户允许或拒绝",
    2003,
  ),
  timelineItem(
    replyKey,
    "r-1",
    "activity",
    "分析输入",
    "mock agent 等待补充说明",
    2004,
  ),
  timelineItem(
    replyKey,
    "r-2",
    "reply",
    "请求回复",
    "用户需要输入单行或多行回复",
    2005,
  ),
  timelineItem(
    choiceKey,
    "ch-1",
    "reply",
    "请求选择",
    "mock agent 等待用户选择执行方案",
    2006,
  ),
  timelineItem(
    completedKey,
    "c-1",
    "system",
    "同步用量",
    "用量数据已经清洗为可展示数字",
    2007,
  ),
  timelineItem(
    completedKey,
    "c-2",
    "activity",
    "完成 turn",
    "mock turn 已完成",
    2008,
  ),
];

/// 创建 fallback session 列表项。
function sessionItem(
  sessionKey: SessionKey,
  projectLabel: string,
  conversationLabel: string,
  statusLabel: string,
  statusKind: SessionListItemViewModel["status_kind"],
  summary: string,
  usage5h: string,
  actions: SessionListItemViewModel["actions"],
): SessionListItemViewModel {
  return {
    session_key: sessionKey,
    agent_label: "Codex CLI",
    project_label: projectLabel,
    conversation_label: conversationLabel,
    status_label: statusLabel,
    status_kind: statusKind,
    summary: { text: summary, truncated: false, max_chars: 96 },
    updated_at_label: "1000",
    usage_5h: usageValue(usage5h, "mock-status-5h"),
    usage_weekly: usageValue(
      usage5h === "--" ? "--" : "64 percent",
      "mock-status-weekly",
    ),
    actions,
    inline_interaction: inlineInteraction(
      sessionKey,
      statusKind,
      summary,
      actions,
    ),
  };
}

/// 创建 fallback 用量值。
function usageValue(
  label: string,
  sourceKey: string,
): SessionListItemViewModel["usage_5h"] {
  if (label === "--") {
    return {
      value_label: "--",
      amount_label: null,
      unit: null,
      source_key: null,
      source_label: null,
      scope: null,
      updated_at: null,
    };
  }

  const [amountLabel, unit] = label.split(" ");
  return {
    value_label: label,
    amount_label: amountLabel,
    unit: unit ?? null,
    source_key: sourceKey,
    source_label: "Mock /status",
    scope: "session",
    updated_at: { value: 1000 },
  };
}

/// 创建 fallback 行内交互。
function inlineInteraction(
  sessionKey: SessionKey,
  statusKind: SessionListItemViewModel["status_kind"],
  summary: string,
  actions: SessionListItemViewModel["actions"],
): SessionListItemViewModel["inline_interaction"] {
  const interactionId =
    statusKind === "waiting_for_approval"
      ? { value: "approval-1" }
      : statusKind === "waiting_for_answer" &&
          sessionKey.conversation_id.value === "reply-turn"
        ? { value: "reply-1" }
        : statusKind === "waiting_for_answer" &&
            sessionKey.conversation_id.value === "choice-turn"
          ? { value: "choice-1" }
          : null;
  const kind =
    interactionId?.value === "approval-1"
      ? "approval"
      : interactionId?.value === "reply-1"
        ? "text_reply"
        : interactionId?.value === "choice-1"
          ? "choice"
          : null;

  return {
    summary: interactionId === null ? null : summary,
    interaction_id: interactionId,
    kind,
    can_jump: actions.includes("jump"),
    can_send_reply: actions.includes("send_reply"),
    can_resolve_approval: actions.includes("resolve_approval"),
    can_create_followup_turn: actions.includes("create_followup_turn"),
    can_view_process_timeline: actions.includes("view_process_timeline"),
    choice_box:
      kind === "choice"
        ? {
            enabled: true,
            allows_multiple: false,
            choices: [
              {
                value: "plan-a",
                label: "先完成 mock 闭环",
                tooltip: "优先验证前端审批、回复和选项闭环",
              },
              {
                value: "plan-b",
                label: "先补真实终端 adapter",
                tooltip: "优先推进真实终端跳回能力",
              },
            ],
            disabled_reason: null,
          }
        : {
            enabled: false,
            allows_multiple: false,
            choices: [],
            disabled_reason: "当前会话没有选项交互",
          },
  };
}

/// 创建 fallback session 详情。
function detailItem(
  sessionKey: SessionKey,
  header: string,
  identity: string,
  pendingInteraction: string | null,
  interactionId: string | null,
  kind: SessionDetailViewModel["pending_interaction_kind"],
  actions: SessionDetailViewModel["toolbar_actions"],
  replyEnabled: boolean,
  choiceBox: SessionDetailViewModel["choice_box"] | null = null,
): SessionDetailViewModel {
  return {
    header,
    identity,
    usage: "5H --，本周 --",
    summary: {
      text: pendingInteraction ?? "mock turn 已完成",
      truncated: false,
      max_chars: 240,
    },
    execution_info: pendingInteraction === null ? "已完成" : "等待用户操作",
    pending_interaction: pendingInteraction,
    pending_interaction_id:
      interactionId === null ? null : { value: interactionId },
    pending_interaction_kind: kind,
    reply_box: {
      enabled: replyEnabled,
      disabled_reason: replyEnabled ? null : "当前会话不支持回复",
    },
    choice_box: choiceBox ?? {
      enabled: false,
      allows_multiple: false,
      choices: [],
      disabled_reason: "当前会话没有选项交互",
    },
    toolbar_actions: actions,
  };
}

/// 创建 fallback 时间线条目。
function timelineItem(
  sessionKey: SessionKey,
  itemId: string,
  kind: TimelineEventKind,
  title: string,
  body: string,
  createdAt: number,
): ProcessTimelineItem {
  return {
    item_id: itemId,
    session_key: sessionKey,
    kind,
    title,
    body,
    created_at: { value: createdAt },
  };
}

/// fallback 审批提交。
function fallbackResolveApproval(request: ResolveApprovalRequest): void {
  if (request.inject_failure) {
    return;
  }
  completeFallbackSession(request.session_key, "审批已处理");
}

/// fallback 文本回复提交。
function fallbackSendReply(request: SendReplyRequest): void {
  if (request.inject_failure) {
    return;
  }
  completeFallbackSession(request.session_key, "回复已发送");
}

/// fallback 选项提交。
function fallbackSubmitChoice(request: SubmitChoiceRequest): void {
  if (request.inject_failure) {
    return;
  }
  completeFallbackSession(request.session_key, "选项已提交");
}

/// fallback 完成指定 session。
function completeFallbackSession(
  sessionKey: SessionKey,
  summary: string,
): void {
  const id = sessionKeyToFallbackId(sessionKey);
  fallbackSessions = fallbackSessions.map((session) =>
    sessionKeyToFallbackId(session.session_key) === id
      ? {
          ...session,
          status_label: "已完成",
          status_kind: "completed",
          summary: { text: summary, truncated: false, max_chars: 96 },
          actions: session.actions.filter(
            (action) =>
              action !== "resolve_approval" && action !== "send_reply",
          ),
        }
      : session,
  );
  const detail = fallbackDetails.get(id);
  if (detail !== undefined) {
    fallbackDetails.set(id, {
      ...detail,
      execution_info: "已完成",
      pending_interaction: null,
      pending_interaction_id: null,
      pending_interaction_kind: null,
      reply_box: {
        enabled: false,
        disabled_reason: "当前会话不支持回复",
      },
      summary: { text: summary, truncated: false, max_chars: 240 },
      toolbar_actions: detail.toolbar_actions.filter(
        (action) => action !== "resolve_approval" && action !== "send_reply",
      ),
    });
  }
}

/// fallback 查询时间线。
function fallbackQueryTimeline(query: TimelineQuery): TimelinePage {
  const search = query.search?.trim().toLowerCase();
  const filtered = fallbackTimelineItems.filter((item) => {
    if (
      sessionKeyToFallbackId(item.session_key) !==
      sessionKeyToFallbackId(query.session_key)
    ) {
      return false;
    }
    if (query.kind !== null && item.kind !== query.kind) {
      return false;
    }
    if (search !== undefined && search.length > 0) {
      const haystack = `${item.title}\n${item.body}`.toLowerCase();
      return haystack.includes(search);
    }
    return true;
  });
  const pageSize = Math.max(1, Math.min(query.page_size, 50));
  const start = query.page * pageSize;
  const items = filtered.slice(start, start + pageSize);

  return {
    items,
    page: query.page,
    page_size: pageSize,
    total: filtered.length,
    has_next: start + items.length < filtered.length,
    filter_count:
      (search === undefined || search.length === 0 ? 0 : 1) +
      (query.kind === null ? 0 : 1),
  };
}

/// 创建 fallback session ID。
function sessionKeyToFallbackId(sessionKey: SessionKey): string {
  return `${sessionKey.agent_kind}::${sessionKey.project_id.value}::${sessionKey.conversation_id.value}`;
}

/// 归一错误消息。
function errorMessage(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message.length > 0) {
    return error.message;
  }
  if (typeof error === "string" && error.length > 0) {
    return error;
  }

  return fallback;
}

/// 创建保留 cause 的错误。
function errorWithCause(error: unknown, fallback: string): Error {
  return new Error(errorMessage(error, fallback), { cause: error });
}

/// 判断当前是否运行在 Tauri 环境。
function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}
