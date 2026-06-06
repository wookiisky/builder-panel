//! 本地 bridge NDJSON 协议编解码。

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::agent_session::AgentKind;

/// 当前本地 bridge 协议版本。
pub const BRIDGE_SCHEMA_VERSION: u16 = 1;

const NEWLINE: u8 = b'\n';

/// bridge codec 错误。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BridgeCodecError {
    /// JSON envelope 格式错误。
    MalformedEnvelope,
    /// envelope schema_version 不受支持。
    UnsupportedSchemaVersion(u16),
}

/// bridge command 类型。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeCommandType {
    /// 处理 agent hook payload。
    ProcessAgentHook,
}

/// bridge response 结果类型。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeResultType {
    /// 请求已确认。
    Ack,
    /// 返回 hook stdout directive。
    Directive,
    /// 请求处理失败。
    Error,
}

/// hook 事件名。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BridgeHookEventName {
    /// 会话启动。
    SessionStart,
    /// 用户提交 prompt。
    UserPromptSubmit,
    /// 工具执行前。
    PreToolUse,
    /// 权限请求。
    PermissionRequest,
    /// 工具执行后。
    PostToolUse,
    /// 通知事件。
    Notification,
    /// turn 停止。
    Stop,
    /// 会话结束。
    SessionEnd,
}

/// hook directive 类型。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeDirectiveKind {
    /// 允许权限请求。
    Allow,
    /// 拒绝权限请求。
    Deny,
    /// Claude PreToolUse 权限决定。
    ToolPermission,
}

/// Claude PreToolUse 权限决策。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeToolPermissionDecision {
    /// 允许工具调用继续。
    Allow,
    /// 拒绝工具调用。
    Deny,
    /// 交还 agent 继续询问用户。
    Ask,
}

/// bridge 错误码。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum BridgeErrorCode {
    /// 本地 bridge 不可用。
    BridgeUnavailable,
    /// Agent payload 无法清洗。
    MalformedAgentPayload,
    /// Agent 协议暂不支持。
    AgentProtocolUnsupported,
    /// directive 编码失败。
    DirectiveEncodeFailed,
}

/// 已清洗 hook payload。
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ValidatedHookPayload {
    /// Agent 来源。
    pub agent_kind: AgentKind,
    /// hook 事件名。
    pub hook_event_name: BridgeHookEventName,
    /// 当前工作目录。
    pub cwd: String,
    /// agent 会话 ID。
    pub session_id: String,
    /// 可选模型名。
    pub model: Option<String>,
    /// 可选权限模式。
    pub permission_mode: Option<String>,
    /// 可选 transcript 路径。
    pub transcript_path: Option<String>,
    /// 可选终端或 APP 名称。
    pub terminal_app: Option<String>,
    /// 可选终端 session ID。
    pub terminal_session_id: Option<String>,
    /// 可选终端 TTY。
    pub terminal_tty: Option<String>,
    /// 可选终端标题。
    pub terminal_title: Option<String>,
    /// 可选 turn ID。
    pub turn_id: Option<String>,
    /// 可选 tool 名。
    pub tool_name: Option<String>,
    /// 已验证为 JSON 对象的 tool input。
    pub tool_input: Option<Value>,
    /// 可选用户 prompt。
    pub prompt: Option<String>,
    /// 可选最后 assistant 消息。
    pub last_assistant_message: Option<String>,
    /// 已验证为 JSON 数组的权限建议。
    pub permission_suggestions: Option<Value>,
}

/// bridge request payload。
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BridgeRequestPayload {
    /// 已清洗 hook payload。
    pub validated_payload: ValidatedHookPayload,
}

/// bridge request envelope。
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BridgeRequestEnvelope {
    /// 协议版本。
    pub schema_version: u16,
    /// command 类型。
    pub command_type: BridgeCommandType,
    /// 请求 ID。
    pub request_id: String,
    /// 请求 payload。
    pub payload: BridgeRequestPayload,
}

impl BridgeRequestEnvelope {
    /// 创建处理 hook 的 request envelope。
    pub fn process_agent_hook(request_id: String, payload: ValidatedHookPayload) -> Self {
        Self {
            schema_version: BRIDGE_SCHEMA_VERSION,
            command_type: BridgeCommandType::ProcessAgentHook,
            request_id,
            payload: BridgeRequestPayload {
                validated_payload: payload,
            },
        }
    }
}

/// hook directive payload。
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BridgeDirectivePayload {
    /// directive 类型。
    pub directive_kind: BridgeDirectiveKind,
    /// directive 目标 agent。
    pub agent_kind: AgentKind,
    /// 可选原因。
    pub reason: Option<String>,
    /// 可选输入改写。
    pub updated_input: Option<Value>,
    /// 可选权限更新。
    pub updated_permissions: Option<Value>,
    /// 可选工具权限决策。
    pub tool_permission_decision: Option<BridgeToolPermissionDecision>,
    /// 可选附加上下文。
    pub additional_context: Option<String>,
    /// 是否中断当前 turn。
    pub interrupt: Option<bool>,
}

impl BridgeDirectivePayload {
    /// 创建 allow directive。
    pub fn allow(agent_kind: AgentKind) -> Self {
        Self {
            directive_kind: BridgeDirectiveKind::Allow,
            agent_kind,
            reason: None,
            updated_input: None,
            updated_permissions: None,
            tool_permission_decision: None,
            additional_context: None,
            interrupt: None,
        }
    }

