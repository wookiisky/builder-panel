//! 终端交互 adapter。

use crate::domain::agent_session::{JumpTarget, SessionKey};
use crate::domain::app_error::{AppError, AppErrorCode, FallbackAction};
use crate::ports::jump_target_port::JumpTargetPort;

/// URL 打开边界。
pub trait UrlOpener {
    /// 打开指定 URL。
    fn open_url(&mut self, url: &str) -> Result<(), String>;
}

/// 系统 URL 打开器。
pub struct SystemUrlOpener;

impl UrlOpener for SystemUrlOpener {
    fn open_url(&mut self, url: &str) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            let status = std::process::Command::new("open")
                .arg(url)
                .status()
                .map_err(|error| error.to_string())?;
            if status.success() {
                return Ok(());
            }

            return Err(format!("open exited with status {status}"));
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = url;
            Err("当前平台未验证 URL 跳回".to_string())
        }
    }
}

/// 终端跳回 adapter。
pub struct TerminalJumpAdapter<Opener = SystemUrlOpener>
where
    Opener: UrlOpener,
{
    /// 已记录的跳回请求。
    recorded_jumps: Vec<(SessionKey, JumpTarget)>,
    /// 下一次跳回是否失败。
    fail_next_jump: bool,
    /// URL 打开器。
    opener: Opener,
}

impl TerminalJumpAdapter<SystemUrlOpener> {
    /// 创建终端跳回 adapter。
    pub fn new() -> Self {
        Self {
            recorded_jumps: Vec::new(),
            fail_next_jump: false,
            opener: SystemUrlOpener,
        }
    }
}

impl<Opener> TerminalJumpAdapter<Opener>
where
    Opener: UrlOpener,
{
    /// 使用指定 URL 打开器创建 adapter。
    pub fn with_opener(opener: Opener) -> Self {
        Self {
            recorded_jumps: Vec::new(),
            fail_next_jump: false,
            opener,
        }
    }

    /// 设置下一次跳回失败。
    pub fn fail_next_jump(&mut self) {
        self.fail_next_jump = true;
    }

    /// 返回已记录的跳回请求。
    pub fn recorded_jumps(&self) -> &[(SessionKey, JumpTarget)] {
        &self.recorded_jumps
    }
}

impl Default for TerminalJumpAdapter<SystemUrlOpener> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Opener> JumpTargetPort for TerminalJumpAdapter<Opener>
where
    Opener: UrlOpener,
{
    fn jump_to_session(
        &mut self,
        session_key: &SessionKey,
        jump_target: &JumpTarget,
    ) -> Result<(), AppError> {
        if self.fail_next_jump {
            self.fail_next_jump = false;
            return Err(AppError::new(
                AppErrorCode::UnsupportedReplyTarget,
                "跳回终端失败",
                Some("terminal jump unavailable".to_string()),
                true,
                Some(FallbackAction::CopyToClipboard),
            ));
        }

        self.recorded_jumps
            .push((session_key.clone(), jump_target.clone()));
        if jump_target.location.starts_with("codex://") {
            return self
                .opener
                .open_url(&jump_target.location)
                .map_err(|detail| {
                    AppError::new(
                        AppErrorCode::UnsupportedReplyTarget,
                        "跳回工具界面失败",
                        Some(detail),
                        true,
                        Some(FallbackAction::CopyToClipboard),
                    )
                });
        }

        Err(AppError::new(
            AppErrorCode::UnsupportedReplyTarget,
            "当前跳回目标暂不支持自动打开",
            Some(jump_target.location.clone()),
            true,
            Some(FallbackAction::CopyToClipboard),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{TerminalJumpAdapter, UrlOpener};
    use crate::domain::agent_session::{
        AgentKind, ConversationId, JumpTarget, ProjectId, SessionKey,
    };
    use crate::domain::app_error::FallbackAction;
    use crate::ports::jump_target_port::JumpTargetPort;

    #[test]
    fn records_jump_without_sending_reply() {
        let mut adapter = TerminalJumpAdapter::with_opener(FakeUrlOpener {
            opened_urls: Vec::new(),
        });
        let key = session_key();
        let target = JumpTarget {
            label: "Codex APP".to_string(),
            location: "codex://threads/conversation".to_string(),
        };

        adapter
            .jump_to_session(&key, &target)
            .expect("jump should record");

        assert_eq!(adapter.recorded_jumps().len(), 1);
        assert_eq!(adapter.recorded_jumps()[0].0, key);
        assert_eq!(adapter.recorded_jumps()[0].1, target);
    }

    #[test]
    fn jump_failure_uses_clipboard_fallback() {
        let mut adapter = TerminalJumpAdapter::new();
        adapter.fail_next_jump();

        let result = adapter.jump_to_session(
            &session_key(),
            &JumpTarget {
                label: "Ghostty".to_string(),
                location: "window:1".to_string(),
            },
        );

        let error = result.expect_err("jump should fail");
        assert_eq!(error.fallback_action, Some(FallbackAction::CopyToClipboard));
        assert!(adapter.recorded_jumps().is_empty());
    }

    struct FakeUrlOpener {
        opened_urls: Vec<String>,
    }

    impl UrlOpener for FakeUrlOpener {
        fn open_url(&mut self, url: &str) -> Result<(), String> {
            self.opened_urls.push(url.to_string());
            Ok(())
        }
    }

    fn session_key() -> SessionKey {
        SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("project"),
            ConversationId::new("conversation"),
        )
    }
}
