# UI Runtime

## 职责

UI Runtime 记录前端运行时、扩展模式布局、设置页和本地 UI 验证事实。

UI Runtime 不记录 React 组件内部私有状态细节，不替代外部行为文档。

## 运行时事实

阶段 7 首屏仍为扩展模式。

阶段 7 不提供 mini 模式切换入口。

Tauri 主窗口默认宽度面向扩展模式工作台，默认高度面向首屏内容自适应。

前端合并 Codex CLI 和 Codex APP session 后按展示分组和首次观察顺序排序。

运行中、等待审批和等待用户回复属于未完成分组，展示在完成、失败和失联分组上方。

前端跨来源合并 session 时维护独立首次观察序；身份键使用包含 runtime source 的前端展示身份。

同一次刷新里首次观察到多个新 session 时，前端按返回数组顺序分配首次观察序。

刷新时状态变化可以触发 session 跨展示分组移动；摘要或更新时间变化不改变首次观察序。

当前端已把 Codex APP sub agent 排在 parent 下方时，前端把该 parent-child 相邻段作为不可拆散的展示块。

Codex APP parent-child 展示块内任一 session 未完成时，整个块进入未完成分组。

前端 session 行按 `indent_level` 设置左侧缩进，当前最多展示 1 级缩进。

前端读取 session 时按来源独立收敛失败；单一来源失败不会阻断其它来源展示。

前端 session 列表刷新由统一调度器收口定时刷新和实时事件刷新。

刷新进行中收到新的实时事件时，调度器会在当前刷新结束后补一次刷新。

连续实时事件不会无限后延列表刷新。

标题栏显示运行中、等待中和 session 总数。

标题栏按工具展示整体用量摘要。

工具整体用量按真实 session 的账号窗口用量计算。

同一工具同一 `source_key` 的账号窗口用量只取 `updated_at` 最新值，不按 session 求和。

标题栏最右侧提供设置图标按钮和关闭窗口图标按钮。

标题栏最右侧提供最小化窗口图标按钮。

前端支持浅色和深色两种主题，由 Display 设置控制。

前端主界面使用紧凑工作台布局，主体为无标题单列 session table。

前端主窗口高度由当前面板内容派生，不把高度作为长期用户偏好恢复。

当前面板内容少于窗口最大高度时，窗口高度随内容收缩，避免展示大面积空白背景。

前端内容高度自适应以标题栏、边框、外层 padding 和 session 内容区自然高度计算，不以当前 WebView 视口高度作为自然高度来源。

session 自然内容层与受限滚动视口是两个独立布局层；自然内容层提供测量，滚动视口负责窗口达到上限后的裁剪和滚动。

设置弹窗打开时会作为当前可见内容参与窗口高度自适应，仍受窗口最大高度和屏幕可用高度限制。

当前打开的 overlay 面板尺寸变化时，前端会主动请求窗口内容高度自适应；动态子级 overlay 通过 DOM 子树变化监听纳入同一测量和收敛链路，挂载、卸载都会触发重算。

当前面板内容超过用户配置的窗口最大高度时，窗口高度限制在该最大高度内，session 内容区滚动。

前端根布局限制在当前 WebView 视口内，超过上限的内容滚动必须收敛在 session 内容区。

Panel `max_window_height` 控制窗口最大逻辑高度，默认值为 400，前端设置边界最小值为 160、最大值为 2000。

当前屏幕可用高度小于用户配置的窗口最大高度时，窗口高度限制在屏幕内，session 内容区滚动。

窗口内容高度自适应上限按当前窗口顶边和屏幕可用区域底边计算，避免窗口底部越出屏幕。

窗口和当前显示器工作区几何来自 Tauri API，并按当前窗口 scale factor 转换为同一逻辑坐标系。

窗口高度调整前读取真实窗口高度；手动拉高会重新收缩，手动改宽会保留真实宽度并触发内容重排后的高度计算。

内容高度调整串行执行，并在执行期间把多次变化合并为一次最新状态重跑。

session 展示文本、行内交互和影响行高的显示设置变化时，前端会主动请求窗口内容高度自适应。

当前 WebView 不提供 `ResizeObserver` 时，前端仍会在首屏和窗口 resize 时请求一次内容高度自适应。

