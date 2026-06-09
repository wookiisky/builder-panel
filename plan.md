# Builder Panel 分阶段开发方案

## 1. 总体策略

Builder Panel 按“先核心模型、再本地链路、再真实 agent、最后完整体验”的顺序推进。每个阶段都必须具备可验证交付物，不允许只完成代码但无法证明行为正确。

开发原则：

1. 先写测试或验证样例，再实现功能。
2. 先打通 mock agent 闭环，再接入真实 Codex 和 Claude Code。
3. 先实现可靠能力，再实现降级能力。
4. 先保证 Domain 纯粹和边界正确，再扩展 adapter。
5. 每个阶段完成后同步更新相关文档。
6. 不把未验证的 agent 私有协议当成已支持能力。
7. 不展示 UI 假按钮，所有动作必须来自显式 capability。
8. 文档系统按 `SPEC_DOC.md` 维护，`spec/00_INDEX.md` 是唯一事实导航入口。

完成定义：

1. 自动化测试通过。
2. 人工验证步骤可复现。
3. 相关文档更新。
4. 降级路径可验证。
5. 不新增跨层依赖。
6. `spec/` 中受影响事实文档已同步。

## 2. 当前实现状态总览

本节按 `REQ.md`、`spec/` 事实文档和当前代码入口梳理已实现、部分实现和未实现内容。

### 2.1 已实现或已建立验证基线

1. 工程骨架、Tauri 入口、React 前端入口、Rust 分层目录和架构边界检查已建立。
2. Domain 核心类型、归一事件、纯 reducer、排序规则、用量模型、错误模型和 view model 纯转换已建立。
3. 本地 bridge NDJSON codec、Mac Unix Domain Socket 传输、Windows Named Pipe 传输代码和 hook helper fail-open 链路已建立。
5. Codex CLI hook payload 到 session 状态的真实闭环已建立，`PermissionRequest` 可经 panel 返回 allow 或 deny directive。
7. Codex APP 已建立 app-server schema 探针、request 编码和 notification 转换。
8. 审批、选项、文本回复、快捷回复过滤、预设命令计划、跳回端口和复制降级模型已建立。
10. 扩展模式工作台、session 合并排序、统计、动作隐藏、长文本布局约束、设置页和设置保存冲突处理已建立。
11. 通知计划服务、重复通知合并、当前 session 抑制、点击定位和记录型通知 adapter 已建立。
12. JSON 设置文件默认化、缺字段补齐、未知字段丢弃、临时文件原子写入和损坏降级已建立。
13. hook 安装预览、备份、manifest、安装、卸载和失败回滚基础设施已建立。
14. 日志脱敏、spec 文档门禁和性能预算静态场景脚本已建立。
15. 设置页 hook 安装入口已建立，支持 Codex CLI 和 Claude CLI 目标选择、安装预览、显式安装和卸载。
16. hook 安装入口已要求先生成当前选择对应的预览，再允许写入第三方配置。
17. panel 收缩状态、窗口位置和窗口尺寸的设置持久化入口已建立。
18. panel 窗口移动和尺寸变化的局部保存已合并处理，避免同一 debounce 窗口内丢失位置或尺寸字段。

### 2.2 部分实现或只完成自动化验证

1. Windows Named Pipe 有代码和平台无关测试入口，但未在 Windows 本机完成人工验收。
3. Claude Code CLI 和 Claude Code APP 当前只有阶段 2 hook payload 基础校验、stdout directive 编码边界和 adapter 占位模块，不声明真实 session 闭环完成。
4. 终端跳回和新对话创建当前只有端口、计划模型和复制降级，不声明真实 tmux、Ghostty、PowerShell 或 cmd 人工闭环完成。
5. 通知当前只实现通知计划服务和记录型 adapter，不声明真实 Mac 或 Windows 系统通知已接入。
6. 性能预算当前只建立静态场景脚本，不替代 10 分钟空闲 CPU、真实内存和真实滚动性能人工采样。
7. panel 收缩状态、窗口位置和窗口尺寸已有保存和恢复入口，但 Mac 重启恢复、拖动和焦点仍未完成原生人工验收。
8. 设置页 hook 安装入口已有自动化和 fixture 验证，但真实 Codex 和 Claude 配置安装、卸载、trust review 尚未人工验收。
9. 前端当前未建立 Playwright 自动化截图验证基础。

### 2.3 未实现或未验收

1. Claude Code APP 真实发现、进程级状态、跳回和有限回写能力尚未完成。
2. Claude Code CLI `SessionStart`、`UserPromptSubmit`、`PreToolUse`、`PermissionRequest`、`Notification`、`Stop` 和 `SessionEnd` 到真实 session 状态的 adapter 闭环尚未完成。
4. 开机启动尚未完成。
5. 真实 Mac 和 Windows 系统通知 adapter 尚未接入。
6. Mac 全量发布验收矩阵尚未完成。
7. Windows 本机验证按当前执行口径跳过，Windows 原生窗口、Named Pipe、系统通知和真实 agent 流程仍未验收。
8. 10 分钟空闲 CPU、真实内存和真实滚动性能人工采样尚未完成。

## 3. 阶段 0：工程骨架与架构边界

目标：建立 Tauri + Rust + React 工程骨架，确定目录分层、测试入口、基础窗口和架构约束。

### 3.1 任务：初始化工程骨架

交付物：

1. Tauri + React + TypeScript + Rust 项目结构。
2. `src-tauri/src/domain/`、`ports/`、`adapters/`、`services/`、`tauri_api/` 目录。
3. 前端 `src/` 下按 `components/`、`stores/`、`views/`、`api/` 拆分。
4. 基础 `package.json`、`Cargo.toml`、lint、format、test 命令。

验证标准：

1. Mac 和 Windows 均可启动空 panel。
2. Rust 测试、前端测试、lint 命令可运行。
3. 目录边界与 `tech.md` 一致。
4. Domain 目录中不出现 Tauri、React、文件系统或系统 API 依赖。

验证方法：

1. 运行 `cargo test`。
2. 运行 `npm test` 或 `pnpm test`。
3. 运行 `npm run lint` 或 `pnpm lint`。
4. 运行 `npm run tauri dev` 或对应 dev 命令，确认空 panel 可启动。
5. 使用 `rg "tauri|std::fs|tokio|serde_json::Value" src-tauri/src/domain` 检查 Domain 污染。

### 3.2 任务：建立架构守护测试

交付物：

1. Rust 侧架构边界测试。
2. 前端 import 边界检查。
3. CI 中加入架构检查步骤。

验证标准：

1. Domain 不依赖 adapter、service、Tauri。
2. UI 不直接引用 adapter。
3. adapter 不反向依赖 UI。
4. CI 能在边界破坏时失败。

验证方法：

1. 使用静态脚本扫描 Rust module import。
2. 使用 ESLint import rule 或自定义脚本检查前端 import。
3. 人为构造一次非法 import，确认检查失败后还原。

### 3.3 任务：基础窗口能力探针

交付物：

1. 可置顶、可拖动的基础 panel。
2. 收缩和展开状态字段。
3. 多显示器位置修正函数。
4. 焦点行为验证记录。

验证标准：

1. panel 可置顶。
2. panel 可拖动。
3. 重启后位置可恢复。
4. 恢复位置不会落到屏幕外。
5. 默认不抢焦点，点击输入框时可获得焦点。

验证方法：

1. Mac 手工拖动 panel，重启 APP 验证位置。
2. Windows 手工拖动 panel，重启 APP 验证位置。
3. 模拟显示器尺寸变化，验证位置修正函数单元测试。
4. 使用 Playwright 或人工记录焦点行为。

### 3.4 任务：建立 spec 文档系统骨架

交付物：

1. `spec/00_INDEX.md`。
2. `spec/SYSTEM_OVERVIEW.md`。
3. `spec/SYSTEM_FLOWS.md`。
4. `spec/EXTERNAL_BEHAVIOR.md`。
5. `spec/INTERNAL_BEHAVIOR.md`。
6. `spec/ERROR_HANDLING.md`。
7. `spec/DECISION_LOG.md`。
8. `spec/TEST.md`。
9. `spec/SERVICE/`。
10. `spec/DOMAIN/`。
11. `spec/API/`。
12. `spec/INFRA/`。
13. `spec/INTEGRATIONS/`。
14. `spec/TOOLS/`。
15. `spec/WORKERS/`。

说明：

1. 首版没有数据库主事实，先不创建 `spec/STORE/`。
2. 后续出现持久化主事实时，再创建 `spec/STORE/` 并登记索引。
3. `spec/` 文档只记录稳定事实，不替代代码、需求文档或用户手册。

验证标准：

1. `spec/00_INDEX.md` 是唯一导航入口。
2. 索引登记所有实际存在的 spec 文档。
3. 每篇文档先说明职责和边界。
4. 每篇文档预留代码事实入口和测试入口位置。
5. 文档不使用流程图、表格、emoji 和实现代码片段。
6. 文档不复制完整字段清单。
7. 文档不形成连续引用链。

