# Codex APP

## 职责

本文档记录 Builder Panel 对 Codex APP app-server 的阶段 4 接入事实。

本文档不记录 Codex CLI hook 接入，不记录 Claude Code APP。

## 支持能力

Codex APP app-server schema 通过本机 `codex app-server generate-json-schema --experimental` 探针确认。

当前探针验证 `ThreadStartParams`、`ThreadStartResponse`、`TurnStartParams`、`ThreadStartedNotification`、`TurnStartedNotification`、`AgentMessageDeltaNotification`、`ThreadTokenUsageUpdatedNotification`、`TurnCompletedNotification` 和 `ThreadStatusChangedNotification` schema 存在。

Codex APP adapter 可编码 `initialize`、`initialized`、`thread/start` 和 `turn/start` JSON-RPC 消息。

Codex APP adapter 可把 `thread/started`、`turn/started`、`item/agentMessage/delta`、`thread/status/changed`、`thread/tokenUsage/updated` 和 `turn/completed` notification 转换为归一事件。

Codex APP token usage 只在 app-server 发送 `thread/tokenUsage/updated` 且包含 `tokenUsage.total.totalTokens` 时展示已验证 token 数。

## 降级能力

schema 探针失败时，Codex APP 能力视为不可用。

未验证的 app-server 方法和字段不进入能力矩阵。

Codex APP 当前不展示结构化审批回写入口。

Codex APP 当前不展示开放性回复入口。

Codex APP 当前不展示 follow-up turn 入口。

Codex APP 当前不展示 process timeline 入口。

Codex APP 当前不从 app-server 原始 JSON 直接污染 Domain。

## 不支持能力

当前未实现 Codex APP app-server 常驻子进程管理。

当前未实现 Codex APP 审批回写。

当前未实现 Codex APP 已有会话自动发现。

当前未实现 Codex APP 过程事件持久 timeline。

当前不承诺 app-server experimental 字段跨 Codex 版本稳定。

## 协议事实入口

Codex app-server 使用 JSON-RPC 2.0 风格消息，但 wire 上省略 `jsonrpc` 字段。

app-server schema 必须以当前本机 Codex 生成结果为准。

WebSocket transport 当前属于 experimental and unsupported，阶段 4 首选 stdio 或本地 Unix socket。

## 代码入口

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP schema 探针、消息编码和 notification 转换入口。

`src-tauri/src/tauri_api/commands.rs` 是 Codex APP schema 探针 command 入口。

## 相关测试

`src-tauri/src/adapters/codex_app/mod.rs` 覆盖 schema 文件探针、notification 转换和 request 编码。

人工验证通过本机 `codex app-server generate-json-schema --experimental` 确认关键 schema 文件存在。
