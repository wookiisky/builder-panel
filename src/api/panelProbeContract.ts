/// panel 展示模式。
export type PanelMode = "expanded";

/// panel 基础探针视图。
export interface PanelProbeView {
  /// 当前 panel 模式。
  readonly mode: PanelMode;
  /// 当前是否收缩。
  readonly collapsed: boolean;
  /// panel 是否应保持置顶。
  readonly always_on_top: boolean;
  /// panel 是否允许拖动。
  readonly draggable: boolean;
}
