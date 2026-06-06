import { invoke } from "@tauri-apps/api/core";

import type { SessionKey } from "./mockPanelContract";

/// 前端运行时来源。
export type SessionJumpRuntimeSource = "mock" | "codex_cli" | "codex_app";

/// 跳回 session 请求。
export interface JumpToSessionRequest {
  /// 运行时来源。
  readonly runtime_source: SessionJumpRuntimeSource;
  /// 会话唯一键。
  readonly session_key: SessionKey;
}

/// 跳回 session 结果。
export interface JumpToSessionResult {
  /// 是否完成跳回。
  readonly jumped: boolean;
  /// 用户可读状态。
  readonly message: string;
  /// 复制降级文本。
  readonly fallback_text: string | null;
}

/// 跳回指定 session。
export const jumpToSession = async (
  request: JumpToSessionRequest,
): Promise<JumpToSessionResult> => {
  try {
    return await invoke<JumpToSessionResult>("jump_to_session", { request });
  } catch (error) {
    if (error instanceof Error && error.message.length > 0) {
      throw new Error(error.message, { cause: error });
    }
    if (typeof error === "string" && error.length > 0) {
      throw new Error(error, { cause: error });
    }
    throw new Error("跳回 session 失败", { cause: error });
  }
};
