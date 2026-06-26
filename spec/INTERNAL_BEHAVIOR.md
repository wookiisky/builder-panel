# 内部行为

## 职责

本文档记录内部模块协作约束、状态不变量和边界破坏风险。

本文档不复制外部 agent 协议。

## 依赖约束

Domain 不依赖 Tauri。

Domain 不直接读写文件系统。

Domain 不依赖异步运行时。

Domain 不接收 `serde_json::Value` 作为核心逻辑输入。

Domain 不依赖 adapter、service 或 tauri_api。

ports 不反向依赖 adapter 具体实现。

前端 `api` 层不反向依赖 UI 或 store。

前端 UI 不直接引用 Rust 工程或 adapter。

## 状态不变量

阶段 0 只存在 `expanded` panel 模式。

Panel `collapsed` 字段保留在设置模型中，但运行时必须归一化为 `false`。

panel 位置修正函数不执行 IO。

panel 位置修正函数不修改 panel 尺寸。

`SessionKey` 由 agent、项目和对话共同确定。

`UsageValue::Unavailable` 是用量不可用的唯一 Domain 表达。

已验证用量必须显式携带来源键和作用域。

pending interaction 由 reducer 统一管理。

pending interaction 的会话键以外层事件会话键为准。

view model 由 Domain 纯转换生成，UI 不需要理解内部状态细节。

view model 动作由 session 状态、pending interaction 和 capability 共同决定。

session detail view model 显式暴露 pending interaction ID 和 pending interaction 类型。

session list item view model 显式暴露行内交互摘要。

跳回动作只在 session 具备跳回能力且存在跳回目标时生成。

mock agent runtime 是本地内存测试基线，不是持久化事实来源，也不是产品运行时 session 来源。

mock agent runtime 通过 Domain reducer 写入测试用 session 状态。

Codex CLI runtime 是阶段 4 真实 hook 状态来源，当前仍为进程内内存状态。

Codex CLI runtime 在事件入口拒绝 `agent_kind` 不是 Codex CLI 的归一事件和 hook payload。

Codex CLI runtime 提供按 `(cwd, thread_id)` 删除孤儿 session 的入口；删除时同时清理对应 rollout path 和该 session 的 pending approval，并通过 session 更新端口发布通知。

Codex CLI runtime 可记录已知 session 的 rollout path，用于实时 tail 该 session 的新增追加行。

Codex CLI pending approval 必须同时匹配 `SessionKey` 和 `InteractionId` 后才能唤醒 hook request。

Codex CLI pending approval 超时后必须从 runtime 移除，并清理 session pending interaction。

Codex APP hook payload、app-server notification 和 app-server server request 只在 adapter 边界读取 `serde_json::Value`，转换后只向 Domain 写归一事件。

Codex APP runtime 是 Codex APP hook 和 app-server 状态来源，当前仍为进程内内存状态。

Codex APP runtime 在事件入口拒绝 `agent_kind` 不是 Codex APP 的归一事件和 hook payload。

Codex APP runtime 收录新的 `thread_id` 后，必须通过注入式回调通知 Codex CLI runtime 清理同 `(cwd, thread_id)` 的孤儿 session；通知必须在更新 `thread_id -> cwd` 映射后立即触发。

Codex APP runtime 必须用 `thread_id -> cwd` 映射统一 hook 通道和 app-server 通道的 `SessionKey`。

Codex APP runtime 只能用可信 cwd 建立真实项目 session key；可信 cwd 来源包括 hook payload、app-server message、app-server `thread/list` 元数据和 Codex rollout `session_meta`。

Codex APP runtime 不得使用 Builder Panel 进程 cwd 代替 Codex thread cwd。

Codex APP runtime 在缺少可信 cwd 时使用待识别项目占位 session，且占位 session 不生成跳回目标。

