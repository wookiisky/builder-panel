import { invoke } from "@tauri-apps/api/core";

import type {
  ApprovalDecision,
  InteractionId,
  SessionDetailViewModel,
  SessionKey,
  SessionListItemViewModel,
  TimelinePage,
  TimelineQuery,
} from "./mockPanelContract";

/// Codex CLI 审批提交请求。
export interface ResolveCodexApprovalRequest {
  /// 所属会话。
  readonly session_key: SessionKey;
  /// 所属交互。
  readonly interaction_id: InteractionId;
  /// 审批决策。
  readonly decision: ApprovalDecision;
}

/// 读取 Codex CLI session 列表。
export const fetchCodexCliSessions = async (): Promise<
  readonly SessionListItemViewModel[]
> => {
  try {
    return await invoke<SessionListItemViewModel[]>("get_codex_cli_sessions");
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "读取 Codex CLI session 失败");
    }
    return [];
  }
};

/// 读取 Codex CLI session 详情。
export const fetchCodexCliSessionDetail = async (
  sessionKey: SessionKey,
): Promise<SessionDetailViewModel | null> => {
  try {
    return await invoke<SessionDetailViewModel | null>(
      "get_codex_cli_session_detail",
      { sessionKey },
    );
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "读取 Codex CLI session 详情失败");
    }
    return null;
  }
};

/// 提交 Codex CLI 审批。
export const submitCodexCliApproval = async (
  request: ResolveCodexApprovalRequest,
): Promise<void> => {
  try {
    await invoke("resolve_codex_cli_approval", { request });
  } catch (error) {
    throw errorWithCause(error, "Codex CLI 审批回写失败");
  }
};

/// 查询 Codex CLI 时间线。
export const fetchCodexCliTimeline = async (
  query: TimelineQuery,
): Promise<TimelinePage> => {
  try {
    return await invoke<TimelinePage>("query_codex_cli_timeline", { query });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "读取 Codex CLI 时间线失败");
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

/// 释放 Codex CLI 时间线大文本缓存。
export const releaseCodexCliTimelineCache = async (
  sessionKey: SessionKey,
): Promise<void> => {
  try {
    await invoke<number>("release_codex_cli_timeline_cache", { sessionKey });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "释放 Codex CLI 时间线缓存失败");
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
