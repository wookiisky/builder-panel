import { readFileSync } from "node:fs";
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { describe, expect, it, vi } from "vitest";

import {
  actionLabel,
  applyLatestSavedPanelWindowPreferences,
  aggregateToolUsage,
  applyPanelWindowControlError,
  canToggleFollowupRow,
  canJumpOnSessionClick,
  canSelectOnSessionClick,
  countSessionsByStatus,
  createPanelSessionCaptureOrderStore,
  createSessionRefreshScheduler,
  elapsedDurationLabel,
  elementHasBlockOverflow,
  elementHasInlineOverflow,
  fetchSessionsForSource,
  handleTooltipPanelDoubleClick,
  isLatestSettingsSaveResponse,
  isCodexCliRuntime,
  isCodexAppRuntime,
  isPanelSessionUnfinishedForDisplay,
  defaultHookAgentStatuses,
  isTooltipLinkEventTarget,
  isHookActionDisabled,
  mergePanelSessionsByCaptureOrder,
  mergePanelWindowStateUpdate,
  mergePanelWindowStateIntoSettings,
  panelWindowContentResizeSignature,
  panelWindowUpdateForPersistence,
  PanelTitleActions,
  panelSessionToId,
  parseTooltipMarkdown,
  positionTooltipPanel,
  relativePastLabel,
  selectPanelSession,
  selectFirstSessionWhenMissing,
  SessionDetail,
  SessionStream,
  sessionActionRowClassName,
  shouldAutoShowSessionActionRow,
  shouldHoverExpandSessionActionRow,
  shouldShowInteractionSummary,
  shouldShowSessionActionRow,
  shouldSubmitReplyOnKeyDown,
  shouldUseFollowupShortcut,
  sourceTag,
  stopTooltipPortalEvent,
  sessionSideTimeLabel,
  textDisplayParagraph,
  type PanelSessionListItem,
} from "./BuilderPanelApp";
import type {
  SessionDetailViewModel,
  SessionKey,
  SessionStatus,
} from "../api/mockPanelContract";
import { defaultSettings } from "../api/settingsApi";
import {
  PanelIcon,
  PanelSessionStatusIcon,
  type PanelIconName,
} from "../components/PanelIcon";
import { PanelShell } from "../components/PanelShell";
import {
  createDefaultMockPanelUiState,
  sessionKeyToId,
} from "../stores/mockPanelStore";

const EXPECTED_ICON_VISUAL_STROKE_WIDTH = 1.5;

const codexSessionKey: SessionKey = {
  agent_kind: "codex_cli",
  project_id: { value: "/tmp/builder-panel" },
  conversation_id: { value: "codex-session-1" },
};

const codexAppSessionKey: SessionKey = {
  agent_kind: "codex_app",
  project_id: { value: "/tmp/builder-panel-app" },
  conversation_id: { value: "codex-app-session-1" },
};

const expectThinAbsoluteSvgStroke = (svg: SVGElement | null): void => {
  expect(svg).not.toBeNull();
  if (svg === null) {
    return;
  }

  const strokeWidth = Number(svg.getAttribute("stroke-width"));
  const width = Number(svg.getAttribute("width"));
  const visualStrokeWidth = (strokeWidth * width) / 24;

  expect(visualStrokeWidth).toBeCloseTo(EXPECTED_ICON_VISUAL_STROKE_WIDTH, 5);
};