Codex APP runtime 维护最多 65535 字符的当前 turn Agent 输出缓冲；新 turn 开始或 follow-up 成功提交时必须清空对应 thread 的当前输出。

Codex APP runtime 在完成或 idle 事件中优先保留当前 turn 最新 Agent 输出。

Codex APP runtime 接收 `thread/name/updated` 时只更新 thread 标题，不重置 session 状态、pending interaction 或最后消息。

Codex CLI adapter 写入 session 摘要时，只能使用用户输入原文、assistant 输出原文或完成时最后 assistant 输出。

Codex APP adapter 写入最后消息时，只能使用用户输入原文、assistant 输出原文或完成时最后 assistant 输出。

Codex CLI 和 Codex APP adapter 不为工具 hook 事件、工具参数或工具结束事件写活动摘要。

Codex CLI 和 Codex APP adapter 使用可信 cwd 派生项目展示名；`.claude/worktrees` 和 `.git/worktrees` 路径显示项目根目录名。

Codex rollout adapter 不得把未知工具 JSON arguments 作为 preview 写入 Domain。

没有原文或 preview 的 session start、turn start、turn complete、idle 和工具开始事件不得覆盖已有 session 摘要。

Codex APP app-server `thread/loaded/list` 只提供当前已加载 thread id；当前已加载 thread 需要通过 `thread/list` 元数据补齐 cwd、标题、状态、预览和 rollout path。metadata 状态只应用于新建 session，若 session 已存在，只能补齐缺失信息，不得用后台 metadata 折叠实时运行状态。

Codex APP session index 标题可直接补齐当前已知 session；只允许替换缺失标题或形似模型名的标题，不得覆盖已有真实标题，不得创建新 session。

Codex APP thread metadata 补齐已有 session 的标题、项目、跳回目标或能力时，必须发布 `session_updated` 事件。

Codex APP app-server `thread/list` 边界清洗必须跳过单条无效 thread，保留同批有效 thread 继续补齐。

Codex APP app-server `thread/list` 中缺少 cwd 但带 path 的 thread 不得直接创建或迁移 session；runtime 已有该 thread 可信 cwd 时，可用该 metadata 补齐标题和 rollout path。

Codex APP app-server `thread/list` 中 `status.type` 类型错误属于单条脏数据，必须跳过该 thread，不得默认成 idle。

Codex APP rollout 历史只能应用到已有或可迁移的候选 session，不得单独创建当前 session。

Codex APP recent rollout 扫描只能应用到已知候选 thread，包括已通过 `thread/list` 元数据补齐的当前已加载 thread、thread 历史返回的待识别或缺标题 thread、当前待识别 thread 和当前已知但缺标题的 thread；单独的 loaded thread id 不创建 session 或 rollout 候选，不得把无关历史 rollout 拉入当前 session 列表。

Codex APP thread 历史元数据只能应用到当前待识别 thread 或当前已知但缺标题的 thread；thread 元数据携带的 rollout path 必须与该 thread ID 匹配后才能补齐 runtime。

Codex APP rollout path 必须限制在 Codex sessions root 内，并在读取前校验文件名、普通文件类型和文件大小。

Codex rollout tailer 必须限制在 Codex sessions root 内，并在读取前校验文件名、普通文件类型和文件大小。

Codex rollout tailer 只接收已知 session 的 watch target，不得从任意 rollout 文件创建 session。

Codex rollout tailer 按 canonical path 去重。

Codex rollout tailer 遇到文件截断、替换、超大文件或超长行时必须降级跳过或重置本地读取状态，不得污染 Domain。

Codex rollout tailer 输出必须先清洗为归一事件，不得把 rollout 原始 JSON 写入 Domain。

Codex APP pending approval 必须同时匹配 `SessionKey` 和 `InteractionId` 后才能唤醒 hook request 或回写 app-server response。

Codex APP app-server 审批、文本回复或选项回写成功后，只写入 `InteractionCompleted` 清理 pending 并保持运行态，等待真实完成或 idle 事件后才允许 follow-up。

