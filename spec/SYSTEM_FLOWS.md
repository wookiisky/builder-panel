# 系统流程

## 职责

本文档记录已建立的主流程、恢复流程和异常收敛流程。

本文档记录已实现的真实 Codex 接入流程。mock agent 流程只作为测试基线，不作为产品运行时流程。

## APP 启动流程

Tauri 桌面入口启动 Rust 后端。

Rust 后端注册 Tauri command。

Vite 前端渲染基础 panel。

前端通过 `get_panel_probe` 读取基础 panel 探针状态。

前端读取失败时使用本地默认探针状态展示空 panel。

## panel 状态流程

阶段 0 的 panel 默认处于 `expanded` 模式。

前端读取设置后将 Panel `collapsed` 归一化为 `false`。

前端不展示收缩按钮。

panel 窗口状态局部保存 command 不持久化 `collapsed`。

Tauri 环境读取设置后尝试恢复上次窗口位置、尺寸和置顶偏好。

Tauri 环境监听窗口移动和尺寸变化。

窗口位置或尺寸变化后，前端通过 panel 窗口状态局部保存 command 持久化几何信息。

设置保存成功且响应仍是最新请求时，前端按保存响应应用当前窗口置顶偏好。

浏览器开发环境不执行 Tauri 窗口偏好应用和几何恢复。

用户点击关闭按钮时，前端通过窗口 API 请求关闭当前 Tauri 窗口。

浏览器开发环境中的关闭按钮不执行系统窗口关闭。

## session 列表流程

前端读取 Codex CLI 和 Codex APP session 列表。

Codex CLI session 列表只来自当前 APP 进程内已经捕捉到的实时 hook 状态。

Codex APP session 列表可来自当前 APP 进程内实时状态、app-server 已加载 thread id 对应的 `thread/read` 或 `thread/list` 元数据和 Codex rollout 历史补齐。

Codex APP 内部建议生成等隐藏 turn 不进入 session 列表；若隐藏 turn 只留下启动空壳，runtime 会清理该空壳并刷新列表。

Codex APP 当前已加载且 `active` 的 thread 即使暂时没有真实标题或预览文本，也可创建运行中 session 列表项。

Codex APP 已加载 thread 的元数据没有可信 cwd 时，不创建真实项目 session 列表项。

APP 启动时首次读取仍可返回空列表，因为 Codex APP app-server 或历史补齐可能不可用。

APP 启动前已经开始且当前仍在 Codex APP app-server 中 loaded/running 的任务，可通过 loaded thread 同步进入 session 列表。

APP 启动前已经开始、app-server 标记为 `notLoaded` 但本地 rollout 在最近活跃窗口内仍未完成且有可展示内容的 Codex APP 任务，可通过 recent active rollout 同步进入 session 列表。

APP 启动后仍在运行的任务如果继续发出 hook、notification 或 server request，也会在后续刷新中进入 session 列表。

单一来源读取失败时，前端保留其它来源结果。

前端合并 session 后按展示分组和首次观察顺序排序。

运行中、等待审批和等待回复的未完成 session 展示在完成、失败和失联 session 上方。

刷新时新观察到的 session 按返回数组顺序分配前端首次观察序。

状态变化可以让 session 在未完成分组和已结束分组之间移动；摘要或更新时间变化不改变首次观察序。

前端顶部状态区从合并后的 session 计算运行中数量和总数。

前端顶部状态区从 Codex CLI 和 Codex APP session 聚合工具整体用量。

同一工具同一 `source_key` 的整体用量只保留更新时间最新值。

前端主体以单列列表展示所有 session。

等待用户回复的 session 在行内展示最后一段输出和回复区。

完成或失败且可 follow-up 的 Codex APP session 默认保持单行，点击右侧展开按钮后展示自由输入区。

有选项的回复区展示选项按钮。

无选项的回复区展示设置中的自定义快捷输入。

## session 实时更新流程

Codex CLI hook、Codex APP hook、Codex APP app-server 和 Codex rollout tailer 事件先在 adapter 边界清洗为归一事件。

adapter 清洗 Codex APP 最后消息时只保留用户输入原文、assistant 输出原文和完成时最后 assistant 输出；工具调用、hook 工具事件和工具参数不写最后消息。