describe("BuilderPanelApp session refresh", () => {
  it("selects a codex session that appears after the initial empty refresh", () => {
    const emptyState = createDefaultMockPanelUiState();
    const stateAfterEmptyRefresh = selectFirstSessionWhenMissing(
      emptyState,
      [],
    );

    const stateAfterCodexRefresh = selectFirstSessionWhenMissing(
      stateAfterEmptyRefresh,
      [sessionItem(codexSessionKey, "codex_cli")],
    );

    expect(stateAfterEmptyRefresh.selectedSessionId).toBeNull();
    expect(stateAfterCodexRefresh.selectedSessionId).toBe(
      panelSessionToId(sessionItem(codexSessionKey, "codex_cli")),
    );
  });

  it("selects a codex app session that appears after the initial empty refresh", () => {
    const emptyState = createDefaultMockPanelUiState();
    const stateAfterEmptyRefresh = selectFirstSessionWhenMissing(
      emptyState,
      [],
    );

    const stateAfterCodexAppRefresh = selectFirstSessionWhenMissing(
      stateAfterEmptyRefresh,
      [sessionItem(codexAppSessionKey, "codex_app")],
    );

    expect(stateAfterEmptyRefresh.selectedSessionId).toBeNull();
    expect(stateAfterCodexAppRefresh.selectedSessionId).toBe(
      panelSessionToId(sessionItem(codexAppSessionKey, "codex_app")),
    );
  });

  it("keeps the user selected session during later refreshes", () => {
    const selectedState = selectPanelSession(
      createDefaultMockPanelUiState(),
      sessionItem(codexAppSessionKey, "codex_app"),
    );

    const refreshedState = selectFirstSessionWhenMissing(selectedState, [
      sessionItem(codexSessionKey, "codex_cli"),
      sessionItem(codexAppSessionKey, "codex_app"),
    ]);

    expect(refreshedState.selectedSessionId).toBe(
      panelSessionToId(sessionItem(codexAppSessionKey, "codex_app")),
    );
  });

  it("selects the first available session when the previous selection disappears", () => {
    const selectedState = selectPanelSession(
      createDefaultMockPanelUiState(),
      sessionItem(codexAppSessionKey, "codex_app"),
    );
    const refreshedState = selectFirstSessionWhenMissing(selectedState, [
      sessionItem(codexSessionKey, "codex_cli"),
    ]);

    expect(refreshedState.selectedSessionId).toBe(
      panelSessionToId(sessionItem(codexSessionKey, "codex_cli")),
    );
  });

  it("routes by runtime source instead of agent kind", () => {
    const codexAppRuntimeSession = sessionItem(codexSessionKey, "codex_app");
    const codexCliRuntimeSession = sessionItem(codexSessionKey, "codex_cli");
    const codexAppSession = sessionItem(
      {
        ...codexSessionKey,
        agent_kind: "codex_app",
      },
      "codex_app",
    );

    expect(isCodexCliRuntime(codexAppRuntimeSession)).toBe(false);
    expect(isCodexCliRuntime(codexCliRuntimeSession)).toBe(true);
    expect(isCodexAppRuntime(codexAppSession)).toBe(true);
    expect(isCodexCliRuntime(codexAppSession)).toBe(false);
  });

  it("shows runtime source labels with Codex app branding", () => {
    expect(sourceTag(sessionItem(codexSessionKey, "codex_app"))).toBe("Codex");
    expect(sourceTag(sessionItem(codexSessionKey, "codex_cli"))).toBe(
      "Codex CLI",
    );
  });

  it("keeps runtime source in selected session identity", () => {
    const codexAppRuntimeSession = sessionItem(codexSessionKey, "codex_app");
    const codexCliRuntimeSession = sessionItem(codexSessionKey, "codex_cli");
    const selectedState = selectPanelSession(
      createDefaultMockPanelUiState(),
      codexAppRuntimeSession,
    );
    const sessions = [codexCliRuntimeSession, codexAppRuntimeSession];

    const selectedSession = sessions.find(
      (session) =>
        panelSessionToId(session) === selectedState.selectedSessionId,
    );

    expect(panelSessionToId(codexAppRuntimeSession)).not.toBe(
      panelSessionToId(codexCliRuntimeSession),
    );
    expect(selectedSession?.runtimeSource).toBe("codex_app");
  });

  it("only jumps on row click when jump action exists", () => {
    const settings = defaultSettings();

    expect(canJumpOnSessionClick(["jump"], settings)).toBe(true);
    expect(canJumpOnSessionClick(["send_reply"], settings)).toBe(false);
    expect(
      canJumpOnSessionClick(["jump"], {
        ...settings,
        terminal: {
          ...settings.terminal,
          jump_enabled: false,
        },
      }),
    ).toBe(false);
  });

  it("can select a jump-capable row even when terminal jump is disabled", () => {
    expect(canSelectOnSessionClick(["jump"])).toBe(true);
    expect(canSelectOnSessionClick(["send_reply"])).toBe(false);
  });

  it("returns empty source sessions when one runtime fetch fails", async () => {
    const sessions = await fetchSessionsForSource(true, async () => {
      throw new Error("Codex APP 不可用");
    });
    const disabledSessions = await fetchSessionsForSource(false, async () => [
      sessionItem(codexAppSessionKey, "codex_app"),
    ]);

    expect(sessions).toEqual([]);
    expect(disabledSessions).toEqual([]);
  });

  it("keeps unfinished sessions above finished sessions while preserving capture order", () => {
    const store = createPanelSessionCaptureOrderStore();
    const running = sessionItem(
      sessionKey("project-a", "running"),
      "codex_app",
      "running",
      "运行中",
      3000,
    );
    const olderWaiting = sessionItem(
      sessionKey("project-a", "older-waiting"),
      "codex_app",
      "waiting_for_answer",
      "等待回复",
      1000,
    );
    const newerWaiting = sessionItem(
      sessionKey("project-a", "newer-waiting"),
      "codex_app",
      "waiting_for_approval",
      "等待审批",
      2000,
    );
    const completed = sessionItem(
      sessionKey("project-a", "completed"),
      "codex_app",
      "completed",
      "已完成",
      4000,
    );

    const initiallyCaptured = mergePanelSessionsByCaptureOrder(store, [
      running,
      olderWaiting,
    ]);
    expect(
      initiallyCaptured.map((session) => session.conversation_label),
    ).toEqual(["running", "older-waiting"]);

    const refreshed = mergePanelSessionsByCaptureOrder(store, [
      newerWaiting,
      completed,
      olderWaiting,
      running,
    ]);

    expect(refreshed.map((session) => session.conversation_label)).toEqual([
      "newer-waiting",
      "running",
      "older-waiting",
      "completed",
    ]);
  });

  it("keeps existing capture rank when a session moves between display groups", () => {
    const store = createPanelSessionCaptureOrderStore();
    const running = sessionItem(
      sessionKey("project-a", "running"),
      "codex_app",
      "running",
      "运行中",
      1000,
    );
    const waiting = sessionItem(
      sessionKey("project-a", "waiting"),
      "codex_app",
      "waiting_for_answer",
      "等待回复",
      1000,
    );
    const captured = mergePanelSessionsByCaptureOrder(store, [
      running,
      waiting,
    ]);
    expect(captured.map((session) => session.conversation_label)).toEqual([
      "running",
      "waiting",
    ]);

    const refreshed = mergePanelSessionsByCaptureOrder(store, [
      {
        ...waiting,
        summary: textDisplay("新的 Agent 摘要"),
        status_kind: "completed",
        status_label: "已完成",
      },
      {
        ...running,
        summary: textDisplay("运行中 Agent 摘要"),
      },
    ]);

    expect(refreshed.map((session) => session.conversation_label)).toEqual([
      "running",
      "waiting",
    ]);
    expect(refreshed[1].summary.full_text).toBe("新的 Agent 摘要");

    const resumed = mergePanelSessionsByCaptureOrder(store, [
      {
        ...refreshed[1],
        status_kind: "running",
        status_label: "运行中",
      },
      refreshed[0],
    ]);

    expect(resumed.map((session) => session.conversation_label)).toEqual([
      "running",
      "waiting",
    ]);
  });

  it("keeps Codex APP parent-child block together and uses unfinished child as block anchor", () => {
    const store = createPanelSessionCaptureOrderStore();
    const oldRunning = sessionItem(
      sessionKey("project-b", "old-running"),
      "codex_cli",
      "running",
      "运行中",
      1000,
    );
    const parent = sessionItem(
      sessionKey("project-a", "parent"),
      "codex_app",
      "running",
      "运行中",
      1000,
    );
    const child = {
      ...sessionItem(
        sessionKey("project-a", "child"),
        "codex_app",
        "running",
        "运行中",
        1000,
      ),
      indent_level: 1,
    };
    mergePanelSessionsByCaptureOrder(store, [oldRunning]);
    const previous = mergePanelSessionsByCaptureOrder(store, [
      oldRunning,
      parent,
    ]);
    expect(previous.map((session) => session.conversation_label)).toEqual([
      "parent",
      "old-running",
    ]);

    const refreshed = mergePanelSessionsByCaptureOrder(store, [
      parent,
      child,
      oldRunning,
    ]);

    expect(refreshed.map((session) => session.conversation_label)).toEqual([
      "parent",
      "child",
      "old-running",
    ]);
    expect(refreshed[1].indent_level).toBe(1);
  });

  it("does not let a completed child raise the unfinished parent block", () => {
    const store = createPanelSessionCaptureOrderStore();
    const parent = sessionItem(
      sessionKey("project-a", "parent"),
      "codex_app",
      "running",
      "运行中",
      1000,
    );
    const otherRunning = sessionItem(
      sessionKey("project-a", "other-running"),
      "codex_app",
      "running",
      "运行中",
      1000,
    );
    const child = {
      ...sessionItem(
        sessionKey("project-a", "child"),
        "codex_app",
        "completed",
        "已完成",
        1000,
      ),
      indent_level: 1,
    };
    mergePanelSessionsByCaptureOrder(store, [parent]);
    const withOtherRunning = mergePanelSessionsByCaptureOrder(store, [
      otherRunning,
      parent,
    ]);
    const refreshed = mergePanelSessionsByCaptureOrder(store, [
      parent,
      child,
      otherRunning,
    ]);

    expect(
      withOtherRunning.map((session) => session.conversation_label),
    ).toEqual(["other-running", "parent"]);
    expect(refreshed.map((session) => session.conversation_label)).toEqual([
      "other-running",
      "parent",
      "child",
    ]);
  });

  it("uses returned array order as first observation order for same-refresh sessions", () => {
    const store = createPanelSessionCaptureOrderStore();
    const codexApp = {
      ...sessionItem(
        sessionKey("project-a", "same-key"),
        "codex_app",
        "running",
        "运行中",
        1000,
      ),
      session_key: codexAppSessionKey,
    };
    const codexCli = {
      ...sessionItem(
        codexAppSessionKey,
        "codex_cli",
        "running",
        "运行中",
        1000,
      ),
      runtimeSource: "codex_cli" as const,
    };

    const captured = mergePanelSessionsByCaptureOrder(store, [
      codexApp,
      codexCli,
    ]);

    expect(captured.map((session) => session.runtimeSource)).toEqual([
      "codex_app",
      "codex_cli",
    ]);
  });

  it("keeps capture stores isolated", () => {
    const firstStore = createPanelSessionCaptureOrderStore();
    const secondStore = createPanelSessionCaptureOrderStore();
    const first = sessionItem(
      sessionKey("project-a", "first"),
      "codex_app",
      "running",
      "运行中",
    );
    const second = sessionItem(
      sessionKey("project-a", "second"),
      "codex_app",
      "running",
      "运行中",
    );

    mergePanelSessionsByCaptureOrder(firstStore, [first]);
    const firstStoreOrder = mergePanelSessionsByCaptureOrder(firstStore, [
      first,
      second,
    ]);
    const secondStoreOrder = mergePanelSessionsByCaptureOrder(secondStore, [
      first,
      second,
    ]);

    expect(
      firstStoreOrder.map((session) => session.conversation_label),
    ).toEqual(["second", "first"]);
    expect(
      secondStoreOrder.map((session) => session.conversation_label),
    ).toEqual(["first", "second"]);
  });

  it("classifies session display groups explicitly", () => {
    expect(isPanelSessionUnfinishedForDisplay("running")).toBe(true);
    expect(isPanelSessionUnfinishedForDisplay("waiting_for_approval")).toBe(
      true,
    );
    expect(isPanelSessionUnfinishedForDisplay("waiting_for_answer")).toBe(true);
    expect(isPanelSessionUnfinishedForDisplay("completed")).toBe(false);
    expect(isPanelSessionUnfinishedForDisplay("failed")).toBe(false);
    expect(isPanelSessionUnfinishedForDisplay("detached")).toBe(false);
  });

  it("renders the refreshed list row summary for the same session", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const session = {
      ...sessionItem(codexAppSessionKey, "codex_app", "running", "运行中"),
      summary: textDisplay("旧摘要"),
      actions: ["jump"] as const,
    };
    const refreshed = {
      ...session,
      summary: textDisplay("新的 Agent 摘要"),
    };
    const container = document.createElement("div");
    const root = createRoot(container);
    const renderStream = (items: readonly PanelSessionListItem[]) =>
      createElement(SessionStream, {
        mockUiState: createDefaultMockPanelUiState(),
        currentTimeMs: 2000,
        sessions: items,
        settings: defaultSettings(),
        selectedSessionId: null,
        onCreateFollowupTurn: () => undefined,
        onDraftChange: () => undefined,
        onJump: () => undefined,
        onResolveApproval: () => undefined,
        onSendReply: () => undefined,
        onSubmitChoice: () => undefined,
        onToggleChoice: () => undefined,
      });

    await act(async () => {
      root.render(renderStream([session]));
    });
    expect(container.textContent).toContain("旧摘要");

    await act(async () => {
      root.render(renderStream([refreshed]));
    });
    expect(container.textContent).toContain("新的 Agent 摘要");

    await act(async () => {
      root.unmount();
    });
  });

  it("renders child session with one-level indent", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const parent = sessionItem(
      sessionKey("project-a", "parent"),
      "codex_app",
      "running",
      "运行中",
    );
    const child = {
      ...sessionItem(
        sessionKey("project-a", "child"),
        "codex_app",
        "running",
        "运行中",
      ),
      indent_level: 1,
    };
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions: [parent, child],
          settings: defaultSettings(),
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const rows = container.querySelectorAll<HTMLElement>(".session-table-row");
    expect(rows[0].style.getPropertyValue("--session-indent")).toBe("0px");
    expect(rows[1].style.getPropertyValue("--session-indent")).toBe("14px");

    await act(async () => {
      root.unmount();
    });
  });

  it("renders status icons in a leading column with source and project on the identity line", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const running = sessionItem(
      sessionKey("project-a", "running"),
      "codex_app",
      "running",
      "运行中",
    );
    const completed = sessionItem(
      sessionKey("project-b", "completed"),
      "codex_cli",
      "completed",
      "已完成",
    );
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions: [running, completed],
          settings: defaultSettings(),
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const rowMain = container.querySelector(".session-row-main");
    const statusIcons = container.querySelectorAll(".session-status-icon");
    expect(rowMain?.firstElementChild).toBe(statusIcons[0]);
    expect(statusIcons).toHaveLength(2);
    expect(statusIcons[0].getAttribute("aria-label")).toBe("运行中");
    expect(statusIcons[0].getAttribute("role")).toBe("img");
    expect(statusIcons[0].getAttribute("title")).toBe("运行中");
    expect(statusIcons[0].textContent).toBe("");
    expect(
      statusIcons[0].querySelector("svg[aria-hidden='true']"),
    ).not.toBeNull();
    expect(statusIcons[1].getAttribute("aria-label")).toBe("已完成");
    expect(statusIcons[1].getAttribute("role")).toBe("img");
    expect(statusIcons[1].getAttribute("title")).toBe("已完成");
    expect(statusIcons[1].textContent).toBe("");
    expect(
      statusIcons[1].querySelector("svg[aria-hidden='true']"),
    ).not.toBeNull();
    expect(
      container.querySelector('[role="img"][aria-label="运行中"]'),
    ).not.toBeNull();

    const firstIdentityLine = container.querySelector(".session-identity-line");
    expect(
      firstIdentityLine?.querySelector(".session-source")?.textContent,
    ).toBe("Codex");
    expect(
      firstIdentityLine?.querySelector(".session-project")?.textContent,
    ).toBe("project-a");
    expect(container.querySelector(".session-thread")?.textContent).toBe(
      "Thread running",
    );

    const stopButton = container.querySelector<HTMLButtonElement>(
      ".session-stop-placeholder",
    );
    expect(stopButton?.disabled).toBe(true);
    expect(stopButton?.className).toBe("session-stop-placeholder");
    expect(stopButton?.getAttribute("aria-label")).toBe("停止占位");
    expect(stopButton?.getAttribute("title")).toBe("停止能力尚未接入");
    expect(stopButton?.textContent).toBe("");
    expect(stopButton?.querySelector("span")).toBeNull();
    const stopIcon =
      stopButton?.querySelector<SVGElement>("svg[aria-hidden='true']") ?? null;
    expect(stopIcon).not.toBeNull();
    expect(stopIcon?.classList.contains("lucide-octagon-x")).toBe(true);
    expectThinAbsoluteSvgStroke(stopIcon);
    expect(stopIcon?.querySelectorAll("path")).toHaveLength(3);
    expect(stopIcon?.querySelector("rect")).toBeNull();
    expect(stopIcon?.querySelector("circle")).toBeNull();

    await act(async () => {
      root.unmount();
    });
  });

  it("hides source and project labels for child agent rows", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const parent = sessionItem(
      sessionKey("project-a", "parent"),
      "codex_app",
      "running",
      "运行中",
    );
    const child = {
      ...sessionItem(
        sessionKey("project-a", "child"),
        "codex_app",
        "running",
        "运行中",
      ),
      indent_level: 1,
    };
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions: [parent, child],
          settings: defaultSettings(),
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const rows = container.querySelectorAll(".session-table-row");
    expect(rows).toHaveLength(2);
    expect(rows[0].querySelector(".session-source")?.textContent).toBe("Codex");
    expect(rows[0].querySelector(".session-project")?.textContent).toBe(
      "project-a",
    );
    expect(rows[1].querySelector(".session-source")).toBeNull();
    expect(rows[1].querySelector(".session-project")).toBeNull();
    expect(rows[1].querySelector(".session-thread")?.textContent).toBe(
      "Thread child",
    );

    await act(async () => {
      root.unmount();
    });
  });

  it("renders every session status with an open-source svg icon", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const statusCases: ReadonlyArray<{
      readonly status: SessionStatus;
      readonly label: string;
    }> = [
      { status: "running", label: "运行中" },
      { status: "waiting_for_approval", label: "等待审批" },
      { status: "waiting_for_answer", label: "等待回复" },
      { status: "completed", label: "已完成" },
      { status: "failed", label: "失败" },
      { status: "detached", label: "失联" },
    ];
    const sessions = statusCases.map(({ status, label }) =>
      sessionItem(
        sessionKey("project-icons", status),
        "codex_app",
        status,
        label,
      ),
    );
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions,
          settings: defaultSettings(),
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const statusIcons = Array.from(
      container.querySelectorAll<HTMLElement>(".session-status-icon"),
    );
    expect(statusIcons).toHaveLength(statusCases.length);
    statusCases.forEach(({ label }, index) => {
      const icon = statusIcons[index];
      expect(icon.getAttribute("aria-label")).toBe(label);
      expect(icon.getAttribute("role")).toBe("img");
      expect(icon.getAttribute("title")).toBe(label);
      expect(icon.textContent).toBe("");
      expectThinAbsoluteSvgStroke(
        icon.querySelector<SVGElement>("svg[aria-hidden='true']"),
      );
    });
    const approvalSvg = statusIcons[1].querySelector("svg[aria-hidden='true']");
    const answerSvg = statusIcons[2].querySelector("svg[aria-hidden='true']");
    expect(approvalSvg?.outerHTML).not.toBe(answerSvg?.outerHTML);

    await act(async () => {
      root.unmount();
    });
  });

  it("keeps waiting answer and approval status colors distinct", () => {
    const styles = readFileSync("src/styles.css", "utf8");

    expect(styles).toMatch(
      /\.session-status-waiting_for_approval\s*{\s*--status-color:\s*var\(--warn\);\s*}/,
    );
    expect(styles).toMatch(
      /\.session-status-waiting_for_answer\s*{\s*--status-color:\s*var\(--accent-strong\);\s*}/,
    );
  });

  it("keeps status icons separate from source label bold text", () => {
    const styles = readFileSync("src/styles.css", "utf8");
    const sharedIconAndSourceBlock = styles.match(
      /\.session-status-icon,\s*\.session-source\s*{(?<body>[^}]*)}/,
    );
    const sourceBlocks = Array.from(
      styles.matchAll(/\.session-source\s*{(?<body>[^}]*)}/g),
    );
    const sourceBlock = sourceBlocks.at(-1);

    expect(sharedIconAndSourceBlock?.groups?.body).not.toContain("font-weight");
    expect(sourceBlock?.groups?.body).toMatch(/font-weight:\s*800;/);
  });

  it("allows the desktop session thread column to shrink before the summary", () => {
    const styles = readFileSync("src/styles.css", "utf8");
    const rowMainBlock = styles.match(/\.session-row-main\s*{(?<body>[^}]*)}/);
    const tooltipWrapperBlock = styles.match(
      /\.session-identity\s*>\s*\.markdown-tooltip,\s*\.session-row-main\s*>\s*\.markdown-tooltip,\s*\.session-row-action\s*>\s*\.markdown-tooltip\s*{(?<body>[^}]*)}/,
    );

    expect(rowMainBlock?.groups?.body).toMatch(
      /grid-template-columns:\s*20px\s+minmax\(0,\s*0\.34fr\)\s+minmax\(0,\s*1fr\)\s+auto;/,
    );
    expect(rowMainBlock?.groups?.body).not.toContain("minmax(176px, 0.34fr)");
    expect(tooltipWrapperBlock?.groups?.body).toMatch(/min-width:\s*0;/);
  });

  it("keeps short panel content natural and long content scrollable", () => {
    const styles = readFileSync("src/styles.css", "utf8");
    const surfaceBlock = styles.match(/\.app-surface\s*{(?<body>[^}]*)}/);
    const shellBlock = styles.match(/\.panel-shell\s*{(?<body>[^}]*)}/);
    const contentBlock = styles.match(/\.panel-content\s*{(?<body>[^}]*)}/);
    const naturalContentBlock = styles.match(
      /\.panel-natural-content\s*{(?<body>[^}]*)}/,
    );

    expect(surfaceBlock?.groups?.body).not.toMatch(
      /(?:^|[^-])height:\s*100vh;/,
    );
    expect(surfaceBlock?.groups?.body).toMatch(/max-height:\s*100vh;/);
    expect(surfaceBlock?.groups?.body).toMatch(/overflow:\s*hidden;/);
    expect(shellBlock?.groups?.body).not.toMatch(/height:\s*100%;/);
    expect(shellBlock?.groups?.body).toMatch(
      /max-height:\s*calc\(100vh - 12px\);/,
    );
    expect(contentBlock?.groups?.body).toMatch(
      /max-height:\s*calc\(100vh - 44px\);/,
    );
    expect(contentBlock?.groups?.body).toMatch(/overflow:\s*auto;/);
    expect(naturalContentBlock?.groups?.body).toMatch(/display:\s*grid;/);
    expect(naturalContentBlock?.groups?.body).not.toMatch(/max-height:/);
  });

  it("renders reusable panel icons with thin absolute strokes", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const iconNames: readonly PanelIconName[] = [
      "window-minimize",
      "window-settings",
      "window-close",
      "send",
      "stop-placeholder",
    ];
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(
          "div",
          null,
          iconNames.map((name) =>
            createElement(PanelIcon, {
              key: name,
              name,
            }),
          ),
        ),
      );
    });

    const svgs = Array.from(
      container.querySelectorAll<SVGElement>("svg[aria-hidden='true']"),
    );
    expect(svgs).toHaveLength(iconNames.length);
    svgs.forEach((svg) => {
      expectThinAbsoluteSvgStroke(svg);
    });

    await act(async () => {
      root.unmount();
    });
  });

  it("renders all session status icons with thin absolute strokes", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const statuses: readonly SessionStatus[] = [
      "running",
      "waiting_for_approval",
      "waiting_for_answer",
      "completed",
      "failed",
      "detached",
    ];
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(
          "div",
          null,
          statuses.map((status) =>
            createElement(PanelSessionStatusIcon, {
              key: status,
              status,
            }),
          ),
        ),
      );
    });

    const svgs = Array.from(
      container.querySelectorAll<SVGElement>("svg[aria-hidden='true']"),
    );
    expect(svgs).toHaveLength(statuses.length);
    svgs.forEach((svg) => {
      expectThinAbsoluteSvgStroke(svg);
    });

    await act(async () => {
      root.unmount();
    });
  });

  it("renders title actions as icon buttons without character labels", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const onOpenSettings = vi.fn();
    const onMinimize = vi.fn();
    const onClose = vi.fn();
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(PanelTitleActions, {
          onClose,
          onMinimize,
          onOpenSettings,
        }),
      );
    });

    const buttons = Array.from(container.querySelectorAll("button"));
    expect(buttons).toHaveLength(3);
    [{ label: "最小化" }, { label: "设置" }, { label: "关闭" }].forEach(
      ({ label }, index) => {
        const button = buttons[index];
        expect(button.getAttribute("aria-label")).toBe(label);
        expect(button.getAttribute("title")).toBe(label);
        expect(button.textContent).toBe("");
        expectThinAbsoluteSvgStroke(
          button.querySelector<SVGElement>("svg[aria-hidden='true']"),
        );
      },
    );
    expect(
      buttons[1]
        .querySelector("svg[aria-hidden='true']")
        ?.classList.contains("lucide-bolt"),
    ).toBe(true);

    await act(async () => {
      buttons[0].click();
      buttons[1].click();
      buttons[2].click();
    });

    expect(onMinimize).toHaveBeenCalledTimes(1);
    expect(onOpenSettings).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);

    await act(async () => {
      root.unmount();
    });
  });

  it("detects inline overflow from rendered text dimensions", () => {
    expect(elementHasInlineOverflow(null)).toBe(false);
    expect(elementHasInlineOverflow({ clientWidth: 0, scrollWidth: 0 })).toBe(
      false,
    );
    expect(
      elementHasInlineOverflow({ clientWidth: 120, scrollWidth: 120 }),
    ).toBe(false);
    expect(
      elementHasInlineOverflow({ clientWidth: 120, scrollWidth: 121 }),
    ).toBe(true);
  });

  it("detects block overflow from rendered text dimensions", () => {
    expect(elementHasBlockOverflow(null)).toBe(false);
    expect(elementHasBlockOverflow({ clientHeight: 0, scrollHeight: 0 })).toBe(
      false,
    );
    expect(
      elementHasBlockOverflow({ clientHeight: 40, scrollHeight: 40 }),
    ).toBe(false);
    expect(
      elementHasBlockOverflow({ clientHeight: 40, scrollHeight: 41 }),
    ).toBe(true);
  });

  it("only enables project and thread tooltips when their visible text overflows", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const restoreInlineSize = mockTooltipLabelInlineSize(100, 100);
    const session = {
      ...sessionItem(codexAppSessionKey, "codex_app", "running", "运行中"),
      project_label: "builder-panel",
      thread_label: "修复 session 摘要 tooltip 多行未生效",
    };
    const container = document.createElement("div");
    const root = createRoot(container);

    try {
      await act(async () => {
        root.render(
          createElement(SessionStream, {
            mockUiState: createDefaultMockPanelUiState(),
            currentTimeMs: 2000,
            sessions: [session],
            settings: defaultSettings(),
            selectedSessionId: null,
            onCreateFollowupTurn: () => undefined,
            onDraftChange: () => undefined,
            onJump: () => undefined,
            onResolveApproval: () => undefined,
            onSendReply: () => undefined,
            onSubmitChoice: () => undefined,
            onToggleChoice: () => undefined,
          }),
        );
      });

      expect(
        container.querySelector(".session-project.markdown-tooltip-enabled"),
      ).toBeNull();
      expect(
        container.querySelector(".session-thread.markdown-tooltip-enabled"),
      ).toBeNull();
      expect(container.querySelector(".session-project")?.textContent).toBe(
        "builder-panel",
      );
      expect(container.querySelector(".session-thread")?.textContent).toBe(
        "修复 session 摘要 tooltip 多行未生效",
      );

      restoreInlineSize.setSize(140, 80);
      await act(async () => {
        window.dispatchEvent(new Event("resize"));
      });

      expect(
        container.querySelector(".session-project.markdown-tooltip-enabled"),
      ).not.toBeNull();
      expect(
        container.querySelector(".session-thread.markdown-tooltip-enabled"),
      ).not.toBeNull();

      restoreInlineSize.setSize(80, 100);
      await act(async () => {
        window.dispatchEvent(new Event("resize"));
      });

      expect(
        container.querySelector(".session-project.markdown-tooltip-enabled"),
      ).toBeNull();
      expect(
        container.querySelector(".session-thread.markdown-tooltip-enabled"),
      ).toBeNull();
    } finally {
      restoreInlineSize.restore();
      await act(async () => {
        root.unmount();
      });
    }
  });

  it("renders full tooltip paragraph even when the row summary is truncated", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const session = {
      ...sessionItem(codexAppSessionKey, "codex_app", "running", "运行中"),
      summary: {
        text: "截断",
        full_text: "第一段完整内容\n\n第二段完整内容很长",
        truncated: true,
        max_chars: 3,
      },
    };
    const settings = {
      ...defaultSettings(),
      display: {
        ...defaultSettings().display,
        summary_tooltip_paragraphs: 1,
      },
    };
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions: [session],
          settings,
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const summaryTooltip = container.querySelector(".session-summary-tooltip");
    expect(summaryTooltip?.textContent).toBe("第二段");

    await act(async () => {
      summaryTooltip?.dispatchEvent(
        new MouseEvent("mouseover", { bubbles: true }),
      );
    });

    expect(document.body.textContent).toContain("第二段完整内容很长");

    await act(async () => {
      root.unmount();
    });
  });

  it("does not enable summary tooltip when the complete summary is visible", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const restoreBlockSize = mockTooltipLabelBlockSize(40, 40);
    const session = {
      ...sessionItem(codexAppSessionKey, "codex_app", "running", "运行中"),
      summary: {
        text: "摘要完整内容",
        full_text: "摘要完整内容",
        truncated: false,
        max_chars: 120,
      },
    };
    const container = document.createElement("div");
    const root = createRoot(container);

    try {
      await act(async () => {
        root.render(
          createElement(SessionStream, {
            mockUiState: createDefaultMockPanelUiState(),
            currentTimeMs: 2000,
            sessions: [session],
            settings: defaultSettings(),
            selectedSessionId: null,
            onCreateFollowupTurn: () => undefined,
            onDraftChange: () => undefined,
            onJump: () => undefined,
            onResolveApproval: () => undefined,
            onSendReply: () => undefined,
            onSubmitChoice: () => undefined,
            onToggleChoice: () => undefined,
          }),
        );
      });

      const summaryTooltip = container.querySelector(
        ".session-summary-tooltip",
      );
      expect(
        summaryTooltip?.classList.contains("markdown-tooltip-enabled"),
      ).toBe(false);

      await act(async () => {
        summaryTooltip?.dispatchEvent(
          new MouseEvent("mouseover", { bubbles: true }),
        );
      });

      expect(document.body.querySelector(".markdown-tooltip-panel")).toBeNull();
    } finally {
      restoreBlockSize.restore();
      await act(async () => {
        root.unmount();
      });
    }
  });

  it("enables summary tooltip when the complete paragraph is visually clipped", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const restoreBlockSize = mockTooltipLabelBlockSize(80, 40);
    const session = {
      ...sessionItem(codexAppSessionKey, "codex_app", "running", "运行中"),
      summary: {
        text: "摘要完整内容",
        full_text: "摘要完整内容",
        truncated: false,
        max_chars: 120,
      },
    };
    const container = document.createElement("div");
    const root = createRoot(container);

    try {
      await act(async () => {
        root.render(
          createElement(SessionStream, {
            mockUiState: createDefaultMockPanelUiState(),
            currentTimeMs: 2000,
            sessions: [session],
            settings: defaultSettings(),
            selectedSessionId: null,
            onCreateFollowupTurn: () => undefined,
            onDraftChange: () => undefined,
            onJump: () => undefined,
            onResolveApproval: () => undefined,
            onSendReply: () => undefined,
            onSubmitChoice: () => undefined,
            onToggleChoice: () => undefined,
          }),
        );
      });

      const summaryTooltip = container.querySelector(
        ".session-summary-tooltip",
      );
      expect(
        summaryTooltip?.classList.contains("markdown-tooltip-enabled"),
      ).toBe(true);

      await act(async () => {
        summaryTooltip?.dispatchEvent(
          new MouseEvent("mouseover", { bubbles: true }),
        );
      });

      expect(document.body.textContent).toContain("摘要完整内容");
    } finally {
      restoreBlockSize.restore();
      await act(async () => {
        root.unmount();
      });
    }
  });

  it("jumps when double-clicking the visible summary tooltip panel", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const session = {
      ...sessionItem(codexAppSessionKey, "codex_app", "running", "运行中"),
      actions: ["jump"] as const,
      summary: {
        text: "摘要",
        full_text: "摘要完整内容",
        truncated: false,
        max_chars: 2,
      },
    };
    const onJump = vi.fn();
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions: [session],
          settings: defaultSettings(),
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const summaryTooltip = container.querySelector(".session-summary-tooltip");
    await act(async () => {
      summaryTooltip?.dispatchEvent(
        new MouseEvent("mouseover", { bubbles: true }),
      );
    });

    const tooltipPanel = document.body.querySelector(".markdown-tooltip-panel");
    await act(async () => {
      tooltipPanel?.dispatchEvent(
        new MouseEvent("dblclick", { bubbles: true, cancelable: true }),
      );
    });

    expect(onJump).toHaveBeenCalledTimes(1);
    expect(onJump).toHaveBeenCalledWith(session);

    await act(async () => {
      root.unmount();
    });
  });

  it("keeps markdown link behavior when double-clicking a summary tooltip link", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const session = {
      ...sessionItem(codexAppSessionKey, "codex_app", "running", "运行中"),
      actions: ["jump"] as const,
      summary: {
        text: "摘要",
        full_text: "查看 [链接](https://example.com)",
        truncated: false,
        max_chars: 2,
      },
    };
    const onJump = vi.fn();
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions: [session],
          settings: defaultSettings(),
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const summaryTooltip = container.querySelector(".session-summary-tooltip");
    await act(async () => {
      summaryTooltip?.dispatchEvent(
        new MouseEvent("mouseover", { bubbles: true }),
      );
    });

    const tooltipLink = document.body.querySelector<HTMLAnchorElement>(
      ".markdown-tooltip-panel a",
    );
    const linkDoubleClick = new MouseEvent("dblclick", {
      bubbles: true,
      cancelable: true,
    });
    const dispatched = tooltipLink?.dispatchEvent(linkDoubleClick);

    expect(dispatched).toBe(true);
    expect(linkDoubleClick.defaultPrevented).toBe(false);
    expect(onJump).not.toHaveBeenCalled();

    await act(async () => {
      root.unmount();
    });
  });

  it("does not jump when double-clicking an action summary tooltip", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const session = sessionItem(
      codexAppSessionKey,
      "codex_app",
      "waiting_for_approval",
      "等待审批",
    );
    const onJump = vi.fn();
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions: [session],
          settings: defaultSettings(),
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const actionTooltip = container.querySelector(".session-action-tooltip");
    await act(async () => {
      actionTooltip?.dispatchEvent(
        new MouseEvent("mouseover", { bubbles: true }),
      );
    });

    const tooltipPanel = document.body.querySelector(".markdown-tooltip-panel");
    await act(async () => {
      tooltipPanel?.dispatchEvent(
        new MouseEvent("dblclick", { bubbles: true, cancelable: true }),
      );
    });

    expect(onJump).not.toHaveBeenCalled();

    await act(async () => {
      root.unmount();
    });
  });

  it("runs a queued refresh after the current refresh finishes", async () => {
    const resolvers: Array<() => void> = [];
    const refresh = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolvers.push(resolve);
        }),
    );
    const scheduler = createSessionRefreshScheduler({
      eventDelayMs: 350,
      refresh,
      reportError: () => undefined,
      scheduleTimeout: window.setTimeout,
      clearTimeout: window.clearTimeout,
    });

    scheduler.requestImmediate();
    scheduler.requestEvent();

    expect(refresh).toHaveBeenCalledTimes(1);
    resolvers.shift()?.();
    await flushPromises();

    expect(refresh).toHaveBeenCalledTimes(2);
    resolvers.shift()?.();
    await flushPromises();
    scheduler.dispose();
  });

  it("throttles continuous session update events without waiting for silence", async () => {
    vi.useFakeTimers();
    const refresh = vi.fn<() => Promise<void>>(async () => undefined);
    const scheduler = createSessionRefreshScheduler({
      eventDelayMs: 350,
      refresh,
      reportError: () => undefined,
      scheduleTimeout: window.setTimeout,
      clearTimeout: window.clearTimeout,
    });

    scheduler.requestEvent();
    vi.advanceTimersByTime(100);
    scheduler.requestEvent();
    vi.advanceTimersByTime(100);
    scheduler.requestEvent();
    expect(refresh).not.toHaveBeenCalled();

    vi.advanceTimersByTime(150);
    await flushPromises();
    expect(refresh).toHaveBeenCalledTimes(1);

    scheduler.requestEvent();
    vi.advanceTimersByTime(100);
    scheduler.requestEvent();
    vi.advanceTimersByTime(250);
    await flushPromises();
    expect(refresh).toHaveBeenCalledTimes(2);

    scheduler.dispose();
    vi.useRealTimers();
  });

  it("counts waiting and running sessions for the expanded panel summary", () => {
    const counts = countSessionsByStatus([
      sessionItem(
        sessionKey("project-a", "approval"),
        "codex_app",
        "waiting_for_approval",
        "等待审批",
        1000,
      ),
      sessionItem(
        sessionKey("project-a", "answer"),
        "codex_app",
        "waiting_for_answer",
        "等待回复",
        1000,
      ),
      sessionItem(
        sessionKey("project-a", "running"),
        "codex_app",
        "running",
        "运行中",
        1000,
      ),
      sessionItem(
        sessionKey("project-a", "completed"),
        "codex_app",
        "completed",
        "已完成",
        1000,
      ),
    ]);

    expect(counts).toEqual({ waiting: 2, running: 1 });
  });

  it("aggregates tool usage by latest source value without summing sessions", () => {
    const olderCodex = {
      ...sessionItem(
        sessionKey("project-a", "codex-old"),
        "codex_app",
        "running",
        "运行中",
        1000,
      ),
      usage_5h: verifiedUsage("10 tokens", "codex-account", 1000),
    };
    const newerCodex = {
      ...sessionItem(
        sessionKey("project-a", "codex-new"),
        "codex_app",
        "running",
        "运行中",
        2000,
      ),
      usage_5h: verifiedUsage("20 tokens", "codex-account", 2000),
    };
    const usage = aggregateToolUsage([olderCodex, newerCodex]);

    expect(usage).toEqual([
      {
        family: "Codex",
        usage5h: "20 tokens",
        weekly: "--",
        tooltip: "test 20 tokens",
      },
    ]);
  });

  it("ignores session scoped usage in tool usage summary", () => {
    const codexSessionUsage = {
      ...sessionItem(
        sessionKey("project-a", "codex-session"),
        "codex_app",
        "running",
        "运行中",
        1000,
      ),
      usage_5h: verifiedUsage("99 tokens", "codex-session", 1000, "session"),
    };

    const usage = aggregateToolUsage([codexSessionUsage]);

    expect(usage).toEqual([]);
  });

  it("submits reply on Enter but keeps Shift Enter for multiline input", () => {
    expect(shouldSubmitReplyOnKeyDown(true, "Enter", false)).toBe(true);
    expect(shouldSubmitReplyOnKeyDown(true, "Enter", true)).toBe(false);
    expect(shouldSubmitReplyOnKeyDown(false, "Enter", false)).toBe(false);
    expect(shouldSubmitReplyOnKeyDown(true, "A", false)).toBe(false);
  });

  it("builds default tooltip from the most recent five full paragraphs", () => {
    const display = textDisplayParagraph({
      text: "第六段",
      full_text:
        "第一段\n\n第二段\n\n第三段\n\n第四段\n\n第五段\n\n第六段内容很长",
      truncated: false,
      max_chars: 4,
    });

    expect(display.visibleText).toBe("第六段内");
    expect(display.fullParagraph).toBe("第六段内容很长");
    expect(display.paragraphTruncated).toBe(true);
    expect(display.hasHiddenTooltipContent).toBe(true);
    expect(display.tooltipText).toBe(
      "第二段\n\n第三段\n\n第四段\n\n第五段\n\n第六段内容很长",
    );
  });

  it("keeps line breaks inside the full tooltip paragraphs", () => {
    const display = textDisplayParagraph({
      text: "第一段",
      full_text: "第一段\n\n- 第一项\n- **第二项**\n继续说明",
      truncated: false,
      max_chars: 8,
    });

    expect(display.visibleText).toBe("- 第一项\n- ");
    expect(display.fullParagraph).toBe("- 第一项\n- **第二项**\n继续说明");
    expect(display.hasHiddenTooltipContent).toBe(true);
    expect(display.tooltipText).toBe(
      "第一段\n\n- 第一项\n- **第二项**\n继续说明",
    );
  });

  it("keeps tooltip text independent from display text and max-char truncation", () => {
    const display = textDisplayParagraph({
      text: "截断展示",
      full_text: "第一段完整内容\n\n第二段完整内容很长",
      truncated: true,
      max_chars: 3,
    });

    expect(display.visibleText).toBe("第二段");
    expect(display.paragraphTruncated).toBe(true);
    expect(display.hasHiddenTooltipContent).toBe(true);
    expect(display.tooltipText).toBe("第一段完整内容\n\n第二段完整内容很长");
  });

  it("keeps tooltip text but marks single complete paragraph as fully visible", () => {
    const display = textDisplayParagraph({
      text: "短文本",
      full_text: "短文本但可能被行宽省略",
      truncated: false,
      max_chars: 100,
    });

    expect(display.visibleText).toBe("短文本但可能被行宽省略");
    expect(display.paragraphTruncated).toBe(false);
    expect(display.hasHiddenTooltipContent).toBe(false);
    expect(display.tooltipText).toBe("短文本但可能被行宽省略");
  });

  it("builds tooltip from the most recent paragraphs while showing only the last", () => {
    const display = textDisplayParagraph(
      {
        text: "第三段",
        full_text: "第一段\n\n第二段\n\n第三段",
        truncated: false,
        max_chars: 100,
      },
      2,
    );

    expect(display.visibleText).toBe("第三段");
    expect(display.fullParagraph).toBe("第三段");
    expect(display.hasHiddenTooltipContent).toBe(true);
    expect(display.tooltipText).toBe("第二段\n\n第三段");
  });

  it("does not mark content hidden when tooltip count keeps only the complete last paragraph", () => {
    const display = textDisplayParagraph(
      {
        text: "第三段",
        full_text: "第一段\n\n第二段\n\n第三段",
        truncated: false,
        max_chars: 100,
      },
      1,
    );

    expect(display.visibleText).toBe("第三段");
    expect(display.fullParagraph).toBe("第三段");
    expect(display.hasHiddenTooltipContent).toBe(false);
    expect(display.tooltipText).toBe("第三段");
  });

  it("returns all paragraphs when fewer than the requested tooltip count", () => {
    const display = textDisplayParagraph(
      {
        text: "第二段",
        full_text: "第一段\n\n第二段",
        truncated: false,
        max_chars: 100,
      },
      5,
    );

    expect(display.visibleText).toBe("第二段");
    expect(display.hasHiddenTooltipContent).toBe(true);
    expect(display.tooltipText).toBe("第一段\n\n第二段");
  });

  it("parses tooltip markdown blocks for rendered session tooltips", () => {
    expect(
      parseTooltipMarkdown(
        "## 标题\n\n- **允许**\n- `拒绝`\n\n> 保留\n> 换行\n\n```txt\nline 1\nline 2\n```",
      ),
    ).toEqual([
      { kind: "heading", level: 2, text: "标题" },
      { kind: "unordered-list", items: ["**允许**", "`拒绝`"] },
      { kind: "blockquote", lines: ["保留", "换行"] },
      { kind: "code", text: "line 1\nline 2" },
    ]);
  });

  it("positions tooltip above the anchor when the bottom would be clipped", () => {
    expect(
      positionTooltipPanel(
        { bottom: 290, left: 120, top: 260 },
        { height: 90, width: 240 },
        { height: 300, width: 500 },
      ),
    ).toMatchObject({
      left: 120,
      placement: "top",
      top: 164,
    });
  });

  it("keeps tooltip inside the right viewport edge", () => {
    expect(
      positionTooltipPanel(
        { bottom: 40, left: 470, top: 20 },
        { height: 90, width: 220 },
        { height: 300, width: 500 },
      ),
    ).toMatchObject({
      left: 272,
      placement: "bottom",
    });
  });

  it("limits tooltip width to the available viewport width", () => {
    expect(
      positionTooltipPanel(
        { bottom: 40, left: 20, top: 20 },
        { height: 90, width: 520 },
        { height: 300, width: 300 },
      ),
    ).toMatchObject({
      left: 8,
      maxWidth: 284,
    });
  });

  it("keeps an oversized tooltip inside the top viewport edge", () => {
    expect(
      positionTooltipPanel(
        { bottom: 160, left: 20, top: 140 },
        { height: 500, width: 240 },
        { height: 180, width: 360 },
      ),
    ).toMatchObject({
      left: 20,
      top: 8,
    });
  });

  it("stops tooltip portal events from bubbling to the session row", () => {
    const stopPropagation = vi.fn();

    stopTooltipPortalEvent({ stopPropagation });

    expect(stopPropagation).toHaveBeenCalledTimes(1);
  });

  it("detects tooltip link targets from element and text nodes", () => {
    const anchor = document.createElement("a");
    const nested = document.createElement("span");
    const text = document.createTextNode("链接");
    nested.append(text);
    anchor.append(nested);

    expect(isTooltipLinkEventTarget(anchor)).toBe(true);
    expect(isTooltipLinkEventTarget(nested)).toBe(true);
    expect(isTooltipLinkEventTarget(text)).toBe(true);
    expect(isTooltipLinkEventTarget(document.createElement("span"))).toBe(
      false,
    );
    expect(isTooltipLinkEventTarget(null)).toBe(false);
  });

  it("stops tooltip double-clicks and skips callbacks for links", () => {
    const anchor = document.createElement("a");
    const stopPropagation = vi.fn();
    const onTooltipDoubleClick = vi.fn();

    handleTooltipPanelDoubleClick(
      { target: anchor, stopPropagation },
      onTooltipDoubleClick,
    );

    expect(stopPropagation).toHaveBeenCalledTimes(1);
    expect(onTooltipDoubleClick).not.toHaveBeenCalled();
  });

  it("renders full thread label in session detail", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const longThreadLabel = "一二三四五六七八九十十一完整线程标题";
    const selectedSession = {
      ...sessionItem(codexAppSessionKey, "codex_app"),
      thread_label: longThreadLabel,
    };
    const detail: SessionDetailViewModel = {
      header: longThreadLabel,
      identity: `/tmp/builder-panel-app / ${longThreadLabel}`,
      usage: "5H --，本周 --",
      summary: {
        text: "摘要",
        full_text: "摘要",
        truncated: false,
        max_chars: 240,
      },
      execution_info: "完成",
      pending_interaction: null,
      pending_interaction_id: null,
      pending_interaction_kind: null,
      reply_box: {
        enabled: false,
        disabled_reason: "当前会话不支持回复",
      },
      choice_box: {
        enabled: false,
        allows_multiple: false,
        choices: [],
        disabled_reason: "当前会话没有选项交互",
      },
      toolbar_actions: [],
    };
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionDetail, {
          detail,
          selectedSession,
          draft: "",
          selectedChoiceValues: [],
          settings: defaultSettings(),
          submittingInteractionId: null,
          onDraftChange: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onToggleChoice: () => undefined,
          onSubmitChoice: () => undefined,
          onCreateFollowupTurn: () => undefined,
        }),
      );
    });

    expect(container.querySelector(".session-detail strong")?.textContent).toBe(
      longThreadLabel,
    );
    expect(container.querySelector(".session-detail p")?.textContent).toBe(
      `/tmp/builder-panel-app / ${longThreadLabel}`,
    );

    const detailButton = Array.from(container.querySelectorAll("button")).find(
      (button) => button.textContent === "详情",
    );
    await act(async () => {
      detailButton?.click();
    });

    expect(
      container.querySelector(".session-detail-overlay p")?.textContent,
    ).toBe(`Codex / ${longThreadLabel}`);

    await act(async () => {
      root.unmount();
    });
  });

  it("removes shortcut inputs from waiting text reply detail", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const selectedSession = waitingTextReplySession();
    const detail: SessionDetailViewModel = {
      header: "等待回复",
      identity: "/tmp/builder-panel-app / 等待回复",
      usage: "5H --，本周 --",
      summary: {
        text: "请选择下一步",
        full_text: "请选择下一步",
        truncated: false,
        max_chars: 240,
      },
      execution_info: "等待回复",
      pending_interaction: "请选择下一步",
      pending_interaction_id: { value: "reply-1" },
      pending_interaction_kind: "text_reply",
      reply_box: {
        enabled: true,
        disabled_reason: null,
      },
      choice_box: {
        enabled: false,
        allows_multiple: false,
        choices: [],
        disabled_reason: "当前会话没有选项交互",
      },
      toolbar_actions: ["send_reply"],
    };
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionDetail, {
          detail,
          selectedSession,
          draft: "",
          selectedChoiceValues: [],
          settings: defaultSettings(),
          submittingInteractionId: null,
          onDraftChange: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onToggleChoice: () => undefined,
          onSubmitChoice: () => undefined,
          onCreateFollowupTurn: () => undefined,
        }),
      );
    });

    const replyInline = container.querySelector(".reply-inline");
    const buttons = Array.from(replyInline?.querySelectorAll("button") ?? []);

    expect(replyInline?.querySelector(".shortcut-row")).toBeNull();
    expect(buttons.map((button) => button.textContent)).toEqual(["打开回复"]);

    await act(async () => {
      root.unmount();
    });
  });

  it("adds choice tooltips in session detail", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const selectedSession = waitingChoiceSession();
    const detail: SessionDetailViewModel = {
      header: "等待选择",
      identity: "/tmp/builder-panel-app / 等待选择",
      usage: "5H --，本周 --",
      summary: {
        text: "请选择下一步",
        full_text: "请选择下一步",
        truncated: false,
        max_chars: 240,
      },
      execution_info: "等待回复",
      pending_interaction: "请选择下一步",
      pending_interaction_id: { value: "choice-1" },
      pending_interaction_kind: "choice",
      reply_box: {
        enabled: false,
        disabled_reason: "当前会话没有文本回复交互",
      },
      choice_box: {
        enabled: true,
        allows_multiple: false,
        choices: selectedSession.inline_interaction.choice_box.choices,
        disabled_reason: null,
      },
      toolbar_actions: ["send_reply"],
    };
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionDetail, {
          detail,
          selectedSession,
          draft: "",
          selectedChoiceValues: [],
          settings: defaultSettings(),
          submittingInteractionId: null,
          onDraftChange: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onToggleChoice: () => undefined,
          onSubmitChoice: () => undefined,
          onCreateFollowupTurn: () => undefined,
        }),
      );
    });

    const choiceRows = Array.from(
      container.querySelectorAll<HTMLLabelElement>(".choice-row"),
    );

    expect(choiceRows[0].getAttribute("title")).toBe(
      "接入已验证的 app-server `turn/interrupt`；Codex CLI 继续显示禁用占位，风险最小。",
    );
    expect(choiceRows[1].getAttribute("title")).toBe(
      "同时规划 CLI 停止能力；但当前仓库没有稳定 CLI 停止协议事实，需要额外探查和更大改动。",
    );

    await act(async () => {
      root.unmount();
    });
  });

  it("only auto-shows approval action rows", () => {
    expect(shouldAutoShowSessionActionRow("waiting_for_approval")).toBe(true);
    expect(shouldAutoShowSessionActionRow("waiting_for_answer")).toBe(false);
    expect(shouldAutoShowSessionActionRow("completed")).toBe(false);
    expect(shouldAutoShowSessionActionRow("failed")).toBe(false);
  });

  it("uses hover expansion for waiting answers and follow-up rows", () => {
    expect(
      shouldHoverExpandSessionActionRow("waiting_for_answer", false, true),
    ).toBe(true);
    expect(
      shouldHoverExpandSessionActionRow("waiting_for_answer", false, false),
    ).toBe(false);
    expect(
      shouldHoverExpandSessionActionRow("waiting_for_approval", false),
    ).toBe(false);
    expect(shouldHoverExpandSessionActionRow("completed", true)).toBe(true);
    expect(shouldHoverExpandSessionActionRow("failed", true)).toBe(true);
    expect(shouldHoverExpandSessionActionRow("completed", false)).toBe(false);
  });

  it("keeps interaction summaries for approval and answer rows", () => {
    expect(shouldShowInteractionSummary("waiting_for_approval")).toBe(true);
    expect(shouldShowInteractionSummary("waiting_for_answer")).toBe(true);
    expect(shouldShowInteractionSummary("completed")).toBe(false);
  });

  it("renders completed and failed follow-up rows for hover expansion", () => {
    expect(canToggleFollowupRow("failed", true)).toBe(true);
    expect(canToggleFollowupRow("completed", true)).toBe(true);
    expect(canToggleFollowupRow("failed", false)).toBe(false);
    expect(shouldShowSessionActionRow("completed", true)).toBe(true);
    expect(shouldShowSessionActionRow("failed", true)).toBe(true);
    expect(shouldShowSessionActionRow("completed", false)).toBe(false);
    expect(shouldUseFollowupShortcut("failed", true)).toBe(true);
    expect(shouldUseFollowupShortcut("completed", true)).toBe(true);
    expect(shouldUseFollowupShortcut("running", true)).toBe(false);
  });

  it("requests layout resize when hover-expanded rows change visibility", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const onLayoutChange = vi.fn();
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions: [followupSession("completed", "已完成")],
          settings: defaultSettings(),
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump: () => undefined,
          onLayoutChange,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const row = container.querySelector<HTMLElement>(".session-table-row");
    const actionRow = container.querySelector<HTMLElement>(
      ".session-row-action-followup",
    );
    expect(row).not.toBeNull();
    expect(actionRow).not.toBeNull();

    await act(async () => {
      row?.dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
    });
    await act(async () => {
      row?.dispatchEvent(new FocusEvent("focusout", { bubbles: true }));
    });
    await act(async () => {
      const transitionEvent = new Event("transitionend", { bubbles: true });
      Object.defineProperty(transitionEvent, "propertyName", {
        value: "max-height",
      });
      actionRow?.dispatchEvent(transitionEvent);
    });

    expect(onLayoutChange).toHaveBeenCalledTimes(3);

    await act(async () => {
      root.unmount();
    });
  });

  it("uses the compact two-column action row for manual follow-up expansion", () => {
    expect(sessionActionRowClassName("completed")).toContain(
      "session-row-action-followup",
    );
    expect(sessionActionRowClassName("failed")).toContain(
      "session-row-action-followup",
    );
    expect(sessionActionRowClassName("waiting_for_answer", true)).toContain(
      "session-row-action-hover",
    );
    expect(sessionActionRowClassName("waiting_for_answer", true)).toContain(
      "session-row-action-choice",
    );
  });

  it("renders waiting choice options in hover row without shortcut inputs", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const session = waitingChoiceSession();
    const onToggleChoice = vi.fn();
    const onSubmitChoice = vi.fn();
    const container = document.createElement("div");
    const root = createRoot(container);
    const mockUiState = {
      ...createDefaultMockPanelUiState(),
      selectedChoicesByInteractionId: {
        "choice-1": ["仅 Codex APP (Recommended)"],
      },
    };

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState,
          currentTimeMs: 2000,
          sessions: [session],
          settings: defaultSettings(),
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice,
          onToggleChoice,
        }),
      );
    });

    const row = container.querySelector<HTMLElement>(".session-table-row");
    const actionRow = container.querySelector(".session-row-action-hover");
    const choiceRows = Array.from(
      container.querySelectorAll<HTMLLabelElement>(".choice-row"),
    );
    const submitButton = Array.from(container.querySelectorAll("button")).find(
      (button) => button.textContent === "提交选择",
    );

    expect(row?.getAttribute("tabindex")).toBe("0");
    expect(row?.getAttribute("aria-label")).toContain("等待回复");
    expect(actionRow).not.toBeNull();
    expect(actionRow?.querySelector(".shortcut-row")).toBeNull();
    expect(choiceRows).toHaveLength(2);
    expect(choiceRows[0].textContent).toBe("仅 Codex APP (Recommended)");
    expect(choiceRows[1].textContent).toBe("Codex APP + CLI");
    expect(choiceRows[0].getAttribute("title")).toBe(
      "接入已验证的 app-server `turn/interrupt`；Codex CLI 继续显示禁用占位，风险最小。",
    );
    expect(choiceRows[1].getAttribute("title")).toBe(
      "同时规划 CLI 停止能力；但当前仓库没有稳定 CLI 停止协议事实，需要额外探查和更大改动。",
    );

    await act(async () => {
      choiceRows[1].querySelector("input")?.click();
    });

    expect(onToggleChoice).toHaveBeenCalledWith(
      { value: "choice-1" },
      "Codex APP + CLI",
      false,
    );

    await act(async () => {
      submitButton?.click();
    });

    expect(onSubmitChoice).toHaveBeenCalledWith(
      session,
      { value: "choice-1" },
      ["仅 Codex APP (Recommended)"],
    );

    await act(async () => {
      root.unmount();
    });
  });

  it("hides the second row for external-only waiting answers", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const session = waitingExternalTextReplySession();
    const onToggleChoice = vi.fn();
    const onSubmitChoice = vi.fn();
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions: [session],
          settings: defaultSettings(),
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice,
          onToggleChoice,
        }),
      );
    });

    expect(container.textContent).toContain("请选择停止能力范围");
    expect(container.querySelector(".session-row-action-hover")).toBeNull();
    expect(container.querySelector(".choice-row")).toBeNull();
    expect(container.textContent).not.toContain("仅 Codex APP");
    expect(container.textContent).not.toContain("Codex APP + CLI");
    expect(container.textContent).not.toContain("提交选择");

    expect(onToggleChoice).not.toHaveBeenCalled();
    expect(onSubmitChoice).not.toHaveBeenCalled();

    await act(async () => {
      root.unmount();
    });
  });

  it("renders waiting text reply without shortcut inputs", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const session = waitingTextReplySession();
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions: [session],
          settings: defaultSettings(),
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const actionRow = container.querySelector(".session-row-action-hover");
    const textArea = actionRow?.querySelector("textarea");
    const sendButton = Array.from(
      actionRow?.querySelectorAll("button") ?? [],
    ).find((button) => button.textContent === "发送");

    expect(actionRow?.querySelector(".shortcut-row")).toBeNull();
    expect(textArea?.getAttribute("placeholder")).toBe("输入回复");
    expect(sendButton).not.toBeNull();

    await act(async () => {
      root.unmount();
    });
  });

  it("renders completed follow-up input before shortcut inputs", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const session = followupSession("completed", "已完成");
    const onCreateFollowupTurn = vi.fn();
    const container = document.createElement("div");
    const root = createRoot(container);
    const draft = "继续处理";
    const mockUiState = {
      ...createDefaultMockPanelUiState(),
      draftsBySessionId: {
        [sessionKeyToId(session.session_key)]: draft,
      },
    };

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState,
          currentTimeMs: 2000,
          sessions: [session],
          settings: defaultSettings(),
          selectedSessionId: null,
          onCreateFollowupTurn,
          onDraftChange: () => undefined,
          onJump: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const actionRow = container.querySelector(".session-row-action-followup");
    const followupInput = actionRow?.querySelector(".inline-reply-followup");
    const shortcutRow = actionRow?.querySelector(".shortcut-row");
    const followupChildren = Array.from(followupInput?.children ?? []);
    const textarea = followupInput?.querySelector("textarea");
    const sendButton = followupInput?.querySelector<HTMLButtonElement>(
      "button[aria-label='发送']",
    );

    expect(actionRow?.children[0]).toBe(followupInput);
    expect(actionRow?.children[1]).toBe(shortcutRow);
    expect(followupChildren[0]).toBe(textarea);
    expect(followupChildren[1]).toBe(sendButton);
    expect(sendButton?.getAttribute("title")).toBe("发送");
    expect(sendButton?.textContent).toBe("");
    expectThinAbsoluteSvgStroke(
      sendButton?.querySelector<SVGElement>("svg") ?? null,
    );

    await act(async () => {
      sendButton?.click();
    });

    expect(onCreateFollowupTurn).toHaveBeenCalledTimes(1);
    expect(onCreateFollowupTurn).toHaveBeenCalledWith(session, draft);

    await act(async () => {
      root.unmount();
    });
  });

  it("uses the same follow-up layout for failed sessions", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const session = followupSession("failed", "失败");
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions: [session],
          settings: defaultSettings(),
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const actionRow = container.querySelector(".session-row-action-followup");
    const followupInput = actionRow?.querySelector(".inline-reply-followup");
    const shortcutRow = actionRow?.querySelector(".shortcut-row");

    expect(actionRow?.children[0]).toBe(followupInput);
    expect(actionRow?.children[1]).toBe(shortcutRow);

    await act(async () => {
      root.unmount();
    });
  });

  it("lets follow-up input occupy the whole row without shortcut inputs", async () => {
    const reactActGlobal = globalThis as typeof globalThis & {
      IS_REACT_ACT_ENVIRONMENT?: boolean;
    };
    reactActGlobal.IS_REACT_ACT_ENVIRONMENT = true;
    const session = followupSession("completed", "已完成");
    const settings = {
      ...defaultSettings(),
      replies: {
        ...defaultSettings().replies,
        shortcut_replies_enabled: false,
      },
    };
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(SessionStream, {
          mockUiState: createDefaultMockPanelUiState(),
          currentTimeMs: 2000,
          sessions: [session],
          settings,
          selectedSessionId: null,
          onCreateFollowupTurn: () => undefined,
          onDraftChange: () => undefined,
          onJump: () => undefined,
          onResolveApproval: () => undefined,
          onSendReply: () => undefined,
          onSubmitChoice: () => undefined,
          onToggleChoice: () => undefined,
        }),
      );
    });

    const actionRow = container.querySelector(".session-row-action-followup");
    const followupInput = actionRow?.querySelector(".inline-reply-followup");

    expect(actionRow?.querySelector(".shortcut-row")).toBeNull();
    expect(actionRow?.children).toHaveLength(1);
    expect(actionRow?.children[0]).toBe(followupInput);

    await act(async () => {
      root.unmount();
    });
  });

  it("uses stable labels for executable capability actions", () => {
    expect(actionLabel("resolve_approval")).toBe("审批");
    expect(actionLabel("send_reply")).toBe("回复");
    expect(actionLabel("create_followup_turn")).toBe("新对话");
  });

  it("ignores stale settings save responses", () => {
    expect(isLatestSettingsSaveResponse(1, 2)).toBe(false);
    expect(isLatestSettingsSaveResponse(2, 2)).toBe(true);
  });

  it("applies window preferences for the latest settings save response", async () => {
    const applyPreferences = vi.fn<() => Promise<void>>();
    const settings = {
      ...defaultSettings(),
      general: {
        ...defaultSettings().general,
        keep_panel_on_top: false,
      },
    };

    const applied = await applyLatestSavedPanelWindowPreferences(
      true,
      2,
      2,
      settings,
      applyPreferences,
    );

    expect(applied).toBe(true);
    expect(applyPreferences).toHaveBeenCalledTimes(1);
    expect(applyPreferences).toHaveBeenCalledWith(settings);
  });

  it("does not apply window preferences for stale settings save responses", async () => {
    const applyPreferences = vi.fn<() => Promise<void>>();

    const applied = await applyLatestSavedPanelWindowPreferences(
      true,
      1,
      2,
      defaultSettings(),
      applyPreferences,
    );

    expect(applied).toBe(false);
    expect(applyPreferences).not.toHaveBeenCalled();
  });

  it("keeps window preferences untouched when settings save fails", async () => {
    const applyPreferences = vi.fn<() => Promise<void>>();

    const applied = await applyLatestSavedPanelWindowPreferences(
      false,
      2,
      2,
      defaultSettings(),
      applyPreferences,
    );

    expect(applied).toBe(false);
    expect(applyPreferences).not.toHaveBeenCalled();
  });

  it("creates default hook install statuses for the settings list", () => {
    const statuses = defaultHookAgentStatuses();

    expect(statuses.map((status) => status.agent)).toEqual(["codex", "claude"]);
    expect(statuses.every((status) => status.can_install)).toBe(true);
    expect(statuses.every((status) => status.can_uninstall)).toBe(false);
  });

  it("disables duplicate hook install and uninstall actions", () => {
    const installed = {
      ...defaultHookAgentStatuses()[0],
      state: "installed" as const,
      message: "已安装",
      can_install: false,
      can_uninstall: true,
    };
    const notInstalled = defaultHookAgentStatuses()[1];

    expect(isHookActionDisabled(installed, "install", null)).toBe(true);
    expect(isHookActionDisabled(installed, "uninstall", null)).toBe(false);
    expect(isHookActionDisabled(notInstalled, "install", null)).toBe(false);
    expect(isHookActionDisabled(notInstalled, "uninstall", null)).toBe(true);
    expect(isHookActionDisabled(installed, "uninstall", "codex")).toBe(true);
  });

  it("merges panel window updates during the debounce window", () => {
    const merged = mergePanelWindowStateUpdate(
      {
        window_position: { x: 12, y: 20 },
      },
      {
        window_width: 860,
      },
    );

    expect(merged).toEqual({
      window_position: { x: 12, y: 20 },
      window_width: 860,
    });
  });

  it("keeps the latest window geometry when another setting is saved", () => {
    const settings = defaultSettings();
    const nextSettings = mergePanelWindowStateIntoSettings(settings, {
      window_position: { x: -120, y: 48 },
      window_width: 860,
    });

    expect(nextSettings.panel.window_position).toEqual({ x: -120, y: 48 });
    expect(nextSettings.panel.window_width).toBe(860);
    expect(nextSettings.display).toEqual(settings.display);
  });

  it("keeps moved positions and changed widths for persistence", () => {
    expect(
      panelWindowUpdateForPersistence(
        { window_position: { x: 12, y: 20 } },
        665,
      ),
    ).toEqual({ window_position: { x: 12, y: 20 } });
    expect(panelWindowUpdateForPersistence({ window_width: 860 }, 665)).toEqual(
      { window_width: 860 },
    );
  });

  it("drops repeated width updates caused by programmatic height resize", () => {
    expect(
      panelWindowUpdateForPersistence({ window_width: 666 }, 665),
    ).toBeNull();
  });

  it("changes panel resize signature when visible summary text changes", () => {
    const settings = defaultSettings();
    const shortSession = {
      ...sessionItem(codexAppSessionKey, "codex_app", "running", "运行中"),
      summary: textDisplay("短摘要"),
    };
    const longSession = {
      ...shortSession,
      summary: textDisplay("更长的摘要会让当前输出区域换行并改变内容高度"),
    };

    const shortSignature = panelWindowContentResizeSignature({
      errorMessage: null,
      settings,
      settingsModalOpen: false,
      sessions: [shortSession],
    });
    const longSignature = panelWindowContentResizeSignature({
      errorMessage: null,
      settings,
      settingsModalOpen: false,
      sessions: [longSession],
    });

    expect(longSignature).not.toBe(shortSignature);
  });

  it("ignores settings that do not affect panel content height in resize signature", () => {
    const settings = defaultSettings();
    const session = sessionItem(
      codexAppSessionKey,
      "codex_app",
      "running",
      "运行中",
    );
    const baseSignature = panelWindowContentResizeSignature({
      errorMessage: null,
      settings,
      settingsModalOpen: false,
      sessions: [session],
    });
    const nextSignature = panelWindowContentResizeSignature({
      errorMessage: null,
      settings: {
        ...settings,
        terminal: {
          ...settings.terminal,
          jump_enabled: false,
        },
      },
      settingsModalOpen: false,
      sessions: [session],
    });

    expect(nextSignature).toBe(baseSignature);
  });

  it("changes panel resize signature when configured max window height changes", () => {
    const settings = defaultSettings();
    const session = sessionItem(
      codexAppSessionKey,
      "codex_app",
      "running",
      "运行中",
    );
    const baseSignature = panelWindowContentResizeSignature({
      errorMessage: null,
      settings,
      settingsModalOpen: false,
      sessions: [session],
    });
    const nextSignature = panelWindowContentResizeSignature({
      errorMessage: null,
      settings: {
        ...settings,
        panel: {
          ...settings.panel,
          max_window_height: 520,
        },
      },
      settingsModalOpen: false,
      sessions: [session],
    });

    expect(nextSignature).not.toBe(baseSignature);
  });

  it("changes panel resize signature when the settings overlay opens", () => {
    const settings = defaultSettings();
    const session = sessionItem(
      codexAppSessionKey,
      "codex_app",
      "running",
      "运行中",
    );
    const closedSignature = panelWindowContentResizeSignature({
      errorMessage: null,
      settings,
      settingsModalOpen: false,
      sessions: [session],
    });
    const openSignature = panelWindowContentResizeSignature({
      errorMessage: null,
      settings,
      settingsModalOpen: true,
      sessions: [session],
    });

    expect(openSignature).not.toBe(closedSignature);
  });

  it("keeps submitting state when panel window controls fail", () => {
    const current = {
      ...createDefaultMockPanelUiState(),
      submittingInteractionId: "approval-1",
    };

    const next = applyPanelWindowControlError(current, "最小化窗口失败");

    expect(next.errorMessage).toBe("最小化窗口失败");
    expect(next.submittingInteractionId).toBe("approval-1");
  });

  it("marks title text as drag region without marking window buttons", async () => {
    const container = document.createElement("div");
    const root = createRoot(container);

    await act(async () => {
      root.render(
        createElement(PanelShell, {
          actions: createElement(PanelTitleActions, {
            onClose: vi.fn(),
            onMinimize: vi.fn(),
            onOpenSettings: vi.fn(),
          }),
          children: createElement("div"),
          title: "Builder Panel",
          titleMeta: createElement(
            "span",
            { "data-tauri-drag-region": true },
            "运行 0 等待 0 总计 1",
          ),
        }),
      );
    });

    const title = container.querySelector("h1");
    const statusDot = container.querySelector(".panel-status-dot");
    const minimizeButton = container.querySelector(
      "button[aria-label='最小化']",
    );

    expect(title?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(statusDot?.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(minimizeButton?.hasAttribute("data-tauri-drag-region")).toBe(false);

    await act(async () => {
      root.unmount();
    });
  });

  it("formats active session elapsed time with hour rollover", () => {
    expect(elapsedDurationLabel(1000, 62_000)).toBe("01:01");
    expect(elapsedDurationLabel(1000, 3_663_000)).toBe("01:01:02");
    expect(elapsedDurationLabel(3000, 1000)).toBe("00:00");
  });

  it("formats completed session relative time", () => {
    expect(relativePastLabel(10_000, 59_000)).toBe("刚刚");
    expect(relativePastLabel(10_000, 130_000)).toBe("2 分钟前");
    expect(relativePastLabel(10_000, 7_210_000)).toBe("2 小时前");
  });

  it("uses started and completed timestamps for the session side label", () => {
    const running = sessionItem(
      sessionKey("project-a", "running"),
      "codex_app",
      "running",
      "运行中",
      10_000,
    );
    const completed = {
      ...sessionItem(
        sessionKey("project-a", "completed"),
        "codex_app",
        "completed",
        "已完成",
        30_000,
      ),
      completed_at: { value: 30_000 },
    };

    expect(sessionSideTimeLabel(running, 72_000)).toBe("01:02");
    expect(sessionSideTimeLabel(completed, 150_000)).toBe("2 分钟前");
  });
});

