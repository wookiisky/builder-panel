# 测试

## 职责

本文档记录测试目标、测试分层、断言模型和验收入口。

本文档不复制测试代码。

## 测试目标

测试优先验证分层边界、纯规则、状态转换和降级行为。

阶段 1 测试 Domain 类型、事件、reducer 和 view model 纯转换。

当前测试不测试真实 agent 协议。

阶段 2 测试本地 bridge codec、hook helper fail-open、payload 基础校验、stdout directive 编码和 Mac UDS 单请求往返。

阶段 2 不声明 Windows Named Pipe 已在 Windows 本机验证。

阶段 3 不测试真实 Codex 或 Claude Code 协议。

阶段 3 不声明 Windows 本机人工验证。

阶段 4 当前测试 Codex CLI hook adapter、Codex CLI runtime、Codex APP hook 分流、Codex APP schema 探针、Codex APP request 编码、notification 转换、可信 cwd、rollout 历史补齐和完整能力 capability。

阶段 4 当前不声明 Claude Code 真实闭环已完成。

阶段 4 当前不执行 Windows 本机人工验证。

阶段 5 测试审批允许并记住、选项提交、快捷回复过滤、预设命令计划、跳回降级和前端选项状态。

阶段 5 当前不执行 Windows 本机人工验证。

阶段 6 当前不执行 Windows 本机人工验证。

阶段 7 测试扩展模式 session 捕捉顺序、统计、工具用量聚合、设置默认值、设置文件读写、通知合并、行内交互和自定义快捷输入。

阶段 7 当前未建立 Playwright 自动化截图验证。

阶段 7 当前不执行 Windows 本机人工验证。

阶段 8 测试配置缺字段默认化、原子写失败不覆盖旧配置、hook 状态查询、hook 安装卸载 fixture、日志脱敏、spec 文档门禁和性能预算静态场景。

阶段 8 测试设置页 hook 状态列表、panel 默认持久化状态和收缩状态归一化。

阶段 8 当前不执行 Windows 本机人工验证。

阶段 8 当前不声明 10 分钟空闲 CPU 人工采样已完成。

阶段 0 不测试未实现的持久化恢复。

## 测试分层

Rust 单元测试验证 Domain 纯规则。

前端单元测试验证 UI store 的纯状态转换。

前端单元测试验证合并 session 的首次捕捉顺序稳定，且新捕捉 session 插入顶部。

架构脚本验证跨层依赖边界。

Rust adapter 测试验证 bridge 和 hook helper 边界行为。

Rust mock adapter 测试验证 mock event、directive 记录和回写失败保留 pending。

Rust Codex CLI adapter 测试验证 hook payload 到归一事件、非阻塞 ack、pending approval 和 directive 等待。

Rust Codex CLI runtime 测试验证审批等待超时会清理 pending approval 并拒绝迟到决策。

Rust Codex CLI runtime 测试验证迟到 UI 决策早于 bridge 超时清理时仍会清理 session pending。

Rust Codex CLI runtime 测试验证同一 session 新审批会让旧审批等待器过期。

Rust Codex CLI adapter 测试验证 hook payload 的模型字段不作为 thread 标题展示。

Rust Codex APP adapter 测试验证 app-server schema 探针、request 编码、notification 到归一事件转换、Codex APP hook 分流和完整能力 capability。

Rust Codex APP adapter 测试验证 hook cwd 与 app-server thread 事件会折叠到同一 session。

Rust Codex APP adapter 测试验证首个 app-server 实时审批或回复 request 可初始化可操作 session。

Rust Codex APP adapter 测试验证 app-server 实时消息空白 cwd 会进入待识别 session，且不生成跳回目标。

Rust Codex APP adapter 测试验证无可信 cwd 的 app-server 实时事件不会使用 Builder Panel cwd，且不会生成跳回动作。

