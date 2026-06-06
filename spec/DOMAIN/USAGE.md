# Usage

## 职责

Usage 记录 agent 会话或账号上下文中的已验证用量事实。

Usage 负责区分已验证数字和不可用状态。

Usage 不推断来源，不跨 agent 换算单位，不用魔法数字表达不可用。

## 代码入口

`src-tauri/src/domain/usage.rs` 定义用量值、用量快照和领域时间戳。

`src-tauri/src/domain/agent_session.rs` 将用量快照挂到 session。

`src-tauri/src/domain/view_model.rs` 定义用量到 UI 标签的纯转换。

## 当前语义

`UsageValue::Verified` 表示来源已经验证，数字可以展示。

已验证数字必须是非负有限数。

`UsageValue::Unavailable` 表示来源不可用或未验证。

不可用状态展示为 `--`。

已验证状态展示数字、可选单位和来源标签。

单位不可确认时只展示数字。

不同 agent 的用量单位不强行合并。

## 时间语义

`UnixMillis` 是 Domain 内部当前使用的时间戳值对象。

`UnixMillis` 不读取系统时间。

adapter 或 service 负责在进入 Domain 前提供具体时间。

## 相关测试

`src-tauri/src/domain/usage.rs` 覆盖不可用占位、已验证数字、单位和来源标签。

`src-tauri/src/domain/usage.rs` 覆盖负数、非数字和无穷大不可进入已验证数字。

`src-tauri/src/domain/view_model.rs` 覆盖用量 view model 映射。
