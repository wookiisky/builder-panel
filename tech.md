# Builder Panel 技术方案

## 1. 结论

Builder Panel 首版采用 Tauri + Rust + TypeScript + React 构建跨平台常驻桌面控制面板。系统以 Rust 侧 Domain 和 app-service 为核心，React 只负责展示和交互，所有 agent 协议、系统 API、桥接通信和终端能力都隔离在 adapter 层。

本方案借鉴 `open-vibe-island/` 的事件归一、纯 reducer、hook helper、本地 bridge、fail-open、pending interaction 和测试思路；但不移植 SwiftUI/AppKit、notch UI、macOS-only AppleScript，也不复用其 Swift 类型或原始协议字段。

首版只实现扩展模式。未来 mini 模式只在类型和布局边界预留，不提供入口，不进入验收。

## 2. 设计原则

### 2.1 架构原则

1. Domain 层保持纯粹，不依赖 Tauri、React、文件系统、网络、系统通知、终端和第三方 agent 协议。
2. 第三方 payload 在 adapter 边界完成校验和清洗，核心逻辑不接收裸 JSON、`Any` 或未验证字段。
3. 状态变更集中通过 reducer 和 app-service 用例完成，避免 UI 层直接拼装业务状态。
4. 所有关键状态使用强类型表达，禁止魔法字符串散落在 UI 或 adapter 中。
5. 回写能力、跳回能力、审批能力和 follow-up 能力都显式建模，UI 只展示当前 session 真实支持的动作。

### 2.2 产品边界

2. 默认不持久化 transcript 全文。
3. Windows 首版不支持 WSL，不承诺向任意已有终端注入输入。
4. Claude Code APP 在未验证公开本地协议前，只承诺只读状态、跳回和有限回写。
5. 用量数字只展示已验证来源提供的数据，不可用时展示 `--` 或隐藏。

## 3. `open-vibe-island/` 借鉴与改造

### 3.1 已确认参考对象

用户提到的 `open-vibe-land` 在当前仓库中不存在。当前可用参考项目为：

1. `open-vibe-island/docs/architecture.md`
2. `open-vibe-island/docs/hooks.md`
3. `open-vibe-island/Sources/OpenIslandCore/AgentEvent.swift`
4. `open-vibe-island/Sources/OpenIslandCore/SessionState.swift`
5. `open-vibe-island/Sources/OpenIslandCore/BridgeTransport.swift`
6. `open-vibe-island/Sources/OpenIslandHooks/OpenIslandHooksCLI.swift`
7. `open-vibe-island/Tests/OpenIslandCoreTests/SessionStateTests.swift`

本方案中的“参考项目”均指 `open-vibe-island/`。

### 3.2 可借鉴内容

1. 统一事件模型  
   `open-vibe-island` 使用 `AgentEvent` 承载 session start、activity update、permission request、question asked、completed、jump target updated 等事件。Builder Panel 采用同样的事件归一思想，但重新定义 Rust schema。

2. 纯 reducer  
   `SessionState.apply` 是参考项目的核心状态入口。Builder Panel 将其映射为 Rust 纯函数式 reducer：输入旧状态和 `AgentEvent`，输出新状态，不执行 IO。

3. hook helper + local bridge  
   参考项目由 hook CLI 读取 stdin payload，通过本地 socket 转交 APP，再按需 stdout directive 回写 agent。Builder Panel 延续该链路，但抽象为 Mac Unix domain socket + Windows Named Pipe。

4. fail-open  
   bridge 不可用、APP 不响应、payload 非关键错误时，hook helper 不输出阻塞 directive，让 agent 继续运行。

5. pending interaction  
   审批、选项、开放性问题都收敛为 pending interaction。UI 处理后通过 app-service 找到对应 pending，再由 reply/approval port 回写。

6. 跳回与文本回写分离  
   参考项目区分 terminal jump 和 text sender。Builder Panel 映射为 `JumpTargetPort` 与 `ReplySenderPort`，避免“能跳回”被误认为“能可靠输入”。

7. 测试重点  
   借鉴 reducer、hook payload、bridge codec、pending cleanup、fail-open、jump target merge 等测试方向。

### 3.3 明确不照搬内容

1. 不使用 SwiftUI/AppKit 作为主架构。
2. 不采用 notch / island UI。
3. 不把 AppleScript 作为跨平台核心能力。
4. 不复用 Swift 类型、字段名和编码结构。
5. 不沿用参考项目的大量 agent 支持范围。
6. 不假设参考项目验证过的 Codex 或 Claude Code 字段仍然兼容当前版本。
7. 不用 macOS Unix socket 结论覆盖 Windows Named Pipe。

### 3.4 映射关系

