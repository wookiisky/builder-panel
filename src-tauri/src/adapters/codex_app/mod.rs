//! Codex APP adapter。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{json, Value};

use crate::adapters::bridge::codec::{
    BridgeCommandType, BridgeDirectivePayload, BridgeErrorCode, BridgeErrorPayload,
    BridgeHookEventName, BridgeRequestEnvelope, BridgeResponseEnvelope, ValidatedHookPayload,
};
use crate::adapters::timeline::InMemoryProcessTimelineCache;
use crate::domain::agent_event::{
    ActivityUpdatedEvent, AgentEvent, AnswerRequestedEvent, ApprovalRequestedEvent, DetachedEvent,
    FailedEvent, InteractionCompletedEvent, JumpTargetUpdatedEvent, SessionStartedEvent,
    TurnCompletedEvent, UsageUpdatedEvent,
};
use crate::domain::agent_interaction::{
    AgentInteraction, AnswerInteraction, ApprovalInteraction, ChoiceInteraction,
    HookDirectiveTarget, InteractionChoice, InteractionId, InteractionStatus, ReplyTarget,
    StructuredRpcTarget, TextReplyInteraction,
};
use crate::domain::agent_session::{
    AgentKind, ConversationId, JumpTarget, ProjectId, SessionCapabilities, SessionKey,
    SessionStatus,
};
use crate::domain::app_error::{AppError, AppErrorCode, FallbackAction};
use crate::domain::session_state::SessionState;
use crate::domain::usage::{
    UnixMillis, UsageAmount, UsageSnapshot, UsageValue, VerifiedUsageValue,
};
use crate::domain::view_model::{
    session_detail_view_model, session_list_view_models, SessionDetailViewModel,
    SessionListItemViewModel,
};
use crate::ports::agent_adapter_port::ApprovalDecision;
use crate::ports::agent_adapter_port::ChoiceSubmission;
use crate::ports::process_timeline_port::{
    ProcessTimelineItem, ProcessTimelineReaderPort, ProcessTimelineReleasePort,
};

const REQUIRED_SCHEMA_FILES: [&str; 15] = [
    "v2/ThreadStartParams.json",
    "v2/ThreadStartResponse.json",
    "v2/TurnStartParams.json",
    "v2/TurnStartResponse.json",
    "v2/ThreadStartedNotification.json",
    "v2/TurnStartedNotification.json",
    "v2/AgentMessageDeltaNotification.json",
    "v2/ThreadTokenUsageUpdatedNotification.json",
    "v2/TurnCompletedNotification.json",
    "v2/ThreadStatusChangedNotification.json",
    "CommandExecutionRequestApprovalResponse.json",
    "FileChangeRequestApprovalResponse.json",
    "PermissionsRequestApprovalResponse.json",
    "ToolRequestUserInputResponse.json",
    "McpServerElicitationRequestResponse.json",
];

const APP_SERVER_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const APPROVAL_WAIT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const MAX_APP_SERVER_LINE_BYTES: usize = 8 * 1024 * 1024;

/// Codex APP app-server schema 探针结果。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodexAppSchemaProbe {
    /// 是否成功生成 schema。
    pub schema_available: bool,
    /// 已验证存在的 schema 文件。
    pub verified_schema_files: Vec<String>,
    /// 缺失的必需 schema 文件。
    pub missing_schema_files: Vec<String>,
    /// 可选诊断。
    pub diagnostic: Option<String>,
}

/// Codex APP app-server adapter。
pub struct CodexAppAdapter;

impl CodexAppAdapter {
    /// 生成当前 Codex app-server schema 并校验关键入口。
    pub fn probe_schema() -> CodexAppSchemaProbe {
        let out_dir = schema_probe_dir();
        if let Err(error) = fs::create_dir_all(&out_dir) {
            return CodexAppSchemaProbe {
                schema_available: false,
                verified_schema_files: Vec::new(),
                missing_schema_files: required_schema_files(),
                diagnostic: Some(error.to_string()),
            };
        }

        let output = Command::new("codex")
            .args([
                "app-server",
                "generate-json-schema",
                "--experimental",
                "--out",
            ])
            .arg(&out_dir)
            .output();

        let Ok(output) = output else {
            return CodexAppSchemaProbe {
                schema_available: false,
                verified_schema_files: Vec::new(),
                missing_schema_files: required_schema_files(),
                diagnostic: Some("无法执行 codex app-server generate-json-schema".to_string()),
            };
        };

        if !output.status.success() {
            return CodexAppSchemaProbe {
                schema_available: false,
                verified_schema_files: Vec::new(),
                missing_schema_files: required_schema_files(),
                diagnostic: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            };
        }

        schema_probe_from_dir(&out_dir)
    }

    /// 将 app-server JSON-RPC notification 转换为归一事件。
    pub fn event_from_notification(
        notification: &Value,
        cwd: &str,
        updated_at: UnixMillis,
    ) -> Result<Option<AgentEvent>, CodexAppAdapterError> {
        let object = notification
            .as_object()
            .ok_or(CodexAppAdapterError::NotObject)?;
        let method = required_string(object.get("method"), "method")?;
        let params = object
            .get("params")
            .ok_or(CodexAppAdapterError::MissingField("params"))?;

        match method.as_str() {
            "thread/started" => Ok(Some(started_from_thread(params, cwd, updated_at)?)),
            "turn/started" => Ok(Some(turn_activity(
                params,
                cwd,
                "Codex APP turn 已开始",
                updated_at,
            )?)),
            "item/agentMessage/delta" => Ok(Some(agent_message_delta(params, cwd, updated_at)?)),
            "thread/status/changed" => Ok(status_changed(params, cwd, updated_at)?),
            "thread/tokenUsage/updated" => Ok(Some(usage_updated(params, cwd, updated_at)?)),
            "turn/completed" => Ok(Some(turn_completed(params, cwd, updated_at)?)),
            _ => Ok(None),
        }
    }

    /// 将 Codex hook payload 转换为 Codex APP 归一事件。
    pub fn events_from_hook_payload(
        request_id: &str,
        payload: &ValidatedHookPayload,
        updated_at: UnixMillis,
    ) -> Result<Vec<AgentEvent>, CodexAppAdapterError> {
        if payload.agent_kind != AgentKind::CodexApp {
            return Err(CodexAppAdapterError::AgentMismatch);
        }

        let session_key = session_key(&payload.cwd, &payload.session_id);
        let event = match payload.hook_event_name {
            BridgeHookEventName::SessionStart => {
                return Ok(vec![
                    AgentEvent::SessionStarted(started_from_hook(
                        payload,
                        session_key.clone(),
                        updated_at,
                    )),
                    jump_target_event(session_key, &payload.session_id, updated_at),
                ]);
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
                summary: hook_tool_summary("Codex APP 准备执行工具", payload),
                updated_at,
            }),
            BridgeHookEventName::PermissionRequest => {
                return Ok(vec![
                    AgentEvent::SessionStarted(started_from_hook(
                        payload,
                        session_key.clone(),
                        updated_at,
                    )),
                    jump_target_event(session_key.clone(), &payload.session_id, updated_at),
                    AgentEvent::ApprovalRequested(ApprovalRequestedEvent {
                        session_key: session_key.clone(),
                        interaction: ApprovalInteraction {
                            interaction_id: hook_interaction_id(request_id),
                            session_key,
                            created_at: updated_at,
                            expires_at: None,
                            reply_target: ReplyTarget::HookDirective(HookDirectiveTarget {
                                request_id: request_id.to_string(),
                            }),
                            status: InteractionStatus::Pending,
                            request_summary: hook_tool_summary("Codex APP 请求权限", payload),
                        },
                        updated_at,
                    }),
                ]);
            }
            BridgeHookEventName::PostToolUse => AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
                session_key,
                summary: hook_tool_summary("Codex APP 工具执行完成", payload),
                updated_at,
            }),
            BridgeHookEventName::Stop => AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key,
                summary: payload
                    .last_assistant_message
                    .as_ref()
                    .map(|message| format!("Codex APP turn 完成：{}", truncate(message, 120))),
                updated_at,
            }),
            BridgeHookEventName::Notification | BridgeHookEventName::SessionEnd => {
                return Err(CodexAppAdapterError::UnsupportedEvent);
            }
        };

        Ok(vec![event])
    }

    /// 将 app-server JSON-RPC request 转换为等待用户处理的交互事件。
    pub fn event_from_server_request(
        request: &Value,
        cwd: &str,
        updated_at: UnixMillis,
    ) -> Result<Option<AgentEvent>, CodexAppAdapterError> {
        let object = request.as_object().ok_or(CodexAppAdapterError::NotObject)?;
        let request_id = required_request_id(object.get("id"), "id")?;
        let method = required_string(object.get("method"), "method")?;
        let params = object
            .get("params")
            .ok_or(CodexAppAdapterError::MissingField("params"))?;

        match method.as_str() {
            "item/commandExecution/requestApproval"
            | "item/fileChange/requestApproval"
            | "item/permissions/requestApproval"
            | "applyPatchApproval"
            | "execCommandApproval" => Ok(Some(approval_from_server_request(
                &request_id,
                &method,
                params,
                cwd,
                updated_at,
            )?)),
            "item/tool/requestUserInput" | "mcpServer/elicitation/request" => Ok(Some(
                answer_from_server_request(&request_id, &method, params, cwd, updated_at)?,
            )),
            _ => Ok(None),
        }
    }

    /// 编码 initialize request。
    pub fn initialize_request(id: u64) -> Value {
        json!({
            "id": id,
            "method": "initialize",
            "params": {
                "clientInfo": {
                    "name": "builder_panel",
                    "title": "Builder Panel",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": true
                }
            }
        })
    }

    /// 编码 initialized notification。
    pub fn initialized_notification() -> Value {
        json!({
            "method": "initialized",
            "params": {}
        })
    }

    /// 编码 thread/start request。
    pub fn thread_start_request(id: u64, cwd: &str, model: Option<&str>) -> Value {
        json!({
            "id": id,
            "method": "thread/start",
            "params": {
                "cwd": cwd,
                "model": model,
                "approvalPolicy": "on-request",
                "approvalsReviewer": "user",
                "ephemeral": true
            }
        })
    }

    /// 编码 turn/start request。
    pub fn turn_start_request(id: u64, thread_id: &str, prompt: &str) -> Value {
        json!({
            "id": id,
            "method": "turn/start",
            "params": {
                "threadId": thread_id,
                "input": [
                    {
                        "type": "text",
                        "text": prompt
                    }
                ]
            }
        })
    }
}

