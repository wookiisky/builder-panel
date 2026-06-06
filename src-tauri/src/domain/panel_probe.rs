//! panel 基础能力探针模型。

use serde::Serialize;

/// panel 展示模式。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelMode {
    /// 首版扩展模式。
    Expanded,
}

/// panel 基础探针状态。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PanelProbe {
    /// 当前 panel 展示模式。
    pub mode: PanelMode,
    /// 当前是否收缩。
    pub collapsed: bool,
    /// 当前窗口是否应置顶。
    pub always_on_top: bool,
    /// 当前窗口是否可拖动。
    pub draggable: bool,
}

impl PanelProbe {
    /// 创建阶段 0 默认 panel 探针。
    pub fn expanded_default() -> Self {
        Self {
            mode: PanelMode::Expanded,
            collapsed: false,
            always_on_top: true,
            draggable: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PanelMode, PanelProbe};

    #[test]
    fn creates_expanded_default_probe() {
        let probe = PanelProbe::expanded_default();

        assert_eq!(probe.mode, PanelMode::Expanded);
        assert!(!probe.collapsed);
        assert!(probe.always_on_top);
        assert!(probe.draggable);
    }
}
