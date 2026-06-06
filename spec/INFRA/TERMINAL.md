# Terminal

## 模块职责

本文档记录阶段 5 终端交互边界。

本文档不声明未验证的真实终端控制能力。

## 代码入口

`src-tauri/src/ports/jump_target_port.rs` 是跳回端口入口。

`src-tauri/src/adapters/terminal/mod.rs` 是终端跳回 adapter 入口。

`src-tauri/src/ports/reply_sender_port.rs` 是文本回写端口入口。

## 当前能力

阶段 5 建立 `JumpTargetPort`，用于表达跳回 agent 所在 APP 或终端的能力。

阶段 5 的终端 adapter 提供可测试的跳回记录、系统 URL 打开和失败降级模型。

macOS 上 `codex://` 跳回目标通过系统 `open` 打开。

非 macOS 平台的 `codex://` 跳回目标当前返回复制降级。

不支持的跳回目标返回复制降级。

阶段 5 不声明 Ghostty、tmux、PowerShell 或 cmd 的人工端到端验证已完成。

阶段 5 不执行 Windows 本机验证。

## 降级策略

跳回失败时返回可重试错误。

跳回失败时降级动作为复制到剪贴板。

跳回失败不触发文本回写。

文本回写失败不触发跳回补偿。

## 相关测试

`src-tauri/src/adapters/terminal/mod.rs` 覆盖跳回记录、系统 URL 打开、失败和复制降级。