验证方法：

1. 运行 `find spec -maxdepth 3 -type f | sort`，确认文档结构。
2. 人工检查 `spec/00_INDEX.md` 是否覆盖全部实际文档。
3. 运行 `rg "mermaid|```|\\|" spec`，检查流程图、代码块和表格痕迹。
4. 抽查每篇文档是否只引用直接相关文档和代码事实入口。
5. 对照 `SPEC_DOC.md` 的质量门禁逐项检查。

## 4. 阶段 1：Domain 与纯 Reducer

目标：完成纯 Domain 模型和 reducer，为后续 bridge、adapter、UI 提供稳定契约。

### 4.1 任务：定义核心类型

交付物：

1. `AgentKind`
2. `ProjectId`
3. `ConversationId`
4. `SessionKey`
5. `SessionStatus`
6. `SessionCapabilities`
7. `UsageSnapshot`
8. `UsageValue`
9. `AgentInteraction`
10. `ReplyTarget`
11. `AppError`

验证标准：

1. 四类 agent 入口被显式建模。
2. session 唯一键由 `agent_kind`、`project_id`、`conversation_id` 共同确定。
3. 用量不可用使用专门状态，不使用魔法数字。
5. 错误类型覆盖 `tech.md` 中列出的错误码。

验证方法：

1. Rust 单元测试校验 `SessionKey` 等值和排序。
2. Rust 单元测试校验 `UsageValue::Unavailable` 不生成展示数字。
3. Rust 单元测试校验 capability 到 action 的映射。
4. 类型检查确保无裸字符串状态。

### 4.2 任务：定义 AgentEvent

交付物：

1. `AgentEvent` enum。
2. `SessionStartedEvent`
3. `ActivityUpdatedEvent`
4. `ApprovalRequestedEvent`
5. `AnswerRequestedEvent`
6. `TurnCompletedEvent`
7. `FailedEvent`
8. `DetachedEvent`
9. `CapabilitiesUpdatedEvent`
10. `UsageUpdatedEvent`
11. `JumpTargetUpdatedEvent`

验证标准：

1. 每个事件必须携带 `SessionKey`。
2. 事件 schema 不复用 `open-vibe-island` Swift 字段结构。
3. 第三方 payload 不进入事件。
4. 事件可序列化和反序列化。

验证方法：

1. Rust snapshot 测试校验事件 JSON。
2. 构造 Codex 和 Claude mock payload，确认 adapter 转换后事件不含原始 JSON。
3. 运行 `cargo test domain::agent_event`。

### 4.3 任务：实现 SessionState reducer

交付物：

1. `SessionState`。
2. `apply_event` 纯 reducer。
3. session 排序纯函数。
4. pending interaction 清理规则。

验证标准：

1. `SessionStarted` 创建或更新 session。
2. `ActivityUpdated` 不覆盖 pending approval。
3. `ActivityUpdated` 不覆盖 pending answer。
4. `ApprovalRequested` 进入 `WaitingForApproval`。
5. `AnswerRequested` 进入 `WaitingForAnswer`。
6. `TurnCompleted` 清理 pending。
7. `Detached` 不删除历史状态。
8. 多项目会话不合并。
9. 同项目不同对话不合并。
10. 等待用户操作的会话排序优先。

验证方法：

1. 编写 reducer 单元测试覆盖所有事件分支。
2. 使用 property test 或表驱动测试覆盖多 session 排序。
3. 对照 `open-vibe-island/Tests/OpenIslandCoreTests/SessionStateTests.swift` 的测试意图重写 Rust 测试，不复用其类型。
4. 运行 `cargo test session_state`。

### 4.4 任务：实现 View Model 纯转换

交付物：

1. `SessionListItemViewModel`。
2. `SessionDetailViewModel`。
4. capability 到 UI action 的映射函数。

验证标准：

1. 用量不可用时展示 `--` 或空状态。
2. 不支持能力时不生成对应 action。
3. 长路径和长摘要带截断策略字段。
4. UI 不需要理解 domain 内部状态细节。

验证方法：

1. Rust 单元测试校验 view model 映射。
2. 构造无回写能力 session，确认不生成发送按钮 action。
3. 构造已验证用量，确认生成数字和来源标签。

### 4.5 任务：同步 Domain 事实文档

交付物：

1. `spec/DOMAIN/SESSION_STATE.md`。
2. `spec/DOMAIN/AGENT_EVENT.md`。
3. `spec/DOMAIN/USAGE.md`。
4. `spec/DOMAIN/ERRORS.md`。
5. `spec/TEST.md` 中登记 Domain 测试入口。
6. `spec/00_INDEX.md` 中登记新增文档。

验证标准：

1. 文档说明 Domain 职责、边界和不负责的内容。
2. 文档说明 session 唯一键、状态语义、pending interaction 和用量不可用语义。
3. 文档只描述稳定事实，不复制完整 Rust 字段清单。
4. 每篇文档包含直接相关代码事实入口。
5. 每篇文档包含相关测试入口。

验证方法：

1. 人工对照 `src-tauri/src/domain/` 检查代码事实入口。
2. 人工对照 Domain 测试目录检查测试入口。
3. 运行 `rg "```|\\||mermaid" spec/DOMAIN spec/TEST.md`，确认无代码块、表格和流程图。
4. 从 `spec/00_INDEX.md` 单跳打开所有新增文档。

## 5. 阶段 2：本地 Bridge 与 Hook Helper

目标：实现跨平台本地通信链路，使 hook helper 可以安全地向 APP 发送事件并按需获得 directive。

### 5.1 任务：实现 NDJSON Bridge Codec

交付物：

1. request envelope。
2. response envelope。
3. `schema_version`。
4. `command_type`。
5. `request_id`。
6. `result_type`。
7. 错误编码结构。
8. NDJSON encode/decode。

验证标准：

1. 每行只包含一个 JSON envelope。
2. 半包数据不会被提前解析。
3. 多行数据可连续解析。
4. 非法 JSON 返回 `MalformedAgentPayload` 或 codec 错误。
5. Mac UDS 与 Windows Named Pipe 使用同一 schema。

验证方法：

1. Rust 单元测试覆盖单行、多行、半包、空行、非法 JSON。
2. snapshot 测试固定 envelope 格式。
3. 运行 `cargo test bridge_codec`。

### 5.2 任务：实现 Mac Unix Domain Socket Bridge

交付物：

1. UDS server。
2. UDS client。
3. 请求超时。
4. 连接清理。
5. 测试专用 socket 路径。

验证标准：

1. hook client 可连接 APP bridge。
2. APP 可接收 command 并返回 response。
3. APP 退出后 socket 清理。
4. 连接失败不会阻塞 hook。

验证方法：

1. Mac 上运行集成测试。
2. 测试 bridge 不存在时 client 超时返回。
3. 测试 APP 停止后再次启动不因旧 socket 失败。

### 5.3 任务：实现 Windows Named Pipe Bridge

交付物：

1. Named Pipe server。
2. Named Pipe client。
3. 与 UDS 一致的 request/response codec。
4. 请求超时和连接失败处理。

验证标准：

1. Windows 上 hook client 可连接 APP bridge。
2. Named Pipe codec 与 Mac UDS schema 一致。
3. bridge 不存在时 fail-open。
4. 多请求不会串包。

验证方法：

1. Windows 本机或 CI 运行集成测试。
2. 使用同一组 codec fixture 测试 UDS 和 Named Pipe。
3. 人工关闭 APP 后运行 hook helper，确认不输出阻塞 directive。

### 5.4 任务：实现 builder-panel-hook

交付物：

1. `builder-panel-hook --source codex`。
2. `builder-panel-hook --source claude`。
3. stdin 读取。
4. payload 基础校验。
5. bridge command 发送。
6. stdout directive 编码。
7. fail-open。

验证标准：

1. 空 stdin 直接退出。
2. 非法 JSON 不输出 directive。
3. bridge 不可用时不输出阻塞 directive。
4. Codex `PermissionRequest` 可等待 directive。
5. Claude `PermissionRequest` 可等待 directive。
6. 非阻塞事件短超时返回。

验证方法：

1. CLI 测试传入 fixture stdin。
2. bridge mock 返回 allow/deny directive，校验 stdout。
3. 不启动 bridge 运行 hook，校验退出码和 stdout。
4. 运行 `cargo test builder_panel_hook`。

### 5.5 任务：同步 Bridge 与 Hook 文档

交付物：

1. `spec/API/LOCAL_BRIDGE.md`。
2. `spec/API/HOOK_HELPER.md`。
3. `spec/INTEGRATIONS/CODEX_HOOKS.md`。
4. `spec/INTEGRATIONS/CLAUDE_HOOKS.md`。
5. `spec/ERROR_HANDLING.md` 中登记 bridge、payload 和 fail-open 错误收敛。
6. `spec/00_INDEX.md` 中登记新增文档。

