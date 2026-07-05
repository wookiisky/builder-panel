//! Codex rollout JSONL 发现和摘要清洗。

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

use serde_json::Value;

use super::internal_prompt::is_codex_internal_prompt;
use crate::domain::agent_event::{
    ActivityUpdatedEvent, AgentEvent, AnswerRequestedEvent, InteractionCompletedEvent,
    TurnCompletedEvent, UserMessageUpdatedEvent,
};
use crate::domain::agent_interaction::{
    AnswerInteraction, ExternalReplyTarget, InteractionId, InteractionStatus, ReplyTarget,
    TextReplyInteraction,
};
use crate::domain::agent_session::SessionKey;
use crate::domain::usage::UnixMillis;

const DEFAULT_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const DEFAULT_MAX_FILES: usize = 40;
const DEFAULT_MAX_VISITED_ENTRIES: usize = 5_000;
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_LINE_BYTES: usize = 2 * 1024 * 1024;
const MAX_TAIL_READ_BYTES: usize = 256 * 1024;
const MAX_FINAL_OUTPUT_CHARS: usize = 65_535;

/// Codex rollout 清洗后的会话快照。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexRolloutSnapshot {
    /// Codex thread/session ID。
    pub session_id: String,
    /// rollout 中的真实 cwd。
    pub cwd: String,
    /// 最近可展示摘要。
    pub summary: Option<String>,
    /// 最近 Agent 输出。
    pub last_agent_message: Option<String>,
    /// rollout 文件路径。
    pub path: PathBuf,
    /// 最近更新时间。
    pub updated_at: UnixMillis,
    /// 当前 rollout 是否已经完成或中止。
    pub completed: bool,
    /// 当前仍未处理的 request_user_input。
    pub pending_user_input: Option<CodexRolloutPendingUserInput>,
}

/// Codex rollout 中恢复出的等待用户输入请求。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexRolloutPendingUserInput {
    /// Codex turn ID。
    pub turn_id: String,
    /// function_call call_id。
    pub call_id: String,
    /// 等待回答的问题列表。
    pub questions: Vec<CodexRolloutPendingQuestion>,
    /// 自动消解等待时间。
    pub auto_resolution_ms: Option<u64>,
}

/// Codex rollout 中的单个等待输入问题。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexRolloutPendingQuestion {
    /// 问题稳定 ID。
    pub id: String,
    /// 可选短标题。
    pub header: Option<String>,
    /// 问题正文。
    pub question: String,
}

impl CodexRolloutPendingUserInput {
    /// 转换为只读回答请求事件。
    pub fn answer_requested_event(
        &self,
        session_key: &SessionKey,
        updated_at: UnixMillis,
    ) -> AgentEvent {
        AgentEvent::AnswerRequested(AnswerRequestedEvent {
            session_key: session_key.clone(),
            interaction: self.answer_interaction(session_key, updated_at),
            updated_at,
        })
    }

    /// 转换为只读回答交互。
    fn answer_interaction(
        &self,
        session_key: &SessionKey,
        updated_at: UnixMillis,
    ) -> AnswerInteraction {
        let summary = self.request_summary();
        let reply_target = ReplyTarget::ExternalOnly(ExternalReplyTarget {
            handler_label: "Codex App".to_string(),
            reason: "Codex App 私有等待输入只能在原线程处理".to_string(),
        });

        AnswerInteraction::TextReply(TextReplyInteraction {
            interaction_id: self.interaction_id(),
            session_key: session_key.clone(),
            created_at: updated_at,
            expires_at: None,
            reply_target,
            status: InteractionStatus::Pending,
            request_summary: summary.clone(),
            prompt: summary,
        })
    }

    /// 创建稳定交互 ID。
    fn interaction_id(&self) -> InteractionId {
        InteractionId::new(format!("rollout:{}:{}", self.turn_id, self.call_id))
    }

    /// 创建等待输入摘要。
    pub fn request_summary(&self) -> String {
        let first_question = self
            .questions
            .first()
            .map(|question| question.question.as_str())
            .unwrap_or("Codex App 等待输入");
        if self.questions.len() <= 1 {
            return truncate(first_question, 240);
        }

        truncate(
            &format!(
                "{}；另有 {} 个问题需在 Codex App 中处理",
                first_question,
                self.questions.len().saturating_sub(1)
            ),
            240,
        )
    }
}

/// Codex rollout 实时 tail 目标。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexRolloutWatchTarget {
    /// 所属 session。
    pub session_key: SessionKey,
    /// rollout 文件路径。
    pub path: PathBuf,
}

/// Codex rollout 追加行 tailer。
#[derive(Debug)]
pub struct CodexRolloutTailer {
    /// rollout 根目录。
    root: PathBuf,
    /// 已跟踪路径。
    tracked: BTreeMap<PathBuf, CodexRolloutTailState>,
}

/// 单个 rollout tail 状态。
#[derive(Debug)]
struct CodexRolloutTailState {
    /// 所属 session。
    session_key: SessionKey,
    /// 已读取偏移。
    offset: u64,
    /// 当前文件身份。
    file_identity: CodexRolloutFileIdentity,
    /// 尚未遇到换行的半行缓存。
    partial_line: Vec<u8>,
    /// 当前是否正在丢弃超长行剩余内容。
    dropping_overlong_line: bool,
    /// 当前 turn 是否已完成。
    completed: bool,
    /// 当前追加流是否处在 Codex 内部隐藏 turn 中。
    current_turn_is_internal: bool,
    /// 当前等待输入 function call ID。
    pending_user_input_call_id: Option<String>,
}

/// rollout 文件身份。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CodexRolloutFileIdentity {
    /// Unix 文件身份。
    #[cfg(unix)]
    Unix { dev: u64, ino: u64 },
    /// Windows 文件身份。
    #[cfg(windows)]
    Windows {
        volume_serial_number: Option<u32>,
        file_index: Option<u64>,
        creation_time: u64,
    },
    /// 其它平台的保守身份。
    #[cfg(not(any(unix, windows)))]
    Portable { created_millis: Option<u128> },
}

impl CodexRolloutTailer {
    /// 使用默认 `~/.codex/sessions` 创建 tailer。
    pub fn default_root() -> Self {
        Self::new(default_rollout_root())
    }

    /// 创建 tailer。
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            tracked: BTreeMap::new(),
        }
    }

    /// 同步 watch 目标；新目标从当前 EOF 后开始，不回放历史。
    pub fn sync_targets(&mut self, targets: Vec<CodexRolloutWatchTarget>) {
        let mut next = BTreeMap::new();
        for target in targets {
            let Some(path) = valid_rollout_path(&self.root, &target.path) else {
                continue;
            };
            if let Some(mut state) = self.tracked.remove(&path) {
                state.session_key = target.session_key;
                next.insert(path, state);
                continue;
            }
            next.insert(path.clone(), tail_state_at_eof(&path, target.session_key));
        }

        self.tracked = next;
    }

    /// 读取所有目标的新增完整行并转换为归一事件。
    pub fn poll_events(&mut self, updated_at: UnixMillis) -> Vec<AgentEvent> {
        let paths = self.tracked.keys().cloned().collect::<Vec<_>>();
        let mut events = Vec::new();
        for path in paths {
            let Some(mut state) = self.tracked.remove(&path) else {
                continue;
            };
            events.extend(poll_path_events(&path, &mut state, updated_at));
            self.tracked.insert(path, state);
        }

        events
    }

    /// 返回当前跟踪路径数量。
    #[cfg(test)]
    pub fn tracked_count(&self) -> usize {
        self.tracked.len()
    }
}

/// Codex rollout 发现器。
#[derive(Clone, Debug)]
pub struct CodexRolloutDiscovery {
    /// rollout 根目录。
    root: PathBuf,
    /// 最大文件年龄。
    max_age: Duration,
    /// 最大读取文件数。
    max_files: usize,
}

