//! Agent hook payload 边界校验。

use serde_json::Value;

use crate::adapters::bridge::codec::{BridgeHookEventName, ValidatedHookPayload};
use crate::domain::agent_session::AgentKind;

/// hook 来源参数。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookSource {
    /// Codex CLI hook。
    Codex,
    /// Claude Code CLI hook。
    Claude,
}

impl HookSource {
    /// 从 CLI 参数解析 hook 来源。
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            _ => None,
        }
    }

    /// 转换为领域 agent 类型。
    pub fn agent_kind(self) -> AgentKind {
        match self {
            Self::Codex => AgentKind::CodexCli,
            Self::Claude => AgentKind::ClaudeCodeCli,
        }
    }
}

/// hook payload 校验错误。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HookPayloadValidationError {
    /// JSON 不是对象。
    NotObject,
    /// 缺少必填字段。
    MissingField(&'static str),
    /// 字段类型错误。
    InvalidField(&'static str),
    /// hook 事件不支持。
    UnsupportedEvent(String),
}

/// 校验 hook stdin JSON。
pub fn validate_hook_payload(
    source: HookSource,
    input: &[u8],
) -> Result<ValidatedHookPayload, HookPayloadValidationError> {
    let value: Value =
        serde_json::from_slice(input).map_err(|_| HookPayloadValidationError::NotObject)?;
    let object = value
        .as_object()
        .ok_or(HookPayloadValidationError::NotObject)?;

    let hook_event_name = required_string(object.get("hook_event_name"), "hook_event_name")?;
    let cwd = required_string(object.get("cwd"), "cwd")?;
    let session_id = required_string(object.get("session_id"), "session_id")?;

    let terminal_app = optional_string(object.get("terminal_app"), "terminal_app")?;
    let agent_kind = agent_kind_for_payload(source, terminal_app.as_deref());

    Ok(ValidatedHookPayload {
        agent_kind,
        hook_event_name: parse_event_name(source, &hook_event_name)?,
        cwd,
        session_id,
        model: optional_string(object.get("model"), "model")?,
        permission_mode: optional_string(object.get("permission_mode"), "permission_mode")?,
        transcript_path: optional_string(object.get("transcript_path"), "transcript_path")?,
        terminal_app,
        terminal_session_id: optional_string(
            object.get("terminal_session_id"),
            "terminal_session_id",
        )?,
        terminal_tty: optional_string(object.get("terminal_tty"), "terminal_tty")?,
        terminal_title: optional_string(object.get("terminal_title"), "terminal_title")?,
        turn_id: optional_string(object.get("turn_id"), "turn_id")?,
        tool_name: optional_string(object.get("tool_name"), "tool_name")?,
        tool_input: optional_object_value(object.get("tool_input"), "tool_input")?,
        prompt: optional_string(object.get("prompt"), "prompt")?,
        last_assistant_message: optional_string(
            object.get("last_assistant_message"),
            "last_assistant_message",
        )?,
        permission_suggestions: optional_array_value(
            object.get("permission_suggestions"),
            "permission_suggestions",
        )?,
    })
}

fn agent_kind_for_payload(source: HookSource, terminal_app: Option<&str>) -> AgentKind {
    if source == HookSource::Codex && terminal_app.map(is_codex_app_terminal).unwrap_or(false) {
        return AgentKind::CodexApp;
    }

    source.agent_kind()
}

/// 判断给定终端/进程标识是否对应 Codex.app。
///
/// `bundle_id`/`TERM_PROGRAM` 之类的字段都用同一套归一化:剥掉非字母数字,
/// 转小写,允许 "Codex.app"、"Codex App"、"com.openai.codex" 这类写法。
pub fn is_codex_app_terminal(value: &str) -> bool {
    let normalized = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();

    normalized == "codexapp" || normalized == "comopenaicodex"
}

fn parse_event_name(
    source: HookSource,
    value: &str,
) -> Result<BridgeHookEventName, HookPayloadValidationError> {
    let event = match value {
        "SessionStart" => BridgeHookEventName::SessionStart,
        "UserPromptSubmit" => BridgeHookEventName::UserPromptSubmit,
        "PreToolUse" => BridgeHookEventName::PreToolUse,
        "PermissionRequest" => BridgeHookEventName::PermissionRequest,
        "PostToolUse" => BridgeHookEventName::PostToolUse,
        "Notification" if source == HookSource::Claude => BridgeHookEventName::Notification,
        "Stop" => BridgeHookEventName::Stop,
        "SessionEnd" if source == HookSource::Claude => BridgeHookEventName::SessionEnd,
        other => {
            return Err(HookPayloadValidationError::UnsupportedEvent(
                other.to_string(),
            ))
        }
    };

    Ok(event)
}

fn required_string(
    value: Option<&Value>,
    field: &'static str,
) -> Result<String, HookPayloadValidationError> {
    let text =
        optional_string(value, field)?.ok_or(HookPayloadValidationError::MissingField(field))?;
    if text.trim().is_empty() {
        return Err(HookPayloadValidationError::InvalidField(field));
    }

    Ok(text)
}

fn optional_string(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Option<String>, HookPayloadValidationError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(HookPayloadValidationError::InvalidField(field)),
    }
}

