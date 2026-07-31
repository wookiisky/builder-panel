import type { ReactNode } from "react";

import type {
  HookInstallAgent,
  HookInstallAgentStatus,
} from "../api/hookInstallApi";
import type {
  BuilderPanelSettings,
  UiDensity,
  UiTheme,
} from "../api/settingsContract";
import {
  PANEL_WINDOW_DEFAULT_MAX_HEIGHT,
  PANEL_WINDOW_MAX_CONFIGURED_MAX_HEIGHT,
  PANEL_WINDOW_MIN_CONFIGURED_MAX_HEIGHT,
} from "../api/settingsContract";
import { PanelIcon, type PanelIconName } from "./PanelIcon";

/// hook 安装 UI 状态。
export interface HookInstallPanelState {
  /// 当前 agent hook 安装状态。
  readonly agentStatuses: readonly HookInstallAgentStatus[];
  /// 当前 hook 操作状态提示。
  readonly statusMessage: string | null;
  /// 正在执行操作的 agent。
  readonly workingAgent: HookInstallAgent | null;
  /// 是否正在刷新 hook 状态。
  readonly refreshing: boolean;
}

/// 设置页属性。
export interface SettingsPanelProps {
  /// 当前设置。
  readonly settings: BuilderPanelSettings;
  /// 当前状态提示。
  readonly statusMessage: string | null;
  /// 是否正在保存。
  readonly saving: boolean;
  /// hook 安装 UI 状态。
  readonly hookInstall: HookInstallPanelState;
  /// 日志文件路径（仅展示）。
  readonly logPath: string | null;
  /// 设置变化回调。
  readonly onChange: (settings: BuilderPanelSettings) => void;
  /// 安装单个 hook 回调。
  readonly onInstallHook: (agent: HookInstallAgent) => void;
  /// 卸载单个 hook 回调。
  readonly onUninstallHook: (agent: HookInstallAgent) => void;
  /// 打开日志目录回调。
  readonly onOpenLogFolder: () => void;
}