/// Codex APP adapter 错误。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodexAppAdapterError {
    /// JSON 不是对象。
    NotObject,
    /// 缺少必填字段。
    MissingField(&'static str),
    /// 字段类型错误。
    InvalidField(&'static str),
    /// payload 来源不是 Codex APP。
    AgentMismatch,
    /// 事件暂不支持。
    UnsupportedEvent,
}

/// Codex APP runtime。
pub struct CodexAppRuntime {
    /// 当前折叠后的 session 状态。
    session_state: SessionState,
    /// 等待 UI 决策的 hook 审批。
    pending_hook_approvals: BTreeMap<InteractionId, PendingHookApproval>,
    /// 等待 UI 回写的 app-server approval 上下文。
    pending_rpc_approvals: BTreeMap<InteractionId, PendingRpcApproval>,
    /// 等待 UI 回写的 app-server answer 上下文。
    pending_rpc_answers: BTreeMap<InteractionId, PendingRpcAnswer>,
    /// 已生成 response 但尚未确认写入完成的 app-server RPC 交互。
    pending_rpc_submissions: BTreeSet<InteractionId>,
    /// Codex APP thread 到真实 cwd 的映射。
    thread_cwds: BTreeMap<String, String>,
    /// 已发起但尚未完成写入确认的 follow-up turn。
    pending_followup_turns: BTreeSet<SessionKey>,
    /// 托管事件时间线缓存。
    timeline_cache: InMemoryProcessTimelineCache,
}

impl CodexAppRuntime {
    /// 创建空 Codex APP runtime。
    pub fn empty() -> Self {
        Self {
            session_state: SessionState::empty(),
            pending_hook_approvals: BTreeMap::new(),
            pending_rpc_approvals: BTreeMap::new(),
            pending_rpc_answers: BTreeMap::new(),
            pending_rpc_submissions: BTreeSet::new(),
            thread_cwds: BTreeMap::new(),
            pending_followup_turns: BTreeSet::new(),
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

    /// 应用 Codex APP hook request。
    pub fn apply_hook_request(
        &mut self,
        request: &BridgeRequestEnvelope,
        updated_at: UnixMillis,
    ) -> Result<Option<PendingCodexAppHookApprovalWait>, AppError> {
        if request.command_type != BridgeCommandType::ProcessAgentHook {
            return Err(protocol_error("不支持的 bridge command"));
        }

        let payload = &request.payload.validated_payload;
        self.thread_cwds
            .insert(payload.session_id.clone(), payload.cwd.clone());
        self.migrate_codex_app_thread_to_cwd(&payload.session_id, &payload.cwd)?;
        let events =
            CodexAppAdapter::events_from_hook_payload(&request.request_id, payload, updated_at)
                .map_err(|_| protocol_error("Codex APP hook payload 不受支持"))?;

        for event in events {
            self.apply_event(event)?;
        }

        if payload.hook_event_name != BridgeHookEventName::PermissionRequest {
            return Ok(None);
        }

        let interaction_id = hook_interaction_id(&request.request_id);
        let session_key = session_key(&payload.cwd, &payload.session_id);
        let waiter = PendingHookApprovalWaiter::new();
        self.expire_stale_hook_approvals_for_session(&session_key, &interaction_id);
        let replaced = self.pending_hook_approvals.insert(
            interaction_id.clone(),
            PendingHookApproval {
                session_key: session_key.clone(),
                waiter: waiter.clone(),
            },
        );
        if let Some(replaced) = replaced {
            replaced.waiter.expire();
        }

        Ok(Some(PendingCodexAppHookApprovalWait {
            session_key,
            interaction_id,
            waiter,
        }))
    }

    /// 应用 app-server JSON-RPC 消息。
    pub fn apply_app_server_message(
        &mut self,
        message: &Value,
        cwd: &str,
        updated_at: UnixMillis,
    ) -> Result<(), AppError> {
        let object = message
            .as_object()
            .ok_or_else(|| protocol_error("Codex APP app-server 消息不是对象"))?;
        if let Some((thread_id, cwd)) = self.record_message_thread_cwd(message) {
            self.migrate_codex_app_thread_to_cwd(&thread_id, &cwd)?;
        }
        let resolved_cwd = self.message_cwd(message, cwd);
        let method = object.get("method").and_then(Value::as_str);
        if method.is_some_and(is_server_request_method) || object.contains_key("id") {
            match CodexAppAdapter::event_from_server_request(message, &resolved_cwd, updated_at) {
                Ok(Some(event)) => {
                    let event_session_key = event.session_key().clone();
                    self.clear_rpc_pending_for_session(&event_session_key);
                    if let Some(approval) =
                        pending_rpc_approval_from_server_request(message, &resolved_cwd)
                    {
                        self.pending_rpc_approvals
                            .insert(approval.interaction_id.clone(), approval);
                    }
                    if let Some(answer) =
                        pending_rpc_answer_from_server_request(message, &resolved_cwd)
                    {
                        self.pending_rpc_answers
                            .insert(answer.interaction_id.clone(), answer);
                    }
                    self.apply_event(event)?;
                }
                Ok(None) => {
                    self.record_server_request_failure(
                        message,
                        &resolved_cwd,
                        "Codex APP app-server request 不受支持",
                        updated_at,
                    )?;
                    return Err(protocol_error("Codex APP app-server request 不受支持"));
                }
                Err(_) => {
                    self.record_server_request_failure(
                        message,
                        &resolved_cwd,
                        "Codex APP app-server request 格式无效",
                        updated_at,
                    )?;
                    return Err(protocol_error("Codex APP app-server request 格式无效"));
                }
            }
            return Ok(());
        }

        if object.contains_key("method") {
            if let Some(event) =
                CodexAppAdapter::event_from_notification(message, &resolved_cwd, updated_at)
                    .map_err(|_| protocol_error("Codex APP app-server notification 不受支持"))?
            {
                self.clear_detached_rpc_pending(&event);
                self.apply_event(event)?;
            }
        }

        Ok(())
    }

    /// 提交审批决策。
    pub fn resolve_approval(
        &mut self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
        decision: ApprovalDecision,
    ) -> Result<Option<CodexAppRpcWrite>, AppError> {
        let interaction = self
            .pending_interaction(session_key, interaction_id)?
            .clone();
        let AgentInteraction::Approval(approval) = interaction else {
            return Err(invalid_interaction("当前交互不是审批"));
        };

        match approval.reply_target {
            ReplyTarget::HookDirective(_) => {
                let Some(pending) = self.pending_hook_approvals.get(interaction_id) else {
                    return Err(invalid_interaction("审批交互不存在或已处理"));
                };
                if pending.session_key != *session_key {
                    return Err(invalid_interaction("审批交互不属于当前会话"));
                }
                if !self.current_pending_approval_matches(session_key, interaction_id) {
                    if let Some(pending) = self.pending_hook_approvals.remove(interaction_id) {
                        pending.waiter.expire();
                    }
                    return Err(invalid_interaction("审批交互已不是当前等待项"));
                }
                if !pending.waiter.resolve(decision) {
                    self.pending_hook_approvals.remove(interaction_id);
                    self.fail_expired_hook_approval(session_key)?;
                    return Err(invalid_interaction("审批交互已过期"));
                }
                self.pending_hook_approvals.remove(interaction_id);
                self.complete_interaction(session_key, decision_summary(decision))?;
                Ok(None)
            }
            ReplyTarget::StructuredRpc(_) => {
                let approval = self
                    .pending_rpc_approvals
                    .get(interaction_id)
                    .ok_or_else(|| invalid_interaction("审批回复上下文不存在或已处理"))?;
                let response = approval.approval_response(decision);
                let rpc_id = approval.rpc_id.clone();
                self.reserve_rpc_submission(interaction_id)?;
                Ok(Some(CodexAppRpcWrite::response(rpc_id, response)))
            }
            ReplyTarget::ManagedProcessStdin(_)
            | ReplyTarget::ControlledTerminal(_)
            | ReplyTarget::ClipboardOnly(_) => Err(invalid_interaction("审批回复目标不受支持")),
        }
    }

    /// 发送开放性回复。
    pub fn send_reply(
        &mut self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
        content: &str,
    ) -> Result<CodexAppRpcWrite, AppError> {
        let content = content.trim();
        if content.is_empty() {
            return Err(invalid_interaction("回复内容不能为空"));
        }
        let interaction = self
            .pending_interaction(session_key, interaction_id)?
            .clone();
        let AgentInteraction::TextReply(reply) = interaction else {
            return Err(invalid_interaction("当前交互不是文本回复"));
        };
        let ReplyTarget::StructuredRpc(_) = reply.reply_target else {
            return Err(invalid_interaction("文本回复目标不受支持"));
        };

        let answer = self
            .pending_rpc_answers
            .get(interaction_id)
            .ok_or_else(|| invalid_interaction("文本回复上下文不存在或已处理"))?;
        let response = answer.text_response(content.to_string());
        let rpc_id = answer.rpc_id.clone();
        self.reserve_rpc_submission(interaction_id)?;
        Ok(CodexAppRpcWrite::response(rpc_id, response))
    }

    /// 提交选项回复。
    pub fn submit_choice(
        &mut self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
        submission: ChoiceSubmission,
    ) -> Result<CodexAppRpcWrite, AppError> {
        let interaction = self
            .pending_interaction(session_key, interaction_id)?
            .clone();
        let AgentInteraction::Choice(choice) = interaction else {
            return Err(invalid_interaction("当前交互不是选项"));
        };
        let selected_values = validated_choice_values(&choice, &submission.selected_values)?;
        let ReplyTarget::StructuredRpc(_) = choice.reply_target else {
            return Err(invalid_interaction("选项回复目标不受支持"));
        };
        let answer = self
            .pending_rpc_answers
            .get(interaction_id)
            .ok_or_else(|| invalid_interaction("选项回复上下文不存在或已处理"))?;
        let response = answer.choice_response(selected_values);
        let rpc_id = answer.rpc_id.clone();
        self.reserve_rpc_submission(interaction_id)?;

        Ok(CodexAppRpcWrite::response(rpc_id, response))
    }

    /// 标记 app-server RPC 交互已成功写回。
    pub fn complete_rpc_interaction(
        &mut self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
        summary: &str,
    ) -> Result<(), AppError> {
        self.pending_rpc_submissions.remove(interaction_id);
        let interaction = self
            .pending_interaction(session_key, interaction_id)?
            .clone();
        let target = interaction.reply_target();
        if !matches!(target, ReplyTarget::StructuredRpc(_)) {
            return Err(invalid_interaction("交互回复目标不支持 RPC 完成"));
        }

        self.pending_rpc_approvals.remove(interaction_id);
        self.pending_rpc_answers.remove(interaction_id);
        self.complete_interaction(session_key, summary)
    }

    /// 释放未成功写入的 RPC 提交占位。
    pub fn release_rpc_submission(&mut self, interaction_id: &InteractionId) {
        self.pending_rpc_submissions.remove(interaction_id);
    }

    /// 创建后续 turn。
    pub fn create_followup_turn(
        &mut self,
        session_key: &SessionKey,
        prompt: &str,
    ) -> Result<CodexAppRpcWrite, AppError> {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Err(invalid_interaction("follow-up 内容不能为空"));
        }
        let Some(session) = self.session_state.sessions.get(session_key) else {
            return Err(invalid_interaction("会话不存在"));
        };
        if !session.capabilities.can_create_followup_turn {
            return Err(invalid_interaction("当前会话不支持 follow-up turn"));
        }
        if session.pending_interaction.is_some() {
            return Err(invalid_interaction(
                "当前会话仍有待处理交互，不能创建 follow-up turn",
            ));
        }
        if self.pending_followup_turns.contains(session_key) {
            return Err(invalid_interaction("当前会话已有 follow-up turn 正在提交"));
        }
        if !matches!(
            session.status,
            SessionStatus::Completed | SessionStatus::Failed
        ) {
            return Err(invalid_interaction(
                "当前会话尚未空闲，不能创建 follow-up turn",
            ));
        }

        self.pending_followup_turns.insert(session_key.clone());
        Ok(CodexAppRpcWrite::request(
            CodexAppAdapter::turn_start_request(0, &session_key.conversation_id.value, prompt),
        ))
    }

    /// 标记 follow-up turn 已成功写入 app-server。
    pub fn complete_followup_turn(&mut self, session_key: &SessionKey) -> Result<(), AppError> {
        self.pending_followup_turns.remove(session_key);
        self.apply_event(AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
            session_key: session_key.clone(),
            summary: "Codex APP follow-up 已提交".to_string(),
            updated_at: unix_now(),
        }))
    }

    /// 释放未成功写入的 follow-up turn。
    pub fn release_followup_turn(&mut self, session_key: &SessionKey) {
        self.pending_followup_turns.remove(session_key);
    }

    fn apply_event(&mut self, event: AgentEvent) -> Result<(), AppError> {
        self.ensure_codex_app_realtime_session(&event)?;
        self.apply_event_direct(event)
    }

    fn apply_event_direct(&mut self, event: AgentEvent) -> Result<(), AppError> {
        let codex_app_started = match &event {
            AgentEvent::SessionStarted(started)
                if started.session_key.agent_kind == AgentKind::CodexApp =>
            {
                Some((
                    started.session_key.clone(),
                    started.session_key.conversation_id.value.clone(),
                    started.updated_at,
                ))
            }
            _ => None,
        };
        self.timeline_cache.record_agent_event(&event)?;
        self.session_state = self.session_state.apply_event(event);
        if let Some((session_key, thread_id, updated_at)) = codex_app_started {
            let jump_event = jump_target_event(session_key, &thread_id, updated_at);
            self.timeline_cache.record_agent_event(&jump_event)?;
            self.session_state = self.session_state.apply_event(jump_event);
        }
        Ok(())
    }

    fn ensure_codex_app_realtime_session(&mut self, event: &AgentEvent) -> Result<(), AppError> {
        let session_key = event.session_key();
        if session_key.agent_kind != AgentKind::CodexApp {
            return Ok(());
        }
        if matches!(event, AgentEvent::SessionStarted(_)) {
            return Ok(());
        }
        if self.session_state.sessions.contains_key(session_key) {
            return Ok(());
        }

        self.apply_event_direct(AgentEvent::SessionStarted(realtime_started_event(
            session_key.clone(),
            event_updated_at(event),
        )))
    }

    fn pending_interaction(
        &self,
        session_key: &SessionKey,
        interaction_id: &InteractionId,
    ) -> Result<&AgentInteraction, AppError> {
        let Some(session) = self.session_state.sessions.get(session_key) else {
            return Err(invalid_interaction("会话不存在"));
        };
        let Some(interaction) = &session.pending_interaction else {
            return Err(invalid_interaction("当前会话没有待处理交互"));
        };

        match interaction {
            AgentInteraction::Approval(value) if value.interaction_id == *interaction_id => {
                Ok(interaction)
            }
            AgentInteraction::TextReply(value) if value.interaction_id == *interaction_id => {
                Ok(interaction)
            }
            AgentInteraction::Choice(value) if value.interaction_id == *interaction_id => {
                Ok(interaction)
            }
            AgentInteraction::Approval(_)
            | AgentInteraction::TextReply(_)
            | AgentInteraction::Choice(_) => Err(invalid_interaction("交互已变化")),
        }
    }

    fn reserve_rpc_submission(&mut self, interaction_id: &InteractionId) -> Result<(), AppError> {
        if self.pending_rpc_submissions.contains(interaction_id) {
            return Err(invalid_interaction("当前交互正在提交"));
        }
        self.pending_rpc_submissions.insert(interaction_id.clone());
        Ok(())
    }

    fn complete_interaction(
        &mut self,
        session_key: &SessionKey,
        summary: &str,
    ) -> Result<(), AppError> {
        self.apply_event(AgentEvent::InteractionCompleted(
            InteractionCompletedEvent {
                session_key: session_key.clone(),
                summary: Some(summary.to_string()),
                updated_at: unix_now(),
            },
        ))
    }

    fn expire_hook_approval(&mut self, session_key: &SessionKey, interaction_id: &InteractionId) {
        let Some(pending) = self.pending_hook_approvals.get(interaction_id) else {
            return;
        };
        if pending.session_key != *session_key {
            return;
        }

        self.pending_hook_approvals.remove(interaction_id);
        let _ = self.fail_expired_hook_approval(session_key);
    }

    fn expire_stale_hook_approvals_for_session(
        &mut self,
        session_key: &SessionKey,
        current_interaction_id: &InteractionId,
    ) {
        let stale_ids: Vec<InteractionId> = self
            .pending_hook_approvals
            .iter()
            .filter(|(interaction_id, pending)| {
                pending.session_key == *session_key && *interaction_id != current_interaction_id
            })
            .map(|(interaction_id, _)| interaction_id.clone())
            .collect();

        for stale_id in stale_ids {
            if let Some(pending) = self.pending_hook_approvals.remove(&stale_id) {
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

    fn fail_expired_hook_approval(&mut self, session_key: &SessionKey) -> Result<(), AppError> {
        self.apply_event(AgentEvent::Failed(FailedEvent {
            session_key: session_key.clone(),
            error: AppError::new(
                AppErrorCode::BridgeUnavailable,
                "Codex APP 审批等待超时",
                None,
                true,
                Some(FallbackAction::RetryLater),
            ),
            updated_at: unix_now(),
        }))
    }

    fn record_message_thread_cwd(&mut self, message: &Value) -> Option<(String, String)> {
        let thread_id = message_thread_id(message)?;
        let cwd = message_thread_cwd(message)?;

        self.thread_cwds.insert(thread_id.clone(), cwd.clone());
        Some((thread_id, cwd))
    }

    fn message_cwd(&self, message: &Value, default_cwd: &str) -> String {
        if let Some(cwd) = message_thread_cwd(message) {
            return cwd;
        }
        if let Some(thread_id) = message_thread_id(message) {
            if let Some(cwd) = self.thread_cwds.get(&thread_id) {
                return cwd.clone();
            }
        }

        default_cwd.to_string()
    }

    fn clear_detached_rpc_pending(&mut self, event: &AgentEvent) {
        let AgentEvent::Detached(event) = event else {
            return;
        };
        self.clear_rpc_pending_for_session(&event.session_key);
    }

    fn clear_rpc_pending_for_session(&mut self, session_key: &SessionKey) {
        let approval_submission_ids: Vec<InteractionId> = self
            .pending_rpc_approvals
            .iter()
            .filter(|(_, pending)| pending.session_key == *session_key)
            .map(|(interaction_id, _)| interaction_id.clone())
            .collect();
        let answer_submission_ids: Vec<InteractionId> = self
            .pending_rpc_answers
            .iter()
            .filter(|(_, pending)| pending.session_key == *session_key)
            .map(|(interaction_id, _)| interaction_id.clone())
            .collect();
        for interaction_id in approval_submission_ids
            .into_iter()
            .chain(answer_submission_ids.into_iter())
        {
            self.pending_rpc_submissions.remove(&interaction_id);
        }
        self.pending_rpc_approvals
            .retain(|_, pending| pending.session_key != *session_key);
        self.pending_rpc_answers
            .retain(|_, pending| pending.session_key != *session_key);
    }

    fn migrate_codex_app_thread_to_cwd(
        &mut self,
        thread_id: &str,
        cwd: &str,
    ) -> Result<(), AppError> {
        let target_key = session_key(cwd, thread_id);
        let stale_keys = self
            .session_state
            .sessions
            .keys()
            .filter(|key| {
                key.agent_kind == AgentKind::CodexApp
                    && key.conversation_id.value == thread_id
                    && **key != target_key
            })
            .cloned()
            .collect::<Vec<_>>();

        for stale_key in stale_keys {
            self.migrate_session_key(&stale_key, &target_key)?;
        }

        Ok(())
    }

    fn migrate_session_key(
        &mut self,
        stale_key: &SessionKey,
        target_key: &SessionKey,
    ) -> Result<(), AppError> {
        let Some(mut stale_session) = self.session_state.sessions.remove(stale_key) else {
            return Ok(());
        };

        stale_session.session_key = target_key.clone();
        stale_session.project_label = project_label(&target_key.project_id.value);
        stale_session.conversation_label = target_key.conversation_id.value.clone();
        stale_session.pending_interaction = stale_session
            .pending_interaction
            .take()
            .map(|interaction| interaction.aligned_to_session_key(target_key));
        if stale_session.capabilities == SessionCapabilities::none() {
            stale_session.capabilities = codex_app_capabilities();
        }
        if stale_session.jump_target.is_none() {
            stale_session.jump_target =
                Some(codex_app_jump_target(&target_key.conversation_id.value));
        }

        if let Some(target_session) = self.session_state.sessions.get_mut(target_key) {
            if target_session.pending_interaction.is_none() {
                target_session.pending_interaction = stale_session.pending_interaction.take();
            }
            if target_session.summary.is_none() {
                target_session.summary = stale_session.summary;
            }
            if target_session.title.is_none() {
                target_session.title = stale_session.title;
            }
            if target_session.last_error.is_none() {
                target_session.last_error = stale_session.last_error;
            }
            if target_session.jump_target.is_none() {
                target_session.jump_target = stale_session.jump_target;
            }
            if target_session.updated_at < stale_session.updated_at {
                target_session.status = stale_session.status;
                target_session.usage = stale_session.usage;
                target_session.updated_at = stale_session.updated_at;
            }
        } else {
            self.session_state
                .sessions
                .insert(target_key.clone(), stale_session);
        }

        for pending in self.pending_hook_approvals.values_mut() {
            if pending.session_key == *stale_key {
                pending.session_key = target_key.clone();
            }
        }
        for pending in self.pending_rpc_approvals.values_mut() {
            if pending.session_key == *stale_key {
                pending.session_key = target_key.clone();
            }
        }
        for pending in self.pending_rpc_answers.values_mut() {
            if pending.session_key == *stale_key {
                pending.session_key = target_key.clone();
            }
        }
        if self.pending_followup_turns.remove(stale_key) {
            self.pending_followup_turns.insert(target_key.clone());
        }
        self.timeline_cache
            .migrate_session_key(stale_key, target_key)?;

        Ok(())
    }

    fn record_server_request_failure(
        &mut self,
        message: &Value,
        cwd: &str,
        summary: &str,
        updated_at: UnixMillis,
    ) -> Result<(), AppError> {
        let Some(thread_id) = message_thread_id(message) else {
            return Ok(());
        };
        let session_key = session_key(cwd, &thread_id);
        self.clear_rpc_pending_for_session(&session_key);
        self.apply_event(AgentEvent::Failed(FailedEvent {
            session_key,
            error: protocol_error(summary),
            updated_at,
        }))
    }
}

impl ProcessTimelineReaderPort for CodexAppRuntime {
    fn read_timeline(
        &self,
        session_key: &SessionKey,
    ) -> Result<Vec<ProcessTimelineItem>, AppError> {
        self.timeline_cache.read_timeline(session_key)
    }
}

impl ProcessTimelineReleasePort for CodexAppRuntime {
    fn release_large_texts(&mut self, session_key: &SessionKey) -> Result<usize, AppError> {
        self.timeline_cache.release_large_texts(session_key)
    }
}

/// 待写入 app-server 的 JSON-RPC 消息。
#[derive(Clone, Debug, PartialEq)]
pub struct CodexAppRpcWrite {
    /// JSON-RPC 消息。
    pub message: Value,
    /// 是否需要等待 response。
    pub waits_for_response: bool,
}

impl CodexAppRpcWrite {
    /// 创建 app-server response 写入。
    pub fn response(id: Value, result: Value) -> Self {
        Self {
            message: json!({
                "id": id,
                "result": result
            }),
            waits_for_response: false,
        }
    }

    /// 创建 app-server request 写入。
    pub fn request(message: Value) -> Self {
        Self {
            message,
            waits_for_response: true,
        }
    }
}

/// 等待 hook 审批的上下文。
pub struct PendingCodexAppHookApprovalWait {
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

/// 等待 UI 回写的 app-server approval 上下文。
#[derive(Clone, Debug, PartialEq)]
struct PendingRpcApproval {
    /// 所属会话。
    session_key: SessionKey,
    /// 所属交互。
    interaction_id: InteractionId,
    /// app-server 原始 JSON-RPC id。
    rpc_id: Value,
    /// approval response 协议类型。
    kind: PendingRpcApprovalKind,
    /// permissions approval 请求的权限内容。
    permissions: Option<Value>,
}

/// app-server approval response 协议类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingRpcApprovalKind {
    /// 新版 item approval decision。
    ModernDecision,
    /// legacy ReviewDecision。
    LegacyReviewDecision,
    /// permissions grant response。
    PermissionsGrant,
}

impl PendingRpcApproval {
    /// 创建 approval response。
    fn approval_response(&self, decision: ApprovalDecision) -> Value {
        match self.kind {
            PendingRpcApprovalKind::ModernDecision => modern_approval_response(decision),
            PendingRpcApprovalKind::LegacyReviewDecision => legacy_approval_response(decision),
            PendingRpcApprovalKind::PermissionsGrant => permissions_approval_response(
                decision,
                self.permissions.clone().unwrap_or_else(|| json!({})),
            ),
        }
    }
}

/// 等待 UI 回写的 app-server answer 上下文。
#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingRpcAnswer {
    /// 所属会话。
    session_key: SessionKey,
    /// 所属交互。
    interaction_id: InteractionId,
    /// app-server 原始 JSON-RPC id。
    rpc_id: Value,
    /// app-server answer 目标。
    target: PendingRpcAnswerTarget,
}

/// app-server answer 目标协议。
#[derive(Clone, Debug, Eq, PartialEq)]
enum PendingRpcAnswerTarget {
    /// requestUserInput 问题 id。
    RequestUserInput { question_id: String },
    /// MCP elicitation request。
    McpElicitation,
}

impl PendingRpcAnswer {
    /// 创建 requestUserInput 文本 response。
    fn text_response(&self, content: String) -> Value {
        match &self.target {
            PendingRpcAnswerTarget::RequestUserInput { question_id } => {
                json!({
                    "answers": {
                        question_id.clone(): {
                            "answers": [content]
                        }
                    }
                })
            }
            PendingRpcAnswerTarget::McpElicitation => {
                json!({
                    "action": "accept",
                    "content": {
                        "answer": content
                    }
                })
            }
        }
    }

    /// 创建 requestUserInput 选项 response。
    fn choice_response(&self, selected_values: Vec<String>) -> Value {
        match &self.target {
            PendingRpcAnswerTarget::RequestUserInput { question_id } => {
                json!({
                    "answers": {
                        question_id.clone(): {
                            "answers": selected_values
                        }
                    }
                })
            }
            PendingRpcAnswerTarget::McpElicitation => {
                let content = selected_values.first().cloned().unwrap_or_default();
                json!({
                    "action": "accept",
                    "content": {
                        "answer": content
                    }
                })
            }
        }
    }
}

fn message_thread_id(message: &Value) -> Option<String> {
    let params = message.get("params")?;
    if let Some(thread_id) = params.get("threadId").and_then(Value::as_str) {
        return Some(thread_id.to_string());
    }
    if let Some(thread_id) = params
        .get("thread")
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
    {
        return Some(thread_id.to_string());
    }

    None
}

fn message_thread_cwd(message: &Value) -> Option<String> {
    let params = message.get("params")?;
    if let Some(cwd) = params.get("cwd").and_then(Value::as_str) {
        return Some(cwd.to_string());
    }
    if let Some(cwd) = params
        .get("thread")
        .and_then(|thread| thread.get("cwd"))
        .and_then(Value::as_str)
    {
        return Some(cwd.to_string());
    }

    None
}

/// hook 审批等待器。
#[derive(Clone)]
struct PendingHookApprovalWaiter {
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

    /// 标记等待器过期并唤醒等待线程。
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

/// 处理 Codex APP bridge request。
pub fn handle_codex_app_bridge_request(
    runtime: Arc<Mutex<CodexAppRuntime>>,
    request: BridgeRequestEnvelope,
) -> BridgeResponseEnvelope {
    let request_id = request.request_id.clone();
    let waiter = {
        let runtime_lock = runtime.lock();
        let Ok(mut runtime) = runtime_lock else {
            return error_response(request_id, "Codex APP runtime 锁已损坏");
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
            BridgeDirectivePayload::allow(AgentKind::CodexApp),
        ),
        PendingHookApprovalWaitResult::Resolved(ApprovalDecision::Deny) => {
            BridgeResponseEnvelope::directive(
                request_id,
                BridgeDirectivePayload::deny(
                    AgentKind::CodexApp,
                    Some("用户拒绝 Codex APP 权限请求".to_string()),
                    None,
                ),
            )
        }
        PendingHookApprovalWaitResult::Expired => BridgeResponseEnvelope::error(request_id, {
            if let Ok(mut runtime) = runtime.lock() {
                runtime.expire_hook_approval(&wait.session_key, &wait.interaction_id);
            }
            BridgeErrorPayload {
                code: BridgeErrorCode::BridgeUnavailable,
                message: format!(
                    "Codex APP 审批等待超时：{} / {}",
                    wait.session_key.conversation_id.value, wait.interaction_id.value
                ),
            }
        }),
    }
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

fn modern_approval_response(decision: ApprovalDecision) -> Value {
    match decision {
        ApprovalDecision::Allow => json!({"decision": "accept"}),
        ApprovalDecision::AllowAndRemember => json!({"decision": "acceptForSession"}),
        ApprovalDecision::Deny => json!({"decision": "decline"}),
    }
}

fn legacy_approval_response(decision: ApprovalDecision) -> Value {
    match decision {
        ApprovalDecision::Allow => json!({"decision": "approved"}),
        ApprovalDecision::AllowAndRemember => json!({"decision": "approved_for_session"}),
        ApprovalDecision::Deny => json!({"decision": "denied"}),
    }
}

fn permissions_approval_response(decision: ApprovalDecision, permissions: Value) -> Value {
    match decision {
        ApprovalDecision::Allow => json!({
            "permissions": permissions,
            "scope": "turn"
        }),
        ApprovalDecision::AllowAndRemember => json!({
            "permissions": permissions,
            "scope": "session"
        }),
        ApprovalDecision::Deny => json!({
            "permissions": {},
            "scope": "turn"
        }),
    }
}

fn decision_summary(decision: ApprovalDecision) -> &'static str {
    match decision {
        ApprovalDecision::Allow => "Codex APP 审批已允许",
        ApprovalDecision::AllowAndRemember => "Codex APP 审批已允许并记住",
        ApprovalDecision::Deny => "Codex APP 审批已拒绝",
    }
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

/// Codex APP app-server stdio 客户端。
pub struct CodexAppServerClient {
    /// app-server 子进程。
    child: Arc<Mutex<Child>>,
    /// app-server stdin。
    stdin: Arc<Mutex<ChildStdin>>,
    /// 待匹配 request。
    pending: Arc<(Mutex<BTreeMap<u64, PendingRpcResult>>, Condvar)>,
    /// 下一个 request id。
    next_id: Arc<Mutex<u64>>,
}

impl CodexAppServerClient {
    /// 启动 app-server 并完成 initialize。
    pub fn start(
        codex_path: &Path,
        cwd: String,
        runtime: Arc<Mutex<CodexAppRuntime>>,
    ) -> Result<Self, AppError> {
        let mut child = Command::new(codex_path)
            .args(["app-server", "--listen", "stdio://"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                app_server_error("Codex APP app-server 启动失败", error.to_string())
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| app_server_error("Codex APP app-server stdin 不可用", String::new()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| app_server_error("Codex APP app-server stdout 不可用", String::new()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| app_server_error("Codex APP app-server stderr 不可用", String::new()))?;
        let stdin = Arc::new(Mutex::new(stdin));
        let pending = Arc::new((Mutex::new(BTreeMap::new()), Condvar::new()));

        start_stdout_reader(
            stdout,
            Arc::clone(&stdin),
            Arc::clone(&pending),
            Arc::clone(&runtime),
            cwd.clone(),
        );
        thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or_default() > 0 {
                line.clear();
            }
        });

        let client = Self {
            child: Arc::new(Mutex::new(child)),
            stdin,
            pending,
            next_id: Arc::new(Mutex::new(1)),
        };
        if let Err(error) = client.initialize() {
            client.stop_and_wait();
            return Err(error);
        }

        Ok(client)
    }

    fn initialize(&self) -> Result<(), AppError> {
        self.send_request_value(CodexAppAdapter::initialize_request(self.next_request_id()?))?;
        self.write_message(&CodexAppAdapter::initialized_notification())
    }

    /// 发送 follow-up turn。
    pub fn start_followup_turn(&self, thread_id: &str, prompt: &str) -> Result<(), AppError> {
        let request =
            CodexAppAdapter::turn_start_request(self.next_request_id()?, thread_id, prompt);
        self.send_request_value(request)?;
        Ok(())
    }

    /// 写入不等待响应的 JSON-RPC response。
    pub fn write_rpc_response(&self, write: CodexAppRpcWrite) -> Result<(), AppError> {
        self.write_message(&write.message)
    }

    /// 写入等待响应的 JSON-RPC request。
    pub fn write_rpc_request(&self, mut write: CodexAppRpcWrite) -> Result<(), AppError> {
        if !write.waits_for_response {
            return Err(app_server_error(
                "Codex APP app-server request 写入类型无效",
                String::new(),
            ));
        }
        if write.message.get("id").and_then(Value::as_u64) == Some(0) {
            write.message["id"] = json!(self.next_request_id()?);
        }
        self.send_request_value(write.message)?;
        Ok(())
    }

    /// 停止 app-server。
    pub fn stop(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }

    /// 判断 app-server 子进程是否仍存活。
    pub fn is_running(&self) -> bool {
        let Ok(mut child) = self.child.lock() else {
            return false;
        };
        match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) | Err(_) => false,
        }
    }

    fn stop_and_wait(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn next_request_id(&self) -> Result<u64, AppError> {
        let mut id = self.next_id.lock().map_err(|_| {
            app_server_error("Codex APP app-server request id 锁已损坏", String::new())
        })?;
        let current = *id;
        *id += 1;
        Ok(current)
    }

    fn send_request_value(&self, request: Value) -> Result<Value, AppError> {
        let request_id = request
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| app_server_error("Codex APP request 缺少 id", String::new()))?;
        {
            let (lock, _) = &*self.pending;
            let mut pending = lock.lock().map_err(|_| {
                app_server_error("Codex APP pending request 锁已损坏", String::new())
            })?;
            pending.insert(request_id, PendingRpcResult::Waiting);
        }

        if let Err(error) = self.write_message(&request) {
            let (lock, _) = &*self.pending;
            if let Ok(mut pending) = lock.lock() {
                pending.remove(&request_id);
            }
            return Err(error);
        }
        self.wait_response(request_id)
    }

    fn write_message(&self, message: &Value) -> Result<(), AppError> {
        let mut line = serde_json::to_vec(message)
            .map_err(|error| app_server_error("Codex APP message 编码失败", error.to_string()))?;
        line.push(b'\n');
        let mut stdin = self
            .stdin
            .lock()
            .map_err(|_| app_server_error("Codex APP app-server stdin 锁已损坏", String::new()))?;
        stdin.write_all(&line).map_err(|error| {
            app_server_error("Codex APP app-server 写入失败", error.to_string())
        })?;
        stdin
            .flush()
            .map_err(|error| app_server_error("Codex APP app-server flush 失败", error.to_string()))
    }

    fn wait_response(&self, request_id: u64) -> Result<Value, AppError> {
        let (lock, condvar) = &*self.pending;
        let pending = lock
            .lock()
            .map_err(|_| app_server_error("Codex APP pending request 锁已损坏", String::new()))?;
        let (mut pending, timeout) = condvar
            .wait_timeout_while(pending, APP_SERVER_REQUEST_TIMEOUT, |pending| {
                matches!(pending.get(&request_id), Some(PendingRpcResult::Waiting))
            })
            .map_err(|_| app_server_error("Codex APP pending request 等待失败", String::new()))?;
        if timeout.timed_out() {
            pending.remove(&request_id);
            return Err(app_server_error(
                "Codex APP app-server request 超时",
                String::new(),
            ));
        }

        match pending.remove(&request_id) {
            Some(PendingRpcResult::Ready(result)) => Ok(result),
            Some(PendingRpcResult::Failed(message)) => Err(app_server_error(
                "Codex APP app-server request 失败",
                message,
            )),
            Some(PendingRpcResult::Waiting) | None => Err(app_server_error(
                "Codex APP app-server response 丢失",
                String::new(),
            )),
        }
    }
}

impl Drop for CodexAppServerClient {
    fn drop(&mut self) {
        self.stop_and_wait();
    }
}

/// app-server pending request 结果。
enum PendingRpcResult {
    /// 等待 response。
    Waiting,
    /// 已收到 result。
    Ready(Value),
    /// 已收到 error。
    Failed(String),
}

fn start_stdout_reader(
    stdout: impl std::io::Read + Send + 'static,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Arc<(Mutex<BTreeMap<u64, PendingRpcResult>>, Condvar)>,
    runtime: Arc<Mutex<CodexAppRuntime>>,
    cwd: String,
) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            let Ok(bytes) = reader.read_line(&mut line) else {
                break;
            };
            if bytes == 0 {
                break;
            }
            if line.len() > MAX_APP_SERVER_LINE_BYTES {
                continue;
            }
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if handle_rpc_response(&message, &pending) {
                continue;
            }
            let apply_result = if let Ok(mut runtime) = runtime.lock() {
                runtime.apply_app_server_message(&message, &cwd, unix_now())
            } else {
                Err(app_server_error(
                    "Codex APP runtime 锁已损坏",
                    String::new(),
                ))
            };
            if let Err(error) = apply_result {
                if let Some(response) =
                    app_server_protocol_error_response(&message, &error.user_message)
                {
                    let _ = write_app_server_message(&stdin, &response);
                }
            }
        }
    });
}

