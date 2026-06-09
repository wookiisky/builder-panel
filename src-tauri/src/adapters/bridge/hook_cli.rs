//! builder-panel-hook CLI 运行逻辑。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::adapters::bridge::codec::{BridgeRequestEnvelope, BridgeResponseEnvelope};
use crate::adapters::bridge::hook_output::{standard_output_for_response, HookOutputError};
use crate::adapters::bridge::hook_payload::{
    is_codex_app_terminal, validate_hook_payload, HookSource,
};
use crate::adapters::bridge::transport::{send_bridge_request, BridgeTransportError};
use crate::domain::agent_session::AgentKind;

const NON_BLOCKING_HOOK_TIMEOUT: Duration = Duration::from_secs(45);
const CODEX_PERMISSION_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const CLAUDE_PERMISSION_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

/// hook CLI 运行结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookCliRun {
    /// 进程退出码。
    pub exit_code: i32,
    /// stdout 内容。
    pub stdout: Vec<u8>,
    /// stderr 诊断内容。
    pub stderr: Vec<u8>,
}

impl HookCliRun {
    /// 创建 fail-open 空结果。
    pub fn fail_open(message: Option<String>) -> Self {
        let stderr = message
            .map(|text| format!("[builder-panel-hook] {text}\n").into_bytes())
            .unwrap_or_default();

        Self {
            exit_code: 0,
            stdout: Vec::new(),
            stderr,
        }
    }
}

/// 运行 hook CLI。
pub fn run_hook_cli(arguments: &[String], stdin: &[u8]) -> HookCliRun {
    run_hook_cli_with_sender_and_env(
        arguments,
        stdin,
        |request, timeout| send_bridge_request(request, timeout),
        |key| std::env::var(key).ok(),
    )
}

/// 使用注入 sender 运行 hook CLI,供测试覆盖 fail-open 和 directive。
pub fn run_hook_cli_with_sender<F>(arguments: &[String], stdin: &[u8], sender: F) -> HookCliRun
where
    F: FnMut(
        &BridgeRequestEnvelope,
        Duration,
    ) -> Result<Option<BridgeResponseEnvelope>, BridgeTransportError>,
{
    run_hook_cli_with_sender_and_env(arguments, stdin, sender, |key| std::env::var(key).ok())
}

/// 使用注入 sender + env reader 运行 hook CLI,供测试覆盖 terminal_app 兜底。
pub fn run_hook_cli_with_sender_and_env<F, E>(
    arguments: &[String],
    stdin: &[u8],
    mut sender: F,
    mut env_reader: E,
) -> HookCliRun
where
    F: FnMut(
        &BridgeRequestEnvelope,
        Duration,
    ) -> Result<Option<BridgeResponseEnvelope>, BridgeTransportError>,
    E: FnMut(&str) -> Option<String>,
{
    if stdin.is_empty() {
        return HookCliRun::fail_open(None);
    }

    let Some(source) = parse_source(arguments) else {
        return HookCliRun::fail_open(Some("缺少或不支持 --source 参数".to_string()));
    };

    let mut payload = match validate_hook_payload(source, stdin) {
        Ok(payload) => payload,
        Err(error) => {
            return HookCliRun::fail_open(Some(format!("hook payload 校验失败：{error:?}")));
        }
    };
    apply_codex_app_env_fallback(source, &mut payload, &mut env_reader);
    let timeout = timeout_for(source, &payload.hook_event_name);
    let request = BridgeRequestEnvelope::process_agent_hook(generate_request_id(), payload);
    let response = match sender(&request, timeout) {
        Ok(Some(response)) => response,
        Ok(None) => return HookCliRun::fail_open(None),
        Err(error) => {
            return HookCliRun::fail_open(Some(format!("bridge 不可用：{error:?}")));
        }
    };

    if response.request_id != request.request_id {
        return HookCliRun::fail_open(Some("bridge response request_id 不匹配".to_string()));
    }

    let expected_agent_kind = &request.payload.validated_payload.agent_kind;
    let stdout = match standard_output_for_response(&response, expected_agent_kind) {
        Ok(Some(stdout)) => stdout,
        Ok(None) => Vec::new(),
        Err(error) => return fail_open_for_output_error(error),
    };

    HookCliRun {
        exit_code: 0,
        stdout,
        stderr: Vec::new(),
    }
}

