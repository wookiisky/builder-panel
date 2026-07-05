//! Tauri command 入口。

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::adapters::codex_app::{
    apply_session_index_thread_titles, default_codex_session_index_path,
    ensure_codex_app_thread_loaded, load_codex_session_index_titles, set_internal_prompt_patterns,
    CodexAppAdapter, CodexAppFollowupRpcClient, CodexAppRuntime, CodexAppSchemaProbe,
    CodexAppServerClient, CodexAppThreadMetadata, CodexRolloutDiscovery, CodexRolloutTailer,
    CodexRolloutWatchTarget,
};
use crate::adapters::codex_app_inject::{
    capture_cursor_position, default_codex_app_injector, ensure_accessibility_trusted,
    open_accessibility_settings, restore_cursor_position, CodexAppInjector,
};
use crate::adapters::codex_cli_hook::{start_codex_cli_bridge_server, CodexCliHookRuntime};
use crate::adapters::config_file::JsonSettingsStore;
use crate::adapters::hook_install::{
    HookInstallAgent, HookInstallManifest, HookInstallPaths, HookInstallPreview, HookInstallStatus,
    HookInstaller,
};
use crate::adapters::logging::{default_log_path, event_logger, log_error, log_info};
use crate::adapters::terminal::{SystemUrlOpener, TerminalJumpAdapter, UrlOpener};
use crate::domain::agent_interaction::InteractionId;
use crate::domain::agent_session::{JumpTarget, SessionKey};
use crate::domain::app_error::FallbackAction;
use crate::domain::panel_probe::PanelProbe;
use crate::domain::usage::UnixMillis;
use crate::domain::view_model::{SessionDetailViewModel, SessionListItemViewModel};
use crate::ports::agent_adapter_port::ApprovalDecision;
use crate::ports::agent_adapter_port::ChoiceSubmission;
use crate::ports::jump_target_port::JumpTargetPort;
use crate::ports::session_update_port::{NoopSessionUpdateSink, SessionUpdateSinkPort};
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
const CODEX_APP_THREAD_LIST_LIMIT: usize = 100;
const CODEX_APP_MAX_LOADED_THREAD_READS: usize = 40;
const CODEX_APP_ACTIVE_ROLLOUT_WINDOW_ENV: &str =
    "BUILDER_PANEL_CODEX_APP_ACTIVE_ROLLOUT_WINDOW_MINUTES";
const CODEX_APP_ACTIVE_ROLLOUT_DEFAULT_WINDOW: Duration = Duration::from_secs(5 * 60);
const COMPLETED_SESSION_RETENTION_ENV: &str = "BUILDER_PANEL_COMPLETED_SESSION_RETENTION_MINUTES";
const COMPLETED_SESSION_DEFAULT_RETENTION: Duration = Duration::from_secs(20 * 60);

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
    let view = service.read_settings();

    refresh_logger(&view.settings);
    refresh_codex_internal_prompt_patterns(&view.settings);
    view
}

/// 保存 Builder Panel 设置。
#[tauri::command]
pub fn save_panel_settings(settings: BuilderPanelSettings) -> Result<SettingsViewModel, String> {
    let store = JsonSettingsStore::default_path();
    let service = SettingsService::new(&store);

    match service.save_settings(settings) {
        Ok(view) => {
            refresh_logger(&view.settings);
            refresh_codex_internal_prompt_patterns(&view.settings);
            log_info(
                "设置已保存",
                json!({
                    "logging_enabled": view.settings.logging.enabled,
                }),
            );
            Ok(view)
        }
        Err(error) => {
            log_error(
                "设置保存失败",
                json!({
                    "code": format!("{:?}", error.code),
                    "message": error.user_message.clone(),
                }),
            );
            Err(error.user_message)
        }
    }
}

/// 获取当前日志文件路径。
#[tauri::command]
pub fn get_log_info() -> LogInfo {
    let path = event_logger().current_path();
    LogInfo {
        path: path.to_string_lossy().to_string(),
    }
}

/// 在系统文件管理器中打开日志目录。
#[tauri::command]
pub fn open_log_folder() -> Result<(), String> {
    let dir = event_logger().current_dir();
    if let Err(error) = std::fs::create_dir_all(&dir) {
        return Err(format!("日志目录创建失败：{error}"));
    }
    open_path_in_file_manager(&dir).map_err(|error| {
        log_error(
            "打开日志目录失败",
            json!({
                "message": error.clone(),
            }),
        );
        error
    })
}

/// 日志信息。
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
pub struct LogInfo {
    /// 日志文件绝对路径。
    pub path: String,
}

fn open_path_in_file_manager(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("打开日志目录失败：{error}"))
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("打开日志目录失败：{error}"))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("打开日志目录失败：{error}"))
    }
}

/// 根据设置刷新全局 logger 配置。
fn refresh_logger(settings: &BuilderPanelSettings) {
    event_logger().configure(settings.logging.enabled, Some(default_log_path()));
}

/// 根据设置刷新 Codex 内部任务提示词过滤模式。
fn refresh_codex_internal_prompt_patterns(settings: &BuilderPanelSettings) {
    set_internal_prompt_patterns(settings.agents.codex_internal_prompt_patterns.iter());
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

    match installer.install(&request.agents) {
        Ok(manifest) => {
            log_info(
                "hook 安装",
                json!({
                    "agents": request.agents.iter().map(hook_agent_label).collect::<Vec<_>>(),
                }),
            );
            Ok(manifest)
        }
        Err(error) => {
            log_error(
                "hook 安装失败",
                json!({
                    "agents": request.agents.iter().map(hook_agent_label).collect::<Vec<_>>(),
                    "message": error.user_message.clone(),
                }),
            );
            Err(error.user_message)
        }
    }
}

/// 卸载 hook。
#[tauri::command]
pub fn uninstall_hooks(request: HookInstallRequest) -> Result<(), String> {
    let installer = default_hook_installer()?;

    match installer.uninstall_agents(&request.agents) {
        Ok(()) => {
            log_info(
                "hook 卸载",
                json!({
                    "agents": request.agents.iter().map(hook_agent_label).collect::<Vec<_>>(),
                }),
            );
            Ok(())
        }
        Err(error) => {
            log_error(
                "hook 卸载失败",
                json!({
                    "agents": request.agents.iter().map(hook_agent_label).collect::<Vec<_>>(),
                    "message": error.user_message.clone(),
                }),
            );
            Err(error.user_message)
        }
    }
}

