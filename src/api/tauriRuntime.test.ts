import { describe, expect, it, vi } from "vitest";

const tauriCoreMocks = vi.hoisted(() => ({
  isTauri: vi.fn<() => boolean>(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: tauriCoreMocks.isTauri,
}));

import { isTauriRuntime } from "./tauriRuntime";

describe("tauriRuntime", () => {
  it("returns true when official Tauri runtime marker is true", () => {
    tauriCoreMocks.isTauri.mockReturnValueOnce(true);

    expect(isTauriRuntime()).toBe(true);
  });

  it("returns false when official Tauri runtime marker is false", () => {
    tauriCoreMocks.isTauri.mockReturnValueOnce(false);

    expect(isTauriRuntime()).toBe(false);
  });
});
