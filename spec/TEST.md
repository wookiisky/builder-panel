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

阶段 7 测试扩展模式 session 捕捉顺序、统计、工具用量聚合、设置默认值、设置文件读写、通知合并、行内交互、等待回复展开和自定义快捷输入。

阶段 7 当前未建立 Playwright 自动化截图验证。

阶段 7 当前不执行 Windows 本机人工验证。

阶段 8 测试配置缺字段默认化、原子写失败不覆盖旧配置、hook 状态查询、hook 安装卸载 fixture、日志脱敏、spec 文档门禁和性能预算静态场景。

阶段 8 测试设置页 hook 状态列表、panel 默认持久化状态和收缩状态归一化。

阶段 8 当前不执行 Windows 本机人工验证。

阶段 8 当前不声明 10 分钟空闲 CPU 人工采样已完成。

阶段 0 不测试未实现的持久化恢复。

## 测试分层

Rust 单元测试验证 Domain 纯规则。

Rust Domain 测试验证已完成 session 过期 key 纯计算规则，包括严格超过窗口、刚好到达窗口保留、未来完成时间保留和非完成状态保留。

前端单元测试验证 UI store 的纯状态转换。

前端单元测试验证合并 session 的展示分组、首次观察序、父子展示块锚点和 store 隔离。

前端单元测试验证 panel 目标高度以 Tauri 真实窗口高度为比较基准，并验证 resize 串行、最新请求合并、失败后重试和释放行为。

前端单元测试验证 panel 独立自然内容层不使用当前 WebView 视口高度，打开的 overlay 面板参与高度测量并触发尺寸观察，动态子级 overlay 挂载、卸载会触发重算，且长内容滚动约束收敛在内容区。

前端单元测试验证 Tauri 物理窗口和当前显示器工作区按 scale factor 转换到统一逻辑坐标，包括多屏负坐标和显示器缺失降级。

架构脚本验证跨层依赖边界。

Rust adapter 测试验证 bridge 和 hook helper 边界行为。

Rust mock adapter 测试验证 mock event、directive 记录和回写失败保留 pending。

Rust Codex CLI adapter 测试验证 hook payload 到归一事件、非阻塞 ack、pending approval 和 directive 等待。

Rust Codex CLI runtime 测试验证审批等待超时会清理 pending approval 并拒绝迟到决策。

Rust Codex CLI runtime 测试验证迟到 UI 决策早于 bridge 超时清理时仍会清理 session pending。

Rust Codex CLI runtime 测试验证同一 session 新审批会让旧审批等待器过期。

Rust Codex CLI runtime 测试验证超过保留窗口的已完成 session 会被清理，并同步移除 rollout watch target、发布 session 更新通知。

Rust Codex CLI adapter 测试验证 hook payload 的模型字段不作为 thread 标题展示。

Rust Codex APP adapter 测试验证 app-server schema 探针、request 编码、notification 到归一事件转换、Codex APP hook 分流和完整能力 capability。

Rust Codex APP adapter 测试验证 hook cwd 与 app-server thread 事件会折叠到同一 session。

Rust Codex APP adapter 测试验证首个 app-server 实时审批或回复 request 可初始化可操作 session。

Rust Codex APP adapter 测试验证 app-server 实时消息空白 cwd 会进入待识别 session，且不生成跳回目标。

Rust Codex APP adapter 测试验证无可信 cwd 的 app-server 实时事件不会使用 Builder Panel cwd，且不会生成跳回动作。

Rust Codex APP adapter 测试验证 app-server thread 元数据可迁移待识别 session，且不覆盖 pending、summary 或状态。

Rust Codex APP adapter 测试验证 parent-only thread metadata 只记录父子关系不创建 session，child 和 parent session 都存在后会发布层级更新。

Rust Codex APP adapter 测试验证显式 `thread_spawn` 内嵌 parent 字段可建立 child 到 parent 的层级关系，并在列表 view model 暴露一级缩进。

Rust Codex APP adapter 测试验证清理 parent 空壳 session 会同步清理 child 层级关系。

Rust Codex APP runtime 测试验证超过保留窗口的已完成 session 会被清理，并同步移除 thread 缓存、rollout watch target、当前输出、follow-up 占位，且 parent 被清理时 child 回退为顶层并发布更新通知。

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

Rust Codex APP rollout 测试验证 `session_meta` 可清洗顶层 parent、嵌套 sub agent `thread_spawn` parent 和受非内部机制 sub agent 来源保护的 `session_id` 父级 fallback，且普通主 session 或内部机制 sub agent 不从 `session_id` 误解析父级。

Rust Codex APP rollout 测试验证完成事件会标记快照完成，完成后新的用户输入会重置为未完成。

Rust Codex APP rollout 测试验证范围外 path 被忽略，超长 JSONL 行被跳过且后续有效行仍可读取。