Rust Codex APP adapter 测试验证 app-server thread 元数据可迁移待识别 session，且不覆盖 pending、summary 或状态。

Rust Codex APP adapter 测试验证 app-server `thread/list` 元数据可在存在真实标题、预览文本或系统错误时创建当前 session。

Rust Codex APP adapter 测试验证当前 loaded `active` thread metadata 即使无标题、无预览也可创建运行中 session、跳回目标和 rollout watch target。

Rust Codex APP adapter 测试验证历史候选中无标题、无预览的 `active`、`idle` 或 `notLoaded` thread metadata 不创建空白 session，且不扩大 cwd 兜底认领范围。

Rust Codex APP adapter 测试验证 `thread/list` response 支持当前 schema 的 `data` 字段和旧版 `threads` 字段。

Rust Codex APP adapter 测试验证 `thread/read` request 使用 `threadId` 与 `includeTurns` wire 字段，并清洗 `thread` response 字段。

Rust Codex APP adapter 测试验证 metadata 预览命中内部提示词时不创建可见 session，`ephemeral` metadata 不创建 session。

Rust Codex APP adapter 测试验证内部建议提示词会清理 `SessionStart` 留下的空壳 session 和相关 runtime 缓存，真实用户提示词会保留 session。

Rust Codex APP adapter 测试验证 `thread/loaded/list` 响应只清洗 loaded thread id，空白 id 被跳过，重复 id 去重，缺失 `data` 或非字符串 id 会被拒绝。

Rust Codex APP adapter 测试验证同步刷新专用 try-RPC 在 request id、pending、stdin 锁竞争、非阻塞写失败或 Unix stdin pipe 已满时快速失败，不进入阻塞等待，并清理已插入的 pending request。

Rust Codex APP adapter 测试验证 `thread/name/updated` 可把模型名标题更新为真实 thread 标题，且不覆盖摘要或状态。

Rust Codex APP runtime 测试验证 session index 可直接补齐当前已知但缺标题或标题形似模型名的 session，且不创建无关 session；模型名标题被过滤后不会单独创建空白 session。

Rust Codex APP runtime 测试验证 path-only thread metadata 只补齐已有可信 cwd 的 session，不创建无关历史 session。

Rust Codex APP adapter 测试验证已有运行态 session 会忽略后台 `idle` thread 元数据状态，避免覆盖实时摘要或运行状态。

Rust Codex APP adapter 测试验证当前 turn Agent message delta 会累积展示、运行中列表摘要 `full_text` 保留有界当前 turn 输出、缓存有界，完成后仍保留最新 Agent 输出。

Rust Codex APP adapter 测试验证同一 thread 新 turn 不串联上一 turn 输出。

Rust Codex APP adapter 测试验证 follow-up 成功提交会清空上一 turn Agent 输出，并用用户输入原文更新摘要。

Rust Codex APP follow-up 测试验证未加载 thread 会先 resume，已加载 thread 不重复 resume，loaded thread 查询、resume 或 turn/start 失败会释放提交占位。

Rust Codex APP follow-up 测试验证 follow-up 提交期间 session key 迁移后，成功完成和失败释放仍会按 thread id 清理提交占位。

Rust Tauri command 测试验证 Codex APP hook 分流同步刷新在 app-server slot 或 runtime 锁竞争时快速跳过，不阻塞 hook 路径。

Rust Codex APP rollout 测试验证 `session_meta`、`agent_message`、`task_complete.last_agent_message` 和 assistant `output_text` 清洗。

Rust Codex APP rollout 测试验证完成事件会标记快照完成，完成后新的用户输入会重置为未完成。

Rust Codex APP rollout 测试验证范围外 path 被忽略，超长 JSONL 行被跳过且后续有效行仍可读取。

Rust Codex APP rollout 测试验证 tailer 只读取已知 session 的新增追加行，并验证用户输入事件、工具 preview 不写最后消息、重复工具事件不生成摘要、未知 JSON arguments 不展示、工具结束不写摘要、超长追加行后继续读取有效行。