/// Builder Panel 设置页。
export const SettingsPanel = ({
  settings,
  statusMessage,
  hookInstall,
  logPath,
  onChange,
  onInstallHook,
  onUninstallHook,
  onOpenLogFolder,
}: SettingsPanelProps) => {
  const update = (next: BuilderPanelSettings): void => {
    onChange(next);
  };

  return (
    <section className="settings-panel" aria-label="设置">
      {statusMessage !== null && (
        <p className="settings-status">{statusMessage}</p>
      )}
      <SettingsGroup title="General">
        <ToggleRow
          checked={settings.general.keep_panel_on_top}
          label="面板置顶"
          onChange={(checked) => {
            update({
              ...settings,
              general: {
                ...settings.general,
                keep_panel_on_top: checked,
              },
            });
          }}
        />
        <ToggleRow
          checked={settings.general.notify_on_completion}
          label="完成通知"
          onChange={(checked) => {
            update({
              ...settings,
              general: {
                ...settings.general,
                notify_on_completion: checked,
              },
            });
          }}
        />
        <ToggleRow
          checked={settings.general.notify_on_waiting}
          label="等待通知"
          onChange={(checked) => {
            update({
              ...settings,
              general: {
                ...settings.general,
                notify_on_waiting: checked,
              },
            });
          }}
        />
      </SettingsGroup>
      <SettingsGroup title="Panel">
        <label className="select-row">
          <span>窗口最大高度</span>
          <input
            max={PANEL_WINDOW_MAX_CONFIGURED_MAX_HEIGHT}
            min={PANEL_WINDOW_MIN_CONFIGURED_MAX_HEIGHT}
            step={1}
            type="number"
            value={settings.panel.max_window_height}
            onChange={(event) => {
              update({
                ...settings,
                panel: {
                  ...settings.panel,
                  max_window_height: panelMaxWindowHeightFromInput(
                    event.target.value,
                  ),
                },
              });
            }}
          />
        </label>
      </SettingsGroup>
      <SettingsGroup title="Display">
        <ToggleRow
          checked={settings.display.show_usage}
          label="用量展示"
          onChange={(checked) => {
            update({
              ...settings,
              display: {
                ...settings.display,
                show_usage: checked,
              },
            });
          }}
        />
        <label className="select-row">
          <span>主题</span>
          <select
            value={settings.display.theme}
            onChange={(event) => {
              update({
                ...settings,
                display: {
                  ...settings.display,
                  theme: event.target.value as UiTheme,
                },
              });
            }}
          >
            <option value="light">浅色</option>
            <option value="dark">深色</option>
          </select>
        </label>
        <label className="select-row">
          <span>密度</span>
          <select
            value={settings.display.density}
            onChange={(event) => {
              update({
                ...settings,
                display: {
                  ...settings.display,
                  density: event.target.value as UiDensity,
                },
              });
            }}
          >
            <option value="comfortable">标准</option>
            <option value="compact">紧凑</option>
          </select>
        </label>
        <ToggleRow
          checked={settings.display.animation_level === "full"}
          label="完整动画"
          onChange={(checked) => {
            update({
              ...settings,
              display: {
                ...settings.display,
                animation_level: checked ? "full" : "reduced",
              },
            });
          }}
        />
        <label className="select-row">
          <span>摘要悬浮段数</span>
          <input
            min={1}
            step={1}
            type="number"
            value={settings.display.summary_tooltip_paragraphs}
            onChange={(event) => {
              const parsed = Number.parseInt(event.target.value, 10);
              const next = Number.isFinite(parsed) && parsed >= 1 ? parsed : 1;
              update({
                ...settings,
                display: {
                  ...settings.display,
                  summary_tooltip_paragraphs: next,
                },
              });
            }}
          />
        </label>
      </SettingsGroup>
      <SettingsGroup title="Agents">
        <ToggleRow
          checked={settings.agents.codex_cli_enabled}
          label="Codex CLI"
          onChange={(checked) => {
            update({
              ...settings,
              agents: {
                ...settings.agents,
                codex_cli_enabled: checked,
              },
            });
          }}
        />
        <ToggleRow
          checked={settings.agents.codex_app_enabled}
          label="Codex APP"
          onChange={(checked) => {
            update({
              ...settings,
              agents: {
                ...settings.agents,
                codex_app_enabled: checked,
              },
            });
          }}
        />
        <ToggleRow
          checked={settings.agents.claude_cli_enabled}
          disabled={true}
          label="Claude CLI"
          onChange={(checked) => {
            update({
              ...settings,
              agents: {
                ...settings.agents,
                claude_cli_enabled: checked,
              },
            });
          }}
        />
        <ToggleRow
          checked={settings.agents.claude_app_enabled}
          disabled={true}
          label="Claude APP"
          onChange={(checked) => {
            update({
              ...settings,
              agents: {
                ...settings.agents,
                claude_app_enabled: checked,
              },
            });
          }}
        />
      </SettingsGroup>
      <SettingsGroup layout="list" title="Hook Install">
        <div className="hook-install-list">
          {hookInstall.agentStatuses.map((status) => (
            <HookInstallRow
              key={status.agent}
              status={status}
              workingAgent={hookInstall.workingAgent}
              onInstall={onInstallHook}
              onUninstall={onUninstallHook}
            />
          ))}
        </div>
        {hookInstall.statusMessage !== null && (
          <p className="settings-status">{hookInstall.statusMessage}</p>
        )}
        {hookInstall.refreshing && (
          <p className="settings-note">正在读取 hook 状态</p>
        )}
      </SettingsGroup>
      <SettingsGroup layout="stack" title="Replies">
        <ToggleRow
          checked={settings.replies.enter_to_send}
          label="Enter 发送"
          onChange={(checked) => {
            update({
              ...settings,
              replies: {
                ...settings.replies,
                enter_to_send: checked,
              },
            });
          }}
        />
        <ToggleRow
          checked={settings.replies.shortcut_replies_enabled}
          label="快捷回复"
          onChange={(checked) => {
            update({
              ...settings,
              replies: {
                ...settings.replies,
                shortcut_replies_enabled: checked,
              },
            });
          }}
        />
        <div className="shortcut-editor">
          {settings.replies.custom_shortcuts.map((shortcut, index) => (
            <div className="shortcut-editor-row" key={shortcut.id}>
              <label>
                <span>标签</span>
                <input
                  value={shortcut.label}
                  onChange={(event) => {
                    update({
                      ...settings,
                      replies: {
                        ...settings.replies,
                        custom_shortcuts: updateShortcutAt(
                          settings.replies.custom_shortcuts,
                          index,
                          {
                            ...shortcut,
                            label: event.target.value,
                          },
                        ),
                      },
                    });
                  }}
                />
              </label>
              <label>
                <span>内容</span>
                <input
                  value={shortcut.content}
                  onChange={(event) => {
                    update({
                      ...settings,
                      replies: {
                        ...settings.replies,
                        custom_shortcuts: updateShortcutAt(
                          settings.replies.custom_shortcuts,
                          index,
                          {
                            ...shortcut,
                            content: event.target.value,
                          },
                        ),
                      },
                    });
                  }}
                />
              </label>
              <ToggleRow
                checked={shortcut.enabled}
                label="启用"
                onChange={(checked) => {
                  update({
                    ...settings,
                    replies: {
                      ...settings.replies,
                      custom_shortcuts: updateShortcutAt(
                        settings.replies.custom_shortcuts,
                        index,
                        {
                          ...shortcut,
                          enabled: checked,
                        },
                      ),
                    },
                  });
                }}
              />
              <div className="shortcut-editor-actions">
                <SettingsActionButton
                  ariaLabel={`上移快捷输入：${shortcut.label}`}
                  disabled={index === 0}
                  iconName="shortcut-move-up"
                  onClick={() => {
                    update({
                      ...settings,
                      replies: {
                        ...settings.replies,
                        custom_shortcuts: moveShortcut(
                          settings.replies.custom_shortcuts,
                          index,
                          index - 1,
                        ),
                      },
                    });
                  }}
                />
                <SettingsActionButton
                  ariaLabel={`下移快捷输入：${shortcut.label}`}
                  disabled={
                    index === settings.replies.custom_shortcuts.length - 1
                  }
                  iconName="shortcut-move-down"
                  onClick={() => {
                    update({
                      ...settings,
                      replies: {
                        ...settings.replies,
                        custom_shortcuts: moveShortcut(
                          settings.replies.custom_shortcuts,
                          index,
                          index + 1,
                        ),
                      },
                    });
                  }}
                />
                <SettingsActionButton
                  ariaLabel={`删除快捷输入：${shortcut.label}`}
                  iconName="shortcut-delete"
                  onClick={() => {
                    update({
                      ...settings,
                      replies: {
                        ...settings.replies,
                        custom_shortcuts:
                          settings.replies.custom_shortcuts.filter(
                            (item) => item.id !== shortcut.id,
                          ),
                      },
                    });
                  }}
                />
              </div>
            </div>
          ))}
          <SettingsActionButton
            ariaLabel="新增快捷输入"
            iconName="shortcut-add"
            onClick={() => {
              update({
                ...settings,
                replies: {
                  ...settings.replies,
                  custom_shortcuts: [
                    ...settings.replies.custom_shortcuts,
                    {
                      id: nextShortcutId(settings.replies.custom_shortcuts),
                      label: "新快捷输入",
                      content: "继续。",
                      enabled: true,
                      order: nextShortcutOrder(
                        settings.replies.custom_shortcuts,
                      ),
                    },
                  ],
                },
              });
            }}
          >
            新增快捷输入
          </SettingsActionButton>
        </div>
      </SettingsGroup>
      <SettingsGroup title="Presets">
        <ToggleRow
          checked={settings.presets.prefer_structured_create}
          label="结构化创建优先"
          onChange={(checked) => {
            update({
              ...settings,
              presets: {
                prefer_structured_create: checked,
              },
            });
          }}
        />
      </SettingsGroup>
      <SettingsGroup title="Terminal">
        <ToggleRow
          checked={settings.terminal.jump_enabled}
          label="跳回入口"
          onChange={(checked) => {
            update({
              ...settings,
              terminal: {
                ...settings.terminal,
                jump_enabled: checked,
              },
            });
          }}
        />
        <ToggleRow
          checked={settings.terminal.copy_fallback_enabled}
          label="复制降级"
          onChange={(checked) => {
            update({
              ...settings,
              terminal: {
                ...settings.terminal,
                copy_fallback_enabled: checked,
              },
            });
          }}
        />
      </SettingsGroup>
      <SettingsGroup title="Advanced">
        <ToggleRow
          checked={settings.advanced.developer_diagnostics}
          label="开发诊断"
          onChange={(checked) => {
            update({
              ...settings,
              advanced: {
                developer_diagnostics: checked,
              },
            });
          }}
        />
      </SettingsGroup>
      <SettingsGroup layout="stack" title="Logging">
        <ToggleRow
          checked={settings.logging.enabled}
          label="启用事件日志"
          onChange={(checked) => {
            update({
              ...settings,
              logging: {
                ...settings.logging,
                enabled: checked,
              },
            });
          }}
        />
        {logPath !== null && (
          <p className="settings-note settings-log-path" title={logPath}>
            日志文件：{logPath}
          </p>
        )}
        <div className="settings-log-actions">
          <SettingsActionButton
            ariaLabel="打开日志目录"
            iconName="log-folder"
            onClick={onOpenLogFolder}
          >
            打开日志目录
          </SettingsActionButton>
        </div>
      </SettingsGroup>
    </section>
  );
};

