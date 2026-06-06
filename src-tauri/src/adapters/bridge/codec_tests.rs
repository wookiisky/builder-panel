//! bridge codec 测试。

use serde_json::json;

use super::{
    encode_request_line, encode_response_line, BridgeCommandType, BridgeDirectivePayload,
    BridgeHookEventName, BridgeRequestDecoder, BridgeRequestEnvelope, BridgeResponseDecoder,
    BridgeResponseEnvelope, BridgeResultType, ValidatedHookPayload, BRIDGE_SCHEMA_VERSION,
};
use crate::domain::agent_session::AgentKind;

fn payload() -> ValidatedHookPayload {
    ValidatedHookPayload {
        agent_kind: AgentKind::CodexCli,
        hook_event_name: BridgeHookEventName::PermissionRequest,
        cwd: "/tmp/project".to_string(),
        session_id: "session-1".to_string(),
        model: Some("gpt-5-codex".to_string()),
        permission_mode: Some("default".to_string()),
        transcript_path: None,
        turn_id: Some("turn-1".to_string()),
        tool_name: Some("Bash".to_string()),
        tool_input: Some(json!({"command": "cargo test"})),
        prompt: None,
        last_assistant_message: None,
        permission_suggestions: None,
    }
}

#[test]
fn request_line_contains_one_json_envelope() {
    let envelope = BridgeRequestEnvelope::process_agent_hook("req-1".to_string(), payload());

    let line = encode_request_line(&envelope).expect("request should encode");
    let text = String::from_utf8(line).expect("request should be utf8");

    assert!(text.ends_with('\n'));
    assert_eq!(text.lines().count(), 1);
    assert!(text.contains("\"schema_version\":1"));
    assert!(text.contains("\"command_type\":\"process_agent_hook\""));
    assert!(text.contains("\"request_id\":\"req-1\""));
}

#[test]
fn decoder_waits_for_complete_line() {
    let envelope = BridgeRequestEnvelope::process_agent_hook("req-1".to_string(), payload());
    let line = encode_request_line(&envelope).expect("request should encode");
    let split_at = line.len() - 1;
    let mut decoder = BridgeRequestDecoder::new();

    let first = decoder
        .push_bytes(&line[..split_at])
        .expect("partial bytes should not fail");
    let second = decoder
        .push_bytes(&line[split_at..])
        .expect("complete line should decode");

    assert!(first.is_empty());
    assert_eq!(second, vec![envelope]);
}

#[test]
fn decoder_parses_multiple_lines_and_skips_empty_lines() {
    let first = BridgeRequestEnvelope::process_agent_hook("req-1".to_string(), payload());
    let second = BridgeRequestEnvelope::process_agent_hook("req-2".to_string(), payload());
    let mut bytes = Vec::new();
    bytes.extend(encode_request_line(&first).expect("first should encode"));
    bytes.push(b'\n');
    bytes.extend(encode_request_line(&second).expect("second should encode"));
    let mut decoder = BridgeRequestDecoder::new();

    let decoded = decoder.push_bytes(&bytes).expect("lines should decode");

    assert_eq!(decoded, vec![first, second]);
}

#[test]
fn malformed_json_is_rejected() {
    let mut decoder = BridgeRequestDecoder::new();

    let result = decoder.push_bytes(b"{not-json}\n");

    assert!(result.is_err());
}

#[test]
fn response_schema_matches_request_schema() {
    let response = BridgeResponseEnvelope::directive(
        "req-1".to_string(),
        BridgeDirectivePayload::allow(AgentKind::CodexCli),
    );
    let line = encode_response_line(&response).expect("response should encode");
    let mut decoder = BridgeResponseDecoder::new();

    let decoded = decoder.push_bytes(&line).expect("response should decode");

    assert_eq!(decoded, vec![response]);
    assert_eq!(decoded[0].schema_version, BRIDGE_SCHEMA_VERSION);
    assert_eq!(decoded[0].request_id, "req-1");
    assert_eq!(decoded[0].result_type, BridgeResultType::Directive);
}

#[test]
fn request_decodes_command_type_as_explicit_enum() {
    let envelope = BridgeRequestEnvelope::process_agent_hook("req-1".to_string(), payload());
    let line = encode_request_line(&envelope).expect("request should encode");
    let mut decoder = BridgeRequestDecoder::new();

    let decoded = decoder.push_bytes(&line).expect("request should decode");

    assert_eq!(decoded[0].command_type, BridgeCommandType::ProcessAgentHook);
}
