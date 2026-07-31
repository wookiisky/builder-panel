import { isTauri } from "@tauri-apps/api/core";

/// 判断当前是否运行在 Tauri 环境。
export const isTauriRuntime = (): boolean => {
  return isTauri();
};
