# Builder Panel 文档索引

## 职责

本文档是 `spec/` 的唯一导航入口。

本文档只登记稳定事实文档、主要模块边界、代码事实入口和测试入口。

本文档不替代需求文档、技术方案、开发计划或代码实现。

## 全局规则

`spec/` 只记录长期成立的系统事实。

字段定义以代码中的类型、schema、配置和测试为准。

文档不得使用流程图、表格、emoji 和实现代码片段。

新增、删除或重命名 `spec/` 文档时，必须同步更新本文档。

## 项目主入口

`REQ.md` 记录产品需求。

`README.md` 记录项目定位、本地开发、验证命令和文档导航。

`tech.md` 记录技术方案和架构边界。

`plan.md` 记录分阶段开发计划。

`SPEC_DOC.md` 记录文档系统规范。

## 工程结构

`src-tauri/src/domain/` 保存纯领域模型、纯状态转换和纯规则。

`src-tauri/src/ports/` 保存系统边界抽象接口。

`src-tauri/src/adapters/` 保存外部系统和基础设施 adapter。

`src-tauri/src/services/` 保存应用服务和用例编排。

`src-tauri/src/tauri_api/` 保存 Tauri command 和事件边界。

`src/` 保存 React 前端。

`scripts/` 保存本地检查脚本。

## 文档入口

`spec/SYSTEM_OVERVIEW.md` 回答系统职责、分层和依赖边界。

`spec/SYSTEM_FLOWS.md` 回答阶段 0 已建立的主流程和边界流程。

`spec/EXTERNAL_BEHAVIOR.md` 回答外部使用者可观察到的行为。

`spec/INTERNAL_BEHAVIOR.md` 回答内部协作约束和状态不变量。

`spec/ERROR_HANDLING.md` 回答错误分类、降级和外部表现。

`spec/DECISION_LOG.md` 回答当前生效的核心决策。

`spec/TEST.md` 回答测试分层、断言模型和验收入口。

## 子目录事实入口

`spec/SERVICE/` 保存应用服务事实文档。

`spec/DOMAIN/` 保存领域模型和纯规则事实文档。

`spec/API/` 保存 Tauri command、事件和 hook CLI 事实文档。

`spec/INFRA/` 保存工程运行时、配置和 CI 事实文档。

`spec/INTEGRATIONS/` 保存第三方 agent 接入边界事实文档。

`spec/TOOLS/` 保存检查脚本和开发工具事实文档。

`spec/WORKERS/` 保存常驻 worker 和后台任务事实文档。

## Domain 文档

`spec/DOMAIN/SESSION_STATE.md` 回答会话状态、pending interaction、reducer 和排序规则。

`spec/DOMAIN/AGENT_EVENT.md` 回答归一事件契约和事件边界。

`spec/DOMAIN/USAGE.md` 回答用量可用性、来源键、作用域、不可用语义和展示口径。

`spec/DOMAIN/ERRORS.md` 回答应用错误分类和失败状态影响。

## Service 文档

`spec/SERVICE/SESSION_SERVICE.md` 回答 session 列表与详情读取、view model 输出和 mock 测试基线状态来源。

`spec/SERVICE/INTERACTION_SERVICE.md` 回答审批提交、pending 校验、directive 回写和失败不清理规则。

`spec/SERVICE/REPLY_SERVICE.md` 回答文本回复校验、发送、草稿保留口径和失败不清理规则。

`spec/SERVICE/SHORTCUT_REPLY_SERVICE.md` 回答快捷回复过滤、排序、绑定、自定义快捷输入展示边界和失败回填草稿口径。

`spec/SERVICE/PRESET_COMMAND_SERVICE.md` 回答预设命令计划生成、结构化创建优先和复制降级口径。

`spec/SERVICE/PROCESS_TIMELINE_SERVICE.md` 回答过程事件时间线分页、搜索、类型筛选和缓存释放口径。

`spec/SERVICE/SETTINGS_SERVICE.md` 回答设置模型、默认化、保存和配置损坏降级口径。

`spec/SERVICE/NOTIFICATION_SERVICE.md` 回答通知计划、重复通知合并和点击定位口径。

## Infra 文档

`spec/INFRA/PROJECT_RUNTIME.md` 回答工程工具链、依赖解析和验证入口。

`spec/INFRA/HOOK_INSTALL.md` 回答 hook 安装、备份、manifest、卸载和 trust review 边界。

`spec/INFRA/RELEASE_QUALITY.md` 回答日志脱敏、性能预算、文档门禁和 CI 发布质量入口。

`spec/INFRA/TERMINAL.md` 回答终端跳回边界、降级策略和 Windows 未验证口径。

`spec/INFRA/UI_RUNTIME.md` 回答阶段 7 扩展模式、顶部状态区、设置弹窗、行内交互和前端运行时验证入口。

## API 文档

`spec/API/LOCAL_BRIDGE.md` 回答本地 bridge 协议、传输、错误和验收入口。