Rust Codex APP adapter 测试验证孤立 rollout 快照不会单独创建当前 session。

Rust Codex APP adapter 测试验证 recent active rollout 可从空 runtime 创建运行中 session，并验证完成、空内容或内部提示词快照不创建 session。

Rust Codex APP adapter 测试验证 recent active rollout 创建 session 时触发 Codex CLI 孤儿清理回调。

Rust Codex APP adapter 测试验证后台 `notLoaded` metadata 不把 recent active rollout 创建的运行中 session 降级为失联。

Rust Tauri command 测试验证 rollout recent scan 候选集合只包含已加载、历史返回、当前待识别 thread 和当前已知但缺标题的 thread。

Rust Tauri command 测试验证 recent active rollout 活跃窗口默认 5 分钟、支持正整数分钟配置，并过滤完成、过期或未来时间快照。

Rust Tauri command 测试验证 `thread/read` 方法不可用时会触发 `thread/list` 降级判定，普通详情清洗错误不触发该降级。

Rust Tauri command 测试验证 thread 历史元数据只应用到当前待识别 thread 或当前已知但缺标题的 thread，thread path 快照必须匹配 thread ID 和候选集合。

Rust Codex APP adapter 测试验证无 cwd app-server 实时事件后续可随 hook 真实 cwd 迁移，且不产生重复 session。

Rust Codex APP adapter 测试验证 requestUserInput 与 MCP elicitation 回复编码。

Rust Codex APP adapter 测试验证 thread 列表单条无效 thread 不会丢弃同批有效 thread。

Rust Codex APP adapter 测试验证缺 cwd 或空白 cwd 但带 path 的 thread 可保留为 rollout 候选，空白 path 不会成为候选，且 `status.type` 类型错误的 thread 会被跳过。

Rust Codex APP adapter 测试验证 rollout 快照可迁移待识别 session 到真实 cwd。

Rust Codex APP adapter 测试验证 rollout 快照可用最近 Agent 输出刷新运行中 session 摘要，且不会用仅来自用户输入的摘要覆盖当前 Agent 摘要。

Rust Codex APP adapter 测试验证 permissions approval、legacy approval enum、JSON-RPC id 类型保留和 follow-up 成功前不写 activity。

Rust Codex APP adapter 测试验证 server request id 与本端 pending request id 碰撞时仍进入 runtime。

Rust Codex APP adapter 测试验证 app-server 回写成功只清 pending 并保持运行态，`idle` 才允许 follow-up，错误状态不会映射为运行态。

Rust Codex APP adapter 测试验证未知 app-server server request 会编码 JSON-RPC error，`notLoaded` 会清理 pending RPC 上下文。

Rust hook 安装测试验证 Codex `config.toml` 通过 TOML AST 处理格式变体并拒绝无效 TOML。

Rust bridge transport 测试验证长请求等待时 listener 仍可接收后续请求。

Rust service 测试验证选项校验、快捷回复过滤和预设命令计划生成。

Rust terminal adapter 测试验证跳回记录、系统 URL 打开边界和复制降级。

前端 mock store 测试验证选项选择按 interaction 隔离，失败后可保留，成功后只清当前交互。

前端 mock store 测试验证复制筛选结果只复制正文和虚拟列表可见范围计算。

前端 Builder Panel 测试验证轮询刷新后新出现的 Codex CLI 或 Codex APP session 会被选中。

前端 Builder Panel 测试验证 session 路由以 runtime source 为准，不以 agent kind 猜测来源，包含 Codex APP runtime source。

前端 Builder Panel 测试验证同一个 `SessionKey` 的 Codex CLI 和 Codex APP session 拥有不同 UI 选中身份。

前端 Builder Panel 测试验证合并后的 session 首次捕捉顺序稳定、新 session 插入顶部、统计数量和动作标签。