验证标准：

1. bridge 文档说明 request、response、错误和超时语义。
2. hook 文档说明 stdin、stdout directive 和 fail-open 边界。
3. 集成文档说明 Codex 与 Claude hook 的已验证事件范围。
4. 文档不复制完整 JSON 字段清单，只指向协议代码入口。
5. 错误文档说明 bridge 不可用、payload malformed 和 directive 编码失败的外部表现。

验证方法：

1. 人工对照 bridge codec、hook helper 和 adapter validator 代码入口。
2. 人工对照 hook CLI 测试入口。
3. 使用 fixture 名称检查文档中的验收场景是否能定位到测试。
4. 运行 `rg "```|\\||mermaid" spec/API spec/INTEGRATIONS spec/ERROR_HANDLING.md`。

## 6. 阶段 3：Mock Agent 与端到端闭环


### 6.1 任务：实现 Mock Agent Adapter

交付物：

1. mock session start。
2. mock running update。
3. mock approval request。
4. mock answer request。
5. mock completed。
6. mock failed。
7. mock usage update。

验证标准：

1. mock event 可更新 `SessionState`。
2. mock 多项目、多对话不会合并。
3. mock 用量可用时 UI 展示数字。
4. mock 用量不可用时不展示虚假数字。

验证方法：

1. Rust app-service 测试注入 mock adapter。
2. 前端使用 mock Tauri API 渲染 session 列表。
3. Playwright 打开 panel 验证列表内容。

### 6.2 任务：实现 Mock Approval 闭环

交付物：

1. mock approval pending。
2. UI 允许/拒绝按钮。
3. app-service 处理提交。
4. mock directive 接收记录。

验证标准：

1. 点击允许后 mock agent 收到 allow。
2. 点击拒绝后 mock agent 收到 deny。
3. 提交中按钮不能重复点击。
4. 回写失败时 pending 不被误清理。

验证方法：

1. app-service 单元测试校验 allow/deny。
2. Playwright 点击按钮并读取 mock 记录。
3. 注入回写失败，验证 UI 展示错误且按钮恢复。

### 6.3 任务：实现 Mock Reply 闭环

交付物：

1. 开放性回复输入框。
2. 单行和多行输入。
3. `Enter` 发送。
4. `Shift+Enter` 换行。
5. mock reply receiver。

验证标准：

1. 非空内容可发送。
2. 空内容不能发送。
3. 超过最大长度不能发送。
4. 发送成功清空草稿。
5. 发送失败保留草稿。
6. 切换 session 保留各自草稿。

验证方法：

1. 前端组件测试覆盖输入行为。
2. app-service 测试覆盖校验规则。
3. Playwright 验证发送成功和失败路径。


交付物：

1. 完成或失败 session 默认单行。
2. 可 follow-up 的完成或失败 session 右侧展示展开按钮。
3. 展开后第二行只展示快捷输入和单行输入区。
4. 输入框和发送按钮保持一行高度。

验证标准：

1. 完成或失败 session 不自动展示第二行。
2. 点击展开按钮后展示第二行。
3. 第二行不展示独立历史详情入口。
4. 全局 UI 状态不保存长文本全文。

验证方法：

1. 前端组件测试 follow-up 默认单行和手动展开状态。
2. Playwright 验证快捷输入、输入框和发送按钮布局。
3. 性能预算验证 follow-up 展开集合操作。

### 6.5 任务：同步 Mock 闭环文档

交付物：

2. `spec/SERVICE/SESSION_SERVICE.md`。
3. `spec/SERVICE/INTERACTION_SERVICE.md`。
4. `spec/SERVICE/REPLY_SERVICE.md`。
5. `spec/TEST.md` 中登记 mock agent 端到端测试入口。
6. `spec/00_INDEX.md` 中登记新增文档。

验证标准：

1. 系统流程按事实变化顺序描述，不写函数内部步骤。
2. service 文档说明职责、边界、状态写入点、错误收敛和测试入口。
3. mock 文档只作为验证基线，不描述成真实 agent 能力。
4. 文档能说明 mock 闭环如何证明后续真实 adapter 不破坏核心流程。

验证方法：

1. 人工对照 app-service 代码入口。
2. 人工对照 mock adapter 和 Playwright 测试入口。
3. 从 `spec/00_INDEX.md` 单跳定位 mock 流程和测试入口。
4. 运行 `rg "后续完善|灵活处理|按需扩展" spec`，确认没有空泛表述。

## 7. 阶段 4：四类 Agent Adapter 接入

目标：按能力边界接入 Codex APP、Codex CLI、Claude Code APP、Claude Code CLI。

当前执行状态：

1. 已先执行 Codex 部分。
2. Codex CLI 已实现 hook payload 到 session 状态的真实闭环，panel 打开期间会刷新 Codex CLI session，支持 `PermissionRequest` 经 panel 返回 allow 或 deny directive。
3. Codex APP 已实现 app-server schema 探针、request 编码和 notification 转换，schema 以本机 `codex app-server generate-json-schema --experimental` 验证结果为准。
5. Claude Code APP 和 Claude Code CLI 尚未执行阶段 4 真实闭环。
6. Windows 部分不做本机验证。

### 7.1 任务：Codex APP Adapter

交付物：

1. Codex APP 发现能力。
2. app-server schema 探针结果。
3. notification 接收。
4. thread / turn / waiting 状态转换。
5. RPC 创建会话和回复能力探针。
6. 用量来源解析。
7. 降级能力矩阵。

验证标准：

1. Codex APP 运行时可发现会话。
2. 等待审批或等待输入时状态更新。
3. 支持 RPC 时才展示结构化发送能力。
4. 不支持回写时 UI 不展示发送按钮。
5. 用量只展示已验证字段。
6. adapter 不把 app-server 原始 JSON 放入 domain。

验证方法：

1. 使用记录下来的 app-server fixture 做 adapter 单元测试。
2. 使用本机 Codex APP 做人工验证。
3. 断开 app-server，确认只读或复制降级。
4. 用 `rg "serde_json::Value" src-tauri/src/domain` 检查脏数据未进入 domain。

### 7.2 任务：Codex CLI Adapter

交付物：

1. Codex hook payload validator。
2. `SessionStart` 转换。
3. `UserPromptSubmit` 转换。
4. `PermissionRequest` 转换。
5. `Stop` 转换。
6. Codex directive encoder。
7. `/status` 或已验证用量解析。

验证标准：

1. Codex CLI 启动后 panel 展示 session。
2. Codex CLI 请求审批时 session 进入 `WaitingForApproval`。
3. 用户允许/拒绝后 stdout directive 正确。
4. turn 完成后状态为 `Completed` 并触发通知。
5. bridge 不可用时 hook fail-open。

验证方法：

1. fixture 测试 Codex hook payload 校验。
2. CLI 集成测试模拟 stdin 和 stdout。
3. 本机 Codex CLI 人工触发审批。
4. 不启动 APP 直接运行 hook，确认不阻塞 Codex。

### 7.3 任务：Claude Code APP Adapter

交付物：

1. Claude Code APP 发现能力。
2. 公开本地协议验证记录。
3. 进程级状态展示。
4. 跳回能力。
5. 有限回写能力判断。
6. 降级能力矩阵。

验证标准：

1. Claude Code APP 可被发现时 panel 展示对应状态。
2. 未验证结构化协议前不展示审批回写能力。
3. 不支持回写时只展示跳回或复制降级。
4. 托管启动会话可按托管能力展示回复入口。

验证方法：

1. 人工启动 Claude Code APP 验证发现。
2. adapter 单元测试校验 unsupported 能力。
3. Playwright 确认不出现虚假发送按钮。
4. 托管启动样例验证有限回写。

### 7.4 任务：Claude Code CLI Adapter

交付物：

1. Claude hook payload validator。
2. `SessionStart` 转换。
3. `UserPromptSubmit` 转换。
4. `PreToolUse` 转换。
5. `PermissionRequest` 转换。
6. `Notification` 转换。
7. `Stop` 和 `SessionEnd` 转换。
8. Claude directive encoder。
9. 用量来源解析。

验证标准：

1. Claude Code CLI hook 能触发 session 状态更新。
2. `PermissionRequest` 能在 panel 中处理。
3. `Stop` 后触发完成状态和通知。
4. 结构化问题进入 `WaitingForAnswer`。
5. 非结构化自然语言不被误解析为选项。
6. 用量不可用时输出 `Unavailable`。

验证方法：

1. fixture 测试 Claude hook payload 校验。
2. CLI 集成测试校验 stdout directive。
3. 本机 Claude Code CLI 人工触发审批。
4. 模拟 malformed payload，确认被 adapter 拒绝。

### 7.5 任务：同步 Agent 集成文档

交付物：