fn app_server_protocol_error_response(message: &Value, error_message: &str) -> Option<Value> {
    if !message.as_object()?.contains_key("method") {
        return None;
    }
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    Some(json!({
        "id": id,
        "error": {
            "code": -32601,
            "message": error_message
        }
    }))
}

fn write_app_server_message(
    stdin: &Arc<Mutex<ChildStdin>>,
    message: &Value,
) -> Result<(), AppError> {
    let mut line = serde_json::to_vec(message)
        .map_err(|error| app_server_error("Codex APP message 编码失败", error.to_string()))?;
    line.push(b'\n');
    let mut stdin = stdin
        .lock()
        .map_err(|_| app_server_error("Codex APP app-server stdin 锁已损坏", String::new()))?;
    stdin
        .write_all(&line)
        .map_err(|error| app_server_error("Codex APP app-server 写入失败", error.to_string()))?;
    stdin
        .flush()
        .map_err(|error| app_server_error("Codex APP app-server flush 失败", error.to_string()))
}

fn handle_rpc_response(
    message: &Value,
    pending: &Arc<(Mutex<BTreeMap<u64, PendingRpcResult>>, Condvar)>,
) -> bool {
    let Some(id) = message.get("id").and_then(Value::as_u64) else {
        return false;
    };
    if message.get("method").is_some() {
        return false;
    }
    let is_response = message.get("result").is_some() || message.get("error").is_some();
    if !is_response {
        return false;
    }
    let (lock, condvar) = &**pending;
    let Ok(mut pending) = lock.lock() else {
        return true;
    };
    if !pending.contains_key(&id) {
        return false;
    }
    if let Some(error) = message.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown app-server error")
            .to_string();
        pending.insert(id, PendingRpcResult::Failed(message));
    } else {
        pending.insert(
            id,
            PendingRpcResult::Ready(message.get("result").cloned().unwrap_or_else(|| json!({}))),
        );
    }
    condvar.notify_all();
    true
}