前端 Builder Panel 测试验证已有 session 摘要刷新后不改变首次捕捉顺序。

前端 Builder Panel 测试验证列表刷新调度器在刷新中收到实时事件时会补刷，且连续事件不会无限后延。

前端 Builder Panel 测试验证同一 session 摘要刷新后列表行展示新摘要。

前端 Builder Panel 测试验证 session 行点击只有在存在 jump action 且跳回设置开启时才触发跳回。

前端 Builder Panel 测试验证具备 jump action 的 session 在全局跳回关闭时仍可被点击选中。

前端 Builder Panel 测试验证完成和失败状态在可 follow-up 时默认单行，点击展开后展示输入区。

前端 Builder Panel 测试验证工具用量按工具和来源键取最新值且不按 session 求和。

前端 Builder Panel 测试验证单一 session 来源读取失败时不会阻断其它来源。

前端 Builder Panel 测试验证旧选中项消失后会自动选择当前可用 session。

前端 Builder Panel 测试验证设置保存旧响应不会覆盖最新状态。

前端设置测试验证阶段 7 默认设置不包含自动更新配置项。

前端设置测试验证默认 panel 状态为展开且没有虚构窗口几何。

前端设置测试验证自定义快捷输入会清洗非法项、重复 ID 和排序值。

前端 Builder Panel 测试验证 hook 安装默认状态。

前端 Builder Panel 测试验证 hook 安装和卸载按钮避免重复操作。

前端 Settings Panel 测试验证 hook 状态列表展示和单项安装卸载按钮回调。

前端 Builder Panel 测试验证窗口移动和尺寸变化的局部保存更新会合并。

Rust settings service 测试验证配置缺失、配置损坏和保存。

Rust config file adapter 测试验证设置文件缺失、读写和损坏 JSON。

Rust config file adapter 测试验证缺字段默认化、未知字段丢弃、临时文件写入失败不覆盖旧配置和并发保存临时文件隔离。

Rust hook install adapter 测试验证状态查询、安装预览、Codex hook 写入、重复安装跳过、混合 group 保留用户 handler、重复 agent 去重、失败回滚、旧备份和旧 manifest 保护、单项安装 manifest 保留、单项卸载回滚、备份恢复、manifest 删除和缺失配置卸载删除。

Rust log sanitizer 测试验证敏感字段脱敏、长文本截断和中文业务事件名。

CI 串行执行依赖安装、架构检查、前端 lint、前端测试和 Rust 测试。

CI 执行 spec 文档门禁和性能预算静态场景。

## 断言模型

Domain 测试断言输入和输出的确定关系。

Domain reducer 测试覆盖所有已定义事件分支。

Domain view model 测试覆盖 capability 到 UI action 的映射。

Domain view model 测试覆盖终态 session 不生成过期回复动作。

前端 store 测试断言状态转换不修改原对象。

架构脚本断言禁止依赖不会进入 Domain 和前端边界。

Bridge codec 测试断言 NDJSON 半包不会被提前解析。

Hook helper 测试断言 fail-open 不输出 stdout。

Hook output 测试断言 directive JSON 结构。

Mock adapter 测试断言多项目、多对话不会合并。

Mock adapter 测试断言用量不可用不会生成虚假数字。

Codex CLI adapter 测试断言第三方 payload 不进入 Domain 事件。

Codex CLI runtime 测试断言审批决策唤醒等待中的 hook request。

Codex CLI runtime 测试断言审批等待超时后不会保留可被迟到 UI 操作完成的 pending approval。

Codex CLI runtime 测试断言当前 session pending interaction 不匹配时旧审批不能完成。

Codex APP adapter 测试断言 token usage 只来自 app-server 已验证 notification 字段。

Interaction Service 测试断言 allow、deny 和回写失败路径。

Interaction Service 测试断言 allow and remember、单选、多选空选择、非法选项和回写失败路径。