工具输出结束事件不写活动摘要。

Codex APP runtime 会跟踪只由启动信号创建的空壳候选；后续命中内部提示词的 `UserPromptSubmit` 会清理仍为空壳的 session 及相关 runtime 缓存。

runtime 通过 session 更新端口发布轻量通知。

Tauri 事件发布器按 session 合并更新，并在短节流窗口后向前端发送 `session_updated` 事件。

`session_updated` 事件只包含 runtime source、session key 和更新时间。

前端订阅 `session_updated` 后短延迟刷新 session 列表。

前端 session 列表刷新进行中收到新的实时更新时，会在当前刷新结束后补一次刷新。

前端连续收到高频实时更新时，必须周期性刷新列表，不等待事件完全停止。

前端仍保留定时刷新作为实时事件缺失时的兜底。

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

Settings Service 保存前会清洗自定义快捷输入。

Settings Service 保存前会强制 Panel `collapsed` 为 `false`。

保存设置时，JSON 设置文件 adapter 先写入同目录临时文件。

临时文件写入和刷盘成功后，adapter 替换目标配置文件。

临时文件写入失败时，旧配置文件保持不变。

浏览器开发环境从 localStorage 读取 fallback 设置，并在使用前校验结构。

浏览器 fallback 设置同样会清洗自定义快捷输入并强制 Panel `collapsed` 为 `false`。

## hook 安装流程

设置页以列表展示 Codex CLI hook 和 Claude CLI hook 状态。

设置页打开后读取 hook 安装状态。

Tauri hook 安装 command 使用默认路径生成安装器。

默认 hook helper 路径来自当前应用同目录下的 `builder-panel-hook`。

环境变量 `BUILDER_PANEL_HOOK_PATH` 存在时覆盖默认 hook helper 路径。

hook 安装器读取 manifest 和实际配置文件生成状态。

严格已安装状态会禁用安装入口。

未安装且没有目标 manifest 记录时会禁用卸载入口。

需要修复状态允许用户点击安装重写受管 handler。

用户点击单项安装后，hook 安装器备份已存在的第三方配置文件。

hook 安装器在写入前保护本轮会覆盖的旧备份和旧 manifest。

hook 安装器读取 JSON 配置，配置不存在时使用空对象。

hook 安装器移除已有 Builder Panel hook handler。

hook 安装器写入当前 agent 支持的命令型 hook handler。

hook 安装器写入安装 manifest，并保留其它 agent 的 manifest 记录。

用户点击单项卸载后，hook 安装器读取 manifest。

安装前配置存在时，卸载恢复备份文件。

安装前配置不存在时，卸载删除本次创建的配置文件。

单项卸载写回剩余 manifest 失败时，hook 安装器回滚目标配置和旧 manifest。

最后一个 agent 卸载成功后，hook 安装器删除 manifest。

安装或卸载完成后，设置页重新读取 hook 安装状态。

hook 安装器不执行真实 Codex 或 Claude Code 进程。

hook 安装器不绕过 Codex hook trust review。

## 发布质量流程

spec 文档门禁扫描 `spec/` 下实际存在的 Markdown 文档。

spec 文档门禁校验索引覆盖、职责说明、代码入口、测试或验收入口。

spec 文档门禁拒绝代码块、流程图语法和 Markdown 表格。

性能预算脚本不声明 Mac 或 Windows 人工性能验收完成。

## 通知流程

Notification Service 接收已清洗的通知请求。

当前查看 session 与通知 session 相同时，Notification Service 不生成通知计划。

同一 session 同一类型通知在合并窗口内重复出现时，Notification Service 增加合并数量。

通知点击被转换为聚焦 panel、展开 panel 和定位 session。

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

Codex CLI hook payload 携带 transcript path 时，runtime 记录该 session 的 rollout tail 目标。

bridge server 对每个已接收连接启动独立处理线程。

长等待的 `PermissionRequest` 不阻塞 listener 接收后续 hook 请求。

非阻塞 hook 事件写入 runtime session state 后返回 ack。

`PermissionRequest` 写入 pending approval 后，bridge server 等待 panel 审批决策。

用户在 panel 点击允许或拒绝后，runtime 唤醒等待中的 bridge request。

bridge server 返回 Codex allow 或 deny stdout directive。

