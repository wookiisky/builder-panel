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
    return {
      entries: [...new Set(request.agents)].flatMap(fallbackManifestEntries),
    };
  }
};

/// 卸载 hook。
export const uninstallHooks = async (): Promise<void> => {
  try {
    await invoke<void>("uninstall_hooks");
  } catch (error) {
    if (isTauriRuntime()) {
      throw errorWithCause(error, "hook 卸载失败");
    }
  }
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