Reply Service 测试断言非空、空内容、超长和回写失败路径。

Shortcut Reply Service 测试断言启用状态、agent 绑定、项目绑定和排序。

Preset Command Service 测试断言结构化创建优先、托管进程降级和复制降级。

Mock panel store 测试断言虚拟列表不会按一万条记录全量计算可见范围。

Builder Panel 测试断言主界面不依赖收缩状态。

Builder Panel 测试断言合并后的首次捕捉顺序稳定，且新 session 插入顶部。

Settings Service 测试断言配置损坏时核心 UI 使用默认设置。

Config file adapter 测试断言配置缺字段时对应字段使用默认值。

Settings Service 测试断言默认 panel 状态为展开且没有虚构窗口几何。

Settings Service 测试断言保存和读取会将收缩状态归一化为展开。

Config file adapter 测试断言临时文件写入失败时旧配置仍保留。

Config file adapter 测试断言同一路径并发保存不会共享临时文件。

Hook install adapter 测试断言安装前可获得修改文件、备份文件和 manifest 路径。

Hook install adapter 测试断言状态查询可识别未安装、已安装和需要修复。

Hook install adapter 测试断言卸载可恢复安装前已有配置。

Hook install adapter 测试断言安装失败不会留下无 manifest 的半安装配置。

Hook install adapter 测试断言卸载成功后 manifest 不再生效。

Hook install adapter 测试断言单项安装和单项卸载会保留其它 agent 的 manifest 记录。

Notification Service 测试断言通知点击只定位 session。

## 资源隔离

阶段 0 测试不启动真实 agent。

阶段 0 测试不读写用户配置。

阶段 0 测试不访问网络。

阶段 2 hook helper 测试不启动真实 Codex 或 Claude Code。

阶段 3 mock agent 测试不启动真实 Codex 或 Claude Code。

阶段 3 mock agent 测试不读写真正用户配置。

Codex APP schema 真实验证执行本机 `codex app-server generate-json-schema --out <tmpdir> --experimental`。

Codex APP app-server 真实 smoke 验证执行本机 `codex app-server --listen stdio://` 并完成基础初始化。

Codex APP app-server 真实 smoke 不验证已加载 thread id 与 `thread/list` 元数据联动同步。

Codex CLI hook 真实 smoke 验证构建 `builder-panel-hook`，安装真实 Codex hook 配置，并用代表性 hook payload 验证 helper 到 bridge 的投递路径。

阶段 4 自动测试不启动真实 Codex APP app-server 长驻进程。

阶段 7 自动测试不调用真实系统通知 API。

阶段 7 自动测试不读写真正用户配置路径；设置 adapter 测试使用临时文件。

阶段 8 hook 安装测试不读写真正用户配置路径；hook 安装测试使用临时目录 fixture。

阶段 8 性能预算脚本不启动真实 agent，不访问网络，不读取用户配置。

## 禁止方式

不得通过修改运行时语义来满足类型检查。

不得只验证 happy path。

不得把未验证的人工行为写成已通过结论。

## 代码入口

`src-tauri/src/domain/panel_geometry.rs` 是 Rust 位置修正测试入口。

`src-tauri/src/domain/panel_probe.rs` 是 Rust 探针测试入口。

`src-tauri/src/domain/agent_session.rs` 是 session key 和 capability 测试入口。

`src-tauri/src/domain/agent_interaction.rs` 是 pending interaction 测试入口。

`src-tauri/src/domain/agent_event.rs` 是事件序列化测试入口。

`src-tauri/src/domain/session_state.rs` 是 reducer、pending 清理、多会话隔离和排序测试入口。

`src-tauri/src/domain/usage.rs` 是用量测试入口。

`src-tauri/src/domain/app_error.rs` 是错误对象测试入口。

`src-tauri/src/domain/view_model.rs` 是 view model 映射、行内交互和跳回动作测试入口。

