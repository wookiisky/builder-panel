//! 会话应用服务。

use crate::adapters::mock_agent::MockAgentRuntime;
use crate::domain::agent_session::SessionKey;
use crate::domain::view_model::{SessionDetailViewModel, SessionListItemViewModel};

/// 会话应用服务。
pub struct SessionService<'a> {
    /// Mock agent runtime。
    runtime: &'a MockAgentRuntime,
}

impl<'a> SessionService<'a> {
    /// 创建会话应用服务。
    pub fn new(runtime: &'a MockAgentRuntime) -> Self {
        Self { runtime }
    }

    /// 读取 session 列表。
    pub fn list_sessions(&self) -> Vec<SessionListItemViewModel> {
        self.runtime.session_list()
    }

    /// 读取 session 详情。
    pub fn session_detail(&self, session_key: &SessionKey) -> Option<SessionDetailViewModel> {
        self.runtime.session_detail(session_key)
    }
}

#[cfg(test)]
mod tests {
    use super::SessionService;
    use crate::adapters::mock_agent::MockAgentRuntime;

    #[test]
    fn service_returns_mock_sessions() {
        let runtime = MockAgentRuntime::stage3_default();
        let service = SessionService::new(&runtime);
        let sessions = service.list_sessions();

        assert_eq!(sessions.len(), 5);
        assert!(sessions
            .iter()
            .any(|session| session.project_label == "Mock Alpha"));
    }

    #[test]
    fn service_returns_detail_for_known_session() {
        let runtime = MockAgentRuntime::stage3_default();
        let service = SessionService::new(&runtime);
        let key = service.list_sessions()[0].session_key.clone();
        let detail = service
            .session_detail(&key)
            .expect("detail should exist for known session");

        assert!(!detail.header.is_empty());
    }
}
