//! hook stdout directive 编码。

use serde::Serialize;
use serde_json::Value;

use crate::adapters::bridge::codec::{
    BridgeDirectiveKind, BridgeDirectivePayload, BridgeResponseEnvelope, BridgeResultType,
    BridgeToolPermissionDecision,
};
use crate::domain::agent_session::AgentKind;

/// hook stdout 编码错误。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HookOutputError {
    /// response 与目标 agent 不匹配。
    AgentMismatch,
    /// response payload 缺失。
    MissingPayload,
    /// Claude PreToolUse directive 缺少显式权限决策。
    MissingToolPermissionDecision,
    /// directive 类型不支持。
    UnsupportedDirective,
    /// JSON 编码失败。
    EncodeFailed,
}

/// 按 agent hook 协议编码 stdout directive。
pub fn standard_output_for_response(
    response: &BridgeResponseEnvelope,
    expected_agent_kind: &AgentKind,
) -> Result<Option<Vec<u8>>, HookOutputError> {
    if response.result_type == BridgeResultType::Ack {
        return Ok(None);
    }

    if response.result_type == BridgeResultType::Error {
        return Ok(None);
    }

    let payload = response
        .payload
        .as_ref()
        .ok_or(HookOutputError::MissingPayload)?;

    if &payload.agent_kind != expected_agent_kind {
        return Err(HookOutputError::AgentMismatch);
    }

    match payload.agent_kind {
        AgentKind::CodexCli => encode_codex_output(payload),
        AgentKind::ClaudeCodeCli => encode_claude_output(payload),
        AgentKind::CodexApp | AgentKind::ClaudeCodeApp => Err(HookOutputError::AgentMismatch),
    }
}

fn encode_codex_output(
    payload: &BridgeDirectivePayload,
) -> Result<Option<Vec<u8>>, HookOutputError> {
    match payload.directive_kind {
        BridgeDirectiveKind::Allow => {
            let output = CodexPermissionRequestOutput {
                continue_processing: true,
                hook_specific_output: CodexPermissionHookOutput {
                    hook_event_name: "PermissionRequest",
                    decision: PermissionRequestDecision {
                        behavior: "allow",
                        message: None,
                        interrupt: None,
                        updated_input: None,
                        updated_permissions: None,
                    },
                },
            };
            encode_json_line(&output).map(Some)
        }
        BridgeDirectiveKind::Deny => {
            let output = CodexPermissionRequestOutput {
                continue_processing: true,
                hook_specific_output: CodexPermissionHookOutput {
                    hook_event_name: "PermissionRequest",
                    decision: PermissionRequestDecision {
                        behavior: "deny",
                        message: payload.reason.as_deref(),
                        interrupt: None,
                        updated_input: None,
                        updated_permissions: None,
                    },
                },
            };
            encode_json_line(&output).map(Some)
        }
        BridgeDirectiveKind::ToolPermission => Err(HookOutputError::UnsupportedDirective),
    }
}

fn encode_claude_output(
    payload: &BridgeDirectivePayload,
) -> Result<Option<Vec<u8>>, HookOutputError> {
    match payload.directive_kind {
        BridgeDirectiveKind::Allow => {
            let output = ClaudePermissionRequestOutput {
                continue_processing: true,
                suppress_output: true,
                hook_specific_output: ClaudePermissionHookOutput {
                    hook_event_name: "PermissionRequest",
                    decision: PermissionRequestDecision {
                        behavior: "allow",
                        message: None,
                        interrupt: None,
                        updated_input: payload.updated_input.as_ref(),
                        updated_permissions: payload.updated_permissions.as_ref(),
                    },
                },
            };
            encode_json_line(&output).map(Some)
        }
        BridgeDirectiveKind::Deny => {
            let output = ClaudePermissionRequestOutput {
                continue_processing: true,
                suppress_output: true,
                hook_specific_output: ClaudePermissionHookOutput {
                    hook_event_name: "PermissionRequest",
                    decision: PermissionRequestDecision {
                        behavior: "deny",
                        message: payload.reason.as_deref(),
                        interrupt: payload.interrupt,
                        updated_input: None,
                        updated_permissions: None,
                    },
                },
            };
            encode_json_line(&output).map(Some)
        }
        BridgeDirectiveKind::ToolPermission => {
            let permission_decision = payload
                .tool_permission_decision
                .as_ref()
                .ok_or(HookOutputError::MissingToolPermissionDecision)?;
            let output = ClaudePreToolUseOutput {
                continue_processing: true,
                suppress_output: true,
                hook_specific_output: ClaudePreToolUseHookOutput {
                    hook_event_name: "PreToolUse",
                    permission_decision: Some(permission_decision.as_stdout_value()),
                    permission_decision_reason: payload.reason.as_deref(),
                    updated_input: payload.updated_input.as_ref(),
                    additional_context: payload.additional_context.as_deref(),
                },
            };
            encode_json_line(&output).map(Some)
        }
    }
}

