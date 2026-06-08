//! Tauri command 入口。

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::adapters::codex_app::{
    CodexAppAdapter, CodexAppRuntime, CodexAppSchemaProbe, CodexAppServerClient,
    CodexRolloutDiscovery, CodexRolloutTailer, CodexRolloutWatchTarget,
};
use crate::adapters::codex_cli_hook::{start_codex_cli_bridge_server, CodexCliHookRuntime};
use crate::adapters::config_file::JsonSettingsStore;
use crate::adapters::hook_install::{
    HookInstallAgent, HookInstallManifest, HookInstallPaths, HookInstallPreview, HookInstallStatus,
    HookInstaller,
};
use crate::adapters::terminal::TerminalJumpAdapter;
use crate::domain::agent_interaction::InteractionId;
use crate::domain::agent_session::{JumpTarget, SessionKey};
use crate::domain::app_error::FallbackAction;
use crate::domain::panel_probe::PanelProbe;
use crate::domain::usage::UnixMillis;
use crate::domain::view_model::{SessionDetailViewModel, SessionListItemViewModel};
use crate::ports::agent_adapter_port::ApprovalDecision;
use crate::ports::agent_adapter_port::ChoiceSubmission;
use crate::ports::jump_target_port::JumpTargetPort;
use crate::ports::process_timeline_port::ProcessTimelineReleasePort;
use crate::ports::session_update_port::{NoopSessionUpdateSink, SessionUpdateSinkPort};
use crate::services::process_timeline_service::{
    ProcessTimelineService, TimelinePage, TimelineQuery,
};
use crate::services::settings_service::{
    BuilderPanelSettings, PanelWindowPosition, PanelWindowSize, SettingsService, SettingsViewModel,
};

/// 全局 Codex CLI runtime，用于阶段 4 真实 hook 闭环。
static CODEX_CLI_RUNTIME: OnceLock<Arc<Mutex<CodexCliHookRuntime>>> = OnceLock::new();
/// 全局 Codex APP runtime，用于 hook 和 app-server 闭环。
static CODEX_APP_RUNTIME: OnceLock<Arc<Mutex<CodexAppRuntime>>> = OnceLock::new();
/// 全局 Codex APP app-server 客户端槽。
static CODEX_APP_SERVER: OnceLock<Mutex<CodexAppServerSlot>> = OnceLock::new();
/// Codex APP app-server 启动失败退避状态。
static CODEX_APP_STARTUP_FAILURE: OnceLock<Mutex<Option<CodexAppStartupFailure>>> = OnceLock::new();
/// Codex CLI bridge server 启动标记。
static CODEX_CLI_BRIDGE_STARTED: OnceLock<Mutex<bool>> = OnceLock::new();
/// Codex rollout watcher 启动标记。
static CODEX_ROLLOUT_WATCHER_STARTED: OnceLock<Mutex<bool>> = OnceLock::new();
/// Codex rollout 最近一次全量扫描时间。
static CODEX_APP_ROLLOUT_LAST_SYNC: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
/// Codex APP thread 元数据最近一次同步时间。
static CODEX_APP_THREAD_METADATA_LAST_SYNC: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
/// Codex APP 后台上下文同步是否正在运行。
static CODEX_APP_CONTEXT_SYNC_IN_FLIGHT: OnceLock<Mutex<bool>> = OnceLock::new();
/// 全局 session 更新发布端口。
static SESSION_UPDATE_SINK: OnceLock<Arc<dyn SessionUpdateSinkPort>> = OnceLock::new();

const CODEX_APP_STARTUP_RETRY_DELAY: Duration = Duration::from_secs(30);
const CODEX_APP_ROLLOUT_SYNC_INTERVAL: Duration = Duration::from_secs(10);
const CODEX_APP_THREAD_METADATA_SYNC_INTERVAL: Duration = Duration::from_secs(5);
const CODEX_ROLLOUT_WATCH_INTERVAL: Duration = Duration::from_millis(300);

/// Codex APP app-server 启动失败退避记录。
struct CodexAppStartupFailure {
    /// 下次允许重试的时间。
    retry_at: Instant,
    /// 最近一次用户可读错误。
    message: String,
}

/// Codex APP app-server client 槽状态。
enum CodexAppServerSlot {
    /// 尚未连接。
    Empty,
    /// 正在启动或同步。
    Starting,
    /// 已连接 client。
    Ready(Arc<CodexAppServerClient>),
}

/// Codex APP app-server 启动准备结果。
enum CodexAppStartupAction {
    /// 已有可用 client。
    AlreadyRunning,
    /// 需要启动新 client。
    Start,
    /// 已有 client 退出。
    Exited,
}

/// 获取基础 panel 探针状态。
#[tauri::command]
pub fn get_panel_probe() -> PanelProbe {
    PanelProbe::expanded_default()
}

/// 读取 Builder Panel 设置。
#[tauri::command]
pub fn get_panel_settings() -> SettingsViewModel {
    let store = JsonSettingsStore::default_path();
    let service = SettingsService::new(&store);

    service.read_settings()
}