fn app_server_error(message: &str, detail: String) -> AppError {
    AppError::new(
        AppErrorCode::BridgeUnavailable,
        message,
        if detail.is_empty() {
            None
        } else {
            Some(detail)
        },
        true,
        Some(FallbackAction::RetryLater),
    )
}

fn schema_probe_from_dir(out_dir: &Path) -> CodexAppSchemaProbe {
    let mut verified_schema_files = Vec::new();
    let mut missing_schema_files = Vec::new();

    for file in REQUIRED_SCHEMA_FILES {
        if out_dir.join(file).is_file() {
            verified_schema_files.push(file.to_string());
        } else {
            missing_schema_files.push(file.to_string());
        }
    }

    CodexAppSchemaProbe {
        schema_available: missing_schema_files.is_empty(),
        verified_schema_files,
        missing_schema_files,
        diagnostic: None,
    }
}

fn started_from_thread(
    params: &Value,
    cwd: &str,
    updated_at: UnixMillis,
) -> Result<AgentEvent, CodexAppAdapterError> {
    let thread = params
        .get("thread")
        .ok_or(CodexAppAdapterError::MissingField("thread"))?;
    let thread_id = required_string(thread.get("id"), "thread.id")?;
    let title = optional_string(thread.get("name"), "thread.name")?;
    let session_key = session_key(cwd, &thread_id);

    Ok(AgentEvent::SessionStarted(SessionStartedEvent {
        session_key,
        project_label: project_label(cwd),
        conversation_label: thread_id,
        title,
        summary: Some("Codex APP thread 已启动".to_string()),
        capabilities: codex_app_capabilities(),
        usage: UsageSnapshot::unavailable(),
        updated_at,
    }))
}