/// 从设置输入中读取 panel 最大窗口高度。
const panelMaxWindowHeightFromInput = (value: string): number => {
  const parsed = Number(value);
  if (
    !Number.isInteger(parsed) ||
    parsed < PANEL_WINDOW_MIN_CONFIGURED_MAX_HEIGHT
  ) {
    return PANEL_WINDOW_DEFAULT_MAX_HEIGHT;
  }

  return Math.min(parsed, PANEL_WINDOW_MAX_CONFIGURED_MAX_HEIGHT);
};

/// 更新指定快捷输入。
const updateShortcutAt = (
  shortcuts: BuilderPanelSettings["replies"]["custom_shortcuts"],
  index: number,
  shortcut: BuilderPanelSettings["replies"]["custom_shortcuts"][number],
): BuilderPanelSettings["replies"]["custom_shortcuts"] => {
  return shortcuts.map((item, itemIndex) =>
    itemIndex === index ? shortcut : item,
  );
};

/// 移动快捷输入并重排顺序。
const moveShortcut = (
  shortcuts: BuilderPanelSettings["replies"]["custom_shortcuts"],
  from: number,
  to: number,
): BuilderPanelSettings["replies"]["custom_shortcuts"] => {
  const next = [...shortcuts];
  const [item] = next.splice(from, 1);
  if (item === undefined) {
    return shortcuts;
  }
  next.splice(to, 0, item);
  return next.map((shortcut, index) => ({
    ...shortcut,
    order: (index + 1) * 10,
  }));
};

