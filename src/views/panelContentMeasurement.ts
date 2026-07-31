/// panel 内容高度组成。
export interface PanelWindowContentHeightParts {
  /// 外层垂直 padding。
  readonly surfacePaddingBlock: number;
  /// panel 外壳垂直边框。
  readonly shellBorderBlock: number;
  /// 标题栏高度。
  readonly titlebarHeight: number;
  /// 内容层自然高度。
  readonly contentNaturalHeight: number;
}

/// 计算不受当前视口裁剪的 panel 内容高度。
export const panelWindowContentHeightFromParts = ({
  contentNaturalHeight,
  shellBorderBlock,
  surfacePaddingBlock,
  titlebarHeight,
}: PanelWindowContentHeightParts): number => {
  return (
    surfacePaddingBlock +
    shellBorderBlock +
    titlebarHeight +
    contentNaturalHeight
  );
};

/// 测量当前 panel 完整自然内容高度。
export const measurePanelWindowContentHeight = (
  surface: HTMLElement,
): number | null => {
  const shell = surface.querySelector<HTMLElement>(".panel-shell");
  const titlebar = surface.querySelector<HTMLElement>(".panel-drag-region");
  const naturalContent = surface.querySelector<HTMLElement>(
    ".panel-natural-content",
  );
  if (shell === null || titlebar === null || naturalContent === null) {
    return null;
  }

  const surfaceStyle = window.getComputedStyle(surface);
  const shellStyle = window.getComputedStyle(shell);
  const normalContentHeight = Math.max(
    naturalContent.scrollHeight,
    naturalContent.getBoundingClientRect().height,
  );
  const contentNaturalHeight = Math.max(
    normalContentHeight,
    panelOverlayNaturalHeight(surface),
  );

  return panelWindowContentHeightFromParts({
    contentNaturalHeight,
    shellBorderBlock:
      numberFromCssPixels(shellStyle.borderTopWidth) +
      numberFromCssPixels(shellStyle.borderBottomWidth),
    surfacePaddingBlock:
      numberFromCssPixels(surfaceStyle.paddingTop) +
      numberFromCssPixels(surfaceStyle.paddingBottom),
    titlebarHeight: titlebar.getBoundingClientRect().height,
  });
};

/// 注册会影响 panel 窗口高度的可见元素。
export const observePanelContentResizeTargets = (
  surface: HTMLElement,
  observer: ResizeObserver | null,
): void => {
  if (observer === null) {
    return;
  }

  observer.observe(surface);
  const naturalContent = surface.querySelector<HTMLElement>(
    ".panel-natural-content",
  );
  if (naturalContent !== null) {
    observer.observe(naturalContent);
  }
  surface
    .querySelectorAll<HTMLElement>(".overlay-panel")
    .forEach((overlayPanel) => {
      observer.observe(overlayPanel);
    });
};

/// 监听动态内容与 overlay 的挂载、卸载。
export const observePanelContentMutations = (
  surface: HTMLElement,
  onMutation: () => void,
): MutationObserver | null => {
  if (typeof MutationObserver === "undefined") {
    return null;
  }

  const observer = new MutationObserver(() => {
    onMutation();
  });
  observer.observe(surface, {
    childList: true,
    subtree: true,
  });
  return observer;
};

/// 测量当前打开 overlay 的自然高度。
const panelOverlayNaturalHeight = (surface: HTMLElement): number => {
  const overlays = [
    ...surface.querySelectorAll<HTMLElement>(".overlay-backdrop"),
  ];
  if (overlays.length === 0) {
    return 0;
  }

  return Math.max(...overlays.map(panelOverlayBackdropNaturalHeight));
};

/// 测量单个 overlay 背景内面板需要的高度。
const panelOverlayBackdropNaturalHeight = (backdrop: HTMLElement): number => {
  const backdropStyle = window.getComputedStyle(backdrop);
  const backdropPaddingBlock =
    numberFromCssPixels(backdropStyle.paddingTop) +
    numberFromCssPixels(backdropStyle.paddingBottom);
  const panels = [...backdrop.querySelectorAll<HTMLElement>(".overlay-panel")];
  if (panels.length === 0) {
    return backdropPaddingBlock;
  }

  return (
    backdropPaddingBlock +
    Math.max(...panels.map(panelOverlayPanelNaturalHeight))
  );
};

/// 测量 overlay 面板的完整滚动高度。
const panelOverlayPanelNaturalHeight = (panel: HTMLElement): number => {
  const panelStyle = window.getComputedStyle(panel);
  const panelHeight = Math.max(
    panel.scrollHeight,
    panel.getBoundingClientRect().height,
  );

  return (
    panelHeight +
    numberFromCssPixels(panelStyle.marginTop) +
    numberFromCssPixels(panelStyle.marginBottom)
  );
};

/// 读取 CSS 像素数。
const numberFromCssPixels = (value: string): number => {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed)) {
    return 0;
  }

  return parsed;
};