fn turn_activity(
    params: &Value,
    cwd: &str,
    summary: &str,
    updated_at: UnixMillis,
) -> Result<AgentEvent, CodexAppAdapterError> {
    let thread_id = required_string(params.get("threadId"), "threadId")?;

    Ok(AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
        session_key: session_key(cwd, &thread_id),
        summary: summary.to_string(),
        updated_at,
    }))
}

fn agent_message_delta(
    params: &Value,
    cwd: &str,
    updated_at: UnixMillis,
) -> Result<AgentEvent, CodexAppAdapterError> {
    let thread_id = required_string(params.get("threadId"), "threadId")?;
    let delta = required_string(params.get("delta"), "delta")?;

    Ok(AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
        session_key: session_key(cwd, &thread_id),
        summary: truncate(&format!("Codex APP 回复中：{delta}"), 120),
        updated_at,
    }))
}

fn status_changed(
    params: &Value,
    cwd: &str,
    updated_at: UnixMillis,
) -> Result<Option<AgentEvent>, CodexAppAdapterError> {
    let thread_id = required_string(params.get("threadId"), "threadId")?;
    let status_type = required_string(
        params.get("status").and_then(|value| value.get("type")),
        "status.type",
    )?;
    let session_key = session_key(cwd, &thread_id);
    match status_type.as_str() {
        "active" => Ok(Some(AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
            session_key,
            summary: "Codex APP thread 运行中".to_string(),
            updated_at,
        }))),
        "idle" => Ok(Some(AgentEvent::TurnCompleted(TurnCompletedEvent {
            session_key,
            summary: Some("Codex APP thread 空闲".to_string()),
            updated_at,
        }))),
        "systemError" => Ok(Some(AgentEvent::Failed(FailedEvent {
            session_key,
            error: app_server_error("Codex APP thread 系统错误", String::new()),
            updated_at,
        }))),
        "notLoaded" => Ok(Some(AgentEvent::Detached(DetachedEvent {
            session_key,
            reason: Some("Codex APP thread 未加载".to_string()),
            updated_at,
        }))),
        _ => Ok(None),
    }
}

fn usage_updated(
    params: &Value,
    cwd: &str,
    updated_at: UnixMillis,
) -> Result<AgentEvent, CodexAppAdapterError> {
    let thread_id = required_string(params.get("threadId"), "threadId")?;
    let total_tokens = params
        .get("tokenUsage")
        .and_then(|value| value.get("total"))
        .and_then(|value| value.get("totalTokens"))
        .and_then(Value::as_i64)
        .ok_or(CodexAppAdapterError::MissingField(
            "tokenUsage.total.totalTokens",
        ))?;
    let usage = UsageSnapshot {
        usage_5h: UsageValue::Verified(VerifiedUsageValue {
            value: UsageAmount::new(total_tokens as f64)
                .map_err(|_| CodexAppAdapterError::InvalidField("totalTokens"))?,
            unit: Some("tokens".to_string()),
            source_key: "codex-app-server-token-usage".to_string(),
            source_label: "Codex APP app-server".to_string(),
            scope: crate::domain::usage::UsageScope::AccountWindow,
            updated_at: Some(updated_at),
        }),
        usage_weekly: UsageValue::Unavailable,
    };

    Ok(AgentEvent::UsageUpdated(UsageUpdatedEvent {
        session_key: session_key(cwd, &thread_id),
        usage,
        updated_at,
    }))
}

fn turn_completed(
    params: &Value,
    cwd: &str,
    updated_at: UnixMillis,
) -> Result<AgentEvent, CodexAppAdapterError> {
    let thread_id = required_string(params.get("threadId"), "threadId")?;

    Ok(AgentEvent::TurnCompleted(TurnCompletedEvent {
        session_key: session_key(cwd, &thread_id),
        summary: Some("Codex APP turn 已完成".to_string()),
        updated_at,
    }))
}

fn started_from_hook(
    payload: &ValidatedHookPayload,
    session_key: SessionKey,
    updated_at: UnixMillis,
) -> SessionStartedEvent {
    SessionStartedEvent {
        session_key,
        project_label: project_label(&payload.cwd),
        conversation_label: payload.session_id.clone(),
        title: payload.model.clone(),
        summary: Some(start_hook_summary(payload)),
        capabilities: codex_app_capabilities(),
        usage: UsageSnapshot::unavailable(),
        updated_at,
    }
}

fn realtime_started_event(session_key: SessionKey, updated_at: UnixMillis) -> SessionStartedEvent {
    SessionStartedEvent {
        project_label: project_label(&session_key.project_id.value),
        conversation_label: session_key.conversation_id.value.clone(),
        title: None,
        summary: Some("Codex APP 实时事件已捕捉".to_string()),
        capabilities: codex_app_capabilities(),
        usage: UsageSnapshot::unavailable(),
        session_key,
        updated_at,
    }
}

fn event_updated_at(event: &AgentEvent) -> UnixMillis {
    match event {
        AgentEvent::SessionStarted(event) => event.updated_at,
        AgentEvent::ActivityUpdated(event) => event.updated_at,
        AgentEvent::ApprovalRequested(event) => event.updated_at,
        AgentEvent::AnswerRequested(event) => event.updated_at,
        AgentEvent::InteractionCompleted(event) => event.updated_at,
        AgentEvent::TurnCompleted(event) => event.updated_at,
        AgentEvent::Failed(event) => event.updated_at,
        AgentEvent::Detached(event) => event.updated_at,
        AgentEvent::CapabilitiesUpdated(event) => event.updated_at,
        AgentEvent::UsageUpdated(event) => event.updated_at,
        AgentEvent::JumpTargetUpdated(event) => event.updated_at,
    }
}

fn approval_from_server_request(
    request_id: &str,
    method: &str,
    params: &Value,
    cwd: &str,
    updated_at: UnixMillis,
) -> Result<AgentEvent, CodexAppAdapterError> {
    let thread_id = required_string(params.get("threadId"), "threadId")?;
    let session_key = session_key(cwd, &thread_id);

    Ok(AgentEvent::ApprovalRequested(ApprovalRequestedEvent {
        session_key: session_key.clone(),
        interaction: ApprovalInteraction {
            interaction_id: rpc_interaction_id(request_id),
            session_key,
            created_at: updated_at,
            expires_at: None,
            reply_target: ReplyTarget::StructuredRpc(StructuredRpcTarget {
                target_id: request_id.to_string(),
            }),
            status: InteractionStatus::Pending,
            request_summary: approval_request_summary(method, params)?,
        },
        updated_at,
    }))
}

fn answer_from_server_request(
    request_id: &str,
    method: &str,
    params: &Value,
    cwd: &str,
    updated_at: UnixMillis,
) -> Result<AgentEvent, CodexAppAdapterError> {
    let thread_id = required_string(params.get("threadId"), "threadId")?;
    let session_key = session_key(cwd, &thread_id);
    if method == "mcpServer/elicitation/request" {
        let prompt = optional_string(params.get("message"), "message")?
            .unwrap_or_else(|| "Codex APP MCP 等待输入".to_string());
        return Ok(AgentEvent::AnswerRequested(AnswerRequestedEvent {
            session_key: session_key.clone(),
            interaction: AnswerInteraction::TextReply(TextReplyInteraction {
                interaction_id: rpc_interaction_id(request_id),
                session_key,
                created_at: updated_at,
                expires_at: None,
                reply_target: ReplyTarget::StructuredRpc(StructuredRpcTarget {
                    target_id: request_id.to_string(),
                }),
                status: InteractionStatus::Pending,
                request_summary: prompt.clone(),
                prompt,
            }),
            updated_at,
        }));
    }

    let questions = params
        .get("questions")
        .and_then(Value::as_array)
        .ok_or(CodexAppAdapterError::MissingField("questions"))?;
    if questions.len() > 1 {
        return Err(CodexAppAdapterError::InvalidField("questions"));
    }
    let Some(first_question) = questions.first() else {
        return Err(CodexAppAdapterError::MissingField("questions[0]"));
    };
    let prompt = optional_string(first_question.get("question"), "questions[0].question")?
        .or_else(|| {
            optional_string(first_question.get("label"), "questions[0].label")
                .ok()
                .flatten()
        })
        .unwrap_or_else(|| "Codex APP 等待输入".to_string());
    let choices = choices_from_question(first_question)?;

    if choices.is_empty() {
        return Ok(AgentEvent::AnswerRequested(AnswerRequestedEvent {
            session_key: session_key.clone(),
            interaction: AnswerInteraction::TextReply(TextReplyInteraction {
                interaction_id: rpc_interaction_id(request_id),
                session_key,
                created_at: updated_at,
                expires_at: None,
                reply_target: ReplyTarget::StructuredRpc(StructuredRpcTarget {
                    target_id: request_id.to_string(),
                }),
                status: InteractionStatus::Pending,
                request_summary: prompt.clone(),
                prompt,
            }),
            updated_at,
        }));
    }

    Ok(AgentEvent::AnswerRequested(AnswerRequestedEvent {
        session_key: session_key.clone(),
        interaction: AnswerInteraction::Choice(ChoiceInteraction {
            interaction_id: rpc_interaction_id(request_id),
            session_key,
            created_at: updated_at,
            expires_at: None,
            reply_target: ReplyTarget::StructuredRpc(StructuredRpcTarget {
                target_id: request_id.to_string(),
            }),
            status: InteractionStatus::Pending,
            request_summary: prompt,
            choices,
            allows_multiple: false,
        }),
        updated_at,
    }))
}

fn choices_from_question(value: &Value) -> Result<Vec<InteractionChoice>, CodexAppAdapterError> {
    let Some(options) = value.get("options") else {
        return Ok(Vec::new());
    };
    let options = options
        .as_array()
        .ok_or(CodexAppAdapterError::InvalidField("questions[0].options"))?;

    let mut choices = Vec::new();
    for option in options {
        let value = required_string(option.get("value"), "option.value")?;
        let label = optional_string(option.get("label"), "option.label")?.unwrap_or(value.clone());
        let tooltip = optional_string(option.get("tooltip"), "option.tooltip")?
            .or(optional_string(
                option.get("description"),
                "option.description",
            )?)
            .or(optional_string(option.get("help"), "option.help")?)
            .or(optional_string(option.get("detail"), "option.detail")?);
        choices.push(InteractionChoice {
            value,
            label,
            tooltip,
        });
    }

    Ok(choices)
}

fn pending_rpc_approval_from_server_request(
    message: &Value,
    cwd: &str,
) -> Option<PendingRpcApproval> {
    let object = message.as_object()?;
    let method = object.get("method")?.as_str()?;
    let request_id = required_request_id(object.get("id"), "id").ok()?;
    let thread_id = message_thread_id(message)?;
    let rpc_id = object.get("id")?.clone();
    let kind = match method {
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
            PendingRpcApprovalKind::ModernDecision
        }
        "applyPatchApproval" | "execCommandApproval" => {
            PendingRpcApprovalKind::LegacyReviewDecision
        }
        "item/permissions/requestApproval" => PendingRpcApprovalKind::PermissionsGrant,
        _ => return None,
    };
    let permissions = if kind == PendingRpcApprovalKind::PermissionsGrant {
        object
            .get("params")
            .and_then(|params| params.get("permissions"))
            .cloned()
    } else {
        None
    };

    Some(PendingRpcApproval {
        session_key: session_key(cwd, &thread_id),
        interaction_id: rpc_interaction_id(&request_id),
        rpc_id,
        kind,
        permissions,
    })
}

fn is_server_request_method(method: &str) -> bool {
    matches!(
        method,
        "item/commandExecution/requestApproval"
            | "item/fileChange/requestApproval"
            | "item/permissions/requestApproval"
            | "applyPatchApproval"
            | "execCommandApproval"
            | "item/tool/requestUserInput"
            | "mcpServer/elicitation/request"
    )
}