/// 保存 Builder Panel 设置。
#[tauri::command]
pub fn save_panel_settings(settings: BuilderPanelSettings) -> Result<SettingsViewModel, String> {
    let store = JsonSettingsStore::default_path();
    let service = SettingsService::new(&store);

    service
        .save_settings(settings)
        .map_err(|error| error.user_message)
}

/// 保存 panel 窗口局部状态。
#[tauri::command]
pub fn save_panel_window_state(update: PanelWindowStateUpdate) -> Result<(), String> {
    let store = JsonSettingsStore::default_path();
    let service = SettingsService::new(&store);
    let mut settings = service.read_settings().settings;

    settings.panel.collapsed = false;
    if let Some(position) = update.window_position {
        settings.panel.window_position = Some(position);
    }
    if let Some(size) = update.window_size {
        settings.panel.window_size = Some(size);
    }

    service
        .save_settings(settings)
        .map(|_| ())
        .map_err(|error| error.user_message)
}

/// 预览 hook 安装。
#[tauri::command]
pub fn preview_hook_install(request: HookInstallRequest) -> Result<HookInstallPreview, String> {
    let installer = default_hook_installer()?;

    Ok(installer.preview(&request.agents))
}

/// 查询 hook 安装状态。
#[tauri::command]
pub fn get_hook_install_status() -> Result<HookInstallStatus, String> {
    let installer = default_hook_installer()?;

    Ok(installer.status())
}

/// 安装 hook。
#[tauri::command]
pub fn install_hooks(request: HookInstallRequest) -> Result<HookInstallManifest, String> {
    let installer = default_hook_installer()?;

    installer
        .install(&request.agents)
        .map_err(|error| error.user_message)
}

/// 卸载 hook。
#[tauri::command]
pub fn uninstall_hooks(request: HookInstallRequest) -> Result<(), String> {
    let installer = default_hook_installer()?;

    installer
        .uninstall_agents(&request.agents)
        .map_err(|error| error.user_message)
}

/// 获取 Codex CLI session 列表。
#[tauri::command]
pub fn get_codex_cli_sessions() -> Result<Vec<SessionListItemViewModel>, String> {
    ensure_codex_cli_bridge_started()?;
    let runtime = lock_codex_cli_runtime()?;

    Ok(runtime.session_list())
}

/// 获取 Codex APP session 列表。
#[tauri::command]
pub fn get_codex_app_sessions() -> Result<Vec<SessionListItemViewModel>, String> {
    ensure_codex_cli_bridge_started()?;
    schedule_codex_app_context_sync();
    let runtime = lock_codex_app_runtime()?;

    Ok(runtime.session_list())
}

/// 探测本机 Codex APP app-server schema。
#[tauri::command]
pub fn probe_codex_app_schema() -> CodexAppSchemaProbe {
    CodexAppAdapter::probe_schema()
}

/// 获取 Codex CLI session 详情。
#[tauri::command]
pub fn get_codex_cli_session_detail(
    session_key: SessionKey,
) -> Result<Option<SessionDetailViewModel>, String> {
    ensure_codex_cli_bridge_started()?;
    let runtime = lock_codex_cli_runtime()?;

    Ok(runtime.session_detail(&session_key))
}

/// 获取 Codex APP session 详情。
#[tauri::command]
pub fn get_codex_app_session_detail(
    session_key: SessionKey,
) -> Result<Option<SessionDetailViewModel>, String> {
    ensure_codex_cli_bridge_started()?;
    schedule_codex_app_context_sync();
    let runtime = lock_codex_app_runtime()?;

    Ok(runtime.session_detail(&session_key))
}

/// 跳回指定 session 的工具界面。
#[tauri::command]
pub fn jump_to_session(request: JumpToSessionRequest) -> JumpToSessionResult {
    let Some(jump_target) = session_jump_target(&request) else {
        return JumpToSessionResult::not_jumped("当前 session 没有可用跳回目标", None);
    };

    let mut adapter = TerminalJumpAdapter::new();
    match adapter.jump_to_session(&request.session_key, &jump_target) {
        Ok(()) => JumpToSessionResult {
            jumped: true,
            message: "已跳回工具界面".to_string(),
            fallback_text: None,
        },
        Err(error) => {
            let fallback_text = if error.fallback_action == Some(FallbackAction::CopyToClipboard) {
                Some(jump_target.location)
            } else {
                None
            };
            JumpToSessionResult::not_jumped(error.user_message, fallback_text)
        }
    }
}

/// 提交 Codex CLI 审批决策。
#[tauri::command]
pub fn resolve_codex_cli_approval(request: ResolveCodexApprovalRequest) -> Result<(), String> {
    ensure_codex_cli_bridge_started()?;
    let runtime = codex_cli_runtime();
    let mut runtime = runtime
        .lock()
        .map_err(|_| "Codex CLI runtime 锁已损坏".to_string())?;

    runtime
        .resolve_approval(
            &request.session_key,
            &request.interaction_id,
            request.decision,
        )
        .map_err(|error| error.user_message)
}

