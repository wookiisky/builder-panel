import { invoke } from "@tauri-apps/api/core";
import {
  getCurrentWindow,
  PhysicalPosition,
  PhysicalSize,
} from "@tauri-apps/api/window";

import type { PanelSettings } from "./settingsContract";

/// panel 窗口状态局部保存请求。
export interface PanelWindowStateUpdate {
  /// 是否处于收缩状态。
  readonly collapsed?: boolean;
  /// 上次窗口位置。
  readonly window_position?: PanelSettings["window_position"];
  /// 上次窗口尺寸。
  readonly window_size?: PanelSettings["window_size"];
}

/// 保存 panel 窗口状态。
export const savePanelWindowState = async (
  update: PanelWindowStateUpdate,
): Promise<void> => {
  try {
    await invoke<void>("save_panel_window_state", { update });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "保存 panel 状态失败");
    }
    saveFallbackPanelState(update);
  }
};

/// 应用已持久化的窗口几何。
export const applyPanelWindowGeometry = async (
  panel: PanelSettings,
): Promise<void> => {
  if (!isTauriRuntime()) {
    return;
  }

  const appWindow = getCurrentWindow();
  if (panel.window_size !== null) {
    await appWindow.setSize(
      new PhysicalSize(panel.window_size.width, panel.window_size.height),
    );
  }
  if (panel.window_position !== null) {
    await appWindow.setPosition(
      new PhysicalPosition(panel.window_position.x, panel.window_position.y),
    );
  }
};

/// 监听窗口位置和尺寸变化。
export const subscribePanelWindowGeometry = async (
  onChange: (update: PanelWindowStateUpdate) => void,
): Promise<() => void> => {
  if (!isTauriRuntime()) {
    return () => {};
  }

  const appWindow = getCurrentWindow();
  const unlistenMoved = await appWindow.onMoved(({ payload }) => {
    onChange({
      window_position: {
        x: payload.x,
        y: payload.y,
      },
    });
  });
  const unlistenResized = await appWindow.onResized(({ payload }) => {
    onChange({
      window_size: {
        width: payload.width,
        height: payload.height,
      },
    });
  });

  return () => {
    unlistenMoved();
    unlistenResized();
  };
};

/// 浏览器开发环境保存 panel 状态。
const saveFallbackPanelState = (update: PanelWindowStateUpdate): void => {
  const raw = window.localStorage.getItem("builder-panel-settings");
  if (raw === null) {
    return;
  }

  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) {
      return;
    }
    const settings = parsed as { panel?: PanelSettings };
    if (settings.panel === undefined) {
      return;
    }
    settings.panel = {
      ...settings.panel,
      ...update,
    };
    window.localStorage.setItem(
      "builder-panel-settings",
      JSON.stringify(settings),
    );
  } catch {
    // 浏览器 fallback 不把局部保存失败暴露成主流程错误。
  }
};

/// 创建保留 cause 的错误。
const errorWithCause = (error: unknown, fallback: string): Error => {
  if (error instanceof Error && error.message.length > 0) {
    return new Error(error.message, { cause: error });
  }
  if (typeof error === "string" && error.length > 0) {
    return new Error(error, { cause: error });
  }

  return new Error(fallback, { cause: error });
};

/// 判断当前是否运行在 Tauri 环境。
const isTauriRuntime = (): boolean => {
  return "__TAURI_INTERNALS__" in window;
};
