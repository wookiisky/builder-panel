# 外部行为

## 职责

本文档记录外部使用者可以观察到的行为和限制。

本文档不记录内部实现细节。

## 桌面窗口行为

应用启动后展示一个基础 Builder Panel 窗口。

窗口配置为置顶、无系统装饰、可调整大小。

窗口顶部区域声明为可拖动区域。

用户可以拖动顶部区域移动 Tauri 窗口。

窗口首版只展示扩展模式。

阶段 7 主窗口默认尺寸面向扩展模式工作台。

应用读取设置后会尝试恢复上次窗口位置和窗口尺寸。

窗口移动或调整大小后，Tauri 环境会保存新的窗口位置和尺寸。

用户可以点击顶部关闭按钮关闭 Tauri 窗口。

用户可以点击顶部最小化按钮最小化 Tauri 窗口。

## 前端行为

panel 展示 Codex CLI 和 Codex APP 会话列表。

panel 不展示 mini 模式切换入口。

panel 顶部状态区展示运行中数量和 session 总数。

panel 顶部状态区展示 Codex 和 Claude 工具维度整体用量。

同一工具同一来源键的整体用量只取最新值，不按 session 求和。

panel 顶部状态区右侧展示最小化、设置和关闭按钮。

session 列表展示等待审批、等待回复、运行中、完成和失败状态。

每个 session 行从左到右展示固定宽度状态区、来源标签、项目名和当前输出文本。

当前输出文本超过行宽时截断展示。

完成、失败或等待用户回复的 session 可展示为两行。

两行 session 的第一行展示最后一段输出文本。

两行 session 的第二行展示回复区。

session 列表合并 Codex CLI 和 Codex APP session 后，等待用户操作的 session 排在前面。

同一状态的 session 按更新时间倒序展示。

每次打开 Builder Panel APP 时，session 列表先从进程内空状态开始；Codex APP 可随后通过当前已加载 thread 元数据补出当前 APP thread。

Builder Panel 可为 Codex APP 读取 app-server 已加载 thread 元数据和 Codex rollout 历史，用于补齐项目名、跳回目标和最新 Agent 输出。

APP 打开后仍在运行并继续发出实时事件的任务会进入 session 列表。

APP 打开前已经结束且不再发出实时事件的 Codex CLI 或未接入 agent 历史任务不会进入 session 列表。

不支持的动作不展示为可点击按钮。

长摘要、长路径和长命令不会撑破面板布局。

用户点击具备跳回能力和跳回目标的 session 时，panel 会尝试跳转到对应工具界面。

用户点击不具备跳回能力或没有跳回目标的 session 时，panel 不跳转、不选中且不展示错误。

用户点击具备跳回目标但全局跳回开关关闭的 session 时，panel 只选中该 session，不跳转且不展示错误。

用户可以在 Codex CLI 或 Codex APP 审批 session 中点击允许、拒绝或允许并记住。

用户可以在 Codex APP 回复 session 的行内回复区输入单行或多行文本回复。

用户可以在 Codex APP 选项 session 中提交单选或多选回复。

选项存在 tooltip 时，用户可通过选项按钮 tooltip 查看说明。

`Enter` 发送 Codex APP 文本回复。

`Shift+Enter` 在 Codex APP 行内回复框中换行。

用户可以在无选项的 Codex APP 文本回复 session 中点击自定义快捷输入。

自定义快捷输入发送失败后，快捷输入内容保留在当前草稿中。

用户可以打开支持 timeline 的 Codex CLI 或 Codex APP session 过程事件弹层。

用户可以搜索、筛选和复制单条 timeline 条目。

用户可以复制当前筛选页的 timeline 条目。

用户可以在 timeline 弹层中跳到最新条目。

关闭 timeline 弹层后，当前页缓存被释放。

关闭 timeline 弹层后，后端会尝试释放该 session 的大文本正文缓存。

timeline 弹层不提供导出过程事件文件入口。

mock agent 不再作为产品运行时 session 来源。

会话列表可展示 Codex CLI hook 产生的真实 session。

panel 打开期间会刷新 Codex CLI session，后续 hook 事件可进入列表和详情。

Codex CLI 请求权限时，用户可在 panel 中点击允许或拒绝。

Codex CLI 审批决策会通过 hook stdout directive 返回给 Codex CLI。

Codex CLI 的允许并记住入口当前按允许 directive 返回，不声明真实记忆规则已支持。

Codex CLI hook 产生的托管事件可在支持 timeline 的 session 中查询。

Codex APP 开关默认开启。开关开启后，panel 会尝试启动 Codex APP app-server 并接收启动后的 Codex APP 实时事件。