/// codex 客户端未上报 `terminal_app` 时,用 env 兜底判定 Codex.app。
///
/// 仅在 source=Codex 且 payload 仍是 CodexCli 时尝试,逐个读取已知能标记
/// Codex.app 的环境变量。命中则覆写 agent_kind + terminal_app。
fn apply_codex_app_env_fallback<E>(
    source: HookSource,
    payload: &mut crate::adapters::bridge::codec::ValidatedHookPayload,
    env_reader: &mut E,
) where
    E: FnMut(&str) -> Option<String>,
{
    if source != HookSource::Codex || payload.agent_kind != AgentKind::CodexCli {
        return;
    }

    for key in CODEX_APP_ENV_HINTS {
        let Some(value) = env_reader(key) else {
            continue;
        };
        if is_codex_app_terminal(&value) {
            payload.agent_kind = AgentKind::CodexApp;
            if payload.terminal_app.is_none() {
                payload.terminal_app = Some(value);
            }
            return;
        }
    }
}

/// 用于兜底识别 Codex.app 的环境变量,按优先级排列。
const CODEX_APP_ENV_HINTS: &[&str] = &[
    "BUILDER_PANEL_HOOK_TERMINAL_APP",
    "__CFBundleIdentifier",
    "TERM_PROGRAM",
];

fn parse_source(arguments: &[String]) -> Option<HookSource> {
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == "--source" {
            return arguments
                .get(index + 1)
                .and_then(|source| HookSource::parse(source));
        }

        index += 1;
    }

    None
}

fn timeout_for(
    source: HookSource,
    event_name: &crate::adapters::bridge::codec::BridgeHookEventName,
) -> Duration {
    if *event_name != crate::adapters::bridge::codec::BridgeHookEventName::PermissionRequest {
        return NON_BLOCKING_HOOK_TIMEOUT;
    }

    match source {
        HookSource::Codex => CODEX_PERMISSION_TIMEOUT,
        HookSource::Claude => CLAUDE_PERMISSION_TIMEOUT,
    }
}

fn generate_request_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("hook-{}-{millis}", std::process::id())
}

