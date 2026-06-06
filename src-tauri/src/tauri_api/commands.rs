//! Tauri command 入口。

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::adapters::codex_app::{CodexAppAdapter, CodexAppSchemaProbe};
use crate::adapters::codex_cli_hook::{start_codex_cli_bridge_server, CodexCliHookRuntime};
use crate::adapters::config_file::JsonSettingsStore;
use crate::adapters::hook_install::{
    HookInstallAgent, HookInstallManifest, HookInstallPaths, HookInstallPreview, HookInstaller,
};
use crate::adapters::mock_agent::MockAgentRuntime;
use crate::domain::agent_session::SessionKey;
use crate::domain::panel_probe::PanelProbe;
use crate::domain::view_model::{SessionDetailViewModel, SessionListItemViewModel};
use crate::ports::agent_adapter_port::ApprovalDecision;
use crate::ports::process_timeline_port::ProcessTimelineReleasePort;
use crate::services::interaction_service::{
    InteractionService, ResolveApprovalRequest, SubmitChoiceRequest,
};
use crate::services::process_timeline_service::{
    ProcessTimelineService, TimelinePage, TimelineQuery,
};
use crate::services::reply_service::{ReplyService, SendReplyRequest};
use crate::services::session_service::SessionService;
use crate::services::settings_service::{
    BuilderPanelSettings, PanelWindowPosition, PanelWindowSize, SettingsService, SettingsViewModel,
};

/// 全局 mock runtime，仅用于阶段 3 本地闭环。
static MOCK_RUNTIME: OnceLock<Mutex<MockAgentRuntime>> = OnceLock::new();
/// 全局 Codex CLI runtime，用于阶段 4 真实 hook 闭环。
static CODEX_CLI_RUNTIME: OnceLock<Arc<Mutex<CodexCliHookRuntime>>> = OnceLock::new();
/// Codex CLI bridge server 启动标记。
static CODEX_CLI_BRIDGE_STARTED: OnceLock<Mutex<bool>> = OnceLock::new();

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

    if let Some(collapsed) = update.collapsed {
        settings.panel.collapsed = collapsed;
    }
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
pub fn uninstall_hooks() -> Result<(), String> {
    let installer = default_hook_installer()?;

    installer.uninstall().map_err(|error| error.user_message)
}

/// 获取 mock session 列表。
#[tauri::command]
pub fn get_mock_sessions() -> Result<Vec<SessionListItemViewModel>, String> {
    let runtime = lock_mock_runtime()?;
    let service = SessionService::new(&runtime);

    Ok(service.list_sessions())
}

/// 获取 Codex CLI session 列表。
#[tauri::command]
pub fn get_codex_cli_sessions() -> Result<Vec<SessionListItemViewModel>, String> {
    ensure_codex_cli_bridge_started()?;
    let runtime = lock_codex_cli_runtime()?;

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

/// 获取 mock session 详情。
#[tauri::command]
pub fn get_mock_session_detail(
    session_key: SessionKey,
) -> Result<Option<SessionDetailViewModel>, String> {
    let runtime = lock_mock_runtime()?;
    let service = SessionService::new(&runtime);

    Ok(service.session_detail(&session_key))
}

/// 提交 mock 审批决策。
#[tauri::command]
pub fn resolve_mock_approval(request: ResolveApprovalRequest) -> Result<(), String> {
    let mut runtime = lock_mock_runtime()?;
    let mut service = InteractionService::new(&mut runtime);

    service
        .resolve_approval(request)
        .map_err(|error| error.user_message)
}

/// 提交 mock 选项回复。
#[tauri::command]
pub fn submit_mock_choice(request: SubmitChoiceRequest) -> Result<(), String> {
    let mut runtime = lock_mock_runtime()?;
    let mut service = InteractionService::new(&mut runtime);

    service
        .submit_choice(request)
        .map_err(|error| error.user_message)
}

/// 提交 mock 文本回复。
#[tauri::command]
pub fn send_mock_reply(request: SendReplyRequest) -> Result<(), String> {
    let mut runtime = lock_mock_runtime()?;
    let mut service = ReplyService::new(&mut runtime);

    service
        .send_reply(request)
        .map_err(|error| error.user_message)
}

/// 查询 mock 过程事件时间线。
#[tauri::command]
pub fn query_mock_timeline(query: TimelineQuery) -> Result<TimelinePage, String> {
    let runtime = lock_mock_runtime()?;
    let service = ProcessTimelineService::new(&*runtime);

    service
        .query_timeline(query)
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

/// 释放 mock 过程事件时间线大文本缓存。
#[tauri::command]
pub fn release_mock_timeline_cache(session_key: SessionKey) -> Result<usize, String> {
    let mut runtime = lock_mock_runtime()?;

    runtime
        .release_large_texts(&session_key)
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

/// 重置 mock runtime。
#[tauri::command]
pub fn reset_mock_runtime() -> Result<(), String> {
    let mut runtime = lock_mock_runtime()?;
    *runtime = MockAgentRuntime::stage3_default();

    Ok(())
}

/// 获取 mock runtime 锁。
fn lock_mock_runtime() -> Result<MutexGuard<'static, MockAgentRuntime>, String> {
    MOCK_RUNTIME
        .get_or_init(|| Mutex::new(MockAgentRuntime::stage3_default()))
        .lock()
        .map_err(|_| "mock runtime 锁已损坏".to_string())
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
        .get_or_init(|| Arc::new(Mutex::new(CodexCliHookRuntime::empty())))
        .clone()
}

/// 获取 Codex CLI runtime 锁。
fn lock_codex_cli_runtime() -> Result<MutexGuard<'static, CodexCliHookRuntime>, String> {
    CODEX_CLI_RUNTIME
        .get_or_init(|| Arc::new(Mutex::new(CodexCliHookRuntime::empty())))
        .lock()
        .map_err(|_| "Codex CLI runtime 锁已损坏".to_string())
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
    start_codex_cli_bridge_server(runtime)
        .map_err(|error| format!("Codex CLI bridge 启动失败：{error:?}"))?;
    *started = true;
    Ok(())
}
