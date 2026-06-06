//! Builder Panel hook helper 入口。

use std::io::{Read, Write};

use builder_panel_lib::adapters::bridge::hook_cli::run_hook_cli;

/// 执行 hook helper，所有失败路径均由运行逻辑 fail-open 收敛。
fn main() {
    let mut stdin = Vec::new();
    if std::io::stdin().read_to_end(&mut stdin).is_err() {
        return;
    }

    let arguments = std::env::args().skip(1).collect::<Vec<String>>();
    let run = run_hook_cli(&arguments, &stdin);

    if !run.stdout.is_empty() {
        let _ = std::io::stdout().write_all(&run.stdout);
    }

    if !run.stderr.is_empty() {
        let _ = std::io::stderr().write_all(&run.stderr);
    }

    std::process::exit(run.exit_code);
}
