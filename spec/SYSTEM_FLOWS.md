# 系统流程

## 职责

本文档记录已建立的主流程、恢复流程和异常收敛流程。

本文档记录已实现的真实 Codex 接入流程。阶段 3 的 mock agent 流程仍作为核心闭环验证基线。

## APP 启动流程

Tauri 桌面入口启动 Rust 后端。

Rust 后端注册 Tauri command。

Vite 前端渲染基础 panel。

前端通过 `get_panel_probe` 读取基础 panel 探针状态。

前端读取失败时使用本地默认探针状态展示空 panel。

## panel 状态流程

阶段 0 的 panel 默认处于 `expanded` 模式。

前端读取设置中的 Panel 配置初始化本地 `collapsed` 状态。

用户点击收缩按钮时，前端通过纯状态转换切换 `collapsed`。

前端通过 panel 窗口状态局部保存 command 持久化 `collapsed`。

收缩和展开不会触发 session 选中或草稿清理。

Tauri 环境读取设置后尝试恢复上次窗口位置和尺寸。

Tauri 环境监听窗口移动和尺寸变化。

窗口位置或尺寸变化后，前端通过 panel 窗口状态局部保存 command 持久化几何信息。

浏览器开发环境不执行 Tauri 窗口几何恢复。

## 设置流程

前端启动后读取 panel 设置。

Tauri 环境通过 settings command 进入 Settings Service。

Settings Service 通过设置存储端口读取配置。

配置不存在时，Settings Service 返回默认设置。

配置缺失字段时，反序列化边界补齐对应默认值。

配置未知字段不会进入设置模型。

配置损坏时，Settings Service 返回默认设置和提示。

前端设置页修改设置后立即调用保存设置 command。

前端只采纳最新保存请求对应的响应。

保存失败时，前端保留当前 UI 选择并展示错误提示。

保存设置时，JSON 设置文件 adapter 先写入同目录临时文件。

临时文件写入和刷盘成功后，adapter 替换目标配置文件。

临时文件写入失败时，旧配置文件保持不变。

浏览器开发环境从 localStorage 读取 fallback 设置，并在使用前校验结构。

## hook 安装流程

设置页展示 Codex CLI hook 和 Claude CLI hook 选择项。

用户选择安装目标后点击预览。

Tauri hook 安装 command 使用默认路径生成安装器。

默认 hook helper 路径来自当前应用同目录下的 `builder-panel-hook`。

环境变量 `BUILDER_PANEL_HOOK_PATH` 存在时覆盖默认 hook helper 路径。

hook 安装器生成安装预览。

安装预览列出将修改的配置文件、备份文件和 manifest 文件。

用户点击安装后，hook 安装器备份已存在的第三方配置文件。

hook 安装器读取 JSON 配置，配置不存在时使用空对象。

hook 安装器移除已有 Builder Panel hook handler。

hook 安装器写入当前 agent 支持的命令型 hook handler。

hook 安装器写入安装 manifest。

用户点击卸载后，hook 安装器读取 manifest。

安装前配置存在时，卸载恢复备份文件。

安装前配置不存在时，卸载删除本次创建的配置文件。

hook 安装器不执行真实 Codex 或 Claude Code 进程。

hook 安装器不绕过 Codex hook trust review。

## 发布质量流程

spec 文档门禁扫描 `spec/` 下实际存在的 Markdown 文档。

spec 文档门禁校验索引覆盖、职责说明、代码入口、测试或验收入口。

spec 文档门禁拒绝代码块、流程图语法和 Markdown 表格。

性能预算脚本生成 10 session、1000 event 和 1 万 timeline 静态场景。

性能预算脚本校验虚拟列表范围、timeline 淘汰和大文本释放。

性能预算脚本不声明 Mac 或 Windows 人工性能验收完成。

## 通知流程

Notification Service 接收已清洗的通知请求。

当前查看 session 与通知 session 相同时，Notification Service 不生成通知计划。

同一 session 同一类型通知在合并窗口内重复出现时，Notification Service 增加合并数量。

通知点击被转换为聚焦 panel、展开 panel 和定位 session。

通知点击不打开过程时间线。

## 位置修正流程

窗口位置修正由纯函数接收 panel 矩形和屏幕可用区域。

当 panel 落在屏幕外时，纯函数将左上角修正到可见区域内。

当屏幕尺寸小于 panel 尺寸时，纯函数将 panel 左上角修正为屏幕左上角。

## hook helper 流程

`builder-panel-hook` 从 stdin 读取 hook JSON。

空 stdin 直接退出。

hook helper 根据 `--source` 选择 Codex 或 Claude payload 校验。

payload 校验失败时 fail-open。

payload 校验成功后，hook helper 创建本地 bridge command。

hook helper 连接本地 bridge 并发送 NDJSON request。

bridge 返回 ack 或 error 时，hook helper 不输出 stdout。

bridge 返回 directive 时，hook helper 先校验 response request ID 和目标 agent。

校验通过后，hook helper 按 agent 来源编码 stdout directive。

