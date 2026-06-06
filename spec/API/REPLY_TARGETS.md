# Reply Targets

## 模块职责

本文档记录 `ReplyTarget` 的稳定语义、跳回与回写边界，以及失败后的外部表现。

本文档不记录真实 agent 私有协议字段。

## 代码入口

`src-tauri/src/domain/agent_interaction.rs` 是 `ReplyTarget` 定义入口。

`src-tauri/src/ports/reply_sender_port.rs` 是文本回复回写端口入口。

`src-tauri/src/ports/jump_target_port.rs` 是跳回端口入口。

`src-tauri/src/adapters/terminal/mod.rs` 是阶段 5 终端跳回 adapter 入口。

## ReplyTarget 语义

`StructuredRpc` 表示通过结构化 RPC 回写。

`HookDirective` 表示通过 hook stdout directive 回写。

`ManagedProcessStdin` 表示通过托管进程 stdin 回写。

`ControlledTerminal` 表示通过受控终端输入回写。

`ClipboardOnly` 表示不能可靠自动回写，只能复制降级。

## 跳回与回写

跳回和文本回写是两个独立能力。

跳回成功不能推导出发送能力。

发送成功不能推导出跳回能力。

UI 必须分别读取 `can_jump` 和 `can_send_reply`。

回写失败时不得自动二次发送。

复制降级只提示用户手动处理，不冒充已完成回写。

## 相关测试

`src-tauri/src/domain/view_model.rs` 覆盖 capability 到 UI 动作的独立映射。

`src-tauri/src/adapters/terminal/mod.rs` 覆盖跳回记录和复制降级错误。

`src-tauri/src/services/reply_service.rs` 覆盖文本回复失败不清 pending。
