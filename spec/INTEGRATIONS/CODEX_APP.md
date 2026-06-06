# Codex APP

## 职责

本文档记录 Builder Panel 对 Codex APP hook 和 app-server 的接入事实。

本文档不记录 Codex CLI hook 接入，不记录 Claude Code APP。

## 支持能力

Codex APP app-server schema 通过本机 `codex app-server generate-json-schema --experimental` 探针确认。

当前探针验证 thread/turn request 与 response、thread/turn notification、agent message delta、token usage、status changed、审批 response、用户输入 response 和 MCP elicitation response 相关 schema 存在；具体文件清单以 `src-tauri/src/adapters/codex_app/mod.rs` 的 `REQUIRED_SCHEMA_FILES` 为准。

Codex APP adapter 可编码 `initialize`、`initialized`、`thread/loaded/list`、`thread/read`、`thread/start` 和 `turn/start` JSON-RPC 消息。

Codex APP adapter 可把 `thread/started`、`turn/started`、`item/agentMessage/delta`、`thread/status/changed`、`thread/tokenUsage/updated` 和 `turn/completed` notification 转换为归一事件。

Codex APP `thread/status/changed` 的 `idle` 映射为完成态；`systemError` 映射为失败态；`notLoaded` 映射为失联态。

Codex APP hook payload 由 Codex hook 入口接收；当 `terminal_app` 为 `Codex.app` 时，payload 被清洗为 `Codex APP` session。

Codex APP runtime 可处理 hook `SessionStart`、`UserPromptSubmit`、`PreToolUse`、`PermissionRequest`、`PostToolUse` 和 `Stop` 事件。

Codex APP `PermissionRequest` 可通过 hook stdout directive 返回 allow 或 deny。

Codex APP app-server server request 可转换为结构化审批、文本回复或选项回复 pending interaction。

Codex APP app-server 支持 `item/commandExecution/requestApproval`、`item/fileChange/requestApproval`、`item/permissions/requestApproval`、`applyPatchApproval` 和 `execCommandApproval` 审批请求。

Codex APP app-server response 必须保留原始 JSON-RPC `id` 类型。

Codex APP app-server stdout 只有不含 `method` 且包含 `result` 或 `error` 的消息会被视为本端 request response；带 `method` 的 server request 即使 id 与本端 pending request 碰撞，也必须进入 runtime。

Codex APP app-server 未识别或畸形 server request 会回写 JSON-RPC error；若能识别 thread，则同步写失败状态，避免 app-server request 悬挂。

Codex APP legacy approval response 使用 `approved`、`approved_for_session`、`denied` 枚举；新版 item approval response 使用 `accept`、`acceptForSession`、`decline` 枚举。

Codex APP `item/tool/requestUserInput` 可回写文本或选项答案。

Codex APP `mcpServer/elicitation/request` 当前可回写文本 answer。

Codex APP follow-up turn 通过 app-server `turn/start` 发送。

Codex APP follow-up 只允许在 session 无 pending interaction 且状态为完成或失败时创建；app-server `idle` 状态会映射为可 follow-up 的完成态。

Codex APP follow-up 只有在 `turn/start` 写入成功后才写入 activity 事件。

Codex APP follow-up 的 app-server request id 由 app-server client 内部递增分配。

Codex APP runtime 维护 `thread_id -> cwd` 映射；hook payload 的 `cwd` 和 app-server thread 详情的 `cwd` 都可补齐该映射，保证 hook 与 app-server 事件折叠为同一个 `SessionKey`。

Codex APP session 跳回目标使用 `codex://threads/<thread_id>`。

Codex APP hook 和 app-server 事件写入进程内 process timeline。

Codex APP token usage 只在 app-server 发送 `thread/tokenUsage/updated` 且包含 `tokenUsage.total.totalTokens` 时展示已验证 token 数。

Codex hook 安装通过 TOML AST 编辑 `~/.codex/config.toml` 的 `features.hooks = true`，并在写入前验证输出 TOML 合法。

## 降级能力

schema 探针失败时，Codex APP 能力视为不可用。

Codex APP app-server 启动或同步失败时，前端跳过 Codex APP session，mock 和 Codex CLI session 仍可展示。

Codex APP app-server 启动或同步失败后，后端在短时间退避窗口内不重复 spawn。

Codex APP hook session 列表、详情、hook 审批和 hook timeline 不依赖 app-server 成功启动。

Codex APP session 列表、详情和 timeline 查询会尽量启动 app-server；启动失败不阻断已存在的 hook runtime 状态读取。

已缓存的 Codex APP app-server client 若检测到子进程退出，会清理缓存并进入重新启动或退避流程。

Codex APP app-server 写入时只短暂读取全局 client slot；等待 response 时不持有全局 slot 锁。

Codex APP app-server 启动和 loaded thread 同步时只短暂切换全局 slot 状态；实际启动、初始化和同步等待在 slot 锁外执行。

未验证的 app-server 方法和字段不进入能力矩阵。

Codex APP 当前不从 app-server 原始 JSON 直接污染 Domain。

## 不支持能力

当前不持久化 Codex APP timeline。

当前不从 Codex transcript 或 rollout 文件恢复 Codex APP timeline。

当前不声明 Codex APP app-server WebSocket transport 已接入。

当前不承诺 app-server experimental 字段跨 Codex 版本稳定。

## 协议事实入口

Codex app-server 使用 JSON-RPC 2.0 风格消息，但 wire 上省略 `jsonrpc` 字段。

app-server schema 必须以当前本机 Codex 生成结果为准。

WebSocket transport 当前不作为 Builder Panel 的 Codex APP 接入方式。

## 代码入口

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP schema 探针、hook 转换、app-server stdio 客户端、runtime、消息编码和 notification 转换入口。

`src-tauri/src/tauri_api/commands.rs` 是 Codex APP schema 探针、session、审批、回复、follow-up 和 timeline command 入口。

## 相关测试

`src-tauri/src/adapters/codex_app/mod.rs` 覆盖 schema 文件探针、hook payload 分流、notification 转换、request 编码和完整能力 capability。

人工验证通过本机 `codex app-server generate-json-schema --experimental` 确认关键 schema 文件存在。
