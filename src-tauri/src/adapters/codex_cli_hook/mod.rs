//! Codex CLI hook adapter。

use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::adapters::bridge::codec::{
    BridgeCommandType, BridgeDirectivePayload, BridgeErrorCode, BridgeErrorPayload,
    BridgeHookEventName, BridgeRequestEnvelope, BridgeResponseEnvelope, ValidatedHookPayload,
};
use crate::adapters::bridge::transport::default_bridge_location;
use crate::adapters::codex_app::{handle_codex_app_bridge_request, CodexAppRuntime};
use crate::adapters::timeline::InMemoryProcessTimelineCache;
use crate::domain::agent_event::{
    ActivityUpdatedEvent, AgentEvent, ApprovalRequestedEvent, FailedEvent, SessionStartedEvent,
    TurnCompletedEvent,
};
use crate::domain::agent_interaction::{
    AgentInteraction, ApprovalInteraction, HookDirectiveTarget, InteractionId, InteractionStatus,
    ReplyTarget,
};
use crate::domain::agent_session::{
    AgentKind, ConversationId, ProjectId, SessionCapabilities, SessionKey,
};
use crate::domain::app_error::{AppError, AppErrorCode, FallbackAction};
use crate::domain::session_state::SessionState;
use crate::domain::usage::{UnixMillis, UsageSnapshot};
use crate::domain::view_model::{
    session_detail_view_model, session_list_view_models, SessionDetailViewModel,
    SessionListItemViewModel,
};
use crate::ports::agent_adapter_port::ApprovalDecision;
use crate::ports::process_timeline_port::{
    ProcessTimelineItem, ProcessTimelineReaderPort, ProcessTimelineReleasePort,
};

const APPROVAL_WAIT_TIMEOUT: Duration = Duration::from_secs(60 * 60);

/// Codex CLI hook adapter。
pub struct CodexCliHookAdapter;

impl CodexCliHookAdapter {
    /// 将已清洗 hook payload 转换为归一领域事件。
    pub fn events_from_payload(
        request_id: &str,
        payload: &ValidatedHookPayload,
        updated_at: UnixMillis,
    ) -> Result<Vec<AgentEvent>, CodexCliHookAdapterError> {
        if payload.agent_kind != AgentKind::CodexCli {
            return Err(CodexCliHookAdapterError::AgentMismatch);
        }

        let session_key = session_key(payload);
        let event = match payload.hook_event_name {
            BridgeHookEventName::SessionStart => {
                AgentEvent::SessionStarted(started_event(payload, session_key, updated_at))
            }
            BridgeHookEventName::UserPromptSubmit => {
                AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
                    session_key,
                    summary: prompt_summary(payload),
                    updated_at,
                })
            }
            BridgeHookEventName::PreToolUse => AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
                session_key,
                summary: tool_summary("准备执行工具", payload),
                updated_at,
            }),
            BridgeHookEventName::PermissionRequest => {
                return Ok(vec![
                    AgentEvent::SessionStarted(started_event(
                        payload,
                        session_key.clone(),
                        updated_at,
                    )),
                    AgentEvent::ApprovalRequested(ApprovalRequestedEvent {
                        session_key: session_key.clone(),
                        interaction: ApprovalInteraction {
                            interaction_id: interaction_id(request_id),
                            session_key,
                            created_at: updated_at,
                            expires_at: None,
                            reply_target: ReplyTarget::HookDirective(HookDirectiveTarget {
                                request_id: request_id.to_string(),
                            }),
                            status: InteractionStatus::Pending,
                            request_summary: tool_summary("Codex 请求权限", payload),
                        },
                        updated_at,
                    }),
                ]);
            }
            BridgeHookEventName::PostToolUse => AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
                session_key,
                summary: tool_summary("工具执行完成", payload),
                updated_at,
            }),
            BridgeHookEventName::Stop => AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key,
                summary: stop_summary(payload),
                updated_at,
            }),
            BridgeHookEventName::Notification | BridgeHookEventName::SessionEnd => {
                return Err(CodexCliHookAdapterError::UnsupportedEvent);
            }
        };

        Ok(vec![event])
    }
}

