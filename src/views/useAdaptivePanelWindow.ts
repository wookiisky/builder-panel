import { useCallback, useLayoutEffect, useRef, type RefObject } from "react";

import {
  readPanelWindowLogicalGeometry,
  resizePanelWindowContentHeight,
} from "../api/panelWindowApi";
import { createPanelAdaptiveResizeController } from "../stores/panelAdaptiveResizeController";
import {
  panelAdaptiveWindowHeight,
  panelWindowHeightNeedsResize,
} from "../stores/panelAdaptiveSizing";
import {
  measurePanelWindowContentHeight,
  observePanelContentMutations,
  observePanelContentResizeTargets,
} from "./panelContentMeasurement";

/// panel 自适应窗口 hook 输入。
export interface AdaptivePanelWindowOptions {
  /// 窗口偏好是否已经恢复完毕。
  readonly enabled: boolean;
  /// 当前用户配置的窗口最大高度。
  readonly configuredMaxHeight: number;
  /// 会影响内容高度的稳定签名。
  readonly contentSignature: string;
  /// overlay 观察目标变化版本。
  readonly overlayOpen: boolean;
  /// panel 根元素引用。
  readonly surfaceRef: RefObject<HTMLElement | null>;
  /// 窗口 resize 错误收敛入口。
  readonly reportError: (error: unknown) => void;
}

/// panel 自适应窗口 hook 输出。
export interface AdaptivePanelWindowController {
  /// 请求按最新内容和真实窗口几何重新调整高度。
  readonly requestResize: () => void;
}

/// 根据自然内容持续收敛 panel 真实窗口高度。
export const useAdaptivePanelWindow = ({
  configuredMaxHeight,
  contentSignature,
  enabled,
  overlayOpen,
  reportError,
  surfaceRef,
}: AdaptivePanelWindowOptions): AdaptivePanelWindowController => {
  const configuredMaxHeightRef = useRef(configuredMaxHeight);
  const reportErrorRef = useRef(reportError);
  const resizeFrameRef = useRef<number | null>(null);
  const resizeControllerRef = useRef<ReturnType<
    typeof createPanelAdaptiveResizeController
  > | null>(null);
  configuredMaxHeightRef.current = configuredMaxHeight;
  reportErrorRef.current = reportError;

  const resizeToLatestContent = useCallback(async (): Promise<void> => {
    const surface = surfaceRef.current;
    if (surface === null) {
      return;
    }

    const contentHeight = measurePanelWindowContentHeight(surface);
    if (contentHeight === null) {
      return;
    }

    const geometry = await readPanelWindowLogicalGeometry();
    if (geometry === null) {
      return;
    }

    const targetHeight = panelAdaptiveWindowHeight({
      configuredMaxHeight: configuredMaxHeightRef.current,
      contentHeight,
      geometry,
    });
    if (
      !panelWindowHeightNeedsResize(geometry.windowSize.height, targetHeight)
    ) {
      return;
    }

    await resizePanelWindowContentHeight(targetHeight);
  }, [surfaceRef]);

  const requestResize = useCallback((): void => {
    if (!enabled) {
      return;
    }
    if (resizeFrameRef.current !== null) {
      window.cancelAnimationFrame(resizeFrameRef.current);
    }

    resizeFrameRef.current = window.requestAnimationFrame(() => {
      resizeFrameRef.current = null;
      resizeControllerRef.current?.request();
    });
  }, [enabled]);

  useLayoutEffect(() => {
    if (!enabled) {
      return;
    }

    const controller = createPanelAdaptiveResizeController({
      reportError: (error) => {
        reportErrorRef.current(error);
      },
      resize: resizeToLatestContent,
    });
    resizeControllerRef.current = controller;
    requestResize();

    return () => {
      controller.dispose();
      resizeControllerRef.current = null;
      if (resizeFrameRef.current !== null) {
        window.cancelAnimationFrame(resizeFrameRef.current);
        resizeFrameRef.current = null;
      }
    };
  }, [enabled, requestResize, resizeToLatestContent]);

  useLayoutEffect(() => {
    if (!enabled) {
      return;
    }
    const surface = surfaceRef.current;
    if (surface === null) {
      return;
    }

    const observer =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(() => {
            requestResize();
          });
    observePanelContentResizeTargets(surface, observer);
    const mutationObserver = observePanelContentMutations(surface, () => {
      observePanelContentResizeTargets(surface, observer);
      requestResize();
    });
    window.addEventListener("resize", requestResize);
    requestResize();

    return () => {
      observer?.disconnect();
      mutationObserver?.disconnect();
      window.removeEventListener("resize", requestResize);
    };
  }, [enabled, overlayOpen, requestResize, surfaceRef]);

  useLayoutEffect(() => {
    requestResize();
  }, [contentSignature, requestResize]);

  return { requestResize };
};