const sessionItem = (
  sessionKey: SessionKey,
  runtimeSource: PanelSessionListItem["runtimeSource"],
  statusKind: PanelSessionListItem["status_kind"] = "waiting_for_approval",
  statusLabel = "等待审批",
  updatedAt = 1000,
): PanelSessionListItem => ({
  runtimeSource,
  session_key: sessionKey,
  agent_label: sessionKey.agent_kind,
  project_label: sessionKey.project_id.value,
  thread_label: `Thread ${sessionKey.conversation_id.value}`,
  conversation_label: sessionKey.conversation_id.value,
  status_label: statusLabel,
  status_kind: statusKind,
  summary: {
    text: "等待 Codex 审批",
    full_text: "等待 Codex 审批",
    truncated: false,
    max_chars: 120,
  },
  updated_at_label: updatedAt.toString(),
  started_at: { value: updatedAt },
  completed_at:
    statusKind === "completed" || statusKind === "failed"
      ? { value: updatedAt }
      : null,
  usage_5h: {
    value_label: "--",
    amount_label: null,
    unit: null,
    source_key: null,
    source_label: null,
    scope: null,
    updated_at: null,
  },
  usage_weekly: {
    value_label: "--",
    amount_label: null,
    unit: null,
    source_key: null,
    source_label: null,
    scope: null,
    updated_at: null,
  },
  actions: ["resolve_approval"],
  inline_interaction: {
    summary: "等待 Codex 审批",
    interaction_id: { value: "approval-1" },
    kind: "approval",
    can_jump: false,
    can_send_reply: false,
    can_resolve_approval: true,
    can_create_followup_turn: false,
    choice_box: {
      enabled: false,
      allows_multiple: false,
      choices: [],
      disabled_reason: "当前会话没有选项交互",
    },
  },
  indent_level: 0,
});