/// Codex CLI hook adapter 错误。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodexCliHookAdapterError {
    /// payload 来源不是 Codex CLI。
    AgentMismatch,
    /// Codex CLI 当前不支持该事件。
    UnsupportedEvent,
}

/// Codex CLI hook runtime。
pub struct CodexCliHookRuntime {
    /// 当前折叠后的 session 状态。
    session_state: SessionState,
    /// 等待 UI 决策的 hook 审批。
    pending_approvals: BTreeMap<InteractionId, PendingHookApproval>,
    /// 托管 hook 事件时间线缓存。
    timeline_cache: InMemoryProcessTimelineCache,
}

impl CodexCliHookRuntime {
    /// 创建空 Codex CLI hook runtime。
    pub fn empty() -> Self {
        Self {
            session_state: SessionState::empty(),
            pending_approvals: BTreeMap::new(),
            timeline_cache: InMemoryProcessTimelineCache::new(),
        }
    }

    /// 返回 session 列表 view model。
    pub fn session_list(&self) -> Vec<SessionListItemViewModel> {
        session_list_view_models(&self.session_state)
    }

    /// 返回 session 详情 view model。
    pub fn session_detail(&self, session_key: &SessionKey) -> Option<SessionDetailViewModel> {
        self.session_state
            .sessions
            .get(session_key)
            .map(session_detail_view_model)
    }

    /// 返回当前 session 状态。
    pub fn session_state(&self) -> &SessionState {
        &self.session_state
    }

    /// 读取指定 session 的过程事件时间线。
    pub fn read_timeline(
        &self,
        session_key: &SessionKey,
    ) -> Result<Vec<ProcessTimelineItem>, AppError> {
        self.timeline_cache.read_timeline(session_key)
    }

    /// 释放指定 session 的大文本时间线缓存。
    pub fn release_timeline_large_texts(
        &mut self,
        session_key: &SessionKey,
    ) -> Result<usize, AppError> {
        self.timeline_cache.release_large_texts(session_key)
    }

    /// 应用 Codex hook request，并返回可能需要等待的审批。
    pub fn apply_hook_request(
        &mut self,
        request: &BridgeRequestEnvelope,
        updated_at: UnixMillis,
    ) -> Result<Option<PendingHookApprovalWait>, AppError> {
        if request.command_type != BridgeCommandType::ProcessAgentHook {
            return Err(protocol_error("不支持的 bridge command"));
        }

        let payload = &request.payload.validated_payload;
        let events =
            CodexCliHookAdapter::events_from_payload(&request.request_id, payload, updated_at)
                .map_err(|_| protocol_error("Codex hook payload 不受支持"))?;

        for event in events {
            self.timeline_cache.record_agent_event(&event)?;
            self.session_state = self.session_state.apply_event(event);
        }

        if payload.hook_event_name != BridgeHookEventName::PermissionRequest {
            return Ok(None);
        }

        let interaction_id = interaction_id(&request.request_id);
        let session_key = session_key(payload);
        let waiter = PendingHookApprovalWaiter::new();
        self.expire_stale_approvals_for_session(&session_key, &interaction_id);
        let replaced = self.pending_approvals.insert(
            interaction_id.clone(),
            PendingHookApproval {
                session_key: session_key.clone(),
                waiter: waiter.clone(),
            },
        );
        if let Some(replaced) = replaced {
            replaced.waiter.expire();
        }

        Ok(Some(PendingHookApprovalWait {
            session_key,
            interaction_id,
            waiter,
        }))
    }