bridge 不可用、超时、response 错配或 directive 编码失败时，hook helper fail-open。

## bridge codec 流程

bridge codec 按行解析 NDJSON。

半包数据保留在 decoder buffer。

空行被忽略。

非法 JSON 被拒绝。

request 和 response 使用同一协议版本。

## bridge 传输流程

Mac 使用 Unix Domain Socket 发送和接收 request。

Windows 使用 Named Pipe 发送和接收 request。

Windows client 通过调用方 timeout 收敛等待。

Windows Named Pipe 代码当前未完成 Windows 本机验收。

## Codex CLI hook 流程

前端读取 Codex CLI session 列表时，Rust 后端启动 Mac Unix Domain Socket bridge server。

前端在 panel 打开期间定时刷新 Codex CLI session 列表。

Codex CLI hook helper 将已清洗 payload 发送到 bridge server。

bridge server 调用 Codex CLI hook adapter 生成归一事件。

bridge server 对每个已接收连接启动独立处理线程。

长等待的 `PermissionRequest` 不阻塞 listener 接收后续 hook 请求。

非阻塞 hook 事件写入 runtime session state 后返回 ack。

`PermissionRequest` 写入 pending approval 后，bridge server 等待 panel 审批决策。

用户在 panel 点击允许或拒绝后，runtime 唤醒等待中的 bridge request。

bridge server 返回 Codex allow 或 deny stdout directive。

等待超时时，runtime 移除对应 pending approval 并写入失败 session 状态。

等待超时或 runtime 锁损坏时返回 bridge error，hook helper fail-open。

bridge server 首次 bind 失败时不会标记为已启动，后续读取 Codex CLI session 时可重试启动。

## Codex APP app-server 流程

Codex APP adapter 通过本机 `codex app-server generate-json-schema --experimental` 生成 schema。

schema 探针确认关键 request、response 和 notification schema 文件存在。

schema 探针覆盖所有当前 adapter 已消费的 app-server notification schema。

adapter 编码 `initialize`、`initialized`、`thread/start` 和 `turn/start` JSON-RPC 消息。

adapter 接收 app-server notification 后，在 adapter 边界读取 JSON 字段并转换为归一事件。

未识别 notification 不写 session 状态。

Codex APP 当前不生成 follow-up turn 和 process timeline UI 能力。

## mock session 主流程

Mock agent adapter 生成已清洗的归一事件。

Mock agent runtime 使用 Domain reducer 折叠事件。

Session Service 从 mock agent runtime 读取 session state。

Session Service 调用 Domain view model 转换列表和详情。

前端通过 Tauri command 读取 mock session 列表。

前端选中 session 后读取该 session 详情。

浏览器预览环境无法调用 Tauri command 时，前端使用同契约 fallback mock 数据。

## mock 审批链路

前端只在 session view model 生成 `ResolveApproval` 动作时展示审批按钮。

用户点击允许、拒绝或允许并记住后，前端提交 `SessionKey`、`InteractionId` 和审批决策。

Interaction Service 校验当前 session 仍等待同一个 pending approval。

Mock agent runtime 记录 allow、allow and remember 或 deny directive。

Mock agent runtime 写入 `TurnCompleted` 事件。

Domain reducer 清理 pending interaction。

前端刷新 session 列表和详情。

回写失败时，Interaction Service 返回错误。

回写失败时，pending approval 不被清理。

## mock 回复链路

前端只在 session view model 生成 `SendReply` 动作时展示回复框。

前端按 session 保存回复草稿。

`Enter` 提交文本回复。

`Shift+Enter` 在文本框中换行。

Reply Service 拒绝空内容和超长内容。

Reply Service 校验当前 session 仍等待同一个 pending text reply。

Mock agent runtime 记录文本回复 directive。

Mock agent runtime 写入 `TurnCompleted` 事件。

Domain reducer 清理 pending interaction。

前端发送成功后只清理当前 session 草稿。

回写失败时，pending text reply 和前端草稿都保留。

## mock 选项链路

前端只在 session view model 生成 `SendReply` 动作且 pending interaction 是 choice 时展示选项。

单选点击后只保留最后一个选项。

多选至少选择一项后才能提交。

Interaction Service 校验当前 session 仍等待同一个 pending choice。

Interaction Service 校验选项值来自当前交互，且单选不能提交多个值。

Mock agent runtime 记录 choice directive。

Mock agent runtime 写入 `TurnCompleted` 事件。

Domain reducer 清理 pending interaction。

回写失败时，pending choice 和前端已选选项都保留。

## 快捷回复链路

Shortcut Reply Service 根据启用状态、agent 类型、项目 ID 和排序值输出可用快捷回复。

前端只在文本回复可回写时展示快捷回复。

快捷回复点击后复用文本回复提交路径。

快捷回复发送失败时，快捷回复内容填回当前 session 草稿。

## 预设命令链路

Preset Command Service 根据预设命令和创建能力生成计划。

支持结构化创建时优先生成结构化创建计划。

不支持结构化创建但支持托管进程时，生成托管进程计划。

