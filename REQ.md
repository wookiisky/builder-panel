# Builder Panel 需求文档

## 1. 项目概述

### 1.1 产品目标

Builder Panel 是一个运行在 Mac 和 Windows 上的常驻桌面 APP。它以浮动 panel 的形式始终展示在最前，用于实时查看 Coding Agent 状态、处理审批和选项、发送回复、启动预设对话，并按需查看托管会话的过程事件。

首版重点服务四类入口：

1. Codex APP。
2. Codex CLI。
3. Claude Code APP。
4. Claude Code CLI。

### 1.2 目标用户

目标用户是日常使用 Codex、Claude Code 等 Coding Agent 的开发者，尤其是：

1. 同时运行多个 agent 会话的用户。
2. 需要及时处理审批、选项和开放性回复的用户。
3. 希望减少终端、编辑器和 agent 桌面端之间来回切换的用户。
4. 需要排查 agent 执行过程、tool 调用和错误原因的用户。

### 1.3 核心价值

本产品提供一个本地优先的 agent 控制面板：

1. 实时聚合 Codex 和 Claude Code 的状态。
2. 在浮动 panel 中处理审批、选项和回复。
3. 通过快捷回复和预设命令减少重复输入。
4. 在弹出层中查看托管会话的过程事件。
5. 保持高性能、低资源占用，不影响开发环境。

### 1.4 首版范围

首版必须支持：

1. Mac 和 Windows 通用桌面 APP。
2. panel 始终置顶、可拖动、可收缩、可展开。
3. Codex APP、Codex CLI、Claude Code APP、Claude Code CLI 状态展示。
4. 等待审批、等待选择、运行中、完成、失败、失联等状态展示。
5. 系统完成通知和待处理通知。
6. agent 选项展示与点击回写。
7. 开放性回复输入框。
8. 自定义快捷回复。
9. 创建新对话并发送预设命令。
10. 托管会话过程事件弹出层。
11. 设置页和本地配置保存。
12. 首版实现扩展模式，并在类型和布局边界保留未来 mini 模式扩展点。
13. 已验证来源可用时展示 5H 用量和周用量数字。

### 1.5 非目标

首版不做：

1. 云同步、账号体系、团队协作。
2. 代理模型 API。
3. 完整聊天客户端。
4. 所有终端的无差别注入输入。
5. 复杂历史迁移。
6. 插件市场。
7. 未经验证的 agent 私有协议能力。

## 2. open-vibe-island 借鉴与改造要求

### 2.1 借鉴目标

本项目参考 `open-vibe-island/`，但不是移植项目。参考重点是 agent 接入、事件归一、状态管理和本地 bridge，不照搬 macOS-only 技术栈和 notch UI。

必须借鉴：

1. 统一 AgentEvent 思路。
2. 纯 reducer 管理 SessionState。
3. hook helper + local bridge 的本地事件链路。
4. fail-open 原则。
5. pending interaction 模型。
6. 终端跳回和文本回写分离。
7. 用测试覆盖 hook payload、bridge codec、session reducer。

不得照搬：

1. SwiftUI/AppKit 作为主架构。
2. notch / island UI 形态。
3. macOS-only AppleScript 作为跨平台核心能力。
4. 原项目同时支持大量 agent 的广泛范围。

### 2.2 具体落地方案

在 Builder Panel 中，`open-vibe-island` 的设计应映射为：

1. `OpenIslandCore.AgentEvent` 思路映射为 Rust `domain::AgentEvent`。
2. `OpenIslandCore.SessionState.apply` 思路映射为 Rust 纯 reducer。
3. `OpenIslandHooks` 思路映射为 `builder-panel-hook` CLI。
4. Unix socket bridge 思路扩展为 Mac Unix domain socket + Windows Named Pipe。
5. pending approval / question 思路映射为 `AgentInteraction`。
6. terminal jump 和 text sender 思路映射为 `JumpTargetPort` 与 `ReplySenderPort` 两个 port。
7. `CodexAppServer` 思路映射为 Codex APP adapter。
8. 事件流记录思路映射为 `ProcessTimelineService` 的内存接收 adapter。

### 2.3 UI 改造要求

`open-vibe-island` 的 notch UI 只作为“轻量状态面板”的参考，不作为视觉实现目标。

Builder Panel UI 必须改造为：

1. 首版扩展模式：对应完整控制面板，展示 session 列表、执行信息、状态动画、操作区和过程入口。
2. 未来 mini 模式：只作为类型和布局扩展点预留，首版不实现、不验收。
3. 弹出层：对应过程查看，不放在常驻 panel 主区域内。
4. 视觉风格参考 Codex 和 Claude Code 的克制、工程化、可扫读风格。

### 2.4 技术边界要求

1. Domain 层不得引入 `open-vibe-island` 的 Swift 类型或 macOS 概念。
2. 任何从参考项目借鉴的协议字段，必须在本项目中重新定义 schema。
3. 参考项目中的 hook 行为必须重新验证，不得直接假设当前 Codex 或 Claude Code 版本仍兼容。
4. 借鉴方案必须同时覆盖 Mac 和 Windows。
5. 借鉴点必须体现在测试中，尤其是 reducer、bridge 和 fail-open。

## 3. 功能需求

### 3.1 浮动 Panel

需求：

1. APP 必须提供一个常驻浮动 panel。
2. panel 必须始终置顶。
3. panel 必须支持拖动位置。
4. panel 必须支持收缩和展开。
5. panel 必须支持多显示器。
6. panel 必须记住上次位置、大小和收缩状态。
7. panel 默认不抢焦点。
8. 用户点击输入框时，panel 可以临时获得输入焦点。
9. 输入框失焦后，panel 应恢复轻量非激活浮窗行为。
10. panel 在紧凑状态下不得展示复杂内容。
11. 首版必须实现 `expanded` 扩展模式，显示 agent 执行信息、状态动画和完整操作区。
12. 类型和布局边界可以预留 `mini` 模式扩展点，但首版不实现、不进入验收。
13. 扩展模式必须支持回复相关操作。

验收：

1. 用户可在 Mac 和 Windows 上移动 panel。
2. APP 重启后 panel 位置和状态能恢复。
3. 在其他窗口前操作时，panel 保持可见。
4. 未点击输入框时，panel 不主动抢走用户焦点。
5. 用户可使用扩展模式完成核心操作。
6. 扩展模式中能执行当前 session 可用的回复操作。