Codex APP hook payload 中 `terminal_app` 为 `Codex.app` 时，会显示为 Codex APP session。

Codex APP 请求权限时，用户可在 panel 中点击允许或拒绝。

Codex APP 审批决策可通过 hook stdout directive 或 app-server response 回写。

Codex APP 等待文本输入时，用户可在 panel 行内回复区发送文本。

Codex APP 等待选项输入时，用户可在 panel 中提交选项。

Codex APP app-server 不可用时，Codex CLI session 仍可刷新展示。

Codex APP 完成、失败或 app-server 标记为空闲且无待处理交互的 session 可通过行内输入区、快捷输入或 Follow-up 入口创建后续 turn。

Codex APP app-server 实时事件缺少可信项目路径且历史补齐尚未完成时，session 项目名显示为待识别项目。

Codex APP 待识别项目 session 不提供跳回行为。

Codex APP session 可打开 timeline 弹层查看 hook 和 app-server 过程事件。

Codex APP session 跳回目标为 `codex://threads/<thread_id>`。

macOS 上 Codex APP 跳回通过系统打开 `codex://` URL。

阶段 7 设置弹窗包含 General、Display、Agents、Replies、Presets、Terminal 和 Advanced。

设置页包含 Hook Install 分组。

设置页不提供自动更新配置项。

用户可以在 Display 分组选择浅色或深色主题。

用户可以在 Hook Install 分组查看 Codex CLI hook 和 Claude CLI hook 的当前状态。

Hook Install 分组按列表展示 hook 项。

Hook Install 每个 hook 项只展示安装和卸载按钮。

用户点击安装后才会写入第三方 hook 配置。

用户点击卸载后会按 manifest 恢复对应 hook 的安装前配置。

Codex CLI 和 Codex APP 开关会影响对应来源的 session 读取和展示。

Claude Code CLI 和 Claude Code APP 设置开关当前显示为禁用，不触发 session 读取。

未来接入 Claude Code 或其它 coding agent session 时，初始空列表、不主动读取历史记录和启动后实时事件可进入列表的规则保持一致。

用量展示开关关闭后，顶部状态区和 session 行不展示用量信息。

快捷回复开关关闭后，文本回复区域不展示自定义快捷输入入口。

Enter 发送开关关闭后，Enter 不触发文本回复发送。

配置不存在时使用默认设置。

配置缺失字段时使用对应默认值。

配置未知字段不会进入设置模型。

配置损坏时使用默认设置并提示。

通知点击只定位 session 并聚焦 panel。

通知点击不直接打开过程弹出层。

Codex APP app-server schema 可由后端探针验证。

## hook CLI 行为

`builder-panel-hook` 可作为命令执行。

阶段 0 的 hook CLI 不输出阻塞 directive。

阶段 2 的 hook CLI 支持 `--source codex` 和 `--source claude`。

阶段 2 的 hook CLI 从 stdin 读取 hook JSON。

空 stdin、非法 JSON、payload 校验失败和 bridge 不可用时，hook CLI 不输出阻塞 directive。

bridge 返回有效 directive 时，hook CLI 向 stdout 输出对应 agent 的 directive JSON。

hook CLI fail-open 时退出码为 0。

hook CLI 可向 stderr 输出简短诊断。

## hook 安装行为

hook 安装前可展示将修改的文件、将创建的备份文件和 manifest 路径。

hook 安装入口已接入设置页。

hook 安装状态入口已接入设置页。

hook 安装会先备份已存在的第三方配置。

hook 卸载会按 manifest 恢复目标 agent 的安装前状态。

重复安装已完整安装的 hook 不会再次写入第三方配置。

重复卸载没有 manifest 记录的 hook 不会改写第三方配置。

Codex hook 安装会写入 `~/.codex/hooks.json`，并确保 `~/.codex/config.toml` 中启用 `[features] hooks = true`。

Codex hook 安装不绕过 Codex 自身 hook trust review。

## 限制

阶段 0 不承诺真实 Codex APP、Codex CLI、Claude Code APP 或 Claude Code CLI 接入。

阶段 2 不承诺真实 Codex CLI 或 Claude Code CLI 已完成人工端到端验收。

阶段 2 不承诺 Codex APP 或 Claude Code APP 接入。

阶段 2 不承诺 Windows Named Pipe 已在 Windows 本机验收。

阶段 3 不承诺真实 Codex 或 Claude Code 审批、回复和 timeline 接入。

阶段 4 当前声明 Codex CLI 审批 hook 闭环和 Codex APP hook、app-server、审批、回复、follow-up、跳回与 timeline 接入。