Codex APP app-server `notLoaded` 必须清理 session pending interaction 和对应 app-server RPC 上下文。

审批提交必须同时匹配 `SessionKey` 和当前 pending approval 的 `InteractionId`。

文本回复提交必须同时匹配 `SessionKey` 和当前 pending text reply 的 `InteractionId`。

选项提交必须同时匹配 `SessionKey` 和当前 pending choice 的 `InteractionId`。

选项提交值必须来自当前 choice interaction。

单选 interaction 只能提交一个选项值。

文本回复最大长度为 1000 个字符。

前端回复草稿按 session key 派生 ID 隔离保存。

前端 follow-up 展开状态按 session key 派生 ID 隔离保存，只影响 UI 展示。

完成或失败 session 的 follow-up 提交成功后，前端必须清理对应草稿和展开状态。

前端选项选择按 interaction ID 隔离保存。

快捷回复不得绕过 Reply Service 的能力校验和 pending 校验。

自定义快捷输入不得绕过当前 session 的回写或 follow-up 能力校验。

跳回能力和文本回写能力必须分别建模。

本地 bridge request 和 response 由显式 enum 表达 command 类型、结果类型、hook 事件名和 directive 类型。

Claude PreToolUse 工具权限由显式 allow、deny、ask 决策表达。

hook payload 中的第三方 JSON 先在 adapter 边界完成基础校验，再进入本项目 bridge command。

`serde_json::Value` 只允许停留在 adapter 或边界 payload 中，不进入 Domain 核心输入。

## 并发和副作用

阶段 0 没有后台 worker。

阶段 2 没有后台 worker。

阶段 3 没有后台 worker。

阶段 4 Codex CLI bridge server 是后台监听线程。

Codex CLI bridge server 的 listener 不等待单个 hook request 完成。

Codex CLI bridge server 每个连接由独立线程处理。

Codex CLI bridge server 在按 `agent_kind` 分流前，会按 hook payload 的 `terminal_app` 与 hook helper env 兜底改判 Codex APP；仍是 Codex CLI 时，会用 Codex APP runtime 已知 thread/cwd 再判一次。

Codex CLI bridge 分流的同步刷新只允许调用一次注入式回调；回调内部只能使用已存在且仍存活的 app-server client，thread list 拉取必须有总时长上限，不得启动 app-server 或无限阻塞 hook 路径。

Codex rollout watcher 是后台轮询线程。

Codex rollout watcher 同步 watch target 时只短暂读取 runtime 中的已知目标，文件 IO 不持有 runtime 锁。

Codex rollout watcher 读取新增追加行后，按事件所属 runtime 写回 Codex CLI 或 Codex APP runtime。

runtime 应用归一事件后通过 session 更新端口发布轻量通知。

Tauri session 更新发布器按 session 合并高频通知并节流发送。

session 更新通知不得携带第三方原始 JSON 或大文本正文。

阶段 0 没有托管进程。

阶段 0 没有持久化写入。

Tauri command 只返回基础探针状态，不执行外部系统访问。

Domain reducer 不执行 IO。

Domain view model 转换不执行 IO。

hook helper 读取 stdin、连接 bridge 和写 stdout 都属于系统边缘副作用。

bridge transport 执行 socket 或 pipe IO。

hook helper 输出 directive 前必须校验 response 与当前 request 和 agent source 匹配。

Mock agent runtime 记录 directive 和折叠事件属于本地测试副作用。

Mock agent runtime 可记录审批、选项和文本回复 directive。

Tauri 产品 command 不访问 mock runtime。

Tauri command 使用进程内 Codex CLI runtime 锁收口阶段 4 Codex CLI 状态访问。

Tauri command 使用进程内 Codex APP runtime 锁收口 Codex APP 状态访问。

