//! Session 实时更新通知边界。

use serde::{Deserialize, Serialize};

use crate::domain::agent_session::{AgentKind, SessionKey};
use crate::domain::usage::UnixMillis;

/// Session 实时更新事件名。
pub const SESSION_UPDATED_EVENT: &str = "session_updated";

/// Session 运行时来源。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRuntimeSource {
    /// Codex CLI runtime。
    CodexCli,
    /// Codex APP runtime。
    CodexApp,
}

impl SessionRuntimeSource {
    /// 从 agent 类型推导当前支持的运行时来源。
    pub fn from_agent_kind(agent_kind: &AgentKind) -> Option<Self> {
        match agent_kind {
            AgentKind::CodexCli => Some(Self::CodexCli),
            AgentKind::CodexApp => Some(Self::CodexApp),
            AgentKind::ClaudeCodeApp | AgentKind::ClaudeCodeCli => None,
        }
    }
}

/// Session 更新影响区域。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionUpdateArea {
    /// Session 摘要、状态或动作变化。
    Session,
    /// 过程时间线变化。
    Timeline,
    /// Session 和时间线均有变化。
    Both,
}

/// 清洗后的 session 更新通知。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionUpdateNotification {
    /// 更新来源。
    pub runtime_source: SessionRuntimeSource,
    /// 所属 session。
    pub session_key: SessionKey,
    /// 影响区域。
    pub changed_area: SessionUpdateArea,
    /// 更新时间。
    pub updated_at: UnixMillis,
}

/// Session 更新发布端口。
pub trait SessionUpdateSinkPort: Send + Sync {
    /// 发布清洗后的 session 更新。
    fn publish_session_update(&self, notification: SessionUpdateNotification);
}

/// 空发布器，用于测试和无 Tauri 边界场景。
#[derive(Default)]
pub struct NoopSessionUpdateSink;

impl SessionUpdateSinkPort for NoopSessionUpdateSink {
    fn publish_session_update(&self, _notification: SessionUpdateNotification) {}
}
