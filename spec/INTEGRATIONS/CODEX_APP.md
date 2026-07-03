# Codex APP

## 职责

本文档记录 Builder Panel 对 Codex APP hook 和 app-server 的接入事实。

本文档不记录 Codex CLI hook 接入，不记录 Claude Code APP。

## 支持能力

Codex APP app-server schema 通过本机 `codex app-server generate-json-schema --experimental` 探针确认。

当前探针验证 thread/turn request 与 response、thread/turn notification、thread name updated、agent message delta、token usage、status changed、审批 response、用户输入 response 和 MCP elicitation response 相关 schema 存在；具体文件清单以 `src-tauri/src/adapters/codex_app/mod.rs` 的 `REQUIRED_SCHEMA_FILES` 为准。

Codex APP adapter 可编码 `initialize`、`initialized`、`thread/start`、`thread/resume`、`turn/start`、`thread/loaded/list`、`thread/read` 和 `thread/list` JSON-RPC 消息。

Codex APP adapter 可把 `thread/started`、`thread/name/updated`、`turn/started`、`item/agentMessage/delta`、`thread/status/changed`、`thread/tokenUsage/updated` 和 `turn/completed` notification 转换为归一事件。

Codex APP `thread/status/changed` 的 `idle` 映射为完成态；`systemError` 映射为失败态；`notLoaded` 映射为失联态。

Codex APP hook payload 由 Codex hook 入口接收；当 `terminal_app` 归一化后等于 `codexapp` 时，payload 被清洗为 `Codex APP` session。

Codex hook payload 缺少 `terminal_app` 时，hook helper 会按优先级读取 `BUILDER_PANEL_HOOK_TERMINAL_APP`、`__CFBundleIdentifier` 和 `TERM_PROGRAM` 环境变量；归一化后等于 `codexapp` 或 `comopenaicodex` 时改判为 Codex APP，并把命中的值回填到 `terminal_app`。

Codex hook payload 仍被判为 Codex CLI 时，本地 bridge 会在分流前查询 Codex APP runtime 已知 thread；命中已知 `thread_id` 或已知 cwd 后改判为 Codex APP，避免把 Codex APP 的 hook 误存到 Codex CLI runtime。

Codex APP runtime 收录新的 `thread_id` 后会通知 Codex CLI runtime 清理同 `(cwd, thread_id)` 的孤儿 session；通知触发的删除会发布 `session_updated` 事件让前端立刻刷新。

Codex hook 在 bridge 分流阶段第一次未命中 Codex APP 已知 thread 时，可以触发一次受限超时的同步 thread list 刷新，再判一次归属；同步刷新只使用已存在且仍存活的 app-server client，不负责启动 app-server，总时长上限不得超过 300 毫秒。

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

Codex APP follow-up 创建前会先通过 `thread/loaded/list` 确认目标 thread 已加载；未加载时先调用 `thread/resume`。

Codex APP `thread/loaded/list` 只返回当前内存已加载的 thread id；空白 id 会在 adapter 边界跳过，重复 id 会去重，缺失 `data`、`data` 非数组或数组元素非字符串时视为列表格式错误。

Codex APP follow-up 只允许在 session 无 pending interaction 且状态为完成或失败时创建；app-server `idle` 状态会映射为可 follow-up 的完成态。

Codex APP follow-up 只有在 `turn/start` 写入成功后才写入用户输入原文事件。

Codex APP follow-up 的 app-server request id 由 app-server client 内部递增分配。

Codex APP schema 探针必须验证 `thread/loaded/list` 和 `thread/resume` 相关 schema 后，才能把 follow-up resume 链路视为已验证能力。

Codex APP runtime 维护 `thread_id -> cwd` 映射；hook payload 的 `cwd` 和 app-server 实时消息中的 `cwd` 都可补齐该映射，保证 hook 与 app-server 事件折叠为同一个 `SessionKey`。

Codex APP runtime 在事件入口拒绝 `agent_kind` 不是 Codex APP 的归一事件和 hook payload；非 Codex APP payload 不会进入 Codex APP runtime 状态。

Codex APP runtime 只把可信 cwd 写入 session key；可信 cwd 来源包括 hook payload、app-server message 的 `params.cwd`、`params.thread.cwd`、app-server `thread/read` 或 `thread/list` 元数据和 Codex rollout 的 `session_meta.payload.cwd`。

Codex APP runtime 不使用 Builder Panel 进程 cwd 冒充 Codex thread cwd。

Codex APP app-server 实时事件缺少可信 cwd 时，会创建待识别项目占位 session；该占位 session 不生成跳回目标。

Codex APP 使用可信 cwd 派生项目展示名；`.claude/worktrees` 和 `.git/worktrees` 路径显示项目根目录名。

Codex APP session 列表和详情读取会尝试通过 app-server `thread/loaded/list` 获取当前已加载 thread id，再优先通过机会能力 `thread/read` 按 id 精确补齐当前已加载 thread 的 cwd、预览、状态和跳回目标；`thread/read` 不可用、超时或出现方法/协议类错误时降级为一次有限数量 `thread/list` 读取。

