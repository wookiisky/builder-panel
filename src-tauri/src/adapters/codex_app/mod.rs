//! Codex APP adapter。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{json, Value};

use crate::domain::agent_event::{
    ActivityUpdatedEvent, AgentEvent, SessionStartedEvent, TurnCompletedEvent, UsageUpdatedEvent,
};
use crate::domain::agent_session::{
    AgentKind, ConversationId, ProjectId, SessionCapabilities, SessionKey,
};
use crate::domain::usage::{
    UnixMillis, UsageAmount, UsageSnapshot, UsageValue, VerifiedUsageValue,
};

const REQUIRED_SCHEMA_FILES: [&str; 9] = [
    "v2/ThreadStartParams.json",
    "v2/ThreadStartResponse.json",
    "v2/TurnStartParams.json",
    "v2/ThreadStartedNotification.json",
    "v2/TurnStartedNotification.json",
    "v2/AgentMessageDeltaNotification.json",
    "v2/ThreadTokenUsageUpdatedNotification.json",
    "v2/TurnCompletedNotification.json",
    "v2/ThreadStatusChangedNotification.json",
];

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
    let summary = match status_type.as_str() {
        "active" => "Codex APP thread 运行中",
        "idle" => "Codex APP thread 空闲",
        "systemError" => "Codex APP thread 系统错误",
        "notLoaded" => "Codex APP thread 未加载",
        _ => return Ok(None),
    };

    Ok(Some(AgentEvent::ActivityUpdated(ActivityUpdatedEvent {
        session_key: session_key(cwd, &thread_id),
        summary: summary.to_string(),
        updated_at,
    })))
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
            source_label: "Codex APP app-server".to_string(),
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
        can_send_reply: false,
        can_resolve_approval: false,
        can_create_followup_turn: false,
        can_view_process_timeline: false,
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

fn schema_probe_dir() -> PathBuf {
    std::env::temp_dir().join(format!("builder-panel-codex-schema-{}", unix_millis()))
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn required_schema_files() -> Vec<String> {
    REQUIRED_SCHEMA_FILES
        .iter()
        .map(|file| file.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{schema_probe_from_dir, CodexAppAdapter};
    use crate::domain::agent_event::AgentEvent;
    use crate::domain::agent_session::AgentKind;
    use crate::domain::usage::UnixMillis;

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
        assert!(!event.capabilities.can_create_followup_turn);
        assert!(!event.capabilities.can_view_process_timeline);
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
}