Rust Codex APP rollout 测试验证 tailer 只读取已知 session 的新增追加行，并验证用户输入事件、外部只读等待输入事件、等待输入问题摘要更新、已扫描等待输入的完成清理、工具 preview 不写最后消息、重复工具事件不生成摘要、未知 JSON arguments 不展示、工具结束不写摘要、超长追加行后继续读取有效行。

Rust Codex APP adapter 测试验证孤立 rollout 快照不会单独创建当前 session。

Rust Codex APP adapter 测试验证 recent active rollout 可从空 runtime 创建运行中 session，并验证完成、空内容或内部提示词快照不创建 session。

Rust Codex APP adapter 测试验证 recent active rollout 可在 parent 先到或 child 先到时建立 child 到 parent 的层级关系，并在列表 view model 暴露一级缩进。

Rust Codex APP adapter 测试验证 rollout 快照只带来层级关系变化时也会发布 child session 更新通知。

Rust Codex APP adapter 测试验证内部机制来源的 recent active rollout 不创建可见 session。

Rust Codex APP adapter 测试验证 recent active rollout 创建 session 时触发 Codex CLI 孤儿清理回调。

Rust Codex APP adapter 测试验证后台 `notLoaded` metadata 不把 recent active rollout 创建的运行中 session 降级为失联。

Rust Tauri command 测试验证 rollout recent scan 候选集合只包含已加载、历史返回、当前待识别 thread 和当前已知但缺标题的 thread。

Rust Tauri command 测试验证 recent active rollout 活跃窗口默认 5 分钟、支持正整数分钟配置，并过滤完成、过期或未来时间快照。

Rust Tauri command 测试验证已完成 session 保留窗口默认 20 分钟，且缺失、空白、非法或 0 配置会回退默认值，正整数分钟配置会生效。

Rust Tauri command 测试验证 `thread/read` 方法不可用时会触发 `thread/list` 降级判定，普通详情清洗错误不触发该降级。

Rust Tauri command 测试验证 thread 历史元数据只应用到当前待识别 thread 或当前已知但缺标题的 thread，thread path 快照必须匹配 thread ID 和候选集合。

Rust Codex APP adapter 测试验证无 cwd app-server 实时事件后续可随 hook 真实 cwd 迁移，且不产生重复 session。

Rust Codex APP adapter 测试验证 requestUserInput 与 MCP elicitation 回复编码。

Rust Codex APP adapter 测试验证 requestUserInput 选项在 `value` 缺失、`null` 或空白时可用非空 `label` 作为提交值，并验证非字符串 `value` 仍作为字段错误处理。

Rust Codex APP adapter 测试验证 thread 列表单条无效 thread 不会丢弃同批有效 thread。

Rust Codex APP adapter 测试验证缺 cwd 或空白 cwd 但带 path 的 thread 可保留为 rollout 候选，空白 path 不会成为候选，且 `status.type` 类型错误的 thread 会被跳过。

Rust Codex APP adapter 测试验证 rollout 快照可迁移待识别 session 到真实 cwd。

Rust Codex APP adapter 测试验证 rollout 快照可用最近 Agent 输出刷新运行中 session 摘要，且不会用仅来自用户输入的摘要覆盖当前 Agent 摘要。

Rust Codex APP adapter 测试验证 rollout 快照恢复出的等待输入会进入等待回复状态、发布 session 更新，并使用只读回复目标。

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

前端 Builder Panel 测试验证合并后的 session 未完成分组置顶、已结束分组下沉、首次观察序、统计数量和动作标签。

前端 Builder Panel 测试验证已有 session 摘要刷新后不改变首次观察序。

前端 Builder Panel 测试验证后端刷新顺序可保持 parent-child 相邻，child session 按一级缩进展示，且 parent-child 展示块按未完成锚点排序。

前端 Builder Panel 测试验证列表刷新调度器在刷新中收到实时事件时会补刷，且连续事件不会无限后延。

前端 Builder Panel 测试验证同一 session 摘要刷新后列表行展示新摘要。

前端 Builder Panel 测试验证 session 状态 icon 使用开源 SVG 图标资源，全部状态保留可访问语义并位于行首独立列。

前端 Builder Panel 测试验证等待回复和等待审批状态使用不同 SVG 图标与状态色。

前端 Builder Panel 测试验证标题栏窗口操作图标按钮保留无文本和可访问语义。

前端 Builder Panel 测试验证来源标签和项目名同排展示，运行中停止占位位使用开源 octagon-x SVG 图标资源、不通过 CSS 填充绘制基础形状、不展示红底按钮样式并保持禁用语义。

前端 Builder Panel 测试验证项目名和 thread 名只在单行视觉截断时启用 tooltip。

前端 Builder Panel 测试验证桌面 session 主行允许 thread 列收缩，并由 thread 文本截断保护摘要列。

