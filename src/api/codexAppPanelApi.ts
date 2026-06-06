import { invoke } from "@tauri-apps/api/core";

import type {
  ApprovalDecision,
  InteractionId,
  SendReplyRequest,
  SessionDetailViewModel,
  SessionKey,
  SessionListItemViewModel,
  SubmitChoiceRequest,
  TimelinePage,
  TimelineQuery,
} from "./mockPanelContract";

/// Codex APP 审批提交请求。
export interface ResolveCodexAppApprovalRequest {
  /// 所属会话。
  readonly session_key: SessionKey;
  /// 所属交互。
  readonly interaction_id: InteractionId;
  /// 审批决策。
  readonly decision: ApprovalDecision;
}

/// Codex APP follow-up turn 请求。
export interface CodexAppFollowupRequest {
  /// 所属会话。
  readonly session_key: SessionKey;
  /// 用户输入。
  readonly prompt: string;
}

/// 读取 Codex APP session 列表。
export const fetchCodexAppSessions = async (): Promise<
  readonly SessionListItemViewModel[]
> => {
  try {
    return await invoke<SessionListItemViewModel[]>("get_codex_app_sessions");
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "读取 Codex APP session 失败");
    }
    return [];
  }
};

/// 读取 Codex APP session 详情。
export const fetchCodexAppSessionDetail = async (
  sessionKey: SessionKey,
): Promise<SessionDetailViewModel | null> => {
  try {
    return await invoke<SessionDetailViewModel | null>(
      "get_codex_app_session_detail",
      { sessionKey },
    );
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "读取 Codex APP session 详情失败");
    }
    return null;
  }
};

/// 提交 Codex APP 审批。
export const submitCodexAppApproval = async (
  request: ResolveCodexAppApprovalRequest,
): Promise<void> => {
  try {
    await invoke("resolve_codex_app_approval", { request });
  } catch (error) {
    throw errorWithCause(error, "Codex APP 审批回写失败");
  }
};

/// 提交 Codex APP 文本回复。
export const submitCodexAppReply = async (
  request: SendReplyRequest,
): Promise<void> => {
  try {
    await invoke("send_codex_app_reply", { request });
  } catch (error) {
    throw errorWithCause(error, "Codex APP 回复回写失败");
  }
};

/// 提交 Codex APP 选项回复。
export const submitCodexAppChoice = async (
  request: SubmitChoiceRequest,
): Promise<void> => {
  try {
    await invoke("submit_codex_app_choice", { request });
  } catch (error) {
    throw errorWithCause(error, "Codex APP 选项回写失败");
  }
};

/// 创建 Codex APP follow-up turn。
export const createCodexAppFollowupTurn = async (
  request: CodexAppFollowupRequest,
): Promise<void> => {
  try {
    await invoke("create_codex_app_followup_turn", { request });
  } catch (error) {
    throw errorWithCause(error, "Codex APP follow-up 创建失败");
  }
};

/// 查询 Codex APP 时间线。
export const fetchCodexAppTimeline = async (
  query: TimelineQuery,
): Promise<TimelinePage> => {
  try {
    return await invoke<TimelinePage>("query_codex_app_timeline", { query });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "读取 Codex APP 时间线失败");
    }
    return {
      items: [],
      page: query.page,
      page_size: query.page_size,
      total: 0,
      has_next: false,
      filter_count:
        (query.search === null || query.search.trim().length === 0 ? 0 : 1) +
        (query.kind === null ? 0 : 1),
    };
  }
};

/// 释放 Codex APP 时间线大文本缓存。
export const releaseCodexAppTimelineCache = async (
  sessionKey: SessionKey,
): Promise<void> => {
  try {
    await invoke<number>("release_codex_app_timeline_cache", { sessionKey });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "释放 Codex APP 时间线缓存失败");
    }
  }
};

/// 判断当前是否运行在 Tauri 环境。
const isTauriRuntime = (): boolean => {
  return "__TAURI_INTERNALS__" in window;
};

/// 将未知异常收敛成 Error。
const errorWithCause = (error: unknown, fallbackMessage: string): Error => {
  if (error instanceof Error) {
    return error;
  }

  if (typeof error === "string" && error.trim().length > 0) {
    return new Error(error);
  }

  return new Error(fallbackMessage);
};
