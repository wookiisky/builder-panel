# Claude Hooks

## 职责

本文档记录 Builder Panel 对 Claude Code CLI hook 的当前接入边界。

本文档不记录 Claude Code APP 接入，不声明真实 Claude Code hook 已完成端到端人工验收。

## 当前边界

Claude Code CLI 通过 `builder-panel-hook --source claude` 进入本地 bridge。

hook helper 只在 payload 基础校验通过后发送 bridge command。

当前支持 `SessionStart`、`UserPromptSubmit`、`PreToolUse`、`PermissionRequest`、`PostToolUse`、`Notification`、`Stop` 和 `SessionEnd`。

Claude Code CLI session 读取当前不接入产品主界面。

未来接入 Claude Code CLI session 时，只允许使用 APP 启动后送达 bridge 的实时 hook payload。

未来接入 Claude Code CLI session 时，不得从 transcript、JSONL 或其它历史记录恢复 session。

APP 启动后仍在运行的 Claude Code CLI 任务如果继续触发 hook payload，可以进入当前 session 状态。

`PermissionRequest` 可返回 allow 或 deny directive。

`PreToolUse` 可返回工具权限 directive。

`PreToolUse` 工具权限 directive 必须显式携带 allow、deny 或 ask 决策。

非阻塞事件返回 ack 时不输出 stdout。

阶段 8 提供 Claude Code CLI hook 安装、备份、manifest 和卸载 adapter。

hook 安装器写入 Claude `settings.json` 的 `hooks` 字段。

hook 安装器不执行真实 Claude Code 进程。

## 官方资料边界

Claude Code command hook 的输入通过 stdin 传入。

Claude Code command hook 可通过 stdout JSON 返回决策。

`PermissionRequest` 可以通过 `hookSpecificOutput` 中的 `decision` 对象 allow 或 deny。

`PreToolUse` 可以通过 `hookSpecificOutput` 中的 `permissionDecision` allow、deny 或 ask。

`suppressOutput` 可用于隐藏 hook stdout 进入 transcript。

本阶段不实现 HTTP hook、MCP tool hook、prompt hook 或 agent hook。

## 参考实现边界

`open-vibe-island` 的 Claude hook payload、bridge 转发和 stdout directive 作为实现思路参考。

参考实现不替代 Claude Code 官方 Hooks reference。

stdout directive 格式必须由本项目测试固定。

## 限制

当前不承诺 Claude Code APP 结构化控制能力。

当前不从 Claude transcript 或 JSONL 反读过程事件或 session 状态。

当前不承诺 Windows Claude hook 已完成本机验收。

当前不处理 Claude Code 全量 hook 事件。

## 代码入口

`src-tauri/src/adapters/bridge/hook_payload.rs` 是 Claude hook payload 基础校验入口。

`src-tauri/src/adapters/bridge/hook_output.rs` 是 Claude stdout directive 编码入口。

`src-tauri/src/adapters/claude_cli_hook/mod.rs` 是 Claude CLI hook adapter 预留入口。

`src-tauri/src/adapters/hook_install/mod.rs` 是 Claude hook 安装和卸载入口。

## 相关测试

`src-tauri/src/adapters/bridge/hook_payload.rs` 覆盖 Claude 权限建议类型校验。

`src-tauri/src/adapters/bridge/hook_cli.rs` 覆盖 Claude `PermissionRequest` 长等待和 deny directive。

`src-tauri/src/adapters/bridge/hook_output.rs` 覆盖 Claude deny stdout。

`src-tauri/src/adapters/hook_install/mod.rs` 覆盖 Claude 配置缺失时安装创建和卸载删除。