/// 提交 Codex APP 审批决策。
#[tauri::command]
pub fn resolve_codex_app_approval(request: ResolveCodexApprovalRequest) -> Result<(), String> {
    ensure_codex_cli_bridge_started()?;
    let write = {
        let runtime = codex_app_runtime();
        let mut runtime = runtime
            .lock()
            .map_err(|_| "Codex APP runtime 锁已损坏".to_string())?;
        runtime
            .resolve_approval(
                &request.session_key,
                &request.interaction_id,
                request.decision,
            )
            .map_err(|error| error.user_message)?
    };

    if let Some(write) = write {
        if let Err(error) = ensure_codex_app_started() {
            release_codex_app_rpc_submission(&request.interaction_id);
            return Err(error);
        }
        write_codex_app_rpc_submission(write, &request.interaction_id)?;
        let runtime = codex_app_runtime();
        let mut runtime = runtime
            .lock()
            .map_err(|_| "Codex APP runtime 锁已损坏".to_string())?;
        runtime
            .complete_rpc_interaction(
                &request.session_key,
                &request.interaction_id,
                codex_app_decision_summary(request.decision),
            )
            .map_err(|error| error.user_message)?;
    }

    Ok(())
}

/// 提交 Codex APP 选项回复。
#[tauri::command]
pub fn submit_codex_app_choice(request: SubmitCodexAppChoiceRequest) -> Result<(), String> {
    ensure_codex_app_started()?;
    let write = {
        let runtime = codex_app_runtime();
        let mut runtime = runtime
            .lock()
            .map_err(|_| "Codex APP runtime 锁已损坏".to_string())?;
        runtime
            .submit_choice(
                &request.session_key,
                &request.interaction_id,
                ChoiceSubmission {
                    selected_values: request.selected_values.clone(),
                },
            )
            .map_err(|error| error.user_message)?
    };

    write_codex_app_rpc_submission(write, &request.interaction_id)?;

    let runtime = codex_app_runtime();
    let mut runtime = runtime
        .lock()
        .map_err(|_| "Codex APP runtime 锁已损坏".to_string())?;
    runtime
        .complete_rpc_interaction(&request.session_key, &request.interaction_id, "")
        .map_err(|error| error.user_message)
}

/// 提交 Codex APP 文本回复。
#[tauri::command]
pub fn send_codex_app_reply(request: SendCodexAppReplyRequest) -> Result<(), String> {
    ensure_codex_app_started()?;
    let write = {
        let runtime = codex_app_runtime();
        let mut runtime = runtime
            .lock()
            .map_err(|_| "Codex APP runtime 锁已损坏".to_string())?;
        runtime
            .send_reply(
                &request.session_key,
                &request.interaction_id,
                &request.content,
            )
            .map_err(|error| error.user_message)?
    };

    write_codex_app_rpc_submission(write, &request.interaction_id)?;

    let runtime = codex_app_runtime();
    let mut runtime = runtime
        .lock()
        .map_err(|_| "Codex APP runtime 锁已损坏".to_string())?;
    runtime
        .complete_rpc_interaction(&request.session_key, &request.interaction_id, "")
        .map_err(|error| error.user_message)
}

/// 创建 Codex APP follow-up turn。
#[tauri::command]
pub fn create_codex_app_followup_turn(request: CodexAppFollowupRequest) -> Result<(), String> {
    ensure_codex_app_started()?;
    let write = {
        let runtime = codex_app_runtime();
        let mut runtime = runtime
            .lock()
            .map_err(|_| "Codex APP runtime 锁已损坏".to_string())?;
        runtime
            .create_followup_turn(&request.session_key, &request.prompt)
            .map_err(|error| error.user_message)?
    };
    let server = codex_app_server_client()?;
    if write.waits_for_response {
        if let Err(error) = server.write_rpc_request(write) {
            let runtime = codex_app_runtime();
            if let Ok(mut runtime) = runtime.lock() {
                runtime.release_followup_turn(&request.session_key);
            }
            return Err(error.user_message);
        }
        let runtime = codex_app_runtime();
        let mut runtime = runtime
            .lock()
            .map_err(|_| "Codex APP runtime 锁已损坏".to_string())?;
        runtime
            .complete_followup_turn(&request.session_key)
            .map_err(|error| error.user_message)?;
        return Ok(());
    }
    server
        .write_rpc_response(write)
        .map_err(|error| error.user_message)
}

/// 查询 Codex CLI 过程事件时间线。
#[tauri::command]
pub fn query_codex_cli_timeline(query: TimelineQuery) -> Result<TimelinePage, String> {
    ensure_codex_cli_bridge_started()?;
    let runtime = lock_codex_cli_runtime()?;
    let service = ProcessTimelineService::new(&*runtime);

    service
        .query_timeline(query)
        .map_err(|error| error.user_message)
}

/// 查询 Codex APP 过程事件时间线。
#[tauri::command]
pub fn query_codex_app_timeline(query: TimelineQuery) -> Result<TimelinePage, String> {
    ensure_codex_cli_bridge_started()?;
    let _ = ensure_codex_app_started();
    let runtime = lock_codex_app_runtime()?;
    let service = ProcessTimelineService::new(&*runtime);

    service
        .query_timeline(query)
        .map_err(|error| error.user_message)
}

