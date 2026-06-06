//! 回复发送抽象边界。

use crate::domain::agent_interaction::{InteractionId, ReplyTarget};
use crate::domain::agent_session::SessionKey;
use crate::domain::app_error::AppError;

/// 开放性文本回复发送端口。
pub trait ReplySenderPort {
    /// 发送已通过应用服务校验的文本回复。
    fn send_reply(
        &mut self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
        reply_target: &ReplyTarget,
        content: &str,
    ) -> Result<(), AppError>;
}
