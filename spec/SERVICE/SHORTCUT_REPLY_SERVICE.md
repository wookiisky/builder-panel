# Shortcut Reply Service

## 模块职责

Shortcut Reply Service 负责根据当前 session 过滤可用快捷回复。

Shortcut Reply Service 只处理配置内快捷回复的启用状态、agent 绑定、项目绑定和排序。

Shortcut Reply Service 不直接发送回复，不绕过 Reply Service 的能力校验和 pending 校验。

## 代码入口

`src-tauri/src/services/shortcut_reply_service.rs` 是快捷回复过滤和排序入口。

`src-tauri/src/services/reply_service.rs` 是快捷回复最终发送复用的文本回复入口。

`src/views/BuilderPanelApp.tsx` 是阶段 5 前端快捷回复展示入口。

## 核心流程

调用方提供当前 `SessionKey`。

Shortcut Reply Service 过滤未启用快捷回复。

Shortcut Reply Service 过滤不匹配当前 agent 类型的快捷回复。

Shortcut Reply Service 过滤不匹配当前项目的快捷回复。

Shortcut Reply Service 按排序值、标签和 ID 输出稳定顺序。

前端只在 view model 生成可回写动作时展示快捷回复入口。

快捷回复点击后复用文本回复提交路径。

发送失败时，快捷回复内容写回当前 session 草稿。

## 边界与错误

快捷回复不改变 session pending 状态。

快捷回复不直接访问真实 agent 协议。

不支持回写时，快捷回复不得自动发送。

回写失败由 Reply Service 收敛，pending 和草稿保留。

## 相关测试

`src-tauri/src/services/shortcut_reply_service.rs` 覆盖启用状态、agent 绑定、项目绑定和排序。

`src/stores/mockPanelStore.test.ts` 覆盖回复草稿按 session 隔离和失败保留所需状态。
