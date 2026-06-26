import { afterEach, describe, expect, it, vi } from "vitest";

const tauriWindowMocks = vi.hoisted(() => ({
  close: vi.fn<() => Promise<void>>(),
  getCurrentWindow: vi.fn(),
  minimize: vi.fn<() => Promise<void>>(),
  setAlwaysOnTop: vi.fn<(alwaysOnTop: boolean) => Promise<void>>(),
  setPosition: vi.fn<(position: unknown) => Promise<void>>(),
  setSize: vi.fn<(size: unknown) => Promise<void>>(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: tauriWindowMocks.getCurrentWindow,
  PhysicalPosition: class PhysicalPosition {
    /// 物理位置 x 坐标。
    readonly x: number;
    /// 物理位置 y 坐标。
    readonly y: number;

    /// 创建物理位置。
    constructor(x: number, y: number) {
      this.x = x;
      this.y = y;
    }
  },
  PhysicalSize: class PhysicalSize {
    /// 物理宽度。
    readonly width: number;
    /// 物理高度。
    readonly height: number;

    /// 创建物理尺寸。
    constructor(width: number, height: number) {
      this.width = width;
      this.height = height;
    }
  },
}));

import { defaultSettings } from "./settingsApi";
import {
  applyPanelWindowPreferences,
  closePanelWindow,
  minimizePanelWindow,
} from "./panelWindowApi";

describe("panelWindowApi", () => {
  afterEach(() => {
    delete (window as Window & { __TAURI_INTERNALS__?: unknown })
      .__TAURI_INTERNALS__;
    vi.clearAllMocks();
  });

  it("ignores window commands outside Tauri runtime", async () => {
    await minimizePanelWindow();
    await closePanelWindow();
    await applyPanelWindowPreferences(defaultSettings());

    expect(tauriWindowMocks.getCurrentWindow).not.toHaveBeenCalled();
  });

  it("minimizes the current Tauri window", async () => {
    enableTauriRuntime();
    tauriWindowMocks.getCurrentWindow.mockReturnValue({
      minimize: tauriWindowMocks.minimize,
    });

    await minimizePanelWindow();

    expect(tauriWindowMocks.getCurrentWindow).toHaveBeenCalledTimes(1);
    expect(tauriWindowMocks.minimize).toHaveBeenCalledTimes(1);
  });

  it("preserves the Tauri minimize failure cause", async () => {
    enableTauriRuntime();
    const failure = new Error("permission denied");
    tauriWindowMocks.minimize.mockRejectedValueOnce(failure);
    tauriWindowMocks.getCurrentWindow.mockReturnValue({
      minimize: tauriWindowMocks.minimize,
    });

    try {
      await minimizePanelWindow();
      throw new Error("expected minimize to fail");
    } catch (error) {
      expect(error).toBeInstanceOf(Error);
      expect((error as Error).message).toBe("permission denied");
      expect((error as Error).cause).toBe(failure);
    }
  });

  it("applies the saved always-on-top preference to the Tauri window", async () => {
    enableTauriRuntime();
    tauriWindowMocks.getCurrentWindow.mockReturnValue(currentWindowMock());
    const settings = {
      ...defaultSettings(),
      general: {
        ...defaultSettings().general,
        keep_panel_on_top: false,
      },
    };

    await applyPanelWindowPreferences(settings);

    expect(tauriWindowMocks.setAlwaysOnTop).toHaveBeenCalledTimes(1);
    expect(tauriWindowMocks.setAlwaysOnTop).toHaveBeenCalledWith(false);
  });

  it("applies always-on-top and saved geometry together", async () => {
    enableTauriRuntime();
    tauriWindowMocks.getCurrentWindow.mockReturnValue(currentWindowMock());
    const settings = {
      ...defaultSettings(),
      panel: {
        collapsed: false,
        window_position: { x: 12, y: 20 },
        window_size: { width: 860, height: 640 },
      },
    };

    await applyPanelWindowPreferences(settings);

    expect(tauriWindowMocks.setAlwaysOnTop).toHaveBeenCalledWith(true);
    expect(tauriWindowMocks.setSize).toHaveBeenCalledWith({
      width: 860,
      height: 640,
    });
    expect(tauriWindowMocks.setPosition).toHaveBeenCalledWith({
      x: 12,
      y: 20,
    });
  });

  it("keeps applying geometry when always-on-top fails", async () => {
    enableTauriRuntime();
    const failure = new Error("permission denied");
    tauriWindowMocks.setAlwaysOnTop.mockRejectedValueOnce(failure);
    tauriWindowMocks.getCurrentWindow.mockReturnValue(currentWindowMock());
    const settings = {
      ...defaultSettings(),
      general: {
        ...defaultSettings().general,
        keep_panel_on_top: false,
      },
      panel: {
        collapsed: false,
        window_position: { x: 12, y: 20 },
        window_size: { width: 860, height: 640 },
      },
    };

    try {
      await applyPanelWindowPreferences(settings);
      throw new Error("expected window preferences to fail");
    } catch (error) {
      expect(error).toBeInstanceOf(Error);
      expect((error as Error).message).toBe("permission denied");
      expect((error as Error).cause).toBe(failure);
    }
    expect(tauriWindowMocks.setSize).toHaveBeenCalledWith({
      width: 860,
      height: 640,
    });
    expect(tauriWindowMocks.setPosition).toHaveBeenCalledWith({
      x: 12,
      y: 20,
    });
  });
});

/// 模拟 Tauri 运行时标记。
const enableTauriRuntime = (): void => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    configurable: true,
    value: {},
  });
};

/// 创建当前窗口 mock。
const currentWindowMock = () => ({
  close: tauriWindowMocks.close,
  minimize: tauriWindowMocks.minimize,
  setAlwaysOnTop: tauriWindowMocks.setAlwaysOnTop,
  setPosition: tauriWindowMocks.setPosition,
  setSize: tauriWindowMocks.setSize,
});
