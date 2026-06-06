# Notification Service

## 模块职责

Notification Service 负责把 session 状态变化转换为通知计划。

Notification Service 负责当前查看 session 抑制、同 session 短时间重复通知合并和通知点击定位动作。

Notification Service 不直接调用系统通知 API，不打开 timeline，不修改 session 状态。

## 代码入口

`src-tauri/src/services/notification_service.rs` 是通知计划、合并规则和点击动作入口。

`src-tauri/src/ports/notification_port.rs` 是通知发送端口入口。

`src-tauri/src/adapters/notification/mod.rs` 是记录型通知 adapter 入口。

## 对外接口

通知输入必须携带 `SessionKey`、通知类型、标题和正文。

通知类型覆盖 turn 完成、等待审批、等待选择和失败。

通知计划包含 `SessionKey`、通知类型、标题、正文和合并数量。

通知点击动作只要求聚焦 panel、展开 panel 和定位 session。

通知点击动作不打开过程时间线。

## 状态与合并

当前查看的 session 不发送重复通知。

同一 session 同一通知类型在合并窗口内重复触发时合并数量递增。

通知合并窗口由 service 常量定义。

合并状态保存在 Notification Service 进程内状态中。

## 降级与限制

当前系统只提供记录型通知 adapter，用于自动化测试和本地降级验证。

当前不声明真实 Mac 或 Windows 系统通知已接入。

Windows 系统通知本轮不做本机验证。

## 相关测试

`src-tauri/src/services/notification_service.rs` 覆盖当前 session 抑制、重复通知合并和点击不打开 timeline。

`src-tauri/src/adapters/notification/mod.rs` 提供记录型 adapter，供通知服务测试使用。
