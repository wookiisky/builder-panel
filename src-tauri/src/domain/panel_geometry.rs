//! panel 窗口几何规则。

/// 屏幕可用区域。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayBounds {
    /// 屏幕左上角横坐标。
    pub x: i32,
    /// 屏幕左上角纵坐标。
    pub y: i32,
    /// 屏幕可用宽度。
    pub width: u32,
    /// 屏幕可用高度。
    pub height: u32,
}

/// panel 窗口矩形。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PanelRect {
    /// panel 左上角横坐标。
    pub x: i32,
    /// panel 左上角纵坐标。
    pub y: i32,
    /// panel 宽度。
    pub width: u32,
    /// panel 高度。
    pub height: u32,
}

/// 将 panel 位置修正到屏幕可见区域内。
pub fn clamp_panel_rect_to_display(
    panel_rect: PanelRect,
    display_bounds: DisplayBounds,
) -> PanelRect {
    let max_x = display_bounds.x + display_bounds.width.saturating_sub(panel_rect.width) as i32;
    let max_y = display_bounds.y + display_bounds.height.saturating_sub(panel_rect.height) as i32;

    let x = panel_rect
        .x
        .clamp(display_bounds.x, max_x.max(display_bounds.x));
    let y = panel_rect
        .y
        .clamp(display_bounds.y, max_y.max(display_bounds.y));

    PanelRect {
        x,
        y,
        width: panel_rect.width,
        height: panel_rect.height,
    }
}

#[cfg(test)]
mod tests {
    use super::{clamp_panel_rect_to_display, DisplayBounds, PanelRect};

    #[test]
    fn keeps_visible_panel_position() {
        let display_bounds = DisplayBounds {
            x: 0,
            y: 0,
            width: 1200,
            height: 800,
        };
        let panel_rect = PanelRect {
            x: 120,
            y: 96,
            width: 420,
            height: 260,
        };

        let clamped = clamp_panel_rect_to_display(panel_rect, display_bounds);

        assert_eq!(clamped, panel_rect);
    }

    #[test]
    fn moves_panel_back_from_display_edge() {
        let display_bounds = DisplayBounds {
            x: 0,
            y: 0,
            width: 1200,
            height: 800,
        };
        let panel_rect = PanelRect {
            x: 1000,
            y: 700,
            width: 420,
            height: 260,
        };

        let clamped = clamp_panel_rect_to_display(panel_rect, display_bounds);

        assert_eq!(
            clamped,
            PanelRect {
                x: 780,
                y: 540,
                width: 420,
                height: 260,
            }
        );
    }

    #[test]
    fn handles_display_smaller_than_panel() {
        let display_bounds = DisplayBounds {
            x: 80,
            y: 40,
            width: 300,
            height: 160,
        };
        let panel_rect = PanelRect {
            x: 20,
            y: 10,
            width: 420,
            height: 260,
        };

        let clamped = clamp_panel_rect_to_display(panel_rect, display_bounds);

        assert_eq!(
            clamped,
            PanelRect {
                x: 80,
                y: 40,
                width: 420,
                height: 260,
            }
        );
    }
}