fn fail_open_for_output_error(error: HookOutputError) -> HookCliRun {
    HookCliRun::fail_open(Some(format!("directive 编码失败：{error:?}")))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::{run_hook_cli_with_sender, CLAUDE_PERMISSION_TIMEOUT, CODEX_PERMISSION_TIMEOUT};
    use crate::adapters::bridge::codec::{
        BridgeDirectivePayload, BridgeHookEventName, BridgeRequestEnvelope, BridgeResponseEnvelope,
    };
    use crate::domain::agent_session::AgentKind;

    fn codex_permission_input() -> Vec<u8> {
        json!({
            "cwd": "/tmp/project",
            "hook_event_name": "PermissionRequest",
            "session_id": "session-1",
            "model": "gpt-5-codex",
            "permission_mode": "default",
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test"}
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn empty_stdin_exits_without_stdout() {
        let run = run_hook_cli_with_sender(&["--source".into(), "codex".into()], b"", |_, _| {
            panic!("sender should not be called")
        });

        assert_eq!(run.exit_code, 0);
        assert!(run.stdout.is_empty());
    }

    #[test]
    fn malformed_json_fails_open_without_stdout() {
        let run = run_hook_cli_with_sender(
            &["--source".into(), "codex".into()],
            b"{not-json}",
            |_, _| panic!("sender should not be called"),
        );

        assert_eq!(run.exit_code, 0);
        assert!(run.stdout.is_empty());
        assert!(!run.stderr.is_empty());
    }

    #[test]
    fn bridge_unavailable_fails_open_without_stdout() {
        let run = run_hook_cli_with_sender(
            &["--source".into(), "codex".into()],
            &codex_permission_input(),
            |_, _| Err(crate::adapters::bridge::transport::BridgeTransportError::BridgeUnavailable),
        );

        assert_eq!(run.exit_code, 0);
        assert!(run.stdout.is_empty());
    }

    #[test]
    fn codex_permission_request_waits_with_interactive_timeout() {
        let run = super::run_hook_cli_with_sender_and_env(
            &["--source".into(), "codex".into()],
            &codex_permission_input(),
            |request: &BridgeRequestEnvelope, timeout: Duration| {
                assert_eq!(timeout, CODEX_PERMISSION_TIMEOUT);
                assert_eq!(
                    request.payload.validated_payload.hook_event_name,
                    BridgeHookEventName::PermissionRequest
                );
                Ok(Some(BridgeResponseEnvelope::directive(
                    request.request_id.clone(),
                    BridgeDirectivePayload::allow(AgentKind::CodexCli),
                )))
            },
            |_| None,
        );

        let stdout = String::from_utf8(run.stdout).expect("stdout should be utf8");
        assert!(stdout.contains("\"hookEventName\":\"PermissionRequest\""));
        assert!(stdout.contains("\"behavior\":\"allow\""));
    }

    #[test]
    fn claude_permission_request_waits_with_interactive_timeout() {
        let input = json!({
            "cwd": "/tmp/project",
            "hook_event_name": "PermissionRequest",
            "session_id": "session-1",
            "permission_mode": "default",
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test"}
        })
        .to_string()
        .into_bytes();

        let run = run_hook_cli_with_sender(
            &["--source".into(), "claude".into()],
            &input,
            |request: &BridgeRequestEnvelope, timeout: Duration| {
                assert_eq!(timeout, CLAUDE_PERMISSION_TIMEOUT);
                Ok(Some(BridgeResponseEnvelope::directive(
                    request.request_id.clone(),
                    BridgeDirectivePayload::deny(
                        AgentKind::ClaudeCodeCli,
                        Some("用户拒绝审批".to_string()),
                        Some(false),
                    ),
                )))
            },
        );

        let stdout = String::from_utf8(run.stdout).expect("stdout should be utf8");
        assert!(stdout.contains("\"suppressOutput\":true"));
        assert!(stdout.contains("\"behavior\":\"deny\""));
    }

    #[test]
    fn mismatched_response_request_id_fails_open_without_stdout() {
        let run = run_hook_cli_with_sender(
            &["--source".into(), "codex".into()],
            &codex_permission_input(),
            |_request: &BridgeRequestEnvelope, _timeout: Duration| {
                Ok(Some(BridgeResponseEnvelope::directive(
                    "other-request".to_string(),
                    BridgeDirectivePayload::allow(AgentKind::CodexCli),
                )))
            },
        );

        assert_eq!(run.exit_code, 0);
        assert!(run.stdout.is_empty());
        assert!(String::from_utf8(run.stderr)
            .expect("stderr should be utf8")
            .contains("request_id"));
    }

    #[test]
    fn mismatched_response_agent_kind_fails_open_without_stdout() {
        let run = run_hook_cli_with_sender(
            &["--source".into(), "codex".into()],
            &codex_permission_input(),
            |request: &BridgeRequestEnvelope, _timeout: Duration| {
                Ok(Some(BridgeResponseEnvelope::directive(
                    request.request_id.clone(),
                    BridgeDirectivePayload::allow(AgentKind::ClaudeCodeCli),
                )))
            },
        );

        assert_eq!(run.exit_code, 0);
        assert!(run.stdout.is_empty());
        assert!(String::from_utf8(run.stderr)
            .expect("stderr should be utf8")
            .contains("AgentMismatch"));
    }

    #[test]
    fn codex_app_env_hint_promotes_agent_kind_to_codex_app() {
        let captured = std::cell::RefCell::new(None);
        let run = super::run_hook_cli_with_sender_and_env(
            &["--source".into(), "codex".into()],
            &codex_permission_input(),
            |request: &BridgeRequestEnvelope, _timeout: Duration| {
                *captured.borrow_mut() = Some(request.payload.validated_payload.agent_kind.clone());
                Ok(Some(BridgeResponseEnvelope::directive(
                    request.request_id.clone(),
                    BridgeDirectivePayload::allow(AgentKind::CodexApp),
                )))
            },
            |key| match key {
                "__CFBundleIdentifier" => Some("com.openai.codex".to_string()),
                _ => None,
            },
        );

        assert_eq!(run.exit_code, 0);
        assert_eq!(
            captured.into_inner(),
            Some(AgentKind::CodexApp),
            "bundle id 命中应把 agent_kind 改判为 CodexApp"
        );
    }

    #[test]
    fn codex_app_env_hint_via_term_program_promotes_agent_kind() {
        let captured = std::cell::RefCell::new(None);
        super::run_hook_cli_with_sender_and_env(
            &["--source".into(), "codex".into()],
            &codex_permission_input(),
            |request: &BridgeRequestEnvelope, _timeout: Duration| {
                *captured.borrow_mut() = Some(request.payload.validated_payload.agent_kind.clone());
                Ok(Some(BridgeResponseEnvelope::directive(
                    request.request_id.clone(),
                    BridgeDirectivePayload::allow(AgentKind::CodexApp),
                )))
            },
            |key| match key {
                "TERM_PROGRAM" => Some("Codex.app".to_string()),
                _ => None,
            },
        );

        assert_eq!(captured.into_inner(), Some(AgentKind::CodexApp));
    }

    #[test]
    fn codex_env_hint_unrelated_keeps_codex_cli() {
        let captured = std::cell::RefCell::new(None);
        super::run_hook_cli_with_sender_and_env(
            &["--source".into(), "codex".into()],
            &codex_permission_input(),
            |request: &BridgeRequestEnvelope, _timeout: Duration| {
                *captured.borrow_mut() = Some(request.payload.validated_payload.agent_kind.clone());
                Ok(Some(BridgeResponseEnvelope::directive(
                    request.request_id.clone(),
                    BridgeDirectivePayload::allow(AgentKind::CodexCli),
                )))
            },
            |key| match key {
                "TERM_PROGRAM" => Some("iTerm.app".to_string()),
                _ => None,
            },
        );

        assert_eq!(captured.into_inner(), Some(AgentKind::CodexCli));
    }

    #[test]
    fn claude_source_ignores_codex_app_env_hint() {
        let captured = std::cell::RefCell::new(None);
        let input = json!({
            "cwd": "/tmp/project",
            "hook_event_name": "PermissionRequest",
            "session_id": "session-1",
            "permission_mode": "default",
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test"}
        })
        .to_string()
        .into_bytes();

        super::run_hook_cli_with_sender_and_env(
            &["--source".into(), "claude".into()],
            &input,
            |request: &BridgeRequestEnvelope, _timeout: Duration| {
                *captured.borrow_mut() = Some(request.payload.validated_payload.agent_kind.clone());
                Ok(Some(BridgeResponseEnvelope::directive(
                    request.request_id.clone(),
                    BridgeDirectivePayload::allow(AgentKind::ClaudeCodeCli),
                )))
            },
            |key| match key {
                "TERM_PROGRAM" => Some("Codex.app".to_string()),
                _ => None,
            },
        );

        assert_eq!(captured.into_inner(), Some(AgentKind::ClaudeCodeCli));
    }
}