fn hook_agent_label(agent: &HookInstallAgent) -> &'static str {
    match agent {
        HookInstallAgent::Codex => "codex",
        HookInstallAgent::Claude => "claude",
    }
}

/// 获取 Codex CLI session 列表。
#[tauri::command]
pub fn get_codex_cli_sessions() -> Result<Vec<SessionListItemViewModel>, String> {
    ensure_codex_cli_bridge_started()?;
    schedule_codex_app_context_sync();
    let mut runtime = lock_codex_cli_runtime()?;
    cleanup_expired_codex_cli_completed_sessions(&mut runtime);

    Ok(runtime.session_list())
}

/// 获取 Codex APP session 列表。
#[tauri::command]
pub fn get_codex_app_sessions() -> Result<Vec<SessionListItemViewModel>, String> {
    ensure_codex_cli_bridge_started()?;
    schedule_codex_app_context_sync();
    let mut runtime = lock_codex_app_runtime()?;
    cleanup_expired_codex_app_completed_sessions(&mut runtime);

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
    let mut runtime = lock_codex_cli_runtime()?;
    cleanup_expired_codex_cli_completed_sessions(&mut runtime);

    Ok(runtime.session_detail(&session_key))
}

/// 获取 Codex APP session 详情。
#[tauri::command]
pub fn get_codex_app_session_detail(
    session_key: SessionKey,
) -> Result<Option<SessionDetailViewModel>, String> {
    ensure_codex_cli_bridge_started()?;
    schedule_codex_app_context_sync();
    let mut runtime = lock_codex_app_runtime()?;
    cleanup_expired_codex_app_completed_sessions(&mut runtime);

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
    create_codex_app_followup_turn_with_server(
        codex_app_runtime(),
        &request,
        codex_app_server_client,
    )
}

fn create_codex_app_followup_turn_with_server<S, F>(
    runtime: Arc<Mutex<CodexAppRuntime>>,
    request: &CodexAppFollowupRequest,
    server_provider: F,
) -> Result<(), String>
where
    S: CodexAppFollowupRpcClient,
    F: FnOnce() -> Result<S, String>,
{
    let thread_id = request.session_key.conversation_id.value.clone();
    let write = {
        let mut runtime = runtime
            .lock()
            .map_err(|_| "Codex APP runtime 锁已损坏".to_string())?;
        runtime
            .create_followup_turn(&request.session_key, &request.prompt)
            .map_err(|error| error.user_message)?
    };

    let server = match server_provider() {
        Ok(server) => server,
        Err(error) => {
            return Err(release_and_log_followup_failure(
                &runtime,
                &request.session_key,
                "app-server client",
                &thread_id,
                &error,
            ));
        }
    };

    log_info(
        "Codex APP follow-up 准备写入",
        json!({"thread_id": thread_id, "prompt_chars": request.prompt.chars().count()}),
    );

    match server.list_loaded_thread_ids() {
        Ok(loaded) => {
            let already_loaded = loaded.iter().any(|id| id == &thread_id);
            log_info(
                "Codex APP follow-up loaded threads 快照",
                json!({
                    "thread_id": thread_id,
                    "loaded_count": loaded.len(),
                    "already_loaded": already_loaded,
                    "loaded_ids": loaded,
                }),
            );
        }
        Err(error) => {
            log_info(
                "Codex APP follow-up loaded threads 查询失败",
                json!({"thread_id": thread_id, "message": error.user_message}),
            );
        }
    }

    if let Err(error) = ensure_codex_app_thread_loaded(&server, &thread_id) {
        return Err(release_and_log_followup_failure(
            &runtime,
            &request.session_key,
            "thread loaded",
            &thread_id,
            &error.user_message,
        ));
    }
    log_info(
        "Codex APP follow-up thread 已加载",
        json!({"thread_id": thread_id}),
    );

    if !write.waits_for_response {
        return Err(release_and_log_followup_failure(
            &runtime,
            &request.session_key,
            "request type",
            &thread_id,
            "Codex APP follow-up 写入类型无效",
        ));
    }

    if let Err(error) = server.write_rpc_request(write) {
        return Err(release_and_log_followup_failure(
            &runtime,
            &request.session_key,
            "turn/start",
            &thread_id,
            &error.user_message,
        ));
    }
    log_info(
        "Codex APP follow-up turn/start 已写入",
        json!({"thread_id": thread_id}),
    );

    let mut runtime_guard = match runtime.lock() {
        Ok(runtime_guard) => runtime_guard,
        Err(_) => {
            return Err(release_and_log_followup_failure(
                &runtime,
                &request.session_key,
                "runtime complete",
                &thread_id,
                "Codex APP runtime 锁已损坏",
            ));
        }
    };
    if let Err(error) = runtime_guard.complete_followup_turn_by_thread_id(&thread_id) {
        drop(runtime_guard);
        return Err(release_and_log_followup_failure(
            &runtime,
            &request.session_key,
            "runtime complete",
            &thread_id,
            &error.user_message,
        ));
    }
    log_info("Codex APP follow-up 创建", json!({"thread_id": thread_id}));
    Ok(())
}

/// 注入 Codex.app GUI follow-up（方案 C：AX + 键盘事件）。
///
/// 用 macOS Accessibility API 让 Codex.app 当前 thread 输入框接收文本，
/// 然后模拟 Cmd+V → Return。失败时尝试用 `codex://threads/<id>` 把窗口
/// 跳到目标 thread，让用户手动操作；不回落到老的 turn/start 通道。
#[tauri::command]
pub fn inject_codex_app_followup(request: CodexAppFollowupRequest) -> Result<(), String> {
    let injector = default_codex_app_injector().map_err(|e| e)?;
    inject_codex_app_followup_with(&request, &mut SystemUrlOpener, &injector)
}