/// 返回下一条快捷输入 ID。
const nextShortcutId = (
  shortcuts: BuilderPanelSettings["replies"]["custom_shortcuts"],
): string => {
  return `custom-${shortcuts.length + 1}-${Date.now()}`;
};

/// 返回下一条快捷输入排序值。
const nextShortcutOrder = (
  shortcuts: BuilderPanelSettings["replies"]["custom_shortcuts"],
): number => {
  const maxOrder = shortcuts.reduce(
    (current, shortcut) => Math.max(current, shortcut.order),
    0,
  );
  return maxOrder + 10;
};

/// 设置分组属性。
interface SettingsGroupProps {
  /// 分组标题。
  readonly title: string;
  /// 内容布局。
  readonly layout?: "grid" | "list" | "stack";
  /// 分组内容。
  readonly children: ReactNode;
}

/// 设置分组。
const SettingsGroup = ({
  title,
  layout = "grid",
  children,
}: SettingsGroupProps) => (
  <section className="settings-group">
    <h2>{title}</h2>
    <div className={`settings-group-content settings-group-content-${layout}`}>
      {children}
    </div>
  </section>
);

/// 设置动作按钮属性。
interface SettingsActionButtonProps {
  /// 可访问名称。
  readonly ariaLabel: string;
  /// 图标名称。
  readonly iconName: PanelIconName;
  /// 是否禁用。
  readonly disabled?: boolean;
  /// 按钮文字。
  readonly children?: ReactNode;
  /// 点击回调。
  readonly onClick: () => void;
}

