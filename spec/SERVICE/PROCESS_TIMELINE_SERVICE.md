# Process Timeline Service

## 模块职责

Process Timeline Service 负责读取指定 session 的过程事件时间线，并执行分页、搜索和类型筛选。

Process Timeline Service 不负责生成 timeline 原始事件，不负责审批或回复回写，不负责持久化过程事件。

mock 测试基线中，timeline 数据可来自 mock agent runtime。

阶段 6 中，Codex CLI hook 事件由 timeline adapter 写入进程内内存缓存。

Codex APP hook 和 app-server 事件由 Codex APP runtime 写入进程内 timeline。

Codex CLI 和 Codex APP 已知 session 的 rollout 新增追加行可由 rollout tailer 清洗后写入进程内 timeline。

timeline 不进入 `SessionState`。

timeline 条目正文只保存已清洗后的可展示内容；用户输入和 assistant 输出不带来源前缀。

Codex CLI 和 Codex APP 工具调用、hook 工具事件和工具参数不作为最后消息 timeline 条目。

timeline 支持 `activity`、`user`、`tool`、`approval`、`reply` 和 `system` 类型。

`user` 类型用于用户原始输入，前端通过类型样式区分，不依赖正文前缀。

没有可展示正文的 session start、turn complete 或审批请求不生成 timeline 条目。

timeline 不从 transcript 或 JSONL 反向恢复历史。

timeline 允许记录已知 session 的 rollout 新增追加行所产生的实时事件。

timeline 不支持导出过程事件文件。

## 代码入口

`src-tauri/src/services/process_timeline_service.rs` 是 Process Timeline Service 入口。

`src-tauri/src/ports/process_timeline_port.rs` 定义 timeline 条目、类型和读取端口。

`src-tauri/src/adapters/timeline/mod.rs` 是 timeline 内存缓存、去重、淘汰和大文本释放入口。

`src-tauri/src/adapters/mock_agent/mod.rs` 是 mock 测试基线 timeline 数据源入口。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 是阶段 6 Codex CLI hook timeline 接收入口。

`src-tauri/src/adapters/codex_app/mod.rs` 是 Codex APP timeline 接收入口。

`src-tauri/src/adapters/codex_app/codex_rollout.rs` 是已知 rollout 追加行实时事件清洗入口。

`src-tauri/src/tauri_api/commands.rs` 是 timeline 查询 command 入口。

`src/stores/mockPanelStore.ts` 是前端 timeline 弹层缓存状态入口。

## 对外接口

调用方提交 `TimelineQuery`。

请求必须包含 `SessionKey`、页码和每页数量。

搜索关键词为空时不启用搜索过滤。

类型为空时不启用类型过滤。

每页数量被限制在 1 到 50 之间。

关闭弹层时，前端可请求释放该 session 的后端大文本缓存。

## 核心流程

托管 hook 事件在 adapter 边界转换为 `ProcessTimelineItem`。

已知 rollout 追加行在 adapter 边界先转换为归一 agent 事件，再由 timeline adapter 转换为 `ProcessTimelineItem`。

timeline adapter 使用 `SessionKey` 和条目 ID 去重。

Process Timeline Service 通过 reader 端口读取指定 session 的 timeline 条目。

服务先执行类型筛选。

服务再执行正文搜索匹配；隐藏标题不参与搜索。

服务按页码和页大小截取当前页。

服务返回当前页、过滤后总数、是否存在下一页和过滤器数量。

## 状态与幂等

Process Timeline Service 不拥有独立状态。

相同查询对相同数据源返回相同结果。

timeline 内存缓存按 session 分片。

单 session 和全局条目数都有上限。

达到上限时，缓存优先淘汰最旧的低优先级条目。

审批、回复和失败相关条目优先保留。

前端打开 timeline 时读取第一页。

前端关闭 timeline 时清理当前页缓存，并请求后端释放该 session 的大文本正文缓存。

## 错误收敛

reader 端口返回错误时，服务直接返回应用错误。

服务不写 session 状态。

服务不重试。

释放大文本缓存失败时，关闭弹层不被阻塞。

## 观测与验收

只有具备 `ViewProcessTimeline` 动作的 session 展示 timeline 入口。

搜索只展示正文匹配的条目。

类型筛选只展示匹配类型的条目。

关闭弹层后，前端不保留当前页缓存。

复制单条 timeline 只复制该条正文。

复制筛选结果只复制当前筛选页的正文。

跳到最新只影响弹层滚动位置，不改变后端数据。

前端使用固定高度滚动窗口承载大量 timeline 条目。

页面不提供导出过程事件文件入口。

## 相关测试

`src-tauri/src/services/process_timeline_service.rs` 覆盖分页、正文搜索和类型筛选。

`src-tauri/src/adapters/timeline/mod.rs` 覆盖用户类型映射、空摘要不写条目、去重、单 session 上限、全局上限、优先级淘汰和大文本释放。

`src-tauri/src/adapters/codex_cli_hook/mod.rs` 覆盖 Codex CLI hook 事件写入 timeline。

`src/stores/mockPanelStore.test.ts` 覆盖关闭弹层释放缓存页。

`src/stores/mockPanelStore.test.ts` 覆盖复制筛选结果和虚拟列表范围计算。
