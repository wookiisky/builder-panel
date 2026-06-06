# Codex CLI

## 职责

本文档记录 Builder Panel 对 Codex CLI 的阶段 4 接入事实。

本文档不记录 Claude Code CLI，不记录 Codex APP app-server 接入。

## 支持能力

Codex CLI 通过 `builder-panel-hook --source codex` 进入本地 bridge。

hook helper 对 stdin payload 做基础校验后发送本地 bridge command。

Builder Panel APP 读取 Codex CLI session 时会启动 Mac Unix Domain Socket bridge server。

Builder Panel APP 打开期间会定时刷新 Codex CLI session，承接后续真实 hook 事件。

Codex CLI bridge server 只有 bind 成功后才记录为已启动。

Codex CLI bridge server 首次 bind 失败时，后续读取 Codex CLI session 会重试启动。

Codex CLI bridge server 对每个连接启动独立处理线程。

长等待的 `PermissionRequest` 不阻塞 listener 接收后续 hook 请求。

Codex CLI `SessionStart` 会生成运行中的 Codex CLI session。

Codex CLI `UserPromptSubmit` 会更新 session 活动摘要。

Codex CLI `PreToolUse` 和 `PostToolUse` 会更新工具活动摘要。

Codex CLI `PermissionRequest` 会生成 pending approval。

用户在 panel 中允许或拒绝 pending approval 后，bridge 返回 Codex stdout directive。

Codex CLI `PermissionRequest` 等待超时时，runtime 会移除 pending approval 并写入失败 session 状态。

Codex CLI `Stop` 会生成 turn completed 并清理 pending interaction。

Codex CLI hook 事件会写入进程内过程事件 timeline 缓存。

Codex CLI session 具备过程事件能力时，UI 可以展示 timeline 入口。

## 降级能力

bridge 不可用、超时、response 错配或 directive 编码失败时，hook helper fail-open。

Codex CLI 用量字段当前不从 hook payload 推导，展示为不可用。

Codex CLI 当前不支持 panel 直接发送开放性回复。

Codex CLI 当前不支持 panel 创建后续 turn。

Windows Named Pipe 分支保留，但本阶段不做 Windows 本机验证。

阶段 8 提供 Codex hook 安装、备份、manifest 和卸载 adapter。

hook 安装器写入 Codex `hooks.json`，不写入 inline `config.toml` hooks。

hook 安装器不绕过 Codex hook trust review。

## 不支持能力

当前不从 Codex transcript 或 JSONL 反读历史过程事件。

当前不处理 Codex `Notification` 或 `SessionEnd` 事件。

当前不承诺多个 Builder Panel 进程同时监听同一个 bridge socket。

## 协议事实入口

Codex hooks 使用 `hooks` 作为规范 feature key。

Codex 支持 `commandWindows` 作为 Windows 专用 hook command 覆盖。

Codex hook stdout directive 由本项目测试固定，不由 Domain 推导。

## 代码入口

`src-tauri/src/adapters/bridge/hook_payload.rs` 是 Codex hook payload 基础校验入口。

`src-tauri/src/adapters/bridge/hook_output.rs` 是 Codex stdout directive 编码入口。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 是 Codex CLI hook 事件转换、runtime 和 bridge server 入口。

`src-tauri/src/adapters/timeline/mod.rs` 是 Codex CLI 过程事件 timeline 内存缓存入口。

`src-tauri/src/adapters/hook_install/mod.rs` 是 Codex hook 安装和卸载入口。

`src-tauri/src/tauri_api/commands.rs` 是 Codex CLI session 读取和审批 command 入口。

`src/api/codexCliPanelApi.ts` 是前端 Codex CLI command 调用入口。

## 相关测试

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 覆盖 Codex hook 事件转换、非阻塞 ack 和审批 directive 等待。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 覆盖审批等待超时清理 pending approval。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 覆盖 Codex CLI hook 事件写入 timeline 缓存。

`src-tauri/src/adapters/timeline/mod.rs` 覆盖 timeline 去重、淘汰和释放。

`src-tauri/src/adapters/bridge/transport.rs` 覆盖长请求等待时 listener 仍可接收后续请求。

`src-tauri/src/adapters/bridge/hook_cli.rs` 覆盖 Codex hook helper fail-open 和 directive 输出。

`src-tauri/src/adapters/bridge/hook_payload.rs` 覆盖 Codex payload 基础校验。

`src-tauri/src/adapters/bridge/hook_output.rs` 覆盖 Codex allow 和 deny stdout。

`src-tauri/src/adapters/hook_install/mod.rs` 覆盖 Codex hook 写入、重复安装替换和备份恢复。