fn pending_rpc_answer_from_server_request(message: &Value, cwd: &str) -> Option<PendingRpcAnswer> {
    let object = message.as_object()?;
    let method = object.get("method")?.as_str()?;
    let request_id = required_request_id(object.get("id"), "id").ok()?;
    let thread_id = message_thread_id(message)?;
    if method == "mcpServer/elicitation/request" {
        return Some(PendingRpcAnswer {
            session_key: session_key(cwd, &thread_id),
            interaction_id: rpc_interaction_id(&request_id),
            rpc_id: object.get("id")?.clone(),
            target: PendingRpcAnswerTarget::McpElicitation,
        });
    }
    if method != "item/tool/requestUserInput" {
        return None;
    }

    let params = object.get("params")?;
    let questions = params.get("questions")?.as_array()?;
    if questions.len() > 1 {
        return None;
    }
    let first_question = questions.first()?;
    let question_id = optional_string(first_question.get("id"), "questions[0].id")
        .ok()
        .flatten()
        .unwrap_or_else(|| "answer".to_string());

    Some(PendingRpcAnswer {
        session_key: session_key(cwd, &thread_id),
        interaction_id: rpc_interaction_id(&request_id),
        rpc_id: object.get("id")?.clone(),
        target: PendingRpcAnswerTarget::RequestUserInput { question_id },
    })
}

fn validated_choice_values(
    interaction: &ChoiceInteraction,
    selected_values: &[String],
) -> Result<Vec<String>, AppError> {
    if selected_values.is_empty() {
        return Err(invalid_interaction("至少选择一项"));
    }

    if !interaction.allows_multiple && selected_values.len() != 1 {
        return Err(invalid_interaction("当前交互只允许单选"));
    }

    let allowed_values = interaction
        .choices
        .iter()
        .map(|choice| choice.value.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut unique_values = std::collections::BTreeSet::new();
    let mut validated = Vec::new();

    for value in selected_values {
        if !allowed_values.contains(value.as_str()) {
            return Err(invalid_interaction("选项值不在当前交互中"));
        }
        if !unique_values.insert(value.as_str()) {
            return Err(invalid_interaction("选项值重复"));
        }
        validated.push(value.clone());
    }

    Ok(validated)
}

fn approval_request_summary(method: &str, params: &Value) -> Result<String, CodexAppAdapterError> {
    let command = optional_string(params.get("command"), "command")?;
    let reason = optional_string(params.get("reason"), "reason")?;
    let item_id = optional_string(params.get("itemId"), "itemId")?;
    let subject = command
        .or(reason)
        .or(item_id)
        .unwrap_or_else(|| method.to_string());

    Ok(format!("Codex APP 请求审批：{}", truncate(&subject, 120)))
}

fn jump_target_event(
    session_key: SessionKey,
    thread_id: &str,
    updated_at: UnixMillis,
) -> AgentEvent {
    AgentEvent::JumpTargetUpdated(JumpTargetUpdatedEvent {
        session_key,
        jump_target: Some(codex_app_jump_target(thread_id)),
        updated_at,
    })
}

fn codex_app_jump_target(thread_id: &str) -> JumpTarget {
    JumpTarget {
        label: "Codex APP".to_string(),
        location: format!("codex://threads/{thread_id}"),
    }
}

fn required_string(
    value: Option<&Value>,
    field: &'static str,
) -> Result<String, CodexAppAdapterError> {
    let text = optional_string(value, field)?.ok_or(CodexAppAdapterError::MissingField(field))?;
    if text.trim().is_empty() {
        return Err(CodexAppAdapterError::InvalidField(field));
    }

    Ok(text)
}

fn required_request_id(
    value: Option<&Value>,
    field: &'static str,
) -> Result<String, CodexAppAdapterError> {
    match value {
        Some(Value::String(text)) if !text.trim().is_empty() => Ok(text.clone()),
        Some(Value::Number(number)) => Ok(number.to_string()),
        None | Some(Value::Null) => Err(CodexAppAdapterError::MissingField(field)),
        Some(_) => Err(CodexAppAdapterError::InvalidField(field)),
    }
}

fn optional_string(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Option<String>, CodexAppAdapterError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(CodexAppAdapterError::InvalidField(field)),
    }
}

fn session_key(cwd: &str, thread_id: &str) -> SessionKey {
    SessionKey::new(
        AgentKind::CodexApp,
        ProjectId::new(cwd.to_string()),
        ConversationId::new(thread_id.to_string()),
    )
}