### 3.2 Agent 会话状态展示

需求：

1. 系统必须显示所有已发现的 agent 会话。
2. 每个会话必须包含 agent 类型、项目名或工作目录、对话 ID、对话标题、状态、摘要、更新时间，以及可用时的 5H 用量和周用量。
3. 状态必须使用强类型定义，不允许散落魔法字符串。
4. 支持状态：
   - `running`：agent 正在工作。
   - `waiting_for_approval`：等待审批。
   - `waiting_for_answer`：等待选项或文本回复。
   - `completed`：当前 turn 完成。
   - `failed`：agent 或 bridge 出错。
   - `detached`：会话失联或本地进程不可见。
5. 等待用户操作的会话必须优先展示。
6. 同状态内按更新时间倒序展示。
7. 会话能力必须显式建模：
   - `can_jump`
   - `can_send_reply`
   - `can_resolve_approval`
   - `can_create_followup_turn`
   - `can_view_process_timeline`
8. UI 展示动作前必须检查能力，不得展示无法执行的假按钮。
9. 5H 用量和周用量由 adapter 能力决定；已验证数据可用时必须以数字展示。
10. 用量数据不可用时必须展示 `--` 或隐藏，不得虚构数字。
11. 用量数据的单位由 adapter 定义，UI 首版只要求展示数字本身。
12. 系统必须区分不同项目下的 agent 会话。
13. 系统必须区分同一项目下的不同对话。
14. 同一个 agent 在不同项目中的会话不得合并展示。
15. 同一个项目中的不同对话不得合并展示。
16. 会话唯一标识必须由 `agent_kind`、`project_id`、`conversation_id` 共同确定；缺失稳定字段时必须由 adapter 生成临时但不冲突的本地 ID。

验收：

1. mock agent 发送 session start 后，panel 展示新会话。
2. mock agent 发送等待审批事件后，该会话排到前面。
3. 不支持回复的会话不展示可发送按钮。
4. mock agent 提供已验证的 5H 和周用量后，UI 展示对应数字。
5. 用量不可用时，UI 不展示虚假数字。
6. 同一 agent 在两个项目中运行时，UI 展示为两个独立项目分组或两条独立会话。
7. 同一项目中两个对话同时运行时，UI 展示为两个独立对话。

### 3.3 用量展示

需求：

1. 系统必须在类型层显式支持 5H 用量和周用量。
2. adapter 提供已验证 5H 用量时，系统必须展示该数字。
3. adapter 提供已验证周用量时，系统必须展示该数字。
4. 用量数字必须能在扩展模式中展示。
5. 用量数据应归属于具体 agent 会话或 agent 账号上下文。
6. 用量字段必须显式建模：
   - `usage_5h`
   - `usage_weekly`
7. 用量数字只展示已验证的数据。
8. 数据不可用时展示 `--` 或隐藏。
9. 不同 agent 的用量单位可能不同，首版不强行换算。
10. 用量必须按 agent 分别展示，不做 Codex 和 Claude Code 统一口径合并。
11. UI 应展示数字和来源标签；单位由 adapter 提供，无法确认单位时只展示数字。
12. Codex 用量优先来自 `/status`、app-server 可验证字段或企业 Analytics。
13. Claude Code 用量优先来自 CLI 可验证输出或 Admin/Analytics API；Analytics 只能作为企业/Admin 可选数据源，不作为个人实时 5H 余量来源。
14. 如果 adapter 能提供更新时间，UI 应展示或在 tooltip 中展示更新时间。

验收：

1. mock agent 返回已验证 5H 用量后，扩展模式展示该数字。
2. mock agent 返回已验证周用量后，扩展模式展示该数字。
3. 用量不可用时不展示虚假数字。
4. 用量更新时，UI 数字随 view model 更新。
5. Codex 和 Claude Code 用量来源不同时，UI 能展示各自来源标签。

### 3.4 Codex APP 接入

需求：

1. Codex APP 必须作为独立 adapter 实现。
2. 如果 Codex APP 提供 app-server 或本地状态协议，优先使用该协议。
3. 系统必须读取 thread 状态、turn 状态和等待标记。
4. 系统必须接收 app-server notification，用于低延迟更新状态。
5. 如果 app-server 支持 RPC，系统应使用 RPC 创建新对话和发送开放性回复。
6. 如果 Codex APP 不支持结构化回写，UI 必须降级，不展示虚假发送能力。
7. Codex APP adapter 不得污染 domain 层。

验收：

1. Codex APP 运行时，系统可发现相关会话。
2. Codex APP 进入等待审批或等待输入时，panel 状态更新。
3. 如果无法回写，UI 展示只读或复制降级。

### 3.5 Codex CLI 接入

需求：

1. Codex CLI 必须通过 hook helper 接入。
2. hook helper 必须监听：
   - `SessionStart`
   - `UserPromptSubmit`
   - `PermissionRequest`
   - `Stop`
3. hook payload 必须先校验，再转换为内部事件。
4. 对审批类事件，系统必须支持用户在 panel 中处理并回写 directive。
5. 开放性回复和快捷回复优先使用受控终端输入；如果不可用，降级为复制文本。
6. 过程事件只通过 hook、app-server 或托管 stdout 事件接收，不从 transcript / JSONL 反向读取。

验收：

1. Codex CLI 启动会话后，panel 展示状态。
2. Codex CLI 请求审批时，用户可在 panel 允许或拒绝。
3. Codex CLI turn 完成后，系统触发完成通知。

### 3.6 Claude Code APP 接入

需求：

1. Claude Code APP 必须作为独立 adapter 实现。
2. 如果 Claude Code APP 暴露稳定本地协议或状态接口，优先使用该接口。
3. 首版按“只读状态 + 跳回 + 有限回写”处理 Claude Code APP。
4. 如果没有稳定接口，只展示已验证的进程级状态、通知和可跳回能力。
5. 不得虚构 Claude Code APP 的结构化回写能力。
6. 如果 APP 会话由 Builder Panel 托管启动，可通过托管进程或终端输入提供开放性回复。
7. 若后续实测发现公开本地协议，必须先更新 adapter schema 和测试，再开放结构化控制。

验收：

1. Claude Code APP 可被发现时，panel 展示对应会话或进程状态。
2. 不支持回写时，UI 不展示发送按钮。
3. 未验证到本地协议前，Claude Code APP 不进入结构化审批回写验收范围。