前端在 panel 打开期间定时刷新 Codex CLI 和 Codex APP session，避免真实 hook 或 app-server 事件在首次加载后不可见。

Codex APP 后台 metadata、session index 或 rollout 历史补齐已有 session 的可见字段时，后端必须发布 `session_updated` 事件。

前端收到实时更新后必须短延迟刷新 session 列表。

前端 session 列表刷新进行中收到新的实时更新时，必须在当前刷新结束后补一次刷新。

连续实时更新不得让前端列表刷新无限后延。

所有 coding agent session runtime 在 APP 进程启动时为空。

Codex CLI session 只能由 APP 进程启动后的实时 hook 写入。

Codex CLI session 也可由已知 session rollout 追加行写入实时活动或完成事件。

Codex APP session 可由 APP 进程启动后的实时 hook、notification、server request、session index、app-server `thread/list` 元数据或 Codex rollout 历史补齐。

Codex APP session 也可由已知 session rollout 追加行写入实时活动或完成事件。

除 Codex APP 的项目名、跳回目标和最新输出补齐外，其它 coding agent adapter 不得通过历史文件、transcript、JSONL、rollout、已加载 thread id 列表或其它持久化记录恢复 session。

APP 启动后仍在运行的任务如果继续发出实时事件，可以创建当前进程内 session。

前端合并 Codex CLI 和 Codex APP session 后，UI session 选中身份必须包含 runtime source。

前端合并 Codex CLI 和 Codex APP session 后，必须按首次捕捉顺序保持稳定。

刷新时新捕捉到的 session 插入顶部，已捕捉 session 不因状态或更新时间变化重排。

阶段 7 前端设置状态不进入 Domain。

设置模型由 Settings Service 显式建模。

Panel 设置保存窗口位置和窗口尺寸。

Panel 设置中的收缩字段不得保存为 `true`。

设置存储脏数据在 config adapter 和前端 fallback 边界完成校验或降级。

设置缺失字段在反序列化边界补默认值。

设置未知字段在反序列化边界丢弃，不进入设置模型。

自定义快捷输入脏数据在设置边界清洗，不进入 UI 核心流程。

配置损坏时，核心 UI 使用默认设置。

设置页不包含自动更新配置项。

设置页 hook 安装入口只在用户打开设置页、点击安装或点击卸载时调用 hook 安装 command。

设置页 hook 安装状态必须来自 hook 安装状态 command。

前端不得用本轮 UI 记忆推断 hook 是否已经安装。

设置保存响应必须通过请求版本校验后才能覆盖前端设置状态。

面板置顶偏好必须在设置保存响应通过请求版本校验后才应用到当前窗口。

设置保存失败时，前端不得应用本次面板置顶偏好变更。

panel 窗口移动和尺寸变化通过局部保存 command 更新 Panel 设置，不覆盖其它设置分组。

panel 窗口状态局部保存不得把收缩字段写成 `true`。

当前 Codex CLI 和 Codex APP 开关驱动 session 读取。

未接入 session 读取链路的 agent 开关必须在 UI 禁用。

通知计划不进入 `SessionState`。

通知合并状态只存在于 Notification Service 进程内状态。

Codex CLI bridge 启动只有 bind 成功后才记录为已启动。

Codex CLI bridge 首次 bind 失败后允许后续 command 重试启动。

Codex CLI 审批决策唤醒等待器前，必须校验当前 session pending interaction 仍匹配同一个 `InteractionId`。

Codex CLI 允许并记住决策当前只唤醒为 allow directive。

Codex CLI 同一 session 收到新的 `PermissionRequest` 时，旧审批等待器必须过期，避免多个可决策审批同时悬挂。

Codex APP schema 探针会执行本机 `codex app-server generate-json-schema --experimental`，属于 adapter 边界副作用。

Codex APP app-server stdio 客户端会启动 `codex app-server --listen stdio://`，属于 adapter 边界副作用。