/// 释放 Codex CLI 过程事件时间线大文本缓存。
#[tauri::command]
pub fn release_codex_cli_timeline_cache(session_key: SessionKey) -> Result<usize, String> {
    ensure_codex_cli_bridge_started()?;
    let mut runtime = lock_codex_cli_runtime()?;

    runtime
        .release_large_texts(&session_key)
        .map_err(|error| error.user_message)
}

/// 释放 Codex APP 过程事件时间线大文本缓存。
#[tauri::command]
pub fn release_codex_app_timeline_cache(session_key: SessionKey) -> Result<usize, String> {
    ensure_codex_cli_bridge_started()?;
    let mut runtime = lock_codex_app_runtime()?;

    runtime
        .release_large_texts(&session_key)
        .map_err(|error| error.user_message)
}

/// 返回 session 跳回目标。
fn session_jump_target(request: &JumpToSessionRequest) -> Option<JumpTarget> {
    match request.runtime_source {
        RuntimeSource::CodexCli => {
            ensure_codex_cli_bridge_started().ok()?;
            let runtime = lock_codex_cli_runtime().ok()?;
            runtime
                .session_state()
                .sessions
                .get(&request.session_key)
                .and_then(|session| session.jump_target.clone())
        }
        RuntimeSource::CodexApp => {
            let runtime = lock_codex_app_runtime().ok()?;
            runtime
                .session_state()
                .sessions
                .get(&request.session_key)
                .and_then(|session| session.jump_target.clone())
        }
    }
}

/// Codex 审批提交请求。
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct ResolveCodexApprovalRequest {
    /// 所属会话。
    pub session_key: SessionKey,
    /// 所属交互。
    pub interaction_id: crate::domain::agent_interaction::InteractionId,
    /// 审批决策。
    pub decision: ApprovalDecision,
}

/// Codex APP 文本回复请求。
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct SendCodexAppReplyRequest {
    /// 所属会话。
    pub session_key: SessionKey,
    /// 所属交互。
    pub interaction_id: crate::domain::agent_interaction::InteractionId,
    /// 文本内容。
    pub content: String,
}

/// Codex APP 选项提交请求。
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct SubmitCodexAppChoiceRequest {
    /// 所属会话。
    pub session_key: SessionKey,
    /// 所属交互。
    pub interaction_id: crate::domain::agent_interaction::InteractionId,
    /// 用户选择的选项值。
    pub selected_values: Vec<String>,
}

/// Codex APP follow-up turn 请求。
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct CodexAppFollowupRequest {
    /// 所属会话。
    pub session_key: SessionKey,
    /// 用户输入。
    pub prompt: String,
}

/// panel 窗口局部状态更新。
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct PanelWindowStateUpdate {
    /// 是否处于收缩状态。
    pub collapsed: Option<bool>,
    /// 上次窗口位置。
    pub window_position: Option<PanelWindowPosition>,
    /// 上次窗口尺寸。
    pub window_size: Option<PanelWindowSize>,
}

/// hook 安装请求。
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct HookInstallRequest {
    /// 目标 agent 集合。
    pub agents: Vec<HookInstallAgent>,
}

/// 前端 session 运行时来源。
#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSource {
    /// Codex CLI runtime。
    CodexCli,
    /// Codex APP runtime。
    CodexApp,
}

/// 跳回 session 请求。
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct JumpToSessionRequest {
    /// 运行时来源。
    pub runtime_source: RuntimeSource,
    /// 所属会话。
    pub session_key: SessionKey,
}

/// 跳回 session 结果。
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct JumpToSessionResult {
    /// 是否完成跳回。
    pub jumped: bool,
    /// 用户可读状态。
    pub message: String,
    /// 复制降级文本。
    pub fallback_text: Option<String>,
}

impl JumpToSessionResult {
    /// 创建未跳回结果。
    fn not_jumped(message: impl Into<String>, fallback_text: Option<String>) -> Self {
        Self {
            jumped: false,
            message: message.into(),
            fallback_text,
        }
    }
}

/// 创建默认 hook 安装器。
fn default_hook_installer() -> Result<HookInstaller, String> {
    let hook_executable_path = default_hook_executable_path()?;
    let paths = HookInstallPaths::user_defaults(hook_executable_path);

    Ok(HookInstaller::new(paths))
}

/// 返回默认 hook helper 路径。
fn default_hook_executable_path() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("BUILDER_PANEL_HOOK_PATH") {
        return Ok(PathBuf::from(path));
    }

    let current_exe =
        std::env::current_exe().map_err(|error| format!("当前可执行文件路径读取失败：{error}"))?;
    if current_exe
        .file_stem()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "builder-panel-hook")
    {
        return Ok(current_exe);
    }

    let Some(parent) = current_exe.parent() else {
        return Err("当前可执行文件目录读取失败".to_string());
    };

    Ok(parent.join(hook_executable_name()))
}

/// 返回当前平台 hook helper 文件名。
fn hook_executable_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "builder-panel-hook.exe"
    }

    #[cfg(not(target_os = "windows"))]
    {
        "builder-panel-hook"
    }
}