| `open-vibe-island` 概念     | Builder Panel 概念                  | 处理方式                        |
| --------------------------- | ----------------------------------- | ------------------------------- |
| `OpenIslandCore.AgentEvent` | `domain::AgentEvent`                | 借鉴事件归一思想，Rust 重新定义 |
| `SessionState.apply`        | `domain::SessionState::apply_event` | 纯 reducer，无 IO               |
| `OpenIslandHooks`           | `builder-panel-hook`                | Rust CLI，跨平台                |
| Unix socket bridge          | `LocalBridgePort`                   | Mac UDS，Windows Named Pipe     |
| pending approval / question | `AgentInteraction`                  | 统一审批、选项、文本回复        |
| terminal jump               | `JumpTargetPort`                    | 只负责聚焦或跳回                |
| text sender                 | `ReplySenderPort`                   | 只负责可靠回写或复制降级        |
| `CodexAppServer`            | `CodexAppAdapter`                   | schema 重验后接入               |

## 4. 总体架构

### 4.1 分层

系统分为五层：

1. `domain`  
   纯模型、事件、状态 reducer、排序规则、能力规则、错误类型。

2. `ports`

3. `adapters`  
   对接 Codex APP、Codex CLI、Claude Code APP、Claude Code CLI、终端、系统通知、配置文件、IPC、剪贴板、托管进程。

4. `app-services`

5. `ui`  
   React panel、session 列表、session detail、设置页和行内 follow-up 输入区。UI 不直接处理 agent 协议。

### 4.2 依赖方向

依赖方向固定为：

1. `ui` 依赖 `app-services` 暴露的 Tauri command 和事件订阅。
2. `app-services` 依赖 `domain` 和 `ports`。
3. `adapters` 实现 `ports`。
4. `domain` 不依赖任何上层模块。

禁止反向依赖。尤其禁止 adapter payload、Tauri 类型、React view model 进入 domain。

### 4.3 进程形态

首版包含三个运行单元：

1. Builder Panel APP  
   Tauri 桌面应用，内含 Rust backend 和 React frontend。

2. `builder-panel-hook`  
   被 Codex CLI / Claude Code CLI hooks 调用的轻量 CLI。

3. 托管 agent 子进程

## 5. Rust 工程模块

推荐 Rust crate / module 结构：

```text
src-tauri/src/
  domain/
    agent_event.rs
    agent_session.rs
    agent_interaction.rs
    session_state.rs
    usage.rs
    app_error.rs
  ports/
    agent_adapter_port.rs
    local_bridge_port.rs
    reply_sender_port.rs
    jump_target_port.rs
    notification_port.rs
    config_store_port.rs
    managed_process_port.rs
  adapters/
    bridge/
    codex_app/
    codex_cli_hook/
    claude_app/
    claude_cli_hook/
    terminal/
    notification/
    config_file/
  services/
    session_service.rs
    interaction_service.rs
    reply_service.rs
    preset_command_service.rs
    shortcut_reply_service.rs
    notification_service.rs
    settings_service.rs
  tauri_api/
    commands.rs
    events.rs
```

`builder-panel-hook` 建议作为独立 binary：

```text
src-tauri/src/bin/builder-panel-hook.rs
```

如果后续 hook helper 复杂度上升，可拆成 workspace crate。

## 6. Domain 设计

### 6.1 Agent 类型

`AgentKind` 只覆盖首版四类入口：

```rust
/// Agent 来源类型。
pub enum AgentKind {
    /// Codex 桌面 APP。
    CodexApp,
    /// Codex CLI。
    CodexCli,
    /// Claude Code 桌面 APP。
    ClaudeCodeApp,
    /// Claude Code CLI。
    ClaudeCodeCli,
}
```

不在首版加入 Gemini、Cursor、OpenCode 等参考项目已有 agent，避免范围膨胀。

### 6.2 Session 唯一标识

会话唯一键由三段组成：

```rust
/// 会话唯一键，由 agent、项目和对话共同确定。
pub struct SessionKey {
    /// Agent 类型。
    pub agent_kind: AgentKind,
    /// 项目稳定标识，优先由 adapter 从工作目录或官方字段生成。
    pub project_id: ProjectId,
    /// 对话稳定标识，优先来自 thread/session/conversation id。
    pub conversation_id: ConversationId,
}
```

规则：

1. 同一 agent 在不同项目中运行，必须生成不同 `SessionKey`。
2. 同一项目中不同对话，必须生成不同 `SessionKey`。
3. 缺少稳定字段时，adapter 生成本地临时 ID，但必须在当前运行期不冲突。
4. 临时 ID 必须带来源标签，避免被误认为官方稳定 ID。

### 6.3 Session 状态