1. `spec/INTEGRATIONS/CODEX_APP.md`。
2. `spec/INTEGRATIONS/CODEX_CLI.md`。
3. `spec/INTEGRATIONS/CLAUDE_CODE_APP.md`。
4. `spec/INTEGRATIONS/CLAUDE_CODE_CLI.md`。
5. `spec/EXTERNAL_BEHAVIOR.md` 中登记四类 agent 的外部可见能力。
6. `spec/DECISION_LOG.md` 中登记未验证能力的降级决策。
7. `spec/00_INDEX.md` 中登记新增文档。

验证标准：

1. 每篇集成文档说明 supported、degraded 和 unsupported 能力。
2. 每篇集成文档说明协议事实入口、用量来源、回写边界和降级表现。
3. Claude Code APP 文档明确未验证公开协议前不承诺结构化审批和回复。
4. Codex APP 文档明确 app-server schema 必须以本项目验证结果为准。
5. 外部行为文档说明用户能观察到的状态、限制和错误表现。

验证方法：

1. 对照 adapter 代码入口和 fixture 测试入口。
2. 对照真实 agent 人工验证记录。
3. 人工检查每个 unsupported 能力在 UI 中没有动作入口。
4. 从 `spec/00_INDEX.md` 单跳定位四类 adapter 文档。

## 8. 阶段 5：交互能力完整化

目标：完成审批、选项、开放性回复、快捷回复、预设命令、跳回和回写降级。

### 8.1 任务：审批处理

交付物：

1. 审批 UI。
2. 允许、拒绝、允许并记住。
3. 提交中状态。
4. 失败原因展示。
5. 复制降级。

验证标准：

1. 审批信息展示工具名、路径、命令摘要或权限范围。
2. 成功后清理 pending。
3. 失败后 pending 保留。
4. 处理中不能重复点击。
5. 不支持审批能力时不展示审批按钮。

验证方法：

1. app-service 测试审批状态机。
2. Playwright 验证按钮状态和错误恢复。
3. mock agent 校验收到 allow/deny。

### 8.2 任务：选项处理

交付物：

1. 单选 UI。
2. 多选 UI。
3. 多选提交按钮。
4. 长文本换行。
5. 失败重试。

验证标准：

1. 单选点击即发送。
2. 多选至少选择一项才能提交。
3. 发送前检查 session 仍可回写。
4. 失败后保留选择状态。
5. 长选项不溢出按钮。

验证方法：

1. 前端组件测试长文本布局。
2. app-service 测试单选、多选和失败路径。
3. Playwright 验证实际点击行为。

### 8.3 任务：开放性回复

交付物：

1. 单行输入。
2. 多行输入。
3. 发送快捷键。
4. 最大长度配置。
5. 发送失败保留草稿。

验证标准：

1. `can_send_reply = true` 时可编辑。
2. `can_send_reply = false` 时只读或复制降级。
3. 空内容不能发送。
4. 成功清空草稿。
5. 失败保留草稿并展示原因。

验证方法：

1. 前端组件测试键盘行为。
2. app-service 测试 reply target 选择。
3. Playwright 验证成功和失败路径。

### 8.4 任务：快捷回复

交付物：

1. 创建、编辑、排序、禁用。
2. agent 类型绑定。
3. 项目或工作目录绑定。
4. session detail 展示可用快捷回复。
5. 点击复用开放性回复路径。

验证标准：

1. 当前 session 只展示匹配快捷回复。
2. 不支持回写时不直接发送。
3. 发送失败后内容填入输入框。
4. 快捷回复不绕过 capability 判断。

验证方法：

1. shortcut service 单元测试过滤规则。
2. app-service 测试复用 reply service。
3. Playwright 验证失败后填回草稿。

### 8.5 任务：预设命令与创建新对话

交付物：

1. 预设命令 CRUD。
2. agent 类型、工作目录、启动命令、初始 prompt。
3. 自动发送回车配置。
4. 默认快捷回复组。
5. 结构化创建优先。
6. 托管启动和复制降级。

验证标准：

1. 支持结构化创建时优先使用结构化 API。
2. 不支持结构化创建时启动托管进程或打开终端。
3. 无法可靠输入时复制命令并提示。
4. 自动发送回车只在用户配置后执行。
5. Windows 只保证托管进程可靠回写。

验证方法：

1. preset service 单元测试命令生成。
2. mock managed process 集成测试。
3. Mac 验证 tmux / Ghostty 路径。
4. Windows 验证 PowerShell / cmd 托管 stdin。

### 8.6 任务：跳回与回写分离

交付物：

1. `JumpTargetPort` 实现。
2. `ReplySenderPort` 实现。
3. Mac terminal adapter。
4. Windows managed process adapter。
5. clipboard fallback。

验证标准：

1. 能跳回不代表能发送。
2. UI 对跳回和发送展示不同能力。
3. 回写失败时不重复自动发送。
4. clipboard fallback 可用。

验证方法：

1. 单元测试校验 capability 映射。
2. 人工验证终端跳回。
3. 注入回写失败，确认没有二次发送。
4. 验证剪贴板内容正确。

### 8.7 任务：同步交互服务文档

交付物：

1. `spec/SERVICE/SHORTCUT_REPLY_SERVICE.md`。
2. `spec/SERVICE/PRESET_COMMAND_SERVICE.md`。
3. `spec/API/REPLY_TARGETS.md`。
4. `spec/INFRA/TERMINAL.md`。
5. `spec/ERROR_HANDLING.md` 中登记回复失败、终端回写失败和复制降级。
6. `spec/EXTERNAL_BEHAVIOR.md` 中登记审批、选项、回复、快捷回复和预设命令的用户可见行为。
7. `spec/00_INDEX.md` 中登记新增文档。

验证标准：

1. 文档说明跳回和文本回写是两个独立能力。
2. 文档说明各类 `ReplyTarget` 的稳定语义和错误表现。
3. 文档说明失败后草稿、pending 和用户提示的事实变化。
4. 文档说明 Windows 回写限定为托管会话。
5. 文档不复制 UI 组件实现细节。

验证方法：

1. 对照 `ReplySenderPort`、`JumpTargetPort` 和相关 service 代码入口。
2. 对照交互测试入口和 Playwright 场景。
3. 人工检查错误文档中是否说明是否重试、是否补偿、用户是否可见。
4. 运行 `rg "能跳回.*能发送|能发送.*能跳回" spec`，检查是否存在混淆表述。




交付物：

1. 删除独立历史详情 service、port、adapter 缓存和 Tauri command。
2. 保留 session 更新事件去重。
3. 保留 Codex APP rollout 输出摘要补齐，不提供独立详情查询。

验证标准：

1. 当前不提供独立历史详情接收链路。
4. 不从 transcript / JSONL 反读。
5. session 更新事件保持去重。

验证方法：

1. adapter 单元测试验证 session 更新转换和去重。
2. 前端测试验证无独立历史详情入口。


交付物：

1. follow-up 展开状态轻量集合。
2. session 摘要截断策略。
3. session 捕捉顺序合并规则。

验证标准：

1. follow-up 展开状态不进入后端能力模型。
2. session 摘要保持截断。
3. 全局 UI 状态不保存长文本全文。

验证方法：

1. 前端 store 测试验证展开状态切换和清理。
2. 性能预算校验轻量 UI 状态集合。


交付物：

1. 完成或失败 session 默认单行。
2. 可 follow-up 的完成或失败 session 右侧展示展开按钮。
3. 展开后第二行只展示快捷输入和单行输入区。
4. 输入框和发送按钮保持一行高度。
5. 提交成功后收起第二行并清理草稿。

验证标准：

1. 完成和失败 session 不自动展示第二行。
2. 点击展开按钮后展示第二行。
3. 第二行不展示独立历史详情入口。
4. 快捷输入、输入框和发送按钮不撑破布局。
5. follow-up 提交成功后收起第二行。

验证方法：

1. 前端组件测试默认单行和手动展开。
2. 视觉截图检查长文本布局。
3. `rg "export" src` 检查无未实现导出入口。


交付物：

1. 删除独立历史详情服务文档入口。
3. `spec/EXTERNAL_BEHAVIOR.md` 中登记当前不提供独立历史详情浮层。
6. `spec/00_INDEX.md` 中登记新增文档。

验证标准：

3. 文档明确不从 transcript / JSONL 反向读取历史详情数据。
4. 文档说明完成或失败 session 的 follow-up 展开规则。

验证方法：

2. 对照性能预算和前端交互测试入口。
3. 运行 `rg "transcript|JSONL|jsonl|持久化" spec/INTERNAL_BEHAVIOR.md`，人工确认语义为禁止或边界说明。
4. 运行 `rg "```|\\||mermaid" spec/INTERNAL_BEHAVIOR.md spec/EXTERNAL_BEHAVIOR.md`。

## 10. 阶段 7：UI 完整化

目标：完成扩展模式 panel、session 列表、detail、设置页、通知、用量展示和视觉打磨。

### 10.1 任务：扩展模式 Panel

交付物：

1. 扩展模式布局。
2. session 列表区域。
3. session detail 区域。
4. 等待数量和运行数量。
5. 状态动画。
6. 收缩和展开。

