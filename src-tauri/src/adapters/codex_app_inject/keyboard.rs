//! 键盘事件 + 剪贴板注入。

use std::thread::sleep;
use std::time::Duration;

use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::NSString;

use crate::domain::app_error::AppError;

use super::inject_error;

/// V key 的 macOS virtual keycode。
const KEY_CODE_V: CGKeyCode = 9;
/// Return key 的 macOS virtual keycode。
const KEY_CODE_RETURN: CGKeyCode = 36;

/// 把 prompt 粘贴到当前 frontmost app 的 focused 输入框，然后发 Return。
///
/// 步骤：备份剪贴板 → 写入 prompt → Cmd+V → Return → 恢复剪贴板。
pub fn paste_text_and_return(prompt: &str) -> Result<(), AppError> {
    let pasteboard = unsafe { NSPasteboard::generalPasteboard() };

    // 1. 备份现有剪贴板内容（仅备份 string 类型，最佳努力）。
    let backup = read_pasteboard_string(&pasteboard);

    // 2. 写入 prompt。
    write_pasteboard_string(&pasteboard, prompt).map_err(|detail| {
        inject_error("写入剪贴板失败", detail, None)
    })?;

    // 3. 发 Cmd+V。
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| inject_error("无法创建 CGEventSource", "CGEventSource::new failed".to_string(), None))?;

    post_key(&source, KEY_CODE_V, true, Some(CGEventFlags::CGEventFlagCommand))
        .map_err(|d| inject_error("发送 Cmd+V (down) 失败", d, None))?;
    post_key(&source, KEY_CODE_V, false, Some(CGEventFlags::CGEventFlagCommand))
        .map_err(|d| inject_error("发送 Cmd+V (up) 失败", d, None))?;

    // 4. 让 Electron 处理粘贴。
    sleep(Duration::from_millis(60));

    // 5. 发 Return。
    post_key(&source, KEY_CODE_RETURN, true, None)
        .map_err(|d| inject_error("发送 Return (down) 失败", d, None))?;
    post_key(&source, KEY_CODE_RETURN, false, None)
        .map_err(|d| inject_error("发送 Return (up) 失败", d, None))?;

    // 6. 等一下让 Codex 接收完事件再恢复剪贴板。
    sleep(Duration::from_millis(100));

    // 7. 最佳努力恢复剪贴板。
    if let Some(prev) = backup {
        let _ = write_pasteboard_string(&pasteboard, &prev);
    }

    Ok(())
}

fn post_key(
    source: &CGEventSource,
    keycode: CGKeyCode,
    down: bool,
    flags: Option<CGEventFlags>,
) -> Result<(), String> {
    let event = CGEvent::new_keyboard_event(source.clone(), keycode, down)
        .map_err(|_| "CGEvent::new_keyboard_event failed".to_string())?;
    if let Some(f) = flags {
        event.set_flags(f);
    }
    event.post(CGEventTapLocation::HID);
    Ok(())
}

fn read_pasteboard_string(pasteboard: &NSPasteboard) -> Option<String> {
    let s: Option<Retained<NSString>> = unsafe {
        let raw: *mut AnyObject = msg_send![pasteboard, stringForType: NSPasteboardTypeString];
        if raw.is_null() {
            None
        } else {
            Retained::retain(raw as *mut NSString)
        }
    };
    s.map(|v| v.to_string())
}

fn write_pasteboard_string(pasteboard: &NSPasteboard, text: &str) -> Result<(), String> {
    unsafe {
        // clearContents 返回 NSInteger (changeCount)，不是 BOOL。
        let _: isize = msg_send![pasteboard, clearContents];
        let ns_text = NSString::from_str(text);
        let ns_type: &NSString = NSPasteboardTypeString;
        let ok: bool = msg_send![pasteboard, setString: &*ns_text, forType: ns_type];
        if ok {
            Ok(())
        } else {
            Err("NSPasteboard.setString returned NO".to_string())
        }
    }
}
