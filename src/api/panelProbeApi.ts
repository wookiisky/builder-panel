import { invoke } from "@tauri-apps/api/core";

import type { PanelProbeView } from "./panelProbeContract";

/// 读取 Rust 侧基础 panel 探针状态。
export const fetchPanelProbe = async (): Promise<PanelProbeView> => {
  return await invoke<PanelProbeView>("get_panel_probe");
};
