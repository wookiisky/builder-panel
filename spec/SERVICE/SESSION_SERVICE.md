# Session Service

## 模块职责

Session Service 负责读取当前会话状态，并生成前端可消费的 session 列表和详情 view model。

Session Service 不负责生成 agent 事件，不负责处理审批或回复回写，不负责时间线分页，不直接执行 Tauri command。

mock 测试基线中，Session Service 读取 mock agent runtime 中已经折叠完成的 `SessionState`。

## 代码入口

`src-tauri/src/services/session_service.rs` 是 Session Service 入口。

`src-tauri/src/adapters/mock_agent/mod.rs` 是 mock 测试基线 runtime 入口。

`src-tauri/src/domain/session_state.rs` 是 session 状态事实入口。

`src-tauri/src/domain/view_model.rs` 是 view model 纯转换入口。

`src-tauri/src/tauri_api/commands.rs` 是前端读取 session 的 Tauri command 入口。

## 对外接口

Session Service 提供 session 列表读取接口。

Session Service 提供指定 `SessionKey` 的 session 详情读取接口。

调用方必须传入已清洗的 `SessionKey`。

未知 session 详情返回空结果，不写错误事实。

## 核心流程

Mock agent adapter 生成测试用已清洗归一事件。

Mock agent runtime 使用 Domain reducer 折叠测试事件。

Session Service 在测试基线中从 mock agent runtime 读取 `SessionState`。

Session Service 调用 Domain view model 纯转换。

Session view model 的 `TextDisplay` 同时提供截断展示文本和当前 view model 可用的完整清洗文本。

Session 列表 view model 提供当前 turn 开始时间和结束时间，供前端展示运行耗时和结束后的相对时间。

Session 列表 view model 提供 `indent_level`，供前端按后端已计算的有效层级展示少量缩进。

前端 session 行可见摘要只展示最后一段，并按列表展示上限截断。

前端 session 行摘要 tooltip 使用 `TextDisplay.full_text` 最近若干段完整文本，段数由展示设置决定。

Session 详情可使用 `TextDisplay` 的完整清洗文本展示多段摘要。

产品 Tauri command 当前不通过 Session Service 读取 mock runtime。

## 状态与幂等

Session Service 不拥有独立状态。

Session 状态主事实位于 `SessionState`。

同一个 `SessionKey` 的重复读取不会改变 runtime 状态。

列表排序以 Domain reducer 的排序规则为准，包含展示分组、块级捕捉锚点和父子相邻规则。

前端不重新计算 parent-child 排序；Session Service 输出的列表顺序已经包含 Domain 的父子相邻规则。

## 错误收敛

读取未知 session 详情不写失败状态。

mock 测试基线可验证 runtime 锁损坏时的错误收敛。

Session Service 不重试。

Session Service 不清理 pending interaction。

## 观测与验收

mock 测试基线列表必须能展示等待审批、等待回复、完成和失败状态。

mock 测试基线用量可用时展示已验证数字。

mock 测试基线用量不可用时展示不可用占位，不生成虚假数字。

## 相关测试

`src-tauri/src/services/session_service.rs` 覆盖 session 列表和详情读取。

`src-tauri/src/adapters/mock_agent/mod.rs` 覆盖 mock 事件折叠、多项目多对话隔离和用量可用性。

`src-tauri/src/domain/view_model.rs` 覆盖 view model 映射。