/// 获取 Codex CLI runtime。
fn codex_cli_runtime() -> Arc<Mutex<CodexCliHookRuntime>> {
    CODEX_CLI_RUNTIME
        .get_or_init(|| {
            Arc::new(Mutex::new(CodexCliHookRuntime::with_update_sink(
                session_update_sink(),
            )))
        })
        .clone()
}

/// 获取 Codex APP runtime。
fn codex_app_runtime() -> Arc<Mutex<CodexAppRuntime>> {
    CODEX_APP_RUNTIME
        .get_or_init(|| {
            Arc::new(Mutex::new(CodexAppRuntime::with_update_sink(
                session_update_sink(),
            )))
        })
        .clone()
}

/// 获取 Codex CLI runtime 锁。
fn lock_codex_cli_runtime() -> Result<MutexGuard<'static, CodexCliHookRuntime>, String> {
    CODEX_CLI_RUNTIME
        .get_or_init(|| {
            Arc::new(Mutex::new(CodexCliHookRuntime::with_update_sink(
                session_update_sink(),
            )))
        })
        .lock()
        .map_err(|_| "Codex CLI runtime 锁已损坏".to_string())
}

/// 获取 Codex APP runtime 锁。
fn lock_codex_app_runtime() -> Result<MutexGuard<'static, CodexAppRuntime>, String> {
    CODEX_APP_RUNTIME
        .get_or_init(|| {
            Arc::new(Mutex::new(CodexAppRuntime::with_update_sink(
                session_update_sink(),
            )))
        })
        .lock()
        .map_err(|_| "Codex APP runtime 锁已损坏".to_string())
}

/// 配置全局 session 更新发布端口。
pub fn configure_session_update_sink(update_sink: Arc<dyn SessionUpdateSinkPort>) {
    let _ = SESSION_UPDATE_SINK.set(update_sink);
}

/// 返回全局 session 更新发布端口。
fn session_update_sink() -> Arc<dyn SessionUpdateSinkPort> {
    SESSION_UPDATE_SINK
        .get_or_init(|| Arc::new(NoopSessionUpdateSink))
        .clone()
}

/// 确保 Codex CLI bridge server 已启动。
fn ensure_codex_cli_bridge_started() -> Result<(), String> {
    let started = CODEX_CLI_BRIDGE_STARTED.get_or_init(|| Mutex::new(false));
    let mut started = started
        .lock()
        .map_err(|_| "Codex CLI bridge 启动状态锁已损坏".to_string())?;
    if *started {
        return Ok(());
    }

    let runtime = codex_cli_runtime();
    let codex_app_runtime = codex_app_runtime();
    start_codex_cli_bridge_server(runtime, codex_app_runtime)
        .map_err(|error| format!("Codex CLI bridge 启动失败：{error:?}"))?;
    start_codex_rollout_watcher_once();
    *started = true;
    Ok(())
}

/// 启动 Codex rollout 追加行 watcher。
fn start_codex_rollout_watcher_once() {
    let started = CODEX_ROLLOUT_WATCHER_STARTED.get_or_init(|| Mutex::new(false));
    let Ok(mut started) = started.lock() else {
        return;
    };
    if *started {
        return;
    }
    *started = true;

    std::thread::spawn(|| {
        let mut tailer = CodexRolloutTailer::default_root();
        loop {
            tailer.sync_targets(current_codex_rollout_watch_targets());
            let events = tailer.poll_events(command_unix_now());
            for event in events {
                match event.session_key().agent_kind.clone() {
                    crate::domain::agent_session::AgentKind::CodexCli => {
                        if let Ok(mut runtime) = codex_cli_runtime().lock() {
                            let _ = runtime.apply_event(event);
                        }
                    }
                    crate::domain::agent_session::AgentKind::CodexApp => {
                        if let Ok(mut runtime) = codex_app_runtime().lock() {
                            let _ = runtime.apply_rollout_event(event);
                        }
                    }
                    crate::domain::agent_session::AgentKind::ClaudeCodeApp
                    | crate::domain::agent_session::AgentKind::ClaudeCodeCli => {}
                }
            }
            std::thread::sleep(CODEX_ROLLOUT_WATCH_INTERVAL);
        }
    });
}

/// 返回当前 Codex rollout watcher 目标。
fn current_codex_rollout_watch_targets() -> Vec<CodexRolloutWatchTarget> {
    let mut targets = Vec::new();
    if let Ok(runtime) = codex_cli_runtime().lock() {
        targets.extend(runtime.rollout_watch_targets());
    }
    if let Ok(runtime) = codex_app_runtime().lock() {
        targets.extend(runtime.rollout_watch_targets());
    }

    targets
}

