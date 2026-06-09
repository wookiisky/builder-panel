# Codex CLI

## 职责

本文档记录 Builder Panel 对 Codex CLI 的阶段 4 接入事实。

本文档不记录 Claude Code CLI，不记录 Codex APP app-server 接入。

## 支持能力

Codex CLI 通过 `builder-panel-hook --source codex` 进入本地 bridge。

hook helper 对 stdin payload 做基础校验后发送本地 bridge command。

Builder Panel APP 读取 Codex CLI session 时会启动 Mac Unix Domain Socket bridge server。

Builder Panel APP 打开期间会定时刷新 Codex CLI session，承接后续真实 hook 事件。

Codex CLI session 只由当前 APP 进程启动后送达 bridge 的实时 hook 事件创建。


Codex CLI hook payload 携带 transcript path 时，runtime 可把该 path 作为已知 session 的 rollout tail 目标。

已知 Codex CLI session 的 rollout tail 只读取新增追加行，不回放已有历史行。

APP 启动后仍在运行的 Codex CLI 任务如果继续触发 hook 事件，可以进入当前 session 列表。

Codex CLI bridge server 只有 bind 成功后才记录为已启动。

Codex CLI bridge server 首次 bind 失败时，后续读取 Codex CLI session 会重试启动。

Codex CLI bridge server 对每个连接启动独立处理线程。

长等待的 `PermissionRequest` 不阻塞 listener 接收后续 hook 请求。

Codex CLI `SessionStart` 会生成运行中的 Codex CLI session，但不写启动包装摘要。

Codex CLI hook payload 的 `model` 字段不作为 thread 标题；缺少真实标题时显示为未命名。


Codex CLI `PreToolUse` 不写活动摘要。

Codex CLI `PostToolUse` 不写活动摘要。

Codex CLI `Stop` 的最终 assistant 输出按 65535 字符上限保留多段内容。

Codex CLI `PermissionRequest` 会生成 pending approval；有工具 preview 时使用 preview 作为审批请求摘要，但不更新 session 最后消息。

用户在 panel 中允许或拒绝 pending approval 后，bridge 返回 Codex stdout directive。

Codex CLI `PermissionRequest` 等待超时时，runtime 会移除 pending approval 并写入失败 session 状态。

Codex CLI `Stop` 会生成 turn completed 并清理 pending interaction；只有携带 assistant 原文时才更新摘要。

Codex CLI runtime 在事件入口拒绝 `agent_kind` 不是 Codex CLI 的归一事件和 hook payload；非 Codex CLI payload 不会进入 Codex CLI runtime 状态。

Codex CLI runtime 收到 Codex APP 收录新 thread 的迁移通知时，会删除同 `(cwd, thread_id)` 的孤儿 session 并清理对应的 rollout path 与 pending approval；删除会发布 `session_updated` 事件让前端立刻刷新。

Codex CLI bridge 分流阶段会先按 hook payload 的 `terminal_app` 与 hook helper env 兜底改判 agent_kind；仍是 Codex CLI 时，会用 Codex APP runtime 已知 thread/cwd 再判一次，并在第一次未命中时触发一次受限超时的同步 thread list 刷新。



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


`src-tauri/src/adapters/hook_install/mod.rs` 是 Codex hook 安装和卸载入口。

`src-tauri/src/tauri_api/commands.rs` 是 Codex CLI session 读取和审批 command 入口。

`src/api/codexCliPanelApi.ts` 是前端 Codex CLI command 调用入口。

## 相关测试

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 覆盖 Codex hook 事件转换、非阻塞 ack 和审批 directive 等待。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 覆盖审批等待超时清理 pending approval。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 覆盖 Codex CLI runtime 拒绝非 Codex CLI 事件、孤儿 session 迁移和 bridge 分流阶段对已知 Codex APP thread 的改判。

`src-tauri/src/adapters/bridge/transport.rs` 覆盖长请求等待时 listener 仍可接收后续请求。

`src-tauri/src/adapters/bridge/hook_cli.rs` 覆盖 Codex hook helper fail-open 和 directive 输出。

`src-tauri/src/adapters/bridge/hook_payload.rs` 覆盖 Codex payload 基础校验。

`src-tauri/src/adapters/bridge/hook_output.rs` 覆盖 Codex allow 和 deny stdout。

`src-tauri/src/adapters/hook_install/mod.rs` 覆盖 Codex hook 写入、重复安装跳过、漂移状态修复和备份恢复。
