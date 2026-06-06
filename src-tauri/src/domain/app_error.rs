//! 应用错误模型。

use serde::{Deserialize, Serialize};

/// 应用错误码。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorCode {
    /// 本地 bridge 不可用。
    BridgeUnavailable,
    /// Agent payload 无法清洗成领域事件。
    MalformedAgentPayload,
    /// 当前回复目标不支持。
    UnsupportedReplyTarget,
    /// 回复发送失败。
    ReplySendFailed,
    /// 配置读取失败。
    ConfigLoadFailed,
    /// 配置保存失败。
    ConfigSaveFailed,
    /// Agent 协议未支持。
    AgentProtocolUnsupported,
    /// 过程时间线不可用。
    ProcessTimelineUnavailable,
    /// 过程时间线接收失败。
    ProcessTimelineReceiveFailed,
    /// 系统通知发送失败。
    NotificationSendFailed,
    /// hook 安装失败。
    HookInstallFailed,
    /// hook 卸载失败。
    HookUninstallFailed,
}

/// 降级动作。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackAction {
    /// 提示用户复制到剪贴板。
    CopyToClipboard,
    /// 提示用户稍后重试。
    RetryLater,
    /// 提示用户打开设置。
    OpenSettings,
    /// 降级为只读展示。
    ViewReadOnly,
}

/// 应用错误对象。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AppError {
    /// 错误码。
    pub code: AppErrorCode,
    /// 用户可读错误消息。
    pub user_message: String,
    /// 可选技术细节。
    pub technical_detail: Option<String>,
    /// 是否可重试。
    pub retryable: bool,
    /// 可选降级动作。
    pub fallback_action: Option<FallbackAction>,
}

impl AppError {
    /// 创建应用错误对象。
    pub fn new(
        code: AppErrorCode,
        user_message: impl Into<String>,
        technical_detail: Option<String>,
        retryable: bool,
        fallback_action: Option<FallbackAction>,
    ) -> Self {
        Self {
            code,
            user_message: user_message.into(),
            technical_detail,
            retryable,
            fallback_action,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppError, AppErrorCode, FallbackAction};

    #[test]
    fn app_error_carries_code_message_retry_and_fallback() {
        let error = AppError::new(
            AppErrorCode::ReplySendFailed,
            "回复发送失败",
            Some("stdin closed".to_string()),
            true,
            Some(FallbackAction::CopyToClipboard),
        );

        assert_eq!(error.code, AppErrorCode::ReplySendFailed);
        assert_eq!(error.user_message, "回复发送失败");
        assert!(error.retryable);
        assert_eq!(error.fallback_action, Some(FallbackAction::CopyToClipboard));
    }
}
