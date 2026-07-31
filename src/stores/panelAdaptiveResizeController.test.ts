import { describe, expect, it, vi } from "vitest";

import { createPanelAdaptiveResizeController } from "./panelAdaptiveResizeController";

describe("panelAdaptiveResizeController", () => {
  it("serializes resize effects and coalesces pending requests to the latest pass", async () => {
    const firstResize = deferred<void>();
    const resize = vi
      .fn<() => Promise<void>>()
      .mockReturnValueOnce(firstResize.promise)
      .mockResolvedValue(undefined);
    const controller = createPanelAdaptiveResizeController({
      reportError: vi.fn(),
      resize,
    });

    controller.request();
    await Promise.resolve();
    controller.request();
    controller.request();

    expect(resize).toHaveBeenCalledTimes(1);

    firstResize.resolve();
    await controller.whenIdle();

    expect(resize).toHaveBeenCalledTimes(2);
  });

  it("does not poison later requests when one resize fails", async () => {
    const failure = new Error("setSize failed");
    const reportError = vi.fn();
    const resize = vi
      .fn<() => Promise<void>>()
      .mockRejectedValueOnce(failure)
      .mockResolvedValue(undefined);
    const controller = createPanelAdaptiveResizeController({
      reportError,
      resize,
    });

    controller.request();
    await controller.whenIdle();
    controller.request();
    await controller.whenIdle();

    expect(resize).toHaveBeenCalledTimes(2);
    expect(reportError).toHaveBeenCalledWith(failure);
  });

  it("drops queued work after disposal", async () => {
    const firstResize = deferred<void>();
    const resize = vi
      .fn<() => Promise<void>>()
      .mockReturnValue(firstResize.promise);
    const controller = createPanelAdaptiveResizeController({
      reportError: vi.fn(),
      resize,
    });

    controller.request();
    await Promise.resolve();
    controller.request();
    controller.dispose();
    firstResize.resolve();
    await controller.whenIdle();

    expect(resize).toHaveBeenCalledTimes(1);
  });
});

/// 创建可由测试显式完成的 Promise。
const deferred = <T>() => {
  let resolvePromise: (value: T | PromiseLike<T>) => void = () => undefined;
  const promise = new Promise<T>((resolve) => {
    resolvePromise = resolve;
  });

  return {
    promise,
    resolve: resolvePromise,
  };
};
