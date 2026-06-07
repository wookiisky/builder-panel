# Reply Service

## 模块职责

Reply Service 负责处理开放性文本回复提交。

Reply Service 只处理当前 session 中的 pending text reply。

Reply Service 不负责审批，不负责选项交互，不负责快捷回复过滤，不负责真实 agent 私有协议。

mock 测试基线中，文本回复通过本文本回复路径回写到 mock agent runtime 的 directive 记录。

## 代码入口

`src-tauri/src/services/reply_service.rs` 是 Reply Service 入口。

`src-tauri/src/ports/reply_sender_port.rs` 定义文本回复发送端口。

`src-tauri/src/adapters/mock_agent/mod.rs` 是 mock 测试基线文本回复记录入口。

`src-tauri/src/domain/agent_interaction.rs` 是 pending text reply 事实入口。

`src-tauri/src/tauri_api/commands.rs` 是 Codex APP 文本回复提交 command 入口。

`src-tauri/src/services/shortcut_reply_service.rs` 是快捷回复过滤入口。

## 对外接口

调用方提交 `SendReplyRequest`。

请求必须包含 `SessionKey`、`InteractionId` 和文本内容。

mock 测试基线支持注入一次 mock 回写失败，用于验证失败路径。

文本回复最大长度为 1000 个字符。

快捷回复点击后必须复用文本回复提交路径。

## 核心流程

Reply Service 先清理回复内容首尾空白。

Reply Service 拒绝空内容。

Reply Service 拒绝超过最大长度的内容。

Reply Service 校验 session 正处于 `WaitingForAnswer`。

Reply Service 校验 pending interaction 类型是文本回复。

Reply Service 校验请求中的 `InteractionId` 与当前 pending text reply 一致。

校验通过后，Reply Service 调用 reply sender 端口回写文本。

mock 测试基线 runtime 记录 directive 后写入 `TurnCompleted` 事件并清理 pending。

## 状态与幂等

文本回复提交以当前 `SessionKey` 和 `InteractionId` 作为有效性边界。

当前 pending 变化后，旧 `InteractionId` 的提交会被拒绝。

成功回写后，pending interaction 由 Domain reducer 清理。

失败回写不清理 pending interaction。

前端草稿按 session 独立保存。

成功发送后，前端只清理当前 session 草稿。

发送失败后，前端保留当前草稿。

快捷回复发送失败后，前端将快捷回复内容写回当前草稿。

## 错误收敛

空内容、超长内容、会话不存在、状态不匹配、交互类型不匹配或交互 ID 不匹配时，返回应用错误。

mock 测试基线回写失败时，返回可重试错误。

失败不写 `TurnCompleted`。

失败不记录 directive。

失败不清理 pending interaction。

## 观测与验收

非空内容可以发送。

空内容不能发送。

超过最大长度不能发送。

`Enter` 发送文本回复。

`Shift+Enter` 在文本框中换行。

发送成功清空当前 session 草稿。

发送失败保留当前 session 草稿。

## 相关测试

`src-tauri/src/services/reply_service.rs` 覆盖非空、空内容、超长和回写失败。

`src-tauri/src/adapters/mock_agent/mod.rs` 覆盖文本回复 directive 记录和失败不清理 pending。

`src/stores/mockPanelStore.test.ts` 覆盖草稿隔离和清理规则。