Codex APP app-server stdout 按行解析，单行超过上限时丢弃该行。

Codex APP app-server request 写入前必须登记 pending response，避免快速 response 丢失。

Codex APP app-server request 等待必须有超时。

Codex APP app-server request 或 response 写入不得在等待期间持有全局 app-server client slot 锁。

Codex APP app-server 启动不得在等待期间持有全局 app-server client slot 锁；slot 只表达空、启动中或已连接。

Codex APP follow-up 写入前必须确保目标 thread 已加载；未加载 thread 必须先通过 app-server `thread/resume` 恢复。

Codex APP follow-up 从 runtime 登记提交占位后，到成功写入用户输入原文前，任一失败路径都必须释放提交占位。

Codex APP app-server response 必须保留 request 的原始 JSON-RPC `id` 类型。

Codex APP app-server stdout 中带 `method` 的消息必须按 server request 或 notification 处理，不能仅因 id 命中 pending request 就当作 response。

Codex APP 未识别或畸形 app-server server request 必须回写 JSON-RPC error；若消息包含 thread ID，runtime 必须写入失败状态。

Codex APP legacy approval 与新版 item approval 的 response enum 不相同，必须按 server request method 分流编码。

Codex APP follow-up 写入成功后只能写入用户输入原文事件，不写提交成功包装文案。

Codex APP follow-up 必须同时满足无 pending interaction 且 session 状态为 `Completed` 或 `Failed`。

Codex APP follow-up request id 必须由 app-server client 内部递增分配。

Codex APP app-server 子进程在启动初始化或 client drop 失败路径中必须尝试 kill 和 wait 回收。

Codex APP app-server 启动失败必须设置退避，避免前端轮询造成持续 spawn。

Codex APP hook runtime 读取和 hook approval 决策不得依赖 app-server 已连接。

Codex APP session 列表和详情查询返回当前 runtime 状态，不等待 app-server `thread/list` 或 Codex rollout 历史补齐完成。

Codex APP app-server `thread/loaded/list` 与 `thread/list` 读取必须节流，并使用短超时失败降级，避免前端轮询阻断其它 agent session 刷新。

Codex APP app-server 无 cwd 事件先创建的 session，必须在后续真实 cwd 到达时按 thread ID 合并到同一 session。

Codex APP app-server client 被复用前必须检查子进程仍存活；已退出时必须清理 client slot。

前端读取 Codex CLI 和 Codex APP session 时，单一来源失败不得阻断其它来源 session 刷新。

前端工具用量聚合只读取真实 session 的账号窗口用量。

前端工具用量聚合同一工具同一来源键只保留最新值。

前端工具用量聚合不得按 session 求和。

终端跳回 adapter 当前可记录跳回请求、打开系统 URL 和返回失败降级，不执行真实终端控制。

JSON 设置文件读写属于 config adapter 边界副作用。

JSON 设置文件保存使用同目录临时文件写入和目标文件替换。

JSON 设置文件每次保存生成唯一临时文件路径。

临时文件写入失败不得覆盖旧配置。

hook 安装和卸载属于 hook install adapter 边界副作用。

设置页 hook 安装 command 是 hook install adapter 的 Tauri 边界入口。

hook 安装器必须能生成修改文件、备份文件和 manifest 的预览。

hook 安装器必须能查询 Codex CLI hook 和 Claude CLI hook 的安装状态。

hook 安装状态必须同时检查 manifest 和实际配置。

Codex CLI hook 已安装状态必须检查 `hooks.json` handler 和 `config.toml` 的 `[features].hooks`。

严格已安装状态必须禁用重复安装。

需要修复状态必须允许安装重写受管 handler。

没有目标 manifest 记录时必须禁用或跳过重复卸载。

hook 安装不得静默提权，不得绕过 Codex hook trust review。

hook 安装器只移除 Builder Panel 自己写入的旧 handler，不删除用户其他 hook handler。

