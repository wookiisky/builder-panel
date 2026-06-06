//! 终端交互 adapter。

use crate::domain::agent_session::{JumpTarget, SessionKey};
use crate::domain::app_error::{AppError, AppErrorCode, FallbackAction};
use crate::ports::jump_target_port::JumpTargetPort;

/// 终端跳回 adapter。
pub struct TerminalJumpAdapter {
    /// 已记录的跳回请求。
    recorded_jumps: Vec<(SessionKey, JumpTarget)>,
    /// 下一次跳回是否失败。
    fail_next_jump: bool,
}

impl TerminalJumpAdapter {
    /// 创建终端跳回 adapter。
    pub fn new() -> Self {
        Self {
            recorded_jumps: Vec::new(),
            fail_next_jump: false,
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

impl Default for TerminalJumpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl JumpTargetPort for TerminalJumpAdapter {
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::TerminalJumpAdapter;
    use crate::domain::agent_session::{
        AgentKind, ConversationId, JumpTarget, ProjectId, SessionKey,
    };
    use crate::domain::app_error::FallbackAction;
    use crate::ports::jump_target_port::JumpTargetPort;

    #[test]
    fn records_jump_without_sending_reply() {
        let mut adapter = TerminalJumpAdapter::new();
        let key = session_key();
        let target = JumpTarget {
            label: "Ghostty".to_string(),
            location: "window:1".to_string(),
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

    fn session_key() -> SessionKey {
        SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("project"),
            ConversationId::new("conversation"),
        )
    }
}
