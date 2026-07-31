/// panel 窗口最小逻辑高度。
export const PANEL_WINDOW_MIN_LOGICAL_HEIGHT = 90;
/// panel 窗口与屏幕工作区底边的逻辑间距。
export const PANEL_WINDOW_SCREEN_EDGE_GAP = 48;
/// 判断窗口高度是否已经收敛的逻辑像素误差。
export const PANEL_WINDOW_RESIZE_DELTA = 2;

/// panel 自适应高度计算输入。
export interface PanelAdaptiveWindowHeightInput {
  /// 用户配置的最大窗口高度。
  readonly configuredMaxHeight: number;
  /// 当前可见内容的自然高度。
  readonly contentHeight: number;
  /// 当前窗口和显示器工作区几何。
  readonly geometry: PanelWindowLogicalGeometry;
}

/// 根据自然内容和真实窗口几何计算目标逻辑高度。
export const panelAdaptiveWindowHeight = ({
  configuredMaxHeight,
  contentHeight,
  geometry,
}: PanelAdaptiveWindowHeightInput): number => {
  const availableHeight = panelAvailableWindowHeight(geometry);
  const maximumHeight = Math.max(
    PANEL_WINDOW_MIN_LOGICAL_HEIGHT,
    Math.min(configuredMaxHeight, availableHeight),
  );

  return clampNumber(
    Math.ceil(contentHeight),
    PANEL_WINDOW_MIN_LOGICAL_HEIGHT,
    maximumHeight,
  );
};

/// 判断真实窗口高度是否需要调整到目标高度。
export const panelWindowHeightNeedsResize = (
  actualHeight: number,
  targetHeight: number,
): boolean => {
  return Math.abs(actualHeight - targetHeight) > PANEL_WINDOW_RESIZE_DELTA;
};

/// 计算窗口当前位置到显示器工作区底边的可用高度。
const panelAvailableWindowHeight = (
  geometry: PanelWindowLogicalGeometry,
): number => {
  if (geometry.workArea === null) {
    return Number.POSITIVE_INFINITY;
  }

  const workAreaBottom =
    geometry.workArea.position.y + geometry.workArea.size.height;
  return Math.max(
    PANEL_WINDOW_MIN_LOGICAL_HEIGHT,
    workAreaBottom - geometry.windowPosition.y - PANEL_WINDOW_SCREEN_EDGE_GAP,
  );
};

/// 将数值限制在指定区间内。
const clampNumber = (value: number, min: number, max: number): number => {
  return Math.min(max, Math.max(min, value));
};
import type { PanelWindowLogicalGeometry } from "../api/panelWindowGeometryContract";

export type { PanelWindowLogicalGeometry } from "../api/panelWindowGeometryContract";