    /// 提交 UI 审批决策并唤醒等待中的 hook。
    pub fn resolve_approval(
        &mut self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
        decision: ApprovalDecision,
    ) -> Result<(), AppError> {
        let Some(pending) = self.pending_approvals.get(interaction_id) else {
            return Err(invalid_interaction("审批交互不存在或已处理"));
        };

        if pending.session_key != *session_key {
            return Err(invalid_interaction("审批交互不属于当前会话"));
        }

        if !self.current_pending_approval_matches(session_key, interaction_id) {
            if let Some(pending) = self.pending_approvals.remove(interaction_id) {
                pending.waiter.expire();
            }
            return Err(invalid_interaction("审批交互已不是当前等待项"));
        }

        if !pending.waiter.resolve(decision) {
            self.pending_approvals.remove(interaction_id);
            self.fail_expired_approval(session_key);
            return Err(invalid_interaction("审批交互已过期"));
        }
        self.pending_approvals.remove(interaction_id);

        self.session_state =
            self.session_state
                .apply_event(AgentEvent::TurnCompleted(TurnCompletedEvent {
                    session_key: session_key.clone(),
                    summary: Some(match decision {
                        ApprovalDecision::Allow => "Codex 审批已允许".to_string(),
                        ApprovalDecision::AllowAndRemember => {
                            "Codex 审批已允许，本入口不支持记住".to_string()
                        }
                        ApprovalDecision::Deny => "Codex 审批已拒绝".to_string(),
                    }),
                    updated_at: unix_now(),
                }));

        Ok(())
    }

    /// 将等待超时的审批标记为失败并清理 pending。
    pub fn expire_approval(&mut self, session_key: &SessionKey, interaction_id: &InteractionId) {
        let Some(pending) = self.pending_approvals.get(interaction_id) else {
            return;
        };

        if pending.session_key != *session_key {
            return;
        }

        self.pending_approvals.remove(interaction_id);
        self.fail_expired_approval(session_key);
    }

    fn expire_stale_approvals_for_session(
        &mut self,
        session_key: &SessionKey,
        current_interaction_id: &InteractionId,
    ) {
        let stale_ids: Vec<InteractionId> = self
            .pending_approvals
            .iter()
            .filter(|(interaction_id, pending)| {
                pending.session_key == *session_key && *interaction_id != current_interaction_id
            })
            .map(|(interaction_id, _)| interaction_id.clone())
            .collect();

        for stale_id in stale_ids {
            if let Some(pending) = self.pending_approvals.remove(&stale_id) {
                pending.waiter.expire();
            }
        }
    }

    fn current_pending_approval_matches(
        &self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
    ) -> bool {
        let Some(session) = self.session_state.sessions.get(session_key) else {
            return false;
        };
        let Some(AgentInteraction::Approval(interaction)) = &session.pending_interaction else {
            return false;
        };

        interaction.interaction_id == *interaction_id
    }

    fn fail_expired_approval(&mut self, session_key: &SessionKey) {
        self.session_state = self
            .session_state
            .apply_event(AgentEvent::Failed(FailedEvent {
                session_key: session_key.clone(),
                error: AppError::new(
                    AppErrorCode::BridgeUnavailable,
                    "Codex 审批等待超时",
                    None,
                    true,
                    Some(FallbackAction::RetryLater),
                ),
                updated_at: unix_now(),
            }));
    }
}

impl ProcessTimelineReaderPort for CodexCliHookRuntime {
    fn read_timeline(
        &self,
        session_key: &SessionKey,
    ) -> Result<Vec<ProcessTimelineItem>, AppError> {
        self.timeline_cache.read_timeline(session_key)
    }
}

impl ProcessTimelineReleasePort for CodexCliHookRuntime {
    fn release_large_texts(&mut self, session_key: &SessionKey) -> Result<usize, AppError> {
        self.timeline_cache.release_large_texts(session_key)
    }
}