Codex APP `thread/read` 只作为机会能力；schema 探针发现 `ThreadReadParams` 和 `ThreadReadResponse` 时记录为已验证文件，缺失时不让 Codex APP 整体不可用。

Codex APP `thread/list` response 的当前 schema 字段为 `data`；adapter 仍保留 legacy `threads` 字段读取作为旧版本降级。

Codex APP 需要补齐待识别 session 或已知但缺标题的 session 时，也可通过 `thread/list` 读取有限数量 thread 历史。

Codex APP thread 标题可来自 `~/.codex/session_index.jsonl` 中同 ID 的 `thread_name`，app-server `thread/started.thread.name`、thread metadata，或 app-server `thread/name/updated` 实时通知。

Codex APP session index 标题可直接补齐当前 runtime 中已知但缺标题或标题形似模型名的 session，不依赖 app-server 历史列表返回该 thread。

Codex APP `session_index.thread_name`、app-server `thread.name` 或 `thread/name/updated` 中形似 Codex 模型名的值不作为真实 thread 标题展示。

Codex APP thread 元数据中的空白名称在 adapter 边界归一为缺失名称，后续真实名称仍可补齐 session 标题。

Codex APP thread metadata 状态只用于创建新的已加载 session；已有实时 session 只补齐缺失信息，不用后台 metadata 覆盖运行状态或最新摘要。

Codex APP 当前已加载 thread metadata 对未知 thread 创建 session 时，必须至少有可信 cwd，并满足真实标题、预览文本、`systemError` 状态，或 `active` 状态之一；无标题、无预览的当前已加载 `active` thread 会创建运行中 session。

Codex APP 历史 thread metadata 对未知 thread 创建 session 时，必须至少有真实标题、预览文本或 `systemError` 状态；只有 cwd、id、空白标题、模型名标题或空预览的历史 `active`、`idle`、`notLoaded` metadata 不创建列表 session。

Codex APP thread metadata 的预览文本命中内部提示词过滤规则时，不创建新的可见 session，也不写入 session 摘要。

Codex APP 不恢复 `ephemeral` thread metadata。

Codex APP `notLoaded` metadata 只折叠已有 session 或待识别 session，不为未知无内容 thread 创建失联 session；已有运行中 session 不因后台 `notLoaded` metadata 降级为失联。

Codex APP path-only thread metadata 不创建新 session；runtime 已有该 thread 可信 cwd 时，path-only metadata 可补齐标题和 rollout path。

Codex APP thread metadata 或 rollout 历史补齐已有 session 的标题、项目、跳回目标、能力或摘要时，会发布轻量 `session_updated` 事件。

Codex APP `thread/list` response 会逐条清洗；单条 thread 无效不会丢弃同批其它有效 thread。

Codex APP `thread/list` 中缺少 cwd 但带 path 的 thread 可作为 rollout 候选；只有 rollout `session_meta.cwd` 读取成功后才会补齐真实项目。

Codex APP thread status 缺失时可按 idle 处理；`status.type` 类型错误时必须跳过该条 thread。

Codex APP session 列表和详情读取可读取 Codex rollout JSONL 补齐 cwd 和最新 Agent 输出；普通 rollout 历史不单独创建当前 session。

Codex APP 后台同步可扫描最近活跃 rollout，捕捉 app-server 标记为 `notLoaded` 但本地 rollout 仍在追加的运行中 thread。

Codex APP 最近活跃 rollout 创建 session 必须同时满足未完成、处于活跃窗口内、具备 `session_meta` 中的可信 cwd 和可展示用户摘要或 Agent 输出；只有 `session_meta`、无可展示内容或命中内部提示词过滤规则的 rollout 不创建 session。

Codex APP 最近活跃 rollout 恢复窗口由 `BUILDER_PANEL_CODEX_APP_ACTIVE_ROLLOUT_WINDOW_MINUTES` 配置，未配置、为 0 或非法时默认 5 分钟。

Codex APP 最近活跃 rollout 创建 session 时会记录 `thread_id -> cwd` 和 rollout path，并触发 Codex CLI 同 `(cwd, thread_id)` 孤儿 session 清理。

Codex APP thread 元数据或候选快照携带 rollout path 后，可作为已知 session 的实时 tail 目标。

Codex APP rollout tail 只读取已知 session 的新增追加行，不回放历史行。

Codex APP rollout tail 可将新增追加行清洗为用户输入、Agent 文本活动更新或 turn 完成事件。

Codex rollout path 必须位于 `~/.codex/sessions` root 内，文件名必须匹配 `rollout-*.jsonl`，且必须通过普通文件和大小上限校验。

Codex rollout 读取只在 adapter 边界清洗 `session_meta`、`event_msg` 和 `response_item` 的必要字段，不把原始 JSON 写入 Domain。

Codex rollout 中 `event_msg.agent_message`、`task_complete.last_agent_message`、`turn_complete.last_agent_message` 和 assistant `response_item` 的 `output_text` 可作为最新 Agent 输出摘要。

