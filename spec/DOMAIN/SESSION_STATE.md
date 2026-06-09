# Session State

## 职责

Session State 是 agent 会话状态的领域事实来源。

Session State 负责按 `SessionKey` 保存会话、应用归一事件、管理 pending interaction、维护排序规则。

Session State 不负责通知、配置读写、bridge response、Tauri command 或 UI 渲染。

## 代码入口

`src-tauri/src/domain/agent_session.rs` 定义会话身份、状态、能力和跳回目标。

`src-tauri/src/domain/agent_interaction.rs` 定义 pending interaction 和回复目标。

`src-tauri/src/domain/session_state.rs` 定义 reducer 和排序规则。

`src-tauri/src/domain/view_model.rs` 定义 UI view model 纯转换。

## 会话唯一键

`SessionKey` 由 `AgentKind`、`ProjectId` 和 `ConversationId` 共同组成。

同一 agent 在不同项目运行时不得合并。

同一项目中的不同对话不得合并。

缺少稳定字段时，adapter 必须在进入 Domain 前生成当前运行期不冲突的本地临时 ID。

## 状态语义

`Running` 表示 agent 正在工作。

`WaitingForApproval` 表示存在待处理审批。

`WaitingForAnswer` 表示存在待处理选项或文本回复。

`Completed` 表示当前 turn 完成。

`Failed` 表示 agent 或本地链路失败。

`Detached` 表示会话不可见或不可控，但历史状态不删除。

## reducer 规则

`SessionStarted` 创建或更新 session，并保留已有 pending interaction。

`SessionStarted` 只有携带摘要时才更新 session 摘要；空摘要不得清空已有原始摘要。

`SessionStarted` 只有携带标题时才更新 session 标题；空标题不得清空已有真实标题。

没有 pending interaction 时，`SessionStarted` 将旧的完成、失败或失联状态恢复为运行中。

非 `SessionStarted` 的实时事件可以创建占位 session，用于纳入 APP 启动后仍继续运行并发出事件的任务。

占位 session 的 agent 能力和跳回目标由对应 adapter 在事件进入 reducer 前或同批事件中补齐。

`ActivityUpdated` 更新摘要；已有 pending interaction 时不覆盖等待状态。

`UserMessageUpdated` 使用用户输入原文更新摘要；已有 pending interaction 时不覆盖等待状态。

`ApprovalRequested` 设置审批等待状态，并替换旧 pending。

`AnswerRequested` 设置回答等待状态，并替换旧 pending。

审批和回答事件进入 reducer 时，pending interaction 的 `SessionKey` 以外层事件的 `SessionKey` 为准。

`InteractionCompleted` 清理 pending interaction，并保持或恢复运行状态，不表示 turn 完成。

`TurnCompleted` 设置完成状态，并清理 pending interaction。

`TurnCompleted` 只有携带摘要时才更新 session 摘要；空摘要不得生成完成兜底文案。

`Failed` 设置失败状态，记录错误，并清理 pending interaction。

`Detached` 设置失联状态，清理 pending interaction，不删除 session。

`UsageUpdated` 只更新用量。

`CapabilitiesUpdated` 只更新能力。

`JumpTargetUpdated` 只更新跳回目标。

## View Model

Session 列表 view model 暴露项目标签、thread 标签、对话标签、状态、摘要、更新时间、用量、动作和行内交互。

Thread 标签优先来自 `AgentSession.title`，最长展示 10 个字符。


## 排序规则

Session 列表按首次捕捉顺序保持稳定。

新捕捉到的 session 排在已捕捉 session 前面。

已捕捉 session 不因状态、摘要或更新时间变化重排。

捕捉序号相同的异常情况按 `SessionKey` 稳定兜底排序。

## 相关测试

`src-tauri/src/domain/agent_session.rs` 覆盖 session key 和 capability 基础断言。

`src-tauri/src/domain/agent_interaction.rs` 覆盖 pending interaction 访问规则。

`src-tauri/src/domain/session_state.rs` 覆盖 reducer 分支、多项目多对话隔离、pending 清理和排序。

`src-tauri/src/domain/view_model.rs` 覆盖 capability 到 UI action、截断策略和基础展示映射。
