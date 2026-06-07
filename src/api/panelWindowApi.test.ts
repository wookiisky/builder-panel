import { afterEach, describe, expect, it, vi } from "vitest";

const tauriWindowMocks = vi.hoisted(() => ({
  close: vi.fn<() => Promise<void>>(),
  getCurrentWindow: vi.fn(),
  minimize: vi.fn<() => Promise<void>>(),
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

import { closePanelWindow, minimizePanelWindow } from "./panelWindowApi";

describe("panelWindowApi", () => {
  afterEach(() => {
    delete (window as Window & { __TAURI_INTERNALS__?: unknown })
      .__TAURI_INTERNALS__;
    vi.clearAllMocks();
  });

  it("ignores window commands outside Tauri runtime", async () => {
    await minimizePanelWindow();
    await closePanelWindow();

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
});

/// 模拟 Tauri 运行时标记。
const enableTauriRuntime = (): void => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    configurable: true,
    value: {},
  });
};