impl BridgeToolPermissionDecision {
    /// 转换为 Claude stdout 字段值。
    fn as_stdout_value(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Ask => "ask",
        }
    }
}

fn encode_json_line<T: Serialize>(value: &T) -> Result<Vec<u8>, HookOutputError> {
    let mut line = serde_json::to_vec(value).map_err(|_| HookOutputError::EncodeFailed)?;
    line.push(b'\n');
    Ok(line)
}

#[derive(Serialize)]
struct CodexPermissionRequestOutput<'a> {
    #[serde(rename = "continue")]
    continue_processing: bool,
    #[serde(rename = "hookSpecificOutput")]
    hook_specific_output: CodexPermissionHookOutput<'a>,
}

#[derive(Serialize)]
struct CodexPermissionHookOutput<'a> {
    #[serde(rename = "hookEventName")]
    hook_event_name: &'a str,
    decision: PermissionRequestDecision<'a>,
}

#[derive(Serialize)]
struct ClaudePermissionRequestOutput<'a> {
    #[serde(rename = "continue")]
    continue_processing: bool,
    #[serde(rename = "suppressOutput")]
    suppress_output: bool,
    #[serde(rename = "hookSpecificOutput")]
    hook_specific_output: ClaudePermissionHookOutput<'a>,
}

#[derive(Serialize)]
struct ClaudePermissionHookOutput<'a> {
    #[serde(rename = "hookEventName")]
    hook_event_name: &'a str,
    decision: PermissionRequestDecision<'a>,
}

#[derive(Serialize)]
struct ClaudePreToolUseOutput<'a> {
    #[serde(rename = "continue")]
    continue_processing: bool,
    #[serde(rename = "suppressOutput")]
    suppress_output: bool,
    #[serde(rename = "hookSpecificOutput")]
    hook_specific_output: ClaudePreToolUseHookOutput<'a>,
}

#[derive(Serialize)]
struct ClaudePreToolUseHookOutput<'a> {
    #[serde(rename = "hookEventName")]
    hook_event_name: &'a str,
    #[serde(rename = "permissionDecision", skip_serializing_if = "Option::is_none")]
    permission_decision: Option<&'a str>,
    #[serde(
        rename = "permissionDecisionReason",
        skip_serializing_if = "Option::is_none"
    )]
    permission_decision_reason: Option<&'a str>,
    #[serde(rename = "updatedInput", skip_serializing_if = "Option::is_none")]
    updated_input: Option<&'a Value>,
    #[serde(rename = "additionalContext", skip_serializing_if = "Option::is_none")]
    additional_context: Option<&'a str>,
}