/// 处理 bridge request。
pub fn handle_bridge_request(
    runtime: Arc<Mutex<CodexCliHookRuntime>>,
    request: BridgeRequestEnvelope,
) -> BridgeResponseEnvelope {
    let request_id = request.request_id.clone();
    let waiter = {
        let runtime_lock = runtime.lock();
        let Ok(mut runtime) = runtime_lock else {
            return error_response(request_id, "Codex runtime 锁已损坏");
        };

        match runtime.apply_hook_request(&request, unix_now()) {
            Ok(waiter) => waiter,
            Err(error) => return error_response(request_id, &error.user_message),
        }
    };

    let Some(wait) = waiter else {
        return BridgeResponseEnvelope::ack(request_id);
    };

    match wait.waiter.wait(APPROVAL_WAIT_TIMEOUT) {
        PendingHookApprovalWaitResult::Resolved(
            ApprovalDecision::Allow | ApprovalDecision::AllowAndRemember,
        ) => BridgeResponseEnvelope::directive(
            request_id,
            BridgeDirectivePayload::allow(AgentKind::CodexCli),
        ),
        PendingHookApprovalWaitResult::Resolved(ApprovalDecision::Deny) => {
            BridgeResponseEnvelope::directive(
                request_id,
                BridgeDirectivePayload::deny(
                    AgentKind::CodexCli,
                    Some("用户拒绝 Codex 权限请求".to_string()),
                    None,
                ),
            )
        }
        PendingHookApprovalWaitResult::Expired => {
            if let Ok(mut runtime) = runtime.lock() {
                runtime.expire_approval(&wait.session_key, &wait.interaction_id);
            }
            BridgeResponseEnvelope::error(
                request_id,
                BridgeErrorPayload {
                    code: BridgeErrorCode::BridgeUnavailable,
                    message: "Codex 审批等待超时".to_string(),
                },
            )
        }
    }
}

/// 启动 Codex CLI hook bridge server。
#[cfg(unix)]
pub fn start_codex_cli_bridge_server(
    runtime: Arc<Mutex<CodexCliHookRuntime>>,
    codex_app_runtime: Arc<Mutex<CodexAppRuntime>>,
) -> Result<thread::JoinHandle<()>, crate::adapters::bridge::transport::BridgeTransportError> {
    let server = crate::adapters::bridge::transport::unix_transport::UnixBridgeServer::bind(
        default_bridge_location(),
    )?;

    Ok(thread::spawn(move || loop {
        let runtime = Arc::clone(&runtime);
        let codex_app_runtime = Arc::clone(&codex_app_runtime);
        if server
            .accept_one_on_thread(move |request| {
                match request.payload.validated_payload.agent_kind {
                    AgentKind::CodexApp => {
                        handle_codex_app_bridge_request(codex_app_runtime, request)
                    }
                    AgentKind::CodexCli => handle_bridge_request(runtime, request),
                    AgentKind::ClaudeCodeApp | AgentKind::ClaudeCodeCli => {
                        BridgeResponseEnvelope::error(
                            request.request_id,
                            BridgeErrorPayload {
                                code: BridgeErrorCode::AgentProtocolUnsupported,
                                message: "当前 bridge 不处理该 agent".to_string(),
                            },
                        )
                    }
                }
            })
            .is_err()
        {
            thread::sleep(Duration::from_millis(100));
        }
    }))
}

/// Windows bridge server 当前不做本机验证。
#[cfg(windows)]
pub fn start_codex_cli_bridge_server(
    _runtime: Arc<Mutex<CodexCliHookRuntime>>,
    _codex_app_runtime: Arc<Mutex<CodexAppRuntime>>,
) -> Result<thread::JoinHandle<()>, crate::adapters::bridge::transport::BridgeTransportError> {
    Ok(thread::spawn(|| {}))
}

/// 等待 hook 审批的上下文。
pub struct PendingHookApprovalWait {
    /// 所属会话。
    session_key: SessionKey,
    /// 所属交互。
    interaction_id: InteractionId,
    /// 等待器。
    waiter: PendingHookApprovalWaiter,
}

/// 等待中的 hook 审批。
struct PendingHookApproval {
    /// 所属会话。
    session_key: SessionKey,
    /// UI 决策等待器。
    waiter: PendingHookApprovalWaiter,
}

/// hook 审批等待器。
#[derive(Clone)]
pub struct PendingHookApprovalWaiter {
    /// 共享决策状态。
    inner: Arc<(Mutex<PendingHookApprovalState>, Condvar)>,
}

