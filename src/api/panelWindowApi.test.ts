import { afterEach, describe, expect, it, vi } from "vitest";

const tauriWindowMocks = vi.hoisted(() => ({
  close: vi.fn<() => Promise<void>>(),
  currentMonitor: vi.fn<
    () => Promise<{
      workArea: {
        position: { x: number; y: number };
        size: { width: number; height: number };
      };
    } | null>
  >(),
  getCurrentWindow: vi.fn(),
  innerSize: vi.fn<
    () => Promise<{
      toLogical: (scale: number) => { width: number; height: number };
    }>
  >(),
  isTauri: vi.fn<() => boolean>(),
  LogicalSize: vi.fn<(width: number, height: number) => unknown>(),
  minimize: vi.fn<() => Promise<void>>(),
  outerPosition: vi.fn<() => Promise<{ x: number; y: number }>>(),
  scaleFactor: vi.fn<() => Promise<number>>(),
  setAlwaysOnTop: vi.fn<(alwaysOnTop: boolean) => Promise<void>>(),
  setPosition: vi.fn<(position: unknown) => Promise<void>>(),
  setSize: vi.fn<(size: unknown) => Promise<void>>(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  isTauri: tauriWindowMocks.isTauri,
}));

vi.mock("@tauri-apps/api/window", () => ({
  currentMonitor: tauriWindowMocks.currentMonitor,
  getCurrentWindow: tauriWindowMocks.getCurrentWindow,
  LogicalSize: class LogicalSize {
    /// 逻辑宽度。
    readonly width: number;
    /// 逻辑高度。
    readonly height: number;

    /// 创建逻辑尺寸。
    constructor(width: number, height: number) {
      this.width = width;
      this.height = height;
      tauriWindowMocks.LogicalSize(width, height);
    }
  },
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
  applyPanelAlwaysOnTopPreference,
  applyPanelWindowPreferences,
  closePanelWindow,
  minimizePanelWindow,
  readPanelWindowLogicalGeometry,
  resizePanelWindowContentHeight,
} from "./panelWindowApi";

describe("panelWindowApi", () => {
  afterEach(() => {
    vi.clearAllMocks();
    tauriWindowMocks.isTauri.mockReturnValue(false);
  });

  it("ignores window commands outside Tauri runtime", async () => {
    await minimizePanelWindow();
    await closePanelWindow();
    await applyPanelWindowPreferences(defaultSettings());
    await applyPanelAlwaysOnTopPreference(defaultSettings());

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

    await applyPanelAlwaysOnTopPreference(settings);

    expect(tauriWindowMocks.setAlwaysOnTop).toHaveBeenCalledTimes(1);
    expect(tauriWindowMocks.setAlwaysOnTop).toHaveBeenCalledWith(false);
    expect(tauriWindowMocks.setSize).not.toHaveBeenCalled();
    expect(tauriWindowMocks.setPosition).not.toHaveBeenCalled();
  });

  it("applies always-on-top and saved geometry together", async () => {
    enableTauriRuntime();
    tauriWindowMocks.getCurrentWindow.mockReturnValue(currentWindowMock());
    mockCurrentLogicalHeight(180);
    const settings = {
      ...defaultSettings(),
      panel: {
        ...defaultSettings().panel,
        collapsed: false,
        window_position: { x: 12, y: 20 },
        window_size: null,
        window_width: 860,
      },
    };

    await applyPanelWindowPreferences(settings);

    expect(tauriWindowMocks.setAlwaysOnTop).toHaveBeenCalledWith(true);
    expect(tauriWindowMocks.setSize).toHaveBeenCalledWith({
      width: 860,
      height: 180,
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
    mockCurrentLogicalHeight(180);
    const settings = {
      ...defaultSettings(),
      general: {
        ...defaultSettings().general,
        keep_panel_on_top: false,
      },
      panel: {
        ...defaultSettings().panel,
        collapsed: false,
        window_position: { x: 12, y: 20 },
        window_size: null,
        window_width: 860,
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
      height: 180,
    });
    expect(tauriWindowMocks.setPosition).toHaveBeenCalledWith({
      x: 12,
      y: 20,
    });
  });

  it("reads window and monitor physical geometry into one logical coordinate system", async () => {
    enableTauriRuntime();
    tauriWindowMocks.getCurrentWindow.mockReturnValue(currentWindowMock());
    mockCurrentLogicalSize(665, 120);
    tauriWindowMocks.outerPosition.mockResolvedValue({ x: -2800, y: -1600 });
    tauriWindowMocks.currentMonitor.mockResolvedValue({
      workArea: {
        position: { x: -2880, y: -1800 },
        size: { width: 2880, height: 1800 },
      },
    });

    const geometry = await readPanelWindowLogicalGeometry();

    expect(geometry).toEqual({
      windowPosition: { x: -1400, y: -800 },
      windowSize: { width: 665, height: 120 },
      workArea: {
        position: { x: -1440, y: -900 },
        size: { width: 1440, height: 900 },
      },
    });
  });

  it("preserves the actual logical width when resizing content height", async () => {
    enableTauriRuntime();
    tauriWindowMocks.getCurrentWindow.mockReturnValue(currentWindowMock());
    mockCurrentLogicalSize(665, 220);

    await resizePanelWindowContentHeight(128);

    expect(
      Object.prototype.hasOwnProperty.call(window, "__TAURI_INTERNALS__"),
    ).toBe(false);
    expect(tauriWindowMocks.setSize).toHaveBeenCalledWith({
      width: 665,
      height: 128,
    });
  });

  it("does not resize to content outside Tauri runtime", async () => {
    await resizePanelWindowContentHeight(148);

    expect(tauriWindowMocks.getCurrentWindow).not.toHaveBeenCalled();
  });
});

/// 模拟 Tauri 运行时标记。
const enableTauriRuntime = (): void => {
  tauriWindowMocks.isTauri.mockReturnValue(true);
};

/// 创建当前窗口 mock。
const currentWindowMock = () => ({
  close: tauriWindowMocks.close,
  innerSize: tauriWindowMocks.innerSize,
  minimize: tauriWindowMocks.minimize,
  outerPosition: tauriWindowMocks.outerPosition,
  scaleFactor: tauriWindowMocks.scaleFactor,
  setAlwaysOnTop: tauriWindowMocks.setAlwaysOnTop,
  setPosition: tauriWindowMocks.setPosition,
  setSize: tauriWindowMocks.setSize,
});

/// 模拟当前窗口逻辑高度。
const mockCurrentLogicalHeight = (height: number): void => {
  mockCurrentLogicalSize(860, height);
};

/// 模拟当前窗口逻辑尺寸。
const mockCurrentLogicalSize = (width: number, height: number): void => {
  tauriWindowMocks.scaleFactor.mockResolvedValue(2);
  tauriWindowMocks.innerSize.mockResolvedValue({
    toLogical: () => ({ height, width }),
  });
};
