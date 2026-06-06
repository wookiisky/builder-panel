import type { ReactNode } from "react";

import type {
  HookInstallAgent,
  HookInstallPreview,
} from "../api/hookInstallApi";
import type { BuilderPanelSettings, UiDensity } from "../api/settingsContract";

/// hook 安装 UI 状态。
export interface HookInstallPanelState {
  /// 当前选择的安装目标。
  readonly selectedAgents: readonly HookInstallAgent[];
  /// 当前预览对应的安装目标。
  readonly previewAgents: readonly HookInstallAgent[] | null;
  /// 当前预览结果。
  readonly preview: HookInstallPreview | null;
  /// 当前 hook 操作状态提示。
  readonly statusMessage: string | null;
  /// 是否正在执行 hook 操作。
  readonly working: boolean;
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
  /// 设置变化回调。
  readonly onChange: (settings: BuilderPanelSettings) => void;
  /// hook agent 选择变化回调。
  readonly onToggleHookAgent: (agent: HookInstallAgent) => void;
  /// 预览 hook 安装回调。
  readonly onPreviewHookInstall: () => void;
  /// 安装 hook 回调。
  readonly onInstallHooks: () => void;
  /// 卸载 hook 回调。
  readonly onUninstallHooks: () => void;
}

/// Builder Panel 设置页。
export const SettingsPanel = ({
  settings,
  statusMessage,
  saving,
  hookInstall,
  onChange,
  onToggleHookAgent,
  onPreviewHookInstall,
  onInstallHooks,
  onUninstallHooks,
}: SettingsPanelProps) => {
  const update = (next: BuilderPanelSettings): void => {
    onChange(next);
  };

  return (
    <section className="settings-panel" aria-label="设置">
      <header>
        <div>
          <strong>设置</strong>
          <p>配置会立即保存，本轮不提供自动更新配置项。</p>
        </div>
        <span>{saving ? "保存中" : "已就绪"}</span>
      </header>
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
      </SettingsGroup>
      <SettingsGroup title="Agents">
        <ToggleRow
          checked={settings.agents.mock_agent_enabled}
          label="Mock Agent"
          onChange={(checked) => {
            update({
              ...settings,
              agents: {
                ...settings.agents,
                mock_agent_enabled: checked,
              },
            });
          }}
        />
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
          disabled={true}
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
      <SettingsGroup title="Hook Install">
        <ToggleRow
          checked={hookInstall.selectedAgents.includes("codex")}
          label="Codex CLI hook"
          onChange={() => {
            onToggleHookAgent("codex");
          }}
        />
        <ToggleRow
          checked={hookInstall.selectedAgents.includes("claude")}
          label="Claude CLI hook"
          onChange={() => {
            onToggleHookAgent("claude");
          }}
        />
        <div className="hook-actions">
          <button
            disabled={
              hookInstall.working || hookInstall.selectedAgents.length === 0
            }
            type="button"
            onClick={onPreviewHookInstall}
          >
            预览修改
          </button>
          <button
            disabled={
              hookInstall.working || !canInstallHooksFromPreview(hookInstall)
            }
            type="button"
            onClick={onInstallHooks}
          >
            安装
          </button>
          <button
            disabled={hookInstall.working}
            type="button"
            onClick={onUninstallHooks}
          >
            卸载
          </button>
        </div>
        {hookInstall.statusMessage !== null && (
          <p className="settings-status">{hookInstall.statusMessage}</p>
        )}
        {hookInstall.preview !== null && (
          <div className="hook-preview" aria-label="hook 安装预览">
            <PreviewList
              title="将修改"
              values={hookInstall.preview.files_to_modify}
            />
            <PreviewList
              title="将备份"
              values={hookInstall.preview.backup_files}
            />
            <PreviewList
              title="Manifest"
              values={[hookInstall.preview.manifest_path]}
            />
          </div>
        )}
      </SettingsGroup>
      <SettingsGroup title="Replies">
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
    </section>
  );
};

/// 设置分组属性。
interface SettingsGroupProps {
  /// 分组标题。
  readonly title: string;
  /// 分组内容。
  readonly children: ReactNode;
}

/// 设置分组。
const SettingsGroup = ({ title, children }: SettingsGroupProps) => (
  <section className="settings-group">
    <h2>{title}</h2>
    <div>{children}</div>
  </section>
);

/// 预览列表属性。
interface PreviewListProps {
  /// 标题。
  readonly title: string;
  /// 路径列表。
  readonly values: readonly string[];
}

/// hook 安装预览路径列表。
const PreviewList = ({ title, values }: PreviewListProps) => (
  <div>
    <strong>{title}</strong>
    <ul>
      {values.map((value) => (
        <li key={value}>{value}</li>
      ))}
    </ul>
  </div>
);

/// 判断当前预览是否允许安装。
const canInstallHooksFromPreview = (state: HookInstallPanelState): boolean => {
  if (state.preview === null || state.previewAgents === null) {
    return false;
  }

  return sameHookInstallAgents(state.previewAgents, state.selectedAgents);
};

/// 判断两个 hook agent 集合是否一致。
const sameHookInstallAgents = (
  left: readonly HookInstallAgent[],
  right: readonly HookInstallAgent[],
): boolean => {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((agent) => right.includes(agent));
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
