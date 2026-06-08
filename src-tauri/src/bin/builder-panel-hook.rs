//! Builder Panel hook helper 入口。

use std::io::{Read, Write};

use builder_panel_lib::adapters::bridge::hook_cli::run_hook_cli;
use builder_panel_lib::adapters::config_file::JsonSettingsStore;
use builder_panel_lib::adapters::logging::{
    default_log_path, event_logger, log_error, log_info,
};
use builder_panel_lib::ports::config_store_port::SettingsStorePort;
use serde_json::json;

/// 执行 hook helper，所有失败路径均由运行逻辑 fail-open 收敛。
fn main() {
    initialize_event_logger();

    let mut stdin = Vec::new();
    if std::io::stdin().read_to_end(&mut stdin).is_err() {
        log_error("hook helper 读取 stdin 失败", json!({}));
        return;
    }

    let arguments = std::env::args().skip(1).collect::<Vec<String>>();
    log_info(
        "hook helper 调用",
        json!({
            "args": arguments.clone(),
            "stdin_bytes": stdin.len(),
        }),
    );
    let run = run_hook_cli(&arguments, &stdin);

    if !run.stdout.is_empty() {
        let _ = std::io::stdout().write_all(&run.stdout);
    }

    if !run.stderr.is_empty() {
        let _ = std::io::stderr().write_all(&run.stderr);
    }

    if run.exit_code != 0 {
        log_error(
            "hook helper 退出非零",
            json!({
                "exit_code": run.exit_code,
                "stderr_bytes": run.stderr.len(),
            }),
        );
    } else {
        log_info("hook helper 完成", json!({"exit_code": run.exit_code}));
    }

    std::process::exit(run.exit_code);
}

/// 按持久化设置初始化全局事件日志器。
fn initialize_event_logger() {
    let enabled = JsonSettingsStore::default_path()
        .load_settings()
        .ok()
        .flatten()
        .map(|settings| settings.logging.enabled)
        .unwrap_or(false);
    event_logger().configure(enabled, Some(default_log_path()));
}