    /// 创建 deny directive。
    pub fn deny(agent_kind: AgentKind, reason: Option<String>, interrupt: Option<bool>) -> Self {
        Self {
            directive_kind: BridgeDirectiveKind::Deny,
            agent_kind,
            reason,
            updated_input: None,
            updated_permissions: None,
            tool_permission_decision: None,
            additional_context: None,
            interrupt,
        }
    }

    /// 创建 Claude PreToolUse 工具权限 directive。
    pub fn tool_permission(
        agent_kind: AgentKind,
        decision: BridgeToolPermissionDecision,
        reason: Option<String>,
        updated_input: Option<Value>,
        additional_context: Option<String>,
    ) -> Self {
        Self {
            directive_kind: BridgeDirectiveKind::ToolPermission,
            agent_kind,
            reason,
            updated_input,
            updated_permissions: None,
            tool_permission_decision: Some(decision),
            additional_context,
            interrupt: None,
        }
    }
}

/// bridge 错误 payload。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BridgeErrorPayload {
    /// 错误码。
    pub code: BridgeErrorCode,
    /// 错误消息。
    pub message: String,
}

/// bridge response envelope。
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BridgeResponseEnvelope {
    /// 协议版本。
    pub schema_version: u16,
    /// 请求 ID。
    pub request_id: String,
    /// 结果类型。
    pub result_type: BridgeResultType,
    /// 可选 response payload。
    pub payload: Option<BridgeDirectivePayload>,
    /// 可选错误 payload。
    pub error: Option<BridgeErrorPayload>,
}

impl BridgeResponseEnvelope {
    /// 创建 ack response。
    pub fn ack(request_id: String) -> Self {
        Self {
            schema_version: BRIDGE_SCHEMA_VERSION,
            request_id,
            result_type: BridgeResultType::Ack,
            payload: None,
            error: None,
        }
    }

    /// 创建 directive response。
    pub fn directive(request_id: String, payload: BridgeDirectivePayload) -> Self {
        Self {
            schema_version: BRIDGE_SCHEMA_VERSION,
            request_id,
            result_type: BridgeResultType::Directive,
            payload: Some(payload),
            error: None,
        }
    }

    /// 创建 error response。
    pub fn error(request_id: String, error: BridgeErrorPayload) -> Self {
        Self {
            schema_version: BRIDGE_SCHEMA_VERSION,
            request_id,
            result_type: BridgeResultType::Error,
            payload: None,
            error: Some(error),
        }
    }
}

/// NDJSON request encoder。
pub fn encode_request_line(envelope: &BridgeRequestEnvelope) -> Result<Vec<u8>, BridgeCodecError> {
    encode_line(envelope)
}

/// NDJSON response encoder。
pub fn encode_response_line(
    envelope: &BridgeResponseEnvelope,
) -> Result<Vec<u8>, BridgeCodecError> {
    encode_line(envelope)
}

fn encode_line<T: Serialize>(envelope: &T) -> Result<Vec<u8>, BridgeCodecError> {
    let mut line = serde_json::to_vec(envelope).map_err(|_| BridgeCodecError::MalformedEnvelope)?;
    line.push(NEWLINE);
    Ok(line)
}

/// NDJSON request decoder。
#[derive(Default)]
pub struct BridgeRequestDecoder {
    /// 尚未形成完整行的 buffer。
    buffer: Vec<u8>,
}

impl BridgeRequestDecoder {
    /// 创建 request decoder。
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    /// 推入字节并返回已形成的 request envelope。
    pub fn push_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<Vec<BridgeRequestEnvelope>, BridgeCodecError> {
        decode_lines(&mut self.buffer, bytes)
    }
}

/// NDJSON response decoder。
#[derive(Default)]
pub struct BridgeResponseDecoder {
    /// 尚未形成完整行的 buffer。
    buffer: Vec<u8>,
}

impl BridgeResponseDecoder {
    /// 创建 response decoder。
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    /// 推入字节并返回已形成的 response envelope。
    pub fn push_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<Vec<BridgeResponseEnvelope>, BridgeCodecError> {
        decode_lines(&mut self.buffer, bytes)
    }
}

fn decode_lines<T>(buffer: &mut Vec<u8>, bytes: &[u8]) -> Result<Vec<T>, BridgeCodecError>
where
    T: for<'de> Deserialize<'de> + VersionedEnvelope,
{
    buffer.extend_from_slice(bytes);
    let mut envelopes = Vec::new();

    while let Some(newline_index) = buffer.iter().position(|byte| *byte == NEWLINE) {
        let mut line = buffer.drain(..=newline_index).collect::<Vec<u8>>();
        line.pop();

        if line.is_empty() {
            continue;
        }

        let envelope: T =
            serde_json::from_slice(&line).map_err(|_| BridgeCodecError::MalformedEnvelope)?;
        envelope.ensure_supported_version()?;
        envelopes.push(envelope);
    }

    Ok(envelopes)
}

trait VersionedEnvelope {
    /// 校验 schema_version。
    fn ensure_supported_version(&self) -> Result<(), BridgeCodecError>;
}

impl VersionedEnvelope for BridgeRequestEnvelope {
    fn ensure_supported_version(&self) -> Result<(), BridgeCodecError> {
        ensure_supported_schema(self.schema_version)
    }
}

impl VersionedEnvelope for BridgeResponseEnvelope {
    fn ensure_supported_version(&self) -> Result<(), BridgeCodecError> {
        ensure_supported_schema(self.schema_version)
    }
}

fn ensure_supported_schema(schema_version: u16) -> Result<(), BridgeCodecError> {
    if schema_version == BRIDGE_SCHEMA_VERSION {
        return Ok(());
    }

    Err(BridgeCodecError::UnsupportedSchemaVersion(schema_version))
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
