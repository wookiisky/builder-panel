# 系统总览

## 职责

Builder Panel 是本地优先的跨平台桌面控制面板。

系统负责展示 coding agent 会话状态、承接用户交互、通过边界 adapter 回写能力，并提供托管会话过程入口。

系统不承担云同步、账号体系、代理模型 API 或完整聊天客户端职责。

## 分层事实

Rust 后端按 `domain`、`ports`、`adapters`、`services`、`tauri_api` 分层。

React 前端按 `components`、`stores`、`views`、`api` 分层。

Domain 层保持纯粹，不依赖 Tauri、文件系统、异步运行时、adapter、service 或第三方裸 payload。

Tauri command 是前端进入 Rust 后端的边界。

Adapter 负责清洗外部协议和基础设施副作用。

Service 负责用例编排，不反向污染 Domain。

## 运行时事实

阶段 0 建立 Builder Panel APP 和 `builder-panel-hook` 两个入口。

Builder Panel APP 由 Tauri 启动，前端由 Vite 构建。

`builder-panel-hook` 当前以 fail-open 占位方式退出，不阻塞调用方 agent。

阶段 1 建立 Domain 类型、归一事件、纯 reducer 和 view model 纯转换。

阶段 2 建立本地 bridge codec、Mac Unix Domain Socket 传输、Windows Named Pipe 传输代码、hook helper 读取 stdin 和 stdout directive 编码。

阶段 3 曾建立 mock agent adapter、mock agent runtime、session 读取、审批回写、文本回复回写和过程事件时间线查询闭环。

阶段 4 开始接入 Codex CLI 真实 hook 闭环和 Codex APP app-server schema 探针。

阶段 5 建立选项处理、允许并记住审批、快捷回复过滤、预设命令计划和跳回端口边界。

阶段 6 建立托管过程事件 timeline 内存缓存、分页查询、搜索筛选、去重淘汰和关闭释放边界。

阶段 7 建立扩展模式工作台、设置页、设置文件读写、通知计划服务和记录型通知 adapter。

阶段 8 建立设置文件原子读写、hook 安装器、日志脱敏、性能预算脚本和 spec 文档门禁。

阶段 8 设置页已接入 hook 状态查询、安装和卸载入口。

阶段 8 设置已保存 panel 窗口位置和窗口尺寸。

当前主界面固定为展开工作台，不提供收缩入口。

当前主界面以顶部状态区和单列 session 行展示 session。

当前设置页以弹窗形式展示。

当前自定义快捷输入保存在 Replies 设置中，并在无选项的行内回复区展示。

当前 hook helper 可接收 Codex CLI 和 Claude Code CLI 的基础 hook payload，并通过 bridge 发送已清洗 command。

当前 Codex CLI hook 可折叠为 session 状态，并可在 pending approval 上返回 allow 或 deny directive。

当前 Codex CLI hook 事件可写入进程内 timeline 缓存。

当前 Codex APP 已接入 Codex hook 分流、app-server stdio 子进程、session 展示、审批、回复、follow-up turn、跳回和进程内 timeline。

当前 Codex APP hook 与 app-server 事件通过 thread ID 和 cwd 映射统一为同一个 session。

当前产品运行时只读取 Codex CLI 和 Codex APP session。

当前 Tauri 产品 command 不注册 mock session、mock 审批、mock 回复、mock 选项或 mock timeline 入口。

当前系统不声明 Claude Code 真实闭环已完成。

当前 Windows Named Pipe 代码未在 Windows 本机完成人工验收。

当前 mock agent 只作为测试基线，不作为产品运行时 session 来源，也不代表真实 Codex 或 Claude Code 能力。

当前跳回能力和文本回写能力是两个独立能力。

当前终端 adapter 声明可测试跳回记录、macOS 系统 URL 打开和复制降级模型，不声明真实终端人工闭环完成。

当前系统不声明真实 Mac 或 Windows 系统通知已接入。

当前阶段 7 不执行 Windows 本机验证。

当前阶段 8 不执行 Windows 本机验证。

## 代码入口

`src-tauri/src/lib.rs` 是 Tauri 后端启动入口。

`src-tauri/src/main.rs` 是桌面进程入口。

`src-tauri/src/bin/builder-panel-hook.rs` 是 hook helper 入口。

`src-tauri/src/adapters/bridge/codec.rs` 是本地 bridge 协议入口。

`src-tauri/src/adapters/bridge/transport.rs` 是本地 bridge 传输入口。

`src-tauri/src/adapters/bridge/transport/windows_transport.rs` 是 Windows Named Pipe 实现入口。

`src-tauri/src/adapters/bridge/hook_cli.rs` 是 hook helper 运行逻辑入口。

