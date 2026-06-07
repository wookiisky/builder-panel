//! Builder Panel Tauri 后端入口。

pub mod adapters;
pub mod domain;
pub mod ports;
pub mod services;
pub mod tauri_api;

/// 启动 Builder Panel 桌面应用。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            tauri_api::commands::query_codex_cli_timeline,
            tauri_api::commands::query_codex_app_timeline,
            tauri_api::commands::release_codex_cli_timeline_cache,
            tauri_api::commands::release_codex_app_timeline_cache
        ])
        .run(tauri::generate_context!())
        .expect("启动 Builder Panel Tauri 应用失败");
}
