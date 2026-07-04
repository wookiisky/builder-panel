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

Session 保存当前 turn 的开始时间和结束时间。

运行中和等待状态的 session 结束时间为空。

完成和失败状态的 session 记录结束时间。

## reducer 规则

`SessionStarted` 创建或更新 session，并保留已有 pending interaction。

`SessionStarted` 只有携带摘要时才更新 session 摘要；空摘要不得清空已有原始摘要。

`SessionStarted` 只有携带标题时才更新 session 标题；空标题不得清空已有真实标题。

没有 pending interaction 时，`SessionStarted` 将旧的完成、失败或失联状态恢复为运行中。

`SessionStarted` 从完成、失败或失联状态恢复为运行中时，会用事件时间重置当前 turn 开始时间，并清空结束时间。

非 `SessionStarted` 的实时事件可以创建占位 session，用于纳入 APP 启动后仍继续运行并发出事件的任务。

占位 session 的 agent 能力和跳回目标由对应 adapter 在事件进入 reducer 前或同批事件中补齐。

`ActivityUpdated` 更新摘要；已有 pending interaction 时不覆盖等待状态。

`UserMessageUpdated` 使用用户输入原文更新摘要；已有 pending interaction 时不覆盖等待状态。

`ActivityUpdated` 和 `UserMessageUpdated` 从完成、失败或失联状态恢复为运行中时，会用事件时间重置当前 turn 开始时间，并清空结束时间。

`ApprovalRequested` 设置审批等待状态，并替换旧 pending。

`AnswerRequested` 设置回答等待状态，并替换旧 pending。

审批和回答事件进入 reducer 时，pending interaction 的 `SessionKey` 以外层事件的 `SessionKey` 为准。

`InteractionCompleted` 清理 pending interaction，并保持或恢复运行状态，不表示 turn 完成。

`InteractionCompleted` 不重置当前 turn 开始时间。

`TurnCompleted` 设置完成状态，并清理 pending interaction。

`TurnCompleted` 用事件时间记录当前 turn 结束时间。

`TurnCompleted` 只有携带摘要时才更新 session 摘要；空摘要不得生成完成兜底文案。

`Failed` 设置失败状态，记录错误，并清理 pending interaction。

`Failed` 用事件时间记录当前 turn 结束时间。

`Detached` 设置失联状态，清理 pending interaction，不删除 session。

`UsageUpdated` 只更新用量。

`CapabilitiesUpdated` 只更新能力。

`JumpTargetUpdated` 只更新跳回目标。

`HierarchyUpdated` 只更新已有 session 的父级关系和层级深度。

`HierarchyUpdated` 不创建未知 child session，不改变状态、pending interaction、摘要、捕捉序号或跳回目标。

`HierarchyUpdated` 指向自身时清空父级关系。

`HierarchyUpdated` 的层级深度在 reducer 内归一化；没有父级时深度为 0，有父级时深度限制在 1 到 8。

## View Model

Session 列表 view model 暴露项目标签、thread 标签、对话标签、状态、摘要、更新时间、当前 turn 开始时间、当前 turn 结束时间、用量、动作和行内交互。

Session 列表 view model 显式暴露 `indent_level`，表示 UI 可展示的有效缩进层级。

`indent_level` 当前最多展示 1 级；更深的领域层级在列表中按 1 级缩进呈现。

Thread 标签优先来自 `AgentSession.title`，输出完整清洗后的标题；空标题展示为未命名。

Thread 标签不在 Domain 层按展示宽度截断，视觉换行和布局约束由前端负责。

## 排序规则

Session 列表先按展示分组排序，再按块级捕捉锚点排序。

`Running`、`WaitingForApproval` 和 `WaitingForAnswer` 属于未完成分组，展示在顶部。

`Completed`、`Failed` 和 `Detached` 属于已结束分组，展示在未完成分组下方。

无有效父子关系的 session 自身是一个展示块。

存在有效父级关系时，child session 紧跟 parent session 展示。

父子展示块不拆散；块内任一 session 未完成时，整个块进入未完成分组。

展示块内存在未完成 session 时，块级捕捉锚点取块内最新未完成 session 的捕捉序号。

展示块内不存在未完成 session 时，块级捕捉锚点取块内最新 session 的捕捉序号。

同一展示分组内，块级捕捉锚点越新越靠前。

状态变化只影响展示分组和块级捕捉锚点；摘要、标题或更新时间变化不改变捕捉序。

父级缺失、父级无效或父子关系成环时，相关 session 回退为顶层展示。

块级捕捉锚点相同的异常情况按 root `SessionKey` 稳定兜底排序。

## 相关测试

`src-tauri/src/domain/agent_session.rs` 覆盖 session key 和 capability 基础断言。

`src-tauri/src/domain/agent_interaction.rs` 覆盖 pending interaction 访问规则。

`src-tauri/src/domain/session_state.rs` 覆盖 reducer 分支、多项目多对话隔离、pending 清理和排序。

`src-tauri/src/domain/view_model.rs` 覆盖 capability 到 UI action、文本展示策略和基础展示映射。