`spec/API/HOOK_HELPER.md` 回答 hook helper CLI 输入输出、fail-open 和 directive 边界。

`spec/API/REPLY_TARGETS.md` 回答回复目标、跳回和回写独立能力边界。

`spec/API/TAURI_EVENTS.md` 回答前端可订阅的 Tauri 事件契约。

## Integration 文档

`spec/INTEGRATIONS/CODEX_HOOKS.md` 回答 Codex CLI hook 当前接入边界。

`spec/INTEGRATIONS/CODEX_CLI.md` 回答 Codex CLI 阶段 4 真实 hook 闭环、能力和降级边界。

`spec/INTEGRATIONS/CODEX_APP.md` 回答 Codex APP hook、app-server、session、审批、回复、follow-up、timeline 和降级边界。

`spec/INTEGRATIONS/CLAUDE_HOOKS.md` 回答 Claude Code CLI hook 当前接入边界。

## 代码事实入口

`src-tauri/src/domain/mod.rs` 是 Rust Domain 模块入口。

`src-tauri/src/domain/panel_geometry.rs` 是 panel 位置修正规则入口。

`src-tauri/src/domain/panel_probe.rs` 是阶段 0 panel 探针模型入口。

`src-tauri/src/domain/agent_session.rs` 是会话身份、状态和能力入口。

`src-tauri/src/domain/agent_interaction.rs` 是 pending interaction 和回复目标入口。

`src-tauri/src/domain/agent_event.rs` 是归一 agent 事件入口。

`src-tauri/src/domain/session_state.rs` 是 session reducer 和排序入口。

`src-tauri/src/domain/usage.rs` 是用量和领域时间入口。

`src-tauri/src/domain/app_error.rs` 是应用错误模型入口。

`src-tauri/src/domain/view_model.rs` 是 Domain 到 UI view model 的纯转换入口。

`src-tauri/src/adapters/bridge/codec.rs` 是本地 bridge 协议和 NDJSON codec 入口。

`src-tauri/src/adapters/bridge/codec_tests.rs` 是本地 bridge codec 测试入口。

`src-tauri/src/adapters/bridge/transport.rs` 是 Mac UDS 和 Windows Named Pipe 传输入口。

`src-tauri/src/adapters/bridge/transport/windows_transport.rs` 是 Windows Named Pipe 具体实现入口。

`src-tauri/src/adapters/bridge/hook_cli.rs` 是 hook helper 运行逻辑入口。

`src-tauri/src/adapters/bridge/hook_payload.rs` 是 hook payload 基础校验入口。

`src-tauri/src/adapters/bridge/hook_output.rs` 是 hook stdout directive 编码入口。

`src-tauri/src/adapters/mock_agent/mod.rs` 是 mock agent 测试基线 adapter、runtime、directive 记录和 timeline 数据源入口。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 是 Codex CLI hook 事件转换、runtime 和 bridge server 入口。

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP hook、app-server stdio 客户端、runtime、schema 探针、消息编码和 notification 转换入口。

`src-tauri/src/adapters/codex_app/codex_rollout.rs` 是 Codex rollout JSONL 发现、session_meta cwd 清洗、最新 Agent 输出摘要清洗和已知 session 追加行 tail 入口。

`src-tauri/src/ports/session_update_port.rs` 是清洗后 session 更新事件端口入口。

`src-tauri/src/adapters/config_file/mod.rs` 是阶段 8 JSON 设置文件默认路径、原子读写和损坏降级入口。

`src-tauri/src/adapters/hook_install/mod.rs` 是阶段 8 hook 安装、备份、manifest 和卸载入口。

`src-tauri/src/adapters/log_sanitizer/mod.rs` 是阶段 8 日志脱敏入口。

`src-tauri/src/adapters/notification/mod.rs` 是阶段 7 记录型通知 adapter 入口。

`src-tauri/src/adapters/timeline/mod.rs` 是 timeline 内存缓存、去重、淘汰和释放入口。

`src-tauri/src/adapters/terminal/mod.rs` 是阶段 5 终端跳回 adapter、系统 URL 打开和复制降级测试入口。

`src-tauri/src/services/session_service.rs` 是 session 读取应用服务入口。

`src-tauri/src/services/interaction_service.rs` 是审批交互应用服务入口。

`src-tauri/src/services/reply_service.rs` 是文本回复应用服务入口。

`src-tauri/src/services/shortcut_reply_service.rs` 是快捷回复过滤和排序服务入口。

`src-tauri/src/services/preset_command_service.rs` 是预设命令计划生成服务入口。

`src-tauri/src/services/process_timeline_service.rs` 是过程事件时间线应用服务入口。

`src-tauri/src/services/settings_service.rs` 是阶段 7 设置应用服务入口。

`src-tauri/src/services/notification_service.rs` 是阶段 7 通知计划应用服务入口。

`src-tauri/src/tauri_api/commands.rs` 是 Tauri command 入口。

`src/api/panelProbeContract.ts` 是前端基础探针契约入口。

`src/api/mockPanelContract.ts` 是前端 session、交互、timeline 和 view model 契约入口。

