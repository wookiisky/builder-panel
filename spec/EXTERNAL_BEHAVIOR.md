# 外部行为

## 职责

本文档记录外部使用者可以观察到的行为和限制。

本文档不记录内部实现细节。

## 桌面窗口行为

应用启动后展示一个基础 Builder Panel 窗口。

窗口默认配置为置顶、无系统装饰、可调整大小。

窗口顶部区域声明为可拖动区域。

用户可以拖动顶部区域移动 Tauri 窗口。

窗口首版只展示扩展模式。

阶段 7 主窗口默认尺寸面向扩展模式工作台。

应用读取设置后会尝试恢复上次窗口位置、窗口尺寸和面板置顶状态。

用户关闭面板置顶设置并保存成功后，当前窗口不再保持在其它窗口上方。

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

每个 session 行从左到右展示状态、来源标签、项目名、thread 名和当前输出文本。

session 行当前输出文本在后端 session 更新事件后短延迟刷新。

高频输出期间，session 行当前输出文本允许一到两秒体验延迟，但不得等到输出完全停止才刷新。

session 行和详情区展示完整 thread 名。

长 thread 名在面板布局内换行展示，不横向撑破面板。

session 来源标签按运行时来源派生：来自 Codex APP 的 session 显示为 `Codex`，来自 Codex CLI 的 session 显示为 `Codex CLI`。

当前输出文本超过行宽时截断展示。

当前输出文本按段落展示；session 行界面只展示最后一段，用户可通过立即显示的 tooltip 查看最近若干段（段数由展示设置配置，默认 5）完整文本。

当前输出文本 tooltip 保留段内换行，并按 Markdown 渲染标题、列表、引用、代码、链接和基础行内强调。

当前输出文本 tooltip 在视口内展示；下方空间不足时会改为在当前输出文本上方展示。

session 详情可展示当前 view model 可用的完整多段摘要。

任务结束时，最终 Agent 输出按 65535 字符上限保留多段内容。

等待审批和等待用户回复的 session 可自动展示为两行。

完成和失败且可创建后续 turn 的 session 默认展示为单行。

完成和失败且可创建后续 turn 的 session 第一行右侧提供展开按钮。

用户点击展开按钮后，完成或失败 session 才展示第二行。

两行 session 的第一行展示最后一段输出文本。

Codex APP thread 名可由 Codex session index、app-server thread metadata 或 app-server 实时改名通知补齐。

Codex CLI hook 的模型字段和 Codex APP 中形似模型名的值不展示为 thread 名。

Codex APP 内部建议生成等隐藏 turn 不展示为 session；这类 turn 只产生空白启动行时，列表会移除对应空壳 session。

Codex APP 已加载 thread 只有在存在真实标题、预览文本、实时事件、待处理交互或系统错误时才展示；只有 cwd、id、模型名标题或空预览的 thread metadata 不展示为空白 session。

两行 session 的第二行展示快捷输入和输入区。

session 列表合并 Codex CLI 和 Codex APP session 后按首次捕捉顺序保持稳定。

新捕捉到的 session 展示在列表顶部。

每次打开 Builder Panel APP 时，session 列表先从进程内空状态开始；Codex APP 可随后通过当前已加载 thread id 和 `thread/list` 元数据补出当前 APP thread。

Codex APP 通过已加载 thread id 和 `thread/list` 元数据补出当前 APP thread 时，不补出无可展示内容的空白 thread。

Builder Panel 可为 Codex APP 读取 app-server 已加载 thread id、`thread/list` 元数据和 Codex rollout 历史，用于补齐项目名、跳回目标和最新 Agent 输出。

Builder Panel 可在已知 Codex CLI session 的 rollout path 后展示新增追加行产生的用户文本和 assistant 文本。

Builder Panel 可在已知 Codex APP session 的 rollout path 后展示新增追加行产生的用户文本和 assistant 文本。

Codex CLI 和 Codex APP session 最后消息不展示 hook 工具调用、命令预览或其它工具调用参数。

工具输出结束事件不单独展示状态文案。

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

mock agent 不再作为产品运行时 session 来源。

会话列表可展示 Codex CLI hook 产生的真实 session。

panel 打开期间会刷新 Codex CLI session，后续 hook 事件可进入列表和详情。

Codex CLI 请求权限时，用户可在 panel 中点击允许或拒绝。

Codex CLI 审批决策会通过 hook stdout directive 返回给 Codex CLI。

Codex CLI 的允许并记住入口当前按允许 directive 返回，不声明真实记忆规则已支持。