impl CodexRolloutDiscovery {
    /// 使用默认 `~/.codex/sessions` 创建发现器。
    pub fn default_root() -> Self {
        Self::new(default_rollout_root(), DEFAULT_MAX_AGE, DEFAULT_MAX_FILES)
    }

    /// 创建发现器。
    pub fn new(root: PathBuf, max_age: Duration, max_files: usize) -> Self {
        Self {
            root,
            max_age,
            max_files,
        }
    }

    /// 读取指定 rollout 文件。
    pub fn read_path(&self, path: &Path) -> Option<CodexRolloutSnapshot> {
        let path = self.valid_rollout_path(path)?;
        read_rollout_path(&path)
    }

    /// 发现最近 rollout 快照。
    pub fn discover_recent(&self, now: SystemTime) -> Vec<CodexRolloutSnapshot> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };

        let cutoff = now.checked_sub(self.max_age).unwrap_or(UNIX_EPOCH);
        let mut candidates = Vec::new();
        let mut remaining_entries = DEFAULT_MAX_VISITED_ENTRIES;
        collect_rollout_candidates(entries, cutoff, &mut candidates, &mut remaining_entries);
        candidates.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| right.0.cmp(&left.0)));

        candidates
            .into_iter()
            .take(self.max_files)
            .filter_map(|(path, _)| self.read_path(&path))
            .collect()
    }

    /// 校验 rollout path 属于当前 root 且满足文件边界。
    fn valid_rollout_path(&self, path: &Path) -> Option<PathBuf> {
        valid_rollout_path(&self.root, path)
    }
}

fn default_rollout_root() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".codex")
        .join("sessions")
}

fn valid_rollout_path(root: &Path, path: &Path) -> Option<PathBuf> {
    if !is_rollout_file(path) {
        return None;
    }
    let root = root.canonicalize().ok()?;
    let path = path.canonicalize().ok()?;
    if !path.starts_with(root) {
        return None;
    }
    let metadata = fs::metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        return None;
    }

    Some(path)
}

/// 从文件读取 rollout 快照。
fn read_rollout_path(path: &Path) -> Option<CodexRolloutSnapshot> {
    let file = File::open(path).ok()?;
    let modified_at = file
        .metadata()
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(system_time_to_unix_millis)
        .unwrap_or_else(|| UnixMillis::new(0));
    let mut state = CodexRolloutState::new(path.to_path_buf(), modified_at);
    let mut reader = BufReader::new(file);

    for_each_bounded_line(&mut reader, |line| {
        apply_rollout_line(&line, &mut state);
    });

    state.into_snapshot()
}

fn tail_state_at_eof(path: &Path, session_key: SessionKey) -> CodexRolloutTailState {
    let metadata = fs::metadata(path).ok();
    let offset = metadata
        .as_ref()
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let file_identity = metadata
        .as_ref()
        .map(rollout_file_identity)
        .unwrap_or_else(empty_rollout_file_identity);
    let scan_state = scan_rollout_tail_state(path);
    CodexRolloutTailState {
        session_key,
        offset,
        file_identity,
        partial_line: Vec::new(),
        dropping_overlong_line: false,
        completed: false,
        current_turn_is_internal: scan_state.current_turn_is_internal,
        pending_user_input_call_id: scan_state.pending_user_input_call_id,
    }
}

fn scan_rollout_tail_state(path: &Path) -> CodexRolloutScanState {
    let Ok(file) = File::open(path) else {
        return CodexRolloutScanState::default();
    };
    let mut state = CodexRolloutScanState::default();
    let mut reader = BufReader::new(file);

    for_each_bounded_line(&mut reader, |line| {
        apply_scan_line(&line, &mut state);
    });

    state
}

fn poll_path_events(
    path: &Path,
    state: &mut CodexRolloutTailState,
    updated_at: UnixMillis,
) -> Vec<AgentEvent> {
    let Ok(metadata) = fs::metadata(path) else {
        return Vec::new();
    };
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        return Vec::new();
    }
    let file_identity = rollout_file_identity(&metadata);
    if file_identity != state.file_identity {
        reset_tail_state_at_eof(path, state, metadata.len(), file_identity);
        return Vec::new();
    }
    if metadata.len() < state.offset {
        reset_tail_state_at_eof(path, state, metadata.len(), file_identity);
        return Vec::new();
    }
    if metadata.len() == state.offset {
        return Vec::new();
    }

    let Ok(mut file) = File::open(path) else {
        return Vec::new();
    };
    if file.seek(SeekFrom::Start(state.offset)).is_err() {
        return Vec::new();
    }
    let bytes_to_read = (metadata.len() - state.offset).min(MAX_TAIL_READ_BYTES as u64) as usize;
    let mut bytes = vec![0; bytes_to_read];
    let Ok(read_count) = file.read(&mut bytes) else {
        return Vec::new();
    };
    bytes.truncate(read_count);
    state.offset = state.offset.saturating_add(read_count as u64);

    let lines = append_tail_bytes(
        &mut state.partial_line,
        &mut state.dropping_overlong_line,
        &bytes,
    );
    let mut events = Vec::new();
    for line in lines {
        let event_updated_at = UnixMillis::new(
            updated_at
                .value
                .saturating_add(u64::try_from(events.len()).unwrap_or(u64::MAX)),
        );
        events.extend(live_events_from_rollout_line(
            &line,
            state,
            event_updated_at,
        ));
    }

    events
}

fn reset_tail_state_at_eof(
    path: &Path,
    state: &mut CodexRolloutTailState,
    offset: u64,
    file_identity: CodexRolloutFileIdentity,
) {
    state.offset = offset;
    state.file_identity = file_identity;
    state.partial_line.clear();
    state.dropping_overlong_line = false;
    state.completed = false;
    let scan_state = scan_rollout_tail_state(path);
    state.current_turn_is_internal = scan_state.current_turn_is_internal;
    state.pending_user_input_call_id = scan_state.pending_user_input_call_id;
}

#[cfg(unix)]
fn rollout_file_identity(metadata: &fs::Metadata) -> CodexRolloutFileIdentity {
    CodexRolloutFileIdentity::Unix {
        dev: metadata.dev(),
        ino: metadata.ino(),
    }
}

#[cfg(windows)]
fn rollout_file_identity(metadata: &fs::Metadata) -> CodexRolloutFileIdentity {
    CodexRolloutFileIdentity::Windows {
        volume_serial_number: metadata.volume_serial_number(),
        file_index: metadata.file_index(),
        creation_time: metadata.creation_time(),
    }
}

#[cfg(not(any(unix, windows)))]
fn rollout_file_identity(metadata: &fs::Metadata) -> CodexRolloutFileIdentity {
    let created_millis = metadata
        .created()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis());
    CodexRolloutFileIdentity::Portable { created_millis }
}

#[cfg(unix)]
fn empty_rollout_file_identity() -> CodexRolloutFileIdentity {
    CodexRolloutFileIdentity::Unix { dev: 0, ino: 0 }
}

#[cfg(windows)]
fn empty_rollout_file_identity() -> CodexRolloutFileIdentity {
    CodexRolloutFileIdentity::Windows {
        volume_serial_number: None,
        file_index: None,
        creation_time: 0,
    }
}

#[cfg(not(any(unix, windows)))]
fn empty_rollout_file_identity() -> CodexRolloutFileIdentity {
    CodexRolloutFileIdentity::Portable {
        created_millis: None,
    }
}

fn append_tail_bytes(
    partial_line: &mut Vec<u8>,
    dropping_overlong_line: &mut bool,
    bytes: &[u8],
) -> Vec<String> {
    let mut lines = Vec::new();
    for byte in bytes {
        if *dropping_overlong_line {
            if *byte == b'\n' {
                *dropping_overlong_line = false;
                partial_line.clear();
            }
            continue;
        }

        partial_line.push(*byte);
        if partial_line.len() > MAX_LINE_BYTES {
            partial_line.clear();
            *dropping_overlong_line = *byte != b'\n';
            continue;
        }

        if *byte != b'\n' {
            continue;
        }

        let mut line = std::mem::take(partial_line);
        while line
            .last()
            .is_some_and(|byte| *byte == b'\n' || *byte == b'\r')
        {
            line.pop();
        }
        if line.is_empty() {
            continue;
        }
        if let Ok(text) = String::from_utf8(line) {
            lines.push(text);
        }
    }

    lines
}