const followupSession = (
  statusKind: "completed" | "failed",
  statusLabel: string,
): PanelSessionListItem => ({
  ...sessionItem(codexAppSessionKey, "codex_app", statusKind, statusLabel),
  actions: ["create_followup_turn"],
  inline_interaction: {
    summary: null,
    interaction_id: null,
    kind: "text_reply",
    can_jump: false,
    can_send_reply: false,
    can_resolve_approval: false,
    can_create_followup_turn: true,
    choice_box: {
      enabled: false,
      allows_multiple: false,
      choices: [],
      disabled_reason: "当前会话没有选项交互",
    },
  },
});

const waitingChoiceSession = (): PanelSessionListItem => ({
  ...sessionItem(
    codexAppSessionKey,
    "codex_app",
    "waiting_for_answer",
    "等待回复",
  ),
  actions: ["send_reply"],
  summary: {
    text: "请选择下一步",
    full_text: "请选择下一步",
    truncated: false,
    max_chars: 120,
  },
  inline_interaction: {
    summary: "请选择下一步",
    interaction_id: { value: "choice-1" },
    kind: "choice",
    can_jump: false,
    can_send_reply: true,
    can_resolve_approval: false,
    can_create_followup_turn: false,
    choice_box: {
      enabled: true,
      allows_multiple: false,
      choices: [
        {
          value: "仅 Codex APP (Recommended)",
          label: "仅 Codex APP (Recommended)",
          tooltip:
            "接入已验证的 app-server `turn/interrupt`；Codex CLI 继续显示禁用占位，风险最小。",
        },
        {
          value: "Codex APP + CLI",
          label: "Codex APP + CLI",
          tooltip:
            "同时规划 CLI 停止能力；但当前仓库没有稳定 CLI 停止协议事实，需要额外探查和更大改动。",
        },
      ],
      disabled_reason: null,
    },
  },
});

