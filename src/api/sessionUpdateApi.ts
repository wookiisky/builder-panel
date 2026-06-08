import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { SessionKey } from "./mockPanelContract";

/// Session 运行时来源。
export type SessionUpdateRuntimeSource = "codex_cli" | "codex_app";

/// Session 更新影响区域。
export type SessionUpdateArea = "session" | "timeline" | "both";

/// 后端推送的轻量 session 更新。
export interface SessionUpdateNotification {
  /// 更新来源。
  readonly runtime_source: SessionUpdateRuntimeSource;
  /// 所属 session。
  readonly session_key: SessionKey;
  /// 影响区域。
  readonly changed_area: SessionUpdateArea;
  /// 更新时间。
  readonly updated_at: { readonly value: number };
}

/// 订阅后端 session 更新事件。
export const subscribeSessionUpdates = async (
  onUpdate: (notification: SessionUpdateNotification) => void,
): Promise<UnlistenFn> => {
  if (!isTauriRuntime()) {
    return () => {};
  }

  return listen<SessionUpdateNotification>("session_updated", (event) => {
    onUpdate(event.payload);
  });
};

/// 判断当前是否运行在 Tauri 环境。
const isTauriRuntime = (): boolean => {
  return "__TAURI_INTERNALS__" in window;
};