fn live_events_from_rollout_line(
    line: &str,
    state: &mut CodexRolloutTailState,
    updated_at: UnixMillis,
) -> Vec<AgentEvent> {
    let Some(value) = serde_json::from_str::<Value>(line).ok() else {
        return Vec::new();
    };
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    let Some(payload) = object.get("payload").and_then(Value::as_object) else {
        return Vec::new();
    };

    match object.get("type").and_then(Value::as_str) {
        Some("event_msg") => live_events_from_event_msg(payload, state, updated_at),
        Some("response_item") => live_events_from_response_item(payload, state, updated_at),
        _ => Vec::new(),
    }
}

fn live_events_from_event_msg(
    payload: &serde_json::Map<String, Value>,
    state: &mut CodexRolloutTailState,
    updated_at: UnixMillis,
) -> Vec<AgentEvent> {
    match payload.get("type").and_then(Value::as_str) {
        Some("turn_started") | Some("task_started") => {
            if !state.current_turn_is_internal {
                state.completed = false;
            }
            Vec::new()
        }
        Some("user_message") => {
            let Some(message) = clean_string(payload.get("message")) else {
                return Vec::new();
            };
            if is_codex_internal_prompt(&message) {
                state.current_turn_is_internal = true;
                return Vec::new();
            }
            state.current_turn_is_internal = false;
            state.completed = false;
            let mut events = clear_pending_user_input_events(state, None, updated_at);
            events.push(user_message_event(state, &message, updated_at));
            events
        }
        Some("agent_message") if !state.current_turn_is_internal => {
            clean_string(payload.get("message"))
                .map(|message| {
                    vec![activity_event(
                        state,
                        &truncate_strict(&message, MAX_FINAL_OUTPUT_CHARS),
                        updated_at,
                    )]
                })
                .unwrap_or_default()
        }
        Some("agent_message") => Vec::new(),
        Some("task_complete") | Some("turn_complete") => {
            if state.current_turn_is_internal {
                return Vec::new();
            }
            state.completed = true;
            let summary = clean_string(payload.get("last_agent_message"))
                .map(|message| truncate_strict(&message, MAX_FINAL_OUTPUT_CHARS));
            state.pending_user_input_call_id = None;
            vec![AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key: state.session_key.clone(),
                summary,
                updated_at,
            })]
        }
        Some("turn_aborted") => {
            if state.current_turn_is_internal {
                return Vec::new();
            }
            state.completed = true;
            state.pending_user_input_call_id = None;
            vec![AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key: state.session_key.clone(),
                summary: None,
                updated_at,
            })]
        }
        Some("exec_command_begin")
        | Some("terminal_interaction")
        | Some("patch_apply_begin")
        | Some("patch_apply_updated")
        | Some("mcp_tool_call_begin") => Vec::new(),
        Some("dynamic_tool_call_request") => Vec::new(),
        Some("web_search_begin") => Vec::new(),
        Some("web_search_end") => Vec::new(),
        Some("image_generation_begin") => Vec::new(),
        Some("image_generation_end") => Vec::new(),
        Some("view_image_tool_call") => Vec::new(),
        Some("plan_update") => Vec::new(),
        Some("exec_command_end")
        | Some("patch_apply_end")
        | Some("mcp_tool_call_end")
        | Some("dynamic_tool_call_response") => Vec::new(),
        _ => Vec::new(),
    }
}

fn live_events_from_response_item(
    payload: &serde_json::Map<String, Value>,
    state: &mut CodexRolloutTailState,
    updated_at: UnixMillis,
) -> Vec<AgentEvent> {
    if state.current_turn_is_internal {
        return Vec::new();
    }
    let item = payload
        .get("item")
        .and_then(Value::as_object)
        .unwrap_or(payload);
    match item.get("type").and_then(Value::as_str) {
        Some("message") if item.get("role").and_then(Value::as_str) == Some("assistant") => {
            response_message_text(item, "output_text")
                .map(|message| {
                    vec![activity_event(
                        state,
                        &truncate_strict(&message, MAX_FINAL_OUTPUT_CHARS),
                        updated_at,
                    )]
                })
                .unwrap_or_default()
        }
        Some("function_call") => {
            if let Some(pending) = pending_user_input_from_response_item(payload, item) {
                state.pending_user_input_call_id = Some(pending.call_id.clone());
                return vec![
                    activity_event(state, &pending.request_summary(), updated_at),
                    pending.answer_requested_event(&state.session_key, updated_at),
                ];
            }
            function_call_activity(item)
                .map(|summary| vec![activity_event(state, &summary, updated_at)])
                .unwrap_or_default()
        }
        Some("local_shell_call") => vec![activity_event(state, "执行命令…", updated_at)],
        Some("custom_tool_call") => Vec::new(),
        Some("tool_search_call") => vec![activity_event(state, "搜索工具中…", updated_at)],
        Some("web_search_call") => vec![activity_event(state, "联网检索中…", updated_at)],
        Some("image_generation_call") => vec![activity_event(state, "生成图像中…", updated_at)],
        Some("function_call_output") | Some("custom_tool_call_output") => {
            let call_id = clean_string(item.get("call_id"));
            clear_pending_user_input_events(state, call_id.as_deref(), updated_at)
        }
        _ => Vec::new(),
    }
}

fn clear_pending_user_input_events(
    state: &mut CodexRolloutTailState,
    call_id: Option<&str>,
    updated_at: UnixMillis,
) -> Vec<AgentEvent> {
    let Some(pending_call_id) = state.pending_user_input_call_id.as_deref() else {
        return Vec::new();
    };
    if call_id.is_some_and(|value| value != pending_call_id) {
        return Vec::new();
    }

    state.pending_user_input_call_id = None;
    vec![AgentEvent::InteractionCompleted(
        InteractionCompletedEvent {
            session_key: state.session_key.clone(),
            summary: None,
            updated_at,
        },
    )]
}

/// 从 function_call response_item 中提取 request_user_input。
fn pending_user_input_from_response_item(
    payload: &serde_json::Map<String, Value>,
    item: &serde_json::Map<String, Value>,
) -> Option<CodexRolloutPendingUserInput> {
    if item.get("name").and_then(Value::as_str) != Some("request_user_input") {
        return None;
    }
    let call_id = clean_string(item.get("call_id"))?;
    let arguments = item
        .get("arguments")
        .and_then(parse_function_call_arguments)?;
    let questions = pending_questions(arguments.get("questions"))?;
    let turn_id = rollout_turn_id(payload)
        .or_else(|| clean_string(item.get("id")))
        .unwrap_or_else(|| "unknown-turn".to_string());
    let auto_resolution_ms = arguments
        .get("autoResolutionMs")
        .or_else(|| arguments.get("auto_resolution_ms"))
        .and_then(Value::as_u64);

    Some(CodexRolloutPendingUserInput {
        turn_id,
        call_id,
        questions,
        auto_resolution_ms,
    })
}

/// 提取 rollout 内部 turn ID。
fn rollout_turn_id(payload: &serde_json::Map<String, Value>) -> Option<String> {
    payload
        .get("internal_chat_message_metadata_passthrough")
        .and_then(Value::as_object)
        .and_then(|metadata| clean_string(metadata.get("turn_id")))
}