### 3.7 Claude Code CLI 接入

需求：

1. Claude Code CLI 必须通过 hooks 接入。
2. 支持事件：
   - `SessionStart`
   - `UserPromptSubmit`
   - `PreToolUse`
   - `PermissionRequest`
   - `Notification`
   - `Stop`
   - `SessionEnd`
3. 对 hook 原生阻塞等待的交互，必须通过 stdout directive 回写。
4. 对自然语言提问，首版只支持明确结构化来源，不做复杂自然语言解析。
5. 开放性回复通过 panel 输入框提交，实际回写方式由 `reply_target` 决定。

验收：

1. Claude Code CLI hook 能触发 session 状态更新。
2. `PermissionRequest` 能在 panel 中处理。
3. `Stop` 后能触发完成状态和通知。

### 3.8 审批处理

需求：

1. panel 必须展示 agent 发出的审批请求。
2. 审批信息必须包含工具名、路径、命令摘要或权限范围。
3. 用户必须可以执行允许和拒绝。
4. 如果 agent 支持，允许提供“允许并记住本类权限”。
5. 审批 pending 状态必须显式建模。
6. 用户处理后必须清理 pending 状态。
7. 如果回写失败，必须展示失败原因，并保留可复制内容。

验收：

1. 用户点击允许后，mock agent 收到 allow directive。
2. 用户点击拒绝后，mock agent 收到 deny directive。
3. 回写失败不会让 UI 误显示已成功。

### 3.9 Agent 选项处理

需求：

1. panel 必须展示 agent 给出的选项。
2. 支持单选。
3. 支持多选。
4. 单选可直接点击发送。
5. 多选需要先选中，再提交。
6. 选项文本必须支持长文本换行。
7. 选项提交前必须检查 session 是否仍可回写。
8. 选项提交失败时，必须展示错误并允许用户重试或复制。

验收：

1. mock agent 发送选项后，panel 展示所有选项。
2. 用户点击选项后，mock agent 收到正确答案。
3. 长选项文本不溢出按钮。

### 3.10 开放性回复

需求：

1. session detail 底部必须提供开放性回复输入框。
2. 输入框只在 `can_send_reply` 为 true 时可编辑。
3. 输入框支持单行快速回复。
4. 输入框必要时可展开为多行。
5. 默认 `Enter` 发送。
6. 默认 `Shift+Enter` 换行。
7. 发送前必须校验：
   - 内容不能为空。
   - 内容不能超过配置最大长度。
   - 当前 session 仍可回写。
8. 发送成功后清空输入框。
9. 发送失败后不得清空输入框。
10. 发送失败后必须展示失败原因。
11. 不支持回写时，必须展示复制按钮或只读提示。

验收：

1. 用户输入文本并按 Enter 后，mock agent 收到文本。
2. 空内容不能发送。
3. 发送失败时输入内容仍保留。

### 3.11 快捷回复

需求：

1. 用户必须能创建、编辑、排序、禁用快捷回复。
2. 快捷回复可绑定 agent 类型。
3. 快捷回复可绑定项目或工作目录。
4. session detail 必须展示当前可用快捷回复。
5. 点击快捷回复后，应复用开放性回复的发送路径。
6. 快捷回复发送失败时，应把内容填入开放性回复输入框并提示用户。
7. 快捷回复不得绕过 `can_send_reply` 能力判断。

验收：

1. 用户可创建快捷回复。
2. 点击快捷回复后 mock agent 收到对应内容。
3. 不支持回写时，快捷回复不直接发送。

### 3.12 创建新对话与预设命令

需求：

1. 用户必须能创建预设命令。
2. 预设命令必须包含：
   - 名称。
   - agent 类型。
   - 工作目录。
   - 启动命令。
   - 初始 prompt。
   - 是否自动发送回车。
   - 默认快捷回复组。
3. 如果 agent 提供结构化创建会话能力，优先走结构化 API。
4. 如果没有结构化 API，则打开指定终端并输入命令。
5. 如果终端无法可靠输入，则复制命令并提示用户手动粘贴。
6. Windows 首版只承诺 APP 托管启动的新对话可可靠回写。
7. Mac 首版优先支持 tmux / Ghostty 回写；其他终端先做跳回或复制降级。

验收：

1. 用户可通过预设命令启动一个 mock 新会话。
2. 不支持自动输入时，系统复制命令并展示提示。

### 3.13 托管会话过程事件弹出层

需求：

1. session detail 必须为支持过程事件的托管会话提供“过程”入口。
2. session 列表行可在空间足够时展示入口。
3. 非托管启动或未接入事件流的会话不展示过程入口。
4. 点击入口后以弹出层展示指定托管 session 已接收的过程事件。
5. 过程事件必须按时间线展示。
6. 过程事件必须区分类型：
   - 用户输入。
   - agent 输出。
   - tool 调用。
   - tool 结果。
   - 审批。
   - 系统事件。
   - 错误。
7. 弹出层顶部必须展示 agent、项目名、状态、时间范围。
8. 弹出层必须支持搜索关键词。
9. 弹出层必须支持按类型筛选。
10. 弹出层必须支持复制单条内容。
11. 弹出层必须支持复制当前筛选结果。
12. 弹出层必须支持跳到最新。
13. 弹出层必须支持流式追加新事件。
14. 首版只处理 Builder Panel 托管启动、且已接入事件流的会话过程事件。
15. 如果会话不支持过程事件，不展示入口或展示禁用原因。
16. 弹出层只负责查看，不在其中编辑配置。
17. 过程事件默认只读。
18. 不支持用户手动导出过程事件为文件。
19. 过程事件只接收并保存在内存中。
20. 过程事件不写入文件、数据库或其他持久化存储。
21. 不从 transcript / JSONL 反向读取过程事件。
22. 弹出层关闭后，应释放当前弹出层持有的大文本缓存；会话运行中的增量过程缓存按内存上限保留。

验收：

1. 用户打开过程弹出层后能看到托管 mock session 的 timeline。
2. 搜索关键词后只展示匹配项。
3. 类型筛选生效。
4. 复制单条内容成功。
5. 不支持过程事件的会话不展示虚假入口。
6. UI 不提供过程事件导出入口。

### 3.14 系统通知

需求：

