//! Codex.app GUI 文本注入 adapter。
//!
//! 通过 macOS Accessibility API + 模拟键盘事件，把 follow-up 文本注入到
//! Codex.app 当前 thread 的对话输入框。Codex.app 是 Electron 应用
//! (`/Applications/Codex.app/Contents/Frameworks/Codex Framework.framework/Helpers/`
//! 下有 Codex (Renderer).app / Codex (GPU).app)，所以不能用 `kAXValueAttribute`
//! 直接写值（不会触发 React onChange），必须走 NSPasteboard + Cmd+V + Return。
//!
//! 跳转/聚焦 Codex.app 由调用方走现有的 `codex://threads/<id>` URL Scheme，
//! 本模块只负责"已经在 Codex.app 前台"之后的注入步骤。

use crate::domain::app_error::{AppError, AppErrorCode, FallbackAction};

/// Codex.app 注入边界。
pub trait CodexAppInjector {
    /// 等待 Codex.app 成为前台（最多 timeout_ms 毫秒）。
    fn wait_codex_app_frontmost(&self, timeout_ms: u64) -> Result<(), AppError>;

    /// 在 Codex.app 当前 focused 窗口内定位输入框并设置焦点。
    ///
    /// Electron 下设焦点不一定生效；调用方仍可继续做粘贴+回车——只要
    /// Codex.app 自身是 frontmost，键盘事件会送给当前 focused element。
    fn focus_input_field(&self) -> Result<(), AppError>;

    /// 把 prompt 写到剪贴板，发 Cmd+V，然后发 Return。
    ///
    /// 调用前剪贴板内容会被备份；调用结束做最佳努力恢复。
    fn paste_and_return(&self, prompt: &str) -> Result<(), AppError>;
}

/// 创建错误：注入流程相关问题（找不到窗口/输入框、权限缺失等）。
pub(crate) fn inject_error(
    user_message: impl Into<String>,
    technical_detail: impl Into<String>,
    fallback: Option<FallbackAction>,
) -> AppError {
    AppError::new(
        AppErrorCode::ReplySendFailed,
        user_message,
        Some(technical_detail.into()),
        false,
        fallback,
    )
}

#[cfg(target_os = "macos")]
mod ax;
#[cfg(target_os = "macos")]
mod keyboard;
#[cfg(target_os = "macos")]
mod permission;

#[cfg(target_os = "macos")]
pub use permission::{ensure_accessibility_trusted, open_accessibility_settings};

#[cfg(target_os = "macos")]
pub use ax::{capture_cursor_position, restore_cursor_position};

#[cfg(not(target_os = "macos"))]
pub fn capture_cursor_position() -> Option<(f64, f64)> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn restore_cursor_position(_x: f64, _y: f64) {}

#[cfg(target_os = "macos")]
mod imp {
    use super::{ax, inject_error, keyboard, CodexAppInjector};
    use crate::domain::app_error::AppError;

    /// macOS 上的真实 injector。
    pub struct SystemCodexAppInjector;

    impl CodexAppInjector for SystemCodexAppInjector {
        fn wait_codex_app_frontmost(&self, timeout_ms: u64) -> Result<(), AppError> {
            ax::wait_codex_app_frontmost(timeout_ms)
        }

        fn focus_input_field(&self) -> Result<(), AppError> {
            // Electron 下 focus 经常不生效；做最佳努力，失败也不阻塞 paste_and_return。
            // 上层逻辑只要 Codex.app frontmost、用户上次焦点本来就在输入框，键盘事件即可命中。
            match ax::focus_codex_app_input_field() {
                Ok(()) => Ok(()),
                Err(error) => {
                    // 转成警告级（technical_detail 保留），不直接返错。
                    let _ = error;
                    Ok(())
                }
            }
        }

        fn paste_and_return(&self, prompt: &str) -> Result<(), AppError> {
            if prompt.is_empty() {
                return Err(inject_error(
                    "follow-up 内容不能为空",
                    "empty prompt".to_string(),
                    None,
                ));
            }
            keyboard::paste_text_and_return(prompt)
        }
    }

    /// 默认 injector 工厂。
    pub fn default_codex_app_injector() -> Result<SystemCodexAppInjector, String> {
        Ok(SystemCodexAppInjector)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::{inject_error, CodexAppInjector};
    use crate::domain::app_error::AppError;

    /// 非 macOS 平台 stub。
    pub struct SystemCodexAppInjector;

    impl CodexAppInjector for SystemCodexAppInjector {
        fn wait_codex_app_frontmost(&self, _timeout_ms: u64) -> Result<(), AppError> {
            Err(inject_error(
                "当前平台不支持 Codex.app 注入",
                "non-macos platform".to_string(),
                None,
            ))
        }

        fn focus_input_field(&self) -> Result<(), AppError> {
            Err(inject_error(
                "当前平台不支持 Codex.app 注入",
                "non-macos platform".to_string(),
                None,
            ))
        }

        fn paste_and_return(&self, _prompt: &str) -> Result<(), AppError> {
            Err(inject_error(
                "当前平台不支持 Codex.app 注入",
                "non-macos platform".to_string(),
                None,
            ))
        }
    }

    pub fn default_codex_app_injector() -> Result<SystemCodexAppInjector, String> {
        Err("当前平台不支持 Codex.app 注入".to_string())
    }

    /// 非 macOS 上的权限检查 stub。
    pub fn ensure_accessibility_trusted() -> Result<(), AppError> {
        Err(inject_error(
            "当前平台不支持辅助功能注入",
            "non-macos platform".to_string(),
            None,
        ))
    }

    /// 非 macOS 上的设置面板 stub。
    pub fn open_accessibility_settings() {}
}

#[cfg(not(target_os = "macos"))]
pub use imp::{ensure_accessibility_trusted, open_accessibility_settings};

pub use imp::{default_codex_app_injector, SystemCodexAppInjector};

#[cfg(test)]
pub mod fake;