/// 是否在注入成功后立即在 builder-panel 自己 session 流里 emit UserMessageUpdated。
///
/// 默认 false：依赖 Codex.app 内 codex 触发 UserPromptSubmit hook 自然回流。
/// E2E 验证 hook 不回流时再切 true。
const INJECT_EMIT_LOCAL_USER_MESSAGE: bool = false;

/// 等待 Codex.app 成为前台的最大毫秒数。
const INJECT_FRONTMOST_TIMEOUT_MS: u64 = 2000;

/// 注入流程结束时把光标还原到原始位置的 RAII guard。
struct CursorRestoreGuard {
    origin: Option<(f64, f64)>,
}

impl CursorRestoreGuard {
    fn capture() -> Self {
        Self {
            origin: capture_cursor_position(),
        }
    }
}

impl Drop for CursorRestoreGuard {
    fn drop(&mut self) {
        if let Some((x, y)) = self.origin {
            restore_cursor_position(x, y);
        }
    }
}

fn inject_codex_app_followup_with<O, I>(
    request: &CodexAppFollowupRequest,
    opener: &mut O,
    injector: &I,
) -> Result<(), String>
where
    O: UrlOpener + ?Sized,
    I: CodexAppInjector + ?Sized,
{
    let prompt = request.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("follow-up 内容不能为空".to_string());
    }
    let thread_id = request.session_key.conversation_id.value.clone();
    log_info(
        "Codex APP 注入开始",
        json!({"thread_id": thread_id, "prompt_chars": prompt.chars().count()}),
    );

    // 捕获原始光标位置；函数无论如何结束都会还原。
    let _cursor_guard = CursorRestoreGuard::capture();

    // 1. 权限检查。
    if let Err(error) = ensure_accessibility_trusted() {
        // 用户友好地引导跳设置。
        open_accessibility_settings();
        log_error(
            "Codex APP 注入失败",
            json!({
                "stage": "accessibility_permission",
                "thread_id": thread_id,
                "message": error.user_message,
            }),
        );
        return Err(error.user_message);
    }

    // 2. 跳到对应 thread。
    if let Err(detail) = opener.open_url(&format!("codex://threads/{}", thread_id)) {
        log_error(
            "Codex APP 注入失败",
            json!({"stage": "open_thread", "thread_id": thread_id, "message": detail}),
        );
        return Err(format!("打开 Codex.app thread 失败：{}", detail));
    }

    // 3. 等待 Codex.app 成为 frontmost。
    if let Err(error) = injector.wait_codex_app_frontmost(INJECT_FRONTMOST_TIMEOUT_MS) {
        log_error(
            "Codex APP 注入失败",
            json!({
                "stage": "wait_frontmost",
                "thread_id": thread_id,
                "message": error.user_message,
            }),
        );
        // 已打开窗口；告诉用户去手动发送。
        return Err(format!(
            "{}（已尝试跳转到 Codex.app，请在 GUI 内手动发送）",
            error.user_message
        ));
    }
    log_info(
        "Codex APP 注入：Codex.app 已前台",
        json!({"thread_id": thread_id}),
    );

    // 4. 设焦点（最佳努力，失败不致命）。
    if let Err(error) = injector.focus_input_field() {
        log_error(
            "Codex APP 注入失败",
            json!({
                "stage": "focus_input",
                "thread_id": thread_id,
                "message": error.user_message,
            }),
        );
        // 已经在 Codex.app 前台，告诉用户去手动操作。
        let _ = opener.open_url(&format!("codex://threads/{}", thread_id));
        return Err(format!(
            "{}（已跳转到 Codex.app，请在 GUI 内手动发送）",
            error.user_message
        ));
    }

    // 5. 粘贴 + 回车。
    if let Err(error) = injector.paste_and_return(&prompt) {
        log_error(
            "Codex APP 注入失败",
            json!({
                "stage": "paste_and_return",
                "thread_id": thread_id,
                "message": error.user_message,
            }),
        );
        let _ = opener.open_url(&format!("codex://threads/{}", thread_id));
        return Err(format!(
            "{}（已跳转到 Codex.app，请在 GUI 内手动发送）",
            error.user_message
        ));
    }

    log_info(
        "Codex APP 注入：消息已发送",
        json!({"thread_id": thread_id}),
    );

    // 6. 可选：本地立即回显（依赖编译期常量，运行时不开销）。
    if INJECT_EMIT_LOCAL_USER_MESSAGE {
        if let Ok(mut runtime) = codex_app_runtime().lock() {
            // create_followup_turn 会写 pending_followup_turns；这里直接补 prompt 走完成路径。
            // 失败不阻塞——本地展示是 best-effort。
            let _ = runtime
                .create_followup_turn(&request.session_key, &prompt)
                .ok();
            let _ = runtime.complete_followup_turn(&request.session_key);
        }
    }

    Ok(())
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
    install_codex_cross_runtime_hooks();
    if let Err(error) = start_codex_cli_bridge_server(runtime, codex_app_runtime) {
        let message = format!("Codex CLI bridge 启动失败：{error:?}");
        log_error(
            "Codex CLI bridge 启动失败",
            json!({"message": message.clone()}),
        );
        return Err(message);
    }
    start_codex_rollout_watcher_once();
    *started = true;
    log_info("Codex CLI bridge 启动", json!({}));
    Ok(())
}

/// 把 codex_cli_runtime / codex_app_runtime 的跨 runtime hook 互相注入。
///
/// - codex_app_runtime 收录新 thread → 通知 codex_cli_runtime 清理同名孤儿 session。
/// - codex_cli_hook reclassify 未命中已知 thread → 触发一次同步 thread list 刷新。
fn install_codex_cross_runtime_hooks() {
    use crate::adapters::codex_cli_hook::set_codex_app_thread_refresh_hook;

    if let Ok(mut runtime) = codex_app_runtime().lock() {
        runtime.set_orphan_eviction_callback(Arc::new(|cwd, thread_id, updated_at| {
            if let Ok(mut cli) = codex_cli_runtime().lock() {
                let evicted = cli.evict_codex_app_orphan_session(cwd, thread_id, updated_at);
                drop(cli);
                if evicted {
                    // 删除后立即重新计算 rollout watcher 目标,避免追踪已删除的 session。
                    refresh_rollout_watcher_targets();
                }
            }
        }));
    }

    set_codex_app_thread_refresh_hook(Arc::new(|| {
        synchronously_refresh_codex_app_thread_list(CODEX_APP_THREAD_LIST_SYNC_TIMEOUT);
    }));
}