/// hook 审批等待状态。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingHookApprovalState {
    /// 等待用户决策。
    Waiting,
    /// 已收到用户决策。
    Resolved(ApprovalDecision),
    /// 等待已过期。
    Expired,
}

/// hook 审批等待结果。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingHookApprovalWaitResult {
    /// 已收到用户决策。
    Resolved(ApprovalDecision),
    /// 等待已过期。
    Expired,
}

impl PendingHookApprovalWaiter {
    /// 创建空等待器。
    fn new() -> Self {
        Self {
            inner: Arc::new((
                Mutex::new(PendingHookApprovalState::Waiting),
                Condvar::new(),
            )),
        }
    }

    /// 写入决策并唤醒等待线程。
    fn resolve(&self, decision: ApprovalDecision) -> bool {
        let (lock, condvar) = &*self.inner;
        let Ok(mut state) = lock.lock() else {
            return false;
        };
        if *state != PendingHookApprovalState::Waiting {
            return false;
        }

        *state = PendingHookApprovalState::Resolved(decision);
        condvar.notify_all();
        true
    }

    /// 标记等待已过期并唤醒等待线程。
    fn expire(&self) {
        let (lock, condvar) = &*self.inner;
        let Ok(mut state) = lock.lock() else {
            return;
        };
        if *state != PendingHookApprovalState::Waiting {
            return;
        }

        *state = PendingHookApprovalState::Expired;
        condvar.notify_all();
    }

    /// 等待用户决策。
    fn wait(&self, timeout: Duration) -> PendingHookApprovalWaitResult {
        let (lock, condvar) = &*self.inner;
        let Ok(slot) = lock.lock() else {
            return PendingHookApprovalWaitResult::Expired;
        };
        let Ok((slot, _)) = condvar.wait_timeout_while(slot, timeout, |state| {
            *state == PendingHookApprovalState::Waiting
        }) else {
            return PendingHookApprovalWaitResult::Expired;
        };

        let mut state = slot;
        match *state {
            PendingHookApprovalState::Resolved(decision) => {
                PendingHookApprovalWaitResult::Resolved(decision)
            }
            PendingHookApprovalState::Waiting | PendingHookApprovalState::Expired => {
                *state = PendingHookApprovalState::Expired;
                PendingHookApprovalWaitResult::Expired
            }
        }
    }
}

fn started_event(
    payload: &ValidatedHookPayload,
    session_key: SessionKey,
    updated_at: UnixMillis,
) -> SessionStartedEvent {
    SessionStartedEvent {
        session_key,
        project_label: project_label(&payload.cwd),
        conversation_label: payload.session_id.clone(),
        title: payload.model.clone(),
        summary: Some(start_summary(payload)),
        capabilities: codex_cli_capabilities(),
        usage: UsageSnapshot::unavailable(),
        updated_at,
    }
}

fn session_key(payload: &ValidatedHookPayload) -> SessionKey {
    SessionKey::new(
        AgentKind::CodexCli,
        ProjectId::new(payload.cwd.clone()),
        ConversationId::new(payload.session_id.clone()),
    )
}

fn interaction_id(request_id: &str) -> InteractionId {
    InteractionId::new(format!("codex-hook-{request_id}"))
}

fn codex_cli_capabilities() -> SessionCapabilities {
    SessionCapabilities {
        can_jump: true,
        can_send_reply: false,
        can_resolve_approval: true,
        can_create_followup_turn: false,
        can_view_process_timeline: true,
    }
}

fn project_label(cwd: &str) -> String {
    cwd.rsplit('/')
        .next()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(cwd)
        .to_string()
}

fn start_summary(payload: &ValidatedHookPayload) -> String {
    let Some(model) = &payload.model else {
        return "Codex CLI 会话已启动".to_string();
    };

    format!("Codex CLI 会话已启动，模型 {model}")
}

fn prompt_summary(payload: &ValidatedHookPayload) -> String {
    let Some(prompt) = &payload.prompt else {
        return "用户提交了 Codex prompt".to_string();
    };

    format!("用户提交了 Codex prompt：{}", truncate(prompt, 120))
}