const waitingExternalTextReplySession = (): PanelSessionListItem => ({
  ...sessionItem(
    codexAppSessionKey,
    "codex_app",
    "waiting_for_answer",
    "等待回复",
  ),
  actions: [],
  summary: {
    text: "请选择停止能力范围",
    full_text: "请选择停止能力范围",
    truncated: false,
    max_chars: 120,
  },
  inline_interaction: {
    summary: "请选择停止能力范围",
    interaction_id: { value: "external-reply-1" },
    kind: "text_reply",
    can_jump: false,
    can_send_reply: false,
    can_resolve_approval: false,
    can_create_followup_turn: false,
    choice_box: {
      enabled: false,
      allows_multiple: false,
      choices: [],
      disabled_reason: "当前会话没有选项交互",
    },
  },
});

const waitingTextReplySession = (): PanelSessionListItem => ({
  ...sessionItem(
    codexAppSessionKey,
    "codex_app",
    "waiting_for_answer",
    "等待回复",
  ),
  actions: ["send_reply"],
  inline_interaction: {
    summary: "请输入回复",
    interaction_id: { value: "reply-1" },
    kind: "text_reply",
    can_jump: false,
    can_send_reply: true,
    can_resolve_approval: false,
    can_create_followup_turn: false,
    choice_box: {
      enabled: false,
      allows_multiple: false,
      choices: [],
      disabled_reason: "当前会话没有选项交互",
    },
  },
});