`ResizeObserver` 仅补充可见盒尺寸变化触发，不作为 session 内容滚动高度变化的唯一触发来源；`MutationObserver` 负责动态内容和子级 overlay 的挂载、卸载触发。

每个 session 主行左侧展示状态 icon、运行时来源、项目名和 thread 名。

前端状态 icon 和窗口操作 icon 使用开源图标资源，不由 CSS 绘制基础形状。

状态 icon 和 panel 操作 icon 使用细线 SVG 笔画展示，不依赖粗体字重表达。

等待用户回复状态使用区别于等待审批状态的图标和状态色。

状态 icon 位于主行最左侧独立列，并跟随当前 session 缩进层级移动。

运行中状态 icon 的旋转动画由前端样式控制。

panel 逻辑宽度大于 560 时，运行时来源和项目名在同一行展示，thread 名在身份区下一行单行展示。

session 主行使用独立窄屏边界：panel 逻辑宽度大于 560 时保持状态、身份、当前输出和时间操作四列布局，常用 665 逻辑宽度不进入堆叠布局。

panel 逻辑宽度不大于 560 时，session 主行切换为状态、内容、时间操作三列；状态和时间操作跨两行，内容列第一行展示运行时来源、项目名和 thread 名，第二行展示当前输出。

项目名和 thread 名仅在视觉截断时展示 tooltip。

每个 session 主行中间展示当前输出文本，最多展示两行。

运行中、等待审批和等待用户回复的 session 主行右侧展示当前 turn 已运行时间。

运行中 session 主行右侧展示禁用的红色 octagon-x 停止占位图标；当前停止能力未接入真实后端行为。

完成和失败 session 主行右侧展示当前 turn 结束到现在的相对时间。

session 来源标记按运行时来源派生并以无填充样式展示：来自 Codex APP 的 session 显示为 `Codex`，来自 Codex CLI 的 session 显示为 `Codex CLI`。

当前输出文本 tooltip 由前端自绘，hover 或 focus 后立即展示，保留段内换行并按 Markdown 渲染。

当前输出文本 tooltip 只在摘要存在未展示段落、字符截断或两行视觉截断时启用；单段摘要已经完整展示时不启用 tooltip。

当前输出文本 tooltip 通过视口 fixed 浮层展示，并在下方空间不足时翻到触发文本上方，避免被 session 列表和 panel 容器裁剪。

当前输出文本 tooltip 的非链接区域双击复用 session 行跳回入口；Markdown 链接区域保留链接行为。

项目名、thread 名、选项说明和 action summary tooltip 不声明双击跳回行为。

等待审批 session 会自动展示第二行。

等待用户回复 session 在支持 hover 的环境默认保持单行。

用户 hover 或 focus 到等待用户回复 session 行时展示第二行。

触屏、无 hover 或粗指针环境下，等待用户回复 session 默认展示第二行。

完成和失败且可创建后续 turn 的 session 默认视觉上保持单行。

完成和失败且可创建后续 turn 的 session 不展示展开按钮。

用户 hover 或 focus 到完成或失败 session 行时展示第二行。

完成和失败 session 的第二行从左到右展示单行输入区、发送 icon 和快捷输入。

等待用户回复只有具备可回写目标时才展示第二行。

可回写等待用户回复的第二行展示文本回复输入区，或展示选项和提交选择入口。

等待用户回复的列表行和详情面板不展示设置中的自定义快捷输入。

选项行使用区别于自定义快捷输入按钮的样式，并保留 choice tooltip。

设置页通过单独弹窗展示。

前端运行时不再提供收缩按钮。

设置中的 Panel `collapsed` 字段会被归一化为 `false`，不再作为主界面布局状态。

Tauri 环境会在设置读取后尝试恢复上次窗口位置、逻辑宽度和置顶偏好。

旧版 `window_size` 是物理像素尺寸，设置归一化时会清空且不迁移为逻辑宽度。

Tauri 环境会监听主窗口移动和尺寸变化，并以局部保存 command 持久化窗口位置和逻辑宽度。

Tauri 环境不会把内容自适应得到的窗口高度写成稳定偏好。

Tauri 环境依赖 `core:window:allow-start-dragging` 允许顶部拖动区移动窗口。

Tauri 顶部拖动区的非交互子元素直接声明 `data-tauri-drag-region`，窗口操作按钮不声明拖动区。

Tauri 环境依赖 `core:window:allow-minimize` 允许前端最小化主窗口。