```rust
/// 会话运行状态。
pub enum SessionStatus {
    /// Agent 正在工作。
    Running,
    /// 等待用户审批。
    WaitingForApproval,
    /// 等待用户选择或文本回复。
    WaitingForAnswer,
    /// 当前 turn 完成。
    Completed,
    /// Agent 或 bridge 出错。
    Failed,
    /// 会话失联或本地进程不可见。
    Detached,
}
```

`Detached` 不删除历史状态，只表达当前不可见或不可控。是否展示由 view model 排序和过滤规则决定。

### 6.4 Session 能力

```rust
/// 当前会话可执行能力。
pub struct SessionCapabilities {
    /// 是否可跳回 agent 所在 APP 或终端。
    pub can_jump: bool,
    /// 是否可发送开放性回复。
    pub can_send_reply: bool,
    /// 是否可处理审批。
    pub can_resolve_approval: bool,
    /// 是否可创建后续 turn。
    pub can_create_followup_turn: bool,
}
```

能力来自 adapter 和 managed process，不由 UI 推断。

### 6.5 用量模型

```rust
/// 已验证用量数字。
pub struct VerifiedUsageValue {
    /// 展示用数字。
    pub value: f64,
    /// 可选单位，例如 percent、tokens、requests。
    pub unit: Option<String>,
    /// 数据来源标签，例如 Codex /status、Codex app-server、Claude Admin API。
    pub source_label: String,
    /// 来源更新时间。
    pub updated_at: Option<DateTime<Utc>>,
}

/// 用量可用性。
pub enum UsageValue {
    /// 已验证可展示。
    Verified(VerifiedUsageValue),
    /// 来源不可用或未验证。
    Unavailable,
}

/// 会话或账号上下文用量。
pub struct UsageSnapshot {
    /// 5 小时窗口用量。
    pub usage_5h: UsageValue,
    /// 周用量。
    pub usage_weekly: UsageValue,
}
```

规则：

1. 不用 `0`、`-1` 或空字符串表达不可用。
2. 不同 agent 单位不强行换算。
3. UI 展示数字、来源标签和可用单位。
4. 单位不可确认时只展示数字。

### 6.6 交互模型

```rust
/// Agent 正在等待用户处理的交互。
pub enum AgentInteraction {
    /// 审批请求。
    Approval(ApprovalInteraction),
    /// 单选或多选问题。
    Choice(ChoiceInteraction),
    /// 开放性文本回复。
    TextReply(TextReplyInteraction),
}
```

`AgentInteraction` 必须包含：

1. `interaction_id`
2. `session_key`
3. `created_at`
4. `expires_at`
5. `reply_target`
6. `status`
7. agent 原始请求的已清洗摘要

pending 状态由 reducer 管理。用户处理成功后清理 pending；回写失败时保留 pending 或草稿，并展示错误。

### 6.7 ReplyTarget

```rust
/// 回复目标。
pub enum ReplyTarget {
    /// 通过结构化 RPC 回写。
    StructuredRpc(StructuredRpcTarget),
    /// 通过 hook stdout directive 回写。
    HookDirective(HookDirectiveTarget),
    /// 通过托管进程 stdin 回写。
    ManagedProcessStdin(ManagedProcessTarget),
    /// 通过受控终端输入。
    ControlledTerminal(ControlledTerminalTarget),
    /// 不支持自动回写，只能复制。
    ClipboardOnly(ClipboardFallbackTarget),
}
```

发送前必须检查：

1. session 仍存在。
2. `can_send_reply` 或对应审批能力仍为 true。
3. `reply_target` 没有过期。
4. 内容非空。
5. 内容不超过配置最大长度。

### 6.8 AgentEvent

Builder Panel 重新定义事件，不复用参考项目 Swift schema：

```rust
/// 归一后的 agent 事件。
pub enum AgentEvent {
    /// 新会话或恢复会话。
    SessionStarted(SessionStartedEvent),
    /// 活动摘要或运行状态更新。
    ActivityUpdated(ActivityUpdatedEvent),
    /// 审批请求。
    ApprovalRequested(ApprovalRequestedEvent),
    /// 问题或选项请求。
    AnswerRequested(AnswerRequestedEvent),
    /// 当前 turn 完成。
    TurnCompleted(TurnCompletedEvent),
    /// 失败。
    Failed(FailedEvent),
    /// 会话失联。
    Detached(DetachedEvent),
    /// 会话能力更新。
    CapabilitiesUpdated(CapabilitiesUpdatedEvent),
    /// 用量更新。
    UsageUpdated(UsageUpdatedEvent),
    /// 跳回目标更新。
    JumpTargetUpdated(JumpTargetUpdatedEvent),
}
```

事件必须携带 `SessionKey`，避免不同项目或对话被合并。