1. turn 完成时必须触发通知。
2. 等待审批时必须触发通知。
3. 等待用户选择时必须触发通知。
4. agent 失败时必须触发通知。
5. 长时间运行超过阈值时可触发通知。
6. 同一 session 短时间重复通知必须合并。
7. 用户正在查看该 session 时，不重复弹通知。
8. 点击通知后聚焦 panel 并展开对应 session。
9. 通知点击不得直接打开过程弹出层。

验收：

1. mock turn 完成后出现系统通知。
2. 点击通知后 panel 定位到对应 session。
3. 同一 session 的重复通知被合并。

### 3.15 设置与配置管理

需求：

1. 设置页必须支持配置 panel 位置、大小、收缩状态。
2. 设置页必须支持开机启动。
3. 设置页必须支持通知开关。
4. 设置页必须支持 agent 接入开关。
5. 设置页必须支持快捷回复组管理。
6. 设置页必须支持预设命令管理。
7. 设置页必须支持终端 profile 管理。
8. 设置页必须支持开放性回复输入框偏好：
   - 单行或多行。
   - 发送快捷键。
   - 最大长度。
9. 设置页必须支持过程弹出层偏好：
   - 默认筛选类型。
   - 是否自动滚动到最新。
   - 最大缓存条数。
10. 配置结构可预留默认 UI 模式字段，首版固定为扩展模式，不提供 mini 切换设置。
11. 设置页必须支持用量展示开关。
12. 设置页必须展示各 agent 用量数据来源状态。
13. 配置必须保存在本地。
14. 配置读取必须经过校验。
15. 配置写入必须采用临时文件加原子替换。

验收：

1. 修改配置后重启 APP，配置仍然生效。
2. 配置文件损坏时，系统使用默认值并提示用户。

## 4. UI 需求

### 4.1 视觉方向

要求：

1. UI 应克制、清晰、工程感强。
2. 不沿用 `open-vibe-island` 的 notch 视觉。
3. 不做营销式大卡片布局。
4. 色彩只表达状态，不做大面积单色主题。
5. 小窗内不放复杂说明文案。
6. 字体大小不得随 viewport 宽度缩放。
7. 字间距保持默认，不使用负字距。
8. 卡片圆角不超过 8px，除非后续设计系统另有规定。

### 4.2 Panel 模式

首版实现扩展模式：

1. 展示 session 列表。
2. 展示更多 agent 执行信息。
3. 展示状态动画。
4. 展示等待用户操作数量。
5. 展示运行中数量。
6. 已验证用量数据可用时展示 5H 用量数字。
7. 已验证用量数据可用时展示周用量数字。
8. 支持点击 session 进入详情区域。
9. 详情区域展示当前 session 的完整操作区。
10. 详情区域展示审批、选项、开放性回复、快捷回复和过程入口。
11. 只展示当前 session 支持的动作。
12. 必须支持回复相关操作。

未来 mini 模式扩展要求：

1. 首版不得把业务能力硬编码到扩展模式专用组件中。
2. 会话能力、当前选中 session、回复草稿、审批状态必须能被未来 mini 入口复用。
3. 类型层可以预留 `mini` 枚举值，但首版不得展示未实现的 mini 切换入口。
4. 未来 mini 模式应只作为轻量入口，不承载复杂过程查看。

### 4.3 Session 列表 UI

每行必须展示：

1. agent 名称。
2. 项目名或工作目录。
3. 对话标题或对话 ID。
4. 状态标签。
5. 最近摘要。
6. 更新时间。
7. 已验证用量数据可用时展示 5H 用量数字。
8. 已验证用量数据可用时展示周用量数字。
9. 可用动作图标。

约束：

1. 行高稳定，动态内容不能造成明显跳动。
2. 摘要超长时截断或多行限制。
3. 状态标签要清晰可扫读。
4. 等待用户操作状态要有明确视觉优先级。

### 4.4 Session Detail UI

必须包含：

1. 标题区：agent、项目、对话标题、状态。
2. 标识区：项目 ID、工作目录、对话 ID。
3. 用量区：5H 用量、周用量。
4. 摘要区：最近状态或当前问题。
5. 执行信息区：扩展模式展示更多 agent 执行信息和状态动画。
6. 操作区：审批、选项、输入框、快捷回复。
7. 工具区：跳回、复制、过程入口等能力按钮。

约束：

1. 不把按钮文字撑破容器。
2. 长命令、路径和输出必须折叠或换行。
3. 不支持能力时，按钮应隐藏或禁用并说明原因。

### 4.5 过程事件弹出层 UI

必须包含：

1. 标题栏。
2. session 基本信息。
3. 搜索框。
4. 类型筛选控件。
5. 时间线列表。
6. 复制按钮。
7. 跳到最新按钮。
8. 加载状态。
9. 空状态。
10. 接收失败提示。

时间线项必须清楚区分：

1. 用户输入。
2. agent 输出。
3. tool 调用。
4. tool 结果。
5. 审批。
6. 系统事件。
7. 错误。

### 4.6 设置页 UI

设置页必须按模块分组：

1. General：开机启动、通知、语言或基础偏好。
2. Display：panel 位置、大小、置顶、扩展模式布局和收缩行为。
3. Agents：Codex APP、Codex CLI、Claude Code APP、Claude Code CLI 接入状态和用量数据来源状态。
4. Replies：开放性回复偏好、快捷回复组。
5. Presets：预设命令。
6. Terminal：终端 profile、受控启动策略。
7. Advanced：bridge、hook 安装、日志、过程事件内存缓存。

## 5. 交互需求

### 5.1 拖动与位置恢复

1. 用户可拖动 panel。
2. 拖动后位置立即生效。
3. 位置保存应防止窗口被恢复到屏幕外。
4. 显示器变化后，应自动移动到可见区域。

### 5.2 展开与收缩

1. 首版默认使用扩展模式。
2. 用户可收缩或展开 panel。
3. 有等待用户操作事件时，可按配置突出提示或保持当前展开状态。
4. 收缩状态必须降低动画和刷新频率。
5. 扩展模式可以展示状态动画，但动画不得影响常驻性能。
6. 收缩或展开后必须保持当前选中 session。
7. 收缩或展开后回复草稿不得丢失。
8. 未来 mini 模式需要复用当前 session、草稿和 pending interaction 状态，首版不实现 mini 交互入口。

### 5.3 审批交互

1. 用户点击允许或拒绝后，按钮进入处理中状态。
2. 处理中不能重复点击。
3. 成功后关闭审批 UI。
4. 失败后恢复按钮并展示错误。

