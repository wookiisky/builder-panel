# Agent Event

## 职责

Agent Event 是 adapter 清洗外部 agent payload 后进入 Domain 的统一事件契约。

Agent Event 负责表达会话启动、活动更新、审批、回答、完成、失败、失联、能力、用量和跳回目标变化。

Agent Event 不保存第三方原始 payload，不复用参考项目 Swift schema，不承担外部协议兼容逻辑。

## 代码入口

`src-tauri/src/domain/agent_event.rs` 定义事件枚举和事件数据。

`src-tauri/src/domain/agent_session.rs` 定义事件携带的 session identity。

`src-tauri/src/domain/agent_interaction.rs` 定义审批和回答事件携带的交互模型。

`src-tauri/src/domain/session_state.rs` 定义事件如何改变会话状态。

## 事件边界

每个事件必须携带 `SessionKey`。

外部 Codex、Claude Code 或 hook payload 必须先在 adapter 边界清洗。

事件内不得携带裸 JSON、未验证字段或第三方协议原始对象。

事件可以序列化和反序列化，供后续 bridge、测试 fixture 和 API 边界使用。

## 当前事件族

`SessionStarted` 表示新会话或恢复会话。

`ActivityUpdated` 表示活动摘要更新。

`ApprovalRequested` 表示需要用户审批。

`AnswerRequested` 表示需要用户选择或文本回复。

`TurnCompleted` 表示当前 turn 完成。

`Failed` 表示归一后的失败。

`Detached` 表示会话失联。

`CapabilitiesUpdated` 表示能力矩阵变化。

`UsageUpdated` 表示用量快照变化。

`JumpTargetUpdated` 表示跳回目标变化。

## 相关测试

`src-tauri/src/domain/agent_event.rs` 覆盖事件 JSON 序列化、反序列化和不包含原始 payload 的断言。

`src-tauri/src/domain/session_state.rs` 覆盖事件进入 reducer 后的状态变化。
