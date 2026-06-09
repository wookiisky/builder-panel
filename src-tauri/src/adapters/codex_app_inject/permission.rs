//! macOS 辅助功能权限检测和引导。

use std::ffi::c_void;

use core_foundation::base::{Boolean, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::{CFString, CFStringRef};

use crate::domain::app_error::{AppError, FallbackAction};

use super::inject_error;

extern "C" {
    fn AXIsProcessTrusted() -> Boolean;
    fn AXIsProcessTrustedWithOptions(options: *mut c_void) -> Boolean;
}

/// 检测当前进程是否有 Accessibility 权限；没有则触发系统授权提示并返回错误。
///
/// 错误带 `FallbackAction::OpenSettings`，调用方可调用 [`open_accessibility_settings`]
/// 把用户跳到系统设置 → 隐私 → 辅助功能。
pub fn ensure_accessibility_trusted() -> Result<(), AppError> {
    if unsafe { AXIsProcessTrusted() } != 0 {
        return Ok(());
    }

    // 触发一次 TCC 授权提示。这次调用本身可能仍返回 false（用户没立即同意）；
    // 我们不阻塞——返回错误让前端展示提示，下次再试。
    let _ = with_prompt_check();

    Err(inject_error(
        "Builder Panel 需要辅助功能权限才能把消息注入到 Codex.app；请在 系统设置 → 隐私与安全性 → 辅助功能 中勾选 Builder Panel 后重试",
        "AXIsProcessTrusted=false".to_string(),
        Some(FallbackAction::OpenSettings),
    ))
}

fn with_prompt_check() -> bool {
    // 构造 { kAXTrustedCheckOptionPrompt: true } CFDictionary。
    let key = CFString::from_static_string("AXTrustedCheckOptionPrompt");
    let value = CFBoolean::true_value();
    let pairs = vec![(key, value.as_CFType())];
    let dict = CFDictionary::from_CFType_pairs(&pairs);
    let result = unsafe {
        AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef() as *mut c_void)
    };
    result != 0
}

/// 把用户跳到 系统设置 → 隐私与安全性 → 辅助功能 面板。
pub fn open_accessibility_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .status();
}

// 防止 unused 警告：CFStringRef 在未来扩展可能用到。
#[allow(dead_code)]
fn _suppress_unused(_s: CFStringRef) {}
