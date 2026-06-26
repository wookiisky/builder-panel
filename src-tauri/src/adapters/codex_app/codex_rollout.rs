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
    ActivityUpdatedEvent, AgentEvent, TurnCompletedEvent, UserMessageUpdatedEvent,
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
            let metadata = fs::metadata(&path).ok();
            let offset = metadata
                .as_ref()
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            let file_identity = metadata
                .as_ref()
                .map(rollout_file_identity)
                .unwrap_or_else(empty_rollout_file_identity);
            next.insert(
                path,
                CodexRolloutTailState {
                    session_key: target.session_key,
                    offset,
                    file_identity,
                    partial_line: Vec::new(),
                    dropping_overlong_line: false,
                    completed: false,
                },
            );
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
        state.offset = metadata.len();
        state.file_identity = file_identity;
        state.partial_line.clear();
        state.dropping_overlong_line = false;
        state.completed = false;
        return Vec::new();
    }
    if metadata.len() < state.offset {
        state.offset = metadata.len();
        state.file_identity = file_identity;
        state.partial_line.clear();
        state.dropping_overlong_line = false;
        state.completed = false;
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
        if let Some(event) = live_event_from_rollout_line(&line, state, event_updated_at) {
            events.push(event);
        }
    }

    events
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

fn live_event_from_rollout_line(
    line: &str,
    state: &mut CodexRolloutTailState,
    updated_at: UnixMillis,
) -> Option<AgentEvent> {
    let value = serde_json::from_str::<Value>(line).ok()?;
    let object = value.as_object()?;
    let payload = object.get("payload").and_then(Value::as_object)?;

    match object.get("type").and_then(Value::as_str) {
        Some("event_msg") => live_event_from_event_msg(payload, state, updated_at),
        Some("response_item") => live_event_from_response_item(payload, state, updated_at),
        _ => None,
    }
}

fn live_event_from_event_msg(
    payload: &serde_json::Map<String, Value>,
    state: &mut CodexRolloutTailState,
    updated_at: UnixMillis,
) -> Option<AgentEvent> {
    match payload.get("type").and_then(Value::as_str) {
        Some("turn_started") | Some("task_started") => {
            state.completed = false;
            None
        }
        Some("user_message") => {
            state.completed = false;
            clean_string(payload.get("message"))
                // 过滤 Codex 内部生成的隐藏 turn，只保留真实用户任务。
                .filter(|message| !is_codex_internal_prompt(message))
                .map(|message| user_message_event(state, &message, updated_at))
        }
        Some("agent_message") => clean_string(payload.get("message")).map(|message| {
            activity_event(
                state,
                &truncate_strict(&message, MAX_FINAL_OUTPUT_CHARS),
                updated_at,
            )
        }),
        Some("task_complete") | Some("turn_complete") => {
            state.completed = true;
            let summary = clean_string(payload.get("last_agent_message"))
                .map(|message| truncate_strict(&message, MAX_FINAL_OUTPUT_CHARS));
            Some(AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key: state.session_key.clone(),
                summary,
                updated_at,
            }))
        }
        Some("turn_aborted") => {
            state.completed = true;
            Some(AgentEvent::TurnCompleted(TurnCompletedEvent {
                session_key: state.session_key.clone(),
                summary: None,
                updated_at,
            }))
        }
        Some("exec_command_begin")
        | Some("terminal_interaction")
        | Some("patch_apply_begin")
        | Some("patch_apply_updated")
        | Some("mcp_tool_call_begin") => None,
        Some("dynamic_tool_call_request") => None,
        Some("web_search_begin") => None,
        Some("web_search_end") => None,
        Some("image_generation_begin") => None,
        Some("image_generation_end") => None,
        Some("view_image_tool_call") => None,
        Some("plan_update") => None,
        Some("exec_command_end")
        | Some("patch_apply_end")
        | Some("mcp_tool_call_end")
        | Some("dynamic_tool_call_response") => None,
        _ => None,
    }
}

fn live_event_from_response_item(
    payload: &serde_json::Map<String, Value>,
    state: &mut CodexRolloutTailState,
    updated_at: UnixMillis,
) -> Option<AgentEvent> {
    let item = payload
        .get("item")
        .and_then(Value::as_object)
        .unwrap_or(payload);
    match item.get("type").and_then(Value::as_str) {
        Some("message") if item.get("role").and_then(Value::as_str) == Some("assistant") => {
            response_message_text(item, "output_text").map(|message| {
                activity_event(
                    state,
                    &truncate_strict(&message, MAX_FINAL_OUTPUT_CHARS),
                    updated_at,
                )
            })
        }
        Some("function_call") => function_call_activity(item)
            .map(|summary| activity_event(state, &summary, updated_at)),
        Some("local_shell_call") => Some(activity_event(state, "执行命令…", updated_at)),
        Some("custom_tool_call") => None,
        Some("tool_search_call") => {
            Some(activity_event(state, "搜索工具中…", updated_at))
        }
        Some("web_search_call") => Some(activity_event(state, "联网检索中…", updated_at)),
        Some("image_generation_call") => {
            Some(activity_event(state, "生成图像中…", updated_at))
        }
        Some("function_call_output") | Some("custom_tool_call_output") => None,
        _ => None,
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
            if let Some(message) = clean_string(payload.get("message")) {
                apply_agent_message(message, state);
            }
        }
        Some("task_complete") | Some("turn_complete") => {
            state.completed = true;
            if let Some(message) = clean_string(payload.get("last_agent_message")) {
                apply_agent_message(message, state);
            }
        }
        Some("turn_aborted") => {
            state.completed = true;
        }
        Some("user_message") => {
            if state.completed {
                state.completed = false;
            }
            if let Some(message) = clean_string(payload.get("message")) {
                // 过滤 Codex 内部生成的隐藏 turn，避免内部任务覆盖真实用户摘要。
                if !is_codex_internal_prompt(&message) {
                    state.summary = Some(truncate(&message, 120));
                }
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
        Some("function_call")
        | Some("custom_tool_call")
        | Some("local_shell_call")
        | Some("tool_search_call")
        | Some("web_search_call")
        | Some("image_generation_call") => {}
        Some("function_call_output") | Some("custom_tool_call_output") => {}
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
        let _ = std::fs::remove_dir_all(root);
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
