# Hook Helper

## 职责

本文档记录 `builder-panel-hook` 的 CLI 输入输出、fail-open 边界和验收入口。

本文档不记录 hook 安装流程，不复制第三方 agent 完整 payload。

## CLI 行为

`builder-panel-hook` 支持 `--source codex`。

`builder-panel-hook` 支持 `--source claude`。

hook helper 从 stdin 读取 JSON payload。

空 stdin 直接退出。

缺少或不支持 `--source` 时 fail-open。

非法 JSON fail-open。

payload 基础校验失败 fail-open。

bridge 不可用时 fail-open。

bridge 返回 ack 或 error 时不输出 stdout directive。

只有 bridge 返回 directive 且 directive 编码成功时，hook helper 才输出 stdout。

bridge response 的 request ID 必须匹配当前 request。

bridge directive 的目标 agent 必须匹配当前 `--source`。

所有 fail-open 路径退出码为 0。

## Payload 清洗

hook helper 在发送 bridge command 前完成最低限度校验。

必填字段包括 hook 事件名、工作目录和 session ID。

`tool_input` 存在时必须是 JSON 对象。

`permission_suggestions` 存在时必须是 JSON 数组。

Codex 来源当前接受 `SessionStart`、`UserPromptSubmit`、`PreToolUse`、`PermissionRequest`、`PostToolUse` 和 `Stop`。

Claude 来源当前接受 Codex 来源事件，并额外接受 `Notification` 和 `SessionEnd`。

未支持事件不会发送给 bridge。

## Codex.app 兜底识别

Codex 来源 payload 的 `terminal_app` 归一化后等于 `codexapp` 或 `comopenaicodex` 时直接判为 Codex APP。

`terminal_app` 缺失或不匹配时，hook helper 按优先级读取 `BUILDER_PANEL_HOOK_TERMINAL_APP`、`__CFBundleIdentifier` 和 `TERM_PROGRAM` 环境变量；任一值归一化后命中 Codex.app 关键字时改判为 Codex APP，并把命中值回填到 `terminal_app`。

env 兜底只对 Codex 来源生效；Claude 来源不受 env 兜底影响。

## Timeout 语义

非阻塞 hook 等待短超时。

Codex `PermissionRequest` 使用长等待，用于等待用户审批。

Claude `PermissionRequest` 使用更长等待，用于等待用户审批。

hook helper 不得无限等待。

## Stdout Directive

Codex `PermissionRequest` directive 使用 `hookSpecificOutput` 包装 allow 或 deny 决策。

Claude `PermissionRequest` directive 使用 `hookSpecificOutput` 包装 allow 或 deny 决策，并设置 `suppressOutput`。

Claude `PreToolUse` directive 使用 `permissionDecision` 和 `permissionDecisionReason`。

Claude `PreToolUse` directive 的 `permissionDecision` 必须显式为 allow、deny 或 ask。

无法编码有效 directive 时 fail-open。

## 代码入口

`src-tauri/src/bin/builder-panel-hook.rs` 是 CLI 进程入口。

`src-tauri/src/adapters/bridge/hook_cli.rs` 是 hook helper 运行逻辑入口。

`src-tauri/src/adapters/bridge/hook_payload.rs` 是 payload 基础校验入口。

`src-tauri/src/adapters/bridge/hook_output.rs` 是 stdout directive 编码入口。

## 相关测试

`src-tauri/src/adapters/bridge/hook_cli.rs` 覆盖空 stdin、非法 JSON、bridge 不可用、Codex directive 和 Claude directive。

`src-tauri/src/adapters/bridge/hook_cli.rs` 覆盖 response request ID 错配和 agent 错配。

`src-tauri/src/adapters/bridge/hook_cli.rs` 覆盖 Codex.app 环境变量兜底命中和未命中、`TERM_PROGRAM` 命中以及 Claude 来源不受 env 兜底影响。

`src-tauri/src/adapters/bridge/hook_payload.rs` 覆盖来源解析、必填字段、事件范围和基础 JSON 类型校验。

`src-tauri/src/adapters/bridge/hook_output.rs` 覆盖 ack 无 stdout、Codex allow、Codex deny 和 Claude deny。

`src-tauri/src/adapters/bridge/hook_output.rs` 覆盖 Claude `PreToolUse` allow、deny、ask 和缺少 decision 的失败路径。