Codex APP rollout snapshot 可用已清洗的最近 Agent 输出刷新已知运行中 session 摘要；运行中已有摘要时不得用仅来自用户输入的 rollout 摘要覆盖当前摘要。

Codex APP 最终 Agent 输出按 65535 字符上限保留多段内容。

Codex rollout 中的用户输入和 Agent 输出只展示原文，不拼接 `用户输入` 或 `Codex 回复` 前缀。

Codex rollout 中未知工具、动态工具和已知工具的 JSON arguments 不作为最后消息展示。

Codex rollout 中的工具事件不得在完成后覆盖最终 Agent 输出。

Codex APP 当前 turn 的 `item/agentMessage/delta` 会在 runtime 内按 thread 累积最多 65535 字符的有界输出；`turn/started` 或 follow-up 成功提交会清空该 thread 的当前 turn 输出。

Codex APP `turn/completed` 和 `thread/status/changed` 的 `idle` 优先保留当前 turn 最新 Agent 输出；没有当前输出时不写固定完成或空闲文案。

Codex APP session 最后消息只使用用户输入原文、assistant 输出原文或完成时的最后 assistant 输出。

Codex APP hook 工具事件和 app-server 权限请求仍可创建 pending interaction，但不写工具调用正文作为最后消息。

Codex APP hook 权限请求可在 pending approval 摘要中保留清洗后的审批上下文。

Codex APP hook `SessionStart` 若只创建了无标题、无摘要、无 pending 的空壳 session，且随后同 thread 的 `UserPromptSubmit` 命中内部提示词模式，该空壳 session 会被清理，不进入 session 列表。

Codex APP 清理内部提示词空壳 session 时，会同步清理该 thread 的 runtime 缓存和 rollout tail 目标，并发布轻量 `session_updated` 事件。

Codex APP app-server 实时事件可以为当前进程创建 session，即使对应 thread 早于 Builder Panel APP 启动。

Codex APP app-server 无 cwd 实时事件先创建的 session，会在后续真实 cwd 到达时按 thread ID 合并到同一个 session。

Codex APP session 跳回目标使用 `codex://threads/<thread_id>`。

Codex APP runtime 应用归一事件后通过 session 更新端口发布轻量 Tauri 事件。

Codex APP token usage 只在 app-server 发送 `thread/tokenUsage/updated` 且包含 `tokenUsage.total.totalTokens` 时展示已验证 token 数。

Codex hook 安装通过 TOML AST 编辑 `~/.codex/config.toml` 的 `features.hooks = true`，并在写入前验证输出 TOML 合法。

## 降级能力

schema 探针失败时，Codex APP 能力视为不可用。

Codex APP app-server 启动或初始化失败时，前端跳过 Codex APP session，Codex CLI session 仍可展示。

Codex APP app-server 启动或初始化失败后，后端在短时间退避窗口内不重复 spawn。

已缓存的 Codex APP app-server client 若检测到子进程退出，会清理缓存并进入重新启动或退避流程。

Codex APP app-server 写入时只短暂读取全局 client slot；等待 response 时不持有全局 slot 锁。

Codex APP app-server 启动和初始化时只短暂切换全局 slot 状态；实际启动和初始化等待在 slot 锁外执行。

未验证的 app-server 方法和字段不进入能力矩阵。

Codex APP 当前不从 app-server 原始 JSON 直接污染 Domain。

## 不支持能力

当前不持久化 Codex APP session。

当前不从未知或非候选 rollout 文件创建 Codex APP session；最近未完成且处于活跃窗口内的 rollout 是受限例外。

当前不声明 Codex APP app-server WebSocket transport 已接入。

当前不承诺 app-server experimental 字段跨 Codex 版本稳定。

## 协议事实入口

Codex app-server 使用 JSON-RPC 2.0 风格消息，但 wire 上省略 `jsonrpc` 字段。

app-server schema 必须以当前本机 Codex 生成结果为准。

WebSocket transport 当前不作为 Builder Panel 的 Codex APP 接入方式。

## 代码入口

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP schema 探针、hook 转换、app-server stdio 客户端、runtime、消息编码和 notification 转换入口。

## 相关测试

`src-tauri/src/adapters/codex_app/mod.rs` 覆盖 schema 文件探针、hook payload 分流、notification 转换、request 编码和完整能力 capability。

`src-tauri/src/adapters/codex_app/mod.rs` 覆盖 Codex APP runtime 拒绝非 Codex APP 事件、孤儿迁移回调按 thread 元数据触发和按 hook payload 触发。

`src-tauri/src/adapters/codex_app/mod.rs` 覆盖按 `session_id` 和 `cwd` 认领 Codex APP thread 的判定。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 覆盖 Codex CLI bridge 分流阶段对已知 Codex APP thread 的改判。

`src-tauri/src/adapters/bridge/hook_cli.rs` 覆盖 hook helper 通过环境变量兜底识别 Codex.app。

人工验证通过本机 `codex app-server generate-json-schema --experimental` 确认关键 schema 文件存在。
