//! macOS Accessibility 树查找 + 应用激活辅助。
//!
//! 实测 Codex.app（Electron）主窗口 AX 树只有空的 AXGroup 嵌套——
//! Chromium 默认不暴露 webview 内文本控件。因此本模块**不**尝试通过 AX
//! 定位输入框，只负责确保 Codex.app 在前台，让 CGEvent 键盘事件落到
//! 当前焦点（用户正常打开 thread 时焦点本来就在输入框）。

use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::domain::app_error::AppError;

use super::inject_error;

const CODEX_APP_BUNDLE_ID: &str = "com.openai.codex";

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
                    let _ok: bool = unsafe {
                        msg_send![&app, activateWithOptions: 0u64]
                    };
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

/// Electron Codex.app 不通过 AX 暴露输入框，焦点依赖用户打开窗口时的状态。
///
/// 这里只做"确保 Codex.app 是 frontmost"的兜底激活，让后续 CGEvent 键盘事件
/// 命中其当前焦点元素。返回 Ok(()) 不代表 AX 找到了输入框。
pub fn focus_codex_app_input_field() -> Result<(), AppError> {
    if let Some(bid) = ns::frontmost_bundle_id() {
        if bid == CODEX_APP_BUNDLE_ID {
            return Ok(());
        }
    }
    ns::activate_bundle_id(CODEX_APP_BUNDLE_ID);
    sleep(Duration::from_millis(120));
    if let Some(bid) = ns::frontmost_bundle_id() {
        if bid == CODEX_APP_BUNDLE_ID {
            return Ok(());
        }
    }
    Err(inject_error(
        "无法激活 Codex.app 到前台",
        "frontmost is not com.openai.codex after activate".to_string(),
        None,
    ))
}