fn codex_app_capabilities() -> SessionCapabilities {
    SessionCapabilities {
        can_jump: true,
        can_send_reply: true,
        can_resolve_approval: true,
        can_create_followup_turn: true,
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

fn hook_interaction_id(request_id: &str) -> InteractionId {
    InteractionId::new(format!("codex-app-hook-{request_id}"))
}

fn rpc_interaction_id(request_id: &str) -> InteractionId {
    InteractionId::new(format!("codex-app-rpc-{request_id}"))
}

fn start_hook_summary(payload: &ValidatedHookPayload) -> String {
    let Some(model) = &payload.model else {
        return "Codex APP 会话已启动".to_string();
    };

    format!("Codex APP 会话已启动，模型 {model}")
}

fn prompt_summary(payload: &ValidatedHookPayload) -> String {
    let Some(prompt) = &payload.prompt else {
        return "用户提交了 Codex APP prompt".to_string();
    };

    format!("用户提交了 Codex APP prompt：{}", truncate(prompt, 120))
}

fn hook_tool_summary(prefix: &str, payload: &ValidatedHookPayload) -> String {
    let tool_name = payload.tool_name.as_deref().unwrap_or("未知工具");
    let Some(tool_input) = &payload.tool_input else {
        return format!("{prefix}：{tool_name}");
    };
    if let Some(command) = tool_input.get("command").and_then(Value::as_str) {
        return format!("{prefix}：{tool_name} {}", truncate(command, 120));
    }
    if let Some(description) = tool_input.get("description").and_then(Value::as_str) {
        return format!("{prefix}：{tool_name} {}", truncate(description, 120));
    }

    format!("{prefix}：{tool_name}")
}

fn schema_probe_dir() -> PathBuf {
    std::env::temp_dir().join(format!("builder-panel-codex-schema-{}", unix_millis()))
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn unix_now() -> UnixMillis {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default();
    UnixMillis::new(millis)
}

fn required_schema_files() -> Vec<String> {
    REQUIRED_SCHEMA_FILES
        .iter()
        .map(|file| file.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::Duration;

    use serde_json::{json, Value};

    use super::{
        app_server_protocol_error_response, handle_rpc_response, schema_probe_from_dir,
        session_key, CodexAppAdapter, CodexAppRuntime, PendingRpcResult,
    };
    use crate::adapters::bridge::codec::{
        BridgeHookEventName, BridgeRequestEnvelope, ValidatedHookPayload,
    };
    use crate::domain::agent_event::{AgentEvent, TurnCompletedEvent};
    use crate::domain::agent_interaction::AgentInteraction;
    use crate::domain::agent_session::{AgentKind, SessionStatus};
    use crate::domain::usage::UnixMillis;
    use crate::domain::view_model::UiAction;
    use crate::ports::agent_adapter_port::{ApprovalDecision, ChoiceSubmission};
    use crate::ports::process_timeline_port::ProcessTimelineReaderPort;

    #[test]
    fn schema_probe_checks_required_files() {
        let dir =
            std::env::temp_dir().join(format!("builder-panel-schema-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("v2")).expect("dir should create");
        std::fs::write(dir.join("v2/ThreadStartParams.json"), "{}").expect("file should write");

        let probe = schema_probe_from_dir(&dir);

        assert!(!probe.schema_available);
        assert!(probe
            .verified_schema_files
            .contains(&"v2/ThreadStartParams.json".to_string()));
        assert!(probe
            .missing_schema_files
            .contains(&"v2/TurnStartParams.json".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn thread_started_notification_maps_to_session_started() {
        let notification = json!({
            "method": "thread/started",
            "params": {
                "thread": {
                    "id": "thread-1",
                    "name": "阶段 4"
                }
            }
        });

        let event = CodexAppAdapter::event_from_notification(
            &notification,
            "/tmp/builder-panel",
            UnixMillis::new(1),
        )
        .expect("notification should parse")
        .expect("event should exist");

        let AgentEvent::SessionStarted(event) = event else {
            panic!("event should be started");
        };
        assert_eq!(event.session_key.agent_kind, AgentKind::CodexApp);
        assert_eq!(event.conversation_label, "thread-1");
        assert!(event.capabilities.can_resolve_approval);
        assert!(event.capabilities.can_send_reply);
        assert!(event.capabilities.can_create_followup_turn);
        assert!(event.capabilities.can_view_process_timeline);
    }

    #[test]
    fn app_server_thread_started_runtime_adds_jump_target() {
        let mut runtime = CodexAppRuntime::empty();
        let message = json!({
            "method": "thread/started",
            "params": {
                "thread": {
                    "id": "thread-1",
                    "name": "阶段 4"
                }
            }
        });

        runtime
            .apply_app_server_message(&message, "/tmp/builder-panel", UnixMillis::new(1))
            .expect("message should apply");
        let session_key = session_key("/tmp/builder-panel", "thread-1");
        let session = runtime
            .session_state()
            .sessions
            .get(&session_key)
            .expect("session should exist");

        assert_eq!(
            session
                .jump_target
                .as_ref()
                .map(|target| target.location.as_str()),
            Some("codex://threads/thread-1")
        );
        assert!(runtime
            .session_list()
            .first()
            .expect("list item should exist")
            .actions
            .contains(&crate::domain::view_model::UiAction::Jump));
    }

    #[test]
    fn token_usage_notification_maps_verified_usage() {
        let notification = json!({
            "method": "thread/tokenUsage/updated",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "tokenUsage": {
                    "last": {
                        "cachedInputTokens": 0,
                        "inputTokens": 1,
                        "outputTokens": 2,
                        "reasoningOutputTokens": 3,
                        "totalTokens": 6
                    },
                    "total": {
                        "cachedInputTokens": 0,
                        "inputTokens": 10,
                        "outputTokens": 20,
                        "reasoningOutputTokens": 30,
                        "totalTokens": 60
                    }
                }
            }
        });

        let event = CodexAppAdapter::event_from_notification(
            &notification,
            "/tmp/builder-panel",
            UnixMillis::new(1),
        )
        .expect("notification should parse")
        .expect("event should exist");

        let AgentEvent::UsageUpdated(event) = event else {
            panic!("event should be usage");
        };
        assert_eq!(event.session_key.agent_kind, AgentKind::CodexApp);
    }

    #[test]
    fn request_encoding_matches_verified_methods() {
        let initialize = CodexAppAdapter::initialize_request(1);
        let thread_start =
            CodexAppAdapter::thread_start_request(2, "/tmp/builder-panel", Some("gpt-5.4"));
        let turn_start = CodexAppAdapter::turn_start_request(3, "thread-1", "ping");

        assert_eq!(initialize["method"], "initialize");
        assert_eq!(thread_start["method"], "thread/start");
        assert_eq!(turn_start["method"], "turn/start");
        assert_eq!(turn_start["params"]["input"][0]["type"], "text");
    }

    #[test]
    fn runtime_maps_codex_app_hook_permission_to_pending_approval() {
        let mut runtime = CodexAppRuntime::empty();
        let request = BridgeRequestEnvelope::process_agent_hook(
            "request-1".to_string(),
            hook_payload(BridgeHookEventName::PermissionRequest),
        );

        let waiter = runtime
            .apply_hook_request(&request, UnixMillis::new(1))
            .expect("hook should apply");

        assert!(waiter.is_some());
        let session = runtime
            .session_state()
            .sessions
            .values()
            .next()
            .expect("session should exist");
        assert_eq!(session.session_key.agent_kind, AgentKind::CodexApp);
        assert_eq!(session.status, SessionStatus::WaitingForApproval);
        assert!(matches!(
            session.pending_interaction,
            Some(AgentInteraction::Approval(_))
        ));
        assert!(session.capabilities.can_create_followup_turn);
        assert!(session.capabilities.can_view_process_timeline);
    }

    #[test]
    fn runtime_maps_app_server_user_input_request_to_text_reply() {
        let mut runtime = CodexAppRuntime::empty();
        let request = json!({
            "id": 7,
            "method": "item/tool/requestUserInput",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "questions": [
                    {
                        "id": "answer",
                        "question": "继续吗？"
                    }
                ]
            }
        });

        runtime
            .apply_app_server_message(&request, "/tmp/builder-panel", UnixMillis::new(1))
            .expect("request should apply");

        let session = runtime
            .session_state()
            .sessions
            .values()
            .next()
            .expect("session should exist");
        assert_eq!(session.status, SessionStatus::WaitingForAnswer);
        assert!(matches!(
            session.pending_interaction,
            Some(AgentInteraction::TextReply(_))
        ));
    }

    #[test]
    fn first_realtime_approval_request_initializes_operable_session() {
        let mut runtime = CodexAppRuntime::empty();
        let request = json!({
            "id": 9,
            "method": "item/commandExecution/requestApproval",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "command": "cargo test"
            }
        });

        runtime
            .apply_app_server_message(&request, "/tmp/builder-panel", UnixMillis::new(1))
            .expect("request should apply");
        let list_item = runtime
            .session_list()
            .first()
            .expect("session should be visible")
            .clone();
        let (session_key, interaction_id) = pending_interaction_keys(&runtime);

        assert!(list_item.actions.contains(&UiAction::ResolveApproval));
        assert!(list_item.actions.contains(&UiAction::Jump));
        assert!(list_item.actions.contains(&UiAction::ViewProcessTimeline));
        let write = runtime
            .resolve_approval(&session_key, &interaction_id, ApprovalDecision::Allow)
            .expect("approval should encode")
            .expect("rpc response should exist");
        assert_eq!(write.message["id"], 9);
    }

    #[test]
    fn first_realtime_answer_request_initializes_reply_action() {
        let mut runtime = CodexAppRuntime::empty();

        runtime
            .apply_app_server_message(
                &user_input_request(7),
                "/tmp/builder-panel",
                UnixMillis::new(1),
            )
            .expect("request should apply");

        let list_item = runtime
            .session_list()
            .first()
            .expect("session should be visible")
            .clone();
        assert!(list_item.actions.contains(&UiAction::SendReply));
        assert!(list_item.actions.contains(&UiAction::Jump));
        assert!(list_item.actions.contains(&UiAction::ViewProcessTimeline));
    }

    #[test]
    fn rpc_text_reply_rejects_duplicate_submission_until_released() {
        let mut runtime = CodexAppRuntime::empty();
        let request = json!({
            "id": 7,
            "method": "item/tool/requestUserInput",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "questions": [
                    {
                        "id": "answer",
                        "question": "继续吗？"
                    }
                ]
            }
        });
        runtime
            .apply_app_server_message(&request, "/tmp/builder-panel", UnixMillis::new(1))
            .expect("request should apply");
        let (session_key, interaction_id) = pending_interaction_keys(&runtime);

        runtime
            .send_reply(&session_key, &interaction_id, "继续")
            .expect("first reply should encode");
        let duplicate_error = match runtime.send_reply(&session_key, &interaction_id, "继续") {
            Ok(_) => panic!("duplicate reply should reject"),
            Err(error) => error,
        };
        runtime.release_rpc_submission(&interaction_id);
        runtime
            .send_reply(&session_key, &interaction_id, "继续")
            .expect("released reply should allow retry");

        assert_eq!(duplicate_error.user_message, "当前交互正在提交");
    }

    #[test]
    fn request_user_input_rejects_multiple_questions() {
        let mut runtime = CodexAppRuntime::empty();
        let request = json!({
            "id": 7,
            "method": "item/tool/requestUserInput",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "questions": [
                    {
                        "id": "answer-a",
                        "question": "第一个问题？"
                    },
                    {
                        "id": "answer-b",
                        "question": "第二个问题？"
                    }
                ]
            }
        });

        let error = runtime
            .apply_app_server_message(&request, "/tmp/builder-panel", UnixMillis::new(1))
            .expect_err("multiple questions should fail explicitly");
        let session_key = session_key("/tmp/builder-panel", "thread-1");
        let session = runtime
            .session_state()
            .sessions
            .get(&session_key)
            .expect("session should exist");

        assert_eq!(error.user_message, "Codex APP app-server request 格式无效");
        assert_eq!(session.status, SessionStatus::Failed);
    }

    #[test]
    fn rpc_response_handler_keeps_unknown_server_request_for_runtime() {
        let pending = Arc::new((
            Mutex::new(BTreeMap::<u64, PendingRpcResult>::new()),
            Condvar::new(),
        ));
        let message = user_input_request(7);

        assert!(!handle_rpc_response(&message, &pending));

        let mut runtime = CodexAppRuntime::empty();
        runtime
            .apply_app_server_message(&message, "/tmp/builder-panel", UnixMillis::new(1))
            .expect("server request should apply after response handler");
        let session = runtime
            .session_state()
            .sessions
            .values()
            .next()
            .expect("session should exist");
        assert_eq!(session.status, SessionStatus::WaitingForAnswer);
    }

    #[test]
    fn rpc_response_handler_keeps_server_request_when_id_collides_with_pending() {
        let pending = Arc::new((
            Mutex::new(BTreeMap::<u64, PendingRpcResult>::new()),
            Condvar::new(),
        ));
        {
            let (lock, _) = &*pending;
            lock.lock()
                .expect("pending should lock")
                .insert(7, PendingRpcResult::Waiting);
        }
        let message = user_input_request(7);

        assert!(!handle_rpc_response(&message, &pending));

        let (lock, _) = &*pending;
        assert!(matches!(
            lock.lock().expect("pending should lock").get(&7),
            Some(PendingRpcResult::Waiting)
        ));
    }

    #[test]
    fn codex_app_text_reply_keeps_pending_until_write_succeeds() {
        let mut runtime = CodexAppRuntime::empty();
        runtime
            .apply_app_server_message(
                &user_input_request(7),
                "/tmp/builder-panel",
                UnixMillis::new(1),
            )
            .expect("request should apply");
        let (session_key, interaction_id) = pending_interaction_keys(&runtime);

        let write = runtime
            .send_reply(&session_key, &interaction_id, "继续")
            .expect("reply should encode");

        assert_eq!(
            write.message["result"]["answers"]["answer"]["answers"][0],
            "继续"
        );
        assert!(runtime
            .session_state()
            .sessions
            .get(&session_key)
            .expect("session should exist")
            .pending_interaction
            .is_some());

        runtime
            .complete_rpc_interaction(&session_key, &interaction_id, "Codex APP 回复已发送")
            .expect("completion should clear pending");
        assert!(runtime
            .session_state()
            .sessions
            .get(&session_key)
            .expect("session should exist")
            .pending_interaction
            .is_none());
        assert_eq!(
            runtime
                .session_state()
                .sessions
                .get(&session_key)
                .expect("session should exist")
                .status,
            SessionStatus::Running
        );
    }

    #[test]
    fn codex_app_choice_reply_encodes_selected_values() {
        let mut runtime = CodexAppRuntime::empty();
        let request = json!({
            "id": 8,
            "method": "item/tool/requestUserInput",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "questions": [
                    {
                        "id": "plan",
                        "question": "选择方案",
                        "options": [
                            {"value": "a", "label": "A"},
                            {"value": "b", "label": "B"}
                        ]
                    }
                ]
            }
        });
        runtime
            .apply_app_server_message(&request, "/tmp/builder-panel", UnixMillis::new(1))
            .expect("request should apply");
        let (session_key, interaction_id) = pending_interaction_keys(&runtime);

        let write = runtime
            .submit_choice(
                &session_key,
                &interaction_id,
                ChoiceSubmission {
                    selected_values: vec!["b".to_string()],
                },
            )
            .expect("choice should encode");

        assert_eq!(
            write.message["result"]["answers"]["plan"]["answers"][0],
            "b"
        );
        assert!(runtime
            .session_state()
            .sessions
            .get(&session_key)
            .expect("session should exist")
            .pending_interaction
            .is_some());
    }

    #[test]
    fn app_server_message_reuses_hook_cwd_for_same_thread() {
        let mut runtime = CodexAppRuntime::empty();
        let hook = BridgeRequestEnvelope::process_agent_hook(
            "request-1".to_string(),
            hook_payload(BridgeHookEventName::SessionStart),
        );
        runtime
            .apply_hook_request(&hook, UnixMillis::new(1))
            .expect("hook should apply");
        let message = json!({
            "method": "thread/status/changed",
            "params": {
                "threadId": "thread-1",
                "status": {"type": "idle"}
            }
        });

        runtime
            .apply_app_server_message(&message, "/wrong/cwd", UnixMillis::new(2))
            .expect("message should apply");

        let expected_key = crate::domain::agent_session::SessionKey::new(
            AgentKind::CodexApp,
            crate::domain::agent_session::ProjectId::new("/tmp/builder-panel"),
            crate::domain::agent_session::ConversationId::new("thread-1"),
        );
        assert!(runtime.session_state().sessions.contains_key(&expected_key));
        assert_eq!(runtime.session_state().sessions.len(), 1);
    }

    #[test]
    fn hook_real_cwd_migrates_app_server_fallback_session() {
        let mut runtime = CodexAppRuntime::empty();
        runtime
            .apply_app_server_message(&user_input_request(7), "/fallback/cwd", UnixMillis::new(1))
            .expect("request should apply");
        let fallback_key = session_key("/fallback/cwd", "thread-1");
        assert!(runtime.session_state().sessions.contains_key(&fallback_key));
        assert!(!runtime
            .read_timeline(&fallback_key)
            .expect("timeline should read")
            .is_empty());

        let hook = BridgeRequestEnvelope::process_agent_hook(
            "request-1".to_string(),
            hook_payload(BridgeHookEventName::SessionStart),
        );
        runtime
            .apply_hook_request(&hook, UnixMillis::new(2))
            .expect("hook should apply");
        let real_key = session_key("/tmp/builder-panel", "thread-1");
        let session = runtime
            .session_state()
            .sessions
            .get(&real_key)
            .expect("real cwd session should exist");

        assert_eq!(runtime.session_state().sessions.len(), 1);
        assert!(!runtime.session_state().sessions.contains_key(&fallback_key));
        assert!(matches!(
            session.pending_interaction,
            Some(AgentInteraction::TextReply(_))
        ));
        assert_eq!(
            session
                .pending_interaction
                .as_ref()
                .expect("pending should exist")
                .session_key(),
            &real_key
        );
        assert!(runtime
            .read_timeline(&fallback_key)
            .expect("fallback timeline should read")
            .is_empty());
        assert!(!runtime
            .read_timeline(&real_key)
            .expect("real timeline should read")
            .is_empty());

        let interaction_id = session
            .pending_interaction
            .as_ref()
            .expect("pending should exist")
            .status();
        assert_eq!(
            interaction_id,
            crate::domain::agent_interaction::InteractionStatus::Pending
        );
        let (_, migrated_interaction_id) = pending_interaction_keys(&runtime);
        runtime
            .send_reply(&real_key, &migrated_interaction_id, "继续")
            .expect("migrated pending rpc answer should remain operable");
    }

    #[test]
    fn idle_status_marks_session_ready_for_followup() {
        let mut runtime = CodexAppRuntime::empty();
        let started = json!({
            "method": "thread/started",
            "params": {
                "thread": {
                    "id": "thread-1",
                    "name": "Thread 1"
                }
            }
        });
        let idle = json!({
            "method": "thread/status/changed",
            "params": {
                "threadId": "thread-1",
                "status": {"type": "idle"}
            }
        });
        runtime
            .apply_app_server_message(&started, "/tmp/builder-panel", UnixMillis::new(1))
            .expect("thread should start");
        runtime
            .apply_app_server_message(&idle, "/tmp/builder-panel", UnixMillis::new(2))
            .expect("idle should apply");
        let session_key = session_key("/tmp/builder-panel", "thread-1");

        let session = runtime
            .session_state()
            .sessions
            .get(&session_key)
            .expect("session should exist");
        assert_eq!(session.status, SessionStatus::Completed);

        let write = runtime
            .create_followup_turn(&session_key, "继续")
            .expect("idle session should allow followup");
        assert_eq!(write.message["method"], "turn/start");
    }

    #[test]
    fn error_statuses_do_not_map_to_running_activity() {
        let mut runtime = CodexAppRuntime::empty();
        let started = json!({
            "method": "thread/started",
            "params": {
                "thread": {
                    "id": "thread-1",
                    "name": "Thread 1"
                }
            }
        });
        runtime
            .apply_app_server_message(&started, "/tmp/builder-panel", UnixMillis::new(1))
            .expect("thread should start");

        let system_error = json!({
            "method": "thread/status/changed",
            "params": {
                "threadId": "thread-1",
                "status": {"type": "systemError"}
            }
        });
        runtime
            .apply_app_server_message(&system_error, "/tmp/builder-panel", UnixMillis::new(2))
            .expect("system error should apply");
        let session_key = session_key("/tmp/builder-panel", "thread-1");
        let session = runtime
            .session_state()
            .sessions
            .get(&session_key)
            .expect("session should exist");
        assert_eq!(session.status, SessionStatus::Failed);

        let not_loaded = json!({
            "method": "thread/status/changed",
            "params": {
                "threadId": "thread-1",
                "status": {"type": "notLoaded"}
            }
        });
        runtime
            .apply_app_server_message(&not_loaded, "/tmp/builder-panel", UnixMillis::new(3))
            .expect("notLoaded should apply");
        let session = runtime
            .session_state()
            .sessions
            .get(&session_key)
            .expect("session should exist");
        assert_eq!(session.status, SessionStatus::Detached);
    }

    #[test]
    fn not_loaded_clears_pending_rpc_interaction() {
        let mut runtime = CodexAppRuntime::empty();
        let request = user_input_request(7);
        runtime
            .apply_app_server_message(&request, "/tmp/builder-panel", UnixMillis::new(1))
            .expect("request should apply");
        let (session_key, interaction_id) = pending_interaction_keys(&runtime);
        assert!(runtime.pending_rpc_answers.contains_key(&interaction_id));

        let not_loaded = json!({
            "method": "thread/status/changed",
            "params": {
                "threadId": "thread-1",
                "status": {"type": "notLoaded"}
            }
        });
        runtime
            .apply_app_server_message(&not_loaded, "/tmp/builder-panel", UnixMillis::new(2))
            .expect("notLoaded should apply");

        let session = runtime
            .session_state()
            .sessions
            .get(&session_key)
            .expect("session should exist");
        assert_eq!(session.status, SessionStatus::Detached);
        assert!(session.pending_interaction.is_none());
        assert!(!runtime.pending_rpc_answers.contains_key(&interaction_id));
    }

    #[test]
    fn newer_rpc_request_clears_old_context_for_same_session() {
        let mut runtime = CodexAppRuntime::empty();
        runtime
            .apply_app_server_message(
                &user_input_request(7),
                "/tmp/builder-panel",
                UnixMillis::new(1),
            )
            .expect("first request should apply");
        let (_, first_interaction_id) = pending_interaction_keys(&runtime);

        runtime
            .apply_app_server_message(
                &user_input_request(8),
                "/tmp/builder-panel",
                UnixMillis::new(2),
            )
            .expect("second request should apply");
        let (_, second_interaction_id) = pending_interaction_keys(&runtime);

        assert!(!runtime
            .pending_rpc_answers
            .contains_key(&first_interaction_id));
        assert!(runtime
            .pending_rpc_answers
            .contains_key(&second_interaction_id));
    }

    #[test]
    fn unknown_server_request_fails_session_and_encodes_protocol_error() {
        let mut runtime = CodexAppRuntime::empty();
        let request = json!({
            "id": 42,
            "method": "future/request",
            "params": {
                "threadId": "thread-1"
            }
        });

        let error = runtime
            .apply_app_server_message(&request, "/tmp/builder-panel", UnixMillis::new(1))
            .expect_err("unknown request should fail");
        let response = app_server_protocol_error_response(&request, &error.user_message)
            .expect("protocol error should encode");
        let session_key = session_key("/tmp/builder-panel", "thread-1");
        let session = runtime
            .session_state()
            .sessions
            .get(&session_key)
            .expect("session should exist");

        assert_eq!(session.status, SessionStatus::Failed);
        assert_eq!(response["id"], 42);
        assert_eq!(response["error"]["code"], -32601);
    }

    #[test]
    fn known_server_request_without_id_encodes_null_id_error() {
        let mut runtime = CodexAppRuntime::empty();
        let request = json!({
            "method": "item/tool/requestUserInput",
            "params": {
                "threadId": "thread-1",
                "questions": [
                    {
                        "id": "answer",
                        "type": "text",
                        "prompt": "继续？"
                    }
                ]
            }
        });

        let error = runtime
            .apply_app_server_message(&request, "/tmp/builder-panel", UnixMillis::new(1))
            .expect_err("missing id should fail");
        let response = app_server_protocol_error_response(&request, &error.user_message)
            .expect("protocol error should encode");

        assert_eq!(response["id"], Value::Null);
        assert_eq!(response["error"]["code"], -32601);
    }

    #[test]
    fn mcp_elicitation_reply_encodes_accept_content() {
        let mut runtime = CodexAppRuntime::empty();
        let request = json!({
            "id": 9,
            "method": "mcpServer/elicitation/request",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "serverName": "demo",
                "mode": "form",
                "message": "请输入参数",
                "requestedSchema": {}
            }
        });
        runtime
            .apply_app_server_message(&request, "/tmp/builder-panel", UnixMillis::new(1))
            .expect("request should apply");
        let (session_key, interaction_id) = pending_interaction_keys(&runtime);

        let write = runtime
            .send_reply(&session_key, &interaction_id, "参数")
            .expect("reply should encode");

        assert_eq!(write.message["result"]["action"], "accept");
        assert_eq!(write.message["result"]["content"]["answer"], "参数");
    }

    #[test]
    fn permissions_request_maps_to_approval_and_preserves_numeric_response_id() {
        let mut runtime = CodexAppRuntime::empty();
        let request = json!({
            "id": 10,
            "method": "item/permissions/requestApproval",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "cwd": "/tmp/builder-panel",
                "startedAtMs": 1,
                "permissions": {
                    "fileSystem": null,
                    "network": {"enabled": true}
                }
            }
        });
        runtime
            .apply_app_server_message(&request, "/tmp/builder-panel", UnixMillis::new(1))
            .expect("request should apply");
        let (session_key, interaction_id) = pending_interaction_keys(&runtime);

        let write = runtime
            .resolve_approval(
                &session_key,
                &interaction_id,
                crate::ports::agent_adapter_port::ApprovalDecision::AllowAndRemember,
            )
            .expect("approval should encode")
            .expect("rpc response should exist");

        assert_eq!(write.message["id"], 10);
        assert_eq!(write.message["result"]["scope"], "session");
        assert_eq!(
            write.message["result"]["permissions"]["network"]["enabled"],
            true
        );
    }

    #[test]
    fn legacy_approval_response_uses_review_decision_enum() {
        let mut runtime = CodexAppRuntime::empty();
        let request = json!({
            "id": "legacy-1",
            "method": "execCommandApproval",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "command": "cargo test"
            }
        });
        runtime
            .apply_app_server_message(&request, "/tmp/builder-panel", UnixMillis::new(1))
            .expect("request should apply");
        let (session_key, interaction_id) = pending_interaction_keys(&runtime);

        let write = runtime
            .resolve_approval(
                &session_key,
                &interaction_id,
                crate::ports::agent_adapter_port::ApprovalDecision::AllowAndRemember,
            )
            .expect("approval should encode")
            .expect("rpc response should exist");

        assert_eq!(write.message["id"], "legacy-1");
        assert_eq!(write.message["result"]["decision"], "approved_for_session");
    }

    #[test]
    fn followup_turn_does_not_update_runtime_before_write_success() {
        let mut runtime = CodexAppRuntime::empty();
        let hook = BridgeRequestEnvelope::process_agent_hook(
            "request-1".to_string(),
            hook_payload(BridgeHookEventName::SessionStart),
        );
        runtime
            .apply_hook_request(&hook, UnixMillis::new(1))
            .expect("hook should apply");
        let session_key = runtime
            .session_state()
            .sessions
            .keys()
            .next()
            .expect("session should exist")
            .clone();
        runtime
            .apply_event(AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key: session_key.clone(),
                summary: Some("Codex APP thread 空闲".to_string()),
                updated_at: UnixMillis::new(2),
            }))
            .expect("session should become idle");

        let write = runtime
            .create_followup_turn(&session_key, "继续")
            .expect("followup should encode");

        assert!(write.waits_for_response);
        assert_eq!(write.message["method"], "turn/start");
        let session = runtime
            .session_state()
            .sessions
            .get(&session_key)
            .expect("session should exist");
        assert_eq!(session.summary, Some("Codex APP thread 空闲".to_string()));
    }

    #[test]
    fn followup_turn_rejects_duplicate_until_released() {
        let mut runtime = CodexAppRuntime::empty();
        let hook = BridgeRequestEnvelope::process_agent_hook(
            "request-1".to_string(),
            hook_payload(BridgeHookEventName::SessionStart),
        );
        runtime
            .apply_hook_request(&hook, UnixMillis::new(1))
            .expect("hook should apply");
        let session_key = runtime
            .session_state()
            .sessions
            .keys()
            .next()
            .expect("session should exist")
            .clone();
        runtime
            .apply_event(AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key: session_key.clone(),
                summary: Some("Codex APP thread 空闲".to_string()),
                updated_at: UnixMillis::new(2),
            }))
            .expect("session should become idle");

        runtime
            .create_followup_turn(&session_key, "继续")
            .expect("first followup should encode");
        let duplicate_error = match runtime.create_followup_turn(&session_key, "继续") {
            Ok(_) => panic!("duplicate followup should reject"),
            Err(error) => error,
        };
        runtime.release_followup_turn(&session_key);
        runtime
            .create_followup_turn(&session_key, "继续")
            .expect("released followup should allow retry");

        assert_eq!(
            duplicate_error.user_message,
            "当前会话已有 follow-up turn 正在提交"
        );
    }

    #[test]
    fn followup_turn_rejects_running_or_pending_session() {
        let mut runtime = CodexAppRuntime::empty();
        let hook = BridgeRequestEnvelope::process_agent_hook(
            "request-1".to_string(),
            hook_payload(BridgeHookEventName::SessionStart),
        );
        runtime
            .apply_hook_request(&hook, UnixMillis::new(1))
            .expect("hook should apply");
        let session_key = runtime
            .session_state()
            .sessions
            .keys()
            .next()
            .expect("session should exist")
            .clone();

        let running_error = match runtime.create_followup_turn(&session_key, "继续") {
            Ok(_) => panic!("running session should reject followup"),
            Err(error) => error,
        };
        assert_eq!(
            running_error.user_message,
            "当前会话尚未空闲，不能创建 follow-up turn"
        );

        let approval = BridgeRequestEnvelope::process_agent_hook(
            "request-2".to_string(),
            hook_payload(BridgeHookEventName::PermissionRequest),
        );
        runtime
            .apply_hook_request(&approval, UnixMillis::new(2))
            .expect("approval should apply");

        let pending_error = match runtime.create_followup_turn(&session_key, "继续") {
            Ok(_) => panic!("pending session should reject followup"),
            Err(error) => error,
        };
        assert_eq!(
            pending_error.user_message,
            "当前会话仍有待处理交互，不能创建 follow-up turn"
        );
    }

    #[test]
    fn newer_hook_approval_expires_stale_waiter_for_same_session() {
        let mut runtime = CodexAppRuntime::empty();
        let first = BridgeRequestEnvelope::process_agent_hook(
            "request-1".to_string(),
            hook_payload(BridgeHookEventName::PermissionRequest),
        );
        let second = BridgeRequestEnvelope::process_agent_hook(
            "request-2".to_string(),
            hook_payload(BridgeHookEventName::PermissionRequest),
        );

        let first_wait = runtime
            .apply_hook_request(&first, UnixMillis::new(1))
            .expect("first hook should apply")
            .expect("first hook should wait");
        let second_wait = runtime
            .apply_hook_request(&second, UnixMillis::new(2))
            .expect("second hook should apply")
            .expect("second hook should wait");

        assert!(matches!(
            first_wait.waiter.wait(Duration::from_millis(1)),
            super::PendingHookApprovalWaitResult::Expired
        ));
        assert!(matches!(
            second_wait.waiter.wait(Duration::from_millis(1)),
            super::PendingHookApprovalWaitResult::Expired
        ));
    }

    fn user_input_request(id: u64) -> serde_json::Value {
        json!({
            "id": id,
            "method": "item/tool/requestUserInput",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "questions": [
                    {
                        "id": "answer",
                        "question": "继续吗？"
                    }
                ]
            }
        })
    }

    fn pending_interaction_keys(
        runtime: &CodexAppRuntime,
    ) -> (
        crate::domain::agent_session::SessionKey,
        crate::domain::agent_interaction::InteractionId,
    ) {
        let session = runtime
            .session_state()
            .sessions
            .values()
            .next()
            .expect("session should exist");
        let interaction = session
            .pending_interaction
            .as_ref()
            .expect("interaction should exist");

        match interaction {
            AgentInteraction::Approval(interaction) => (
                session.session_key.clone(),
                interaction.interaction_id.clone(),
            ),
            AgentInteraction::TextReply(interaction) => (
                session.session_key.clone(),
                interaction.interaction_id.clone(),
            ),
            AgentInteraction::Choice(interaction) => (
                session.session_key.clone(),
                interaction.interaction_id.clone(),
            ),
        }
    }

    fn hook_payload(event_name: BridgeHookEventName) -> ValidatedHookPayload {
        ValidatedHookPayload {
            agent_kind: AgentKind::CodexApp,
            hook_event_name: event_name,
            cwd: "/tmp/builder-panel".to_string(),
            session_id: "thread-1".to_string(),
            model: Some("gpt-5.4".to_string()),
            permission_mode: Some("default".to_string()),
            transcript_path: None,
            terminal_app: Some("Codex.app".to_string()),
            terminal_session_id: None,
            terminal_tty: None,
            terminal_title: None,
            turn_id: None,
            tool_name: Some("Bash".to_string()),
            tool_input: Some(json!({"command": "cargo test"})),
            prompt: Some("实现 Codex APP".to_string()),
            last_assistant_message: None,
            permission_suggestions: None,
        }
    }
}