hook 安装器必须在写文件前对重复 agent 输入去重。

hook 安装器必须先完成所有目标配置读取和构造，再开始写入文件。

hook 安装器写配置文件和 manifest 时使用临时文件替换目标文件。

hook 安装中途失败时，已写配置、旧备份和旧 manifest 必须回滚到安装前状态。

单项安装必须保留 manifest 中其它 agent 的记录。

hook 卸载必须以 manifest 为恢复依据。

单项卸载必须保留 manifest 中其它 agent 的记录。

单项卸载写回剩余 manifest 失败时，必须回滚目标配置和旧 manifest。

最后一个 agent 卸载成功后必须删除 manifest，避免陈旧恢复记录再次生效。

Tauri 窗口位置和尺寸读取、监听与恢复属于前端 Tauri API 边界副作用。

浏览器开发环境不得执行 Tauri 窗口位置和尺寸恢复。

日志脱敏属于 adapter 边界纯转换。

记录型通知 adapter 只记录通知计划，不调用真实系统通知 API。

## 边界破坏风险

若 Domain 引入 Tauri 或文件系统依赖，后续 reducer 和规则测试会被基础设施污染。

若前端绕过 `api` 层直接访问 adapter，UI 会承担协议清洗职责。

若 hook helper 阶段 0 输出阻塞 directive，可能影响 agent 正常运行。

若 hook helper 在 bridge 不可用时非 0 退出，可能影响 agent 正常运行。

若 hook helper 未校验 payload 就发送 bridge command，第三方脏数据会污染内部协议。

若前端绕过 view model 猜测 interaction ID，pending 已变化时可能提交到错误交互。

若回写失败时清理 pending，用户将失去可重试入口。

若选项失败后清理已选状态，用户将失去重试上下文。

若快捷回复绕过 Reply Service，可能在 pending 已变化后提交到错误交互。

若自定义快捷输入绕过当前 session 能力校验，可能向不可回复或不可 follow-up 的 session 提交文本。

若跳回能力被当成回写能力，UI 可能展示无法可靠完成的发送入口。

若跳回动作在没有跳回目标时生成，用户点击 session 会得到无意义错误而不是无反应。

若 Codex APP adapter 直接透传 app-server 原始 JSON 到 Domain，Domain 纯粹性会被外部协议污染。

若 Codex CLI bridge server 在等待审批时不校验 interaction，旧 UI 决策可能返回给错误 hook request。

若 Codex CLI bridge listener 被单个审批等待阻塞，其他 hook 事件会超时或 fail-open。

若前端只在首屏读取一次 Codex CLI session，后续真实审批请求会不可见并最终超时 fail-open。

若前端按 agent kind 而不是 runtime source 路由，Codex CLI 与 Codex APP 可能互相误走 command。

若 Codex CLI runtime 接收非 Codex CLI 事件，Codex APP 任务可能被错放到 CLI session 列表并显示错误的来源徽章。

若 Codex APP 收录新 thread 时不通知 Codex CLI runtime，hook 早到造成的 Codex APP 孤儿 session 无法被回收，前端会同时看到 Codex CLI 与 Codex APP 两条同 thread session。

若 hook bridge 分流阶段的同步刷新没有总时长上限，长时间未响应的 app-server 会阻塞所有 Codex hook 路径。

若前端选中态不包含 runtime source，同一个 `SessionKey` 的 Codex CLI 和 Codex APP session 可能互相误选。

若后端和前端未共同保持捕捉顺序，刷新后 session 可能因 runtime 拼接、状态或更新时间变化发生跳位。

若设置 fallback 不校验本地缓存结构，脏数据可能让 UI 展示与能力不一致。

若配置保存直接覆盖目标文件，写入失败可能破坏用户旧配置。

若同进程并发保存共享临时文件，失败保存可能污染成功保存结果。

若 hook 安装不备份第三方配置，卸载无法恢复安装前状态。

