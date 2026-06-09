//! 日志脱敏 adapter。

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const MAX_LOG_STRING_CHARS: usize = 120;

/// 已清洗日志事件。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SanitizedLogEvent {
    /// 中文业务事件名。
    pub event_name: String,
    /// 已清洗日志载荷。
    pub payload: Value,
}

impl SanitizedLogEvent {
    /// 创建已清洗日志事件。
    pub fn new(event_name: impl Into<String>, payload: &Value) -> Self {
        Self {
            event_name: event_name.into(),
            payload: sanitize_log_value(payload),
        }
    }
}

/// 清洗日志载荷。
pub fn sanitize_log_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(sanitize_object(object)),
        Value::Array(items) => Value::Array(items.iter().map(sanitize_log_value).collect()),
        Value::String(text) => Value::String(sanitize_log_text(text)),
        Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
    }
}

/// 清洗普通日志文本。
pub fn sanitize_log_text(text: &str) -> String {
    if text.chars().count() <= MAX_LOG_STRING_CHARS {
        return text.to_string();
    }

    let prefix = text.chars().take(MAX_LOG_STRING_CHARS).collect::<String>();
    format!("{prefix}...[已截断]")
}

fn sanitize_object(object: &Map<String, Value>) -> Map<String, Value> {
    object
        .iter()
        .map(|(key, value)| {
            if is_sensitive_key(key) {
                return (key.clone(), Value::String("[已脱敏]".to_string()));
            }

            (key.clone(), sanitize_log_value(value))
        })
        .collect()
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("prompt")
        || key.contains("transcript")
        || key.contains("token")
        || key.contains("secret")
        || key.contains("password")
        || key.contains("api_key")
}

#[cfg(test)]
mod tests {
    use super::{sanitize_log_text, sanitize_log_value, SanitizedLogEvent};
    use serde_json::json;

    #[test]
    fn sensitive_payload_fields_are_redacted() {
        let payload = json!({
            "prompt": "请完整实现功能",
            "api_key": "sk-secret",
            "nested": {
                "transcript_path": "/tmp/transcript.jsonl",
                "safe_count": 3
            }
        });

        let sanitized = sanitize_log_value(&payload);

        assert_eq!(sanitized["prompt"], "[已脱敏]");
        assert_eq!(sanitized["api_key"], "[已脱敏]");
        assert_eq!(sanitized["nested"]["transcript_path"], "[已脱敏]");
        assert_eq!(sanitized["nested"]["safe_count"], 3);
    }

    #[test]
    fn long_log_text_is_truncated() {
        let text = "a".repeat(200);

        let sanitized = sanitize_log_text(&text);

        assert!(sanitized.ends_with("...[已截断]"));
        assert!(sanitized.chars().count() < text.chars().count());
    }

    #[test]
    fn sanitized_event_uses_chinese_business_event_name() {
        let event = SanitizedLogEvent::new("hook 配置安装", &json!({"prompt": "secret"}));

        assert_eq!(event.event_name, "hook 配置安装");
        assert_eq!(event.payload["prompt"], "[已脱敏]");
    }
}