fn optional_object_value(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Option<Value>, HookPayloadValidationError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(_)) => Ok(value.cloned()),
        Some(_) => Err(HookPayloadValidationError::InvalidField(field)),
    }
}

fn optional_array_value(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Option<Value>, HookPayloadValidationError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(_)) => Ok(value.cloned()),
        Some(_) => Err(HookPayloadValidationError::InvalidField(field)),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{validate_hook_payload, HookPayloadValidationError, HookSource};
    use crate::adapters::bridge::codec::BridgeHookEventName;
    use crate::domain::agent_session::AgentKind;

    #[test]
    fn codex_permission_payload_is_cleaned() {
        let input = json!({
            "cwd": "/tmp/project",
            "hook_event_name": "PermissionRequest",
            "session_id": "session-1",
            "model": "gpt-5-codex",
            "permission_mode": "default",
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test"},
            "turn_id": "turn-1"
        });

        let payload = validate_hook_payload(HookSource::Codex, input.to_string().as_bytes())
            .expect("payload should validate");

        assert_eq!(payload.agent_kind, AgentKind::CodexCli);
        assert_eq!(
            payload.hook_event_name,
            BridgeHookEventName::PermissionRequest
        );
        assert_eq!(payload.cwd, "/tmp/project");
        assert_eq!(payload.session_id, "session-1");
        assert_eq!(payload.tool_name.as_deref(), Some("Bash"));
    }

    #[test]
    fn codex_app_payload_is_separated_from_codex_cli() {
        let input = json!({
            "cwd": "/tmp/project",
            "hook_event_name": "PermissionRequest",
            "session_id": "thread-1",
            "model": "gpt-5-codex",
            "terminal_app": "Codex.app",
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test"}
        });

        let payload = validate_hook_payload(HookSource::Codex, input.to_string().as_bytes())
            .expect("payload should validate");

        assert_eq!(payload.agent_kind, AgentKind::CodexApp);
        assert_eq!(payload.terminal_app.as_deref(), Some("Codex.app"));
    }

    #[test]
    fn codex_app_payload_accepts_spaced_terminal_label() {
        let input = json!({
            "cwd": "/tmp/project",
            "hook_event_name": "PermissionRequest",
            "session_id": "thread-1",
            "terminal_app": "Codex App"
        });

        let payload = validate_hook_payload(HookSource::Codex, input.to_string().as_bytes())
            .expect("payload should validate");

        assert_eq!(payload.agent_kind, AgentKind::CodexApp);
    }

    #[test]
    fn claude_permission_suggestions_must_be_array() {
        let input = json!({
            "cwd": "/tmp/project",
            "hook_event_name": "PermissionRequest",
            "session_id": "session-1",
            "permission_suggestions": {"bad": true}
        });

        let result = validate_hook_payload(HookSource::Claude, input.to_string().as_bytes());

        assert_eq!(
            result,
            Err(HookPayloadValidationError::InvalidField(
                "permission_suggestions"
            ))
        );
    }

    #[test]
    fn malformed_json_is_rejected_before_bridge_command() {
        let result = validate_hook_payload(HookSource::Codex, b"{not-json}");

        assert_eq!(result, Err(HookPayloadValidationError::NotObject));
    }

    #[test]
    fn missing_required_fields_are_rejected() {
        let input = json!({
            "cwd": "/tmp/project",
            "hook_event_name": "PermissionRequest"
        });

        let result = validate_hook_payload(HookSource::Codex, input.to_string().as_bytes());

        assert_eq!(
            result,
            Err(HookPayloadValidationError::MissingField("session_id"))
        );
    }

    #[test]
    fn source_restricts_supported_events() {
        let input = json!({
            "cwd": "/tmp/project",
            "hook_event_name": "SessionEnd",
            "session_id": "session-1"
        });

        let result = validate_hook_payload(HookSource::Codex, input.to_string().as_bytes());

        assert_eq!(
            result,
            Err(HookPayloadValidationError::UnsupportedEvent(
                "SessionEnd".to_string()
            ))
        );
    }
}