fn tool_summary(prefix: &str, payload: &ValidatedHookPayload) -> String {
    let tool_name = payload.tool_name.as_deref().unwrap_or("未知工具");
    let Some(tool_input) = &payload.tool_input else {
        return format!("{prefix}：{tool_name}");
    };

    format!("{prefix}：{tool_name} {}", summarize_tool_input(tool_input))
}

fn stop_summary(payload: &ValidatedHookPayload) -> Option<String> {
    payload
        .last_assistant_message
        .as_ref()
        .map(|message| format!("Codex turn 完成：{}", truncate(message, 120)))
        .or_else(|| Some("Codex turn 已完成".to_string()))
}

fn summarize_tool_input(value: &Value) -> String {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(command)) = object.get("command") {
                return truncate(command, 120);
            }
            if let Some(Value::String(path)) = object.get("path") {
                return truncate(path, 120);
            }
            format!("包含 {} 个字段", object.len())
        }
        _ => "输入已清洗".to_string(),
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for (index, character) in value.chars().enumerate() {
        if index >= max_chars {
            output.push_str("...");
            return output;
        }
        output.push(character);
    }

    output
}

fn unix_now() -> UnixMillis {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default();
    UnixMillis::new(millis)
}

fn protocol_error(message: &str) -> AppError {
    AppError::new(
        AppErrorCode::AgentProtocolUnsupported,
        message,
        None,
        false,
        Some(FallbackAction::ViewReadOnly),
    )
}

fn invalid_interaction(message: &str) -> AppError {
    AppError::new(
        AppErrorCode::UnsupportedReplyTarget,
        message,
        None,
        false,
        Some(FallbackAction::ViewReadOnly),
    )
}

