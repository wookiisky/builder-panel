//! Builder Panel Tauri 后端入口。

use std::sync::Arc;

pub mod adapters;
pub mod domain;
pub mod ports;
pub mod services;
pub mod tauri_api;

/// 启动 Builder Panel 桌面应用。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    initialize_event_logger();
    tauri::Builder::default()
        .setup(|app| {
            tauri_api::commands::configure_session_update_sink(Arc::new(
                tauri_api::events::TauriSessionUpdateSink::new(app.handle().clone()),
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            tauri_api::commands::get_panel_probe,
            tauri_api::commands::get_panel_settings,
            tauri_api::commands::save_panel_settings,
            tauri_api::commands::save_panel_window_state,
            tauri_api::commands::preview_hook_install,
            tauri_api::commands::get_hook_install_status,
            tauri_api::commands::install_hooks,
            tauri_api::commands::uninstall_hooks,
            tauri_api::commands::get_codex_cli_sessions,
            tauri_api::commands::get_codex_cli_session_detail,
            tauri_api::commands::get_codex_app_sessions,
            tauri_api::commands::get_codex_app_session_detail,
            tauri_api::commands::jump_to_session,
            tauri_api::commands::probe_codex_app_schema,
            tauri_api::commands::resolve_codex_cli_approval,
            tauri_api::commands::resolve_codex_app_approval,
            tauri_api::commands::submit_codex_app_choice,
            tauri_api::commands::send_codex_app_reply,
            tauri_api::commands::create_codex_app_followup_turn,
            tauri_api::commands::inject_codex_app_followup,
            tauri_api::commands::get_log_info,
            tauri_api::commands::open_log_folder
        ])
        .run(tauri::generate_context!())
        .expect("启动 Builder Panel Tauri 应用失败");
}

/// 按持久化设置初始化全局事件日志器。
fn initialize_event_logger() {
    use adapters::config_file::JsonSettingsStore;
    use adapters::logging::{default_log_path, event_logger};
    use ports::config_store_port::SettingsStorePort;

    let enabled = JsonSettingsStore::default_path()
        .load_settings()
        .ok()
        .flatten()
        .map(|settings| settings.logging.enabled)
        .unwrap_or(false);
    event_logger().configure(enabled, Some(default_log_path()));
}