/// 提取等待输入问题。
fn pending_questions(value: Option<&Value>) -> Option<Vec<CodexRolloutPendingQuestion>> {
    let questions = value?.as_array()?;
    let mut output = Vec::new();
    for (index, question) in questions.iter().enumerate() {
        let Some(question_object) = question.as_object() else {
            continue;
        };
        let question_text = clean_string(question_object.get("question"))
            .or_else(|| clean_string(question_object.get("label")))
            .unwrap_or_else(|| "Codex App 等待输入".to_string());
        let id = clean_string(question_object.get("id"))
            .unwrap_or_else(|| format!("question-{}", index.saturating_add(1)));
        output.push(CodexRolloutPendingQuestion {
            id,
            header: clean_string(question_object.get("header")),
            question: question_text,
        });
    }

    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

/// 只用于恢复 tailer 内部状态，不产生摘要或 UI 事件。
#[derive(Default)]
struct CodexRolloutScanState {
    /// 当前是否处在 Codex 内部隐藏 turn。
    current_turn_is_internal: bool,
    /// 当前未完成的 request_user_input call_id。
    pending_user_input_call_id: Option<String>,
}

fn apply_scan_line(line: &str, state: &mut CodexRolloutScanState) {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return;
    };
    let Some(object) = value.as_object() else {
        return;
    };
    let Some(payload) = object.get("payload").and_then(Value::as_object) else {
        return;
    };
    match object.get("type").and_then(Value::as_str) {
        Some("event_msg") => apply_scan_event_msg(payload, state),
        Some("response_item") => apply_scan_response_item(payload, state),
        _ => {}
    }
}

fn apply_scan_event_msg(
    payload: &serde_json::Map<String, Value>,
    state: &mut CodexRolloutScanState,
) {
    match payload.get("type").and_then(Value::as_str) {
        Some("user_message") => {
            let Some(message) = clean_string(payload.get("message")) else {
                return;
            };
            state.current_turn_is_internal = is_codex_internal_prompt(&message);
            if !state.current_turn_is_internal {
                state.pending_user_input_call_id = None;
            }
        }
        Some("task_complete") | Some("turn_complete") | Some("turn_aborted") => {
            if !state.current_turn_is_internal {
                state.pending_user_input_call_id = None;
            }
        }
        _ => {}
    }
}

fn apply_scan_response_item(
    payload: &serde_json::Map<String, Value>,
    state: &mut CodexRolloutScanState,
) {
    if state.current_turn_is_internal {
        return;
    }
    let item = payload
        .get("item")
        .and_then(Value::as_object)
        .unwrap_or(payload);
    match item.get("type").and_then(Value::as_str) {
        Some("function_call") => {
            if let Some(pending) = pending_user_input_from_response_item(payload, item) {
                state.pending_user_input_call_id = Some(pending.call_id);
            }
        }
        Some("function_call_output") | Some("custom_tool_call_output") => {
            let call_id = clean_string(item.get("call_id"));
            if state
                .pending_user_input_call_id
                .as_deref()
                .is_some_and(|pending_call_id| call_id.as_deref() == Some(pending_call_id))
            {
                state.pending_user_input_call_id = None;
            }
        }
        _ => {}
    }
}

/// 根据 response_item.item 中的 function_call 字段，构造一条"正在执行什么"的简短摘要，
/// 让长时间运行的工具调用（特别是 spawn_agent/wait_agent）期间，session 列表摘要不再卡住。
///
/// 出于安全考虑：未知工具名只暴露工具名本身，绝不读取 arguments，避免泄露未知工具中可能
/// 含有的密钥等敏感字段。仅对显式白名单内的内置工具才解析 arguments。
fn function_call_activity(item: &serde_json::Map<String, Value>) -> Option<String> {
    let name = clean_string(item.get("name"))?;
    let summary = match name.as_str() {
        "exec_command" | "shell" | "local_shell" => item
            .get("arguments")
            .and_then(parse_function_call_arguments)
            .as_ref()
            .and_then(|value| {
                clean_string(value.get("cmd")).or_else(|| clean_string(value.get("command")))
            })
            .map(|cmd| format!("执行: {}", cmd))
            .unwrap_or_else(|| "执行命令…".to_string()),
        "spawn_agent" => item
            .get("arguments")
            .and_then(parse_function_call_arguments)
            .as_ref()
            .and_then(|value| clean_string(value.get("agent_type")))
            .map(|kind| format!("调用子 Agent: {}", kind))
            .unwrap_or_else(|| "调用子 Agent…".to_string()),
        "wait_agent" => "等待子 Agent 返回…".to_string(),
        "close_agent" => "关闭子 Agent…".to_string(),
        _ => return None,
    };
    Some(truncate(&summary, 240))
}

/// arguments 在 rollout 里通常是 JSON 字符串；先解析成 Value 方便逐字段读。
fn parse_function_call_arguments(value: &Value) -> Option<Value> {
    if let Some(text) = value.as_str() {
        return serde_json::from_str(text).ok();
    }
    if value.is_object() {
        return Some(value.clone());
    }
    None
}

/// 有界逐行读取，避免异常长行在分配后才被丢弃。
fn for_each_bounded_line<R, F>(reader: &mut R, mut apply: F)
where
    R: BufRead,
    F: FnMut(String),
{
    let mut line = Vec::new();
    let mut over_limit = false;

    loop {
        let Ok(buffer) = reader.fill_buf() else {
            return;
        };
        if buffer.is_empty() {
            break;
        }

        let consumed = if let Some(index) = buffer.iter().position(|byte| *byte == b'\n') {
            let chunk = &buffer[..=index];
            if !over_limit && line.len() + chunk.len() <= MAX_LINE_BYTES {
                line.extend_from_slice(chunk);
            } else {
                over_limit = true;
            }
            index + 1
        } else {
            if !over_limit && line.len() + buffer.len() <= MAX_LINE_BYTES {
                line.extend_from_slice(buffer);
            } else {
                over_limit = true;
                line.clear();
            }
            buffer.len()
        };
        let reached_line_end = buffer.get(consumed.saturating_sub(1)) == Some(&b'\n');
        reader.consume(consumed);

        if reached_line_end {
            apply_bounded_line(&mut line, over_limit, &mut apply);
            over_limit = false;
        }
    }

    if !line.is_empty() && !over_limit {
        apply_bounded_line(&mut line, false, &mut apply);
    }
}

/// 提交一行已完成读取的 JSONL 文本。
fn apply_bounded_line<F>(line: &mut Vec<u8>, over_limit: bool, apply: &mut F)
where
    F: FnMut(String),
{
    if over_limit {
        line.clear();
        return;
    }
    while line
        .last()
        .is_some_and(|byte| *byte == b'\n' || *byte == b'\r')
    {
        line.pop();
    }
    if line.is_empty() {
        return;
    }
    if let Ok(text) = String::from_utf8(std::mem::take(line)) {
        apply(text);
    }
}

/// 递归收集 rollout 文件候选。
fn collect_rollout_candidates(
    entries: fs::ReadDir,
    cutoff: SystemTime,
    candidates: &mut Vec<(PathBuf, SystemTime)>,
    remaining_entries: &mut usize,
) {
    for entry in entries.flatten() {
        if *remaining_entries == 0 {
            break;
        }
        *remaining_entries -= 1;
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            if let Ok(children) = fs::read_dir(path) {
                collect_rollout_candidates(children, cutoff, candidates, remaining_entries);
            }
            continue;
        }
        if !metadata.is_file() || !is_rollout_file(&path) {
            continue;
        }
        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
        if modified >= cutoff {
            candidates.push((path, modified));
        }
    }
}

/// 判断是否是 Codex rollout JSONL。
fn is_rollout_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("rollout-") && name.ends_with(".jsonl"))
}

/// rollout reducer 内部状态。
struct CodexRolloutState {
    /// 会话 ID。
    session_id: Option<String>,
    /// 真实 cwd。
    cwd: Option<String>,
    /// 最近摘要。
    summary: Option<String>,
    /// 最近 Agent 输出。
    last_agent_message: Option<String>,
    /// 是否已完成。
    completed: bool,
    /// rollout 文件路径。
    path: PathBuf,
    /// 最近更新时间。
    updated_at: UnixMillis,
    /// 当前 reducer 是否处在 Codex 内部隐藏 turn 中。
    current_turn_is_internal: bool,
    /// 当前未完成的 Codex App 等待输入。
    pending_user_input: Option<CodexRolloutPendingUserInput>,
}