### 6.9 SessionState reducer

`SessionState` 是 session 状态唯一事实来源：

```rust
/// 所有会话状态。
pub struct SessionState {
    /// 按唯一键存储的会话。
    pub sessions: BTreeMap<SessionKey, AgentSession>,
}
```

核心规则：

1. `SessionStarted` 创建或更新 session，但保留已有草稿和 pending interaction。
2. `ActivityUpdated` 不能覆盖已有 pending approval 或 pending answer。
3. `ApprovalRequested` 设置 `WaitingForApproval`，清理 question pending。
4. `AnswerRequested` 设置 `WaitingForAnswer`，清理 approval pending。
5. `TurnCompleted` 设置 `Completed`，清理 pending interaction。
6. `Failed` 设置 `Failed`，保留错误摘要，清理不可继续的 pending。
7. `Detached` 设置 `Detached`，不删除 session。
8. `UsageUpdated` 只更新用量，不改变运行状态。
9. `CapabilitiesUpdated` 只更新能力，不改变 pending。
10. reducer 不调用通知、不写配置、不发 bridge response。

### 6.10 排序规则

view model 层排序：

1. 列表先按展示分组排序，再按块级捕捉锚点排序。
2. `Running`、`WaitingForApproval` 和 `WaitingForAnswer` 属于未完成分组，展示在顶部。
3. `Completed`、`Failed` 和 `Detached` 属于已结束分组，展示在未完成分组下方。
4. 存在有效父子关系时，parent-child 展示块不拆散；块内任一 session 未完成时，整个块进入未完成分组。
5. 块内存在未完成 session 时，块级捕捉锚点取块内最新未完成 session 的捕捉序号；否则取块内最新 session 的捕捉序号。
6. 摘要、标题或 `updated_at` 变化不改变捕捉序；状态变化只影响展示分组和块级捕捉锚点。
7. 块级捕捉锚点相同的异常情况按 root `SessionKey` 稳定兜底排序。

排序规则写在 domain 或 app-service 的纯函数中，并有单元测试。

## 7. Ports 设计

### 7.1 AgentAdapterPort

职责：

1. 发现会话。
2. 订阅 agent 状态或 hook 事件。
3. 转换为 `AgentEvent`。
4. 输出能力矩阵和用量来源状态。

该 port 不负责 UI 展示。

### 7.2 LocalBridgePort

职责：

1. 启动本地 bridge server。
2. 接收 hook helper 命令。
3. 返回 hook directive 或 ack。
4. 支持 Mac UDS 与 Windows Named Pipe。

### 7.3 ReplySenderPort

职责：

1. 根据 `ReplyTarget` 发送文本、选项或审批结果。
2. 返回明确结果。
3. 失败时返回 `ReplySendFailed` 或 `UnsupportedReplyTarget`。
4. 不负责跳回。

### 7.4 JumpTargetPort

职责：

1. 聚焦 APP、终端窗口、tmux pane 或托管会话。
2. 返回跳回结果。
3. 不发送文本。

### 7.6 ConfigStorePort

职责：

1. 读取配置文件。
2. 校验配置。
3. 写入临时文件。
4. 原子替换旧配置。
5. 配置损坏时返回默认值和告警。

## 8. Bridge 协议

### 8.1 传输

1. Mac 使用 Unix domain socket。
2. Windows 使用 Named Pipe。
3. 编码使用 newline-delimited JSON。
4. 每行一个 envelope。
5. 每个 request 必须有 `request_id`。
6. codec 必须在 Mac 和 Windows 使用同一套 schema 测试。

### 8.2 Command envelope

```json
{
  "schema_version": 1,
  "command_type": "process_agent_hook",
  "request_id": "req_01",
  "payload": {
    "agent_kind": "codex_cli",
    "hook_event_name": "PermissionRequest",
    "validated_payload": {}
  }
}
```

`payload` 不能直接透传未校验第三方 JSON。hook helper 需要先识别来源，完成最低限度结构校验，再发送本项目 bridge command。

### 8.3 Response envelope

```json
{
  "schema_version": 1,
  "request_id": "req_01",
  "result_type": "directive",
  "payload": {
    "directive_kind": "allow"
  },
  "error": null
}
```

错误响应：

```json
{
  "schema_version": 1,
  "request_id": "req_01",
  "result_type": "error",
  "payload": null,
  "error": {
    "code": "BridgeUnavailable",
    "message": "本地桥接不可用"
  }
}
```

### 8.4 Fail-open 规则

hook helper 遇到以下情况必须 fail-open：

1. bridge socket / pipe 不存在。
2. bridge 连接超时。
3. APP 无响应。
4. 非阻塞 hook payload 无法发送。
5. 无法编码有效 directive。