### 5.4 选项交互

1. 单选点击即发送。
2. 多选需要提交按钮。
3. 提交前必须检查至少选择一项。
4. 失败后保持选择状态。

### 5.5 输入框交互

1. 输入框聚焦时 panel 可接收键盘输入。
2. `Enter` 发送。
3. `Shift+Enter` 换行。
4. 发送中展示 loading。
5. 发送失败保留草稿。
6. 用户切换 session 时，应保留每个 session 的未发送草稿。

### 5.6 快捷回复交互

1. 点击快捷回复直接尝试发送。
2. 发送前检查能力。
3. 发送失败后填入输入框。
4. 用户可从设置页编辑快捷回复。

### 5.7 过程弹出层交互

1. 点击过程入口打开弹出层。
2. 弹出层打开时按需加载第一页。
3. 滚动接近底部时加载更多。
4. 搜索时只筛选已加载或由服务端分页返回的匹配结果，具体实现必须保持一致。
5. 筛选类型后列表立即更新。
6. 点击跳到最新滚动到底部。
7. 新事件到达时，如果用户在底部，自动跟随；如果用户正在查看历史，不强行跳动。
8. 关闭弹出层释放大文本缓存。

### 5.8 降级交互

1. bridge 不可用时，hook helper fail-open。
2. 回写不可用时，展示复制文本。
3. 过程事件不可用时，不展示虚假内容。
4. 配置损坏时，使用默认配置并提示用户。

## 6. 技术需求

### 6.1 技术栈

必须采用：

1. Tauri。
2. Rust。
3. TypeScript。
4. React。

推荐采用：

1. Zustand 或 Jotai 作为轻量 UI 状态管理。
2. Vitest 和 Testing Library 做前端测试。
3. Rust 单元测试覆盖 domain 和 adapter。
4. Playwright 做端到端测试。

### 6.2 架构分层

必须按单向依赖设计：

1. `domain`：纯模型、事件、状态 reducer、业务规则。
2. `ports`：抽象接口。
3. `adapters`：Codex、Claude Code、终端、系统通知、配置、IPC。
4. `app-services`：用例编排。
5. `ui`：React panel 和设置页。

依赖方向：

1. UI 依赖 app-services。
2. app-services 依赖 ports 和 domain。
3. adapters 实现 ports。
4. domain 不依赖任何外部协议、Tauri、React、文件系统或系统 API。

### 6.3 Domain 要求

1. Domain 必须纯粹。
2. 所有关键状态必须显式建模。
3. 第三方 payload 必须在 adapter 边界校验。
4. `Any`、裸 `dict`、第三方 JSON 不得进入核心逻辑。
5. 状态变更必须通过 reducer 或明确的用例方法。
6. 所有运行时稳定存在的属性必须在类型层声明。
7. 5H 用量和周用量必须在类型层显式建模。
8. 用量字段必须允许表达“不可用”，但不得用魔法数字表示不可用。
9. 用量字段进入 UI 前必须转换为明确 view model。

### 6.4 IPC 要求

1. Mac 使用 Unix domain socket。
2. Windows 使用 Named Pipe。
3. 协议使用 newline-delimited JSON。
4. 每行一个 JSON envelope。
5. envelope 必须包含：
   - `schema_version`
   - `command_type`
   - `request_id`
   - `payload`
6. response 必须包含：
   - `request_id`
   - `result_type`
   - `payload`
   - `error`
7. bridge 不可用时，hook helper 必须 fail-open。
8. IPC codec 必须有测试。

### 6.5 Hook Helper 要求

`builder-panel-hook` 必须：

1. 从 stdin 读取 payload。
2. 校验 payload。
3. 转换为内部 bridge command。
4. 发送给本地 APP。
5. 对需要回应的 hook 等待用户选择。
6. 超时或 APP 不可用时 fail-open。
7. 不输出无效 directive。
8. 不阻塞 agent 正常运行。

### 6.6 Process Timeline 要求

1. 首版只接收 Builder Panel 托管启动、且已接入事件流的会话过程事件。
2. 所有过程事件必须转换为 `ProcessTimelineItem`。
3. 过程事件不得进入 `SessionState`。
4. 过程事件由 `ProcessTimelineService` 接收、归一和提供给 UI。
5. 过程事件只保存在内存中。
6. 过程事件不得写入文件、数据库或其他持久化存储。
7. 不从 transcript / JSONL 反向读取过程事件。
8. 支持内存分页或分片读取。
9. 支持增量追加。
10. 支持去重。
11. 支持数据源能力判断。
12. 接收失败不得影响主 panel。
13. 必须有内存上限和淘汰策略，避免长时间运行导致内存无限增长。

### 6.7 终端与回写要求

Mac 优先支持：

1. tmux。
2. Ghostty。
3. Terminal.app / iTerm2 跳回。
4. 其他终端优先复制降级，不承诺首版文本注入。

Windows 优先支持：

1. Windows Terminal。
2. PowerShell / cmd 托管子进程。

约束：

1. Windows 首版不承诺向任意已有终端注入输入。
2. APP 托管启动的会话必须记录 process/session handle。
3. 所有回写前必须检查 `reply_target`。
4. Windows 首版只保证 APP 托管启动会话的可靠回写。
5. 不支持 WSL。

### 6.8 配置存储要求

配置路径：

1. Mac：`~/Library/Application Support/BuilderPanel/`。
2. Windows：`%APPDATA%/BuilderPanel/`。

要求：

1. 首版可使用 JSON 文件。
2. 配置读取必须校验。
3. 缺失字段使用显式默认值。
4. 非法字段不进入 domain。
5. 写入采用临时文件加原子替换。
6. 配置写入失败不得覆盖旧配置。
7. 配置结构可预留默认 UI 模式字段；首版固定为扩展模式。
8. 用量展示开关必须持久化。
9. 首版不提供自动更新配置项。

## 7. 性能需求

### 7.1 常驻性能

1. 空闲时 CPU 应接近 0。
2. 禁止前端高频轮询 agent 状态。
3. 状态更新应以事件驱动为主。
4. 常驻任务优先放在 Rust 侧。
5. panel 收缩或不可见时暂停非必要动画。
6. panel 收缩或不可见时降低刷新等级。

### 7.2 内存需求