impl CodexRolloutState {
    /// 创建空状态。
    fn new(path: PathBuf, updated_at: UnixMillis) -> Self {
        Self {
            session_id: None,
            cwd: None,
            summary: None,
            last_agent_message: None,
            completed: false,
            path,
            updated_at,
            current_turn_is_internal: false,
            pending_user_input: None,
        }
    }

    /// 转换为快照。
    fn into_snapshot(self) -> Option<CodexRolloutSnapshot> {
        Some(CodexRolloutSnapshot {
            session_id: self.session_id?,
            cwd: self.cwd?,
            summary: self.summary,
            last_agent_message: self.last_agent_message,
            path: self.path,
            updated_at: self.updated_at,
            completed: self.completed,
            pending_user_input: self.pending_user_input,
        })
    }
}

/// 应用一行 rollout JSON。
fn apply_rollout_line(line: &str, state: &mut CodexRolloutState) {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return;
    };
    let Some(object) = value.as_object() else {
        return;
    };
    let Some(payload) = object.get("payload").and_then(Value::as_object) else {
        return;
    };

    match object.get("type").and_then(Value::as_str) {
        Some("session_meta") => apply_session_meta(payload, state),
        Some("event_msg") => apply_event_msg(payload, state),
        Some("response_item") => apply_response_item(payload, state),
        _ => {}
    }
}

/// 应用 session_meta。
fn apply_session_meta(payload: &serde_json::Map<String, Value>, state: &mut CodexRolloutState) {
    if let Some(session_id) = clean_string(payload.get("id")) {
        state.session_id = Some(session_id);
    }
    if let Some(cwd) = clean_string(payload.get("cwd")) {
        state.cwd = Some(cwd);
    }
}

/// 应用 event_msg。
fn apply_event_msg(payload: &serde_json::Map<String, Value>, state: &mut CodexRolloutState) {
    match payload.get("type").and_then(Value::as_str) {
        Some("agent_message") => {
            if state.current_turn_is_internal {
                return;
            }
            if let Some(message) = clean_string(payload.get("message")) {
                apply_agent_message(message, state);
            }
        }
        Some("task_complete") | Some("turn_complete") => {
            if state.current_turn_is_internal {
                return;
            }
            state.completed = true;
            state.pending_user_input = None;
            if let Some(message) = clean_string(payload.get("last_agent_message")) {
                apply_agent_message(message, state);
            }
        }
        Some("turn_aborted") => {
            if state.current_turn_is_internal {
                return;
            }
            state.completed = true;
            state.pending_user_input = None;
        }
        Some("user_message") => {
            if let Some(message) = clean_string(payload.get("message")) {
                // 过滤 Codex 内部生成的隐藏 turn，避免内部任务覆盖真实用户摘要。
                if is_codex_internal_prompt(&message) {
                    state.current_turn_is_internal = true;
                    return;
                }
                state.current_turn_is_internal = false;
                if state.completed {
                    state.completed = false;
                }
                state.pending_user_input = None;
                state.last_agent_message = None;
                state.summary = Some(truncate(&message, 120));
            }
        }
        Some("exec_command_begin")
        | Some("terminal_interaction")
        | Some("patch_apply_begin")
        | Some("patch_apply_updated")
        | Some("mcp_tool_call_begin") => {}
        Some("dynamic_tool_call_request") => {}
        Some("web_search_end") => {}
        Some("image_generation_end") => {}
        Some("view_image_tool_call") => {}
        Some("exec_command_end")
        | Some("patch_apply_end")
        | Some("mcp_tool_call_end")
        | Some("dynamic_tool_call_response") => {}
        _ => {}
    }
}

/// 应用 response_item。
fn apply_response_item(payload: &serde_json::Map<String, Value>, state: &mut CodexRolloutState) {
    if state.current_turn_is_internal {
        return;
    }
    let item = payload
        .get("item")
        .and_then(Value::as_object)
        .unwrap_or(payload);
    match item.get("type").and_then(Value::as_str) {
        Some("message") if item.get("role").and_then(Value::as_str) == Some("assistant") => {
            if let Some(message) = response_message_text(item, "output_text") {
                apply_agent_message(message, state);
            }
        }
        Some("function_call") => {
            if let Some(pending) = pending_user_input_from_response_item(payload, item) {
                state.pending_user_input = Some(pending);
            }
        }
        Some("custom_tool_call")
        | Some("local_shell_call")
        | Some("tool_search_call")
        | Some("web_search_call")
        | Some("image_generation_call") => {}
        Some("function_call_output") | Some("custom_tool_call_output") => {
            let call_id = clean_string(item.get("call_id"));
            if state
                .pending_user_input
                .as_ref()
                .is_some_and(|pending| call_id.as_deref() == Some(pending.call_id.as_str()))
            {
                state.pending_user_input = None;
            }
        }
        _ => {}
    }
}

/// 应用 Agent 输出。
fn apply_agent_message(message: String, state: &mut CodexRolloutState) {
    let message = truncate_strict(&message, MAX_FINAL_OUTPUT_CHARS);
    state.last_agent_message = Some(message.clone());
    state.summary = Some(message);
}

/// 提取 response message 文本。
fn response_message_text(item: &serde_json::Map<String, Value>, text_type: &str) -> Option<String> {
    let content = item.get("content")?;
    if let Some(text) = clean_string(Some(content)) {
        return Some(text);
    }

    let items = content.as_array()?;
    let mut parts = Vec::new();
    for item in items {
        let Some(object) = item.as_object() else {
            continue;
        };
        if object.get("type").and_then(Value::as_str) != Some(text_type) {
            continue;
        }
        if let Some(text) = clean_string(object.get("text")) {
            parts.push(text);
        }
    }

    let text = parts.join("\n").trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn activity_event(
    state: &CodexRolloutTailState,
    summary: &str,
    updated_at: UnixMillis,
) -> AgentEvent {
    AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
        session_key: state.session_key.clone(),
        summary: summary.to_string(),
        updated_at,
    })
}

fn user_message_event(
    state: &CodexRolloutTailState,
    summary: &str,
    updated_at: UnixMillis,
) -> AgentEvent {
    AgentEvent::UserMessageUpdated(UserMessageUpdatedEvent {
        session_key: state.session_key.clone(),
        summary: truncate(summary, 120),
        updated_at,
    })
}