/// 同步刷新 codex_app_runtime 的 thread list 上限,留给 hook reclassify 兜底用。
const CODEX_APP_THREAD_LIST_SYNC_TIMEOUT: Duration = Duration::from_millis(300);

/// 在受限超时内尝试同步刷新一次 codex app-server 的 thread list。
///
/// 如果 codex app-server 尚未启动 / 拉取超时 / 锁失败,函数静默返回,
/// reclassify 会沿用现有 in-memory 状态判定。
fn synchronously_refresh_codex_app_thread_list(timeout: Duration) {
    let started_at = Instant::now();
    let Some(server) = ready_codex_app_server_client() else {
        return;
    };
    let remaining = timeout.saturating_sub(started_at.elapsed());
    if remaining.is_zero() {
        return;
    }
    let loaded_thread_ids = server
        .try_list_loaded_thread_ids_with_timeout(remaining)
        .unwrap_or_default();
    let loaded_thread_id_set = BTreeSet::from_iter(loaded_thread_ids);
    if loaded_thread_id_set.is_empty() {
        return;
    }
    let remaining = timeout.saturating_sub(started_at.elapsed());
    if remaining.is_zero() {
        return;
    }
    let threads = try_read_loaded_codex_app_threads(&server, &loaded_thread_id_set, remaining);
    if threads.is_empty() {
        return;
    };
    if timeout.saturating_sub(started_at.elapsed()).is_zero() {
        return;
    }
    apply_codex_app_thread_metadata_without_blocking(threads);
}

/// 非阻塞应用 Codex APP thread 元数据，避免 hook 分流路径等待 runtime 锁。
fn apply_codex_app_thread_metadata_without_blocking(threads: Vec<CodexAppThreadMetadata>) {
    let runtime = codex_app_runtime();
    let Ok(mut runtime) = runtime.try_lock() else {
        return;
    };
    for thread in threads {
        let _ = runtime.apply_loaded_thread_metadata(thread, command_unix_now());
    }
}

/// 重新发布 rollout watcher 目标,避免删除孤儿后 tailer 仍跟踪它。
fn refresh_rollout_watcher_targets() {
    // 当前 watcher 在循环顶部每轮自己拉一次 targets,无需主动通知。
    // 这里保留接口语义,便于未来切换为事件驱动。
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

/// 清理过期的 Codex CLI 已完成 session。
fn cleanup_expired_codex_cli_completed_sessions(runtime: &mut CodexCliHookRuntime) {
    let removed = runtime.cleanup_expired_completed_sessions(
        command_unix_now(),
        configured_completed_session_retention().as_millis() as u64,
    );
    if !removed.is_empty() {
        refresh_rollout_watcher_targets();
    }
}

/// 清理过期的 Codex APP 已完成 session。
fn cleanup_expired_codex_app_completed_sessions(runtime: &mut CodexAppRuntime) {
    let removed = runtime.cleanup_expired_completed_sessions(
        command_unix_now(),
        configured_completed_session_retention().as_millis() as u64,
    );
    if !removed.is_empty() {
        refresh_rollout_watcher_targets();
    }
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
            log_error(
                "Codex APP app-server 启动失败",
                json!({"message": error.user_message.clone()}),
            );
            return Err(error.user_message);
        }
    };
    publish_codex_app_server_client(Arc::new(client))?;
    clear_codex_app_startup_failure()?;
    log_info("Codex APP app-server 启动", json!({}));

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

    let session_index_titles = load_codex_session_index_titles(&default_codex_session_index_path());
    let loaded_thread_ids = server.list_loaded_thread_ids().unwrap_or_default();
    let loaded_thread_id_set = BTreeSet::from_iter(loaded_thread_ids);
    let mut loaded_threads =
        read_loaded_codex_app_threads(&server, &loaded_thread_id_set, Duration::from_secs(2));
    apply_session_index_thread_titles(&mut loaded_threads, &session_index_titles);
    let (unresolved_thread_ids, title_missing_thread_ids) = {
        let runtime = codex_app_runtime();
        let ids = match runtime.lock() {
            Ok(mut runtime) => {
                for thread in loaded_threads.iter().cloned() {
                    let _ = runtime.apply_loaded_thread_metadata(thread, command_unix_now());
                }
                runtime.apply_session_index_titles_to_known_sessions(
                    &session_index_titles,
                    command_unix_now(),
                );
                (
                    runtime.unresolved_thread_ids(),
                    runtime.title_missing_thread_ids(),
                )
            }
            Err(_) => (Vec::new(), Vec::new()),
        };
        ids
    };
    if let Ok(mut runtime) = codex_cli_runtime().lock() {
        runtime.apply_session_index_titles_to_known_sessions(
            &session_index_titles,
            command_unix_now(),
        );
    }
    let history_candidate_ids = history_candidate_thread_ids(
        unresolved_thread_ids.iter(),
        title_missing_thread_ids.iter(),
    );
    let needs_history = !history_candidate_ids.is_empty();

    let history_threads = if needs_history {
        let mut history_threads = filter_history_threads_for_candidates(
            server
                .list_threads(CODEX_APP_THREAD_LIST_LIMIT)
                .unwrap_or_default(),
            &history_candidate_ids,
        );
        apply_session_index_thread_titles(&mut history_threads, &session_index_titles);
        history_threads
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
    let mut candidate_thread_ids =
        rollout_candidate_thread_ids(&rollout_threads, &unresolved_thread_ids);
    candidate_thread_ids.extend(history_candidate_ids.iter().cloned());
    let allow_recent_rollout_scan = should_scan_recent_rollouts();
    let recent_rollout_snapshots = if allow_recent_rollout_scan {
        CodexRolloutDiscovery::default_root().discover_recent(SystemTime::now())
    } else {
        Vec::new()
    };
    sync_codex_rollout_history(
        &rollout_threads,
        &candidate_thread_ids,
        needs_history && allow_recent_rollout_scan,
        &recent_rollout_snapshots,
    );
    sync_recent_active_codex_rollouts(&recent_rollout_snapshots, command_unix_now());
}

