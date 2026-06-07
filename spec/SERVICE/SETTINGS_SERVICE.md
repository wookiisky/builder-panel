# Settings Service

## 模块职责

Settings Service 负责读取、默认化和保存 Builder Panel 设置。

Settings Service 不直接读写文件系统，不决定具体配置路径，不处理 UI 组件状态。

## 代码入口

`src-tauri/src/services/settings_service.rs` 是设置模型、默认值和设置用例入口。

`src-tauri/src/ports/config_store_port.rs` 是设置存储端口入口。

`src-tauri/src/adapters/config_file/mod.rs` 是 JSON 设置文件 adapter 入口。

`src-tauri/src/tauri_api/commands.rs` 是设置读写 command 入口。

## 对外接口

设置模型显式分为 General、Display、Panel、Agents、Replies、Presets、Terminal 和 Advanced。

Display 设置控制用量展示、浅色或深色主题、UI 密度和动画等级。

Panel 设置保留 `collapsed` 字段、上次窗口位置和上次窗口尺寸。

Settings Service 会将 `collapsed` 归一化为 `false`。

Panel `collapsed` 字段不再驱动主界面布局。

Agents 设置控制 Codex CLI、Codex APP、Claude Code CLI 和 Claude Code APP 开关。

当前 UI 允许修改 Codex CLI 和 Codex APP 开关。

Codex APP 开关当前保存在模型中并驱动 Codex APP session 读取；默认值为开启。

Claude Code CLI 和 Claude Code APP 开关当前保存在模型中，但 UI 禁用，不驱动 session 读取。

Replies 设置控制 Enter 发送、快捷回复入口和自定义快捷输入。

自定义快捷输入包含 ID、标签、内容、启用状态和排序值。

自定义快捷输入字段缺失时使用默认快捷输入。

自定义快捷输入数组中的非法项会在设置边界丢弃。

自定义快捷输入 ID 重复时保留第一项。

自定义快捷输入数组存在但全部非法时保存为空数组，不恢复默认项。

设置模型不包含自动更新配置项。

panel 窗口状态可以通过局部保存 command 更新，不要求前端重写完整设置模型。

panel 窗口状态局部保存不会持久化收缩状态。

## 错误与降级

配置文件不存在时返回默认设置。

配置文件缺失字段时使用对应字段默认值。

配置文件未知字段不会进入设置模型。

自定义快捷输入的脏数据在设置反序列化或保存归一化边界收敛。

配置文件损坏或无法解析时返回默认设置，并附带用户可读提示。

配置保存失败时返回应用错误。

Settings Service 不吞掉保存失败。

JSON 设置文件 adapter 使用同目录临时文件写入后替换目标文件。

每次保存会生成唯一临时文件路径，避免同进程并发保存共享临时文件。

临时文件写入失败时不得覆盖旧配置。

## 相关测试

`src-tauri/src/services/settings_service.rs` 覆盖缺失配置、损坏配置和保存。

`src-tauri/src/adapters/config_file/mod.rs` 覆盖设置文件缺失、读写和损坏 JSON。

`src-tauri/src/adapters/config_file/mod.rs` 覆盖缺字段默认化和临时文件写入失败不覆盖旧配置。

`src-tauri/src/adapters/config_file/mod.rs` 覆盖同一路径并发保存不会共享临时文件。

`src/api/settingsApi.test.ts` 覆盖前端默认设置、默认 panel 状态事实和自定义快捷输入归一化。

`src/views/BuilderPanelApp.test.ts` 覆盖 hook 安装按钮禁用规则。