`src-tauri/src/adapters/bridge/codec_tests.rs` 是 bridge codec 测试入口。

`src-tauri/src/adapters/bridge/transport.rs` 是 Unix Domain Socket bridge 测试入口。

`src-tauri/src/adapters/bridge/hook_cli.rs` 是 hook helper 测试入口。

`src-tauri/src/adapters/bridge/hook_payload.rs` 是 hook payload 基础校验测试入口。

`src-tauri/src/adapters/bridge/hook_output.rs` 是 stdout directive 编码测试入口。

`src-tauri/src/adapters/mock_agent/mod.rs` 是 mock adapter 测试入口。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 是 Codex CLI hook adapter 和 runtime 测试入口。

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP app-server adapter 测试入口。

`src-tauri/src/services/session_service.rs` 是 session service 测试入口。

`src-tauri/src/services/interaction_service.rs` 是 interaction service 测试入口。

`src-tauri/src/services/reply_service.rs` 是 reply service 测试入口。

`src-tauri/src/services/shortcut_reply_service.rs` 是 shortcut reply service 测试入口。

`src-tauri/src/services/preset_command_service.rs` 是 preset command service 测试入口。

`src-tauri/src/adapters/terminal/mod.rs` 是 terminal adapter 测试入口。

`src-tauri/src/services/settings_service.rs` 是 settings service 测试入口。

`src-tauri/src/adapters/config_file/mod.rs` 是 JSON 设置文件 adapter 测试入口。

`src-tauri/src/adapters/hook_install/mod.rs` 是 hook 安装器 fixture 测试入口。

`src-tauri/src/adapters/log_sanitizer/mod.rs` 是日志脱敏测试入口。

`scripts/check-spec-docs.mjs` 是 spec 文档质量门禁入口。

`scripts/check-performance-budget.mjs` 是性能预算静态场景入口。

`src-tauri/src/services/notification_service.rs` 是 notification service 测试入口。

`src-tauri/src/adapters/notification/mod.rs` 是记录型通知 adapter 入口。

`src/views/BuilderPanelApp.test.ts` 是前端 Codex CLI session 刷新选择测试入口。

`src/views/BuilderPanelApp.test.ts` 是阶段 7 session 捕捉顺序、统计、动作标签和工具用量聚合测试入口。

`src/api/settingsApi.test.ts` 是前端设置默认值和自定义快捷输入清洗测试入口。

`src/api/panelWindowApi.test.ts` 覆盖前端 panel 窗口置顶偏好应用、状态恢复、局部保存和关闭窗口入口。

`src/api/sessionJumpApi.ts` 是前端 session 跳回 command 调用入口。

`src/api/hookInstallApi.ts` 是前端 hook 状态查询和安装 command 调用入口。

`scripts/check-architecture.mjs` 是架构检查入口。

`.github/workflows/ci.yml` 是 CI 验证入口。

## 命令入口

`pnpm architecture:check` 运行架构边界检查。

`pnpm lint` 运行前端 lint 和架构检查。

`pnpm test` 运行前端测试。

`cargo test --manifest-path src-tauri/Cargo.toml` 运行 Rust 测试。

`cargo test --manifest-path src-tauri/Cargo.toml bridge` 运行阶段 2 bridge 和 hook helper 测试。

`cargo test --manifest-path src-tauri/Cargo.toml codex_cli_hook` 运行 Codex CLI hook adapter 测试。

`cargo test --manifest-path src-tauri/Cargo.toml codex_app` 运行 Codex APP app-server adapter 测试。

`pnpm dev` 启动人工验证空 panel。

`pnpm package` 运行当前平台正式桌面程序打包验证，不属于常规快速验证或默认 CI 入口。

当用户级 Cargo mirror 缺少 lockfile 依赖时，可用命令级 Cargo mirror 配置运行 Rust 测试，不把 mirror 缺包记录为代码失败。