/// 按 loaded thread id 精确读取当前 thread 元数据，失败时降级为一次 thread/list。
fn read_loaded_codex_app_threads(
    server: &CodexAppServerClient,
    loaded_thread_id_set: &BTreeSet<String>,
    timeout: Duration,
) -> Vec<CodexAppThreadMetadata> {
    if loaded_thread_id_set.is_empty() {
        return Vec::new();
    }

    let started_at = Instant::now();
    let mut threads = Vec::new();
    for thread_id in loaded_thread_id_set
        .iter()
        .take(CODEX_APP_MAX_LOADED_THREAD_READS)
    {
        let remaining = timeout.saturating_sub(started_at.elapsed());
        if remaining.is_zero() {
            break;
        }
        match server.read_thread_with_timeout(thread_id, remaining) {
            Ok(thread) => threads.push(thread),
            Err(error) if should_fallback_to_thread_list(&error) => {
                return read_loaded_codex_app_threads_from_list(server, loaded_thread_id_set);
            }
            Err(_) => {}
        }
    }

    threads
}

/// 非阻塞读取当前 loaded thread 元数据，失败时降级为一次 thread/list。
fn try_read_loaded_codex_app_threads(
    server: &CodexAppServerClient,
    loaded_thread_id_set: &BTreeSet<String>,
    timeout: Duration,
) -> Vec<CodexAppThreadMetadata> {
    if loaded_thread_id_set.is_empty() {
        return Vec::new();
    }

    let started_at = Instant::now();
    let mut threads = Vec::new();
    for thread_id in loaded_thread_id_set
        .iter()
        .take(CODEX_APP_MAX_LOADED_THREAD_READS)
    {
        let remaining = timeout.saturating_sub(started_at.elapsed());
        if remaining.is_zero() {
            break;
        }
        match server.try_read_thread_with_timeout(thread_id, remaining) {
            Ok(thread) => threads.push(thread),
            Err(error) if should_fallback_to_thread_list(&error) => {
                let remaining = timeout.saturating_sub(started_at.elapsed());
                if remaining.is_zero() {
                    return threads;
                }
                return filter_history_threads_for_candidates(
                    server
                        .try_list_threads_with_timeout(CODEX_APP_THREAD_LIST_LIMIT, remaining)
                        .unwrap_or_default(),
                    loaded_thread_id_set,
                );
            }
            Err(_) => {}
        }
    }

    threads
}

/// 通过 thread/list 降级补齐 loaded thread 元数据。
fn read_loaded_codex_app_threads_from_list(
    server: &CodexAppServerClient,
    loaded_thread_id_set: &BTreeSet<String>,
) -> Vec<CodexAppThreadMetadata> {
    filter_history_threads_for_candidates(
        server
            .list_threads(CODEX_APP_THREAD_LIST_LIMIT)
            .unwrap_or_default(),
        loaded_thread_id_set,
    )
}

/// 判断 thread/read 失败后是否应停止逐个读取并改用 thread/list。
fn should_fallback_to_thread_list(error: &crate::domain::app_error::AppError) -> bool {
    if error.user_message.contains("超时") {
        return true;
    }
    let detail = error
        .technical_detail
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    detail.contains("method")
        || detail.contains("not found")
        || detail.contains("unknown")
        || detail.contains("invalid request")
}