Tauri 环境依赖 `core:window:allow-set-always-on-top` 允许前端应用面板置顶设置。

panel 窗口 API 通过 `@tauri-apps/api/core.isTauri()` 判断当前是否运行在 Tauri 环境。

浏览器开发环境不执行 Tauri 窗口几何恢复。

session 选中和回复草稿由前端 UI 状态管理。

点击 session 时，如果该 session 明确具备跳回目标和跳回能力，前端会请求跳转到对应工具界面。

没有跳回能力或没有跳回目标的 session 点击后无反应，不更新选中态且不展示错误。

有跳回能力和跳回目标但全局跳回开关关闭的 session 点击后只更新选中态，不请求跳回且不展示错误。

设置弹窗包含 General、Panel、Display、Agents、Hook Install、Replies、Presets、Terminal、Advanced 和 Logging 分组。

设置弹窗内容区不再渲染独立说明 header。

设置弹窗包含 Hook Install 分组。

设置弹窗不提供自动更新配置项。

Hook Install 分组以列表展示 Codex CLI hook 和 Claude CLI hook 状态。

Hook Install 每个 hook 项只提供安装和卸载入口。

设置页语义简单的行内动作使用 icon 按钮或 icon 加文字按钮，并保留可访问名称。

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

`src/api/panelWindowApi.ts` 是前端 panel 窗口位置和逻辑宽度恢复、内容高度自适应、监听、局部保存、最小化和关闭窗口入口。

`src/api/panelWindowGeometryContract.ts` 是统一逻辑窗口几何契约入口。

`src/stores/panelAdaptiveSizing.ts` 是目标高度和真实高度比较纯规则入口。

`src/stores/panelAdaptiveResizeController.ts` 是 resize 串行和最新请求合并入口。

`src/views/panelContentMeasurement.ts` 是自然内容层和 overlay 测量入口。

`src/views/useAdaptivePanelWindow.ts` 是内容观察、窗口事件和 resize 副作用编排入口。

`src/api/tauriRuntime.ts` 是前端 Tauri 运行时判断入口。

`src/api/sessionJumpApi.ts` 是前端 session 跳回 command 调用入口。

`src/api/hookInstallApi.ts` 是前端 hook 状态查询和安装 command 调用入口。

`src/styles.css` 是阶段 7 扩展模式布局入口。

`src-tauri/tauri.conf.json` 是窗口默认尺寸入口。

`src-tauri/src/services/settings_service.rs` 是设置应用服务入口。

`src-tauri/src/adapters/config_file/mod.rs` 是 JSON 设置文件 adapter 入口。

## 相关测试

`src/views/BuilderPanelApp.test.ts` 覆盖合并 session 捕捉顺序、统计、能力动作标签、工具用量聚合、follow-up 展开规则和 follow-up 输入顺序。

`src/views/BuilderPanelApp.test.ts` 覆盖 panel 内容签名、重复宽度保存过滤、长内容滚动约束和标题栏拖动区标记。

`src/views/panelContentMeasurement.test.ts` 覆盖自然内容层、overlay 高度和尺寸观察目标。

`src/stores/panelAdaptiveSizing.test.ts` 覆盖配置上限、真实高度比较、多屏负坐标和显示器缺失降级。

`src/stores/panelAdaptiveResizeController.test.ts` 覆盖 resize 串行、最新请求合并、失败重试和释放行为。

`src/api/settingsApi.test.ts` 覆盖阶段 7 默认设置、panel 最大窗口高度归一化、收缩状态归一化和自定义快捷输入校验。

`src/views/BuilderPanelApp.test.ts` 覆盖 hook 安装按钮禁用规则。

`src/components/SettingsPanel.test.tsx` 覆盖 hook 状态列表展示和单项安装卸载按钮。

`src/api/panelWindowApi.test.ts` 覆盖 panel 窗口逻辑宽度恢复、物理到逻辑几何转换、保留真实宽度的内容高度调整和浏览器环境 no-op。

`src/api/tauriRuntime.test.ts` 覆盖前端 Tauri 运行时判断入口。

`src-tauri/src/services/settings_service.rs` 覆盖配置缺失、配置损坏和保存。

`src-tauri/src/adapters/config_file/mod.rs` 覆盖 JSON 设置文件读写和损坏文件。

`pnpm build` 验证前端类型和生产构建。
