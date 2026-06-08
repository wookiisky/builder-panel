//! Codex APP adapter。

mod codex_rollout;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::adapters::bridge::codec::{
    BridgeCommandType, BridgeDirectivePayload, BridgeErrorCode, BridgeErrorPayload,
    BridgeHookEventName, BridgeRequestEnvelope, BridgeResponseEnvelope, ValidatedHookPayload,
};
use crate::adapters::timeline::InMemoryProcessTimelineCache;
use crate::domain::agent_event::{
    ActivityUpdatedEvent, AgentEvent, AnswerRequestedEvent, ApprovalRequestedEvent, DetachedEvent,
    FailedEvent, InteractionCompletedEvent, JumpTargetUpdatedEvent, SessionStartedEvent,
    TitleUpdatedEvent, TurnCompletedEvent, UsageUpdatedEvent, UserMessageUpdatedEvent,
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
use crate::ports::session_update_port::{
    NoopSessionUpdateSink, SessionRuntimeSource, SessionUpdateArea, SessionUpdateNotification,
    SessionUpdateSinkPort,
};

pub use self::codex_rollout::{CodexRolloutDiscovery, CodexRolloutTailer, CodexRolloutWatchTarget};

use self::codex_rollout::CodexRolloutSnapshot;

const REQUIRED_SCHEMA_FILES: [&str; 16] = [
    "v2/ThreadStartParams.json",
    "v2/ThreadStartResponse.json",
    "v2/TurnStartParams.json",
    "v2/TurnStartResponse.json",
    "v2/ThreadStartedNotification.json",
    "v2/ThreadNameUpdatedNotification.json",
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
const APP_SERVER_THREAD_LIST_TIMEOUT: Duration = Duration::from_secs(2);
const APPROVAL_WAIT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const MAX_APP_SERVER_LINE_BYTES: usize = 8 * 1024 * 1024;
const MAX_CURRENT_TURN_OUTPUT_CHARS: usize = 65_535;
const MAX_FINAL_OUTPUT_CHARS: usize = 65_535;
const UNRESOLVED_CODEX_APP_PROJECT_ID: &str = "__codex_app_unresolved__";
const UNRESOLVED_CODEX_APP_PROJECT_LABEL: &str = "待识别项目";

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
            "thread/name/updated" => Ok(title_updated(params, cwd, updated_at)?),
            "turn/started" => {
                let _ = required_string(params.get("threadId"), "threadId")?;
                Ok(None)
            }
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
                let Some(prompt) = prompt_summary(payload) else {
                    return Ok(Vec::new());
                };
                AgentEvent::UserMessageUpdated(UserMessageUpdatedEvent {
                    session_key,
                    summary: prompt,
                    updated_at,
                })
            }
            BridgeHookEventName::PreToolUse => {
                return Ok(Vec::new());
            }
            BridgeHookEventName::PermissionRequest => {
                let request_summary =
                    hook_tool_preview(payload).unwrap_or_else(|| "等待权限审批".to_string());
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
                            request_summary,
                        },
                        updated_at,
                    }),
                ]);
            }
            BridgeHookEventName::PostToolUse => return Ok(Vec::new()),
            BridgeHookEventName::Stop => AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key,
                summary: payload
                    .last_assistant_message
                    .as_ref()
                    .map(|message| truncate_strict(message, MAX_FINAL_OUTPUT_CHARS)),
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

    /// 编码 thread/loaded/list request。
    pub fn thread_loaded_list_request(id: u64) -> Value {
        json!({
            "id": id,
            "method": "thread/loaded/list",
            "params": {}
        })
    }

    /// 编码 thread/list request。
    pub fn thread_list_request(id: u64, limit: usize) -> Value {
        json!({
            "id": id,
            "method": "thread/list",
            "params": {
                "limit": limit
            }
        })
    }

    /// 清洗 app-server thread/list response。
    pub fn threads_from_response(
        response: &Value,
    ) -> Result<Vec<CodexAppThreadMetadata>, CodexAppAdapterError> {
        let threads = response
            .get("threads")
            .and_then(Value::as_array)
            .ok_or(CodexAppAdapterError::MissingField("threads"))?;
        let mut output = Vec::new();
        for thread in threads {
            if let Ok(metadata) = CodexAppThreadMetadata::from_value(thread) {
                output.push(metadata);
            }
        }

        Ok(output)
    }
}

/// Codex APP app-server thread 清洗后元数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexAppThreadMetadata {
    /// Thread ID。
    pub id: String,
    /// 可选真实 cwd；缺失时只能作为 rollout path 候选。
    pub cwd: Option<String>,
    /// 可选标题。
    pub name: Option<String>,
    /// app-server 预览文本。
    pub preview: Option<String>,
    /// 可选 rollout 路径。
    pub path: Option<PathBuf>,
    /// 状态类型。
    pub status_type: String,
    /// 是否是临时 thread。
    pub ephemeral: bool,
}

impl CodexAppThreadMetadata {
    /// 从第三方 JSON 清洗 thread 元数据。
    fn from_value(value: &Value) -> Result<Self, CodexAppAdapterError> {
        let object = value
            .as_object()
            .ok_or(CodexAppAdapterError::InvalidField("thread"))?;
        let id = required_string(object.get("id"), "thread.id")?;
        let cwd = optional_non_empty_string(object.get("cwd"), "thread.cwd")?;
        let name = optional_thread_title(object.get("name"), "thread.name")?;
        let preview = optional_string(object.get("preview"), "thread.preview")?;
        let path = optional_non_empty_string(object.get("path"), "thread.path")?.map(PathBuf::from);
        if cwd.is_none() && path.is_none() {
            return Err(CodexAppAdapterError::MissingField("thread.cwd"));
        }
        let status_type = thread_metadata_status_type(object.get("status"))?;
        let ephemeral = object
            .get("ephemeral")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        Ok(Self {
            id,
            cwd,
            name,
            preview,
            path,
            status_type,
            ephemeral,
        })
    }
}

/// Codex 本地 session index 条目。
#[derive(Deserialize)]
struct CodexSessionIndexEntry {
    /// Thread ID。
    id: String,
    /// Codex UI 侧边栏展示标题。
    thread_name: Option<String>,
}

/// 返回默认 Codex session index 路径。
pub fn default_codex_session_index_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".codex")
        .join("session_index.jsonl")
}

/// 从 Codex session index 读取真实 thread 标题。
pub fn load_codex_session_index_titles(path: &Path) -> BTreeMap<String, String> {
    let Ok(file) = fs::File::open(path) else {
        return BTreeMap::new();
    };

    codex_session_index_titles_from_reader(BufReader::new(file))
}

/// 用 Codex session index 中的真实标题覆盖 app-server thread 名。
pub fn apply_session_index_thread_titles(
    threads: &mut [CodexAppThreadMetadata],
    titles: &BTreeMap<String, String>,
) {
    for thread in threads {
        let Some(title) = titles.get(&thread.id) else {
            continue;
        };
        thread.name = clean_thread_title(title);
    }
}

/// 从 reader 清洗 Codex session index 标题。
fn codex_session_index_titles_from_reader(reader: impl BufRead) -> BTreeMap<String, String> {
    let mut titles = BTreeMap::new();
    for line in reader.lines().map_while(Result::ok) {
        let Ok(entry) = serde_json::from_str::<CodexSessionIndexEntry>(&line) else {
            continue;
        };
        let Some(thread_name) = entry.thread_name else {
            continue;
        };
        let thread_name = clean_thread_title(&thread_name);
        let id = entry.id.trim();
        if id.is_empty() {
            continue;
        }
        let Some(thread_name) = thread_name else {
            continue;
        };
        titles.insert(id.to_string(), thread_name);
    }

    titles
}

/// 清洗 app-server thread status.type。
fn thread_metadata_status_type(status: Option<&Value>) -> Result<String, CodexAppAdapterError> {
    let Some(status) = status else {
        return Ok("idle".to_string());
    };
    let object = status
        .as_object()
        .ok_or(CodexAppAdapterError::InvalidField("thread.status"))?;
    let Some(status_type) = object.get("type") else {
        return Ok("idle".to_string());
    };
    let status_type = status_type
        .as_str()
        .ok_or(CodexAppAdapterError::InvalidField("thread.status.type"))?;

    Ok(status_type.to_string())
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
    /// Codex APP thread 元数据缓存。
    thread_metadata: BTreeMap<String, CodexAppThreadMetadata>,
    /// Codex APP thread 到 rollout path 的映射。
    thread_rollout_paths: BTreeMap<String, PathBuf>,
    /// 当前 turn 已累积的 Agent 输出。
    current_turn_agent_outputs: BTreeMap<String, String>,
    /// 已发起但尚未完成写入确认的 follow-up turn。
    pending_followup_turns: BTreeMap<SessionKey, String>,
    /// 托管事件时间线缓存。
    timeline_cache: InMemoryProcessTimelineCache,
    /// 清洗后 session 更新发布端口。
    update_sink: Arc<dyn SessionUpdateSinkPort>,
    /// Codex APP 收录新 thread 时通知 Codex CLI runtime 清理同名孤儿 session。
    orphan_eviction_callback: Option<Arc<dyn Fn(&str, &str, UnixMillis) + Send + Sync>>,
}

impl CodexAppRuntime {
    /// 创建空 Codex APP runtime。
    pub fn empty() -> Self {
        Self::with_update_sink(Arc::new(NoopSessionUpdateSink))
    }

    /// 使用指定更新端口创建空 Codex APP runtime。
    pub fn with_update_sink(update_sink: Arc<dyn SessionUpdateSinkPort>) -> Self {
        Self {
            session_state: SessionState::empty(),
            pending_hook_approvals: BTreeMap::new(),
            pending_rpc_approvals: BTreeMap::new(),
            pending_rpc_answers: BTreeMap::new(),
            pending_rpc_submissions: BTreeSet::new(),
            thread_cwds: BTreeMap::new(),
            thread_metadata: BTreeMap::new(),
            thread_rollout_paths: BTreeMap::new(),
            current_turn_agent_outputs: BTreeMap::new(),
            pending_followup_turns: BTreeMap::new(),
            timeline_cache: InMemoryProcessTimelineCache::new(),
            update_sink,
            orphan_eviction_callback: None,
        }
    }