const sessionKey = (projectId: string, conversationId: string): SessionKey => ({
  agent_kind: "codex_cli",
  project_id: { value: projectId },
  conversation_id: { value: conversationId },
});

const mockTooltipLabelInlineSize = (
  initialScrollWidth: number,
  initialClientWidth: number,
): {
  readonly restore: () => void;
  readonly setSize: (scrollWidth: number, clientWidth: number) => void;
} => {
  let scrollWidth = initialScrollWidth;
  let clientWidth = initialClientWidth;
  const originalScrollWidth = Object.getOwnPropertyDescriptor(
    HTMLElement.prototype,
    "scrollWidth",
  );
  const originalClientWidth = Object.getOwnPropertyDescriptor(
    HTMLElement.prototype,
    "clientWidth",
  );
  Object.defineProperty(HTMLElement.prototype, "scrollWidth", {
    configurable: true,
    get() {
      return this instanceof Element &&
        this.classList.contains("markdown-tooltip-label")
        ? scrollWidth
        : 0;
    },
  });
  Object.defineProperty(HTMLElement.prototype, "clientWidth", {
    configurable: true,
    get() {
      return this instanceof Element &&
        this.classList.contains("markdown-tooltip-label")
        ? clientWidth
        : 0;
    },
  });

  return {
    restore: () => {
      restorePropertyDescriptor(
        HTMLElement.prototype,
        "scrollWidth",
        originalScrollWidth,
      );
      restorePropertyDescriptor(
        HTMLElement.prototype,
        "clientWidth",
        originalClientWidth,
      );
    },
    setSize: (nextScrollWidth, nextClientWidth) => {
      scrollWidth = nextScrollWidth;
      clientWidth = nextClientWidth;
    },
  };
};