`src/api/codexCliPanelApi.ts` 是前端 Codex CLI Tauri API 入口。

`src/api/settingsApi.ts` 是前端设置读写和 fallback 校验入口。

`src/api/sessionUpdateApi.ts` 是前端 Tauri session 更新事件订阅入口。

`src/api/settingsContract.ts` 是前端设置契约入口。

`src/api/panelWindowApi.ts` 是前端 panel 窗口状态恢复、监听、局部保存和关闭窗口入口。

`src/api/sessionJumpApi.ts` 是前端 session 跳回 command 调用入口。

`src/api/hookInstallApi.ts` 是前端 hook 状态查询、安装预览、安装和卸载 command 调用入口。

`src/stores/mockPanelStore.ts` 是前端 session 草稿、提交和 timeline 弹层状态入口。

`src/components/SettingsPanel.tsx` 是阶段 7 设置弹窗内容组件入口。

`scripts/check-architecture.mjs` 是架构边界检查入口。

`scripts/check-spec-docs.mjs` 是阶段 8 spec 文档质量门禁入口。

`scripts/check-performance-budget.mjs` 是阶段 8 性能预算静态场景入口。

## 测试入口

`src-tauri/src/domain/panel_geometry.rs` 包含 Rust 侧 panel 位置修正测试。

`src-tauri/src/domain/panel_probe.rs` 包含 Rust 侧 panel 探针测试。

`src-tauri/src/domain/agent_session.rs` 包含 session key 和 capability 测试。

`src-tauri/src/domain/agent_interaction.rs` 包含 pending interaction 测试。

`src-tauri/src/domain/agent_event.rs` 包含事件序列化测试。

`src-tauri/src/domain/session_state.rs` 包含 reducer、pending 清理、多会话隔离和排序测试。

`src-tauri/src/domain/usage.rs` 包含用量可用性测试。

`src-tauri/src/domain/app_error.rs` 包含错误对象测试。

`src-tauri/src/domain/view_model.rs` 包含 view model 映射测试。

`src-tauri/src/adapters/bridge/codec_tests.rs` 包含 bridge codec 测试。

`src-tauri/src/adapters/bridge/transport.rs` 包含 Unix Domain Socket bridge 测试。

`src-tauri/src/adapters/bridge/hook_cli.rs` 包含 hook helper fail-open 和 directive 测试。

`src-tauri/src/adapters/bridge/hook_payload.rs` 包含 hook payload 基础校验测试。

`src-tauri/src/adapters/bridge/hook_output.rs` 包含 stdout directive 编码测试。

`src-tauri/src/adapters/mock_agent/mod.rs` 包含 mock 测试基线 event、directive 和 pending 保留测试。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 包含 Codex CLI hook 事件转换和审批 directive 等待测试。

`src-tauri/src/adapters/codex_app/mod.rs` 包含 Codex APP hook 分流、schema 探针、request 编码、notification 转换和 capability 测试。

`src-tauri/src/adapters/config_file/mod.rs` 包含 JSON 设置文件读写、缺字段默认化、原子写失败和损坏文件测试。

`src-tauri/src/adapters/hook_install/mod.rs` 包含 hook 状态查询、安装预览、写入、备份恢复和卸载测试。

`src-tauri/src/adapters/log_sanitizer/mod.rs` 包含敏感字段脱敏、长文本截断和中文业务事件名测试。

`src-tauri/src/adapters/notification/mod.rs` 提供通知服务测试使用的记录型 adapter。

`src-tauri/src/adapters/timeline/mod.rs` 包含 timeline 内存缓存、去重、淘汰和释放测试。

`src-tauri/src/services/session_service.rs` 包含 session 读取测试。

`src-tauri/src/services/interaction_service.rs` 包含审批 allow、deny 和回写失败测试。

`src-tauri/src/services/reply_service.rs` 包含文本回复校验和失败保留测试。

`src-tauri/src/services/shortcut_reply_service.rs` 包含快捷回复过滤和排序测试。

`src-tauri/src/services/preset_command_service.rs` 包含预设命令计划生成测试。

`src-tauri/src/services/process_timeline_service.rs` 包含 timeline 分页、搜索和类型筛选测试。

`src-tauri/src/services/settings_service.rs` 包含设置缺失、损坏和保存测试。

`src-tauri/src/services/notification_service.rs` 包含通知抑制、合并和点击定位测试。

`src/stores/mockPanelStore.test.ts` 包含前端草稿、提交中和 timeline 缓存测试。

`src/api/settingsApi.test.ts` 包含前端设置默认值和自定义快捷输入清洗测试。

`src/views/BuilderPanelApp.test.ts` 包含阶段 7 session 捕捉顺序、统计、动作标签和工具用量聚合测试。

`scripts/check-architecture.mjs` 包含跨层依赖静态检查。

`scripts/check-spec-docs.mjs` 包含 spec 文档质量静态检查。

`scripts/check-performance-budget.mjs` 包含性能预算静态场景检查。
