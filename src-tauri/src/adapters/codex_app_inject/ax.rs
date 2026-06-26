//! macOS Accessibility 树查找 + 应用激活辅助。
//!
//! 实测 Codex.app（Electron）主窗口 AX 树只有空的 AXGroup 嵌套——
//! Chromium 默认不暴露 webview 内文本控件。但 AX 仍然暴露**窗口本身**的
//! position/size，所以可以用 AX 拿窗口 frame，再用 CGEvent 鼠标点击
//! "底部输入框区域"把焦点拉到 textarea，最后再走键盘事件 paste + Return。

use std::ffi::c_void;
use std::ptr;
use std::thread::sleep;
use std::time::{Duration, Instant};

use accessibility_sys::{
    kAXErrorSuccess, kAXFocusedWindowAttribute, kAXMainWindowAttribute, kAXPositionAttribute,
    kAXSizeAttribute, AXUIElementCopyAttributeValue, AXUIElementCreateApplication, AXUIElementRef,
    AXValueGetValue, AXValueRef,
};
use core_foundation::base::{CFTypeRef, TCFType};
use core_foundation::string::CFString;
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;

use crate::domain::app_error::AppError;

use super::inject_error;

const CODEX_APP_BUNDLE_ID: &str = "com.openai.codex";

/// `kAXValueCGPointType` 常量值（accessibility-sys 0.1 未导出）。
const K_AX_VALUE_CGPOINT_TYPE: u32 = 1;
/// `kAXValueCGSizeType` 常量值。
const K_AX_VALUE_CGSIZE_TYPE: u32 = 2;

/// 距窗口底部多少像素是输入框文本区中线（实测：120 时光标在输入框上沿外，
/// 80 让光标落在文本区上部）。
const INPUT_OFFSET_FROM_BOTTOM_PX: f64 = 80.0;

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
struct CGPointFFI {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
struct CGSizeFFI {
    width: f64,
    height: f64,
}

mod ns {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSRunningApplication, NSWorkspace};
    use objc2_foundation::NSString;

    pub fn frontmost_bundle_id() -> Option<String> {
        let workspace = unsafe { NSWorkspace::sharedWorkspace() };
        let front: Option<Retained<NSRunningApplication>> =
            unsafe { workspace.frontmostApplication() };
        let app = front?;
        let bid: Option<Retained<NSString>> = unsafe { app.bundleIdentifier() };
        bid.map(|s| s.to_string())
    }

    pub fn pid_for_bundle_id(bundle_id: &str) -> Option<i32> {
        let workspace = unsafe { NSWorkspace::sharedWorkspace() };
        let apps: Retained<objc2_foundation::NSArray<NSRunningApplication>> =
            unsafe { workspace.runningApplications() };
        let count = apps.count();
        let target = NSString::from_str(bundle_id);
        for i in 0..count {
            let app: Retained<NSRunningApplication> = unsafe { apps.objectAtIndex(i) };
            if let Some(bid) = unsafe { app.bundleIdentifier() } {
                if unsafe { bid.isEqualToString(&target) } {
                    let pid: i32 = unsafe { msg_send![&app, processIdentifier] };
                    return Some(pid);
                }
            }
        }
        None
    }

    pub fn activate_bundle_id(bundle_id: &str) {
        let workspace = unsafe { NSWorkspace::sharedWorkspace() };
        let apps: Retained<objc2_foundation::NSArray<NSRunningApplication>> =
            unsafe { workspace.runningApplications() };
        let count = apps.count();
        let target = NSString::from_str(bundle_id);
        for i in 0..count {
            let app: Retained<NSRunningApplication> = unsafe { apps.objectAtIndex(i) };
            if let Some(bid) = unsafe { app.bundleIdentifier() } {
                if unsafe { bid.isEqualToString(&target) } {
                    let _ok: bool = unsafe { msg_send![&app, activateWithOptions: 0u64] };
                    return;
                }
            }
        }
    }
}

/// 等待 Codex.app 成为前台。
pub fn wait_codex_app_frontmost(timeout_ms: u64) -> Result<(), AppError> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        if let Some(bid) = ns::frontmost_bundle_id() {
            if bid == CODEX_APP_BUNDLE_ID {
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            // 最后一次主动激活兜底。
            ns::activate_bundle_id(CODEX_APP_BUNDLE_ID);
            sleep(Duration::from_millis(200));
            if let Some(bid) = ns::frontmost_bundle_id() {
                if bid == CODEX_APP_BUNDLE_ID {
                    return Ok(());
                }
            }
            return Err(inject_error(
                "Codex.app 未在前台，无法注入消息",
                format!(
                    "frontmost bundle id = {}",
                    ns::frontmost_bundle_id().unwrap_or_else(|| "<none>".to_string())
                ),
                None,
            ));
        }
        sleep(Duration::from_millis(50));
    }
}