Codex APP 开关默认开启。开关开启后，panel 会尝试启动 Codex APP app-server 并接收启动后的 Codex APP 实时事件。

Codex APP hook payload 中 `terminal_app` 归一化后等于 `codexapp` 时，会显示为 Codex APP session。

Codex APP hook payload 缺少 `terminal_app` 时，hook helper 可通过 `BUILDER_PANEL_HOOK_TERMINAL_APP`、`__CFBundleIdentifier` 或 `TERM_PROGRAM` 环境变量识别 Codex.app；本地 bridge 也会按已知 thread 或已知 cwd 改判，避免 Codex.app 任务被误显示为 Codex CLI。

Codex APP 收录某 thread 元数据后，若该 thread 已被 hook 早到误存为 Codex CLI session，列表中的对应 Codex CLI 行会被移除，只保留 Codex APP 行。

Codex APP 请求权限时，用户可在 panel 中点击允许或拒绝。

Codex APP 审批决策可通过 hook stdout directive 或 app-server response 回写。

Codex APP 等待文本输入时，用户可在 panel 行内回复区发送文本。

Codex APP 等待选项输入时，用户可在 panel 中提交选项。

Codex APP app-server 不可用时，Codex CLI session 仍可刷新展示。

Codex APP 完成、失败或 app-server 标记为空闲且无待处理交互的 session 可通过行内输入区、快捷输入或 Follow-up 入口创建后续 turn。

Codex APP app-server 实时事件缺少可信项目路径且历史补齐尚未完成时，session 项目名显示为待识别项目。

Codex APP 待识别项目 session 不提供跳回行为。

Codex CLI 和 Codex APP session 使用可信 cwd 派生项目名；`.claude/worktrees` 和 `.git/worktrees` 路径显示项目根目录名。

Codex APP session 最后消息不展示 hook 工具调用、命令预览或其它工具调用参数。

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

通知点击只定位 session，不打开其它浮层。

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

阶段 3 不执行 Windows 本机人工验证。

当前窗口位置和尺寸恢复只在 Tauri 环境自动执行。

当前窗口位置和尺寸恢复未执行 Windows 本机验证。

阶段 0 不承诺 Windows 人工验收已经完成。

## 代码入口

`src-tauri/tauri.conf.json` 是窗口配置入口。

`src/components/PanelShell.tsx` 是前端拖动区域入口。

`src-tauri/src/bin/builder-panel-hook.rs` 是 hook CLI 入口。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 是 Codex CLI hook 状态和 directive 入口。

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP hook、app-server、runtime、schema 探针和 notification 转换入口。

`src-tauri/src/adapters/bridge/hook_output.rs` 是 hook stdout directive 编码入口。

`src/views/BuilderPanelApp.tsx` 是 Codex CLI 和 Codex APP session 前端行为入口。

`src/api/codexCliPanelApi.ts` 和 `src/api/codexAppPanelApi.ts` 是前端真实 Codex command 调用入口。

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

`pnpm tauri:dev` 和 `pnpm dev` 用于人工启动桌面 panel。

`pnpm dev:web` 只启动前端 Vite 服务，不启动桌面程序。

`pnpm package` 用于生成当前平台的正式桌面程序 bundle 或 installer。

正式程序通过打包产物运行，不依赖 Vite dev server。

`pnpm test` 用于验证前端状态转换。

`cargo test --manifest-path src-tauri/Cargo.toml` 用于验证 Rust 纯规则。

`cargo test --manifest-path src-tauri/Cargo.toml bridge` 用于验证 bridge 和 hook helper 单元测试。

`pnpm build` 用于验证阶段 3 前端类型和生产构建。

`cargo test --manifest-path src-tauri/Cargo.toml codex_cli_hook` 用于验证 Codex CLI hook adapter。

`cargo test --manifest-path src-tauri/Cargo.toml codex_app` 用于验证 Codex APP app-server adapter。

`./node_modules/.bin/vitest run` 用于验证阶段 7 前端捕捉顺序、统计、设置默认值、工具用量聚合和自定义快捷输入。

`cargo test --manifest-path src-tauri/Cargo.toml settings_service` 用于验证设置服务。

`cargo test --manifest-path src-tauri/Cargo.toml notification_service` 用于验证通知计划服务。

`cargo test --manifest-path src-tauri/Cargo.toml hook_install` 用于验证 hook 安装器。

`pnpm spec:check` 用于验证 spec 文档质量门禁。

`pnpm performance:check` 用于验证性能预算静态场景。