等待超时时，runtime 移除对应 pending approval 并写入失败 session 状态。

等待超时或 runtime 锁损坏时返回 bridge error，hook helper fail-open。

bridge server 首次 bind 失败时不会标记为已启动，后续读取 Codex CLI session 时可重试启动。

Codex rollout watcher 在 bridge server 首次成功启动后启动。

watcher 只 tail 已知 Codex CLI 或 Codex APP session 的已知 rollout path。

watcher 新增目标时从当前 EOF 后开始读取，不回放历史行。

watcher 读取新增完整行后，在 adapter 边界清洗为活动或完成事件。

watcher 生成的事件按 session 所属 runtime 写回 Codex CLI 或 Codex APP runtime。

## Codex APP app-server 流程

Codex hook helper 仍通过 `--source codex` 接收 Codex hook payload。

hook payload 中 `terminal_app` 归一化后等于 `codexapp` 时，payload 被归类为 Codex APP session。

Codex APP hook request 进入 Codex APP runtime，不进入 Codex CLI runtime。

Codex APP `PermissionRequest` 写入 pending approval 后等待 panel 决策。

用户在 panel 点击允许或拒绝后，runtime 唤醒等待中的 hook request。

bridge server 返回 Codex APP allow 或 deny stdout directive。

Codex APP adapter 通过本机 `codex app-server generate-json-schema --experimental` 生成 schema。

schema 探针确认当前 client request、client response、notification 和回写 response 相关 schema 文件存在。

schema 探针覆盖所有当前 adapter 已消费的 app-server notification schema。

adapter 编码 `initialize`、`initialized`、`thread/start`、`thread/resume`、`turn/start`、`thread/loaded/list`、`thread/read` 和 `thread/list` JSON-RPC 消息。

Codex APP 开关开启后，后端尝试启动 `codex app-server --listen stdio://`。

app-server 启动后，后端发送 `initialize` 并写入 `initialized` notification。

session 列表和详情 command 先返回当前 runtime 状态；Codex APP app-server loaded thread id、thread 元数据和 rollout 补齐通过后台同步线程执行。

app-server 启动后，后台同步可按节流窗口调用 `thread/loaded/list` 获取当前已加载 thread id，再优先用 `thread/read` 按 id 精确创建或补齐当前已加载 thread；`thread/read` 不可用、超时或出现方法/协议类错误时降级为一次有限数量 `thread/list` 读取。

后台当前已加载 `active` thread metadata 在具备可信 cwd 时可创建运行中 session；历史 `active`、`idle` 或 `notLoaded` 的无内容 metadata 只补齐已有同 thread session。

后台同步每轮最多按 rollout 扫描节流窗口读取一次 recent rollout；同一批快照同时服务历史候选补齐和 recent active rollout 恢复。

后台 thread metadata 的预览文本命中内部提示词过滤规则时，不创建新的可见 session。

后台 thread metadata 的状态只用于创建新的已加载 session；已有实时 session 只补齐缺失信息，不用 metadata 覆盖运行状态或最新摘要。

后台 thread metadata 补齐已有 session 的标题、项目、跳回目标或能力时，会发布 `session_updated` 通知前端刷新列表。

后台同步会先用 Codex session index 直接补齐当前已知但缺标题或标题形似模型名的 session。

存在待识别 thread 或当前已知但缺标题的 thread 时，后台同步可按节流窗口调用有限数量的 `thread/list` 补齐 thread 历史元数据。

Codex APP session 列表和详情读取可通过 Codex rollout JSONL 补齐真实 cwd 和最新 Agent 输出。

Codex rollout 历史补齐已有 session 的真实 cwd、跳回目标、能力或摘要时，会发布 `session_updated` 通知前端刷新列表。

Codex APP recent rollout 扫描结果默认只补齐已知候选 thread，不从任意历史 rollout 创建当前 session。

Codex APP recent active rollout 恢复是受限例外：快照必须未完成、处于配置的活跃窗口内、具备可信 cwd 和可展示摘要或 Agent 输出；窗口由 `BUILDER_PANEL_CODEX_APP_ACTIVE_ROLLOUT_WINDOW_MINUTES` 配置，默认 5 分钟。

Codex APP thread 元数据携带的 rollout path 只有在快照 session ID 与 thread ID 一致且属于候选集合时才会应用。