    /// 注入 Codex APP 收录新 thread 时的孤儿 session 迁移回调。
    ///
    /// 该回调由 commands.rs 注入,接受 (cwd, thread_id, updated_at),让
    /// codex_cli_runtime 清理同名误存条目。重复设置只保留最后一次。
    pub fn set_orphan_eviction_callback(
        &mut self,
        callback: Arc<dyn Fn(&str, &str, UnixMillis) + Send + Sync>,
    ) {
        self.orphan_eviction_callback = Some(callback);
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

    /// 返回当前已知 rollout tail 目标。
    pub fn rollout_watch_targets(&self) -> Vec<CodexRolloutWatchTarget> {
        self.thread_rollout_paths
            .iter()
            .filter_map(|metadata| {
                let (thread_id, path) = metadata;
                let cwd = self.thread_cwds.get(thread_id)?;
                let target_key = session_key(cwd, thread_id);
                if !self.session_state.sessions.contains_key(&target_key) {
                    return None;
                }

                Some(CodexRolloutWatchTarget {
                    session_key: target_key,
                    path: path.clone(),
                })
            })
            .collect()
    }

    /// 应用 rollout tail 清洗出的实时事件。
    pub fn apply_rollout_event(&mut self, event: AgentEvent) -> Result<(), AppError> {
        self.apply_event(event)
    }

    /// 返回仍缺真实 cwd 的 Codex APP thread ID。
    pub fn unresolved_thread_ids(&self) -> Vec<String> {
        self.session_state
            .sessions
            .keys()
            .filter(|key| is_unresolved_session_key(key))
            .map(|key| key.conversation_id.value.clone())
            .collect()
    }

    /// 判断 hook payload 是否已被 Codex APP runtime 认领。
    ///
    /// 当 codex 客户端没有正确上报 `terminal_app` 时,bridge 层用这个判定把
    /// hook payload 兜底归到 CodexApp:先按 session_id 精确匹配已知 thread,
    /// 再按 cwd 匹配 codex_app 之前注册过的工作目录。两条都未命中才视为
    /// CodexCli,避免把真正的 CLI session 误归到 APP。
    pub fn claims_codex_app_thread(&self, session_id: &str, cwd: &str) -> bool {
        if self.thread_cwds.contains_key(session_id) {
            return true;
        }
        let app_session_key = session_key(cwd, session_id);
        if self.session_state.sessions.contains_key(&app_session_key) {
            return true;
        }
        self.thread_cwds.values().any(|known| known == cwd)
    }

    /// 返回已知但仍缺真实标题的 Codex APP thread ID。
    pub fn title_missing_thread_ids(&self) -> Vec<String> {
        self.session_state
            .sessions
            .values()
            .filter(|session| {
                session.session_key.agent_kind == AgentKind::CodexApp
                    && (missing_title(&session.title)
                        || session.title.as_deref().is_some_and(is_codex_model_label))
            })
            .map(|session| session.session_key.conversation_id.value.clone())
            .collect()
    }

    /// 用 Codex session index 标题补齐当前已知 session。
    pub fn apply_session_index_titles_to_known_sessions(
        &mut self,
        titles: &BTreeMap<String, String>,
        updated_at: UnixMillis,
    ) {
        let updates = self
            .session_state
            .sessions
            .iter()
            .filter_map(|(session_key, session)| {
                if session_key.agent_kind != AgentKind::CodexApp {
                    return None;
                }
                let title = titles
                    .get(&session_key.conversation_id.value)
                    .and_then(|value| clean_thread_title(value))?;
                let candidate = Some(title.clone());
                if !should_replace_session_title(&session.title, &candidate) {
                    return None;
                }

                Some((session_key.clone(), title))
            })
            .collect::<Vec<_>>();

        for (session_key, title) in updates {
            if let Some(session) = self.session_state.sessions.get_mut(&session_key) {
                if session.title.as_deref() == Some(title.as_str()) {
                    continue;
                }
                session.title = Some(title);
                session.updated_at = updated_at;
                self.publish_codex_app_session_update(&session_key, updated_at);
            }
        }
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
        if payload.agent_kind != AgentKind::CodexApp {
            return Err(protocol_error(
                "Codex APP runtime 拒绝非 CodexApp hook payload",
            ));
        }
        self.thread_cwds
            .insert(payload.session_id.clone(), payload.cwd.clone());
        if let Some(callback) = self.orphan_eviction_callback.clone() {
            callback(&payload.cwd, &payload.session_id, updated_at);
        }
        let _ = self.migrate_codex_app_thread_to_cwd(&payload.session_id, &payload.cwd)?;
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
        _cwd: &str,
        updated_at: UnixMillis,
    ) -> Result<(), AppError> {
        let object = message
            .as_object()
            .ok_or_else(|| protocol_error("Codex APP app-server 消息不是对象"))?;
        if let Some((thread_id, cwd)) = self.record_message_thread_cwd(message) {
            let _ = self.migrate_codex_app_thread_to_cwd(&thread_id, &cwd)?;
        }
        if let Some(metadata) = message_thread_metadata(message) {
            self.apply_thread_metadata(metadata, updated_at)?;
        }
        let resolved_cwd = self
            .message_cwd(message)
            .unwrap_or_else(|| UNRESOLVED_CODEX_APP_PROJECT_ID.to_string());
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
            if self.apply_agent_message_delta(message, &resolved_cwd, updated_at)? {
                return Ok(());
            }
            self.clear_current_turn_output_on_turn_start(message);
            if let Some(event) =
                CodexAppAdapter::event_from_notification(message, &resolved_cwd, updated_at)
                    .map_err(|_| protocol_error("Codex APP app-server notification 不受支持"))?
            {
                let event = self.event_with_current_agent_output(event);
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
                self.complete_interaction(session_key, None)?;
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
        _summary: &str,
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
        self.complete_interaction(session_key, None)
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
        if self.pending_followup_turns.contains_key(session_key) {
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

        self.pending_followup_turns
            .insert(session_key.clone(), prompt.to_string());
        Ok(CodexAppRpcWrite::request(
            CodexAppAdapter::turn_start_request(0, &session_key.conversation_id.value, prompt),
        ))
    }

    /// 标记 follow-up turn 已成功写入 app-server。
    pub fn complete_followup_turn(&mut self, session_key: &SessionKey) -> Result<(), AppError> {
        let prompt = self.pending_followup_turns.remove(session_key);
        self.current_turn_agent_outputs
            .remove(&session_key.conversation_id.value);
        let Some(prompt) = prompt else {
            return Ok(());
        };

        self.apply_event(AgentEvent::UserMessageUpdated(UserMessageUpdatedEvent {
            session_key: session_key.clone(),
            summary: truncate(&prompt, 120),
            updated_at: unix_now(),
        }))
    }

    /// 释放未成功写入的 follow-up turn。
    pub fn release_followup_turn(&mut self, session_key: &SessionKey) {
        self.pending_followup_turns.remove(session_key);
    }

    fn apply_event(&mut self, event: AgentEvent) -> Result<(), AppError> {
        if event.session_key().agent_kind != AgentKind::CodexApp {
            return Err(protocol_error(
                "Codex APP runtime 拒绝非 CodexApp session 事件",
            ));
        }
        self.ensure_codex_app_realtime_session(&event)?;
        self.apply_event_direct(event)
    }

    fn apply_event_direct(&mut self, event: AgentEvent) -> Result<(), AppError> {
        if event.session_key().agent_kind != AgentKind::CodexApp {
            return Err(protocol_error(
                "Codex APP runtime 拒绝非 CodexApp session 事件",
            ));
        }
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
        let notification = session_update_notification(&event);
        self.session_state = self.session_state.apply_event(event);
        if let Some(notification) = notification {
            self.update_sink.publish_session_update(notification);
        }
        if let Some((session_key, thread_id, updated_at)) = codex_app_started {
            if is_unresolved_session_key(&session_key) {
                return Ok(());
            }
            let jump_event = jump_target_event(session_key, &thread_id, updated_at);
            self.timeline_cache.record_agent_event(&jump_event)?;
            let notification = session_update_notification(&jump_event);
            self.session_state = self.session_state.apply_event(jump_event);
            if let Some(notification) = notification {
                self.update_sink.publish_session_update(notification);
            }
        }
        Ok(())
    }

    fn ensure_codex_app_realtime_session(&mut self, event: &AgentEvent) -> Result<(), AppError> {
        let session_key = event.session_key();
        debug_assert_eq!(session_key.agent_kind, AgentKind::CodexApp);
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

    fn publish_codex_app_session_update(&self, session_key: &SessionKey, updated_at: UnixMillis) {
        self.update_sink
            .publish_session_update(SessionUpdateNotification {
                runtime_source: SessionRuntimeSource::CodexApp,
                session_key: session_key.clone(),
                changed_area: SessionUpdateArea::Session,
                updated_at,
            });
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

    /// 累积并应用当前 turn 的 Agent 输出 delta。
    fn apply_agent_message_delta(
        &mut self,
        message: &Value,
        cwd: &str,
        updated_at: UnixMillis,
    ) -> Result<bool, AppError> {
        let method = message.get("method").and_then(Value::as_str);
        if method != Some("item/agentMessage/delta") {
            return Ok(false);
        }
        let Some(params) = message.get("params") else {
            return Ok(false);
        };
        let thread_id = required_string(params.get("threadId"), "threadId")
            .map_err(|_| protocol_error("Codex APP agent message delta 格式无效"))?;
        let delta = required_string(params.get("delta"), "delta")
            .map_err(|_| protocol_error("Codex APP agent message delta 格式无效"))?;
        let output = self
            .current_turn_agent_outputs
            .entry(thread_id.clone())
            .or_default();
        output.push_str(&delta);
        truncate_to_recent_chars(output, MAX_CURRENT_TURN_OUTPUT_CHARS);
        let summary = truncate(output, 240);
        self.apply_event(AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
            session_key: session_key(cwd, &thread_id),
            summary,
            updated_at,
        }))?;

        Ok(true)
    }

    /// turn/started 表示当前 thread 进入新 turn，清理上一轮输出。
    fn clear_current_turn_output_on_turn_start(&mut self, message: &Value) {
        if message.get("method").and_then(Value::as_str) != Some("turn/started") {
            return;
        }
        if let Some(thread_id) = message_thread_id(message) {
            self.current_turn_agent_outputs.remove(&thread_id);
        }
    }

    /// 完成或空闲事件优先保留当前 turn 最新 Agent 输出。
    fn event_with_current_agent_output(&self, event: AgentEvent) -> AgentEvent {
        match event {
            AgentEvent::TurnCompleted(mut event) => {
                if let Some(output) = self
                    .current_turn_agent_outputs
                    .get(&event.session_key.conversation_id.value)
                    .filter(|value| !value.trim().is_empty())
                {
                    event.summary = Some(truncate_strict(output, MAX_FINAL_OUTPUT_CHARS));
                }
                AgentEvent::TurnCompleted(event)
            }
            AgentEvent::ActivityUpdated(mut event) => {
                if let Some(output) = self
                    .current_turn_agent_outputs
                    .get(&event.session_key.conversation_id.value)
                    .filter(|value| !value.trim().is_empty())
                {
                    event.summary = truncate(output, 240);
                }
                AgentEvent::ActivityUpdated(event)
            }
            other => other,
        }
    }

    /// 合并 app-server thread 元数据，不覆盖已有实时状态。
    pub fn apply_thread_metadata(
        &mut self,
        metadata: CodexAppThreadMetadata,
        updated_at: UnixMillis,
    ) -> Result<(), AppError> {
        if metadata.ephemeral {
            return Ok(());
        }

        let thread_id = metadata.id.clone();
        if let Some(path) = metadata.path.clone() {
            self.thread_rollout_paths.insert(thread_id.clone(), path);
        }
        let Some(cwd) = metadata
            .cwd
            .clone()
            .or_else(|| self.known_cwd_for_thread(&thread_id))
        else {
            return Ok(());
        };
        let target_key = session_key(&cwd, &thread_id);
        let summary = metadata.preview.clone().filter(|value| !value.is_empty());
        let title = clean_optional_thread_title(&metadata.name);
        self.thread_cwds.insert(thread_id.clone(), cwd.clone());
        self.thread_metadata
            .insert(thread_id.clone(), metadata.clone());
        if let Some(callback) = self.orphan_eviction_callback.clone() {
            callback(&cwd, &thread_id, updated_at);
        }
        let migrated = self.migrate_codex_app_thread_to_cwd(&thread_id, &cwd)?;

        if let Some(session) = self.session_state.sessions.get_mut(&target_key) {
            let mut changed = migrated;
            let next_project_label = project_label(&cwd);
            if session.project_label != next_project_label {
                session.project_label = next_project_label;
                changed = true;
            }
            if session.conversation_label != thread_id {
                session.conversation_label = thread_id.clone();
                changed = true;
            }
            if should_replace_session_title(&session.title, &title) && session.title != title {
                session.title = title.clone();
                changed = true;
            }
            if session.summary.is_none() && summary.is_some() {
                session.summary = summary.clone();
                changed = true;
            }
            if session.capabilities == SessionCapabilities::none()
                || is_unresolved_session_key(&session.session_key)
            {
                let next_capabilities = codex_app_capabilities();
                if session.capabilities != next_capabilities {
                    session.capabilities = next_capabilities;
                    changed = true;
                }
            }
            if session.jump_target.is_none() {
                session.jump_target = Some(codex_app_jump_target(
                    &session.session_key.conversation_id.value,
                ));
                changed = true;
            }
            if changed {
                self.publish_codex_app_session_update(&target_key, updated_at);
            }
            return Ok(());
        }

        self.apply_event_direct(AgentEvent::SessionStarted(SessionStartedEvent {
            session_key: target_key.clone(),
            project_label: project_label(&cwd),
            conversation_label: metadata.id.clone(),
            title,
            summary: summary.clone(),
            capabilities: codex_app_capabilities(),
            usage: UsageSnapshot::unavailable(),
            updated_at,
        }))?;

        self.apply_thread_metadata_status(&target_key, &metadata.status_type, summary, updated_at)
    }

    /// 返回 runtime 已信任的 thread cwd。
    fn known_cwd_for_thread(&self, thread_id: &str) -> Option<String> {
        self.thread_cwds.get(thread_id).cloned().or_else(|| {
            self.session_state
                .sessions
                .keys()
                .find(|key| {
                    key.agent_kind == AgentKind::CodexApp
                        && key.conversation_id.value == thread_id
                        && !is_unresolved_session_key(key)
                })
                .map(|key| key.project_id.value.clone())
        })
    }

    /// 按 app-server thread 元数据受控折叠 session 状态。
    fn apply_thread_metadata_status(
        &mut self,
        target_key: &SessionKey,
        status_type: &str,
        summary: Option<String>,
        updated_at: UnixMillis,
    ) -> Result<(), AppError> {
        match status_type {
            "idle" => self.apply_event_direct(AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key: target_key.clone(),
                summary,
                updated_at,
            })),
            "systemError" => self.apply_event_direct(AgentEvent::Failed(FailedEvent {
                session_key: target_key.clone(),
                error: app_server_error("Codex APP thread 系统错误", String::new()),
                updated_at,
            })),
            "notLoaded" => self.apply_event_direct(AgentEvent::Detached(DetachedEvent {
                session_key: target_key.clone(),
                reason: Some("Codex APP thread 未加载".to_string()),
                updated_at,
            })),
            _ => Ok(()),
        }
    }

    /// 使用 rollout 快照补齐真实 cwd 和最新 Agent 输出。
    pub fn apply_rollout_snapshot(
        &mut self,
        snapshot: CodexRolloutSnapshot,
    ) -> Result<(), AppError> {
        self.thread_cwds
            .insert(snapshot.session_id.clone(), snapshot.cwd.clone());
        self.thread_rollout_paths
            .insert(snapshot.session_id.clone(), snapshot.path.clone());
        let migrated = self.migrate_codex_app_thread_to_cwd(&snapshot.session_id, &snapshot.cwd)?;
        let target_key = session_key(&snapshot.cwd, &snapshot.session_id);
        let Some(session) = self.session_state.sessions.get_mut(&target_key) else {
            return Ok(());
        };

        let mut changed = migrated;
        let next_project_label = project_label(&snapshot.cwd);
        if session.project_label != next_project_label {
            session.project_label = next_project_label;
            changed = true;
        }
        if session.capabilities == SessionCapabilities::none() {
            let next_capabilities = codex_app_capabilities();
            if session.capabilities != next_capabilities {
                session.capabilities = next_capabilities;
                changed = true;
            }
        }
        if session.jump_target.is_none() {
            session.jump_target = Some(codex_app_jump_target(
                &session.session_key.conversation_id.value,
            ));
            changed = true;
        }
        if should_apply_rollout_summary(session) {
            if let Some(summary) = snapshot.summary {
                if session.summary.as_ref() != Some(&summary) {
                    session.summary = Some(summary);
                    changed = true;
                }
            }
        }
        if changed {
            self.publish_codex_app_session_update(&target_key, snapshot.updated_at);
        }

        Ok(())
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
        summary: Option<String>,
    ) -> Result<(), AppError> {
        self.apply_event(AgentEvent::InteractionCompleted(
            InteractionCompletedEvent {
                session_key: session_key.clone(),
                summary,
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

    fn message_cwd(&self, message: &Value) -> Option<String> {
        if let Some(cwd) = message_thread_cwd(message) {
            return Some(cwd);
        }
        if let Some(thread_id) = message_thread_id(message) {
            if let Some(cwd) = self.thread_cwds.get(&thread_id) {
                return Some(cwd.clone());
            }
        }

        None
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
    ) -> Result<bool, AppError> {
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

        let mut changed = false;
        for stale_key in stale_keys {
            changed = self.migrate_session_key(&stale_key, &target_key)? || changed;
        }

        Ok(changed)
    }

    fn migrate_session_key(
        &mut self,
        stale_key: &SessionKey,
        target_key: &SessionKey,
    ) -> Result<bool, AppError> {
        let Some(mut stale_session) = self.session_state.sessions.remove(stale_key) else {
            return Ok(false);
        };

        stale_session.session_key = target_key.clone();
        stale_session.project_label = project_label(&target_key.project_id.value);
        stale_session.conversation_label = target_key.conversation_id.value.clone();
        stale_session.pending_interaction = stale_session
            .pending_interaction
            .take()
            .map(|interaction| interaction.aligned_to_session_key(target_key));
        if !is_unresolved_session_key(target_key) {
            if stale_session.capabilities == SessionCapabilities::none()
                || !stale_session.capabilities.can_jump
            {
                stale_session.capabilities = codex_app_capabilities();
            }
            if stale_session.jump_target.is_none() {
                stale_session.jump_target =
                    Some(codex_app_jump_target(&target_key.conversation_id.value));
            }
        }

        if let Some(target_session) = self.session_state.sessions.get_mut(target_key) {
            if target_session.pending_interaction.is_none() {
                target_session.pending_interaction = stale_session.pending_interaction.take();
            }
            if target_session.summary.is_none() {
                target_session.summary = stale_session.summary;
            }
            if missing_title(&target_session.title) {
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
        if let Some(prompt) = self.pending_followup_turns.remove(stale_key) {
            self.pending_followup_turns
                .insert(target_key.clone(), prompt);
        }
        self.timeline_cache
            .migrate_session_key(stale_key, target_key)?;

        Ok(true)
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
    if let Some(cwd) = clean_non_empty_value(params.get("cwd")) {
        return Some(cwd);
    }
    if let Some(cwd) = params
        .get("thread")
        .and_then(|thread| clean_non_empty_value(thread.get("cwd")))
    {
        return Some(cwd);
    }

    None
}

fn message_thread_metadata(message: &Value) -> Option<CodexAppThreadMetadata> {
    let thread = message.get("params")?.get("thread")?;
    CodexAppThreadMetadata::from_value(thread).ok()
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

    /// 读取已加载 Codex APP threads。
    pub fn list_loaded_threads(&self) -> Result<Vec<CodexAppThreadMetadata>, AppError> {
        let response = self.send_request_value_with_timeout(
            CodexAppAdapter::thread_loaded_list_request(self.next_request_id()?),
            APP_SERVER_THREAD_LIST_TIMEOUT,
        )?;
        CodexAppAdapter::threads_from_response(&response)
            .map_err(|_| app_server_error("Codex APP thread 列表格式无效", String::new()))
    }

    /// 读取 Codex APP thread 历史。
    pub fn list_threads(&self, limit: usize) -> Result<Vec<CodexAppThreadMetadata>, AppError> {
        let response = self.send_request_value_with_timeout(
            CodexAppAdapter::thread_list_request(self.next_request_id()?, limit),
            APP_SERVER_THREAD_LIST_TIMEOUT,
        )?;
        CodexAppAdapter::threads_from_response(&response)
            .map_err(|_| app_server_error("Codex APP thread 历史格式无效", String::new()))
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
        self.send_request_value_with_timeout(request, APP_SERVER_REQUEST_TIMEOUT)
    }

    fn send_request_value_with_timeout(
        &self,
        request: Value,
        timeout: Duration,
    ) -> Result<Value, AppError> {
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
        self.wait_response(request_id, timeout)
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

    fn wait_response(&self, request_id: u64, timeout: Duration) -> Result<Value, AppError> {
        let (lock, condvar) = &*self.pending;
        let pending = lock
            .lock()
            .map_err(|_| app_server_error("Codex APP pending request 锁已损坏", String::new()))?;
        let (mut pending, timeout) = condvar
            .wait_timeout_while(pending, timeout, |pending| {
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
    let title = optional_thread_title(thread.get("name"), "thread.name")?;
    let session_key = session_key(cwd, &thread_id);
    let capabilities = codex_app_capabilities_for_key(&session_key);

    Ok(AgentEvent::SessionStarted(SessionStartedEvent {
        session_key,
        project_label: project_label(cwd),
        conversation_label: thread_id,
        title,
        summary: None,
        capabilities,
        usage: UsageSnapshot::unavailable(),
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
        summary: truncate(&delta, 120),
        updated_at,
    }))
}

fn title_updated(
    params: &Value,
    cwd: &str,
    updated_at: UnixMillis,
) -> Result<Option<AgentEvent>, CodexAppAdapterError> {
    let thread_id = required_string(params.get("threadId"), "threadId")?;
    let Some(title) = optional_thread_title(params.get("name"), "name")? else {
        return Ok(None);
    };

    Ok(Some(AgentEvent::TitleUpdated(TitleUpdatedEvent {
        session_key: session_key(cwd, &thread_id),
        title,
        updated_at,
    })))
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
        "active" => Ok(None),
        "idle" => Ok(Some(AgentEvent::TurnCompleted(TurnCompletedEvent {
            session_key,
            summary: None,
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
        summary: None,
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
        title: None,
        summary: None,
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
        summary: None,
        capabilities: codex_app_capabilities_for_key(&session_key),
        usage: UsageSnapshot::unavailable(),
        session_key,
        updated_at,
    }
}

fn event_updated_at(event: &AgentEvent) -> UnixMillis {
    match event {
        AgentEvent::SessionStarted(event) => event.updated_at,
        AgentEvent::ActivityUpdated(event) => event.updated_at,
        AgentEvent::UserMessageUpdated(event) => event.updated_at,
        AgentEvent::TitleUpdated(event) => event.updated_at,
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

fn approval_request_summary(_method: &str, params: &Value) -> Result<String, CodexAppAdapterError> {
    let command = optional_string(params.get("command"), "command")?;
    let reason = optional_string(params.get("reason"), "reason")?;
    let item_id = optional_string(params.get("itemId"), "itemId")?;
    let Some(subject) = command.or(reason).or(item_id) else {
        return Ok(String::new());
    };

    Ok(truncate(&subject, 120))
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

fn optional_non_empty_string(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Option<String>, CodexAppAdapterError> {
    let Some(_) = value else {
        return Ok(None);
    };
    clean_non_empty_value(value).map_or_else(
        || optional_string(value, field).map(|_| None),
        |text| Ok(Some(text)),
    )
}

fn clean_non_empty_value(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?.trim();
    if text.is_empty() {
        return None;
    }

    Some(text.to_string())
}

fn optional_thread_title(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Option<String>, CodexAppAdapterError> {
    let Some(title) = optional_non_empty_string(value, field)? else {
        return Ok(None);
    };

    Ok(clean_thread_title(&title))
}

fn clean_optional_thread_title(title: &Option<String>) -> Option<String> {
    title.as_deref().and_then(clean_thread_title)
}

pub(crate) fn clean_thread_title(title: &str) -> Option<String> {
    let title = title.trim();
    if title.is_empty() || is_codex_model_label(title) {
        return None;
    }

    Some(title.to_string())
}

pub(crate) fn missing_title(title: &Option<String>) -> bool {
    title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
}

/// 判断是否应该用新的 thread 标题替换当前标题。
pub(crate) fn should_replace_session_title(
    current: &Option<String>,
    candidate: &Option<String>,
) -> bool {
    if missing_title(candidate) {
        return false;
    }
    if candidate.as_deref().is_some_and(is_codex_model_label) {
        return false;
    }

    missing_title(current) || current.as_deref().is_some_and(is_codex_model_label)
}

/// 判断当前标题是否像 Codex 模型名而不是真实 thread 名。
pub(crate) fn is_codex_model_label(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    let has_model_prefix = normalized.starts_with("gpt-")
        || normalized == "o1"
        || normalized.starts_with("o1-")
        || normalized == "o3"
        || normalized.starts_with("o3-")
        || normalized == "o4"
        || normalized.starts_with("o4-")
        || normalized == "o5"
        || normalized.starts_with("o5-");

    has_model_prefix
        && normalized.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '.' | '_')
        })
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

fn codex_app_capabilities_for_key(session_key: &SessionKey) -> SessionCapabilities {
    if is_unresolved_session_key(session_key) {
        return SessionCapabilities {
            can_jump: false,
            can_send_reply: true,
            can_resolve_approval: true,
            can_create_followup_turn: true,
            can_view_process_timeline: true,
        };
    }

    codex_app_capabilities()
}

fn project_label(cwd: &str) -> String {
    if cwd == UNRESOLVED_CODEX_APP_PROJECT_ID {
        return UNRESOLVED_CODEX_APP_PROJECT_LABEL.to_string();
    }

    let cwd = cwd.trim_end_matches('/');
    for marker in ["/.claude/worktrees/", "/.git/worktrees/"] {
        if let Some((project_path, _)) = cwd.split_once(marker) {
            if let Some(project_name) = project_path
                .rsplit('/')
                .find(|value| !value.trim().is_empty())
            {
                return project_name.to_string();
            }
        }
    }

    cwd.rsplit('/')
        .next()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(cwd)
        .to_string()
}

fn is_unresolved_session_key(session_key: &SessionKey) -> bool {
    session_key.project_id.value == UNRESOLVED_CODEX_APP_PROJECT_ID
}

fn should_apply_rollout_summary(session: &crate::domain::agent_session::AgentSession) -> bool {
    matches!(
        session.status,
        SessionStatus::Completed | SessionStatus::Failed
    ) || session.summary.is_none()
}

fn session_update_notification(event: &AgentEvent) -> Option<SessionUpdateNotification> {
    let session_key = event.session_key().clone();
    let runtime_source = SessionRuntimeSource::from_agent_kind(&session_key.agent_kind)?;

    Some(SessionUpdateNotification {
        runtime_source,
        session_key,
        changed_area: SessionUpdateArea::Both,
        updated_at: event_updated_at(event),
    })
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

fn truncate_strict(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn truncate_to_recent_chars(value: &mut String, max_chars: usize) {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return;
    }

    let keep_from = char_count - max_chars;
    let byte_index = value
        .char_indices()
        .nth(keep_from)
        .map(|(index, _)| index)
        .unwrap_or(0);
    value.drain(..byte_index);
}

fn hook_interaction_id(request_id: &str) -> InteractionId {
    InteractionId::new(format!("codex-app-hook-{request_id}"))
}

fn rpc_interaction_id(request_id: &str) -> InteractionId {
    InteractionId::new(format!("codex-app-rpc-{request_id}"))
}

fn prompt_summary(payload: &ValidatedHookPayload) -> Option<String> {
    payload.prompt.as_ref().map(|prompt| truncate(prompt, 120))
}

fn hook_tool_preview(payload: &ValidatedHookPayload) -> Option<String> {
    let tool_input = payload.tool_input.as_ref()?;
    if let Some(command) = tool_input.get("command").and_then(Value::as_str) {
        return Some(truncate(command, 120));
    }
    if let Some(description) = tool_input.get("description").and_then(Value::as_str) {
        return Some(truncate(description, 120));
    }

    None
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
    use std::path::PathBuf;
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::Duration;

    use serde_json::{json, Value};

    use super::{
        app_server_protocol_error_response, apply_session_index_thread_titles,
        codex_app_capabilities, codex_session_index_titles_from_reader, handle_rpc_response,
        schema_probe_from_dir, session_key, CodexAppAdapter, CodexAppRuntime,
        CodexAppThreadMetadata, CodexRolloutSnapshot, CodexRolloutWatchTarget, PendingRpcResult,
        MAX_CURRENT_TURN_OUTPUT_CHARS, MAX_FINAL_OUTPUT_CHARS, UNRESOLVED_CODEX_APP_PROJECT_ID,
        UNRESOLVED_CODEX_APP_PROJECT_LABEL,
    };
    use crate::adapters::bridge::codec::{
        BridgeHookEventName, BridgeRequestEnvelope, ValidatedHookPayload,
    };
    use crate::domain::agent_event::{AgentEvent, SessionStartedEvent, TurnCompletedEvent};
    use crate::domain::agent_interaction::AgentInteraction;
    use crate::domain::agent_session::{AgentKind, SessionStatus};
    use crate::domain::usage::{UnixMillis, UsageSnapshot};
    use crate::domain::view_model::UiAction;
    use crate::ports::agent_adapter_port::{ApprovalDecision, ChoiceSubmission};
    use crate::ports::process_timeline_port::ProcessTimelineReaderPort;
    use crate::ports::session_update_port::{
        SessionUpdateArea, SessionUpdateNotification, SessionUpdateSinkPort,
    };

    #[derive(Default)]
    struct RecordingSessionUpdateSink {
        notifications: Mutex<Vec<SessionUpdateNotification>>,
    }

    impl RecordingSessionUpdateSink {
        fn notifications(&self) -> Vec<SessionUpdateNotification> {
            self.notifications
                .lock()
                .expect("notifications should lock")
                .clone()
        }
    }

    impl SessionUpdateSinkPort for RecordingSessionUpdateSink {
        fn publish_session_update(&self, notification: SessionUpdateNotification) {
            self.notifications
                .lock()
                .expect("notifications should lock")
                .push(notification);
        }
    }

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
        assert_eq!(event.title.as_deref(), Some("阶段 4"));
    }

    #[test]
    fn thread_started_treats_blank_name_as_missing() {
        let notification = json!({
            "method": "thread/started",
            "params": {
                "thread": {
                    "id": "thread-1",
                    "name": "   "
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
        assert_eq!(event.title, None);
    }

    #[test]
    fn thread_started_treats_model_name_as_missing() {
        let notification = json!({
            "method": "thread/started",
            "params": {
                "thread": {
                    "id": "thread-1",
                    "name": "gpt-5.5"
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
        assert_eq!(event.title, None);
    }

    #[test]
    fn thread_name_updated_notification_maps_to_title_update() {
        let notification = json!({
            "method": "thread/name/updated",
            "params": {
                "threadId": "thread-1",
                "name": "说明身份"
            }
        });

        let event = CodexAppAdapter::event_from_notification(
            &notification,
            "/tmp/builder-panel",
            UnixMillis::new(2),
        )
        .expect("notification should parse")
        .expect("event should exist");

        let AgentEvent::TitleUpdated(event) = event else {
            panic!("event should update title");
        };
        assert_eq!(
            event.session_key,
            session_key("/tmp/builder-panel", "thread-1")
        );
        assert_eq!(event.title, "说明身份");
    }

    #[test]
    fn thread_name_updated_ignores_model_name() {
        let notification = json!({
            "method": "thread/name/updated",
            "params": {
                "threadId": "thread-1",
                "name": "gpt-5.5"
            }
        });

        let event = CodexAppAdapter::event_from_notification(
            &notification,
            "/tmp/builder-panel",
            UnixMillis::new(2),
        )
        .expect("notification should parse");

        assert!(event.is_none());
    }

    #[test]
    fn thread_started_uses_worktree_project_label() {
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
            "/Users/test/project/.claude/worktrees/feature",
            UnixMillis::new(1),
        )
        .expect("notification should parse")
        .expect("event should exist");

        let AgentEvent::SessionStarted(event) = event else {
            panic!("event should be started");
        };
        assert_eq!(event.project_label, "project");
    }

    #[test]
    fn hook_post_tool_use_does_not_write_thinking_step() {
        let payload = hook_payload(BridgeHookEventName::PostToolUse);

        let events =
            CodexAppAdapter::events_from_hook_payload("request-1", &payload, UnixMillis::new(1))
                .expect("payload should map");

        assert!(events.is_empty());
    }

    #[test]
    fn hook_stop_keeps_multiline_final_output() {
        let mut payload = hook_payload(BridgeHookEventName::Stop);
        payload.last_assistant_message = Some("第一段\n\n第二段".to_string());

        let events =
            CodexAppAdapter::events_from_hook_payload("request-1", &payload, UnixMillis::new(1))
                .expect("payload should map");

        let AgentEvent::TurnCompleted(event) = &events[0] else {
            panic!("event should complete");
        };
        assert_eq!(event.summary.as_deref(), Some("第一段\n\n第二段"));
    }

    #[test]
    fn hook_stop_final_output_respects_strict_limit() {
        let mut payload = hook_payload(BridgeHookEventName::Stop);
        payload.last_assistant_message = Some("甲".repeat(MAX_FINAL_OUTPUT_CHARS + 1));

        let events =
            CodexAppAdapter::events_from_hook_payload("request-1", &payload, UnixMillis::new(1))
                .expect("payload should map");

        let AgentEvent::TurnCompleted(event) = &events[0] else {
            panic!("event should complete");
        };
        let summary = event.summary.as_ref().expect("summary should exist");
        assert_eq!(summary.chars().count(), MAX_FINAL_OUTPUT_CHARS);
    }

    #[test]
    fn app_server_thread_started_runtime_adds_jump_target() {
        let mut runtime = CodexAppRuntime::empty();
        let message = json!({
            "method": "thread/started",
            "params": {
                "thread": {
                    "id": "thread-1",
                    "cwd": "/tmp/builder-panel",
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
    fn thread_response_skips_invalid_items() {
        let response = json!({
            "threads": [
                {
                    "id": "thread-1",
                    "cwd": "/tmp/builder-panel",
                    "name": "Thread 1"
                },
                {
                    "id": "path-only-thread",
                    "cwd": "   ",
                    "path": " /tmp/rollout-path-only.jsonl "
                },
                {
                    "id": "blank-cwd-thread",
                    "cwd": "   "
                },
                {
                    "id": "blank-path-thread",
                    "path": "   "
                },
                {
                    "id": "bad-thread"
                },
                {
                    "id": "bad-status-thread",
                    "cwd": "/tmp/bad-status",
                    "status": {"type": 123}
                }
            ]
        });

        let threads =
            CodexAppAdapter::threads_from_response(&response).expect("response should clean");

        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].id, "thread-1");
        assert_eq!(threads[1].id, "path-only-thread");
        assert!(threads[1].cwd.is_none());
        assert_eq!(
            threads[1].path.as_deref(),
            Some(std::path::Path::new("/tmp/rollout-path-only.jsonl"))
        );
    }

    #[test]
    fn thread_response_filters_model_name_without_filtering_real_title() {
        let response = json!({
            "threads": [
                {
                    "id": "model-thread",
                    "cwd": "/tmp/builder-panel",
                    "name": " GPT-5.5 "
                },
                {
                    "id": "codex-model-thread",
                    "cwd": "/tmp/builder-panel",
                    "name": "gpt-5.5-codex-max"
                },
                {
                    "id": "real-title-thread",
                    "cwd": "/tmp/builder-panel",
                    "name": "gpt-5.5 迁移说明"
                }
            ]
        });

        let threads =
            CodexAppAdapter::threads_from_response(&response).expect("response should clean");

        assert_eq!(threads[0].name, None);
        assert_eq!(threads[1].name, None);
        assert_eq!(threads[2].name.as_deref(), Some("gpt-5.5 迁移说明"));
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
        assert_eq!(session.title, None);
        let Some(AgentInteraction::Approval(interaction)) = &session.pending_interaction else {
            panic!("session should wait for approval");
        };
        assert_eq!(interaction.request_summary, "cargo test");
        assert_eq!(session.summary, None);
        assert!(session.capabilities.can_create_followup_turn);
        assert!(session.capabilities.can_view_process_timeline);
    }

    #[test]
    fn hook_session_uses_thread_metadata_name_as_title() {
        let mut runtime = CodexAppRuntime::empty();
        let request = BridgeRequestEnvelope::process_agent_hook(
            "request-1".to_string(),
            hook_payload(BridgeHookEventName::SessionStart),
        );

        runtime
            .apply_hook_request(&request, UnixMillis::new(1))
            .expect("hook should apply");
        runtime
            .apply_thread_metadata(
                CodexAppThreadMetadata {
                    id: "thread-1".to_string(),
                    cwd: Some("/tmp/builder-panel".to_string()),
                    name: Some("真实显示名称很长".to_string()),
                    preview: None,
                    path: None,
                    status_type: "active".to_string(),
                    ephemeral: false,
                },
                UnixMillis::new(2),
            )
            .expect("metadata should apply");

        let session = runtime
            .session_state()
            .sessions
            .values()
            .next()
            .expect("session should exist");

        assert_eq!(session.title.as_deref(), Some("真实显示名称很长"));
    }

    #[test]
    fn session_index_title_applies_to_known_session_without_history_thread() {
        let mut runtime = CodexAppRuntime::empty();
        let key = session_key("/tmp/builder-panel", "thread-1");
        runtime
            .apply_event_direct(AgentEvent::SessionStarted(SessionStartedEvent {
                session_key: key.clone(),
                project_label: "builder-panel".to_string(),
                conversation_label: "thread-1".to_string(),
                title: None,
                summary: None,
                capabilities: codex_app_capabilities(),
                usage: UsageSnapshot::unavailable(),
                updated_at: UnixMillis::new(1),
            }))
            .expect("thread started should apply");
        let titles = BTreeMap::from([(
            "thread-1".to_string(),
            "梳理确认项并生成开发计划".to_string(),
        )]);

        runtime.apply_session_index_titles_to_known_sessions(&titles, UnixMillis::new(2));

        let session = runtime
            .session_state()
            .sessions
            .get(&key)
            .expect("session should exist");
        assert_eq!(session.title.as_deref(), Some("梳理确认项并生成开发计划"));
        assert_eq!(session.updated_at, UnixMillis::new(2));
    }

    #[test]
    fn session_index_title_replaces_known_model_title_without_history_thread() {
        let mut runtime = CodexAppRuntime::empty();
        let key = session_key("/tmp/builder-panel", "thread-1");
        runtime
            .apply_event_direct(AgentEvent::SessionStarted(SessionStartedEvent {
                session_key: key.clone(),
                project_label: "builder-panel".to_string(),
                conversation_label: "thread-1".to_string(),
                title: Some("gpt-5.5".to_string()),
                summary: None,
                capabilities: codex_app_capabilities(),
                usage: UsageSnapshot::unavailable(),
                updated_at: UnixMillis::new(1),
            }))
            .expect("thread started should apply");
        let titles = BTreeMap::from([("thread-1".to_string(), "说明身份".to_string())]);

        runtime.apply_session_index_titles_to_known_sessions(&titles, UnixMillis::new(2));

        let session = runtime
            .session_state()
            .sessions
            .get(&key)
            .expect("session should exist");
        assert_eq!(session.title.as_deref(), Some("说明身份"));
    }

    #[test]
    fn session_index_title_does_not_override_existing_real_title_or_create_session() {
        let mut runtime = CodexAppRuntime::empty();
        let key = session_key("/tmp/builder-panel", "thread-1");
        runtime
            .apply_event_direct(AgentEvent::SessionStarted(SessionStartedEvent {
                session_key: key.clone(),
                project_label: "builder-panel".to_string(),
                conversation_label: "thread-1".to_string(),
                title: Some("已有标题".to_string()),
                summary: None,
                capabilities: codex_app_capabilities(),
                usage: UsageSnapshot::unavailable(),
                updated_at: UnixMillis::new(1),
            }))
            .expect("thread started should apply");
        let titles = BTreeMap::from([
            ("thread-1".to_string(), "新标题".to_string()),
            ("thread-2".to_string(), "无关标题".to_string()),
        ]);

        runtime.apply_session_index_titles_to_known_sessions(&titles, UnixMillis::new(2));

        let session = runtime
            .session_state()
            .sessions
            .get(&key)
            .expect("session should exist");
        assert_eq!(session.title.as_deref(), Some("已有标题"));
        assert_eq!(runtime.session_state().sessions.len(), 1);
    }

    #[test]
    fn path_only_thread_metadata_updates_known_session_without_creating_unrelated_session() {
        let mut runtime = CodexAppRuntime::empty();
        let key = session_key("/tmp/builder-panel", "thread-1");
        runtime
            .apply_event_direct(AgentEvent::SessionStarted(SessionStartedEvent {
                session_key: key.clone(),
                project_label: "builder-panel".to_string(),
                conversation_label: "thread-1".to_string(),
                title: None,
                summary: None,
                capabilities: codex_app_capabilities(),
                usage: UsageSnapshot::unavailable(),
                updated_at: UnixMillis::new(1),
            }))
            .expect("thread started should apply");

        runtime
            .apply_thread_metadata(
                CodexAppThreadMetadata {
                    id: "thread-1".to_string(),
                    cwd: None,
                    name: Some("历史标题".to_string()),
                    preview: None,
                    path: Some(PathBuf::from("/tmp/rollout-thread-1.jsonl")),
                    status_type: "active".to_string(),
                    ephemeral: false,
                },
                UnixMillis::new(2),
            )
            .expect("metadata should apply");
        runtime
            .apply_thread_metadata(
                CodexAppThreadMetadata {
                    id: "unrelated-thread".to_string(),
                    cwd: None,
                    name: Some("无关标题".to_string()),
                    preview: None,
                    path: Some(PathBuf::from("/tmp/rollout-unrelated.jsonl")),
                    status_type: "active".to_string(),
                    ephemeral: false,
                },
                UnixMillis::new(3),
            )
            .expect("metadata should apply");

        let session = runtime
            .session_state()
            .sessions
            .get(&key)
            .expect("session should exist");
        assert_eq!(session.title.as_deref(), Some("历史标题"));
        assert_eq!(runtime.session_state().sessions.len(), 1);
    }

    #[test]
    fn session_index_thread_name_overrides_app_server_model_name() {
        let mut metadata = vec![CodexAppThreadMetadata {
            id: "thread-1".to_string(),
            cwd: Some("/tmp/builder-panel".to_string()),
            name: Some("gpt-5.5".to_string()),
            preview: None,
            path: None,
            status_type: "idle".to_string(),
            ephemeral: false,
        }];
        let index = r#"{"id":"thread-1","thread_name":"说明身份","updated_at":"2026-06-08T04:30:16Z"}
{"id":"thread-2","thread_name":"其它"}
"#;
        let titles = codex_session_index_titles_from_reader(index.as_bytes());

        apply_session_index_thread_titles(&mut metadata, &titles);
        let mut runtime = CodexAppRuntime::empty();
        runtime
            .apply_thread_metadata(metadata.remove(0), UnixMillis::new(1))
            .expect("metadata should apply");

        let session = runtime
            .session_state()
            .sessions
            .values()
            .next()
            .expect("session should exist");

        assert_eq!(session.title.as_deref(), Some("说明身份"));
    }

    #[test]
    fn session_index_thread_name_ignores_model_name() {
        let mut metadata = vec![CodexAppThreadMetadata {
            id: "thread-1".to_string(),
            cwd: Some("/tmp/builder-panel".to_string()),
            name: None,
            preview: None,
            path: None,
            status_type: "idle".to_string(),
            ephemeral: false,
        }];
        let index = r#"{"id":"thread-1","thread_name":"gpt-5.5","updated_at":"2026-06-08T04:30:16Z"}
"#;
        let titles = codex_session_index_titles_from_reader(index.as_bytes());

        apply_session_index_thread_titles(&mut metadata, &titles);
        let mut runtime = CodexAppRuntime::empty();
        runtime
            .apply_thread_metadata(metadata.remove(0), UnixMillis::new(1))
            .expect("metadata should apply");

        let session = runtime
            .session_state()
            .sessions
            .values()
            .next()
            .expect("session should exist");

        assert_eq!(session.title, None);
    }

    #[test]
    fn session_index_thread_name_replaces_existing_model_title() {
        let mut runtime = CodexAppRuntime::empty();
        let key = session_key("/tmp/builder-panel", "thread-1");
        runtime
            .apply_event_direct(AgentEvent::SessionStarted(SessionStartedEvent {
                session_key: key.clone(),
                project_label: "builder-panel".to_string(),
                conversation_label: "thread-1".to_string(),
                title: Some("gpt-5.5".to_string()),
                summary: None,
                capabilities: codex_app_capabilities(),
                usage: UsageSnapshot::unavailable(),
                updated_at: UnixMillis::new(1),
            }))
            .expect("thread started should apply");

        runtime
            .apply_thread_metadata(
                CodexAppThreadMetadata {
                    id: "thread-1".to_string(),
                    cwd: Some("/tmp/builder-panel".to_string()),
                    name: Some("说明身份".to_string()),
                    preview: None,
                    path: None,
                    status_type: "idle".to_string(),
                    ephemeral: false,
                },
                UnixMillis::new(2),
            )
            .expect("metadata should apply");

        let session = runtime
            .session_state()
            .sessions
            .get(&key)
            .expect("session should exist");

        assert_eq!(session.title.as_deref(), Some("说明身份"));
    }

    #[test]
    fn thread_metadata_title_replacement_publishes_session_update() {
        let sink = Arc::new(RecordingSessionUpdateSink::default());
        let mut runtime = CodexAppRuntime::with_update_sink(sink.clone());
        let key = session_key("/tmp/builder-panel", "thread-1");
        runtime
            .apply_event_direct(AgentEvent::SessionStarted(SessionStartedEvent {
                session_key: key.clone(),
                project_label: "builder-panel".to_string(),
                conversation_label: "thread-1".to_string(),
                title: Some("gpt-5.5".to_string()),
                summary: None,
                capabilities: codex_app_capabilities(),
                usage: UsageSnapshot::unavailable(),
                updated_at: UnixMillis::new(1),
            }))
            .expect("thread started should apply");
        let notification_count = sink.notifications().len();

        runtime
            .apply_thread_metadata(
                CodexAppThreadMetadata {
                    id: "thread-1".to_string(),
                    cwd: Some("/tmp/builder-panel".to_string()),
                    name: Some("说明身份".to_string()),
                    preview: None,
                    path: None,
                    status_type: "active".to_string(),
                    ephemeral: false,
                },
                UnixMillis::new(2),
            )
            .expect("metadata should apply");

        let notifications = sink.notifications();
        let session = runtime
            .session_state()
            .sessions
            .get(&key)
            .expect("session should exist");

        assert_eq!(session.title.as_deref(), Some("说明身份"));
        assert!(notifications.len() > notification_count);
        assert!(notifications.iter().any(|notification| {
            notification.session_key == key
                && notification.changed_area == SessionUpdateArea::Session
                && notification.updated_at == UnixMillis::new(2)
        }));
    }

    #[test]
    fn realtime_thread_name_update_replaces_existing_model_title() {
        let mut runtime = CodexAppRuntime::empty();
        let key = session_key("/tmp/builder-panel", "thread-1");
        runtime
            .apply_event_direct(AgentEvent::SessionStarted(SessionStartedEvent {
                session_key: key.clone(),
                project_label: "builder-panel".to_string(),
                conversation_label: "thread-1".to_string(),
                title: Some("gpt-5.5".to_string()),
                summary: Some("已有输出".to_string()),
                capabilities: codex_app_capabilities(),
                usage: UsageSnapshot::unavailable(),
                updated_at: UnixMillis::new(1),
            }))
            .expect("thread started should apply");
        runtime
            .apply_event_direct(AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key: key.clone(),
                summary: None,
                updated_at: UnixMillis::new(2),
            }))
            .expect("turn should complete");

        runtime
            .apply_app_server_message(
                &json!({
                    "method": "thread/name/updated",
                    "params": {
                        "threadId": "thread-1",
                        "cwd": "/tmp/builder-panel",
                        "name": "说明身份"
                    }
                }),
                "/wrong/cwd",
                UnixMillis::new(3),
            )
            .expect("name update should apply");

        let session = runtime
            .session_state()
            .sessions
            .get(&key)
            .expect("session should exist");
        assert_eq!(session.title.as_deref(), Some("说明身份"));
        assert_eq!(session.summary.as_deref(), Some("已有输出"));
        assert_eq!(session.status, SessionStatus::Completed);
    }

    #[test]
    fn hook_session_start_does_not_clear_existing_thread_metadata_name() {
        let mut runtime = CodexAppRuntime::empty();
        runtime
            .apply_thread_metadata(
                CodexAppThreadMetadata {
                    id: "thread-1".to_string(),
                    cwd: Some("/tmp/builder-panel".to_string()),
                    name: Some("真实显示名称".to_string()),
                    preview: None,
                    path: None,
                    status_type: "active".to_string(),
                    ephemeral: false,
                },
                UnixMillis::new(1),
            )
            .expect("metadata should apply");
        let request = BridgeRequestEnvelope::process_agent_hook(
            "request-1".to_string(),
            hook_payload(BridgeHookEventName::SessionStart),
        );

        runtime
            .apply_hook_request(&request, UnixMillis::new(2))
            .expect("hook should apply");

        let session = runtime
            .session_state()
            .sessions
            .values()
            .next()
            .expect("session should exist");

        assert_eq!(session.title.as_deref(), Some("真实显示名称"));
    }

    #[test]
    fn later_thread_metadata_name_replaces_blank_title() {
        let mut runtime = CodexAppRuntime::empty();
        runtime
            .apply_thread_metadata(
                CodexAppThreadMetadata {
                    id: "thread-1".to_string(),
                    cwd: Some("/tmp/builder-panel".to_string()),
                    name: None,
                    preview: None,
                    path: None,
                    status_type: "active".to_string(),
                    ephemeral: false,
                },
                UnixMillis::new(1),
            )
            .expect("blank metadata should apply");
        runtime
            .apply_thread_metadata(
                CodexAppThreadMetadata {
                    id: "thread-1".to_string(),
                    cwd: Some("/tmp/builder-panel".to_string()),
                    name: Some("真实显示名称".to_string()),
                    preview: None,
                    path: None,
                    status_type: "active".to_string(),
                    ephemeral: false,
                },
                UnixMillis::new(2),
            )
            .expect("real metadata should apply");

        let session = runtime
            .session_state()
            .sessions
            .values()
            .next()
            .expect("session should exist");

        assert_eq!(session.title.as_deref(), Some("真实显示名称"));
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
    fn realtime_blank_cwd_initializes_unresolved_session_without_jump() {
        let mut runtime = CodexAppRuntime::empty();
        let request = json!({
            "id": 7,
            "method": "item/tool/requestUserInput",
            "params": {
                "threadId": "thread-1",
                "cwd": "   ",
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
            .get(&session_key(UNRESOLVED_CODEX_APP_PROJECT_ID, "thread-1"))
            .expect("unresolved session should exist");

        assert_eq!(session.project_label, UNRESOLVED_CODEX_APP_PROJECT_LABEL);
        assert!(!session.capabilities.can_jump);
        assert!(session.jump_target.is_none());
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
        assert!(!list_item.actions.contains(&UiAction::Jump));
        assert!(list_item.actions.contains(&UiAction::ViewProcessTimeline));
        assert_eq!(list_item.project_label, UNRESOLVED_CODEX_APP_PROJECT_LABEL);
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
        let session_key = session_key(UNRESOLVED_CODEX_APP_PROJECT_ID, "thread-1");
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
            .apply_app_server_message(&request, "/fallback/cwd", UnixMillis::new(1))
            .expect("request should apply");
        let fallback_key = session_key(UNRESOLVED_CODEX_APP_PROJECT_ID, "thread-1");
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
    fn thread_metadata_migrates_unresolved_session_without_overwriting_pending() {
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
            .apply_app_server_message(&request, "/wrong/cwd", UnixMillis::new(1))
            .expect("request should apply");
        let unresolved_key = session_key(UNRESOLVED_CODEX_APP_PROJECT_ID, "thread-1");
        assert!(runtime
            .session_state()
            .sessions
            .contains_key(&unresolved_key));

        runtime
            .apply_thread_metadata(
                CodexAppThreadMetadata {
                    id: "thread-1".to_string(),
                    cwd: Some("/tmp/builder-panel".to_string()),
                    name: Some("Builder Panel".to_string()),
                    preview: Some("历史预览".to_string()),
                    path: None,
                    status_type: "idle".to_string(),
                    ephemeral: false,
                },
                UnixMillis::new(2),
            )
            .expect("metadata should apply");
        let real_key = session_key("/tmp/builder-panel", "thread-1");
        let session = runtime
            .session_state()
            .sessions
            .get(&real_key)
            .expect("real session should exist");

        assert!(!runtime
            .session_state()
            .sessions
            .contains_key(&unresolved_key));
        assert_eq!(session.status, SessionStatus::WaitingForAnswer);
        assert!(session.pending_interaction.is_some());
        assert!(session.capabilities.can_jump);
        assert_eq!(
            session
                .jump_target
                .as_ref()
                .map(|target| target.location.as_str()),
            Some("codex://threads/thread-1")
        );
    }

    #[test]
    fn thread_metadata_migration_publishes_session_update_without_title_or_summary() {
        let sink = Arc::new(RecordingSessionUpdateSink::default());
        let mut runtime = CodexAppRuntime::with_update_sink(sink.clone());
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
            .apply_app_server_message(&request, "/wrong/cwd", UnixMillis::new(1))
            .expect("request should apply");
        let notification_count = sink.notifications().len();

        runtime
            .apply_thread_metadata(
                CodexAppThreadMetadata {
                    id: "thread-1".to_string(),
                    cwd: Some("/tmp/builder-panel".to_string()),
                    name: None,
                    preview: None,
                    path: None,
                    status_type: "active".to_string(),
                    ephemeral: false,
                },
                UnixMillis::new(2),
            )
            .expect("metadata should apply");

        let real_key = session_key("/tmp/builder-panel", "thread-1");
        let notifications = sink.notifications();

        assert!(runtime.session_state().sessions.contains_key(&real_key));
        assert!(notifications.len() > notification_count);
        assert!(notifications.iter().any(|notification| {
            notification.session_key == real_key
                && notification.changed_area == SessionUpdateArea::Session
                && notification.updated_at == UnixMillis::new(2)
        }));
    }

    #[test]
    fn rollout_snapshot_migration_publishes_session_update() {
        let sink = Arc::new(RecordingSessionUpdateSink::default());
        let mut runtime = CodexAppRuntime::with_update_sink(sink.clone());
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
            .apply_app_server_message(&request, "/wrong/cwd", UnixMillis::new(1))
            .expect("request should apply");
        let notification_count = sink.notifications().len();

        runtime
            .apply_rollout_snapshot(CodexRolloutSnapshot {
                session_id: "thread-1".to_string(),
                cwd: "/tmp/builder-panel".to_string(),
                summary: Some("最新输出".to_string()),
                last_agent_message: Some("最新输出".to_string()),
                path: PathBuf::from("/tmp/rollout-thread-1.jsonl"),
                updated_at: UnixMillis::new(2),
            })
            .expect("snapshot should apply");

        let real_key = session_key("/tmp/builder-panel", "thread-1");
        let notifications = sink.notifications();
        let session = runtime
            .session_state()
            .sessions
            .get(&real_key)
            .expect("real session should exist");

        assert_eq!(session.summary.as_deref(), Some("最新输出"));
        assert!(notifications.len() > notification_count);
        assert!(notifications.iter().any(|notification| {
            notification.session_key == real_key
                && notification.changed_area == SessionUpdateArea::Session
                && notification.updated_at == UnixMillis::new(2)
        }));
    }

    #[test]
    fn loaded_thread_metadata_can_create_current_session() {
        let mut runtime = CodexAppRuntime::empty();

        runtime
            .apply_thread_metadata(
                CodexAppThreadMetadata {
                    id: "thread-1".to_string(),
                    cwd: Some("/tmp/builder-panel".to_string()),
                    name: Some("Thread 1".to_string()),
                    preview: Some("最新输出".to_string()),
                    path: None,
                    status_type: "idle".to_string(),
                    ephemeral: false,
                },
                UnixMillis::new(1),
            )
            .expect("loaded thread should apply");

        let key = session_key("/tmp/builder-panel", "thread-1");
        let session = runtime
            .session_state()
            .sessions
            .get(&key)
            .expect("loaded thread should create current session");

        assert_eq!(session.status, SessionStatus::Completed);
        assert_eq!(session.project_label, "builder-panel");
        assert_eq!(session.summary.as_deref(), Some("最新输出"));
        assert_eq!(
            session
                .jump_target
                .as_ref()
                .map(|target| target.location.as_str()),
            Some("codex://threads/thread-1")
        );
    }

    #[test]
    fn existing_running_session_ignores_idle_thread_metadata_status() {
        let mut runtime = CodexAppRuntime::empty();

        runtime
            .apply_thread_metadata(
                CodexAppThreadMetadata {
                    id: "thread-1".to_string(),
                    cwd: Some("/tmp/builder-panel".to_string()),
                    name: Some("Thread 1".to_string()),
                    preview: Some("运行中".to_string()),
                    path: None,
                    status_type: "active".to_string(),
                    ephemeral: false,
                },
                UnixMillis::new(1),
            )
            .expect("active thread should apply");
        runtime
            .apply_thread_metadata(
                CodexAppThreadMetadata {
                    id: "thread-1".to_string(),
                    cwd: Some("/tmp/builder-panel".to_string()),
                    name: Some("Thread 1".to_string()),
                    preview: Some("最终输出".to_string()),
                    path: None,
                    status_type: "idle".to_string(),
                    ephemeral: false,
                },
                UnixMillis::new(2),
            )
            .expect("idle thread should apply");

        let session = runtime
            .session_state()
            .sessions
            .get(&session_key("/tmp/builder-panel", "thread-1"))
            .expect("session should exist");

        assert_eq!(session.status, SessionStatus::Running);
        assert_eq!(session.summary.as_deref(), Some("运行中"));
    }

    #[test]
    fn orphan_rollout_snapshot_does_not_create_session() {
        let mut runtime = CodexAppRuntime::empty();

        runtime
            .apply_rollout_snapshot(CodexRolloutSnapshot {
                session_id: "history-thread".to_string(),
                cwd: "/tmp/history".to_string(),
                summary: Some("历史输出".to_string()),
                last_agent_message: Some("历史输出".to_string()),
                path: PathBuf::from("/tmp/rollout-history.jsonl"),
                updated_at: UnixMillis::new(1),
            })
            .expect("rollout should be ignored without candidate");

        assert!(runtime.session_state().sessions.is_empty());
    }

    #[test]
    fn rollout_snapshot_migrates_unresolved_session() {
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
            .apply_app_server_message(&request, "/wrong/cwd", UnixMillis::new(1))
            .expect("request should apply");

        runtime
            .apply_rollout_snapshot(CodexRolloutSnapshot {
                session_id: "thread-1".to_string(),
                cwd: "/tmp/builder-panel".to_string(),
                summary: Some("历史输出".to_string()),
                last_agent_message: Some("历史输出".to_string()),
                path: PathBuf::from("/tmp/rollout-thread-1.jsonl"),
                updated_at: UnixMillis::new(2),
            })
            .expect("rollout should migrate unresolved session");

        let unresolved_key = session_key(UNRESOLVED_CODEX_APP_PROJECT_ID, "thread-1");
        let real_key = session_key("/tmp/builder-panel", "thread-1");
        let session = runtime
            .session_state()
            .sessions
            .get(&real_key)
            .expect("real session should exist");

        assert!(!runtime
            .session_state()
            .sessions
            .contains_key(&unresolved_key));
        assert_eq!(session.status, SessionStatus::WaitingForAnswer);
        assert!(session.capabilities.can_jump);
        assert_eq!(
            runtime.rollout_watch_targets(),
            vec![CodexRolloutWatchTarget {
                session_key: real_key,
                path: PathBuf::from("/tmp/rollout-thread-1.jsonl"),
            }]
        );
    }

    #[test]
    fn agent_message_delta_is_kept_after_turn_completion() {
        let mut runtime = CodexAppRuntime::empty();
        runtime
            .apply_app_server_message(
                &json!({
                    "method": "turn/started",
                    "params": {
                        "threadId": "thread-1",
                        "cwd": "/tmp/builder-panel",
                        "turn": {"id": "turn-1", "status": "inProgress"}
                    }
                }),
                "/wrong/cwd",
                UnixMillis::new(1),
            )
            .expect("turn should start");
        runtime
            .apply_app_server_message(
                &json!({
                    "method": "item/agentMessage/delta",
                    "params": {
                        "threadId": "thread-1",
                        "cwd": "/tmp/builder-panel",
                        "delta": "第一段"
                    }
                }),
                "/wrong/cwd",
                UnixMillis::new(2),
            )
            .expect("delta should apply");
        runtime
            .apply_app_server_message(
                &json!({
                    "method": "item/agentMessage/delta",
                    "params": {
                        "threadId": "thread-1",
                        "cwd": "/tmp/builder-panel",
                        "delta": "，第二段"
                    }
                }),
                "/wrong/cwd",
                UnixMillis::new(3),
            )
            .expect("delta should apply");
        runtime
            .apply_app_server_message(
                &json!({
                    "method": "turn/completed",
                    "params": {
                        "threadId": "thread-1",
                        "cwd": "/tmp/builder-panel",
                        "turn": {"id": "turn-1", "status": "completed"}
                    }
                }),
                "/wrong/cwd",
                UnixMillis::new(4),
            )
            .expect("turn should complete");

        let session = runtime
            .session_state()
            .sessions
            .get(&session_key("/tmp/builder-panel", "thread-1"))
            .expect("session should exist");

        assert_eq!(session.status, SessionStatus::Completed);
        assert_eq!(session.summary.as_deref(), Some("第一段，第二段"));
    }

    #[test]
    fn agent_message_delta_cache_is_bounded() {
        let mut runtime = CodexAppRuntime::empty();
        runtime
            .apply_app_server_message(
                &json!({
                    "method": "turn/started",
                    "params": {
                        "threadId": "thread-1",
                        "cwd": "/tmp/builder-panel",
                        "turn": {"id": "turn-1", "status": "inProgress"}
                    }
                }),
                "/wrong/cwd",
                UnixMillis::new(1),
            )
            .expect("turn should start");
        runtime
            .apply_app_server_message(
                &json!({
                    "method": "item/agentMessage/delta",
                    "params": {
                        "threadId": "thread-1",
                        "cwd": "/tmp/builder-panel",
                        "delta": "甲".repeat(MAX_CURRENT_TURN_OUTPUT_CHARS + 100)
                    }
                }),
                "/wrong/cwd",
                UnixMillis::new(2),
            )
            .expect("delta should apply");

        let output = runtime
            .current_turn_agent_outputs
            .get("thread-1")
            .expect("output should exist");

        assert_eq!(output.chars().count(), MAX_CURRENT_TURN_OUTPUT_CHARS);
    }

    #[test]
    fn turn_completion_final_output_respects_strict_limit() {
        let mut runtime = CodexAppRuntime::empty();
        runtime
            .apply_app_server_message(
                &json!({
                    "method": "item/agentMessage/delta",
                    "params": {
                        "threadId": "thread-1",
                        "cwd": "/tmp/builder-panel",
                        "delta": "甲".repeat(MAX_FINAL_OUTPUT_CHARS + 1)
                    }
                }),
                "/wrong/cwd",
                UnixMillis::new(1),
            )
            .expect("delta should apply");
        runtime
            .apply_app_server_message(
                &json!({
                    "method": "turn/completed",
                    "params": {
                        "threadId": "thread-1",
                        "cwd": "/tmp/builder-panel",
                        "turn": {"id": "turn-1", "status": "completed"}
                    }
                }),
                "/wrong/cwd",
                UnixMillis::new(2),
            )
            .expect("turn should complete");

        let session = runtime
            .session_state()
            .sessions
            .get(&session_key("/tmp/builder-panel", "thread-1"))
            .expect("session should exist");
        let summary = session.summary.as_ref().expect("summary should exist");
        assert_eq!(summary.chars().count(), MAX_FINAL_OUTPUT_CHARS);
    }

    #[test]
    fn turn_started_clears_previous_agent_output() {
        let mut runtime = CodexAppRuntime::empty();
        for (turn_id, delta, time) in [("turn-1", "上一轮输出", 1), ("turn-2", "当前输出", 3)]
        {
            runtime
                .apply_app_server_message(
                    &json!({
                        "method": "turn/started",
                        "params": {
                            "threadId": "thread-1",
                            "cwd": "/tmp/builder-panel",
                            "turn": {"id": turn_id, "status": "inProgress"}
                        }
                    }),
                    "/wrong/cwd",
                    UnixMillis::new(time),
                )
                .expect("turn should start");
            runtime
                .apply_app_server_message(
                    &json!({
                        "method": "item/agentMessage/delta",
                        "params": {
                            "threadId": "thread-1",
                            "cwd": "/tmp/builder-panel",
                            "delta": delta
                        }
                    }),
                    "/wrong/cwd",
                    UnixMillis::new(time + 1),
                )
                .expect("delta should apply");
        }
        runtime
            .apply_app_server_message(
                &json!({
                    "method": "turn/completed",
                    "params": {
                        "threadId": "thread-1",
                        "cwd": "/tmp/builder-panel",
                        "turn": {"id": "turn-2", "status": "completed"}
                    }
                }),
                "/wrong/cwd",
                UnixMillis::new(5),
            )
            .expect("turn should complete");

        let session = runtime
            .session_state()
            .sessions
            .get(&session_key("/tmp/builder-panel", "thread-1"))
            .expect("session should exist");

        assert_eq!(session.summary.as_deref(), Some("当前输出"));
    }

    #[test]
    fn followup_success_clears_previous_agent_output() {
        let mut runtime = CodexAppRuntime::empty();
        runtime
            .apply_app_server_message(
                &json!({
                    "method": "turn/started",
                    "params": {
                        "threadId": "thread-1",
                        "cwd": "/tmp/builder-panel",
                        "turn": {"id": "turn-1", "status": "inProgress"}
                    }
                }),
                "/wrong/cwd",
                UnixMillis::new(1),
            )
            .expect("turn should start");
        runtime
            .apply_app_server_message(
                &json!({
                    "method": "item/agentMessage/delta",
                    "params": {
                        "threadId": "thread-1",
                        "cwd": "/tmp/builder-panel",
                        "delta": "上一轮输出"
                    }
                }),
                "/wrong/cwd",
                UnixMillis::new(2),
            )
            .expect("delta should apply");
        runtime
            .apply_app_server_message(
                &json!({
                    "method": "turn/completed",
                    "params": {
                        "threadId": "thread-1",
                        "cwd": "/tmp/builder-panel",
                        "turn": {"id": "turn-1", "status": "completed"}
                    }
                }),
                "/wrong/cwd",
                UnixMillis::new(3),
            )
            .expect("turn should complete");
        let key = session_key("/tmp/builder-panel", "thread-1");

        runtime
            .create_followup_turn(&key, "继续")
            .expect("followup should create");
        runtime
            .complete_followup_turn(&key)
            .expect("followup should complete");
        runtime
            .apply_app_server_message(
                &json!({
                    "method": "thread/status/changed",
                    "params": {
                        "threadId": "thread-1",
                        "cwd": "/tmp/builder-panel",
                        "status": {"type": "idle"}
                    }
                }),
                "/wrong/cwd",
                UnixMillis::new(5),
            )
            .expect("idle should apply");

        let session = runtime
            .session_state()
            .sessions
            .get(&key)
            .expect("session should exist");

        assert_eq!(session.status, SessionStatus::Completed);
        assert_eq!(session.summary.as_deref(), Some("继续"));
    }

    #[test]
    fn idle_status_marks_session_ready_for_followup() {
        let mut runtime = CodexAppRuntime::empty();
        let started = json!({
            "method": "thread/started",
            "params": {
                "thread": {
                    "id": "thread-1",
                    "cwd": "/tmp/builder-panel",
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
                    "cwd": "/tmp/builder-panel",
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
        let session_key = session_key(UNRESOLVED_CODEX_APP_PROJECT_ID, "thread-1");
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
                summary: Some("上一轮输出".to_string()),
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
        assert_eq!(session.summary, Some("上一轮输出".to_string()));
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
                summary: Some("上一轮输出".to_string()),
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
                "cwd": "/tmp/builder-panel",
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

    #[test]
    fn apply_hook_request_rejects_non_codex_app_payload() {
        let mut runtime = CodexAppRuntime::empty();
        let mut p = hook_payload(BridgeHookEventName::SessionStart);
        p.agent_kind = AgentKind::CodexCli;
        let request =
            BridgeRequestEnvelope::process_agent_hook("request-foreign".to_string(), p);

        let result = runtime.apply_hook_request(&request, UnixMillis::new(1));
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("non CodexApp hook payload should be rejected"),
        };
        assert!(error.user_message.contains("CodexApp"));
        assert!(runtime.session_state().sessions.is_empty());
    }

    #[test]
    fn apply_rollout_event_rejects_non_codex_app_session_event() {
        use crate::domain::agent_session::{
            ConversationId, ProjectId, SessionCapabilities, SessionKey,
        };

        let mut runtime = CodexAppRuntime::empty();
        let foreign = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("/tmp/cli"),
            ConversationId::new("cli-thread"),
        );
        let event = AgentEvent::SessionStarted(SessionStartedEvent {
            session_key: foreign,
            project_label: "cli".to_string(),
            conversation_label: "cli-thread".to_string(),
            title: None,
            summary: None,
            capabilities: SessionCapabilities {
                can_jump: false,
                can_send_reply: false,
                can_resolve_approval: false,
                can_create_followup_turn: false,
                can_view_process_timeline: false,
            },
            usage: UsageSnapshot::unavailable(),
            updated_at: UnixMillis::new(1),
        });

        let error = runtime
            .apply_rollout_event(event)
            .expect_err("non CodexApp rollout event should be rejected");
        assert!(error.user_message.contains("CodexApp"));
        assert!(runtime.session_state().sessions.is_empty());
    }

    #[test]
    fn claims_codex_app_thread_matches_by_session_id_then_cwd() {
        let mut runtime = CodexAppRuntime::empty();
        let hook = hook_payload(BridgeHookEventName::SessionStart);
        let request =
            BridgeRequestEnvelope::process_agent_hook("request-1".to_string(), hook.clone());
        runtime
            .apply_hook_request(&request, UnixMillis::new(1))
            .expect("hook should register");

        assert!(
            runtime.claims_codex_app_thread(&hook.session_id, &hook.cwd),
            "原始 session_id+cwd 应命中"
        );
        assert!(
            runtime.claims_codex_app_thread("unrelated-thread", &hook.cwd),
            "陌生 session_id + 已知 cwd 也应命中(cwd 兜底)"
        );
        assert!(
            !runtime.claims_codex_app_thread("unrelated-thread", "/tmp/other"),
            "陌生 session_id + 陌生 cwd 不应命中"
        );
    }

    #[test]
    fn orphan_eviction_callback_fires_on_thread_metadata_with_cwd() {
        let mut runtime = CodexAppRuntime::empty();
        let calls: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let calls_clone = Arc::clone(&calls);
        runtime.set_orphan_eviction_callback(Arc::new(move |cwd, thread_id, _| {
            calls_clone
                .lock()
                .expect("lock")
                .push((cwd.to_string(), thread_id.to_string()));
        }));

        runtime
            .apply_thread_metadata(
                CodexAppThreadMetadata {
                    id: "thread-app".to_string(),
                    cwd: Some("/tmp/builder-panel".to_string()),
                    name: Some("codex app".to_string()),
                    preview: None,
                    path: None,
                    status_type: "active".to_string(),
                    ephemeral: false,
                },
                UnixMillis::new(1),
            )
            .expect("metadata should apply");

        let calls = calls.lock().expect("lock");
        assert_eq!(calls.len(), 1, "callback 应触发一次");
        assert_eq!(calls[0].0, "/tmp/builder-panel");
        assert_eq!(calls[0].1, "thread-app");
    }

    #[test]
    fn orphan_eviction_callback_fires_on_hook_request() {
        let mut runtime = CodexAppRuntime::empty();
        let calls: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let calls_clone = Arc::clone(&calls);
        runtime.set_orphan_eviction_callback(Arc::new(move |_, _, _| {
            *calls_clone.lock().expect("lock") += 1;
        }));

        let hook = hook_payload(BridgeHookEventName::SessionStart);
        let request = BridgeRequestEnvelope::process_agent_hook("request-1".to_string(), hook);
        runtime
            .apply_hook_request(&request, UnixMillis::new(1))
            .expect("hook should apply");

        assert_eq!(*calls.lock().expect("lock"), 1);
    }
}
