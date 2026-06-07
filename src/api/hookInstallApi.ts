import { invoke } from "@tauri-apps/api/core";

/// hook 安装目标 agent。
export type HookInstallAgent = "codex" | "claude";

/// hook 安装请求。
export interface HookInstallRequest {
  /// 目标 agent 集合。
  readonly agents: readonly HookInstallAgent[];
}

/// hook 安装预览。
export interface HookInstallPreview {
  /// 将被修改的配置文件。
  readonly files_to_modify: readonly string[];
  /// 将被创建的备份文件。
  readonly backup_files: readonly string[];
  /// 将被写入的 manifest 文件。
  readonly manifest_path: string;
}

/// hook 安装状态类型。
export type HookInstallStateKind =
  | "not_installed"
  | "installed"
  | "partial"
  | "error";

/// 单个 agent hook 安装状态。
export interface HookInstallAgentStatus {
  /// 目标 agent。
  readonly agent: HookInstallAgent;
  /// 当前状态。
  readonly state: HookInstallStateKind;
  /// 用户可读状态文案。
  readonly message: string;
  /// 状态原因。
  readonly reasons: readonly string[];
  /// 当前是否允许安装。
  readonly can_install: boolean;
  /// 当前是否允许卸载。
  readonly can_uninstall: boolean;
}

/// hook 安装总状态。
export interface HookInstallStatus {
  /// 各 agent hook 安装状态。
  readonly agents: readonly HookInstallAgentStatus[];
}

/// hook 安装 manifest 单项。
export interface HookInstallManifestEntry {
  /// 已安装 agent。
  readonly agent: HookInstallAgent;
  /// 被修改的配置文件路径。
  readonly config_path: string;
  /// 修改前配置是否存在。
  readonly existed_before_install: boolean;
  /// 修改前备份文件路径。
  readonly backup_path: string;
}

/// hook 安装 manifest。
export interface HookInstallManifest {
  /// 已安装的 agent 配置记录。
  readonly entries: readonly HookInstallManifestEntry[];
}

/// 查询 hook 安装状态。
export const getHookInstallStatus = async (): Promise<HookInstallStatus> => {
  try {
    return await invoke<HookInstallStatus>("get_hook_install_status");
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "hook 状态读取失败");
    }
    return fallbackStatus;
  }
};

/// 预览 hook 安装。
export const previewHookInstall = async (
  request: HookInstallRequest,
): Promise<HookInstallPreview> => {
  try {
    return await invoke<HookInstallPreview>("preview_hook_install", {
      request,
    });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "hook 安装预览失败");
    }
    return fallbackPreview(request.agents);
  }
};

/// 安装 hook。
export const installHooks = async (
  request: HookInstallRequest,
): Promise<HookInstallManifest> => {
  try {
    return await invoke<HookInstallManifest>("install_hooks", {
      request,
    });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "hook 安装失败");
    }
    const manifest = {
      entries: [...new Set(request.agents)].flatMap(fallbackManifestEntries),
    };
    fallbackStatus = updateFallbackStatus(request.agents, "installed");
    return manifest;
  }
};

/// 卸载 hook。
export const uninstallHooks = async (
  request: HookInstallRequest,
): Promise<void> => {
  try {
    await invoke<void>("uninstall_hooks", { request });
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "hook 卸载失败");
    }
    fallbackStatus = updateFallbackStatus(request.agents, "not_installed");
  }
};

/// 浏览器开发环境的 hook 状态。
let fallbackStatus: HookInstallStatus = {
  agents: [
    fallbackAgentStatus("codex", "not_installed"),
    fallbackAgentStatus("claude", "not_installed"),
  ],
};

/// 浏览器开发环境的 hook 预览。
const fallbackPreview = (
  agents: readonly HookInstallAgent[],
): HookInstallPreview => {
  const uniqueAgents = [...new Set(agents)];
  return {
    files_to_modify: uniqueAgents.flatMap(fallbackConfigPaths),
    backup_files: uniqueAgents.flatMap((agent) =>
      fallbackConfigPaths(agent).map((path) => `${path}.builder-panel.bak`),
    ),
    manifest_path: "~/.config/builder-panel/hook-install-manifest.json",
  };
};

/// 返回浏览器 fallback 的配置路径。
const fallbackConfigPaths = (agent: HookInstallAgent): readonly string[] => {
  if (agent === "codex") {
    return ["~/.codex/hooks.json", "~/.codex/config.toml"];
  }

  return ["~/.claude/settings.json"];
};

/// 返回浏览器 fallback 的 manifest entries。
const fallbackManifestEntries = (
  agent: HookInstallAgent,
): readonly HookInstallManifestEntry[] => {
  return fallbackConfigPaths(agent).map((path) => ({
    agent,
    config_path: path,
    existed_before_install: false,
    backup_path: `${path}.builder-panel.bak`,
  }));
};

/// 返回浏览器 fallback 的单 agent 状态。
function fallbackAgentStatus(
  agent: HookInstallAgent,
  state: HookInstallStateKind,
): HookInstallAgentStatus {
  const installed = state === "installed";
  return {
    agent,
    state,
    message: installed ? "已安装" : "未安装",
    reasons: [],
    can_install: !installed,
    can_uninstall: installed,
  };
}

/// 更新浏览器 fallback hook 状态。
function updateFallbackStatus(
  agents: readonly HookInstallAgent[],
  state: HookInstallStateKind,
): HookInstallStatus {
  const targets = new Set(agents);
  return {
    agents: fallbackStatus.agents.map((status) =>
      targets.has(status.agent)
        ? fallbackAgentStatus(status.agent, state)
        : status,
    ),
  };
}

/// 创建保留 cause 的错误。
const errorWithCause = (error: unknown, fallback: string): Error => {
  if (error instanceof Error && error.message.length > 0) {
    return new Error(error.message, { cause: error });
  }
  if (typeof error === "string" && error.length > 0) {
    return new Error(error, { cause: error });
  }

  return new Error(fallback, { cause: error });
};

/// 判断当前是否运行在 Tauri 环境。
const isTauriRuntime = (): boolean => {
  return "__TAURI_INTERNALS__" in window;
};
