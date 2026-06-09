# Errors

## 职责

Errors 定义进入 Domain 的统一应用错误对象。

Errors 负责表达错误码、用户可读消息、技术细节、可重试性和降级动作。

Errors 不负责日志写入、UI 弹窗、重试执行或资源回收。

## 代码入口

`src-tauri/src/domain/app_error.rs` 定义应用错误码、错误对象和降级动作。

`src-tauri/src/domain/agent_event.rs` 定义失败事件携带的错误对象。

`src-tauri/src/domain/session_state.rs` 定义失败事件如何影响 session。

## 错误分类

`BridgeUnavailable` 表示本地 bridge 不可用。

`MalformedAgentPayload` 表示外部 payload 无法清洗成领域事件。

`UnsupportedReplyTarget` 表示当前回复目标不支持。

`ReplySendFailed` 表示回复发送失败。

`ConfigLoadFailed` 表示配置读取失败。

`ConfigSaveFailed` 表示配置保存失败。

`AgentProtocolUnsupported` 表示 agent 协议未支持。



`NotificationSendFailed` 表示通知发送失败。

`HookInstallFailed` 表示 hook 安装失败。

`HookUninstallFailed` 表示 hook 卸载失败。

## 状态影响

`Failed` 事件写入最近错误。

`Failed` 事件将 session 状态设置为失败。

`Failed` 事件清理不可继续的 pending interaction。

错误降级动作只表达建议，不在 Domain 内执行。

## 相关测试

`src-tauri/src/domain/app_error.rs` 覆盖错误对象字段。

`src-tauri/src/domain/session_state.rs` 覆盖失败事件状态收敛和 pending 清理。
