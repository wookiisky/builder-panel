import { describe, expect, it } from "vitest";

import {
  panelAdaptiveWindowHeight,
  panelWindowHeightNeedsResize,
  type PanelWindowLogicalGeometry,
} from "./panelAdaptiveSizing";

const geometry = (
  windowTop: number,
  workAreaTop: number,
  workAreaHeight: number,
): PanelWindowLogicalGeometry => ({
  windowPosition: { x: 40, y: windowTop },
  windowSize: { width: 665, height: 120 },
  workArea: {
    position: { x: -1440, y: workAreaTop },
    size: { width: 1440, height: workAreaHeight },
  },
});

describe("panelAdaptiveSizing", () => {
  it("uses the natural content height while it remains inside all limits", () => {
    expect(
      panelAdaptiveWindowHeight({
        configuredMaxHeight: 400,
        contentHeight: 188.2,
        geometry: geometry(100, 0, 900),
      }),
    ).toBe(189);
  });

  it("clamps long content to the configured maximum", () => {
    expect(
      panelAdaptiveWindowHeight({
        configuredMaxHeight: 400,
        contentHeight: 900,
        geometry: geometry(100, 0, 1200),
      }),
    ).toBe(400);
  });

  it("uses the current monitor work area with negative multi-screen coordinates", () => {
    expect(
      panelAdaptiveWindowHeight({
        configuredMaxHeight: 2000,
        contentHeight: 1200,
        geometry: geometry(-800, -900, 900),
      }),
    ).toBe(752);
  });

  it("falls back to the configured maximum when the current monitor is unavailable", () => {
    expect(
      panelAdaptiveWindowHeight({
        configuredMaxHeight: 400,
        contentHeight: 900,
        geometry: {
          ...geometry(100, 0, 1200),
          workArea: null,
        },
      }),
    ).toBe(400);
  });

  it("compares the target with the actual window height instead of a cached target", () => {
    expect(panelWindowHeightNeedsResize(220, 128)).toBe(true);
    expect(panelWindowHeightNeedsResize(129, 128)).toBe(false);
  });
});