fail-open 时：

1. 不向 stdout 输出阻塞 directive。
2. 可向 stderr 输出简短诊断。
3. 退出码优先为 0，避免 agent 误判 hook 阻塞。
4. 不记录敏感 payload 全文。

## 9. Hook Helper 方案

### 9.1 CLI 入口

命令形态：

```text
builder-panel-hook --source codex
builder-panel-hook --source claude
```

首版只支持：

1. `codex`
2. `claude`

### 9.2 执行流程

1. 读取 stdin。
2. 解析 JSON。
3. 根据 `--source` 选择 payload validator。
4. 校验 hook 事件名和关键字段。
5. 生成内部 `BridgeCommand`。
6. 连接本地 bridge。
7. 非阻塞事件等待短超时。
8. 审批类阻塞事件按 agent 协议等待较长超时。
9. 收到 directive 后编码 stdout。
10. 任一不可恢复错误走 fail-open。

### 9.3 Codex CLI hook

首版监听：

1. `SessionStart`
2. `UserPromptSubmit`
3. `PermissionRequest`
4. `Stop`

可解析但不默认启用：

1. `PreToolUse`
2. `PostToolUse`

原因：参考项目也将高噪声工具生命周期事件设为可选。Builder Panel 首版以稳定低噪声为优先。

### 9.4 Claude Code CLI hook

首版监听：

1. `SessionStart`
2. `UserPromptSubmit`
3. `PreToolUse`
4. `PermissionRequest`
5. `Notification`
6. `Stop`
7. `SessionEnd`

自然语言问题首版只处理结构化来源，不做复杂 NLP 解析。

## 10. Agent Adapter 设计

### 10.1 Codex APP Adapter

优先级：

1. 官方 app-server 或本地状态协议。
2. app-server notification。
3. app-server RPC。
4. 只读降级。

职责：

1. 发现 Codex APP 会话。
2. 读取 thread、turn、等待标记。
3. 接收 notification 低延迟更新状态。
4. 若 RPC 支持，创建新对话和发送开放性回复。
5. 若不支持结构化回写，输出 `can_send_reply = false`。
6. 解析已验证的 5H 和周用量字段。

边界：

1. app-server schema 必须在实施前技术探针中重验。
2. adapter schema 由 Builder Panel 自定义。
3. 不把 app-server 原始 JSON 放入 domain。

### 10.2 Codex CLI Adapter

输入：

1. `builder-panel-hook --source codex`
2. 可选 `/status` 已验证输出
3. 可选托管进程 stdout 事件

输出：

1. `SessionStarted`
2. `ActivityUpdated`
3. `ApprovalRequested`
4. `TurnCompleted`
5. `UsageUpdated`
6. `JumpTargetUpdated`

回写：

1. 审批优先 hook stdout directive。
2. 开放性回复优先受控终端或托管进程 stdin。
3. 不可靠终端降级复制。

### 10.3 Claude Code APP Adapter

首版策略：

1. 如果存在已验证公开本地协议，按协议实现只读状态和有限回写。
2. 如果没有稳定协议，只展示进程级状态、通知和跳回能力。
3. 不承诺结构化审批回写。
4. 托管启动的 APP 相关会话可通过托管进程能力提供有限回复。

能力默认：

1. `can_jump` 按发现结果决定。
2. `can_send_reply` 默认 false，除非托管进程或已验证协议支持。
3. `can_resolve_approval` 默认 false。

### 10.4 Claude Code CLI Adapter

输入：

1. `builder-panel-hook --source claude`
2. 托管进程 stdout / stderr
3. 可选 Analytics / Usage 已验证来源

输出：

1. `SessionStarted`
2. `ActivityUpdated`
3. `ApprovalRequested`
4. `AnswerRequested`
5. `TurnCompleted`
6. `Failed`
7. `Detached`
8. `UsageUpdated`

回写：

1. `PermissionRequest` 使用 stdout directive。
2. `PreToolUse` 如进入审批，也使用对应 directive。
3. 开放性回复按 `reply_target` 走托管 stdin、受控终端或复制降级。

## 11. 用量展示方案

### 11.1 来源优先级

Codex：

1. `/status` 已验证字段。
2. app-server 可验证字段。
3. 企业 Analytics。

Claude Code：

1. CLI 可验证输出。
2. Admin / Analytics API。
3. Analytics 仅作为企业/Admin 可选数据源，不作为个人实时 5H 余量来源。

### 11.2 可用性状态

每个 adapter 输出：

1. `supported`：已验证可解析。
2. `degraded`：来源存在但权限、延迟或字段不完整。
3. `unsupported`：无已验证来源。
4. `unknown`：尚未完成探针。

UI 只对 `Verified` 展示数字。