const mockTooltipLabelBlockSize = (
  initialScrollHeight: number,
  initialClientHeight: number,
): {
  readonly restore: () => void;
  readonly setSize: (scrollHeight: number, clientHeight: number) => void;
} => {
  let scrollHeight = initialScrollHeight;
  let clientHeight = initialClientHeight;
  const originalScrollHeight = Object.getOwnPropertyDescriptor(
    HTMLElement.prototype,
    "scrollHeight",
  );
  const originalClientHeight = Object.getOwnPropertyDescriptor(
    HTMLElement.prototype,
    "clientHeight",
  );
  Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
    configurable: true,
    get() {
      return this instanceof Element &&
        this.classList.contains("markdown-tooltip-label")
        ? scrollHeight
        : 0;
    },
  });
  Object.defineProperty(HTMLElement.prototype, "clientHeight", {
    configurable: true,
    get() {
      return this instanceof Element &&
        this.classList.contains("markdown-tooltip-label")
        ? clientHeight
        : 0;
    },
  });

  return {
    restore: () => {
      restorePropertyDescriptor(
        HTMLElement.prototype,
        "scrollHeight",
        originalScrollHeight,
      );
      restorePropertyDescriptor(
        HTMLElement.prototype,
        "clientHeight",
        originalClientHeight,
      );
    },
    setSize: (nextScrollHeight, nextClientHeight) => {
      scrollHeight = nextScrollHeight;
      clientHeight = nextClientHeight;
    },
  };
};

const restorePropertyDescriptor = (
  target: object,
  property: string,
  descriptor: PropertyDescriptor | undefined,
): void => {
  if (descriptor === undefined) {
    delete (target as Record<string, unknown>)[property];
    return;
  }

  Object.defineProperty(target, property, descriptor);
};

const verifiedUsage = (
  valueLabel: string,
  sourceKey: string,
  updatedAt: number,
  scope: PanelSessionListItem["usage_5h"]["scope"] = "account_window",
): PanelSessionListItem["usage_5h"] => {
  const [amountLabel, unit] = valueLabel.split(" ");
  return {
    value_label: valueLabel,
    amount_label: amountLabel,
    unit: unit ?? null,
    source_key: sourceKey,
    source_label: "test",
    scope,
    updated_at: { value: updatedAt },
  };
};

const textDisplay = (text: string): PanelSessionListItem["summary"] => ({
  text,
  full_text: text,
  truncated: false,
  max_chars: 120,
});

const flushPromises = async (): Promise<void> => {
  await Promise.resolve();
  await Promise.resolve();
};