/// 确保 Codex APP hook bridge 和 app-server 已启动。
fn ensure_codex_app_started() -> Result<(), String> {
    ensure_codex_cli_bridge_started()?;
    if let Some(message) = codex_app_startup_backoff_message()? {
        return Err(message);
    }
    match prepare_codex_app_startup()? {
        CodexAppStartupAction::AlreadyRunning => return Ok(()),
        CodexAppStartupAction::Start => {}
        CodexAppStartupAction::Exited => {
            record_codex_app_startup_failure("Codex APP app-server 已退出")?;
            return Err("Codex APP app-server 已退出".to_string());
        }
    }

    let codex_path = std::env::var("BUILDER_PANEL_CODEX_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("codex"));
    let cwd = std::env::current_dir()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());
    let client = match CodexAppServerClient::start(&codex_path, cwd, codex_app_runtime()) {
        Ok(client) => client,
        Err(error) => {
            reset_codex_app_server_slot()?;
            record_codex_app_startup_failure(&error.user_message)?;
            return Err(error.user_message);
        }
    };
    publish_codex_app_server_client(Arc::new(client))?;
    clear_codex_app_startup_failure()?;

    Ok(())
}

/// 调度 Codex APP thread 元数据和 rollout 历史后台同步。
fn schedule_codex_app_context_sync() {
    let sync_slot = CODEX_APP_CONTEXT_SYNC_IN_FLIGHT.get_or_init(|| Mutex::new(false));
    let Ok(mut in_flight) = sync_slot.lock() else {
        return;
    };
    if *in_flight || !should_sync_codex_app_thread_metadata() {
        return;
    }
    *in_flight = true;

    std::thread::spawn(|| {
        let _guard = CodexAppContextSyncGuard;
        sync_codex_app_context_worker();
    });
}

/// Codex APP 后台同步结束时释放运行标记。
struct CodexAppContextSyncGuard;

impl Drop for CodexAppContextSyncGuard {
    fn drop(&mut self) {
        let sync_slot = CODEX_APP_CONTEXT_SYNC_IN_FLIGHT.get_or_init(|| Mutex::new(false));
        if let Ok(mut in_flight) = sync_slot.lock() {
            *in_flight = false;
        }
    }
}

/// 同步 Codex APP thread 元数据和 rollout 历史。
fn sync_codex_app_context_worker() {
    let Ok(()) = ensure_codex_app_started() else {
        return;
    };
    let Ok(server) = codex_app_server_client() else {
        return;
    };

    let loaded_threads = server.list_loaded_threads().unwrap_or_default();
    let unresolved_thread_ids = {
        let runtime = codex_app_runtime();
        let ids = match runtime.lock() {
            Ok(mut runtime) => {
                for thread in loaded_threads.iter().cloned() {
                    let _ = runtime.apply_thread_metadata(thread, command_unix_now());
                }
                runtime.unresolved_thread_ids()
            }
            Err(_) => Vec::new(),
        };
        ids
    };
    let needs_history = !unresolved_thread_ids.is_empty();

    let history_threads = if needs_history {
        let unresolved_ids: BTreeSet<String> = unresolved_thread_ids.iter().cloned().collect();
        filter_history_threads_for_unresolved(
            server.list_threads(40).unwrap_or_default(),
            &unresolved_ids,
        )
    } else {
        Vec::new()
    };
    if !history_threads.is_empty() {
        let runtime = codex_app_runtime();
        if let Ok(mut runtime) = runtime.lock() {
            for thread in history_threads.iter().cloned() {
                let _ = runtime.apply_thread_metadata(thread, command_unix_now());
            }
        };
    }

    let mut rollout_threads = loaded_threads;
    rollout_threads.extend(history_threads);
    let candidate_thread_ids =
        rollout_candidate_thread_ids(&rollout_threads, &unresolved_thread_ids);
    sync_codex_rollout_history(&rollout_threads, &candidate_thread_ids, needs_history);
}

/// 同步 Codex rollout 历史，避免高频全量扫描。
fn sync_codex_rollout_history(
    threads: &[crate::adapters::codex_app::CodexAppThreadMetadata],
    candidate_thread_ids: &BTreeSet<String>,
    needs_recent_scan: bool,
) {
    let discovery = CodexRolloutDiscovery::default_root();
    let mut snapshots = Vec::new();
    for thread in threads {
        let Some(path) = thread.path.as_deref() else {
            continue;
        };
        if !candidate_thread_ids.contains(&thread.id) {
            continue;
        }
        if let Some(snapshot) = discovery.read_path(path) {
            if rollout_snapshot_matches_thread(
                &thread.id,
                &snapshot.session_id,
                candidate_thread_ids,
            ) {
                snapshots.push(snapshot);
            }
        }
    }

    if needs_recent_scan && should_scan_recent_rollouts() {
        snapshots.extend(
            discovery
                .discover_recent(SystemTime::now())
                .into_iter()
                .filter(|snapshot| candidate_thread_ids.contains(&snapshot.session_id)),
        );
    }

    if snapshots.is_empty() {
        return;
    }

    let runtime = codex_app_runtime();
    if let Ok(mut runtime) = runtime.lock() {
        for snapshot in snapshots {
            let _ = runtime.apply_rollout_snapshot(snapshot);
        }
    };
}

/// 仅保留能补齐当前待识别 session 的历史 thread。
fn filter_history_threads_for_unresolved(
    threads: Vec<crate::adapters::codex_app::CodexAppThreadMetadata>,
    unresolved_thread_ids: &BTreeSet<String>,
) -> Vec<crate::adapters::codex_app::CodexAppThreadMetadata> {
    threads
        .into_iter()
        .filter(|thread| unresolved_thread_ids.contains(&thread.id))
        .collect()
}