1. 常驻内存必须轻量。
2. 首版目标低于 Electron 同类方案。
3. 过程事件、大文本不得进入全局 UI 状态。
4. 过程弹出层关闭后必须释放大文本缓存。

### 7.3 大数据展示

1. 过程弹出层必须使用虚拟列表。
2. 默认分页加载。
3. 不得一次性渲染全部过程事件。
4. 长内容默认折叠。
5. 1 万条过程记录滚动应保持可用不卡顿。

### 7.4 性能验收

必须覆盖：

1. 空闲 10 分钟 CPU 占用。
2. 10 个 session 同时存在。
3. 连续 1000 条 mock agent 事件输入。
4. 长过程事件不进入全局前端状态。
5. 弹出层关闭后缓存释放。

## 8. 安全与权限需求

### 8.1 本地优先

1. 首版不得依赖远程服务。
2. 不上传 prompt、transcript、日志或过程事件。
3. 不引入账号体系。

### 8.2 Hook 安装

1. 安装 hook 前必须在设置页明确展示将修改的配置。
2. 修改第三方配置前必须备份。
3. 卸载 hook 必须可逆。
4. 不得静默提权。

### 8.3 敏感内容

1. 首版默认不持久化 transcript 全文。
2. 过程事件只接收并保存在内存中。
3. 首版只保存 session 摘要、状态、配置和用户自定义项。
4. 复制内容由用户主动触发。
5. 不支持持久化过程事件。
6. 不支持导出过程事件文件。

### 8.4 命令安全

1. 不自动发送高风险命令。
2. 预设命令应由用户显式创建。
3. 自动发送回车必须由用户配置。
4. 回写失败不得重复自动发送。

## 9. 错误处理与降级需求

### 9.1 Fail-open

1. hook helper 连接不到 APP 时，不输出阻塞 directive。
2. APP 崩溃不影响 agent 继续运行。
3. 用户长时间不回应审批时，按 agent 协议默认超时。
4. hook helper 不得无限等待。

### 9.2 错误分类

必须显式建模：

1. `BridgeUnavailable`
2. `MalformedAgentPayload`
3. `UnsupportedReplyTarget`
4. `ReplySendFailed`
5. `ConfigLoadFailed`
6. `ConfigSaveFailed`
7. `AgentProtocolUnsupported`
8. `ProcessTimelineUnavailable`
9. `ProcessTimelineReceiveFailed`

### 9.3 日志

日志业务事件名使用中文，例如：

1. `桥接服务启动`
2. `收到Agent事件`
3. `用户回复已发送`
4. `终端回写失败`
5. `配置保存失败`
6. `过程事件接收失败`

日志要求：

1. 不记录敏感全文，除非用户启用调试并明确确认。
2. 不做无意义的 catch-log-reraise。
3. 错误通过结果对象或统一错误链路收敛。

## 10. 测试需求

### 10.1 Domain 测试

必须覆盖：

1. session start 创建状态。
2. activity update 不覆盖 pending approval。
3. approval request 进入等待审批。
4. question request 进入等待回复。
5. completed 清理 pending interaction。
6. detached 不删除历史状态。
7. 多 session 排序。
8. 不同项目的会话不会合并。
9. 同一项目的不同对话不会合并。

### 10.2 Adapter 测试

必须覆盖：

1. Codex hook payload 校验。
2. Claude hook payload 校验。
3. 非法 JSON 被拒绝。
4. hook response 编码正确。
5. Unix socket / Named Pipe codec 一致。
6. hook / app-server / 托管 stdout 事件转换为 `ProcessTimelineItem`。
7. 重复过程事件去重。
8. Codex / Claude Code adapter 在已验证来源可用时能解析 5H 用量和周用量。
9. 用量不可用时 adapter 输出明确 unavailable 状态。
10. adapter 能生成稳定的 `project_id` 和 `conversation_id`。

### 10.3 App Service 测试

必须覆盖：

1. 点击选项回写到正确 pending interaction。
2. 开放性回复选择正确 reply target。
3. 快捷回复复用开放性回复发送路径。
4. 新对话预设命令生成正确。
5. 回写失败降级到复制文本。
6. 打开过程弹出层时读取内存中的对应托管 session 过程事件。
7. 过程事件接收失败不影响主 panel。
8. 用量更新能生成正确 view model。
9. 用量不可用时 view model 不产生虚假数字。
10. 同项目多对话 view model 独立。
11. 多项目多对话 view model 独立。

### 10.4 UI 测试

必须覆盖：

1. 扩展模式基础布局。
2. 长选项文本不溢出。
3. 小屏幕下按钮不重叠。
4. 通知点击定位 session。
5. 不同状态展示正确动作。
6. 输入框单行、多行、发送、失败保留草稿。
7. 过程弹出层打开、关闭、搜索、筛选、复制。
8. 过程弹出层长内容不撑破布局。
9. 扩展模式展示执行信息和状态动画。
10. 扩展模式在用量可用时展示 5H 与周用量数字。
11. session 列表能区分项目和对话。
12. 过程弹出层不展示导出入口。
13. 不展示未实现的 mini 模式切换入口。

### 10.5 性能测试

必须覆盖：

1. 空闲 10 分钟 CPU 占用接近 0。
2. 10 个 session 同时存在时 panel 操作不卡顿。
3. 连续 1000 条 mock agent 事件输入时状态不丢失。
4. panel 收缩后动画和刷新降级生效。
5. 长过程事件不进入全局前端状态。
6. 1 万条过程记录虚拟滚动不卡顿。
7. 弹出层关闭后大文本缓存释放。
8. 过程事件内存缓存达到上限后淘汰策略生效。

### 10.6 端到端测试

必须覆盖：

1. mock agent 开始运行。
2. mock agent 等待审批。
3. 用户点击允许。
4. mock agent 收到 directive。
5. mock agent 完成。
6. APP 展示完成并触发通知。
7. 用户输入开放性回复，mock agent 收到文本。
8. 用户打开过程弹出层，能看到托管 mock session 已接收的 timeline。

## 11. 平台兼容需求

### 11.1 Mac

必须支持：

1. 浮动置顶 panel。
2. 多显示器。
3. 系统通知。
4. Unix domain socket。
5. Codex APP / CLI。
6. Claude Code APP / CLI。

优先支持：

1. tmux 回写。
2. Ghostty 回写。
3. Terminal.app / iTerm2 跳回。

### 11.2 Windows

必须支持：