验证标准：

1. 首版默认扩展模式。
2. 不展示 mini 模式切换入口。
3. 收缩和展开后保留选中 session。
4. 收缩和展开后回复草稿不丢失。
5. 收缩后动画和刷新降级。

验证方法：

1. Playwright 验证默认布局和无 mini 入口。
2. 前端 store 测试选中 session 和草稿保留。
3. 性能测试比较收缩前后动画刷新。

### 10.2 任务：Session 列表 UI

交付物：

1. agent 名称。
2. 项目名或工作目录。
3. 对话标题或 ID。
4. 状态标签。
5. 摘要。
6. 更新时间。
7. 用量数字。
8. action 图标。

验证标准：

1. 等待用户操作状态排在前面。
2. 同状态按更新时间倒序。
3. 同一 agent 不同项目显示独立条目。
4. 同项目不同对话显示独立条目。
5. 长摘要不撑破布局。
6. 不支持动作不展示按钮。

验证方法：

1. 前端组件测试排序和渲染。
2. Playwright 注入多项目多对话 fixture。
3. 视觉截图检查长文本。

### 10.3 任务：Session Detail UI

交付物：

1. 标题区。
2. 标识区。
3. 用量区。
4. 摘要区。
5. 执行信息区。
6. 操作区。
7. 工具区。

验证标准：

1. 审批、选项、输入框、快捷回复按能力展示。
2. 用量可用时展示数字和来源。
3. 用量不可用时展示 `--` 或隐藏。
4. 长路径、长命令折叠或换行。
5. 按钮文字不溢出。

验证方法：

1. 组件测试不同 capability fixture。
2. Playwright 视觉截图覆盖长路径和长命令。
3. 用量 view model 测试 verified/unavailable。

### 10.4 任务：设置页

交付物：

1. General。
2. Display。
3. Agents。
4. Replies。
5. Presets。
6. Terminal。
7. Advanced。

验证标准：

1. 设置修改后重启仍生效。
2. 配置损坏时使用默认值并提示。
3. agent 接入开关生效。
4. 用量展示开关生效。
5. hook 安装前展示将修改的配置。
6. 不提供自动更新配置项。

验证方法：

1. app-service 配置测试。
2. Playwright 修改设置并重启 APP。
3. 人工破坏配置文件后启动 APP。
4. hook 安装流程截图验证。

### 10.5 任务：系统通知

交付物：

1. 完成通知。
2. 等待审批通知。
3. 等待选择通知。
4. 失败通知。
5. 重复通知合并。
6. 通知点击定位 session。

验证标准：

1. turn 完成触发通知。
2. 等待审批触发通知。
3. 当前查看 session 时不重复弹通知。
4. 同 session 短时间重复通知被合并。
5. 点击通知后聚焦 panel 并展开对应 session。
6. 点击通知只聚焦 panel 并定位 session。

验证方法：

1. notification service 单元测试合并规则。
2. Mac 人工验证系统通知。
3. Windows 人工验证系统通知。
4. mock notification adapter 测试点击回调。

### 10.6 任务：同步 UI 外部行为文档

交付物：

1. `spec/EXTERNAL_BEHAVIOR.md` 中登记扩展模式、session 列表、session detail、设置页、通知和用量展示。
2. `spec/SERVICE/NOTIFICATION_SERVICE.md`。
3. `spec/INFRA/UI_RUNTIME.md`。
4. `spec/TEST.md` 中登记 UI 组件测试、Playwright 测试和视觉验证入口。
5. `spec/00_INDEX.md` 中登记新增文档。

验证标准：

1. 外部行为文档只记录用户可观察行为，不记录组件内部实现。
2. 文档明确首版不展示 mini 模式入口。
3. 文档明确不展示不可执行动作。
4. 文档明确通知点击只定位 session。
5. 文档包含 UI 验收口径和测试入口。

验证方法：

1. 对照 React view、store 和 Tauri command 入口。
2. 对照 Playwright 与组件测试入口。
3. 人工检查文档没有营销式描述和模糊表达。
4. 从 `spec/00_INDEX.md` 单跳定位 UI 行为和通知行为。

## 11. 阶段 8：配置、安全、性能与发布质量

目标：完成本地配置、安全策略、性能预算、跨平台验证和发布质量检查。

执行口径：

1. 本阶段自动化验证覆盖 Mac 当前开发环境和平台无关逻辑。
2. Windows 路径和 Named Pipe 相关代码只保留静态事实，不执行 Windows 本机验证。
3. 未执行的 Windows 人工验收不得写成已通过结论。

### 11.1 任务：配置原子读写

交付物：

1. Mac 配置路径。
2. Windows 配置路径。
3. JSON schema 校验。
4. 默认值。
5. 临时文件写入。
6. 原子替换。
7. 配置损坏降级。

验证标准：

1. 缺失字段使用默认值。
2. 非法字段不进入 domain。
3. 写入失败不覆盖旧配置。
4. 配置损坏时 UI 提示。
5. 用量展示开关持久化。

验证方法：

1. config adapter 单元测试读写。
2. 模拟写入失败。
3. 人工破坏 JSON 后启动 APP。
4. 重启 APP 验证设置保留。

### 11.2 任务：Hook 安装与卸载

交付物：

1. Codex hook 安装。
2. Codex hook 卸载。
3. Claude hook 安装。
4. Claude hook 卸载。
5. 修改前备份。
6. 安装 manifest。

验证标准：

1. 安装前展示将修改的文件。
2. 修改第三方配置前备份。
3. 卸载可恢复到安装前状态。
4. 不静默提权。
5. Codex trust review 不被绕过。

验证方法：

1. 使用临时目录 fixture 测试配置修改。
2. 对比安装前后文件 diff。
3. 执行卸载后对比备份恢复结果。
4. 人工验证真实 Codex / Claude 配置。

### 11.3 任务：日志与敏感信息保护

交付物：

1. 中文业务事件名。
2. 日志脱敏。
3. 调试模式确认。
4. 错误统一收敛。

验证标准：

1. 默认日志不记录 prompt 全文。
2. 默认日志不记录 transcript 全文。
4. 调试模式开启前有明确确认。
5. 同一错误链路不重复 catch-log-reraise。

验证方法：

1. 单元测试校验日志脱敏函数。
2. 注入包含敏感文本的 mock event，检查日志输出。
3. `rg "catch|error!" src-tauri/src` 人工审查重复日志。

### 11.4 任务：性能预算验证

交付物：

1. 空闲 CPU 测试脚本。
2. 10 session 测试。
3. 1000 event 测试。
5. 收缩模式降级测试。
6. 内存释放测试。

验证标准：

1. 空闲 10 分钟 CPU 接近 0。
2. 10 个 session 同时存在时操作流畅。
3. 连续 1000 条 mock event 不丢失。
5. follow-up 展开集合操作保持轻量。

验证方法：

1. 使用 mock event generator。
2. 使用 Playwright 性能场景。
3. 使用系统监控采集 CPU 和内存。
4. 在 Mac 和 Windows 分别记录结果。

### 11.5 任务：跨平台验收矩阵

交付物：

1. Mac 验证清单。
2. Windows 验证清单。
3. agent 能力矩阵。
4. 降级策略矩阵。

验证标准：

1. Mac 支持置顶 panel、多显示器、通知、UDS、四类 agent 入口。
2. Windows 支持置顶 panel、多显示器、通知、Named Pipe、四类 agent 入口。
3. Windows 只承诺托管会话可靠回写。
4. Mac 优先验证 tmux / Ghostty 回写。
5. 不支持能力被标记为 degraded 或 unsupported。

验证方法：

1. Mac 人工跑完整 mock 和真实 agent 流程。
2. Windows 人工跑完整 mock 和真实 agent 流程。
3. 对每个 agent 记录 supported/degraded/unsupported。
4. 保存验证日志和截图。

### 11.6 任务：文档系统质量门禁

交付物：

1. 完整 `spec/` 文档系统。
2. `spec/00_INDEX.md` 完整事实导航。
3. 代码事实入口检查记录。
4. 测试入口检查记录。
5. 文档质量检查记录。

验证标准：

1. `spec/00_INDEX.md` 能定位全部事实入口。
2. 每个实际存在模块都有职责、边界、代码入口和测试入口。
3. 每个外部接口都有协议事实入口、错误语义和验收场景。
4. 每个核心状态都有来源、值域、转移规则和非法状态处理方式。
5. 每个错误类别都有收敛、降级、重试和外部表现。
6. 每个核心决策都有原因、影响和状态。
7. 文档没有流程图、表格、emoji 和实现代码片段。
8. 文档没有复制完整字段清单。
9. 文档没有连续引用链。
10. 文档与代码事实入口一致。

验证方法：

