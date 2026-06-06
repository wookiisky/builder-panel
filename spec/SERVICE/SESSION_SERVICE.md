# Session Service

## 模块职责

Session Service 负责读取当前会话状态，并生成前端可消费的 session 列表和详情 view model。

Session Service 不负责生成 agent 事件，不负责处理审批或回复回写，不负责时间线分页，不直接执行 Tauri command。

阶段 3 中，Session Service 读取 mock agent runtime 中已经折叠完成的 `SessionState`。

## 代码入口

`src-tauri/src/services/session_service.rs` 是 Session Service 入口。

`src-tauri/src/adapters/mock_agent/mod.rs` 是阶段 3 mock agent runtime 入口。

`src-tauri/src/domain/session_state.rs` 是 session 状态事实入口。

`src-tauri/src/domain/view_model.rs` 是 view model 纯转换入口。

`src-tauri/src/tauri_api/commands.rs` 是前端读取 session 的 Tauri command 入口。

## 对外接口

Session Service 提供 session 列表读取接口。

Session Service 提供指定 `SessionKey` 的 session 详情读取接口。

调用方必须传入已清洗的 `SessionKey`。

未知 session 详情返回空结果，不写错误事实。

## 核心流程

Mock agent adapter 生成已清洗的归一事件。

Mock agent runtime 使用 Domain reducer 折叠事件。

Session Service 从 mock agent runtime 读取 `SessionState`。

Session Service 调用 Domain view model 纯转换。

Tauri command 将 view model 返回前端。

## 状态与幂等

Session Service 不拥有独立状态。

Session 状态主事实位于 `SessionState`。

同一个 `SessionKey` 的重复读取不会改变 runtime 状态。

列表排序以 Domain reducer 的排序规则为准。

## 错误收敛

读取未知 session 详情不写失败状态。

Mock runtime 锁损坏时，Tauri command 返回错误消息。

Session Service 不重试。

Session Service 不清理 pending interaction。

## 观测与验收

Mock 列表必须能展示等待审批、等待回复、完成和失败状态。

Mock 用量可用时展示已验证数字。

Mock 用量不可用时展示不可用占位，不生成虚假数字。

## 相关测试

`src-tauri/src/services/session_service.rs` 覆盖 session 列表和详情读取。

`src-tauri/src/adapters/mock_agent/mod.rs` 覆盖 mock 事件折叠、多项目多对话隔离和用量可用性。

`src-tauri/src/domain/view_model.rs` 覆盖 view model 映射。
