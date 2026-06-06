//! 跳回目标抽象边界。

use crate::domain::agent_session::{JumpTarget, SessionKey};
use crate::domain::app_error::AppError;

/// 跳回目标抽象边界。
pub trait JumpTargetPort {
    /// 跳回指定 session 的 agent 入口。
    fn jump_to_session(
        &mut self,
        session_key: &SessionKey,
        jump_target: &JumpTarget,
    ) -> Result<(), AppError>;
}