1. 浮动置顶 panel。
2. 多显示器。
3. 系统通知。
4. Named Pipe。
5. Codex APP / CLI。
6. Claude Code APP / CLI。
7. APP 托管启动的新会话回写。

优先支持：

1. Windows Terminal。
2. PowerShell / cmd 托管子进程。

约束：

1. 不支持 WSL。
2. Windows 可靠回写范围限定为 APP 托管启动的新会话。

## 12. 验收标准

### 12.1 功能验收

1. 用户能看到 Codex APP、Codex CLI、Claude Code APP、Claude Code CLI 会话。
2. 用户能处理审批。
3. 用户能点击 agent 选项并回写。
4. 用户能输入开放性回复并发送。
5. 用户能创建并使用快捷回复。
6. 用户能创建新对话并发送预设命令。
7. 用户能打开过程弹出层查看托管会话已接收的过程事件。
8. 用户能收到完成通知。
9. 已验证用量数据可用时，用户能看到 5H 用量数字和周用量数字。
10. 用户能区分不同项目中的 agent 会话。
11. 用户能区分同一项目中的不同对话。

### 12.2 UI 验收

1. panel 可拖动、可收缩、可展开。
2. 扩展模式展示清晰，并保留未来 mini 模式扩展边界。
3. 长文本不溢出。
4. 按钮不重叠。
5. 过程弹出层可搜索、筛选、复制、跳到最新。
6. 不支持能力时 UI 不误导用户。
7. 扩展模式支持回复相关操作。
8. 扩展模式能展示 agent 执行信息和状态动画。

### 12.3 技术验收

1. domain 不依赖 Tauri、React、文件系统和系统 API。
2. payload 校验发生在 adapter 边界。
3. IPC codec 有测试。
4. hook helper fail-open。
5. 配置读写有校验和原子写入。
6. 过程事件只保存在内存，不写入持久化存储。

### 12.4 性能验收

1. 空闲 CPU 接近 0。
2. 10 个 session 同时存在时操作流畅。
3. 连续事件输入不丢失。
4. 1 万条过程记录虚拟滚动可用。
5. 弹出层关闭后释放缓存。

### 12.5 安全验收

1. hook 安装前有明确确认。
2. 修改第三方配置前有备份。
3. 不静默提权。
4. 默认不上传任何本地过程事件。
5. 默认不持久化敏感全文。
6. 不支持导出过程事件文件。

## 13. 里程碑需求

### 13.1 M0：工程骨架

交付：

1. Tauri + React + Rust 项目。
2. domain / ports / adapters / app-services / ui 目录边界。
3. 基础测试、lint、format、CI。
4. 架构文档和性能预算文档。
5. 扩展模式基础框架。
6. UI 模式类型和布局边界预留未来 mini 扩展点。

验收：

1. Mac 和 Windows 都能启动空 panel。
2. 单元测试可运行。
3. domain 不依赖 Tauri。
4. 有空闲资源占用基线。
5. 扩展模式可展示基础布局。
6. 不展示未实现的 mini 模式切换入口。

### 13.2 M0.5：技术探针

交付：

1. Codex app-server schema、transport、notification 和 RPC 能力验证报告。
2. Codex hooks 事件字段、Windows `commandWindows` 行为和 trust-review 流程验证报告。
3. Claude Code CLI hooks 事件字段、stdout directive 和配置路径验证报告。
4. Claude Code APP 是否存在公开本地协议的验证报告。
5. Codex 和 Claude Code 5H/周用量来源、权限、延迟和可用性验证报告。
6. Tauri 在 Mac/Windows 下置顶、焦点、拖动、轻量窗口阴影和多显示器行为验证报告。
7. Mac tmux / Ghostty 托管启动与回写路径验证报告。
8. Windows 托管 PowerShell / cmd 会话 stdin 回写路径验证报告。
9. Codex 和 Claude Code 的项目 ID、工作目录、对话 ID 获取方式验证报告。
10. 能力矩阵、adapter schema 草案、降级策略和测试清单。

可从 `open-vibe-island/` 借鉴但必须重验：

1. 可借鉴 AgentEvent 归一和 SessionState reducer 的测试思路，用于验证本项目 domain schema。
2. 可借鉴 hook helper + local bridge 的事件链路，用于验证 Codex / Claude Code hook payload 到本地 bridge 的 codec。
3. 可借鉴 fail-open 行为，用于验证 bridge 不可用、APP 不响应、用户超时未处理时 agent 不被阻塞。
4. 可借鉴 pending interaction 模型，用于验证审批、选项和开放性回复的状态收敛。
5. 可借鉴 terminal jump 与 text sender 分离思路，用于验证 Mac/Windows 回写能力矩阵。
6. 可借鉴事件流记录思路，用于验证托管会话过程事件接收、分页、去重和内存上限。

不得从 `open-vibe-island/` 直接复用结论：

1. 不直接复用 Swift 类型、macOS-only AppKit/AppleScript 行为或 notch UI。
2. 不假设参考项目中的 Codex 或 Claude Code hook 字段仍兼容当前版本。
3. 不用参考项目的 macOS-only bridge 结论覆盖 Windows Named Pipe。
4. 不把参考项目支持的大量 agent 范围扩展到本项目首版。

验收：

1. 每个探针都有实测命令、测试样例、观察结果和结论。
2. 每个 adapter 的能力状态明确标注为 `supported`、`degraded` 或 `unsupported`。
3. 外部 schema 有本项目自定义 JSON schema 或 TypeScript/Rust 类型草案。
4. 降级策略覆盖不可回写、用量不可用、过程事件不可用和 bridge 不可用。
5. 探针结论进入后续 M1/M2 的实现边界。

### 13.3 M1：本地 Bridge 和 Mock Agent

交付：

1. Unix socket / Named Pipe bridge。
2. newline-delimited JSON codec。
3. mock hook helper。
4. session reducer。
5. mock timeline 数据源。

验收：

1. mock agent 事件能更新 panel。
2. 用户点击选项能回到 mock agent。
3. bridge 不可用时 hook helper fail-open。
4. mock timeline 可按 session 分页读取。
5. mock agent 可模拟多项目、多对话。

### 13.4 M2：Codex / Claude Code 最小接入

交付：