#[derive(Serialize)]
struct PermissionRequestDecision<'a> {
    behavior: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interrupt: Option<bool>,
    #[serde(rename = "updatedInput", skip_serializing_if = "Option::is_none")]
    updated_input: Option<&'a Value>,
    #[serde(rename = "updatedPermissions", skip_serializing_if = "Option::is_none")]
    updated_permissions: Option<&'a Value>,
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::standard_output_for_response;
    use crate::adapters::bridge::codec::{
        BridgeDirectivePayload, BridgeResponseEnvelope, BridgeToolPermissionDecision,
    };
    use crate::domain::agent_session::AgentKind;

    fn object_from_output(bytes: Vec<u8>) -> Value {
        assert!(bytes.ends_with(b"\n"));
        serde_json::from_slice(&bytes).expect("stdout should be json")
    }

    #[test]
    fn ack_response_has_no_stdout() {
        let response = BridgeResponseEnvelope::ack("req-1".to_string());

        let output = standard_output_for_response(&response, &AgentKind::CodexCli)
            .expect("ack should encode");

        assert!(output.is_none());
    }

    #[test]
    fn codex_permission_allow_output_matches_hook_contract() {
        let response = BridgeResponseEnvelope::directive(
            "req-1".to_string(),
            BridgeDirectivePayload::allow(AgentKind::CodexCli),
        );

        let output = standard_output_for_response(&response, &AgentKind::CodexCli)
            .expect("allow should encode")
            .expect("allow should produce stdout");
        let object = object_from_output(output);

        assert_eq!(object["continue"], true);
        assert_eq!(
            object["hookSpecificOutput"]["hookEventName"],
            "PermissionRequest"
        );
        assert_eq!(
            object["hookSpecificOutput"]["decision"]["behavior"],
            "allow"
        );
    }

    #[test]
    fn codex_permission_deny_output_carries_message() {
        let response = BridgeResponseEnvelope::directive(
            "req-1".to_string(),
            BridgeDirectivePayload::deny(
                AgentKind::CodexCli,
                Some("用户拒绝审批".to_string()),
                None,
            ),
        );

        let output = standard_output_for_response(&response, &AgentKind::CodexCli)
            .expect("deny should encode")
            .expect("deny should produce stdout");
        let object = object_from_output(output);

        assert_eq!(object["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert_eq!(
            object["hookSpecificOutput"]["decision"]["message"],
            "用户拒绝审批"
        );
    }

    #[test]
    fn claude_permission_deny_output_suppresses_stdout_and_can_interrupt() {
        let response = BridgeResponseEnvelope::directive(
            "req-1".to_string(),
            BridgeDirectivePayload::deny(
                AgentKind::ClaudeCodeCli,
                Some("Permission denied in Builder Panel.".to_string()),
                Some(true),
            ),
        );

        let output = standard_output_for_response(&response, &AgentKind::ClaudeCodeCli)
            .expect("deny should encode")
            .expect("deny should produce stdout");
        let object = object_from_output(output);

        assert_eq!(object["suppressOutput"], true);
        assert_eq!(
            object["hookSpecificOutput"]["hookEventName"],
            "PermissionRequest"
        );
        assert_eq!(object["hookSpecificOutput"]["decision"]["behavior"], "deny");
        assert_eq!(object["hookSpecificOutput"]["decision"]["interrupt"], true);
    }

    #[test]
    fn response_agent_mismatch_fails_open() {
        let response = BridgeResponseEnvelope::directive(
            "req-1".to_string(),
            BridgeDirectivePayload::allow(AgentKind::ClaudeCodeCli),
        );

        let result = standard_output_for_response(&response, &AgentKind::CodexCli);

        assert_eq!(result, Err(super::HookOutputError::AgentMismatch));
    }

    #[test]
    fn claude_pre_tool_use_allow_output_has_explicit_decision() {
        let response = BridgeResponseEnvelope::directive(
            "req-1".to_string(),
            BridgeDirectivePayload::tool_permission(
                AgentKind::ClaudeCodeCli,
                BridgeToolPermissionDecision::Allow,
                None,
                Some(serde_json::json!({"command": "cargo test"})),
                Some("继续执行测试".to_string()),
            ),
        );

        let output = standard_output_for_response(&response, &AgentKind::ClaudeCodeCli)
            .expect("pre tool output should encode")
            .expect("pre tool output should exist");
        let object = object_from_output(output);

        assert_eq!(object["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        assert_eq!(object["hookSpecificOutput"]["permissionDecision"], "allow");
        assert_eq!(
            object["hookSpecificOutput"]["updatedInput"]["command"],
            "cargo test"
        );
        assert_eq!(
            object["hookSpecificOutput"]["additionalContext"],
            "继续执行测试"
        );
    }

    #[test]
    fn claude_pre_tool_use_ask_output_has_explicit_decision() {
        let response = BridgeResponseEnvelope::directive(
            "req-1".to_string(),
            BridgeDirectivePayload::tool_permission(
                AgentKind::ClaudeCodeCli,
                BridgeToolPermissionDecision::Ask,
                Some("需要用户确认".to_string()),
                None,
                None,
            ),
        );

        let output = standard_output_for_response(&response, &AgentKind::ClaudeCodeCli)
            .expect("pre tool output should encode")
            .expect("pre tool output should exist");
        let object = object_from_output(output);

        assert_eq!(object["hookSpecificOutput"]["permissionDecision"], "ask");
        assert_eq!(
            object["hookSpecificOutput"]["permissionDecisionReason"],
            "需要用户确认"
        );
    }

    #[test]
    fn claude_pre_tool_use_deny_output_has_explicit_decision() {
        let response = BridgeResponseEnvelope::directive(
            "req-1".to_string(),
            BridgeDirectivePayload::tool_permission(
                AgentKind::ClaudeCodeCli,
                BridgeToolPermissionDecision::Deny,
                Some("命令不符合策略".to_string()),
                None,
                None,
            ),
        );

        let output = standard_output_for_response(&response, &AgentKind::ClaudeCodeCli)
            .expect("pre tool output should encode")
            .expect("pre tool output should exist");
        let object = object_from_output(output);

        assert_eq!(object["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(
            object["hookSpecificOutput"]["permissionDecisionReason"],
            "命令不符合策略"
        );
    }

    #[test]
    fn claude_pre_tool_use_missing_decision_fails_open() {
        let response = BridgeResponseEnvelope::directive(
            "req-1".to_string(),
            BridgeDirectivePayload {
                directive_kind: crate::adapters::bridge::codec::BridgeDirectiveKind::ToolPermission,
                agent_kind: AgentKind::ClaudeCodeCli,
                reason: None,
                updated_input: None,
                updated_permissions: None,
                tool_permission_decision: None,
                additional_context: None,
                interrupt: None,
            },
        );

        let result = standard_output_for_response(&response, &AgentKind::ClaudeCodeCli);

        assert_eq!(
            result,
            Err(super::HookOutputError::MissingToolPermissionDecision)
        );
    }
}