### 11.3 UI 展示

扩展模式展示：

1. 5H 数字。
2. 周用量数字。
3. 来源标签。
4. 可选更新时间 tooltip。

不可用时：

1. 展示 `--`。
2. 或按设置隐藏。
3. 不展示虚构数字。

### 12.1 边界

首版不提供独立历史详情能力。

不保留独立历史详情 service、port、adapter 缓存或 Tauri command。

### 12.3 内存管理

1. 全局 UI 状态只保存轻量索引，不保存长文本全文。
2. 完成或失败 session 的 follow-up 展开状态只保存在前端轻量集合中，提交成功后清理。
3. follow-up 展开状态不进入后端能力模型。

### 12.4 查询能力

当前不提供独立历史详情查询能力，也不提供对应浮层。

## 13. App Service 设计

### 13.1 SessionService

职责：

1. 接收 adapter 事件。
2. 调用 reducer 更新 `SessionState`。
3. 生成 UI view model。
4. 管理选中 session。
5. 维护每个 session 的草稿引用。

### 13.2 InteractionService

职责：

1. 处理审批允许/拒绝。
2. 处理单选和多选。
3. 防重复提交。
4. 成功后清理 pending。
5. 失败后保留 pending，并返回可复制降级内容。

### 13.3 ReplyService

职责：

1. 校验开放性回复。
2. 根据 `ReplyTarget` 选择发送路径。
3. 发送成功后清空对应 session 草稿。
4. 发送失败后保留草稿。
5. 统一处理快捷回复。

### 13.4 ShortcutReplyService

职责：

1. 读取快捷回复配置。
2. 按 agent、项目、工作目录过滤。
3. 排序和禁用。
4. 点击后复用 `ReplyService`。
5. 失败后把内容填回输入框。

### 13.5 PresetCommandService

职责：

1. 管理预设命令。
2. 根据 agent 能力选择结构化创建、托管启动、终端输入或复制降级。
3. 自动发送回车必须由用户配置。
4. 回写失败不得重复自动发送。

### 13.6 NotificationService

职责：

1. turn 完成通知。
2. 等待审批通知。
3. 等待选择通知。
4. 失败通知。
5. 长时间运行阈值通知。
6. 同 session 短时间合并通知。
7. 当前查看 session 时抑制重复通知。
8. 点击通知后展开 panel 并定位 session。

## 14. UI 技术方案

### 14.1 前端状态

推荐 Zustand 或 Jotai。状态拆分：

1. `sessionListStore`：轻量 session view model。
2. `selectedSessionStore`：当前选中 session。
3. `draftStore`：按 session 保存草稿。
4. `settingsStore`：设置页表单状态。

长文本内容不得进入全局 store。

### 14.2 扩展模式 Panel

首版固定 `expanded`：

1. 左侧或上方展示 session 列表。
2. 详情区域展示当前 session。
3. 操作区展示审批、选项、回复输入、快捷回复。
4. 工具区展示跳回、复制等能力入口。
5. 展示等待数量、运行数量、状态动画。
6. 展示已验证 5H 和周用量。

`mini` 只在类型层预留，不提供 UI 切换。

### 14.3 Session 列表

每行展示：

1. agent 名称。
2. 项目名或工作目录。
3. 对话标题；缺少真实标题时显示未命名，不展示 ID。
4. 状态标签。
5. 摘要。
6. 更新时间。
7. 5H 用量。
8. 周用量。
9. 可用动作图标。

布局约束：

1. 行高稳定。
2. 长摘要截断或两行限制。
3. 长路径折叠。
4. 等待用户操作状态有更高视觉优先级。
5. 不展示不可执行的假按钮。

### 14.4 Session Detail

详情区包含：

1. 标题区：agent、项目、对话标题、状态。
2. 标识区：项目名、thread 标签。
3. 用量区：5H、周用量、来源。
4. 摘要区：最近状态或当前问题。
5. 执行信息区：运行信息和轻量状态动画。
6. 操作区：审批、选项、输入框、快捷回复。
7. 工具区：跳回、复制。

交互规则：

1. 只在 `can_send_reply` 为 true 时启用输入框。
2. `Enter` 发送，`Shift+Enter` 换行。
3. 发送失败不清空草稿。
4. 切换 session 保留各自草稿。
5. 多选提交前必须至少选择一项。
6. 长选项文本换行，不撑破按钮。

### 14.5 Follow-up 行内输入

包含：

1. 完成或失败 session 默认保持单行。
2. 可 follow-up 的完成或失败 session 右侧展示展开按钮。
3. 点击展开后展示第二行。
4. 第二行只包含快捷输入和单行输入区。
5. 提交成功后收起第二行并清理草稿。
6. 发送中状态。
7. 发送失败状态。