/// 校验 rollout 快照是否属于当前 thread 候选。
fn rollout_snapshot_matches_thread(
    thread_id: &str,
    snapshot_session_id: &str,
    candidate_thread_ids: &BTreeSet<String>,
) -> bool {
    thread_id == snapshot_session_id && candidate_thread_ids.contains(snapshot_session_id)
}

/// 汇总允许被 rollout 补齐的已知 thread id。
fn rollout_candidate_thread_ids(
    threads: &[crate::adapters::codex_app::CodexAppThreadMetadata],
    unresolved_thread_ids: &[String],
) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    ids.extend(threads.iter().map(|thread| thread.id.clone()));
    ids.extend(unresolved_thread_ids.iter().cloned());
    ids
}

/// 判断是否允许同步 app-server thread 元数据。
fn should_sync_codex_app_thread_metadata() -> bool {
    let slot = CODEX_APP_THREAD_METADATA_LAST_SYNC.get_or_init(|| Mutex::new(None));
    let Ok(mut last_sync) = slot.lock() else {
        return false;
    };
    let now = Instant::now();
    if last_sync
        .is_some_and(|last| now.duration_since(last) < CODEX_APP_THREAD_METADATA_SYNC_INTERVAL)
    {
        return false;
    }

    *last_sync = Some(now);
    true
}

/// 判断是否允许扫描近期 rollout。
fn should_scan_recent_rollouts() -> bool {
    let slot = CODEX_APP_ROLLOUT_LAST_SYNC.get_or_init(|| Mutex::new(None));
    let Ok(mut last_sync) = slot.lock() else {
        return false;
    };
    let now = Instant::now();
    if last_sync.is_some_and(|last| now.duration_since(last) < CODEX_APP_ROLLOUT_SYNC_INTERVAL) {
        return false;
    }

    *last_sync = Some(now);
    true
}

/// 准备启动 Codex APP app-server。
fn prepare_codex_app_startup() -> Result<CodexAppStartupAction, String> {
    let slot = codex_app_server_slot();
    let mut slot = slot
        .lock()
        .map_err(|_| "Codex APP app-server 锁已损坏".to_string())?;
    match &*slot {
        CodexAppServerSlot::Ready(server) if server.is_running() => {
            Ok(CodexAppStartupAction::AlreadyRunning)
        }
        CodexAppServerSlot::Ready(_) => {
            *slot = CodexAppServerSlot::Empty;
            Ok(CodexAppStartupAction::Exited)
        }
        CodexAppServerSlot::Starting => Err("Codex APP app-server 正在启动".to_string()),
        CodexAppServerSlot::Empty => {
            *slot = CodexAppServerSlot::Starting;
            Ok(CodexAppStartupAction::Start)
        }
    }
}

/// 清空 Codex APP app-server client 槽。
fn reset_codex_app_server_slot() -> Result<(), String> {
    let slot = codex_app_server_slot();
    let mut slot = slot
        .lock()
        .map_err(|_| "Codex APP app-server 锁已损坏".to_string())?;
    *slot = CodexAppServerSlot::Empty;
    Ok(())
}

/// 发布 Codex APP app-server client。
fn publish_codex_app_server_client(client: Arc<CodexAppServerClient>) -> Result<(), String> {
    let slot = codex_app_server_slot();
    let mut slot = slot
        .lock()
        .map_err(|_| "Codex APP app-server 锁已损坏".to_string())?;
    *slot = CodexAppServerSlot::Ready(client);
    Ok(())
}

/// 返回 Codex APP 启动退避错误。
fn codex_app_startup_backoff_message() -> Result<Option<String>, String> {
    let failure_slot = CODEX_APP_STARTUP_FAILURE.get_or_init(|| Mutex::new(None));
    let mut failure_slot = failure_slot
        .lock()
        .map_err(|_| "Codex APP 启动失败状态锁已损坏".to_string())?;
    let Some(failure) = failure_slot.as_ref() else {
        return Ok(None);
    };
    if Instant::now() < failure.retry_at {
        return Ok(Some(failure.message.clone()));
    }

    *failure_slot = None;
    Ok(None)
}

/// 记录 Codex APP 启动失败并设置退避时间。
fn record_codex_app_startup_failure(message: &str) -> Result<(), String> {
    let failure_slot = CODEX_APP_STARTUP_FAILURE.get_or_init(|| Mutex::new(None));
    let mut failure_slot = failure_slot
        .lock()
        .map_err(|_| "Codex APP 启动失败状态锁已损坏".to_string())?;
    *failure_slot = Some(CodexAppStartupFailure {
        retry_at: Instant::now() + CODEX_APP_STARTUP_RETRY_DELAY,
        message: message.to_string(),
    });
    Ok(())
}

/// 清理 Codex APP 启动失败退避状态。
fn clear_codex_app_startup_failure() -> Result<(), String> {
    let failure_slot = CODEX_APP_STARTUP_FAILURE.get_or_init(|| Mutex::new(None));
    let mut failure_slot = failure_slot
        .lock()
        .map_err(|_| "Codex APP 启动失败状态锁已损坏".to_string())?;
    *failure_slot = None;
    Ok(())
}