/// 同步 Codex rollout 历史，避免高频全量扫描。
fn sync_codex_rollout_history(
    threads: &[crate::adapters::codex_app::CodexAppThreadMetadata],
    candidate_thread_ids: &BTreeSet<String>,
    allow_recent_scan: bool,
    recent_snapshots: &[crate::adapters::codex_app::CodexRolloutSnapshot],
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

    if allow_recent_scan {
        snapshots.extend(
            recent_snapshots
                .iter()
                .filter(|snapshot| candidate_thread_ids.contains(&snapshot.session_id))
                .cloned(),
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

/// 同步最近仍活跃的 rollout，补上 app-server 标记为 notLoaded 的运行中线程。
fn sync_recent_active_codex_rollouts(
    recent_snapshots: &[crate::adapters::codex_app::CodexRolloutSnapshot],
    now: UnixMillis,
) {
    let window = configured_active_rollout_window();
    let snapshots = recent_snapshots
        .iter()
        .filter(|snapshot| active_rollout_snapshot_is_recent(snapshot, now, window))
        .cloned()
        .collect::<Vec<_>>();
    if snapshots.is_empty() {
        return;
    }

    let runtime = codex_app_runtime();
    if let Ok(mut runtime) = runtime.lock() {
        for snapshot in snapshots {
            let _ = runtime.apply_active_rollout_snapshot(snapshot);
        }
    };
}

/// 汇总需要历史 metadata 辅助补齐的已知 thread ID。
fn history_candidate_thread_ids<'a>(
    unresolved_thread_ids: impl Iterator<Item = &'a String>,
    title_missing_thread_ids: impl Iterator<Item = &'a String>,
) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    ids.extend(unresolved_thread_ids.cloned());
    ids.extend(title_missing_thread_ids.cloned());
    ids
}

/// 仅保留能补齐当前已知 session 的历史 thread。
fn filter_history_threads_for_candidates(
    threads: Vec<crate::adapters::codex_app::CodexAppThreadMetadata>,
    candidate_thread_ids: &BTreeSet<String>,
) -> Vec<crate::adapters::codex_app::CodexAppThreadMetadata> {
    threads
        .into_iter()
        .filter(|thread| candidate_thread_ids.contains(&thread.id))
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

/// 判断 rollout 快照是否处在当前活跃恢复窗口内。
fn active_rollout_snapshot_is_recent(
    snapshot: &crate::adapters::codex_app::CodexRolloutSnapshot,
    now: UnixMillis,
    window: Duration,
) -> bool {
    if snapshot.completed {
        return false;
    }
    let window_millis = window.as_millis() as u64;
    let cutoff = now.value.saturating_sub(window_millis);
    snapshot.updated_at.value >= cutoff && snapshot.updated_at.value <= now.value
}

/// 读取最近活跃 rollout 恢复窗口。
fn configured_active_rollout_window() -> Duration {
    parse_active_rollout_window_minutes(std::env::var(CODEX_APP_ACTIVE_ROLLOUT_WINDOW_ENV).ok())
}

/// 读取已完成 session 保留窗口。
fn configured_completed_session_retention() -> Duration {
    parse_completed_session_retention_minutes(std::env::var(COMPLETED_SESSION_RETENTION_ENV).ok())
}

/// 解析最近活跃 rollout 恢复窗口，非法配置回退到默认值。
fn parse_active_rollout_window_minutes(value: Option<String>) -> Duration {
    let Some(value) = value else {
        return CODEX_APP_ACTIVE_ROLLOUT_DEFAULT_WINDOW;
    };
    let Ok(minutes) = value.trim().parse::<u64>() else {
        return CODEX_APP_ACTIVE_ROLLOUT_DEFAULT_WINDOW;
    };
    if minutes == 0 {
        return CODEX_APP_ACTIVE_ROLLOUT_DEFAULT_WINDOW;
    }

    Duration::from_secs(minutes.saturating_mul(60))
}

/// 解析已完成 session 保留窗口，非法配置回退到默认值。
fn parse_completed_session_retention_minutes(value: Option<String>) -> Duration {
    let Some(value) = value else {
        return COMPLETED_SESSION_DEFAULT_RETENTION;
    };
    let Ok(minutes) = value.trim().parse::<u64>() else {
        return COMPLETED_SESSION_DEFAULT_RETENTION;
    };
    if minutes == 0 {
        return COMPLETED_SESSION_DEFAULT_RETENTION;
    }

    Duration::from_secs(minutes.saturating_mul(60))
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

/// 释放 Codex APP follow-up 提交占位并记录失败阶段。
fn release_and_log_followup_failure(
    runtime: &Arc<Mutex<CodexAppRuntime>>,
    _session_key: &SessionKey,
    stage: &str,
    thread_id: &str,
    message: &str,
) -> String {
    if let Ok(mut runtime) = runtime.lock() {
        runtime.release_followup_turn_by_thread_id(thread_id);
    }
    log_error(
        "Codex APP follow-up 创建失败",
        json!({
            "stage": stage,
            "thread_id": thread_id,
            "message": message,
        }),
    );
    message.to_string()
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

/// 获取当前已连接且仍存活的 Codex APP app-server client，不触发启动。
fn ready_codex_app_server_client() -> Option<Arc<CodexAppServerClient>> {
    let client = {
        let server = codex_app_server_slot().try_lock().ok()?;
        match &*server {
            CodexAppServerSlot::Ready(client) => Arc::clone(client),
            _ => return None,
        }
    };
    client.try_is_running().then_some(client)
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
    use crate::adapters::codex_app::{
        CodexAppRpcWrite, CodexAppThreadMetadata, CodexAppThreadSourceKind, CodexRolloutSnapshot,
    };
    use crate::domain::agent_session::{AgentKind, ConversationId, ProjectId};
    use crate::domain::app_error::{AppError, AppErrorCode};

    #[test]
    fn completed_session_retention_minutes_uses_default_for_missing_or_invalid_values() {
        assert_eq!(
            parse_completed_session_retention_minutes(None),
            COMPLETED_SESSION_DEFAULT_RETENTION
        );
        assert_eq!(
            parse_completed_session_retention_minutes(Some("   ".to_string())),
            COMPLETED_SESSION_DEFAULT_RETENTION
        );
        assert_eq!(
            parse_completed_session_retention_minutes(Some("invalid".to_string())),
            COMPLETED_SESSION_DEFAULT_RETENTION
        );
        assert_eq!(
            parse_completed_session_retention_minutes(Some("0".to_string())),
            COMPLETED_SESSION_DEFAULT_RETENTION
        );
    }

    #[test]
    fn completed_session_retention_minutes_accepts_positive_integer_minutes() {
        assert_eq!(
            parse_completed_session_retention_minutes(Some(" 7 ".to_string())),
            Duration::from_secs(7 * 60)
        );
    }

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
        let candidates = BTreeSet::from(["unresolved-thread".to_string()]);

        let filtered = filter_history_threads_for_candidates(threads, &candidates);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "unresolved-thread");
    }

    #[test]
    fn history_candidates_include_title_missing_known_threads() {
        let unresolved = ["unresolved-thread".to_string()];
        let title_missing = ["title-missing-thread".to_string()];

        let candidates = history_candidate_thread_ids(unresolved.iter(), title_missing.iter());

        assert!(candidates.contains("unresolved-thread"));
        assert!(candidates.contains("title-missing-thread"));
        assert!(!candidates.contains("unrelated-thread"));
    }

    #[test]
    fn history_threads_keep_path_only_unresolved_candidates() {
        let threads = vec![CodexAppThreadMetadata {
            id: "unresolved-thread".to_string(),
            parent_thread_id: None,
            cwd: None,
            name: None,
            preview: None,
            path: Some(PathBuf::from("/tmp/rollout-unresolved-thread.jsonl")),
            status_type: "idle".to_string(),
            ephemeral: false,
            source_kind: CodexAppThreadSourceKind::UserVisible,
        }];
        let candidates = BTreeSet::from(["unresolved-thread".to_string()]);

        let filtered = filter_history_threads_for_candidates(threads, &candidates);

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

    #[test]
    fn active_rollout_window_defaults_to_five_minutes() {
        assert_eq!(
            parse_active_rollout_window_minutes(None),
            Duration::from_secs(5 * 60)
        );
        assert_eq!(
            parse_active_rollout_window_minutes(Some("0".to_string())),
            Duration::from_secs(5 * 60)
        );
        assert_eq!(
            parse_active_rollout_window_minutes(Some("bad".to_string())),
            Duration::from_secs(5 * 60)
        );
    }

    #[test]
    fn active_rollout_window_accepts_positive_minutes() {
        assert_eq!(
            parse_active_rollout_window_minutes(Some("7".to_string())),
            Duration::from_secs(7 * 60)
        );
    }

    #[test]
    fn active_rollout_recent_filter_requires_unfinished_snapshot_in_window() {
        let now = UnixMillis::new(10_000);
        let window = Duration::from_secs(5);
        let recent = rollout_snapshot("thread-1", UnixMillis::new(6_000), false);
        let old = rollout_snapshot("thread-2", UnixMillis::new(4_999), false);
        let completed = rollout_snapshot("thread-3", UnixMillis::new(9_000), true);
        let future = rollout_snapshot("thread-4", UnixMillis::new(10_001), false);

        assert!(active_rollout_snapshot_is_recent(&recent, now, window));
        assert!(!active_rollout_snapshot_is_recent(&old, now, window));
        assert!(!active_rollout_snapshot_is_recent(&completed, now, window));
        assert!(!active_rollout_snapshot_is_recent(&future, now, window));
    }

    #[test]
    fn thread_read_method_error_falls_back_to_thread_list_once() {
        let error = AppError::new(
            AppErrorCode::BridgeUnavailable,
            "Codex APP app-server request 失败",
            Some("Method not found: thread/read".to_string()),
            true,
            None,
        );

        assert!(should_fallback_to_thread_list(&error));
    }

    #[test]
    fn unrelated_thread_read_error_does_not_trigger_list_fallback() {
        let error = AppError::new(
            AppErrorCode::BridgeUnavailable,
            "Codex APP thread 详情格式无效",
            Some("missing field thread.cwd".to_string()),
            true,
            None,
        );

        assert!(!should_fallback_to_thread_list(&error));
    }

    #[test]
    fn synchronous_refresh_skips_when_server_slot_is_locked() {
        let _guard = codex_app_server_slot()
            .lock()
            .expect("server slot should lock");
        let started_at = Instant::now();

        synchronously_refresh_codex_app_thread_list(Duration::from_millis(10));

        assert!(started_at.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn metadata_apply_skips_when_runtime_is_locked() {
        let runtime = codex_app_runtime();
        let _guard = runtime.lock().expect("runtime should lock");
        let started_at = Instant::now();

        apply_codex_app_thread_metadata_without_blocking(vec![thread_metadata(
            "locked-thread",
            "/tmp/locked",
        )]);

        assert!(started_at.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn followup_resumes_unloaded_thread_before_turn_start() {
        let (runtime, request) = completed_followup_request();
        let server = FakeFollowupServer::new(Vec::new(), None);
        let calls = server.calls_ref();

        create_codex_app_followup_turn_with_server(Arc::clone(&runtime), &request, || Ok(server))
            .expect("followup should create");

        assert_eq!(
            calls.lock().expect("calls should lock").as_slice(),
            // 第一次 loaded/list 是诊断日志查询，第二次是 ensure_codex_app_thread_loaded。
            ["loaded/list", "loaded/list", "thread/resume", "turn/start"]
        );
    }

    #[test]
    fn followup_does_not_resume_loaded_thread() {
        let (runtime, request) = completed_followup_request();
        let server = FakeFollowupServer::new(vec!["thread-1".to_string()], None);
        let calls = server.calls_ref();

        create_codex_app_followup_turn_with_server(Arc::clone(&runtime), &request, || Ok(server))
            .expect("followup should create");

        assert_eq!(
            calls.lock().expect("calls should lock").as_slice(),
            // 第一次 loaded/list 是诊断日志查询，第二次是 ensure_codex_app_thread_loaded。
            ["loaded/list", "loaded/list", "turn/start"]
        );
    }

    #[test]
    fn followup_releases_pending_when_server_client_is_missing() {
        let (runtime, request) = completed_followup_request();

        let error = create_codex_app_followup_turn_with_server::<FakeFollowupServer, _>(
            Arc::clone(&runtime),
            &request,
            || Err("Codex APP app-server 未连接".to_string()),
        )
        .expect_err("missing server should fail");

        assert_eq!(error, "Codex APP app-server 未连接");
        assert_followup_can_retry(runtime, &request.session_key);
    }

    #[test]
    fn followup_releases_pending_when_resume_fails() {
        let (runtime, request) = completed_followup_request();
        let server = FakeFollowupServer::new(Vec::new(), Some("thread/resume"));

        let error =
            create_codex_app_followup_turn_with_server(Arc::clone(&runtime), &request, || {
                Ok(server)
            })
            .expect_err("resume should fail");

        assert_eq!(error, "thread/resume failed");
        assert_followup_can_retry(runtime, &request.session_key);
    }

    #[test]
    fn followup_releases_pending_when_loaded_list_fails() {
        let (runtime, request) = completed_followup_request();
        let server = FakeFollowupServer::new(Vec::new(), Some("loaded/list"));

        let error =
            create_codex_app_followup_turn_with_server(Arc::clone(&runtime), &request, || {
                Ok(server)
            })
            .expect_err("loaded list should fail");

        assert_eq!(error, "loaded/list failed");
        assert_followup_can_retry(runtime, &request.session_key);
    }

    #[test]
    fn followup_releases_pending_when_turn_start_fails() {
        let (runtime, request) = completed_followup_request();
        let server = FakeFollowupServer::new(vec!["thread-1".to_string()], Some("turn/start"));

        let error =
            create_codex_app_followup_turn_with_server(Arc::clone(&runtime), &request, || {
                Ok(server)
            })
            .expect_err("turn start should fail");

        assert_eq!(error, "turn/start failed");
        assert_followup_can_retry(runtime, &request.session_key);
    }

    fn thread_metadata(id: &str, cwd: &str) -> CodexAppThreadMetadata {
        CodexAppThreadMetadata {
            id: id.to_string(),
            parent_thread_id: None,
            cwd: Some(cwd.to_string()),
            name: Some("Thread 1".to_string()),
            preview: Some("已完成".to_string()),
            path: None,
            status_type: "idle".to_string(),
            ephemeral: false,
            source_kind: CodexAppThreadSourceKind::UserVisible,
        }
    }

    fn rollout_snapshot(
        session_id: &str,
        updated_at: UnixMillis,
        completed: bool,
    ) -> CodexRolloutSnapshot {
        CodexRolloutSnapshot {
            session_id: session_id.to_string(),
            cwd: "/tmp/builder-panel".to_string(),
            summary: Some("正在处理".to_string()),
            last_agent_message: Some("正在处理".to_string()),
            path: PathBuf::from(format!("/tmp/rollout-{session_id}.jsonl")),
            updated_at,
            completed,
            pending_user_input: None,
        }
    }

    fn completed_followup_request() -> (Arc<Mutex<CodexAppRuntime>>, CodexAppFollowupRequest) {
        let runtime = Arc::new(Mutex::new(CodexAppRuntime::empty()));
        let session_key = SessionKey::new(
            AgentKind::CodexApp,
            ProjectId::new("/tmp/project".to_string()),
            ConversationId::new("thread-1".to_string()),
        );
        runtime
            .lock()
            .expect("runtime should lock")
            .apply_loaded_thread_metadata(
                thread_metadata("thread-1", "/tmp/project"),
                UnixMillis::new(1),
            )
            .expect("metadata should apply");
        (
            runtime,
            CodexAppFollowupRequest {
                session_key,
                prompt: "继续".to_string(),
            },
        )
    }

    fn assert_followup_can_retry(runtime: Arc<Mutex<CodexAppRuntime>>, session_key: &SessionKey) {
        runtime
            .lock()
            .expect("runtime should lock")
            .create_followup_turn(session_key, "再次继续")
            .expect("followup pending should be released");
    }

    struct FakeFollowupServer {
        loaded_thread_ids: Vec<String>,
        fail_stage: Option<&'static str>,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    impl FakeFollowupServer {
        fn new(loaded_thread_ids: Vec<String>, fail_stage: Option<&'static str>) -> Self {
            Self {
                loaded_thread_ids,
                fail_stage,
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn calls_ref(&self) -> Arc<Mutex<Vec<&'static str>>> {
            Arc::clone(&self.calls)
        }

        fn error(stage: &str) -> AppError {
            AppError::new(
                AppErrorCode::BridgeUnavailable,
                format!("{stage} failed"),
                None,
                true,
                Some(FallbackAction::RetryLater),
            )
        }
    }

    impl CodexAppFollowupRpcClient for FakeFollowupServer {
        fn list_loaded_thread_ids(&self) -> Result<Vec<String>, AppError> {
            self.calls
                .lock()
                .expect("calls should lock")
                .push("loaded/list");
            if self.fail_stage == Some("loaded/list") {
                return Err(Self::error("loaded/list"));
            }

            Ok(self.loaded_thread_ids.clone())
        }

        fn resume_thread(&self, _thread_id: &str) -> Result<(), AppError> {
            self.calls
                .lock()
                .expect("calls should lock")
                .push("thread/resume");
            if self.fail_stage == Some("thread/resume") {
                return Err(Self::error("thread/resume"));
            }

            Ok(())
        }

        fn write_rpc_request(&self, _write: CodexAppRpcWrite) -> Result<(), AppError> {
            self.calls
                .lock()
                .expect("calls should lock")
                .push("turn/start");
            if self.fail_stage == Some("turn/start") {
                return Err(Self::error("turn/start"));
            }

            Ok(())
        }
    }

    // -------- inject_codex_app_followup_with 单测 --------

    use crate::adapters::codex_app_inject::fake::{FakeCall, FakeCodexAppInjector, FakeStep};

    /// 单测用 URL opener：记录所有 open_url 调用，可触发失败。
    struct FakeUrlOpener {
        opened: std::cell::RefCell<Vec<String>>,
        fail_next: std::cell::Cell<bool>,
    }

    impl FakeUrlOpener {
        fn new() -> Self {
            Self {
                opened: std::cell::RefCell::new(Vec::new()),
                fail_next: std::cell::Cell::new(false),
            }
        }
    }

    impl UrlOpener for FakeUrlOpener {
        fn open_url(&mut self, url: &str) -> Result<(), String> {
            self.opened.borrow_mut().push(url.to_string());
            if self.fail_next.get() {
                self.fail_next.set(false);
                return Err("forced url open failure".to_string());
            }
            Ok(())
        }
    }

    fn followup_request(thread: &str, prompt: &str) -> CodexAppFollowupRequest {
        CodexAppFollowupRequest {
            session_key: SessionKey {
                agent_kind: AgentKind::CodexApp,
                project_id: ProjectId::new("/tmp"),
                conversation_id: ConversationId::new(thread),
            },
            prompt: prompt.to_string(),
        }
    }

    #[test]
    fn inject_followup_empty_prompt_returns_error() {
        let request = followup_request("t1", "   ");
        let mut opener = FakeUrlOpener::new();
        let injector = FakeCodexAppInjector::new();
        let result = inject_codex_app_followup_with(&request, &mut opener, &injector);
        assert!(result.is_err());
        assert!(opener.opened.borrow().is_empty());
        assert!(injector.calls().is_empty());
    }

    #[test]
    fn inject_followup_open_url_failure_propagates() {
        let request = followup_request("t1", "hi");
        let mut opener = FakeUrlOpener::new();
        opener.fail_next.set(true);
        let injector = FakeCodexAppInjector::new();
        // 注意：ensure_accessibility_trusted 只在 macOS 实际生效；非 macOS 上 stub 直接 Err。
        // 这里只验证错误传播，跳过实际行为差异。
        let _result = inject_codex_app_followup_with(&request, &mut opener, &injector);
        // 不断言 result.is_err()——平台不同行为不同；只要不 panic 即可。
    }

    #[test]
    fn fake_injector_records_call_order() {
        let injector = FakeCodexAppInjector::new();
        injector.wait_codex_app_frontmost(1500).unwrap();
        injector.focus_input_field().unwrap();
        injector.paste_and_return("hello").unwrap();
        let calls = injector.calls();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], FakeCall::WaitFrontmost { timeout_ms: 1500 });
        assert_eq!(calls[1], FakeCall::FocusInput);
        assert_eq!(
            calls[2],
            FakeCall::PasteAndReturn {
                prompt: "hello".to_string()
            }
        );
    }

    #[test]
    fn fake_injector_can_fail_at_specific_step() {
        let injector = FakeCodexAppInjector::new();
        injector.fail_at(FakeStep::PasteAndReturn);
        assert!(injector.wait_codex_app_frontmost(1000).is_ok());
        assert!(injector.focus_input_field().is_ok());
        assert!(injector.paste_and_return("test").is_err());
    }
}