若 hook 安装中途失败不回滚，会形成没有 manifest 的半安装状态。

若 hook 卸载成功后保留 manifest，后续误触发卸载可能覆盖用户新配置。

若 hook 安装绕过 Codex trust review，会破坏 Codex 自身安全边界。

若 Codex CLI 审批决策不校验当前 pending interaction，旧 UI 决策可能唤醒过期 hook request。

## 代码入口

`scripts/check-architecture.mjs` 是边界守护入口。

`scripts/check-spec-docs.mjs` 是 spec 文档质量门禁入口。

`scripts/check-performance-budget.mjs` 是性能预算静态场景入口。

`src-tauri/src/domain/panel_probe.rs` 是 panel 探针状态入口。

`src-tauri/src/domain/panel_geometry.rs` 是纯窗口规则入口。

`src-tauri/src/domain/session_state.rs` 是 reducer 入口。

`src-tauri/src/domain/view_model.rs` 是 view model 转换入口。

`src-tauri/src/adapters/config_file/mod.rs` 是 JSON 设置文件原子读写入口。

`src-tauri/src/adapters/hook_install/mod.rs` 是 hook 安装和卸载入口。

`src-tauri/src/adapters/log_sanitizer/mod.rs` 是日志脱敏入口。

`src-tauri/src/adapters/bridge/hook_payload.rs` 是第三方 hook payload 清洗入口。

`src-tauri/src/adapters/bridge/codec.rs` 是显式 bridge 契约入口。

`src-tauri/src/adapters/mock_agent/mod.rs` 是 mock 测试基线 runtime 入口。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 是 Codex CLI runtime 和审批等待入口。

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP app-server adapter 入口。

`src-tauri/src/services/interaction_service.rs` 是审批 pending 校验入口。

`src-tauri/src/services/reply_service.rs` 是文本回复 pending 校验和长度校验入口。

`src-tauri/src/services/shortcut_reply_service.rs` 是快捷回复过滤入口。

`src-tauri/src/services/preset_command_service.rs` 是预设命令计划入口。

`src-tauri/src/adapters/terminal/mod.rs` 是跳回降级测试入口。

`src-tauri/src/services/settings_service.rs` 是设置模型和默认化入口。

`src-tauri/src/adapters/config_file/mod.rs` 是设置文件副作用入口。

`src-tauri/src/services/notification_service.rs` 是通知合并和点击动作入口。

`src-tauri/src/adapters/notification/mod.rs` 是记录型通知 adapter 入口。

`src/components/SettingsPanel.tsx` 是设置 UI 入口。

`src/api/sessionJumpApi.ts` 是前端跳回边界入口。

## 相关测试

`pnpm architecture:check` 验证边界不变量。

`cargo test --manifest-path src-tauri/Cargo.toml` 验证 Rust 纯规则。

`cargo test --manifest-path src-tauri/Cargo.toml bridge` 验证 bridge 和 hook helper 边界。

`src-tauri/src/services/interaction_service.rs` 验证审批 pending 校验和失败保留。

`src-tauri/src/services/reply_service.rs` 验证回复校验和失败保留。

`src-tauri/src/services/shortcut_reply_service.rs` 验证快捷回复过滤和排序。

`src-tauri/src/services/preset_command_service.rs` 验证预设命令计划生成。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 验证 Codex CLI runtime pending approval 不变量。

`src-tauri/src/adapters/codex_app/mod.rs` 验证 Codex APP notification 清洗边界。

`src/views/BuilderPanelApp.test.ts` 验证阶段 7 合并捕捉顺序、统计、动作标签和工具用量聚合。

`src-tauri/src/services/settings_service.rs` 验证配置缺失、损坏和保存。

`src/api/settingsApi.test.ts` 验证自定义快捷输入清洗。

`src-tauri/src/services/notification_service.rs` 验证通知抑制、合并和点击定位。
