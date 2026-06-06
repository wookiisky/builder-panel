/// panel 本地 UI 状态。
export interface PanelUiState {
  /// 当前是否收缩。
  readonly collapsed: boolean;
}

/// 创建默认 panel UI 状态。
export const createDefaultPanelUiState = (): PanelUiState => {
  return {
    collapsed: false,
  };
};

/// 切换 panel 收缩状态。
export const togglePanelCollapsed = (state: PanelUiState): PanelUiState => {
  return {
    ...state,
    collapsed: !state.collapsed,
  };
};