规则：

1. 完成或失败 session 默认不自动展示第二行。
2. 第二行不展示独立历史详情入口。
3. follow-up 提交成功后清理对应草稿和展开状态。

### 14.6 设置页

模块：

1. General：开机启动、通知、语言或基础偏好。
2. Display：panel 位置、大小、置顶、扩展布局、收缩行为。
3. Agents：四类 agent 接入状态和用量来源状态。
4. Replies：输入偏好、快捷回复组。
5. Presets：预设命令。
6. Terminal：终端 profile、托管启动策略。
7. Advanced：bridge、hook 安装和日志。

不提供 mini 模式切换入口。

## 15. Tauri 窗口与系统能力

### 15.1 Panel 行为

必须支持：

1. 始终置顶。
2. 可拖动。
3. 可收缩和展开。
4. 多显示器。
5. 位置、大小、收缩状态恢复。
6. 默认不抢焦点。
7. 点击输入框时临时获得焦点。
8. 输入框失焦后恢复轻量非激活浮窗行为。

Tauri 原生能力不足的地方，使用平台 adapter 隔离实现。

### 15.2 Mac

1. Unix domain socket。
2. 系统通知。
3. tmux / Ghostty 优先回写。
4. Terminal.app / iTerm2 优先跳回。
5. AppleScript 只能作为 Mac adapter 的局部跳回实现，不进入 domain。

### 15.3 Windows

1. Named Pipe。
2. 系统通知。
3. Windows Terminal 跳回优先。
4. PowerShell / cmd 托管子进程 stdin 回写。
5. 不支持 WSL。
6. 不承诺向任意已有终端注入输入。

## 16. 配置方案

### 16.1 路径

Mac：

```text
~/Library/Application Support/BuilderPanel/
```

Windows：

```text
%APPDATA%/BuilderPanel/
```

### 16.2 文件

首版使用 JSON：

```text
config.json
shortcuts.json
presets.json
terminal-profiles.json
hook-install-manifest.json
```

可以合并为单文件，但必须保持 schema 清晰。用户自定义项和运行态缓存不要混在一起。

### 16.3 读写规则

1. 读取时做 schema 校验。
2. 缺失字段使用显式默认值。
3. 非法字段不进入 domain。
4. 配置损坏时使用默认值，并在 UI 提示。
5. 写入时先写临时文件。
6. fsync 后原子替换。
7. 写入失败不得覆盖旧配置。

### 16.4 配置内容

必须覆盖：

1. panel 位置、大小、收缩状态。
2. 开机启动。
3. 通知开关。
4. agent 接入开关。
5. 快捷回复组。
6. 预设命令。
7. 终端 profile。
8. 输入框偏好。
9. 用量展示开关。
10. agent 用量来源状态。
11. 默认 UI 模式字段，首版固定 `expanded`。

## 17. 错误处理与日志

### 17.1 错误类型

Domain 显式建模：

1. `BridgeUnavailable`
2. `MalformedAgentPayload`
3. `UnsupportedReplyTarget`
4. `ReplySendFailed`
5. `ConfigLoadFailed`
6. `ConfigSaveFailed`
7. `AgentProtocolUnsupported`

错误对象包含：

1. 错误码。
2. 用户可读消息。
3. 可选技术细节。
4. 是否可重试。
5. 可选降级动作。

### 17.2 错误收敛

1. adapter 将外部错误转换为统一错误。
2. app-service 决定降级策略。
3. UI 展示用户可读消息和可操作按钮。
4. 禁止无意义的 catch-log-reraise。

### 17.3 日志

业务事件名使用中文：

1. `桥接服务启动`
2. `收到Agent事件`
3. `用户回复已发送`
4. `终端回写失败`
5. `配置保存失败`

日志规则：

1. 默认不记录敏感全文。
2. 调试模式记录全文前必须提示用户确认。
3. 日志包含 session key 的脱敏摘要。
4. 错误链路只记录一次，避免重复噪声。

## 18. 安全方案

### 18.1 本地数据

3. 复制动作必须由用户主动触发。
4. 默认只保存 session 摘要、状态、配置和用户自定义项。

### 18.2 Hook 安装

1. 安装前展示将修改的配置文件。
2. 修改第三方配置前备份。
3. 卸载必须可逆。
4. 不静默提权。
5. Codex trust review 这类外部审批不绕过，只提示用户完成。

### 18.3 命令安全

1. 预设命令必须由用户显式创建。
2. 自动发送回车必须由用户显式配置。
3. 高风险命令不自动生成。
4. 回写失败不得自动重复发送。

## 19. 性能方案

### 19.1 常驻性能