fn error_response(request_id: String, message: &str) -> BridgeResponseEnvelope {
    BridgeResponseEnvelope::error(
        request_id,
        BridgeErrorPayload {
            code: BridgeErrorCode::AgentProtocolUnsupported,
            message: message.to_string(),
        },
    )
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use serde_json::json;

    use super::{
        handle_bridge_request, CodexCliHookAdapter, CodexCliHookRuntime, PendingHookApprovalWaiter,
    };
    use crate::adapters::bridge::codec::{
        BridgeHookEventName, BridgeRequestEnvelope, BridgeResultType, ValidatedHookPayload,
    };
    use crate::domain::agent_event::AgentEvent;
    use crate::domain::agent_session::{AgentKind, SessionStatus};
    use crate::domain::usage::UnixMillis;
    use crate::ports::agent_adapter_port::ApprovalDecision;
    use crate::ports::process_timeline_port::ProcessTimelineEventKind;

    #[test]
    fn session_start_maps_to_started_event_without_raw_payload() {
        let payload = payload(BridgeHookEventName::SessionStart);

        let events =
            CodexCliHookAdapter::events_from_payload("request-1", &payload, UnixMillis::new(1))
                .expect("payload should map");

        let AgentEvent::SessionStarted(event) = &events[0] else {
            panic!("event should be started");
        };
        assert_eq!(event.session_key.agent_kind, AgentKind::CodexCli);
        assert_eq!(event.project_label, "builder-panel");
        assert!(event.capabilities.can_resolve_approval);
        assert!(!event.capabilities.can_send_reply);
        assert!(event.capabilities.can_view_process_timeline);
        assert!(!serde_json::to_string(&events)
            .unwrap()
            .contains("tool_input"));
    }

    #[test]
    fn permission_request_maps_to_started_and_pending_approval() {
        let mut payload = payload(BridgeHookEventName::PermissionRequest);
        payload.tool_name = Some("Bash".to_string());
        payload.tool_input = Some(json!({"command": "cargo test"}));

        let events =
            CodexCliHookAdapter::events_from_payload("request-1", &payload, UnixMillis::new(1))
                .expect("payload should map");

        assert_eq!(events.len(), 2);
        let AgentEvent::ApprovalRequested(event) = &events[1] else {
            panic!("event should be approval");
        };
        assert_eq!(
            event.interaction.request_summary,
            "Codex 请求权限：Bash cargo test"
        );
        assert_eq!(
            event.interaction.interaction_id.value,
            "codex-hook-request-1"
        );
    }

    #[test]
    fn runtime_applies_non_blocking_event_and_returns_ack() {
        let runtime = Arc::new(Mutex::new(CodexCliHookRuntime::empty()));
        let request = request(BridgeHookEventName::SessionStart, "request-1");

        let response = handle_bridge_request(Arc::clone(&runtime), request);

        assert_eq!(response.result_type, BridgeResultType::Ack);
        let runtime = runtime.lock().expect("runtime should lock");
        assert_eq!(runtime.session_state().sessions.len(), 1);
    }

    #[test]
    fn runtime_records_managed_hook_events_to_timeline() {
        let mut runtime = CodexCliHookRuntime::empty();
        let request = request(BridgeHookEventName::PreToolUse, "request-1");

        runtime
            .apply_hook_request(&request, UnixMillis::new(1))
            .expect("request should apply");

        let session_key = runtime
            .session_state()
            .sessions
            .keys()
            .next()
            .expect("session should exist")
            .clone();
        let items = runtime
            .read_timeline(&session_key)
            .expect("timeline should read");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ProcessTimelineEventKind::Activity);
        assert!(items[0].body.contains("准备执行工具"));
    }

    #[test]
    fn runtime_waits_for_approval_and_returns_directive() {
        let runtime = Arc::new(Mutex::new(CodexCliHookRuntime::empty()));
        let request = request(BridgeHookEventName::PermissionRequest, "request-1");
        let worker_runtime = Arc::clone(&runtime);
        let worker = thread::spawn(move || handle_bridge_request(worker_runtime, request));

        wait_until_pending(&runtime);
        let pending = {
            let runtime = runtime.lock().expect("runtime should lock");
            let session = runtime
                .session_state()
                .sessions
                .values()
                .find(|session| session.status == SessionStatus::WaitingForApproval)
                .expect("approval session should exist");
            let interaction = session
                .pending_interaction
                .as_ref()
                .expect("pending interaction should exist");
            (session.session_key.clone(), interaction_id(interaction))
        };
        runtime
            .lock()
            .expect("runtime should lock")
            .resolve_approval(&pending.0, &pending.1, ApprovalDecision::Allow)
            .expect("approval should resolve");

        let response = worker.join().expect("worker should join");

        assert_eq!(response.result_type, BridgeResultType::Directive);
        assert_eq!(
            response.payload.expect("payload should exist").agent_kind,
            AgentKind::CodexCli
        );
    }

    #[test]
    fn waiter_times_out_without_decision() {
        let waiter = PendingHookApprovalWaiter::new();

        let result = waiter.wait(Duration::from_millis(1));

        assert_eq!(result, super::PendingHookApprovalWaitResult::Expired);
    }

    #[test]
    fn expired_approval_clears_pending_and_rejects_late_decision() {
        let mut runtime = CodexCliHookRuntime::empty();
        let request = request(BridgeHookEventName::PermissionRequest, "request-timeout");
        let wait = runtime
            .apply_hook_request(&request, UnixMillis::new(1))
            .expect("request should apply")
            .expect("approval should wait");

        assert_eq!(
            wait.waiter.wait(Duration::from_millis(1)),
            super::PendingHookApprovalWaitResult::Expired
        );
        runtime.expire_approval(&wait.session_key, &wait.interaction_id);

        let session = runtime
            .session_state()
            .sessions
            .get(&wait.session_key)
            .expect("session should exist");
        assert_eq!(session.status, SessionStatus::Failed);
        assert!(session.pending_interaction.is_none());
        assert!(runtime
            .resolve_approval(
                &wait.session_key,
                &wait.interaction_id,
                ApprovalDecision::Allow
            )
            .is_err());
    }

    #[test]
    fn late_decision_before_expire_still_clears_session_pending() {
        let mut runtime = CodexCliHookRuntime::empty();
        let request = request(BridgeHookEventName::PermissionRequest, "request-late");
        let wait = runtime
            .apply_hook_request(&request, UnixMillis::new(1))
            .expect("request should apply")
            .expect("approval should wait");

        assert_eq!(
            wait.waiter.wait(Duration::from_millis(1)),
            super::PendingHookApprovalWaitResult::Expired
        );
        assert!(runtime
            .resolve_approval(
                &wait.session_key,
                &wait.interaction_id,
                ApprovalDecision::Allow
            )
            .is_err());
        runtime.expire_approval(&wait.session_key, &wait.interaction_id);

        let session = runtime
            .session_state()
            .sessions
            .get(&wait.session_key)
            .expect("session should exist");
        assert_eq!(session.status, SessionStatus::Failed);
        assert!(session.pending_interaction.is_none());
    }

    #[test]
    fn newer_approval_expires_old_waiter_for_same_session() {
        let mut runtime = CodexCliHookRuntime::empty();
        let first_wait = runtime
            .apply_hook_request(
                &request(BridgeHookEventName::PermissionRequest, "request-old"),
                UnixMillis::new(1),
            )
            .expect("first request should apply")
            .expect("first approval should wait");
        let second_wait = runtime
            .apply_hook_request(
                &request(BridgeHookEventName::PermissionRequest, "request-new"),
                UnixMillis::new(2),
            )
            .expect("second request should apply")
            .expect("second approval should wait");

        assert_eq!(
            first_wait.waiter.wait(Duration::from_millis(1)),
            super::PendingHookApprovalWaitResult::Expired
        );
        assert!(runtime
            .resolve_approval(
                &first_wait.session_key,
                &first_wait.interaction_id,
                ApprovalDecision::Allow
            )
            .is_err());
        runtime
            .resolve_approval(
                &second_wait.session_key,
                &second_wait.interaction_id,
                ApprovalDecision::Allow,
            )
            .expect("current approval should resolve");

        let session = runtime
            .session_state()
            .sessions
            .get(&second_wait.session_key)
            .expect("session should exist");
        assert_eq!(session.status, SessionStatus::Completed);
        assert!(session.pending_interaction.is_none());
    }

    fn payload(event_name: BridgeHookEventName) -> ValidatedHookPayload {
        ValidatedHookPayload {
            agent_kind: AgentKind::CodexCli,
            hook_event_name: event_name,
            cwd: "/tmp/builder-panel".to_string(),
            session_id: "session-1".to_string(),
            model: Some("gpt-5.4".to_string()),
            permission_mode: Some("default".to_string()),
            transcript_path: None,
            terminal_app: None,
            terminal_session_id: None,
            terminal_tty: None,
            terminal_title: None,
            turn_id: None,
            tool_name: None,
            tool_input: None,
            prompt: Some("实现阶段 4".to_string()),
            last_assistant_message: None,
            permission_suggestions: None,
        }
    }

    fn request(event_name: BridgeHookEventName, request_id: &str) -> BridgeRequestEnvelope {
        BridgeRequestEnvelope::process_agent_hook(request_id.to_string(), payload(event_name))
    }

    fn wait_until_pending(runtime: &Arc<Mutex<CodexCliHookRuntime>>) {
        for _ in 0..50 {
            let has_pending = runtime
                .lock()
                .expect("runtime should lock")
                .session_state()
                .sessions
                .values()
                .any(|session| session.status == SessionStatus::WaitingForApproval);
            if has_pending {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }

        panic!("pending approval should appear");
    }

    fn interaction_id(
        interaction: &crate::domain::agent_interaction::AgentInteraction,
    ) -> crate::domain::agent_interaction::InteractionId {
        match interaction {
            crate::domain::agent_interaction::AgentInteraction::Approval(interaction) => {
                interaction.interaction_id.clone()
            }
            crate::domain::agent_interaction::AgentInteraction::Choice(interaction) => {
                interaction.interaction_id.clone()
            }
            crate::domain::agent_interaction::AgentInteraction::TextReply(interaction) => {
                interaction.interaction_id.clone()
            }
        }
    }
}
