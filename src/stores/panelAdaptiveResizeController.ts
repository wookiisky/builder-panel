/// 自适应窗口 resize controller 依赖。
export interface PanelAdaptiveResizeControllerOptions {
  /// 执行一次读取最新状态的 resize 流程。
  readonly resize: () => Promise<void>;
  /// 收敛单次 resize 错误。
  readonly reportError: (error: unknown) => void;
}

/// 串行自适应窗口 resize controller。
export interface PanelAdaptiveResizeController {
  /// 请求执行一次最新 resize 流程。
  readonly request: () => void;
  /// 释放 controller 并丢弃尚未开始的工作。
  readonly dispose: () => void;
  /// 等待当前 resize 队列空闲。
  readonly whenIdle: () => Promise<void>;
}

/// 创建只串行执行副作用并合并中间请求的 resize controller。
export const createPanelAdaptiveResizeController = ({
  reportError,
  resize,
}: PanelAdaptiveResizeControllerOptions): PanelAdaptiveResizeController => {
  let disposed = false;
  let requestedVersion = 0;
  let running = false;
  let idleWaiters: Array<() => void> = [];

  const resolveIdleWaiters = (): void => {
    const waiters = idleWaiters;
    idleWaiters = [];
    waiters.forEach((resolve) => {
      resolve();
    });
  };

  const drain = async (): Promise<void> => {
    running = true;

    while (!disposed) {
      const activeVersion = requestedVersion;
      try {
        await resize();
      } catch (error) {
        reportError(error);
      }

      if (activeVersion === requestedVersion) {
        break;
      }
    }

    running = false;
    resolveIdleWaiters();
  };

  return {
    request: () => {
      if (disposed) {
        return;
      }

      requestedVersion += 1;
      if (!running) {
        void drain();
      }
    },
    dispose: () => {
      disposed = true;
      if (!running) {
        resolveIdleWaiters();
      }
    },
    whenIdle: () => {
      if (!running) {
        return Promise.resolve();
      }

      return new Promise<void>((resolve) => {
        idleWaiters.push(resolve);
      });
    },
  };
};