Codex APP thread 元数据携带的 rollout path 可作为该 thread 的实时 tail 目标。

Codex APP path-only thread metadata 不创建新 session；runtime 已有该 thread 可信 cwd 时，可用于补齐标题和 rollout path。

app-server 启动和初始化在全局 client slot 锁外执行；slot 在启动期间仅标记为启动中。

app-server stdout 由后台线程按行读取。

app-server response 唤醒对应 pending request。

app-server notification 和 server request 在 adapter 边界清洗后写入 Codex APP runtime。

Codex APP app-server 事件缺少可信 cwd 时会先建立待识别项目 session，且不生成跳回目标。

Codex APP `thread/name/updated` notification 只更新 thread 标题；形似模型名的值会被忽略，不覆盖真实标题。

待识别 session 迁移到真实 cwd 后，runtime 会对真实 session key 发布 `session_updated` 通知。

app-server notification 和 server request 写入 runtime 前，会先用可信 cwd 统一 session key；不得用 Builder Panel 进程 cwd 代替 Codex thread cwd。

`item/agentMessage/delta` 会按 thread 累积有界的当前 turn 输出。

`turn/started` 和 follow-up 成功提交会清空对应 thread 的当前 turn 输出。

`turn/completed` 和 `idle` 状态优先使用当前 turn 最新 Agent 输出作为 session 摘要；没有 Agent 原文时不写完成或空闲兜底摘要。

adapter 接收 app-server notification 后，在 adapter 边界读取 JSON 字段并转换为归一事件。

Codex APP app-server 审批、文本回复或选项回写成功后，runtime 只清理 pending interaction 并保持运行态，不提前写 `TurnCompleted`。

未识别 notification 不写 session 状态。

未识别或畸形 server request 回写 JSON-RPC error；能识别 thread 时写失败状态。

app-server 启动或初始化失败时，后端尝试回收子进程，前端本次刷新跳过 Codex APP 来源。

Codex APP follow-up turn 通过 `turn/start` 写入 app-server。

Codex APP follow-up turn 写入前，app-server client 先读取 loaded thread id 列表；目标 thread 未加载时先 `thread/resume`。

Codex APP follow-up turn 创建前，runtime 校验 session 无 pending interaction 且处于完成或失败状态；`idle` 状态 notification 会先折叠为完成态。

Codex APP follow-up turn 写入 app-server 成功后，runtime 才写入用户输入原文事件。

Codex APP follow-up turn 在 app-server client 获取、thread 加载确认、thread resume 或 turn/start 任一阶段失败时，runtime 会释放 follow-up 提交占位。

Codex APP session 跳回目标使用 `codex://threads/<thread_id>`。

## mock 测试基线流程

Mock agent adapter 生成已清洗的归一事件，用于 Rust adapter 和 service 测试。

Mock agent runtime 使用 Domain reducer 折叠事件。

Session Service 的 mock 测试从 mock agent runtime 读取 session state。

Session Service 调用 Domain view model 转换列表和详情。

前端产品运行时不读取 mock session 列表。

## 交互测试基线

Interaction Service 的 mock 测试校验 pending approval、pending choice 和失败保留规则。

Reply Service 的 mock 测试校验文本回复内容、pending text reply 和失败保留规则。

Mock agent runtime 记录 allow、allow and remember、deny、choice 和 text reply directive。

Mock agent runtime 写入 `TurnCompleted` 事件，用于验证 Domain reducer 清理 pending interaction。

mock 回写失败测试断言 pending interaction 不被清理。

## 快捷回复链路

Shortcut Reply Service 根据启用状态、agent 类型、项目 ID 和排序值输出可用快捷回复。

前端只在文本回复可回写时展示快捷回复。

快捷回复点击后复用文本回复提交路径。

快捷回复发送失败时，快捷回复内容填回当前 session 草稿。

## 自定义快捷输入链路

自定义快捷输入从 Settings Service 的 Replies 设置读取。

设置页允许新增、编辑、启用、禁用、排序和删除自定义快捷输入。

设置保存边界会清洗非法自定义快捷输入。

前端只在无选项且文本可回写或可创建 follow-up 时展示自定义快捷输入。

自定义快捷输入点击后复用文本回复或 follow-up 提交流程。