1. 运行 `find spec -type f | sort`，对照 `spec/00_INDEX.md` 检查索引完整性。
2. 运行 `rg "```|\\||mermaid" spec`，检查代码块、表格和流程图。
3. 运行 `rg "后续完善|灵活处理|按需扩展" spec`，检查空泛表述。
4. 人工对照 `src-tauri/src/domain`、`services`、`adapters`、`tauri_api` 检查代码事实入口。
5. 人工对照 Rust、前端和 Playwright 测试目录检查测试入口。
6. 抽查每篇文档只引用直接相关文档和代码入口。
7. 对照 `SPEC_DOC.md` 第 11 节质量门禁逐项验收。

## 12. 验证矩阵汇总

| 阶段   | 自动化验证                                             | 人工验证                                     | 阻塞标准                                                       | 完成标准                                   |
| ------ | ------------------------------------------------------ | -------------------------------------------- | -------------------------------------------------------------- | ------------------------------------------ |
| 阶段 0 | `cargo test`、前端测试、lint、架构扫描、spec 结构检查  | 空 panel 启动、窗口拖动和焦点、spec 索引检查 | 工程无法启动、Domain 污染或 spec 骨架缺失                      | Mac/Windows 空 panel 可运行，spec 入口建立 |
| 阶段 1 | Domain 单元测试、snapshot、排序测试、Domain 文档检查   | 无                                           | reducer 行为不符合需求或 Domain 文档缺失事实入口               | 核心状态和 view model 可稳定输出           |
| 阶段 2 | codec、UDS、Named Pipe、hook CLI 测试、API 文档检查    | bridge 不可用 fail-open                      | hook 阻塞 agent、输出错误 directive 或 bridge 文档缺失错误语义 | hook 到 bridge 链路稳定                    |
| 阶段 3 | mock adapter、app-service、UI 测试、流程文档检查       | mock 端到端流程                              | mock 流程不能闭环或流程文档无法定位测试入口                    | 无真实 agent 时核心流程可用                |
| 阶段 4 | adapter fixture、directive 编码测试、集成文档检查      | 四类真实 agent 验证                          | 虚构未验证能力或集成文档能力矩阵不一致                         | 四类入口按能力展示和降级                   |
| 阶段 5 | interaction、reply、preset 测试、交互文档检查          | 审批、回复、跳回、复制降级                   | 失败时误清状态、重复发送或文档混淆跳回与回写                   | 交互链路可用且可降级                       |
| 阶段 7 | UI 组件、Playwright、通知测试、外部行为文档检查        | 多屏、长文本、通知点击                       | UI 展示假按钮、布局溢出或外部行为文档缺失验收口径              | 扩展模式完整可用                           |
| 阶段 8 | 配置、hook 安装、性能脚本、spec 质量门禁               | Mac/Windows 全量验收、spec 全量审查          | 安全、配置、性能或文档系统不达标                               | 发布质量达标                               |

## 13. 任务执行模板

每个开发任务必须按以下模板执行：

1. 修改前先阅读 `spec/00_INDEX.md`。
2. 明确输入、输出、状态和错误边界。
3. 明确受影响的 spec 文档。
4. 写测试或验证 fixture。
5. 实现最小可用代码。
6. 运行相关自动化测试。
7. 做必要人工验证。
8. 同步更新受影响文档。
9. 检查 `spec/00_INDEX.md`、代码事实入口和测试入口一致。
10. 检查无跨层依赖和无虚假能力展示。
11. 做出核心架构或产品决策时更新 `spec/DECISION_LOG.md`。

任务完成记录应包含：

1. 修改文件。
2. 新增测试。
3. 已运行命令。
4. 人工验证步骤。
5. 未覆盖风险。
6. 后续需要重验的外部协议。
7. 已更新的 spec 文档。
8. 文档质量检查结果。

## 14. 阻塞与回退规则

必须阻塞继续推进的情况：

1. Domain 被 adapter、Tauri 或 UI 类型污染。
2. hook helper 在 bridge 不可用时会阻塞 agent。
3. UI 展示当前 session 实际不支持的动作。
4. 用量来源未验证却展示数字。
6. 配置写入失败会覆盖旧配置。
7. 回写失败后误清草稿或 pending。
8. `spec/00_INDEX.md` 无法定位新增事实入口。
9. 文档复制实现代码、完整字段清单或与代码事实冲突。
10. 文档没有登记相关测试入口。

允许降级继续推进的情况：

1. Codex APP RPC 未验证，降级只读或复制。
2. Claude Code APP 未发现公开协议，降级只读和跳回。
3. Windows 任意已有终端注入不可行，限定托管进程回写。
4. 用量来源不可用，展示 `--` 或隐藏。

回退方法：

1. 保留 mock agent 作为端到端基线。
2. 每个 adapter 以 feature flag 或配置开关控制。
3. 外部协议变化时先禁用对应能力，再更新 schema 和测试。
4. 真实 agent 接入失败不得影响已完成的 mock 和 domain 流程。
5. 文档与代码冲突时，先修正事实来源，再继续实现。

## 15. 执行记录

### 15.1 阶段 0 记录

状态：已建立工程骨架和 spec 骨架。

已交付：

1. Tauri + React + TypeScript + Rust 工程结构。
2. Rust 分层目录：`domain`、`ports`、`adapters`、`services`、`tauri_api`。
3. 前端分层目录：`components`、`stores`、`views`、`api`。
4. 基础 panel 页面、收缩状态、拖动区域和基础探针 command。
5. 多显示器位置修正纯函数。
6. 架构边界检查脚本。
7. `spec/` 文档系统骨架。
8. 本地 pnpm workspace 固定配置。

已验证：

1. `pnpm test`。
2. `pnpm lint`。
3. `pnpm build`。
4. `pnpm format`。
5. `pnpm architecture:check`。
6. Browser 打开 `http://127.0.0.1:1420/` 验证页面可渲染、收缩按钮可交互、控制台无 error。

未覆盖：

1. Windows 原生窗口人工验证。
2. Mac 原生 Tauri 窗口启动、拖动、焦点和重启位置恢复人工验证。

### 15.2 阶段 1 记录

状态：Domain 核心类型、归一事件、纯 reducer 和 view model 纯转换已建立。

已交付：

1. `AgentKind`、`ProjectId`、`ConversationId`、`SessionKey`。
2. `SessionStatus`、`SessionCapabilities`、`AgentSession`。
3. `UsageSnapshot`、`UsageValue`、`VerifiedUsageValue`。
4. `AgentInteraction`、`ReplyTarget` 和回答交互模型。
5. `AppError`、`AppErrorCode` 和降级动作。
6. `AgentEvent` 事件族。
7. `SessionState::apply_event` 纯 reducer。
8. session 排序纯函数。
10. capability 到 UI action 的纯映射。
11. Domain 事实文档和工程运行时文档。
12. Tauri 所需基础图标资源。
13. Tauri 基础 CSP 和图标配置。

已验证：

1. `cargo test --manifest-path src-tauri/Cargo.toml`，29 个 Rust 单元测试通过。

已补充验证：

1. `pnpm test`。
2. `pnpm lint`。
3. `pnpm build`。
4. `pnpm format`。
5. `pnpm architecture:check`。
6. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`。
7. `cargo --config 'source.crates-io.replace-with="tuna"' --config 'source.tuna.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test --manifest-path src-tauri/Cargo.toml`，36 个 Rust 单元测试通过。
8. `pnpm tauri info`。
9. `rg "tauri|std::fs|tokio|serde_json::Value" src-tauri/src/domain` 无命中。
10. `rg "```|\\||mermaid" spec/DOMAIN spec/TEST.md` 无命中。

独立 reviewer 反馈处理：

1. 修复 `SessionStarted` 更新已完成或失败 session 后不恢复运行态的问题。
2. 修复事件外层 `SessionKey` 与 pending interaction 内层 `SessionKey` 不一致时的污染风险。
3. 修复 view model 只看 capability 导致终态 session 生成过期动作的问题。
4. 使用 `UsageAmount` 限制已验证用量数字必须为非负有限数。
5. 删除仓库级 Cargo registry 强制替换配置，避免影响其他开发者和 CI。
6. 显式配置 Tauri 图标和基础 CSP。

已知环境说明：

1. 用户级 `/Users/air/.cargo/config.toml` 将 crates.io 替换为 Aliyun mirror，当前该 mirror 缺少 lockfile 中的 `log 0.4.32`。
2. 本轮 Rust 测试使用命令级 `--config` 覆盖到 TUNA mirror 完成验证。

### 15.3 阶段 2 记录

状态：本地 bridge codec、Mac Unix Domain Socket 传输、Windows Named Pipe 传输代码、hook helper 基础链路和协议文档已建立。

已交付：

