//! Codex rollout JSONL 发现和摘要清洗。

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::domain::usage::UnixMillis;

const DEFAULT_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const DEFAULT_MAX_FILES: usize = 40;
const DEFAULT_MAX_VISITED_ENTRIES: usize = 5_000;
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_LINE_BYTES: usize = 2 * 1024 * 1024;

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
        let root = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".codex")
            .join("sessions");
        Self::new(root, DEFAULT_MAX_AGE, DEFAULT_MAX_FILES)
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
        if !is_rollout_file(path) {
            return None;
        }
        let root = self.root.canonicalize().ok()?;
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
            } else if state.summary.is_none() {
                state.summary = Some("Codex APP turn 已完成".to_string());
            }
        }
        Some("turn_aborted") => {
            state.completed = true;
            state.summary = Some("Codex APP turn 已中断".to_string());
        }
        Some("user_message") => {
            if state.completed {
                state.completed = false;
            }
            if let Some(message) = clean_string(payload.get("message")) {
                state.summary = Some(format!("用户输入：{}", truncate(&message, 120)));
            }
        }
        Some("exec_command_begin")
        | Some("patch_apply_begin")
        | Some("mcp_tool_call_begin")
        | Some("dynamic_tool_call_request")
        | Some("web_search_begin")
        | Some("image_generation_begin")
        | Some("view_image_tool_call")
        | Some("plan_update") => {
            if !state.completed {
                state.summary = Some("Codex APP 正在调用工具".to_string());
            }
        }
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
        | Some("image_generation_call") => {
            if !state.completed {
                state.summary = Some("Codex APP 正在调用工具".to_string());
            }
        }
        Some("function_call_output") | Some("custom_tool_call_output") => {
            if !state.completed {
                state.summary = Some("Codex APP 正在思考".to_string());
            }
        }
        _ => {}
    }
}

/// 应用 Agent 输出。
fn apply_agent_message(message: String, state: &mut CodexRolloutState) {
    let message = truncate(&message, 240);
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{CodexRolloutDiscovery, CodexRolloutSnapshot, MAX_LINE_BYTES};

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
