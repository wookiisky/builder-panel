import { describe, expect, it } from "vitest";

import {
  measurePanelWindowContentHeight,
  observePanelContentMutations,
  observePanelContentResizeTargets,
  panelWindowContentHeightFromParts,
} from "./panelContentMeasurement";

describe("panelContentMeasurement", () => {
  it("measures the natural content layer independently from the viewport height", () => {
    const elements = createPanelElements();
    elements.surface.style.paddingTop = "6px";
    elements.surface.style.paddingBottom = "6px";
    elements.shell.style.borderTopWidth = "1px";
    elements.shell.style.borderBottomWidth = "1px";
    defineElementHeight(elements.titlebar, 30);
    defineElementHeight(elements.viewport, 40);
    defineElementHeight(elements.naturalContent, 96);

    expect(measurePanelWindowContentHeight(elements.surface)).toBe(140);
  });

  it("uses an opened overlay as an additional visible height candidate", () => {
    const elements = createPanelElements();
    const overlay = document.createElement("div");
    const overlayPanel = document.createElement("section");
    overlay.className = "overlay-backdrop";
    overlayPanel.className = "overlay-panel settings-modal";
    overlay.style.paddingTop = "12px";
    overlay.style.paddingBottom = "12px";
    elements.surface.style.paddingTop = "6px";
    elements.surface.style.paddingBottom = "6px";
    elements.shell.style.borderTopWidth = "1px";
    elements.shell.style.borderBottomWidth = "1px";
    defineElementHeight(elements.titlebar, 30);
    defineElementHeight(elements.naturalContent, 60);
    defineElementHeight(overlayPanel, 320);
    overlay.append(overlayPanel);
    elements.naturalContent.append(overlay);

    expect(measurePanelWindowContentHeight(elements.surface)).toBe(388);
  });

  it("observes the natural content and opened overlay instead of the constrained viewport", () => {
    const elements = createPanelElements();
    const overlayPanel = document.createElement("section");
    const observed: Element[] = [];
    overlayPanel.className = "overlay-panel settings-modal";
    elements.naturalContent.append(overlayPanel);
    const observer = {
      observe: (element: Element) => {
        observed.push(element);
      },
    } as ResizeObserver;

    observePanelContentResizeTargets(elements.surface, observer);

    expect(observed).toEqual([
      elements.surface,
      elements.naturalContent,
      overlayPanel,
    ]);
  });

  it("reports dynamically mounted and removed child overlays", async () => {
    const elements = createPanelElements();
    let mutationCount = 0;
    const observer = observePanelContentMutations(elements.surface, () => {
      mutationCount += 1;
    });
    const overlay = document.createElement("div");
    overlay.className = "overlay-backdrop";

    elements.naturalContent.append(overlay);
    await nextMutationTurn();
    overlay.remove();
    await nextMutationTurn();
    observer?.disconnect();

    expect(mutationCount).toBe(2);
  });

  it("combines window chrome and natural content height", () => {
    expect(
      panelWindowContentHeightFromParts({
        contentNaturalHeight: 420,
        shellBorderBlock: 2,
        surfacePaddingBlock: 12,
        titlebarHeight: 30,
      }),
    ).toBe(464);
  });
});

/// 创建 panel 高度测量所需的最小 DOM 结构。
const createPanelElements = () => {
  const surface = document.createElement("main");
  const shell = document.createElement("section");
  const titlebar = document.createElement("header");
  const viewport = document.createElement("div");
  const naturalContent = document.createElement("div");
  surface.className = "app-surface";
  shell.className = "panel-shell";
  titlebar.className = "panel-drag-region";
  viewport.className = "panel-content";
  naturalContent.className = "panel-natural-content";
  viewport.append(naturalContent);
  shell.append(titlebar, viewport);
  surface.append(shell);

  return { naturalContent, shell, surface, titlebar, viewport };
};

/// 固定测试元素的布局高度与滚动高度。
const defineElementHeight = (element: HTMLElement, height: number): void => {
  Object.defineProperty(element, "scrollHeight", {
    configurable: true,
    value: height,
  });
  element.getBoundingClientRect = () =>
    ({
      bottom: height,
      height,
      left: 0,
      right: 100,
      top: 0,
      width: 100,
      x: 0,
      y: 0,
    }) as DOMRect;
};

/// 等待 jsdom 交付 MutationObserver 回调。
const nextMutationTurn = async (): Promise<void> => {
  await new Promise<void>((resolve) => {
    window.setTimeout(resolve, 0);
  });
};