/// 渲染设置页统一动作按钮。
const SettingsActionButton = ({
  ariaLabel,
  iconName,
  disabled = false,
  children,
  onClick,
}: SettingsActionButtonProps) => {
  const className =
    children === undefined
      ? "settings-action-button settings-action-button-icon"
      : "settings-action-button";

  return (
    <button
      aria-label={ariaLabel}
      className={className}
      disabled={disabled}
      title={ariaLabel}
      type="button"
      onClick={onClick}
    >
      <PanelIcon name={iconName} />
      {children !== undefined && <span>{children}</span>}
    </button>
  );
};

/// hook 安装行属性。
interface HookInstallRowProps {
  /// 当前 agent 状态。
  readonly status: HookInstallAgentStatus;
  /// 正在执行操作的 agent。
  readonly workingAgent: HookInstallAgent | null;
  /// 安装回调。
  readonly onInstall: (agent: HookInstallAgent) => void;
  /// 卸载回调。
  readonly onUninstall: (agent: HookInstallAgent) => void;
}

/// hook 安装单行。
const HookInstallRow = ({
  status,
  workingAgent,
  onInstall,
  onUninstall,
}: HookInstallRowProps) => {
  const busy = workingAgent === status.agent;
  const blocked = workingAgent !== null;
  const reasonText = status.reasons.join("；");

  return (
    <div className="hook-install-row" title={reasonText}>
      <div className="hook-install-copy">
        <strong>{hookInstallAgentLabel(status.agent)}</strong>
        <span>{busy ? "处理中" : status.message}</span>
      </div>
      <div className="hook-install-row-actions">
        <SettingsActionButton
          ariaLabel={`安装 ${status.agent} hook`}
          disabled={blocked || !status.can_install}
          iconName="hook-install"
          onClick={() => {
            onInstall(status.agent);
          }}
        >
          安装
        </SettingsActionButton>
        <SettingsActionButton
          ariaLabel={`卸载 ${status.agent} hook`}
          disabled={blocked || !status.can_uninstall}
          iconName="hook-uninstall"
          onClick={() => {
            onUninstall(status.agent);
          }}
        >
          卸载
        </SettingsActionButton>
      </div>
    </div>
  );
};

/// 返回 hook 安装行展示名。
const hookInstallAgentLabel = (agent: HookInstallAgent): string => {
  if (agent === "codex") {
    return "codex & codex cli";
  }

  return "Claude CLI hook";
};

/// 开关行属性。
interface ToggleRowProps {
  /// 是否选中。
  readonly checked: boolean;
  /// 是否禁用。
  readonly disabled?: boolean;
  /// 展示标签。
  readonly label: string;
  /// 变化回调。
  readonly onChange: (checked: boolean) => void;
}

/// 设置开关行。
const ToggleRow = ({
  checked,
  disabled = false,
  label,
  onChange,
}: ToggleRowProps) => (
  <label className={disabled ? "toggle-row toggle-row-disabled" : "toggle-row"}>
    <span>{label}</span>
    <input
      checked={checked}
      disabled={disabled}
      type="checkbox"
      onChange={(event) => {
        onChange(event.target.checked);
      }}
    />
  </label>
);
