# UI Runtime

## 职责

UI Runtime 记录前端运行时、扩展模式布局、设置页和本地 UI 验证事实。

UI Runtime 不记录 React 组件内部私有状态细节，不替代外部行为文档。

## 运行时事实

阶段 7 首屏仍为扩展模式。

阶段 7 不提供 mini 模式切换入口。

Tauri 主窗口默认尺寸调整为扩展模式工作台尺寸。

前端合并 Codex CLI 和 Codex APP session 后按首次捕捉顺序保持稳定。

刷新时新捕捉到的 session 插入列表顶部；已捕捉 session 的状态或更新时间变化不触发重排。

前端跨来源合并 session 时保持首次捕捉顺序；当后端已把 Codex APP sub agent 排在 parent 下方时，前端把该 parent-child 相邻段作为不可拆散的展示块。

前端 session 行按 `indent_level` 设置左侧缩进，当前最多展示 1 级缩进。

前端读取 session 时按来源独立收敛失败；单一来源失败不会阻断其它来源展示。

前端 session 列表刷新由统一调度器收口定时刷新和实时事件刷新。

刷新进行中收到新的实时事件时，调度器会在当前刷新结束后补一次刷新。

连续实时事件不会无限后延列表刷新。

标题栏显示运行中、等待中和 session 总数。

标题栏按工具展示整体用量摘要。

工具整体用量按真实 session 的账号窗口用量计算。

同一工具同一 `source_key` 的账号窗口用量只取 `updated_at` 最新值，不按 session 求和。

标题栏最右侧提供设置按钮和关闭窗口按钮。

标题栏最右侧提供最小化窗口按钮。

前端支持浅色和深色两种主题，由 Display 设置控制。

前端主界面使用紧凑工作台布局，主体为无标题单列 session table。

每个 session 主行左侧多行展示状态、运行时来源、项目名和 thread 名。

每个 session 主行中间展示当前输出文本。

运行中、等待审批和等待用户回复的 session 主行右侧展示当前 turn 已运行时间。

运行中 session 主行右侧展示禁用的停止占位按钮；当前停止能力未接入真实后端行为。

完成和失败 session 主行右侧展示当前 turn 结束到现在的相对时间。

session 来源徽章按运行时来源派生：来自 Codex APP 的 session 显示为 `Codex`，来自 Codex CLI 的 session 显示为 `Codex CLI`。

当前输出文本 tooltip 由前端自绘，hover 或 focus 后立即展示，保留段内换行并按 Markdown 渲染。

当前输出文本 tooltip 通过视口 fixed 浮层展示，并在下方空间不足时翻到触发文本上方，避免被 session 列表和 panel 容器裁剪。

等待审批和等待用户回复的 session 会自动展示第二行。

完成和失败且可创建后续 turn 的 session 默认视觉上保持单行。

完成和失败且可创建后续 turn 的 session 不展示展开按钮。

用户 hover 或 focus 到完成或失败 session 行时展示第二行。

完成和失败 session 的第二行只展示快捷输入和单行输入区。

有选项的行内回复区展示选项按钮，并保留 choice tooltip。

无选项的行内回复区展示设置中的自定义快捷输入。

设置页通过单独弹窗展示。

前端运行时不再提供收缩按钮。

设置中的 Panel `collapsed` 字段会被归一化为 `false`，不再作为主界面布局状态。

Tauri 环境会在设置读取后尝试恢复上次窗口位置、尺寸和置顶偏好。

Tauri 环境会监听主窗口移动和尺寸变化，并以局部保存 command 持久化窗口几何。

Tauri 环境依赖 `core:window:allow-start-dragging` 允许顶部拖动区移动窗口。

Tauri 环境依赖 `core:window:allow-minimize` 允许前端最小化主窗口。

Tauri 环境依赖 `core:window:allow-set-always-on-top` 允许前端应用面板置顶设置。

浏览器开发环境不执行 Tauri 窗口几何恢复。

session 选中和回复草稿由前端 UI 状态管理。

点击 session 时，如果该 session 明确具备跳回目标和跳回能力，前端会请求跳转到对应工具界面。

没有跳回能力或没有跳回目标的 session 点击后无反应，不更新选中态且不展示错误。

有跳回能力和跳回目标但全局跳回开关关闭的 session 点击后只更新选中态，不请求跳回且不展示错误。

设置弹窗包含 General、Display、Agents、Replies、Presets、Terminal 和 Advanced 分组。

设置弹窗包含 Hook Install 分组。

设置弹窗不提供自动更新配置项。

Hook Install 分组以列表展示 Codex CLI hook 和 Claude CLI hook 状态。

Hook Install 每个 hook 项只提供安装和卸载入口。

Hook Install 分组不因 agent 开关变化自动写入第三方配置。

设置修改后立即请求保存；保存失败时保留用户当前 UI 选择并显示错误。

设置保存响应带有前端请求版本保护，旧响应不覆盖较新的 UI 设置状态。

面板置顶设置保存成功后，前端按最新保存响应应用当前窗口置顶状态。

当前 UI 启用 Codex CLI 和 Codex APP 开关。

Codex APP 开关默认开启，并驱动 Codex APP session 读取。

Claude Code CLI 和 Claude Code APP 开关当前禁用。

浏览器开发环境使用 localStorage fallback，并在读取后校验结构。

Tauri 环境通过 settings command 读写 JSON 设置文件。

配置缺失时使用默认设置。

配置损坏时使用默认设置并返回提示。

## 代码入口

`src/views/BuilderPanelApp.tsx` 是扩展模式工作台、session 合并捕捉顺序、顶部状态区、行内交互、工具用量摘要、设置弹窗和跳回调用入口。

`src/components/SettingsPanel.tsx` 是设置页组件入口。

`src/api/settingsContract.ts` 是前端设置契约入口。

`src/api/settingsApi.ts` 是前端设置读写和 fallback 校验入口。

`src/api/panelWindowApi.ts` 是前端 panel 窗口几何恢复、监听、局部保存、最小化和关闭窗口入口。

`src/api/sessionJumpApi.ts` 是前端 session 跳回 command 调用入口。

`src/api/hookInstallApi.ts` 是前端 hook 状态查询和安装 command 调用入口。

`src/styles.css` 是阶段 7 扩展模式布局入口。

`src-tauri/tauri.conf.json` 是窗口默认尺寸入口。

`src-tauri/src/services/settings_service.rs` 是设置应用服务入口。

`src-tauri/src/adapters/config_file/mod.rs` 是 JSON 设置文件 adapter 入口。

## 相关测试

`src/views/BuilderPanelApp.test.ts` 覆盖合并 session 捕捉顺序、统计、能力动作标签、工具用量聚合和 follow-up 展开规则。

`src/api/settingsApi.test.ts` 覆盖阶段 7 默认设置、收缩状态归一化和自定义快捷输入校验。

`src/views/BuilderPanelApp.test.ts` 覆盖 hook 安装按钮禁用规则。

`src/components/SettingsPanel.test.tsx` 覆盖 hook 状态列表展示和单项安装卸载按钮。

`src-tauri/src/services/settings_service.rs` 覆盖配置缺失、配置损坏和保存。

`src-tauri/src/adapters/config_file/mod.rs` 覆盖 JSON 设置文件读写和损坏文件。

`pnpm build` 验证前端类型和生产构建。
