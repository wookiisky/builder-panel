# Tauri Events

## 职责

本文档记录前端可订阅的 Tauri 事件契约。

本文档不记录 Tauri command、bridge NDJSON 协议或第三方 agent 原始 payload。

## 事件边界

Tauri 事件只承载 Builder Panel 已清洗后的轻量通知。

Tauri 事件不得携带 Codex app-server 原始 JSON、hook 原始 payload、rollout 原始行或大文本正文。

Tauri 事件发送失败不写 session 失败状态。

前端必须保留 command 定时刷新作为事件缺失时的兜底。

## Session Updated

`session_updated` 表示某个 Codex CLI 或 Codex APP session 的 session 摘要、状态、动作或 timeline 发生变化。

Codex APP 后台 thread metadata、session index 或 rollout 历史补齐已有 session 的标题、项目、跳回目标、能力或摘要时，也会发送 `session_updated`。

事件 payload 包含运行时来源、session key、变更区域和更新时间。

运行时来源当前只包含 Codex CLI 和 Codex APP。

变更区域当前可表达 session、timeline 或二者均变化。

后端发布器按 session 合并高频更新，并在短节流窗口后发送事件。

前端收到事件后节流刷新 session 列表。

当前 timeline 弹层打开且事件 session 与当前 session 匹配时，前端节流刷新当前 timeline 查询页。

## 代码入口

`src-tauri/src/ports/session_update_port.rs` 定义 session 更新通知端口和 payload。

`src-tauri/src/tauri_api/events.rs` 实现 Tauri session 更新事件发布器。

`src/api/sessionUpdateApi.ts` 实现前端事件订阅。

`src/views/BuilderPanelApp.tsx` 消费 session 更新事件并触发刷新。

## 相关测试

`src/views/BuilderPanelApp.test.ts` 验证 timeline 只响应匹配 session 的实时更新。