两者都不支持时，生成复制降级计划。

阶段 5 不启动真实终端，不声明新对话真实创建完成。

## 跳回链路

JumpTargetPort 表达跳回 agent 所在 APP 或终端的能力。

ReplySenderPort 表达文本回写能力。

跳回和回写必须独立判断。

跳回失败时返回复制降级，不触发文本回写补偿。

## timeline 链路

前端只在 session view model 生成 `ViewProcessTimeline` 动作时展示 timeline 入口。

用户打开 timeline 后，前端请求第一页过程事件。

mock session 的 timeline 条目来自 mock agent runtime。

Codex CLI session 的 timeline 条目来自 hook 事件内存缓存。

Codex CLI hook adapter 生成归一事件后，runtime 同步写入 session state 和 timeline 缓存。

timeline 缓存按 session 分片，并用条目 ID 去重。

timeline 缓存达到上限时优先淘汰最旧低优先级条目。

Process Timeline Service 执行类型筛选、搜索和分页。

前端展示当前页条目。

前端可复制单条或当前筛选页的 timeline 文本。

前端可将 timeline 弹层滚动到最新条目。

用户关闭 timeline 后，前端清理当前页缓存，并请求后端释放该 session 的大文本正文缓存。

timeline 不写文件或数据库。

timeline 不从 transcript 或 JSONL 反向读取。

timeline 不提供导出过程事件文件入口。

## 异常收敛流程

前端调用 `get_panel_probe` 失败时，不写入错误事实。

前端使用默认展示状态继续渲染。

hook helper fail-open 时不写业务状态。

mock 审批和回复回写失败时不清理 pending interaction。

mock 回复回写失败时前端不清理草稿。

timeline 查询失败时不写 session 状态。

timeline 释放失败时不阻塞关闭弹层。

阶段 0 和阶段 2 均不展示错误通知。

## 代码入口

`src-tauri/src/tauri_api/commands.rs` 是基础探针 command 入口。

`src/views/BuilderPanelApp.tsx` 是前端启动和降级展示入口。

`src-tauri/src/domain/panel_geometry.rs` 是位置修正入口。

`src-tauri/src/adapters/bridge/hook_cli.rs` 是 hook helper 流程入口。

`src-tauri/src/adapters/bridge/codec.rs` 是 bridge codec 流程入口。

`src-tauri/src/adapters/bridge/transport.rs` 是 bridge 传输流程入口。

`src-tauri/src/adapters/mock_agent/mod.rs` 是 mock agent 事件、directive 和 timeline 数据源入口。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 是 Codex CLI hook 流程入口。

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP app-server 探针和 notification 转换入口。

`src-tauri/src/services/session_service.rs` 是 mock session 读取流程入口。

`src-tauri/src/services/interaction_service.rs` 是 mock 审批链路入口。

`src-tauri/src/services/reply_service.rs` 是 mock 回复链路入口。

`src-tauri/src/services/shortcut_reply_service.rs` 是快捷回复链路入口。

`src-tauri/src/services/preset_command_service.rs` 是预设命令计划入口。

`src-tauri/src/services/process_timeline_service.rs` 是 timeline 查询链路入口。

`src-tauri/src/adapters/timeline/mod.rs` 是 timeline 内存缓存链路入口。

`src-tauri/src/adapters/terminal/mod.rs` 是跳回降级链路入口。

`src-tauri/src/services/settings_service.rs` 是设置读取和保存流程入口。

`src-tauri/src/adapters/config_file/mod.rs` 是设置文件读写流程入口。

`src-tauri/src/services/notification_service.rs` 是通知计划流程入口。

`src-tauri/src/adapters/notification/mod.rs` 是记录型通知 adapter 入口。

## 相关测试

`src-tauri/src/domain/panel_geometry.rs` 覆盖位置修正正常路径和边界路径。

`src/stores/panelProbeStore.test.ts` 覆盖收缩状态转换。

`src-tauri/src/adapters/bridge/hook_cli.rs` 覆盖 hook helper fail-open 和 directive。

`src-tauri/src/adapters/bridge/codec_tests.rs` 覆盖 NDJSON 解析流程。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 覆盖 Codex CLI hook 流程。

`src-tauri/src/adapters/codex_app/mod.rs` 覆盖 Codex APP app-server 边界转换。

`src-tauri/src/services/settings_service.rs` 覆盖设置缺失、损坏和保存流程。

`src-tauri/src/services/notification_service.rs` 覆盖通知抑制、合并和点击定位流程。

`src-tauri/src/services/interaction_service.rs` 覆盖审批成功和失败路径。

`src-tauri/src/services/reply_service.rs` 覆盖文本回复成功、校验失败和回写失败。

`src-tauri/src/services/shortcut_reply_service.rs` 覆盖快捷回复过滤和排序。

`src-tauri/src/services/preset_command_service.rs` 覆盖预设命令计划生成。

`src-tauri/src/services/process_timeline_service.rs` 覆盖 timeline 分页、搜索和类型筛选。
