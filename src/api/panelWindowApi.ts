import { invoke } from "@tauri-apps/api/core";
import {
  currentMonitor,
  getCurrentWindow,
  LogicalSize,
  PhysicalPosition,
} from "@tauri-apps/api/window";

import type { BuilderPanelSettings, PanelSettings } from "./settingsContract";
import type { PanelWindowLogicalGeometry } from "./panelWindowGeometryContract";
import { isTauriRuntime } from "./tauriRuntime";

/// panel 窗口状态局部保存请求。
export interface PanelWindowStateUpdate {
  /// 上次窗口位置。
  readonly window_position?: PanelSettings["window_position"];
  /// 上次窗口逻辑宽度。
  readonly window_width?: PanelSettings["window_width"];
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

  await applyPanelWindowGeometryToWindow(getCurrentWindow(), panel);
};

/// 应用已持久化的窗口偏好。
export const applyPanelWindowPreferences = async (
  settings: BuilderPanelSettings,
): Promise<void> => {
  if (!isTauriRuntime()) {
    return;
  }

  const appWindow = getCurrentWindow();
  let alwaysOnTopError: unknown = null;
  try {
    await appWindow.setAlwaysOnTop(settings.general.keep_panel_on_top);
  } catch (error) {
    alwaysOnTopError = error;
  }

  await applyPanelWindowGeometryToWindow(appWindow, settings.panel);

  if (alwaysOnTopError !== null) {
    throw errorWithCause(alwaysOnTopError, "应用 panel 窗口设置失败");
  }
};

/// 仅应用运行期间可变的 panel 置顶偏好。
export const applyPanelAlwaysOnTopPreference = async (
  settings: BuilderPanelSettings,
): Promise<void> => {
  if (!isTauriRuntime()) {
    return;
  }

  try {
    await getCurrentWindow().setAlwaysOnTop(settings.general.keep_panel_on_top);
  } catch (error) {
    throw errorWithCause(error, "应用 panel 置顶设置失败");
  }
};

/// 对指定窗口应用已持久化的窗口几何。
const applyPanelWindowGeometryToWindow = async (
  appWindow: ReturnType<typeof getCurrentWindow>,
  panel: PanelSettings,
): Promise<void> => {
  const windowWidth = panel.window_width;
  if (windowWidth !== null) {
    const currentSize = await appWindow.innerSize();
    const scaleFactor = await appWindow.scaleFactor();
    const currentLogicalHeight = currentSize.toLogical(scaleFactor).height;
    await appWindow.setSize(new LogicalSize(windowWidth, currentLogicalHeight));
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
    void appWindow.scaleFactor().then((scaleFactor) => {
      onChange({
        window_width: Math.round(payload.width / scaleFactor),
      });
    });
  });

  return () => {
    unlistenMoved();
    unlistenResized();
  };
};

/// 读取当前窗口和所在显示器工作区的统一逻辑几何。
export const readPanelWindowLogicalGeometry =
  async (): Promise<PanelWindowLogicalGeometry | null> => {
    if (!isTauriRuntime()) {
      return null;
    }

    try {
      const appWindow = getCurrentWindow();
      const [scaleFactor, innerSize, outerPosition, monitor] =
        await Promise.all([
          appWindow.scaleFactor(),
          appWindow.innerSize(),
          appWindow.outerPosition(),
          currentMonitor(),
        ]);
      const logicalSize = innerSize.toLogical(scaleFactor);

      return {
        windowPosition: {
          x: physicalToLogical(outerPosition.x, scaleFactor),
          y: physicalToLogical(outerPosition.y, scaleFactor),
        },
        windowSize: {
          width: logicalSize.width,
          height: logicalSize.height,
        },
        workArea:
          monitor === null
            ? null
            : {
                position: {
                  x: physicalToLogical(
                    monitor.workArea.position.x,
                    scaleFactor,
                  ),
                  y: physicalToLogical(
                    monitor.workArea.position.y,
                    scaleFactor,
                  ),
                },
                size: {
                  width: physicalToLogical(
                    monitor.workArea.size.width,
                    scaleFactor,
                  ),
                  height: physicalToLogical(
                    monitor.workArea.size.height,
                    scaleFactor,
                  ),
                },
              },
      };
    } catch (error) {
      throw errorWithCause(error, "读取 panel 窗口几何失败");
    }
  };

/// 保留当前真实宽度并调整 panel 窗口内容高度。
export const resizePanelWindowContentHeight = async (
  height: number,
): Promise<void> => {
  if (!isTauriRuntime()) {
    return;
  }

  try {
    const appWindow = getCurrentWindow();
    const scaleFactor = await appWindow.scaleFactor();
    const currentSize = await appWindow.innerSize();
    const currentLogicalWidth = currentSize.toLogical(scaleFactor).width;
    await appWindow.setSize(new LogicalSize(currentLogicalWidth, height));
  } catch (error) {
    throw errorWithCause(error, "调整 panel 窗口尺寸失败");
  }
};

/// 关闭当前 panel 窗口。
export const closePanelWindow = async (): Promise<void> => {
  if (!isTauriRuntime()) {
    return;
  }

  await getCurrentWindow().close();
};

/// 最小化当前 panel 窗口。
export const minimizePanelWindow = async (): Promise<void> => {
  if (!isTauriRuntime()) {
    return;
  }

  try {
    await getCurrentWindow().minimize();
  } catch (error) {
    throw errorWithCause(error, "最小化窗口失败");
  }
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

/// 将 Tauri 物理像素转换为当前窗口坐标系下的逻辑像素。
const physicalToLogical = (value: number, scaleFactor: number): number => {
  if (
    !Number.isFinite(value) ||
    !Number.isFinite(scaleFactor) ||
    scaleFactor <= 0
  ) {
    throw new Error("panel 窗口几何数据无效");
  }

  return value / scaleFactor;
};
