/// 二维逻辑坐标。
export interface PanelLogicalPosition {
  /// 横坐标。
  readonly x: number;
  /// 纵坐标。
  readonly y: number;
}

/// 二维逻辑尺寸。
export interface PanelLogicalSize {
  /// 宽度。
  readonly width: number;
  /// 高度。
  readonly height: number;
}

/// 逻辑坐标矩形。
export interface PanelLogicalRect {
  /// 左上角位置。
  readonly position: PanelLogicalPosition;
  /// 矩形尺寸。
  readonly size: PanelLogicalSize;
}

/// 当前 panel 窗口及显示器工作区的逻辑几何。
export interface PanelWindowLogicalGeometry {
  /// 当前窗口左上角位置。
  readonly windowPosition: PanelLogicalPosition;
  /// 当前窗口内部尺寸。
  readonly windowSize: PanelLogicalSize;
  /// 当前显示器工作区；显示器不可识别时为空。
  readonly workArea: PanelLogicalRect | null;
}
