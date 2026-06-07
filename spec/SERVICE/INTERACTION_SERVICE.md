# Interaction Service

## 模块职责

Interaction Service 负责处理用户对 pending approval 的允许、拒绝、允许并记住，以及 pending choice 的选项提交。

Interaction Service 只处理已经存在于当前 session 中的审批和选项交互。

Interaction Service 不负责文本回复，不负责时间线查询，不负责真实 agent 私有协议。

mock 测试基线中，审批结果和选项结果回写到 mock agent runtime 的 directive 记录。

## 代码入口

`src-tauri/src/services/interaction_service.rs` 是 Interaction Service 入口。

`src-tauri/src/ports/agent_adapter_port.rs` 定义审批决策、选项提交和回写端口。

`src-tauri/src/adapters/mock_agent/mod.rs` 是 mock 测试基线 directive 记录入口。

`src-tauri/src/domain/agent_interaction.rs` 是 pending approval 事实入口。

`src-tauri/src/tauri_api/commands.rs` 是真实 Codex 审批提交 command 入口。

## 对外接口

审批调用方提交 `ResolveApprovalRequest`。

审批请求必须包含 `SessionKey`、`InteractionId` 和审批决策。

选项调用方提交 `SubmitChoiceRequest`。

选项请求必须包含 `SessionKey`、`InteractionId` 和已选选项值。

mock 测试基线支持注入一次 mock 回写失败，用于验证失败路径。

## 核心流程

Interaction Service 先读取当前 session。

Interaction Service 校验 session 正处于 `WaitingForApproval`。

Interaction Service 校验 pending interaction 类型是审批。

Interaction Service 校验请求中的 `InteractionId` 与当前 pending approval 一致。

校验通过后，Interaction Service 调用 agent interaction writer 端口回写审批决策。

mock 测试基线 runtime 记录 directive 后写入 `TurnCompleted` 事件并清理 pending。

Interaction Service 处理选项时先读取当前 session。

Interaction Service 校验 session 正处于 `WaitingForAnswer`。

Interaction Service 校验 pending interaction 类型是 choice。

Interaction Service 校验请求中的 `InteractionId` 与当前 pending choice 一致。

Interaction Service 校验至少选择一项。

Interaction Service 校验选项值属于当前 choice。

Interaction Service 校验单选 choice 不提交多个值。

校验通过后，Interaction Service 调用 agent interaction writer 端口回写选项。

## 状态与幂等

审批和选项提交以当前 `SessionKey` 和 `InteractionId` 作为有效性边界。

当前 pending 变化后，旧 `InteractionId` 的提交会被拒绝。

成功回写后，pending interaction 由 Domain reducer 清理。

失败回写不清理 pending interaction。

## 错误收敛

会话不存在、状态不匹配、交互类型不匹配或交互 ID 不匹配时，返回应用错误。

选项空选择、非法选项值、重复选项值或单选提交多个值时，返回应用错误。

mock 测试基线回写失败时，返回可重试错误。

失败不写 `TurnCompleted`。

失败不记录 directive。

失败不清理 pending interaction。

## 观测与验收

测试基线点击允许后，mock agent 收到 allow directive。

测试基线点击拒绝后，mock agent 收到 deny directive。

测试基线点击允许并记住后，mock agent 收到 allow and remember directive。

测试基线提交选项后，mock agent 收到 choice directive。

提交中按钮不能重复点击。

注入失败时，前端展示错误，pending approval 或 pending choice 保持可继续提交。

## 相关测试

`src-tauri/src/services/interaction_service.rs` 覆盖 allow、deny、allow and remember、选项校验和回写失败。

`src-tauri/src/adapters/mock_agent/mod.rs` 覆盖 directive 记录和 pending 清理。

`src/stores/mockPanelStore.test.ts` 覆盖提交中状态和选项选择保留。