/// 清洗字符串。
fn clean_string(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// 转换系统时间。
fn system_time_to_unix_millis(value: SystemTime) -> Option<UnixMillis> {
    let millis = value.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64;
    Some(UnixMillis::new(millis))
}

/// 截断文本。
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

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::time::Duration;

    use super::{
        CodexRolloutDiscovery, CodexRolloutSnapshot, CodexRolloutTailer, CodexRolloutWatchTarget,
        MAX_FINAL_OUTPUT_CHARS, MAX_LINE_BYTES,
    };
    use crate::domain::agent_event::AgentEvent;
    use crate::domain::agent_interaction::{AnswerInteraction, ReplyTarget};
    use crate::domain::agent_session::{AgentKind, ConversationId, ProjectId, SessionKey};
    use crate::domain::usage::UnixMillis;

    #[test]
    fn rollout_reads_session_meta_and_final_agent_message() {
        let root = test_root("rollout-final");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(
            &file,
            [
                r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
                r#"{"type":"event_msg","payload":{"type":"agent_message","message":"正在检查实现。"}}"#,
                r#"{"type":"event_msg","payload":{"type":"task_complete","last_agent_message":"实现已完成。"}}"#,
                r#"{"type":"event_msg","payload":{"type":"exec_command_end","status":"completed"}}"#,
            ]
            .join("\n"),
        )
        .expect("rollout should write");

        let snapshot = CodexRolloutDiscovery::new(root.clone(), Duration::from_secs(60), 10)
            .read_path(&file)
            .expect("snapshot should exist");

        assert_eq!(snapshot.session_id, "thread-1");
        assert_eq!(snapshot.cwd, "/tmp/builder-panel");
        assert_eq!(snapshot.summary.as_deref(), Some("实现已完成。"));
        assert_eq!(snapshot.last_agent_message.as_deref(), Some("实现已完成。"));
        assert!(snapshot.completed);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_active_snapshot_is_not_completed() {
        let snapshot = snapshot_from_lines(&[
            r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
            r#"{"type":"event_msg","payload":{"type":"agent_message","message":"正在检查实现。"}}"#,
        ]);

        assert!(!snapshot.completed);
    }

    #[test]
    fn rollout_completion_marks_snapshot_completed() {
        let snapshot = snapshot_from_lines(&[
            r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
            r#"{"type":"event_msg","payload":{"type":"turn_complete","last_agent_message":"完成"}}"#,
        ]);

        assert!(snapshot.completed);
    }

    #[test]
    fn rollout_user_message_after_completion_reopens_snapshot() {
        let snapshot = snapshot_from_lines(&[
            r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
            r#"{"type":"event_msg","payload":{"type":"turn_complete","last_agent_message":"上一轮完成"}}"#,
            r#"{"type":"event_msg","payload":{"type":"user_message","message":"继续处理"}}"#,
        ]);

        assert!(!snapshot.completed);
        assert_eq!(snapshot.summary.as_deref(), Some("继续处理"));
        assert_eq!(snapshot.last_agent_message, None);
    }

    #[test]
    fn rollout_reads_assistant_output_text_from_response_item() {
        let snapshot = snapshot_from_lines(&[
            r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"第一段"},{"type":"output_text","text":"第二段"}]}}"#,
        ]);

        assert_eq!(snapshot.summary.as_deref(), Some("第一段\n第二段"));
    }

    #[test]
    fn rollout_final_message_respects_strict_limit() {
        let message = "甲".repeat(MAX_FINAL_OUTPUT_CHARS + 1);
        let snapshot = snapshot_from_lines(&[
            r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
            &format!(
                r#"{{"type":"event_msg","payload":{{"type":"task_complete","last_agent_message":"{message}"}}}}"#
            ),
        ]);

        let summary = snapshot.summary.expect("summary should exist");
        assert_eq!(summary.chars().count(), MAX_FINAL_OUTPUT_CHARS);
    }

    #[test]
    fn rollout_response_item_respects_strict_limit() {
        let message = "甲".repeat(MAX_FINAL_OUTPUT_CHARS + 1);
        let snapshot = snapshot_from_lines(&[
            r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
            &format!(
                r#"{{"type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"{message}"}}]}}}}"#
            ),
        ]);

        let summary = snapshot.summary.expect("summary should exist");
        assert_eq!(summary.chars().count(), MAX_FINAL_OUTPUT_CHARS);
    }

    #[test]
    fn rollout_supports_nested_response_item_payload() {
        let snapshot = snapshot_from_lines(&[
            r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
            r#"{"type":"response_item","payload":{"item":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"嵌套输出"}]}}}"#,
        ]);

        assert_eq!(snapshot.summary.as_deref(), Some("嵌套输出"));
    }

    #[test]
    fn rollout_snapshot_recovers_pending_user_input_question() {
        let snapshot = snapshot_from_lines(&[
            r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
            r#"{"type":"event_msg","payload":{"type":"user_message","message":"继续"}} "#,
            r#"{"type":"response_item","payload":{"internal_chat_message_metadata_passthrough":{"turn_id":"turn-1"},"item":{"type":"function_call","name":"request_user_input","call_id":"call-1","arguments":"{\"questions\":[{\"id\":\"stop_scope\",\"header\":\"范围\",\"question\":\"选择停止能力范围\",\"options\":[{\"label\":\"仅 Codex APP (Recommended)\",\"description\":\"只接入 APP\"},{\"label\":\"Codex APP + CLI\",\"description\":\"同时接入 CLI\"}]}]}"}}}"#,
        ]);

        let pending = snapshot
            .pending_user_input
            .expect("pending user input should recover");

        assert_eq!(pending.turn_id, "turn-1");
        assert_eq!(pending.call_id, "call-1");
        assert_eq!(pending.request_summary(), "选择停止能力范围");
    }

    #[test]
    fn rollout_tailer_emits_pending_user_input_for_appended_request() {
        let root = test_root("rollout-tail-user-input");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(&file, "").expect("rollout should write");
        let session_key = SessionKey::new(
            AgentKind::CodexApp,
            ProjectId::new("/tmp/builder-panel"),
            ConversationId::new("thread-1"),
        );
        let mut tailer = CodexRolloutTailer::new(root.clone());
        tailer.sync_targets(vec![CodexRolloutWatchTarget {
            session_key: session_key.clone(),
            path: file.clone(),
        }]);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .expect("rollout should open")
            .write_all(
                r#"{"type":"response_item","payload":{"internal_chat_message_metadata_passthrough":{"turn_id":"turn-1"},"item":{"type":"function_call","name":"request_user_input","call_id":"call-1","arguments":"{\"questions\":[{\"id\":\"stop_scope\",\"question\":\"选择停止能力范围\",\"options\":[{\"label\":\"仅 Codex APP (Recommended)\",\"description\":\"只接入 APP\"},{\"label\":\"Codex APP + CLI\",\"description\":\"同时接入 CLI\"}]}]}"}}}
"#
                .as_bytes(),
            )
            .expect("rollout should append");

        let events = tailer.poll_events(UnixMillis::new(2));
        let [AgentEvent::ActivityUpdated(activity), AgentEvent::AnswerRequested(answer)] =
            events.as_slice()
        else {
            panic!("tailer should emit summary and answer request");
        };
        let AnswerInteraction::TextReply(reply) = &answer.interaction else {
            panic!("answer request should be text reply");
        };

        assert_eq!(activity.summary, "选择停止能力范围");
        assert_eq!(answer.session_key, session_key);
        assert_eq!(reply.request_summary, "选择停止能力范围");
        assert!(matches!(reply.reply_target, ReplyTarget::ExternalOnly(_)));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_tailer_clears_scanned_pending_user_input_output() {
        let root = test_root("rollout-tail-user-input-output");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(
            &file,
            r#"{"type":"response_item","payload":{"internal_chat_message_metadata_passthrough":{"turn_id":"turn-1"},"item":{"type":"function_call","name":"request_user_input","call_id":"call-1","arguments":"{\"questions\":[{\"id\":\"stop_scope\",\"question\":\"选择停止能力范围\",\"options\":[{\"label\":\"仅 Codex APP (Recommended)\",\"description\":\"只接入 APP\"},{\"label\":\"Codex APP + CLI\",\"description\":\"同时接入 CLI\"}]}]}"}}}"#,
        )
        .expect("rollout should write");
        let session_key = SessionKey::new(
            AgentKind::CodexApp,
            ProjectId::new("/tmp/builder-panel"),
            ConversationId::new("thread-1"),
        );
        let mut tailer = CodexRolloutTailer::new(root.clone());
        tailer.sync_targets(vec![CodexRolloutWatchTarget {
            session_key: session_key.clone(),
            path: file.clone(),
        }]);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .expect("rollout should open")
            .write_all(
                r#"
{"type":"response_item","payload":{"item":{"type":"function_call_output","call_id":"call-1","output":"{\"answers\":{}}"}}}
"#
                .as_bytes(),
            )
            .expect("rollout should append");

        let events = tailer.poll_events(UnixMillis::new(2));
        let [AgentEvent::InteractionCompleted(completed)] = events.as_slice() else {
            panic!("tailer should emit interaction completion");
        };

        assert_eq!(completed.session_key, session_key);
        assert_eq!(completed.summary, None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_rejects_path_outside_root() {
        let root = test_root("rollout-root");
        let outside = test_root("rollout-outside").join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::create_dir_all(outside.parent().expect("outside should have parent"))
            .expect("outside root should create");
        std::fs::write(
            &outside,
            r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
        )
        .expect("outside rollout should write");

        let snapshot = CodexRolloutDiscovery::new(root.clone(), Duration::from_secs(60), 10)
            .read_path(&outside);

        assert!(snapshot.is_none());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(outside.parent().expect("outside should have parent"));
    }

    #[test]
    fn rollout_skips_overlong_line_without_dropping_later_lines() {
        let root = test_root("rollout-overlong");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        let overlong = "x".repeat(MAX_LINE_BYTES + 1);
        std::fs::write(
            &file,
            [
                overlong,
                r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#
                    .to_string(),
                r#"{"type":"event_msg","payload":{"type":"agent_message","message":"有效输出"}}"#
                    .to_string(),
            ]
            .join("\n"),
        )
        .expect("rollout should write");

        let snapshot = CodexRolloutDiscovery::new(root.clone(), Duration::from_secs(60), 10)
            .read_path(&file)
            .expect("snapshot should exist");

        assert_eq!(snapshot.summary.as_deref(), Some("有效输出"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_tailer_ignores_appended_tool_preview_lines_for_last_message() {
        let root = test_root("rollout-tail");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(
            &file,
            r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
        )
        .expect("rollout should write");

        let session_key = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("/tmp/builder-panel"),
            ConversationId::new("thread-1"),
        );
        let mut tailer = CodexRolloutTailer::new(root.clone());
        tailer.sync_targets(vec![CodexRolloutWatchTarget {
            session_key: session_key.clone(),
            path: file.clone(),
        }]);
        assert_eq!(tailer.tracked_count(), 1);
        assert!(tailer.poll_events(UnixMillis::new(1)).is_empty());

        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .expect("rollout should open")
            .write_all(
                br#"
{"type":"event_msg","payload":{"type":"exec_command_begin","command":["bash","-lc","cargo test"]}}
"#,
            )
            .expect("rollout should append");

        let events = tailer.poll_events(UnixMillis::new(2));

        assert!(events.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_tailer_resets_when_file_is_replaced_without_shrinking() {
        let root = test_root("rollout-tail-replace");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(
            &file,
            [
                r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
                r#"{"type":"event_msg","payload":{"type":"agent_message","message":"历史输出"}}"#,
            ]
            .join("\n"),
        )
        .expect("rollout should write");
        let session_key = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("/tmp/builder-panel"),
            ConversationId::new("thread-1"),
        );
        let mut tailer = CodexRolloutTailer::new(root.clone());
        tailer.sync_targets(vec![CodexRolloutWatchTarget {
            session_key,
            path: file.clone(),
        }]);

        std::fs::remove_file(&file).expect("old rollout should remove");
        std::fs::write(
            &file,
            [
                r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
                r#"{"type":"event_msg","payload":{"type":"exec_command_begin","command":["bash","-lc","cargo test"]}}"#,
                r#"{"type":"event_msg","payload":{"type":"agent_message","message":"替换文件历史输出"}}"#,
            ]
            .join("\n"),
        )
        .expect("replacement rollout should write");

        assert!(tailer.poll_events(UnixMillis::new(2)).is_empty());

        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .expect("rollout should open")
            .write_all(
                "\n{\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"替换后追加输出\"}}\n"
                    .as_bytes(),
            )
            .expect("rollout should append");
        let events = tailer.poll_events(UnixMillis::new(3));

        assert_eq!(events.len(), 1);
        let AgentEvent::ActivityUpdated(event) = &events[0] else {
            panic!("event should be activity");
        };
        assert_eq!(event.summary, "替换后追加输出");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_tailer_emits_user_message_with_explicit_kind() {
        let root = test_root("rollout-tail-user");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(&file, "").expect("rollout should write");
        let session_key = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("/tmp/builder-panel"),
            ConversationId::new("thread-1"),
        );
        let mut tailer = CodexRolloutTailer::new(root.clone());
        tailer.sync_targets(vec![CodexRolloutWatchTarget {
            session_key: session_key.clone(),
            path: file.clone(),
        }]);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .expect("rollout should open")
            .write_all(
                r#"{"type":"event_msg","payload":{"type":"user_message","message":"继续处理"}}
"#
                .as_bytes(),
            )
            .expect("rollout should append");

        let events = tailer.poll_events(UnixMillis::new(2));

        assert_eq!(events.len(), 1);
        let AgentEvent::UserMessageUpdated(event) = &events[0] else {
            panic!("event should be user message");
        };
        assert_eq!(event.session_key, session_key);
        assert_eq!(event.summary, "继续处理");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_snapshot_ignores_codex_internal_prompt() {
        // 内部建议任务的 user_message 不应覆盖此前真实用户摘要。
        let snapshot = snapshot_from_lines(&[
            r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
            r#"{"type":"event_msg","payload":{"type":"user_message","message":"重构旅游规划提示词"}}"#,
            r#"{"type":"event_msg","payload":{"type":"user_message","message":"Generate 0 to 3 hyperpersonalized suggestions for the user."}}"#,
        ]);

        assert_eq!(snapshot.summary.as_deref(), Some("重构旅游规划提示词"));
    }

    #[test]
    fn rollout_snapshot_internal_turn_does_not_override_visible_completion() {
        let snapshot = snapshot_from_lines(&[
            r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
            r#"{"type":"event_msg","payload":{"type":"user_message","message":"重构旅游规划提示词"}}"#,
            r#"{"type":"event_msg","payload":{"type":"turn_complete","last_agent_message":"真实任务完成"}}"#,
            r#"{"type":"event_msg","payload":{"type":"user_message","message":"You are an expert at upholding safety and compliance standards for Codex ambient suggestions."}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"{\"exclude\":[]}"}]}}"#,
            r#"{"type":"event_msg","payload":{"type":"agent_message","message":"{\"exclude\":[]}"}}"#,
            r#"{"type":"event_msg","payload":{"type":"turn_complete","last_agent_message":"{\"exclude\":[]}"}}"#,
        ]);

        assert_eq!(snapshot.summary.as_deref(), Some("真实任务完成"));
        assert_eq!(snapshot.last_agent_message.as_deref(), Some("真实任务完成"));
        assert!(snapshot.completed);
    }

    #[test]
    fn rollout_tailer_skips_codex_internal_prompt() {
        let root = test_root("rollout-tail-internal");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(&file, "").expect("rollout should write");
        let session_key = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("/tmp/builder-panel"),
            ConversationId::new("thread-1"),
        );
        let mut tailer = CodexRolloutTailer::new(root.clone());
        tailer.sync_targets(vec![CodexRolloutWatchTarget {
            session_key: session_key.clone(),
            path: file.clone(),
        }]);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .expect("rollout should open")
            .write_all(
                r#"{"type":"event_msg","payload":{"type":"user_message","message":"Generate 0 to 3 hyperpersonalized suggestions for the user."}}
"#
                .as_bytes(),
            )
            .expect("rollout should append");

        let events = tailer.poll_events(UnixMillis::new(2));

        assert!(
            events.is_empty(),
            "internal suggestion prompt should not emit user-message event"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_tailer_suppresses_internal_turn_outputs_after_existing_prompt() {
        let root = test_root("rollout-tail-internal-existing");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(
            &file,
            [
                r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
                r#"{"type":"event_msg","payload":{"type":"user_message","message":"Create Codex ambient suggestions for the current thread."}}"#,
            ]
            .join("\n"),
        )
        .expect("rollout should write");
        let session_key = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("/tmp/builder-panel"),
            ConversationId::new("thread-1"),
        );
        let mut tailer = CodexRolloutTailer::new(root.clone());
        tailer.sync_targets(vec![CodexRolloutWatchTarget {
            session_key,
            path: file.clone(),
        }]);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .expect("rollout should open")
            .write_all(
                br#"
{"type":"response_item","payload":{"item":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"ls\"}"}}}
{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"{\"suggestions\":[]}"}]}}
{"type":"event_msg","payload":{"type":"agent_message","message":"{\"suggestions\":[]}"}}
{"type":"event_msg","payload":{"type":"turn_complete","last_agent_message":"{\"suggestions\":[]}"}}
"#,
            )
            .expect("rollout should append");

        let events = tailer.poll_events(UnixMillis::new(2));

        assert!(events.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_tailer_real_user_message_after_internal_turn_is_visible() {
        let root = test_root("rollout-tail-internal-then-real");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(
            &file,
            [
                r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/builder-panel"}}"#,
                r#"{"type":"event_msg","payload":{"type":"user_message","message":"Create Codex ambient suggestions for the current thread."}}"#,
            ]
            .join("\n"),
        )
        .expect("rollout should write");
        let session_key = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("/tmp/builder-panel"),
            ConversationId::new("thread-1"),
        );
        let mut tailer = CodexRolloutTailer::new(root.clone());
        tailer.sync_targets(vec![CodexRolloutWatchTarget {
            session_key,
            path: file.clone(),
        }]);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .expect("rollout should open")
            .write_all(
                r#"
{"type":"event_msg","payload":{"type":"turn_complete","last_agent_message":"{\"suggestions\":[]}"}}
{"type":"event_msg","payload":{"type":"user_message","message":"继续真实任务"}}
"#
                .as_bytes(),
            )
            .expect("rollout should append");

        let events = tailer.poll_events(UnixMillis::new(2));

        assert_eq!(events.len(), 1);
        let AgentEvent::UserMessageUpdated(event) = &events[0] else {
            panic!("event should be user message");
        };
        assert_eq!(event.summary, "继续真实任务");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_tailer_ignores_tool_output_end() {
        let root = test_root("rollout-tail-thinking");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(&file, "").expect("rollout should write");
        let session_key = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("/tmp/builder-panel"),
            ConversationId::new("thread-1"),
        );
        let mut tailer = CodexRolloutTailer::new(root.clone());
        tailer.sync_targets(vec![CodexRolloutWatchTarget {
            session_key,
            path: file.clone(),
        }]);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .expect("rollout should open")
            .write_all(
                br#"{"type":"event_msg","payload":{"type":"exec_command_end"}}
"#,
            )
            .expect("rollout should append");

        let events = tailer.poll_events(UnixMillis::new(2));

        assert!(events.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_tailer_offsets_repeated_events_in_same_poll() {
        let root = test_root("rollout-tail-repeat");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(&file, "").expect("rollout should write");
        let session_key = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("/tmp/builder-panel"),
            ConversationId::new("thread-1"),
        );
        let mut tailer = CodexRolloutTailer::new(root.clone());
        tailer.sync_targets(vec![CodexRolloutWatchTarget {
            session_key,
            path: file.clone(),
        }]);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .expect("rollout should open")
            .write_all(
                br#"{"type":"event_msg","payload":{"type":"exec_command_begin","command":["bash","-lc","cargo test"]}}
{"type":"event_msg","payload":{"type":"exec_command_begin","command":["bash","-lc","cargo test"]}}
"#,
            )
            .expect("rollout should append");

        let events = tailer.poll_events(UnixMillis::new(20));

        assert!(events.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_tailer_does_not_expose_unknown_json_arguments() {
        let root = test_root("rollout-tail-json");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(&file, "").expect("rollout should write");
        let session_key = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("/tmp/builder-panel"),
            ConversationId::new("thread-1"),
        );
        let mut tailer = CodexRolloutTailer::new(root.clone());
        tailer.sync_targets(vec![CodexRolloutWatchTarget {
            session_key,
            path: file.clone(),
        }]);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .expect("rollout should open")
            .write_all(
                br#"{"type":"event_msg","payload":{"type":"dynamic_tool_call_request","arguments":{"token":"secret"}}}
{"type":"response_item","payload":{"item":{"type":"custom_tool_call","name":"unknown","arguments":{"token":"secret"}}}}
{"type":"response_item","payload":{"item":{"type":"function_call","name":"unknown_tool","arguments":"{\"token\":\"secret\"}"}}}
"#,
            )
            .expect("rollout should append");

        let events = tailer.poll_events(UnixMillis::new(20));

        assert!(events.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_tailer_emits_activity_for_function_call_intermediate_steps() {
        let root = test_root("rollout-tail-fn-call");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(&file, "").expect("rollout should write");
        let session_key = SessionKey::new(
            AgentKind::CodexApp,
            ProjectId::new("/tmp/builder-panel"),
            ConversationId::new("thread-1"),
        );
        let mut tailer = CodexRolloutTailer::new(root.clone());
        tailer.sync_targets(vec![CodexRolloutWatchTarget {
            session_key,
            path: file.clone(),
        }]);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .expect("rollout should open")
            .write_all(
                br#"{"type":"response_item","payload":{"item":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"ls -la\"}"}}}
{"type":"response_item","payload":{"item":{"type":"function_call","name":"spawn_agent","arguments":"{\"agent_type\":\"plan_reviewer\"}"}}}
{"type":"response_item","payload":{"item":{"type":"function_call","name":"wait_agent","arguments":"{}"}}}
"#,
            )
            .expect("rollout should append");

        let events = tailer.poll_events(UnixMillis::new(20));

        assert_eq!(events.len(), 3);
        for event in &events {
            match event {
                AgentEvent::ActivityUpdated(updated) => {
                    assert!(!updated.summary.is_empty());
                }
                other => panic!("expected ActivityUpdated, got {:?}", other),
            }
        }
        let summaries: Vec<_> = events
            .iter()
            .map(|event| match event {
                AgentEvent::ActivityUpdated(updated) => updated.summary.clone(),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(summaries[0], "执行: ls -la");
        assert_eq!(summaries[1], "调用子 Agent: plan_reviewer");
        assert_eq!(summaries[2], "等待子 Agent 返回…");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_tailer_skips_overlong_appended_line_and_reads_next_line() {
        let root = test_root("rollout-tail-overlong");
        let file = root.join("rollout-thread-1.jsonl");
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(&file, "").expect("rollout should write");
        let session_key = SessionKey::new(
            AgentKind::CodexCli,
            ProjectId::new("/tmp/builder-panel"),
            ConversationId::new("thread-1"),
        );
        let mut tailer = CodexRolloutTailer::new(root.clone());
        tailer.sync_targets(vec![CodexRolloutWatchTarget {
            session_key,
            path: file.clone(),
        }]);
        let overlong_line = "x".repeat(MAX_LINE_BYTES + 1);
        let valid_line =
            r#"{"type":"event_msg","payload":{"type":"agent_message","message":"有效输出"}}"#;
        std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .expect("rollout should open")
            .write_all(format!("{overlong_line}\n{valid_line}\n").as_bytes())
            .expect("rollout should append");

        let mut events = Vec::new();
        for index in 0..12 {
            events.extend(tailer.poll_events(UnixMillis::new(20 + index)));
        }

        assert_eq!(events.len(), 1);
        let AgentEvent::ActivityUpdated(event) = &events[0] else {
            panic!("event should be activity");
        };
        assert_eq!(event.summary, "有效输出");
        let _ = std::fs::remove_dir_all(root);
    }

    fn snapshot_from_lines(lines: &[&str]) -> CodexRolloutSnapshot {
        let root = test_root("rollout-response-item");
        let file = root.join("rollout-thread-1.jsonl");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("root should create");
        std::fs::write(&file, lines.join("\n")).expect("rollout should write");
        let snapshot = CodexRolloutDiscovery::new(root.clone(), Duration::from_secs(60), 10)
            .read_path(&file)
            .expect("snapshot should exist");
        let _ = std::fs::remove_dir_all(root);
        snapshot
    }

    fn test_root(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir()
            .join(format!("{name}-{}", std::process::id()))
            .join(nanos.to_string())
    }
}
