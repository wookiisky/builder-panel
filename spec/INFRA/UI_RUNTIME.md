# UI Runtime

## 职责

UI Runtime 记录前端运行时、扩展模式布局、设置页和本地 UI 验证事实。

UI Runtime 不记录 React 组件内部私有状态细节，不替代外部行为文档。

## 运行时事实

阶段 7 首屏仍为扩展模式。

阶段 7 不提供 mini 模式切换入口。

Tauri 主窗口默认尺寸调整为扩展模式工作台尺寸。

前端合并 mock 和 Codex CLI session 后再次排序，等待用户操作优先，同状态按更新时间倒序。

前端显示等待数量、运行数量和 session 总数。

收缩状态由 panel UI store 管理。

收缩状态读取设置中的 Panel 配置初始化。

收缩状态变化后通过局部保存 command 写回设置文件。

Tauri 环境会在设置读取后尝试恢复上次窗口位置和尺寸。

Tauri 环境会监听主窗口移动和尺寸变化，并以局部保存 command 持久化窗口几何。

浏览器开发环境不执行 Tauri 窗口几何恢复。

session 选中和回复草稿由 mock panel UI store 管理。

收缩和展开不会清理 session 选中和草稿。

设置页包含 General、Display、Agents、Replies、Presets、Terminal 和 Advanced 分组。

设置页包含 Hook Install 分组。

设置页不提供自动更新配置项。

Hook Install 分组提供 Codex CLI hook 和 Claude CLI hook 选择、预览、安装和卸载入口。

Hook Install 分组不因 agent 开关变化自动写入第三方配置。

设置修改后立即请求保存；保存失败时保留用户当前 UI 选择并显示错误。

设置保存响应带有前端请求版本保护，旧响应不覆盖较新的 UI 设置状态。

当前 UI 只启用 mock agent 和 Codex CLI 开关。

Codex APP、Claude Code CLI 和 Claude Code APP 开关当前禁用。

浏览器开发环境使用 localStorage fallback，并在读取后校验结构。

Tauri 环境通过 settings command 读写 JSON 设置文件。

配置缺失时使用默认设置。

配置损坏时使用默认设置并返回提示。

## 代码入口

`src/views/BuilderPanelApp.tsx` 是扩展模式工作台、session 合并排序、设置页接入和设置影响 UI 的入口。

`src/components/SettingsPanel.tsx` 是设置页组件入口。

`src/api/settingsContract.ts` 是前端设置契约入口。

`src/api/settingsApi.ts` 是前端设置读写和 fallback 校验入口。

`src/api/panelWindowApi.ts` 是前端 panel 窗口几何恢复、监听和局部保存入口。

`src/api/hookInstallApi.ts` 是前端 hook 安装 command 调用入口。

`src/stores/panelProbeStore.ts` 是收缩状态入口。

`src/stores/mockPanelStore.ts` 是 session 选中、草稿和 timeline 弹层状态入口。

`src/styles.css` 是阶段 7 扩展模式布局入口。

`src-tauri/tauri.conf.json` 是窗口默认尺寸入口。

`src-tauri/src/services/settings_service.rs` 是设置应用服务入口。

`src-tauri/src/adapters/config_file/mod.rs` 是 JSON 设置文件 adapter 入口。

## 相关测试

`src/views/BuilderPanelApp.test.ts` 覆盖合并 session 排序、统计和能力动作标签。

`src/stores/panelProbeStore.test.ts` 覆盖收缩状态默认值、切换和草稿保留边界。

`src/api/settingsApi.test.ts` 覆盖阶段 7 默认设置。

`src/views/BuilderPanelApp.test.ts` 覆盖 hook 安装目标选择。

`src-tauri/src/services/settings_service.rs` 覆盖配置缺失、配置损坏和保存。

`src-tauri/src/adapters/config_file/mod.rs` 覆盖 JSON 设置文件读写和损坏文件。

`pnpm build` 验证前端类型和生产构建。