/// 写入 Codex APP app-server RPC 消息。
fn write_codex_app_rpc(write: crate::adapters::codex_app::CodexAppRpcWrite) -> Result<(), String> {
    let server = codex_app_server_client()?;
    server
        .write_rpc_response(write)
        .map_err(|error| error.user_message)
}

/// 写入 Codex APP app-server RPC response，失败时释放提交占位。
fn write_codex_app_rpc_submission(
    write: crate::adapters::codex_app::CodexAppRpcWrite,
    interaction_id: &InteractionId,
) -> Result<(), String> {
    match write_codex_app_rpc(write) {
        Ok(()) => Ok(()),
        Err(error) => {
            release_codex_app_rpc_submission(interaction_id);
            Err(error)
        }
    }
}

/// 释放 Codex APP app-server RPC 提交占位。
fn release_codex_app_rpc_submission(interaction_id: &InteractionId) {
    let runtime = codex_app_runtime();
    match runtime.lock() {
        Ok(mut runtime) => {
            runtime.release_rpc_submission(interaction_id);
        }
        Err(_) => {}
    };
}

/// 获取 Codex APP app-server 客户端快照。
fn codex_app_server_client() -> Result<Arc<CodexAppServerClient>, String> {
    let server = codex_app_server_slot()
        .lock()
        .map_err(|_| "Codex APP app-server 锁已损坏".to_string())?;
    match &*server {
        CodexAppServerSlot::Ready(client) => Ok(Arc::clone(client)),
        CodexAppServerSlot::Starting => Err("Codex APP app-server 正在启动".to_string()),
        CodexAppServerSlot::Empty => Err("Codex APP app-server 未连接".to_string()),
    }
}

/// 返回 Codex APP app-server client 槽锁。
fn codex_app_server_slot() -> &'static Mutex<CodexAppServerSlot> {
    CODEX_APP_SERVER.get_or_init(|| Mutex::new(CodexAppServerSlot::Empty))
}

/// 返回 Codex APP 审批完成摘要。
fn codex_app_decision_summary(decision: ApprovalDecision) -> &'static str {
    match decision {
        ApprovalDecision::Allow => "Codex APP 审批已允许",
        ApprovalDecision::AllowAndRemember => "Codex APP 审批已允许并记住",
        ApprovalDecision::Deny => "Codex APP 审批已拒绝",
    }
}

/// 当前 Unix 毫秒。
fn command_unix_now() -> UnixMillis {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default();
    UnixMillis::new(millis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::codex_app::CodexAppThreadMetadata;

    #[test]
    fn rollout_candidates_only_include_known_thread_ids() {
        let threads = vec![thread_metadata("loaded-thread", "/tmp/loaded")];
        let unresolved = vec!["unresolved-thread".to_string()];

        let candidates = rollout_candidate_thread_ids(&threads, &unresolved);

        assert!(candidates.contains("loaded-thread"));
        assert!(candidates.contains("unresolved-thread"));
        assert!(!candidates.contains("unrelated-history-thread"));
    }

    #[test]
    fn history_threads_only_fill_unresolved_candidates() {
        let threads = vec![
            thread_metadata("unresolved-thread", "/tmp/resolved"),
            thread_metadata("unrelated-history-thread", "/tmp/unrelated"),
        ];
        let unresolved = BTreeSet::from(["unresolved-thread".to_string()]);

        let filtered = filter_history_threads_for_unresolved(threads, &unresolved);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "unresolved-thread");
    }

    #[test]
    fn history_threads_keep_path_only_unresolved_candidates() {
        let threads = vec![CodexAppThreadMetadata {
            id: "unresolved-thread".to_string(),
            cwd: None,
            name: None,
            preview: None,
            path: Some(PathBuf::from("/tmp/rollout-unresolved-thread.jsonl")),
            status_type: "idle".to_string(),
            ephemeral: false,
        }];
        let unresolved = BTreeSet::from(["unresolved-thread".to_string()]);

        let filtered = filter_history_threads_for_unresolved(threads, &unresolved);

        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].cwd.is_none());
        assert!(filtered[0].path.is_some());
    }

    #[test]
    fn rollout_path_snapshot_must_match_thread_candidate() {
        let candidates = BTreeSet::from(["thread-1".to_string()]);

        assert!(rollout_snapshot_matches_thread(
            "thread-1",
            "thread-1",
            &candidates
        ));
        assert!(!rollout_snapshot_matches_thread(
            "thread-1",
            "unrelated-thread",
            &candidates
        ));
        assert!(!rollout_snapshot_matches_thread(
            "unlisted-thread",
            "unlisted-thread",
            &candidates
        ));
    }

    fn thread_metadata(id: &str, cwd: &str) -> CodexAppThreadMetadata {
        CodexAppThreadMetadata {
            id: id.to_string(),
            cwd: Some(cwd.to_string()),
            name: None,
            preview: None,
            path: None,
            status_type: "idle".to_string(),
            ephemeral: false,
        }
    }
}
