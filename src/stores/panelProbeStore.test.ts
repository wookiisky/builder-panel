import { describe, expect, it } from "vitest";

import {
  createDefaultMockPanelUiState,
  sessionKeyToId,
  updateDraft,
} from "./mockPanelStore";
import type { SessionKey } from "../api/mockPanelContract";
import {
  createDefaultPanelUiState,
  togglePanelCollapsed,
} from "./panelProbeStore";

describe("panelProbeStore", () => {
  it("默认保持展开", () => {
    const state = createDefaultPanelUiState();

    expect(state.collapsed).toBe(false);
  });

  it("切换收缩状态时不修改原对象", () => {
    const state = createDefaultPanelUiState();
    const nextState = togglePanelCollapsed(state);

    expect(state.collapsed).toBe(false);
    expect(nextState.collapsed).toBe(true);
  });

  it("收缩状态切换不清理 session 草稿", () => {
    const key: SessionKey = {
      agent_kind: "codex_cli",
      project_id: { value: "project-a" },
      conversation_id: { value: "conversation-a" },
    };
    const mockState = updateDraft(
      createDefaultMockPanelUiState(),
      key,
      "保留草稿",
    );
    const panelState = togglePanelCollapsed(createDefaultPanelUiState());

    expect(panelState.collapsed).toBe(true);
    expect(mockState.draftsBySessionId[sessionKeyToId(key)]).toBe("保留草稿");
  });
});