1. 状态更新以事件驱动为主。
2. 前端禁止高频轮询 agent 状态。
3. 常驻任务放在 Rust 侧。
4. panel 收缩后暂停非必要动画。
5. panel 不可见或收缩时降低刷新等级。

### 19.2 内存

2. 长文本不进入全局 UI 状态。
3. follow-up 展开状态不进入后端能力模型。

### 19.3 大列表

1. session 列表保持展示分组和捕捉锚点排序稳定。
2. 默认只渲染当前列表视图需要的轻量状态。
3. 长内容默认折叠或截断。
4. 1000 个 session 更新和 follow-up 展开集合操作仍应可用。

## 20. View Model 设计

### 20.1 SessionListItemViewModel

字段：

1. `session_key`
2. `agent_label`
3. `project_label`
4. `thread_label`
5. `conversation_label`
6. `status_label`
7. `status_kind`
8. `summary`
9. `updated_at_label`
10. `usage_5h_label`
11. `usage_weekly_label`
12. `usage_source_label`
13. `actions`

`actions` 由 capabilities 转换，不由组件自行判断。

`thread_label` 优先来自 session title，最长展示 10 个字符；缺少 title 时不回退展示 conversation id。

### 20.2 SessionDetailViewModel

字段：

1. `header`
2. `identity`
3. `usage`
4. `summary`
5. `execution_info`
6. `pending_interaction`
7. `reply_box`
8. `shortcut_replies`
9. `toolbar_actions`

字段：

1. `session_header`
2. `filters`
3. `items`
4. `loading_state`
5. `empty_state`
6. `receive_error`
7. `can_copy_filtered_result`
8. `can_jump_to_latest`

## 21. 测试方案

### 21.1 Domain 测试

必须覆盖：

1. session start 创建状态。
2. activity update 不覆盖 pending approval。
3. approval request 进入等待审批。
4. answer request 进入等待回复。
5. completed 清理 pending interaction。
6. detached 不删除历史状态。
7. 多 session 展示分组和捕捉锚点排序。
8. 不同项目会话不合并。
9. 同项目不同对话不合并。
10. usage unavailable 不生成虚假数字。

### 21.2 Adapter 测试

必须覆盖：

1. Codex hook payload 校验。
2. Claude hook payload 校验。
3. 非法 JSON 被拒绝。
4. hook response 编码正确。
5. Unix socket / Named Pipe codec schema 一致。
6. hook 事件转换为 `AgentEvent`。
7. app-server 事件转换为 `AgentEvent`。
8. 用量可用时解析为 `Verified`。
9. 用量不可用时输出 `Unavailable`。
10. adapter 生成稳定 project id 和 conversation id。

### 21.3 App Service 测试

必须覆盖：

1. 点击选项回写到正确 pending interaction。
2. 开放性回复选择正确 `ReplyTarget`。
3. 快捷回复复用开放性回复路径。
4. 新对话预设命令生成正确。
5. 回写失败降级复制。
6. 用量更新生成正确 view model。
7. 多项目多对话 view model 独立。

### 21.4 UI 测试

使用 Vitest、Testing Library 和 Playwright 覆盖：

1. 扩展模式基础布局。
2. 长选项文本不溢出。
3. 小屏幕按钮不重叠。
4. 不同状态展示正确动作。
5. 输入框发送、失败保留草稿。
6. 完成或失败 session 默认单行，点击展开后才展示 follow-up 输入区。
7. 第二行快捷输入、输入框和发送按钮不撑破布局。
8. 用量可用时展示 5H 与周用量。
9. 不展示 mini 模式切换入口。

### 21.5 E2E 测试

使用 mock agent 覆盖：

1. mock agent 开始运行。
2. mock agent 等待审批。
3. 用户点击允许。
4. mock agent 收到 directive。
5. mock agent 完成。
6. APP 展示完成并触发通知。
7. 用户输入开放性回复。
8. mock agent 收到文本。

### 21.6 性能测试

必须覆盖：

1. 空闲 10 分钟 CPU 接近 0。
2. 10 个 session 同时存在时操作流畅。
3. 连续 1000 条 mock agent 事件输入不丢失。
4. panel 收缩后动画和刷新降级。
5. 1000 个 session 更新和 follow-up 展开集合操作可用。

## 22. 文档要求

除本文件外，后续实现时应同步维护：

1. `docs/architecture.md`：实际架构和模块边界。
2. `docs/bridge-protocol.md`：bridge envelope、命令、响应、错误码。
3. `docs/agent-adapters.md`：四类 adapter 能力矩阵和降级策略。
4. `docs/hook-installation.md`：hook 安装、备份、卸载和 trust review。
5. `docs/security.md`：本地数据、敏感内容、日志和命令安全。

本次 `tech.md` 不包含实施计划和里程碑安排。