/// 确保 Codex.app 在前台，然后用 AX 读主窗口 frame、CGEvent 模拟鼠标
/// 点击"底部输入框区域"把焦点拉到 textarea。
///
/// Codex.app 是 Electron，Chromium 不通过 AX 暴露 webview 内的 textarea，
/// 但**窗口本身**的 AXPosition + AXSize 是可读的。点击坐标按图实测推算：
/// 输入框在窗口底部，距下边沿 ~120px 处是文本区中线。
pub fn focus_codex_app_input_field() -> Result<(), AppError> {
    // 1. 确保前台。
    if ns::frontmost_bundle_id().as_deref() != Some(CODEX_APP_BUNDLE_ID) {
        ns::activate_bundle_id(CODEX_APP_BUNDLE_ID);
        sleep(Duration::from_millis(120));
        if ns::frontmost_bundle_id().as_deref() != Some(CODEX_APP_BUNDLE_ID) {
            return Err(inject_error(
                "无法激活 Codex.app 到前台",
                "frontmost is not com.openai.codex after activate".to_string(),
                None,
            ));
        }
    }

    // 2. 读窗口 frame。
    let frame = read_focused_window_frame().ok_or_else(|| {
        inject_error(
            "无法读取 Codex.app 窗口位置",
            "AX position/size 读取失败".to_string(),
            None,
        )
    })?;

    // 3. 算输入框点击坐标：水平居中，垂直在底部偏上 INPUT_OFFSET_FROM_BOTTOM_PX。
    let click_x = frame.0 + frame.2 / 2.0;
    let click_y = frame.1 + frame.3 - INPUT_OFFSET_FROM_BOTTOM_PX;

    crate::adapters::logging::log_info(
        "Codex APP 注入：窗口 frame 与点击坐标",
        serde_json::json!({
            "win_x": frame.0,
            "win_y": frame.1,
            "win_w": frame.2,
            "win_h": frame.3,
            "click_x": click_x,
            "click_y": click_y,
        }),
    );

    // 4. 等待 Codex.app UI 切换到该 thread 完成（thread URL 跳转后 webview 重渲染要时间）。
    sleep(Duration::from_millis(200));

    // 5. 模拟鼠标左键点击。
    click_at(click_x, click_y).map_err(|detail| inject_error("模拟鼠标点击失败", detail, None))?;

    // 6. 让 Electron 处理 focus 事件。
    sleep(Duration::from_millis(120));

    Ok(())
}

/// 返回 Codex.app 主窗口的 frame：(x, y, width, height)。
///
/// 优先用 kAXMainWindow（窗口标题栏黑色的那个，UI 上"主"窗口）；
/// 如果拿不到再 fallback 到 kAXFocusedWindow。
fn read_focused_window_frame() -> Option<(f64, f64, f64, f64)> {
    let pid = ns::pid_for_bundle_id(CODEX_APP_BUNDLE_ID)?;
    let app_el: AXUIElementRef = unsafe { AXUIElementCreateApplication(pid) };
    if app_el.is_null() {
        return None;
    }

    // 主窗口优先；focused 兜底。
    let win_raw = copy_attribute(app_el, kAXMainWindowAttribute)
        .or_else(|| copy_attribute(app_el, kAXFocusedWindowAttribute))?;
    let win = win_raw as AXUIElementRef;
    if win.is_null() {
        return None;
    }

    let pos_raw = copy_attribute(win, kAXPositionAttribute)?;
    let mut pos = CGPointFFI::default();
    let ok = unsafe {
        AXValueGetValue(
            pos_raw as AXValueRef,
            K_AX_VALUE_CGPOINT_TYPE,
            &mut pos as *mut _ as *mut c_void,
        )
    };
    if !ok {
        return None;
    }

    let size_raw = copy_attribute(win, kAXSizeAttribute)?;
    let mut size = CGSizeFFI::default();
    let ok = unsafe {
        AXValueGetValue(
            size_raw as AXValueRef,
            K_AX_VALUE_CGSIZE_TYPE,
            &mut size as *mut _ as *mut c_void,
        )
    };
    if !ok {
        return None;
    }

    Some((pos.x, pos.y, size.width, size.height))
}

fn copy_attribute(el: AXUIElementRef, attr_name: &str) -> Option<CFTypeRef> {
    let attr = CFString::new(attr_name);
    let mut value: CFTypeRef = ptr::null();
    let err = unsafe { AXUIElementCopyAttributeValue(el, attr.as_concrete_TypeRef(), &mut value) };
    if err == kAXErrorSuccess && !value.is_null() {
        Some(value)
    } else {
        None
    }
}

extern "C" {
    fn CGWarpMouseCursorPosition(point: CGPoint) -> i32;
    fn CGAssociateMouseAndMouseCursorPosition(connected: u8) -> i32;
}

/// 抓取当前鼠标位置（屏幕坐标）。失败返回 None。
pub fn capture_cursor_position() -> Option<(f64, f64)> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok()?;
    let event = CGEvent::new(source).ok()?;
    let loc = event.location();
    Some((loc.x, loc.y))
}

/// 把鼠标光标移动回指定位置。
pub fn restore_cursor_position(x: f64, y: f64) {
    let point = CGPoint::new(x, y);
    unsafe {
        let _ = CGWarpMouseCursorPosition(point);
        let _ = CGAssociateMouseAndMouseCursorPosition(1);
    }
}

fn click_at(x: f64, y: f64) -> Result<(), String> {
    let point = CGPoint::new(x, y);
    unsafe {
        let _ = CGWarpMouseCursorPosition(point);
        let _ = CGAssociateMouseAndMouseCursorPosition(1);
    }

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "CGEventSource::new failed".to_string())?;

    // 左键 down。
    let down = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| "create LeftMouseDown failed".to_string())?;
    // kCGMouseEventClickState = 1：告诉 Chromium 这是单次点击。
    down.set_integer_value_field(1, 1);
    down.post(CGEventTapLocation::HID);

    // down 和 up 之间留一点间隔，Chromium 的事件处理才认。
    std::thread::sleep(Duration::from_millis(40));

    // 左键 up。
    let up = CGEvent::new_mouse_event(source, CGEventType::LeftMouseUp, point, CGMouseButton::Left)
        .map_err(|_| "create LeftMouseUp failed".to_string())?;
    up.set_integer_value_field(1, 1);
    up.post(CGEventTapLocation::HID);

    Ok(())
}