`src-tauri/src/adapters/mock_agent/mod.rs` 是 mock agent 测试基线 adapter 和 runtime 入口。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 是 Codex CLI hook adapter、runtime 和 bridge server 入口。

`src-tauri/src/adapters/timeline/mod.rs` 是 timeline 内存缓存 adapter 入口。

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP hook、app-server、runtime、schema 探针和 notification 转换入口。

`src-tauri/src/services/session_service.rs` 是 session 读取服务入口。

`src-tauri/src/services/interaction_service.rs` 是审批交互服务入口。

`src-tauri/src/services/reply_service.rs` 是文本回复服务入口。

`src-tauri/src/services/shortcut_reply_service.rs` 是快捷回复过滤和排序服务入口。

`src-tauri/src/services/preset_command_service.rs` 是预设命令计划生成服务入口。

`src-tauri/src/services/process_timeline_service.rs` 是过程事件时间线服务入口。

`src-tauri/src/adapters/terminal/mod.rs` 是终端跳回 adapter 入口。

`src-tauri/src/services/settings_service.rs` 是设置应用服务入口。

`src-tauri/src/adapters/config_file/mod.rs` 是设置文件 adapter 入口。

`src-tauri/src/adapters/hook_install/mod.rs` 是 hook 安装、备份、manifest 和卸载入口。

`src-tauri/src/tauri_api/commands.rs` 是 hook 安装 command 和 panel 窗口状态局部保存 command 入口。

`src-tauri/src/adapters/log_sanitizer/mod.rs` 是日志脱敏入口。

`src-tauri/src/services/notification_service.rs` 是通知计划应用服务入口。

`src-tauri/src/adapters/notification/mod.rs` 是记录型通知 adapter 入口。

`scripts/check-spec-docs.mjs` 是 spec 文档质量门禁入口。

`scripts/check-performance-budget.mjs` 是性能预算静态场景入口。

`src/main.tsx` 是 React 前端入口。

`src/components/SettingsPanel.tsx` 是设置页组件入口。

`src/api/panelWindowApi.ts` 是前端 panel 窗口状态恢复和局部保存入口。

`src/api/sessionJumpApi.ts` 是前端 session 跳回 command 调用入口。

`src/api/hookInstallApi.ts` 是前端 hook 状态查询和安装 command 调用入口。

`src-tauri/src/domain/session_state.rs` 是 session 状态入口。

`src-tauri/src/domain/agent_event.rs` 是归一事件入口。

## 相关测试

`scripts/check-architecture.mjs` 验证分层边界。

`src-tauri/src/domain/panel_geometry.rs` 验证基础窗口位置修正。

`src-tauri/src/domain/session_state.rs` 验证 Domain reducer。

`src-tauri/src/adapters/bridge/codec_tests.rs` 验证 bridge codec。

`src-tauri/src/adapters/bridge/hook_cli.rs` 验证 hook helper fail-open 和 directive。

`src-tauri/src/adapters/mock_agent/mod.rs` 验证 mock 测试基线 event、用量、directive 和失败保留。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 验证 Codex CLI hook 事件转换和审批 directive 等待。

`src-tauri/src/adapters/timeline/mod.rs` 验证 timeline 去重、淘汰和释放。

`src-tauri/src/adapters/codex_app/mod.rs` 验证 Codex APP schema 探针、hook 分流、request 编码、notification 转换和完整能力 capability。

`src-tauri/src/services/interaction_service.rs` 验证审批闭环。

`src-tauri/src/services/reply_service.rs` 验证文本回复闭环。

`src-tauri/src/services/shortcut_reply_service.rs` 验证快捷回复过滤和排序。

`src-tauri/src/services/preset_command_service.rs` 验证预设命令计划生成。

`src-tauri/src/services/process_timeline_service.rs` 验证 timeline 查询。

`src/stores/mockPanelStore.test.ts` 验证前端草稿、提交和 timeline 缓存状态。

`src/views/BuilderPanelApp.test.ts` 验证阶段 7 session 排序、统计、动作标签和工具用量聚合。

`src-tauri/src/services/settings_service.rs` 验证设置服务默认化和保存。

`src-tauri/src/services/notification_service.rs` 验证通知计划服务。

`src-tauri/src/adapters/config_file/mod.rs` 验证配置缺字段默认化和原子写失败不覆盖旧配置。

`src-tauri/src/adapters/hook_install/mod.rs` 验证 hook 状态查询、安装预览、备份恢复和卸载。

`src-tauri/src/adapters/log_sanitizer/mod.rs` 验证日志脱敏。

`scripts/check-spec-docs.mjs` 验证 spec 文档质量门禁。

`scripts/check-performance-budget.mjs` 验证性能预算静态场景。