前端 Builder Panel 测试验证通用 760 逻辑宽度响应式规则不覆盖 session 主行四列布局；独立的 560 逻辑宽度窄屏边界保持状态、内容、时间操作三列，并把来源、项目名、thread 名和摘要分别放在内容列的第一、第二行。

前端 Builder Panel 测试验证当前输出文本完整展示时不启用 tooltip，发生两行视觉截断时启用 tooltip。

前端 Builder Panel 测试验证当前输出文本 tooltip 非链接区域双击会触发对应 session 跳回入口。

前端 Builder Panel 测试验证当前输出文本 tooltip 内 Markdown 链接区域双击不触发 session 跳回且不取消链接默认行为。

前端 Builder Panel 测试验证 action summary tooltip 双击不触发 session 跳回。

前端 Builder Panel 测试验证 session 行点击只有在存在 jump action 且跳回设置开启时才触发跳回。

前端 Builder Panel 测试验证具备 jump action 的 session 在全局跳回关闭时仍可被点击选中。

前端 Builder Panel 测试验证完成和失败状态在可 follow-up 时默认单行，展开后按输入区、发送 icon 和快捷输入的顺序展示。

前端 Builder Panel 测试验证可回写等待回复状态通过 hover 或 focus 展开第二行，展示文本回复或选项，不展示快捷输入。

前端 Builder Panel 测试验证等待回复选项可展示真实 Codex APP 选项文本，保留 tooltip，详情面板等待回复不展示快捷输入。

前端 Builder Panel 测试验证外部只读 Codex APP 等待回复只展示主行问题摘要，不展示第二行、选项或提交按钮。

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

前端 Settings Panel 测试验证 Panel 分组可更新窗口最大高度。

前端 Builder Panel 测试验证窗口移动和逻辑宽度变化的局部保存更新会合并。

前端 Builder Panel 测试验证 panel 内容签名、重复宽度保存过滤、长内容滚动约束和标题栏拖动区标记。

前端 panel 自适应规则测试验证配置最大高度、真实高度比较、当前显示器工作区限制、多屏负坐标和显示器缺失降级。

前端 panel resize controller 测试验证 resize 副作用串行、进行中请求合并、失败后可重试和释放后不开始新副作用。

前端 panel 内容测量测试验证独立自然内容层、打开 overlay 后的高度候选、ResizeObserver 目标，以及动态 DOM 挂载和卸载的 MutationObserver 触发。

前端 panel window API 测试验证窗口逻辑宽度恢复、物理几何统一转换、内容高度调整保留真实宽度和浏览器环境 no-op。

前端 Tauri runtime 测试验证运行时判断入口跟随官方 Tauri 环境标记。

前端设置测试和 Rust Settings Service 测试验证 Panel 最大窗口高度归一化。

前端设置测试和 Rust Settings Service 测试验证旧版物理像素窗口尺寸会被清空且不迁移。

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

Codex APP adapter 测试断言 metadata、最近活跃 rollout、rollout tail 和 app-server delta 不会把无真实用户上下文的已知内部结构化产物创建为可见 session。

Codex APP adapter 测试断言内部结构化 delta 后的完成、标题和状态事件不会保留空壳 session，也不会把已暂缓的内部输出缓存重新写回摘要。

Codex APP adapter 测试断言内部结构化 rollout snapshot 或最近活跃 rollout 会清理此前 `thread/started` 留下的空壳 session。

Codex APP adapter 测试断言真实用户上下文中的同形 JSON 输出仍可作为 session 摘要展示。

Interaction Service 测试断言 allow、deny 和回写失败路径。

Interaction Service 测试断言 allow and remember、单选、多选空选择、非法选项和回写失败路径。

Reply Service 测试断言非空、空内容、超长和回写失败路径。

Shortcut Reply Service 测试断言启用状态、agent 绑定、项目绑定和排序。

Preset Command Service 测试断言结构化创建优先、托管进程降级和复制降级。

Mock panel store 测试断言虚拟列表不会按一万条记录全量计算可见范围。

Builder Panel 测试断言主界面不依赖收缩状态。

Builder Panel 测试断言合并后的展示分组、首次观察序和 parent-child 展示块排序稳定。

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

`src-tauri/src/domain/session_state.rs` 是 reducer、pending 清理、多会话隔离、展示分组和块级排序测试入口。

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

`src/views/BuilderPanelApp.test.ts` 是阶段 7 session 展示分组、首次观察序、统计、动作标签、工具用量聚合、等待回复展开和 follow-up 输入顺序测试入口。

`src/api/settingsApi.test.ts` 是前端设置默认值和自定义快捷输入清洗测试入口。

`src/api/panelWindowApi.test.ts` 覆盖前端 panel 窗口置顶偏好应用、逻辑宽度恢复、内容尺寸调整、局部保存和关闭窗口入口。

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