1. Codex APP adapter。
2. Codex CLI hook adapter。
3. Claude Code APP adapter。
4. Claude Code CLI hook adapter。
5. hook installer / uninstaller。
6. 等待审批和完成通知。
7. hook / app-server / 托管 stdout 过程事件 adapter。
8. 已验证来源可用时的 5H 和周用量 adapter 字段解析。

验收：

1. 四类 agent 入口可按能力展示状态。
2. 审批可回写。
3. 完成通知可触发。
4. 不支持结构化回写的 APP 会话不会展示虚假发送能力。
5. 支持的托管会话可打开过程弹出层。
6. 已验证用量来源可用时，支持的 agent 可展示 5H 和周用量数字。
7. 支持的 agent 可按项目和对话独立展示。

### 13.5 M3：回复、快捷回复和预设命令

交付：

1. 开放性回复输入框。
2. 快捷回复配置。
3. 预设命令配置。
4. 创建新对话。
5. 终端回写能力检测。
6. 过程弹出层基础 UI。

验收：

1. 用户能发送开放性回复。
2. 用户能创建快捷回复并发送。
3. 用户能用预设命令启动新对话。
4. 不支持回写时明确降级。
5. 用户能打开过程弹出层。

### 13.6 M4：UI 完整化和发布准备

交付：

1. panel 视觉重设计。
2. 设置页。
3. Mac / Windows 打包。
4. 过程弹出层搜索、筛选、复制、虚拟滚动。
5. 文档和测试矩阵。
6. 扩展模式状态动画、用量展示和回复入口打磨。
7. 未来 mini 模式扩展边界复查。

验收：

1. panel 在多显示器下可移动、可收缩、可恢复位置。
2. Mac 和 Windows 包可安装。
3. 核心流程端到端测试通过。
4. 空闲和多会话性能达到预算。
5. 大过程记录场景达到性能预算。
6. 扩展模式满足 UI 验收。
7. 未实现的 mini 模式不暴露给用户。

## 14. 已确认决策与实施前验证项

### 14.1 已确认产品决策

1. Claude Code APP 首版按“只读状态 + 跳回 + 有限回写”处理。
2. 未验证到 Claude Code APP 公开本地协议前，不承诺结构化审批和结构化回复。
3. Windows 首版只保证 APP 托管启动会话的可靠回写。
4. Mac 首版优先支持 tmux / Ghostty 回写，其他终端先做跳回或复制降级。
5. 开放性回复输入框默认 `Enter` 发送，`Shift+Enter` 换行。
6. 托管会话过程事件默认只读。
7. 不支持用户手动导出过程事件为文件。
8. 首版不持久化 transcript 全文，也不从 transcript / JSONL 反向读取过程事件。
9. 过程事件只接收并保存在内存中，不跟随历史自动持久化。
10. UI 接受轻量原生窗口阴影，不强求完全无边框透明浮窗。
11. 不支持 WSL。
12. MVP 暂不做自动更新。
13. 5H 用量和周用量按 agent 分别展示，但仅展示已验证来源提供的数据。
14. 用量单位由 adapter 决定，UI 展示数字、来源标签和可用单位；单位不可确认时只展示数字。
15. Claude Code Analytics 仅作为企业/Admin 可选数据源，不作为个人实时 5H 余量来源。

### 14.2 实施前外部验证项

以下验证项应在 M0.5 技术探针中完成，并标注是否可从 `open-vibe-island/` 借鉴验证方法：

1. 验证当前 Codex 版本的 app-server schema，并生成本项目使用的 JSON schema 或 TypeScript schema。
2. 验证当前 Codex hooks 的事件字段、Windows `commandWindows` 行为和 trust-review 流程。
3. 验证当前 Claude Code CLI hooks 的事件字段、stdout directive 和配置路径。
4. 验证 Claude Code APP 是否存在公开本地协议；如果没有，保持只读和有限回写策略。
5. 验证 Codex `/status`、app-server 或企业 Analytics 可提供的 5H/周用量字段。
6. 验证 Claude Code CLI 或 Admin/Analytics API 可提供的用量字段、延迟和权限要求。
7. 验证 Mac tmux / Ghostty 回写路径。
8. 验证 Windows 托管 PowerShell / cmd 会话的 stdin 回写路径。
9. 验证 Tauri 在 Mac/Windows 下置顶、焦点、拖动、轻量窗口阴影的稳定性。
10. 验证 Codex 和 Claude Code 的项目 ID、工作目录、对话 ID 获取方式。

## 15. 调研依据

### 15.1 Codex 官方资料结论

1. Codex App Server 是官方支持的深度集成接口，可用于认证、会话历史、审批和流式 agent 事件。
2. Codex App Server 使用 JSON-RPC，支持 stdio JSONL、WebSocket 和 Unix socket 等 transport。
3. Codex hooks 官方支持 `SessionStart`、`PreToolUse`、`PermissionRequest`、`PostToolUse`、`UserPromptSubmit`、`Stop` 等事件。
4. Codex hooks 支持 Windows 专用命令字段 `commandWindows` / `command_windows`。
5. Codex APP 和 CLI 支持 `/status` 查看 thread ID、context usage 和 rate limits。
6. Codex 企业 Analytics API 支持按日或周查询 workspace / user / client usage。
7. Codex Windows APP 官方支持 Windows native、PowerShell、Command Prompt、Git Bash 和 WSL；本项目首版明确不支持 WSL。

### 15.2 Claude Code 官方资料结论

1. Claude Code CLI hooks 可作为 CLI 侧结构化接入基础。
2. Claude Desktop / Claude Code APP 存在桌面侧能力，但当前未确认有等同 Codex App Server 的公开本地 JSON-RPC 协议。
3. Claude Code APP 首版应只承诺只读状态、跳回和有限回写。
4. Claude Code Analytics / Usage 类接口适合作为企业或 Admin 可选数据源。
5. Claude Code Analytics 不是个人实时 5H 余量来源。

### 15.3 需求落地结论

1. Codex 优先走 app-server，hooks 作为 CLI 生命周期和审批补充。
2. Claude Code CLI 走 hooks。
3. Claude Code APP 在公开协议未确认前不做结构化控制承诺。
4. Windows 回写范围限定为 APP 托管启动会话。
5. 5H 用量和周用量按 agent 分别展示，不统一换算；仅展示已验证来源提供的数据。
6. 托管会话过程事件默认只读，只接收并保存在内存中，不支持导出，不持久化全文。
7. Coding agent 会话必须按项目和对话独立区分展示。