1. Bridge request envelope。
2. Bridge response envelope。
3. `schema_version`。
4. `command_type`。
5. `request_id`。
6. `result_type`。
7. Bridge 错误编码结构。
8. NDJSON encode 和 decode。
9. Mac Unix Domain Socket server 和 client。
10. Windows Named Pipe server 和 client 代码。
11. `builder-panel-hook --source codex`。
12. `builder-panel-hook --source claude`。
13. stdin 读取、payload 基础校验、bridge command 发送、stdout directive 编码和 fail-open。
14. bridge response request ID 和 agent source 错配保护。
15. Claude PreToolUse allow、deny、ask 显式工具权限 directive。
16. `spec/API/LOCAL_BRIDGE.md`。
17. `spec/API/HOOK_HELPER.md`。
18. `spec/INTEGRATIONS/CODEX_HOOKS.md`。
19. `spec/INTEGRATIONS/CLAUDE_HOOKS.md`。
20. 相关系统、错误、内部、外部、测试、决策和运行时文档更新。

已验证：

1. `cargo --config 'source.crates-io.replace-with="tuna"' --config 'source.tuna.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test --manifest-path src-tauri/Cargo.toml bridge -- --nocapture`，29 个阶段 2 相关 Rust 测试通过。
2. `cargo --config 'source.crates-io.replace-with="tuna"' --config 'source.tuna.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test --manifest-path src-tauri/Cargo.toml`，65 个 Rust 单元测试通过。
3. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`。
4. `pnpm test`。
5. `pnpm lint`。
6. `pnpm build`。
7. `pnpm format`。
8. `rg '```|\\||mermaid' spec/API spec/INTEGRATIONS spec/ERROR_HANDLING.md spec/SYSTEM_FLOWS.md spec/INTERNAL_BEHAVIOR.md spec/TEST.md spec/00_INDEX.md spec/SYSTEM_OVERVIEW.md spec/EXTERNAL_BEHAVIOR.md spec/DECISION_LOG.md spec/INFRA/PROJECT_RUNTIME.md` 无命中。
9. `rg "tauri|std::fs|tokio|serde_json::Value" src-tauri/src/domain` 无命中。
10. `builder-panel-hook --source codex` 在 bridge 不存在时退出码为 0，stdout 为空。

已知环境说明：

1. 用户级 `/Users/air/.cargo/config.toml` 将 crates.io 替换为 Aliyun mirror，当前该 mirror 缺少 lockfile 中的 `log 0.4.32`。
2. 本轮阶段 2 Rust 测试使用命令级 `--config` 覆盖到 TUNA mirror 完成验证。
3. Windows Named Pipe 代码未在 Windows 本机完成验证。
4. Windows 跨目标 `cargo check --target x86_64-pc-windows-msvc` 因依赖下载耗时过长被终止，未形成验证结论。

### 15.4 阶段 3 记录

状态：Mock Agent 与端到端闭环已建立。

已交付：

1. 阶段 3 mock agent adapter。
2. 进程内 mock agent runtime。
3. mock session start、running update、approval request、answer request、completed、failed 和 usage update。
4. session 列表与详情读取服务。
5. mock approval allow 和 deny 回写服务。
6. mock text reply 回写服务。
10. `Enter` 发送和 `Shift+Enter` 换行。
11. 提交中状态、失败提示和失败后 pending 保留。
12. 按 session 隔离的回复草稿。
14. `spec/SERVICE/SESSION_SERVICE.md`。
15. `spec/SERVICE/INTERACTION_SERVICE.md`。
16. `spec/SERVICE/REPLY_SERVICE.md`。
17. 相关系统、错误、内部、外部、测试、决策和索引文档更新。

已验证：

1. `pnpm test`，2 个前端测试文件通过，8 个测试通过。
2. `pnpm lint`，ESLint 和架构边界检查通过。
3. `pnpm build`，TypeScript 和 Vite 生产构建通过。
4. `cargo fmt --manifest-path src-tauri/Cargo.toml`。
5. `cargo --config 'source.crates-io.replace-with="tuna"' --config 'source.tuna.registry="sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' test --manifest-path src-tauri/Cargo.toml`，83 个 Rust 单元测试通过。
6. `pnpm exec prettier --check spec`。
7. `rg '```|\\||mermaid' spec` 无命中。
8. `rg "后续完善|灵活处理|按需扩展" spec` 无命中。

已知环境说明：

1. 用户级 `/Users/air/.cargo/config.toml` 将 crates.io 替换为 Aliyun mirror，当前该 mirror 缺少 lockfile 中的 `log 0.4.32`。
2. 直接运行 `cargo test --manifest-path src-tauri/Cargo.toml` 和追加 `--locked` 都会被该 mirror 缺包阻塞。
3. 本轮 Rust 测试使用命令级 `--config` 覆盖到 TUNA mirror 完成验证。
4. Windows 部分按本轮要求不做本机验证。

独立 reviewer 反馈处理：

1. 修复 `inject_failure` 在 pending 校验前污染下一次有效提交的问题。
3. 修复前端回复长度使用 UTF-16 code unit 计数的问题，改为与 Rust `chars().count()` 对齐的 Unicode 字符计数。

### 15.5 阶段 4 记录

状态：Codex 真实接入先行，Claude Code 真实闭环尚未完成。

已交付：

1. Codex CLI hook payload validator。
2. Codex CLI `SessionStart`、`UserPromptSubmit`、`PermissionRequest` 和 `Stop` 到归一事件转换。
3. Codex CLI hook runtime、进程内 session state 和 bridge server。
4. Codex CLI `PermissionRequest` 等待 panel 决策并返回 allow 或 deny stdout directive。
5. Codex CLI 审批等待超时清理 pending approval。
6. Codex CLI 同一 session 新审批让旧审批等待器过期。
8. 前端 Codex CLI session 读取 API 和 panel 打开期间刷新入口。
9. Codex APP app-server schema 探针。
10. Codex APP `initialize`、`initialized`、`thread/start` 和 `turn/start` request 编码。
11. Codex APP app-server notification 到归一事件转换。
12. Codex APP token usage 只从已验证 notification 字段读取。
14. `spec/INTEGRATIONS/CODEX_APP.md`。
15. `spec/INTEGRATIONS/CODEX_CLI.md`。
16. 相关系统、流程、外部、内部、错误、测试、决策和索引文档更新。

已验证：

1. `src-tauri/src/adapters/codex_cli_hook/mod.rs` 中 Codex CLI hook 转换、审批等待、超时清理和旧等待器过期测试。
2. `src-tauri/src/adapters/codex_app/mod.rs` 中 Codex APP schema 探针、request 编码和 notification 转换测试。
3. `src/views/BuilderPanelApp.test.ts` 中 Codex CLI session 刷新、runtime source 路由和 mock/Codex CLI session 选中身份测试。
4. `spec/TEST.md` 已登记阶段 4 当前测试入口和未完成边界。
5. `spec/EXTERNAL_BEHAVIOR.md` 已登记 Codex CLI 审批闭环和 Codex APP 降级表现。

未覆盖：

1. Claude Code APP 真实发现、状态展示、跳回和有限回写未完成。
2. Claude Code CLI 真实 hook 到 session 状态闭环未完成。
4. Windows 本机验证未执行。
5. Codex APP app-server 只按当前本机 schema 探针结果声明，不作为跨版本稳定协议承诺。
6. 当前不存在 `spec/INTEGRATIONS/CLAUDE_CODE_APP.md` 和 `spec/INTEGRATIONS/CLAUDE_CODE_CLI.md`，Claude 真实接入完成时必须新增并登记索引。

### 15.6 阶段 5 记录

状态：交互能力完整化的 mock 闭环、纯服务契约和降级边界已建立。

已交付：

1. mock 审批允许并记住决策。
2. mock 单选和多选提交服务。
3. choice view model 选项结构化输出。
4. mock choice Tauri command 和前端 API。
5. 前端 choice 单选、多选、提交中状态、失败演练和失败后选择保留。
6. 快捷回复过滤和排序服务。
7. 前端文本回复快捷回复入口，失败后内容回填草稿。
8. 预设命令计划生成服务。
9. 结构化创建优先、托管进程降级和复制降级模型。
10. `JumpTargetPort`。
11. 终端跳回 adapter 的记录和复制降级模型。
12. 跳回能力和文本回写能力分离文档。
13. `spec/SERVICE/SHORTCUT_REPLY_SERVICE.md`。
14. `spec/SERVICE/PRESET_COMMAND_SERVICE.md`。
15. `spec/API/REPLY_TARGETS.md`。
16. `spec/INFRA/TERMINAL.md`。
17. 相关系统、流程、外部、内部、错误、测试、决策、索引和服务文档更新。

已验证：

1. `cargo test --manifest-path src-tauri/Cargo.toml`，110 个 Rust 单元测试通过。
2. `./node_modules/.bin/vitest run`，3 个前端测试文件通过，14 个测试通过。
3. `./node_modules/.bin/tsc --noEmit`。
4. `./node_modules/.bin/eslint .`。
5. `node scripts/check-architecture.mjs`。
6. `./node_modules/.bin/vite build`。

已知环境说明：

1. 直接运行 `pnpm test` 被 pnpm store 版本不一致阻塞，未进入测试本身。
2. 本轮使用本地 `node_modules/.bin` 中的 Vitest、TypeScript、ESLint 和 Vite 完成前端验证。
3. Windows 部分按本轮要求不做本机验证。
4. 真实终端跳回和真实新对话创建尚未做人工闭环验证，不声明已支持。

### 15.7 阶段 6 记录


已交付：

2. 独立历史详情 service、port、adapter 缓存和 Tauri command 已删除。
3. 完成或失败 session 默认单行，用户点击展开后才显示 follow-up 输入区。
4. follow-up 第二行只保留快捷输入和单行输入区。
5. follow-up 展开状态保持为前端轻量集合，提交成功后清理。
6. 相关系统、流程、外部、内部、错误、测试、决策和索引文档更新。

已验证：


未覆盖：

2. 视觉截图检查长文本布局未执行。
3. Windows 本机验证未执行。

### 15.8 阶段 7 记录

状态：扩展模式 UI、设置页、通知计划服务和相关文档已建立。

已交付：

1. 扩展模式工作台默认布局。
2. session 等待、运行和总数统计。
3. mock 与 Codex CLI session 合并后的等待优先和更新时间倒序排序。
4. session 列表动作标签和不支持动作隐藏规则。
5. session detail 用量展示开关、快捷回复开关和 Enter 发送开关。
6. 长摘要、长路径和长命令布局约束。
7. General、Display、Agents、Replies、Presets、Terminal 和 Advanced 设置页。
8. 设置模型、默认设置、配置损坏默认化和 JSON 设置文件 adapter。
9. 浏览器 fallback 设置结构校验。
10. 通知计划服务、当前 session 抑制、短时间重复通知合并和点击定位动作。
11. 记录型通知 adapter。
12. `spec/SERVICE/SETTINGS_SERVICE.md`。
13. `spec/SERVICE/NOTIFICATION_SERVICE.md`。
14. `spec/INFRA/UI_RUNTIME.md`。
15. 相关系统、流程、外部、内部、错误、测试、决策、索引和 Domain 错误文档更新。

已验证：

1. `./node_modules/.bin/vitest run`，4 个前端测试文件通过，25 个测试通过。
2. `./node_modules/.bin/tsc --noEmit`。
3. `./node_modules/.bin/eslint .`。
4. `node scripts/check-architecture.mjs`。
5. `./node_modules/.bin/vite build`。
6. `cargo test --manifest-path src-tauri/Cargo.toml`，125 个 Rust 单元测试通过。
7. `cargo fmt --manifest-path src-tauri/Cargo.toml`。
8. `./node_modules/.bin/prettier --check package.json pnpm-workspace.yaml index.html eslint.config.js tsconfig.json vite.config.ts "src/**/*.{ts,tsx,css}" "scripts/**/*.mjs" "spec/**/*.md" ".github/**/*.yml"`。
9. `rg '```|\||mermaid' spec` 无命中。
10. `rg '后续完善|灵活处理|按需扩展' spec` 无命中。
11. `find spec -maxdepth 3 -type f | sort` 已确认新增文档存在。

已知环境说明：

1. Windows 部分按本轮要求不做本机验证。
2. 真实 Mac 和 Windows 系统通知未接入，本轮只声明通知计划服务和记录型 adapter。
3. 项目当前未建立 Playwright 依赖和截图验证基础，本轮未执行 Playwright 验证。

独立 reviewer 反馈处理：

1. 修复当前选中 session 从刷新结果中消失后不会自动重选的问题，并补充回归测试。
2. 修复设置保存旧响应可能覆盖新 UI 状态的问题，并补充版本判断测试。
3. 将当前未接入读取链路的 Codex APP、Claude Code CLI 和 Claude Code APP 开关标记为禁用，并同步文档事实。

### 15.9 阶段 8 记录

状态：配置、安全、性能和发布质量的自动化基础设施已建立，跨平台人工验收尚未完成。

已交付：

1. JSON 设置文件默认路径模型。
2. 配置缺字段默认化。
3. 配置未知字段丢弃。
4. 配置损坏降级。
5. 同目录临时文件写入和替换。
6. 临时文件写入失败不覆盖旧配置。
7. 并发保存临时文件隔离。
8. hook 安装预览。
9. Codex hook 写入和卸载。
10. Claude hook 写入和卸载。
11. 已存在配置备份和卸载恢复。
12. 安装 manifest 写入和删除。
13. 重复安装替换、混合 group 保留用户 handler 和重复 agent 去重。
14. 安装失败回滚。
15. 日志敏感字段脱敏、长文本截断和中文业务事件名。
16. spec 文档质量门禁脚本。
17. 性能预算静态场景脚本。
18. `spec/INFRA/HOOK_INSTALL.md`。
19. `spec/INFRA/RELEASE_QUALITY.md`。
20. 相关系统、流程、外部、内部、错误、测试、决策和索引文档更新。

已验证：

1. `src-tauri/src/adapters/config_file/mod.rs` 中设置文件缺失、读写、损坏 JSON、缺字段默认化、未知字段丢弃、原子写失败和并发保存测试。
2. `src-tauri/src/adapters/hook_install/mod.rs` 中安装预览、写入、重复安装、备份恢复、失败回滚、manifest 删除和卸载测试。
3. `src-tauri/src/adapters/log_sanitizer/mod.rs` 中敏感字段脱敏、长文本截断和中文业务事件名测试。
4. `scripts/check-spec-docs.mjs` 已作为 `pnpm spec:check` 入口。
5. `scripts/check-performance-budget.mjs` 已作为 `pnpm performance:check` 入口。
6. `spec/TEST.md` 已登记阶段 8 配置、hook 安装、日志脱敏、spec 门禁和性能预算测试入口。

未覆盖：

1. Windows 本机验证未执行。
2. 真实 Codex 和 Claude 配置人工安装、卸载、trust review 验证未执行。
3. 10 分钟空闲 CPU 人工采样未执行。
4. Mac 和 Windows 全量发布验收矩阵未执行。

### 15.10 剩余功能补充记录

状态：设置页 hook 安装入口和 panel 持久化入口已建立，Windows 验证按要求跳过。

已交付：

1. 设置模型新增 Panel 分组，保存收缩状态、窗口位置和窗口尺寸。
2. panel 收缩状态从设置初始化，并在用户切换时通过局部 command 持久化。
3. Tauri 环境读取设置后尝试恢复窗口位置和尺寸。
4. Tauri 环境监听窗口移动和尺寸变化，并通过局部 command 保存窗口几何。
5. 设置页新增 Hook Install 分组。
6. Hook Install 分组支持 Codex CLI hook 和 Claude CLI hook 选择。
7. Hook Install 分组支持预览将修改文件、备份文件和 manifest 路径。
8. Hook Install 分组支持显式安装和卸载。
9. hook helper 默认路径使用当前应用同目录下的 `builder-panel-hook`。
10. `BUILDER_PANEL_HOOK_PATH` 可覆盖 hook helper 默认路径。
11. 前端 hook 安装 API 和 panel 窗口状态 API 已建立。
12. 相关 spec 文档已同步。

已验证：

1. `./node_modules/.bin/vitest run src/api/settingsApi.test.ts src/views/BuilderPanelApp.test.ts src/stores/panelProbeStore.test.ts`，3 个前端测试文件通过，17 个测试通过。
2. `./node_modules/.bin/tsc --noEmit`。
3. `cargo test --manifest-path src-tauri/Cargo.toml settings_service -- --nocapture`，4 个 settings service 相关测试通过。
4. `cargo test --manifest-path src-tauri/Cargo.toml hook_install -- --nocapture`，10 个 hook install 相关测试通过。
5. `./node_modules/.bin/vitest run`，4 个前端测试文件通过，28 个测试通过。
6. `./node_modules/.bin/eslint .`。
7. `node scripts/check-architecture.mjs`。
8. `node scripts/check-spec-docs.mjs`。
9. `./node_modules/.bin/vite build`。
10. `cargo test --manifest-path src-tauri/Cargo.toml`，142 个 Rust 测试通过。
11. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`。

独立 reviewer 反馈处理：

1. 修复 hook 安装可跳过预览直接写第三方配置的问题；安装现在必须先生成当前选择对应的预览，并补充回归测试。
2. 修复窗口移动和尺寸变化在同一 debounce 窗口内只保存最后一个局部字段的问题；现在会合并保存位置和尺寸，并补充回归测试。

未覆盖：

1. Windows 本机验证按本轮要求跳过。
2. Mac 原生窗口拖动、焦点和重启恢复人工验证尚未执行。
3. 真实 Codex 和 Claude 配置人工安装、卸载、trust review 验证尚未执行。
4. 真实系统通知、开机启动、Claude Code 真实闭环和 Codex APP 深度能力未完成。
5. 全量 Prettier 检查仍受本轮未修改的 `scripts/check-spec-docs.mjs` 和 `tech.md` 既有格式问题阻塞。