阶段 4 当前不声明 Claude Code 真实闭环已完成。

阶段 4 当前不声明 Codex APP WebSocket transport 已接入。

阶段 5 当前不声明真实终端跳回人工闭环已完成。

阶段 5 当前不声明真实新对话创建已完成。

阶段 5 当前不执行 Windows 本机验证。

阶段 6 当前不执行 Windows 本机验证。

阶段 7 当前不执行 Windows 本机验证。

阶段 7 当前不声明真实 Mac 或 Windows 系统通知已接入。

阶段 7 当前未建立 Playwright 自动化截图验证。

阶段 8 当前不执行 Windows 本机验证。

阶段 8 性能预算脚本不替代 10 分钟空闲 CPU 人工采样。

阶段 6 不支持从 transcript 或 JSONL 文件恢复 timeline。

阶段 6 不支持导出过程事件文件。

阶段 3 不执行 Windows 本机人工验证。

当前窗口位置和尺寸恢复只在 Tauri 环境自动执行。

当前窗口位置和尺寸恢复未执行 Windows 本机验证。

阶段 0 不承诺 Windows 人工验收已经完成。

## 代码入口

`src-tauri/tauri.conf.json` 是窗口配置入口。

`src/components/PanelShell.tsx` 是前端拖动区域入口。

`src-tauri/src/bin/builder-panel-hook.rs` 是 hook CLI 入口。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 是 Codex CLI hook 状态和 directive 入口。

`src-tauri/src/adapters/timeline/mod.rs` 是过程事件时间线内存缓存入口。

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP hook、app-server、runtime、schema 探针和 notification 转换入口。

`src-tauri/src/adapters/bridge/hook_output.rs` 是 hook stdout directive 编码入口。

`src/views/BuilderPanelApp.tsx` 是 Codex CLI 和 Codex APP session 前端行为入口。

`src/api/codexCliPanelApi.ts` 和 `src/api/codexAppPanelApi.ts` 是前端真实 Codex command 调用入口。

`src-tauri/src/tauri_api/commands.rs` 是 Codex CLI 和 Codex APP session、审批、选项、回复、follow-up 和 timeline command 入口。

`src-tauri/src/services/shortcut_reply_service.rs` 是快捷回复过滤入口。

`src-tauri/src/services/preset_command_service.rs` 是预设命令计划入口。

`src-tauri/src/adapters/terminal/mod.rs` 是终端跳回降级入口。

`src/api/sessionJumpApi.ts` 是前端 session 跳回 command 调用入口。

`src/api/panelWindowApi.ts` 是前端窗口几何恢复、监听、局部保存、最小化和关闭窗口入口。

`src/components/SettingsPanel.tsx` 是设置弹窗内容入口。

`src/api/settingsApi.ts` 是设置读写和浏览器 fallback 入口。

`src-tauri/src/services/settings_service.rs` 是设置默认化和保存入口。

`src-tauri/src/services/notification_service.rs` 是通知计划和点击定位入口。

`src-tauri/src/adapters/notification/mod.rs` 是记录型通知 adapter 入口。

`src-tauri/src/adapters/hook_install/mod.rs` 是 hook 状态查询、安装预览、备份、manifest 和卸载入口。

`src-tauri/src/adapters/log_sanitizer/mod.rs` 是日志脱敏入口。

## 验收入口

`pnpm tauri:dev` 用于人工启动空 panel。

`pnpm test` 用于验证前端状态转换。

`cargo test --manifest-path src-tauri/Cargo.toml` 用于验证 Rust 纯规则。

`cargo test --manifest-path src-tauri/Cargo.toml bridge` 用于验证 bridge 和 hook helper 单元测试。

`pnpm build` 用于验证阶段 3 前端类型和生产构建。

`cargo test --manifest-path src-tauri/Cargo.toml codex_cli_hook` 用于验证 Codex CLI hook adapter。

`cargo test --manifest-path src-tauri/Cargo.toml codex_app` 用于验证 Codex APP app-server adapter。

`./node_modules/.bin/vitest run` 用于验证阶段 7 前端排序、统计、设置默认值、工具用量聚合和自定义快捷输入。

`cargo test --manifest-path src-tauri/Cargo.toml settings_service` 用于验证设置服务。

`cargo test --manifest-path src-tauri/Cargo.toml notification_service` 用于验证通知计划服务。

`cargo test --manifest-path src-tauri/Cargo.toml hook_install` 用于验证 hook 安装器。

`pnpm spec:check` 用于验证 spec 文档质量门禁。

`pnpm performance:check` 用于验证性能预算静态场景。