提交失败时，自定义快捷输入内容填回当前 session 草稿。

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

前端点击 session 时，只对具备跳回能力且存在跳回目标的 session 调用跳回 command。

跳回 command 按 runtime source 读取对应 runtime 的 session 状态。

Codex APP session 的 `codex://` 跳回目标在 macOS 上交给系统打开。

跳回失败时返回复制降级，不触发文本回写补偿。

## 异常收敛流程

前端调用 `get_panel_probe` 失败时，不写入错误事实。

前端使用默认展示状态继续渲染。

hook helper fail-open 时不写业务状态。

Codex APP app-server 回写失败时不清理 pending interaction。

Codex APP 回复回写失败时前端不清理草稿。

阶段 0 和阶段 2 均不展示错误通知。

## 代码入口

`src-tauri/src/tauri_api/commands.rs` 是基础探针 command 入口。

`src/views/BuilderPanelApp.tsx` 是前端启动和降级展示入口。

`src-tauri/src/domain/panel_geometry.rs` 是位置修正入口。

`src-tauri/src/adapters/bridge/hook_cli.rs` 是 hook helper 流程入口。

`src-tauri/src/adapters/bridge/codec.rs` 是 bridge codec 流程入口。

`src-tauri/src/adapters/bridge/transport.rs` 是 bridge 传输流程入口。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 是 Codex CLI hook 流程入口。

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP app-server 探针和 notification 转换入口。

`src-tauri/src/adapters/codex_app/codex_rollout.rs` 是 Codex rollout 历史快照清洗和实时追加行 tail 入口。

`src-tauri/src/ports/session_update_port.rs` 是 session 实时更新端口入口。

`src-tauri/src/tauri_api/events.rs` 是 Tauri session 更新事件发布入口。

`src-tauri/src/services/session_service.rs` 是 mock 测试基线 session 读取流程入口。

`src-tauri/src/services/interaction_service.rs` 是 mock 测试基线审批和选项链路入口。

`src-tauri/src/services/reply_service.rs` 是 mock 测试基线回复链路入口。

`src-tauri/src/services/shortcut_reply_service.rs` 是快捷回复链路入口。

`src-tauri/src/services/preset_command_service.rs` 是预设命令计划入口。

`src-tauri/src/adapters/terminal/mod.rs` 是跳回降级链路入口。

`src-tauri/src/services/settings_service.rs` 是设置读取和保存流程入口。

`src-tauri/src/adapters/config_file/mod.rs` 是设置文件读写流程入口。

`src/api/sessionJumpApi.ts` 是前端跳回 command 调用入口。

`src/api/sessionUpdateApi.ts` 是前端 session 更新事件订阅入口。

`src/api/panelWindowApi.ts` 是前端窗口偏好应用、几何恢复和关闭调用入口。

`src-tauri/src/services/notification_service.rs` 是通知计划流程入口。

`src-tauri/src/adapters/notification/mod.rs` 是记录型通知 adapter 入口。

## 相关测试

`src-tauri/src/domain/panel_geometry.rs` 覆盖位置修正正常路径和边界路径。

`src-tauri/src/adapters/bridge/hook_cli.rs` 覆盖 hook helper fail-open 和 directive。

`src-tauri/src/adapters/bridge/codec_tests.rs` 覆盖 NDJSON 解析流程。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 覆盖 Codex CLI hook 流程。

`src-tauri/src/adapters/codex_app/mod.rs` 覆盖 Codex APP app-server 边界转换。

`src-tauri/src/services/settings_service.rs` 覆盖设置缺失、损坏和保存流程。

`src-tauri/src/services/notification_service.rs` 覆盖通知抑制、合并和点击定位流程。

`src-tauri/src/services/interaction_service.rs` 覆盖审批成功和失败路径。

`src-tauri/src/services/reply_service.rs` 覆盖文本回复成功、校验失败和回写失败。

`src-tauri/src/services/shortcut_reply_service.rs` 覆盖快捷回复过滤和排序。

`src/api/settingsApi.test.ts` 覆盖自定义快捷输入清洗。

`src/views/BuilderPanelApp.test.ts` 覆盖工具用量聚合。

`src-tauri/src/services/preset_command_service.rs` 覆盖预设命令计划生成。
