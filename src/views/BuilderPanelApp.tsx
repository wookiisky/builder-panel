import { useEffect, useMemo, useRef, useState } from "react";

import {
  fetchCodexCliSessionDetail,
  fetchCodexCliSessions,
  fetchCodexCliTimeline,
  releaseCodexCliTimelineCache,
  submitCodexCliApproval,
} from "../api/codexCliPanelApi";
import {
  installHooks,
  previewHookInstall,
  uninstallHooks,
  type HookInstallAgent,
  type HookInstallPreview,
} from "../api/hookInstallApi";
import {
  fetchMockSessionDetail,
  fetchMockSessions,
  fetchMockTimeline,
  releaseMockTimelineCache,
  submitMockApproval,
  submitMockChoice,
  submitMockReply,
} from "../api/mockPanelApi";
import {
  applyPanelWindowGeometry,
  savePanelWindowState,
  subscribePanelWindowGeometry,
  type PanelWindowStateUpdate,
} from "../api/panelWindowApi";
import type {
  ApprovalDecision,
  ProcessTimelineItem,
  SessionDetailViewModel,
  SessionKey,
  SessionListItemViewModel,
  TimelinePage,
  TimelineQuery,
  UiAction,
} from "../api/mockPanelContract";
import { fetchPanelProbe } from "../api/panelProbeApi";
import type { PanelProbeView } from "../api/panelProbeContract";
import {
  defaultSettings,
  fetchPanelSettings,
  savePanelSettings,
} from "../api/settingsApi";
import type {
  BuilderPanelSettings,
  SettingsViewModel,
} from "../api/settingsContract";
import { PanelShell } from "../components/PanelShell";
import { SettingsPanel } from "../components/SettingsPanel";
import {
  beginSubmit,
  beginTimelineLoad,
  clearChoiceSelection,
  clearDraft,
  closeTimeline,
  countReplyChars,
  createDefaultMockPanelUiState,
  endSubmit,
  failTimelineLoad,
  isReplyDraftInvalid,
  openTimeline,
  selectSession,
  sessionKeyToId,
  setTimelinePage,
  timelinePageToCopyText,
  timelineVisibleRange,
  toggleChoiceSelection,
  updateDraft,
  updateTimelineKind,
  updateTimelineSearch,
  type MockPanelUiState,
  type TimelineKindFilter,
} from "../stores/mockPanelStore";
import {
  createDefaultPanelUiState,
  togglePanelCollapsed,
  type PanelUiState,
} from "../stores/panelProbeStore";

/// 会话运行时来源。
type RuntimeSource = "mock" | "codex_cli";

/// 主工作区视图。
type PanelSection = "sessions" | "settings";

/// hook 安装前端状态。
interface HookInstallUiState {
  /// 当前选择的安装目标。
  readonly selectedAgents: readonly HookInstallAgent[];
  /// 当前预览对应的安装目标。
  readonly previewAgents: readonly HookInstallAgent[] | null;
  /// 当前预览结果。
  readonly preview: HookInstallPreview | null;
  /// 当前状态提示。
  readonly statusMessage: string | null;
  /// 是否正在执行 hook 操作。
  readonly working: boolean;
}

/// 前端合并后的 session 列表项。
export type PanelSessionListItem = SessionListItemViewModel & {
  /// 该 session 来源，用于隔离 mock 专用控制。
  readonly runtimeSource: RuntimeSource;
};

/// Codex CLI hook 写入 runtime 后，前端刷新 session 的间隔。
export const SESSION_REFRESH_INTERVAL_MS = 1000;

/// 内置快捷回复，用于阶段 5 本地闭环演练。
const DEFAULT_SHORTCUT_REPLIES = [
  {
    id: "continue",
    label: "继续",
    content: "继续按当前方案执行。",
  },
  {
    id: "need-boundary",
    label: "补充边界",
    content: "请优先说明输入、输出、边界条件和失败处理。",
  },
] as const;

/// Timeline 单次读取页大小。
const TIMELINE_PAGE_SIZE = 50;
/// Timeline 虚拟列表固定行高。
const TIMELINE_ROW_HEIGHT = 82;
/// Timeline 虚拟列表视口高度。
const TIMELINE_VIEWPORT_HEIGHT = 330;
/// Timeline 虚拟列表额外渲染行数。
const TIMELINE_OVERSCAN = 4;

/// session 状态排序权重。
const SESSION_STATUS_ORDER: Record<
  SessionListItemViewModel["status_kind"],
  number
> = {
  waiting_for_approval: 0,
  waiting_for_answer: 0,
  running: 1,
  failed: 2,
  completed: 3,
  detached: 4,
};

/// Builder Panel 首屏应用。
export const BuilderPanelApp = () => {
  const [panelUiState, setPanelUiState] = useState<PanelUiState>(() =>
    createDefaultPanelUiState(),
  );
  const [mockUiState, setMockUiState] = useState<MockPanelUiState>(() =>
    createDefaultMockPanelUiState(),
  );
  const [panelProbe, setPanelProbe] = useState<PanelProbeView | null>(null);
  const [sessions, setSessions] = useState<readonly PanelSessionListItem[]>([]);
  const [selectedDetail, setSelectedDetail] =
    useState<SessionDetailViewModel | null>(null);
  const [activeSection, setActiveSection] = useState<PanelSection>("sessions");
  const [settingsView, setSettingsView] = useState<SettingsViewModel>(() => ({
    settings: defaultSettings(),
    status_message: null,
  }));
  const [settingsHydrated, setSettingsHydrated] = useState(false);
  const [settingsSaving, setSettingsSaving] = useState(false);
  const [hookInstallState, setHookInstallState] = useState<HookInstallUiState>(
    () => ({
      selectedAgents: ["codex", "claude"],
      previewAgents: null,
      preview: null,
      statusMessage: null,
      working: false,
    }),
  );
  const settingsSaveVersion = useRef(0);
  const panelGeometryApplied = useRef(false);
  const panelGeometrySaveTimer = useRef<number | null>(null);
  const pendingPanelWindowUpdate = useRef<PanelWindowStateUpdate>({});

  useEffect(() => {
    let disposed = false;

    fetchPanelProbe()
      .then((probe) => {
        if (!disposed) {
          setPanelProbe(probe);
        }
      })
      .catch(() => {
        if (!disposed) {
          setPanelProbe({
            mode: "expanded",
            collapsed: false,
            always_on_top: true,
            draggable: true,
          });
        }
      });

    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    let disposed = false;

    fetchPanelSettings()
      .then((view) => {
        if (!disposed) {
          setSettingsView(view);
          setPanelUiState({
            collapsed: view.settings.panel.collapsed,
          });
          setSettingsHydrated(true);
        }
      })
      .catch((error: unknown) => {
        if (!disposed) {
          const fallbackView = {
            settings: defaultSettings(),
            status_message: readableError(error, "读取设置失败"),
          };
          setSettingsView(fallbackView);
          setPanelUiState({
            collapsed: fallbackView.settings.panel.collapsed,
          });
          setSettingsHydrated(true);
        }
      });

    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    if (!settingsHydrated || panelGeometryApplied.current) {
      return;
    }

    panelGeometryApplied.current = true;
    applyPanelWindowGeometry(settingsView.settings.panel).catch(
      (error: unknown) => {
        setSettingsView((current) => ({
          ...current,
          status_message: readableError(error, "恢复 panel 窗口状态失败"),
        }));
      },
    );
  }, [settingsHydrated, settingsView.settings.panel]);

  useEffect(() => {
    if (!settingsHydrated) {
      return;
    }

    let disposed = false;
    let unsubscribe: (() => void) | null = null;
    subscribePanelWindowGeometry((update) => {
      if (!disposed) {
        schedulePanelWindowStateSave(update);
      }
    })
      .then((unlisten) => {
        if (disposed) {
          unlisten();
          return;
        }
        unsubscribe = unlisten;
      })
      .catch((error: unknown) => {
        setSettingsView((current) => ({
          ...current,
          status_message: readableError(error, "监听 panel 窗口状态失败"),
        }));
      });

    return () => {
      disposed = true;
      if (unsubscribe !== null) {
        unsubscribe();
      }
      if (panelGeometrySaveTimer.current !== null) {
        window.clearTimeout(panelGeometrySaveTimer.current);
      }
    };
  }, [settingsHydrated]);

  useEffect(() => {
    let disposed = false;
    let refreshing = false;

    const refresh = () => {
      if (refreshing) {
        return;
      }

      refreshing = true;
      fetchAllSessions(settingsView.settings)
        .then((items) => {
          if (disposed) {
            return;
          }
          setSessions(items);
          setMockUiState((current) =>
            selectFirstSessionWhenMissing(current, items),
          );
        })
        .catch((error: unknown) => {
          if (!disposed) {
            setMockUiState((current) =>
              endSubmit(current, readableError(error, "读取 session 失败")),
            );
          }
        })
        .finally(() => {
          refreshing = false;
        });
    };

    refresh();
    const timer = window.setInterval(refresh, SESSION_REFRESH_INTERVAL_MS);

    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [settingsView.settings]);

  const selectedSession = useMemo(() => {
    return (
      sessions.find(
        (session) =>
          panelSessionToId(session) === mockUiState.selectedSessionId,
      ) ?? null
    );
  }, [mockUiState.selectedSessionId, sessions]);

  useEffect(() => {
    let disposed = false;

    if (selectedSession === null) {
      setSelectedDetail(null);
      return;
    }

    fetchSessionDetail(selectedSession)
      .then((detail) => {
        if (!disposed) {
          setSelectedDetail(detail);
        }
      })
      .catch((error: unknown) => {
        if (!disposed) {
          setSelectedDetail(null);
          setMockUiState((current) =>
            endSubmit(current, readableError(error, "读取 session 详情失败")),
          );
        }
      });

    return () => {
      disposed = true;
    };
  }, [selectedSession]);

  useEffect(() => {
    let disposed = false;

    if (!mockUiState.timeline.open || selectedSession === null) {
      return;
    }

    setMockUiState((current) => beginTimelineLoad(current));
    fetchTimelinePage(selectedSession, {
      session_key: selectedSession.session_key,
      page: 0,
      page_size: TIMELINE_PAGE_SIZE,
      search:
        mockUiState.timeline.search.trim().length === 0
          ? null
          : mockUiState.timeline.search,
      kind:
        mockUiState.timeline.kind === "all" ? null : mockUiState.timeline.kind,
    })
      .then((page) => {
        if (!disposed) {
          setMockUiState((current) => setTimelinePage(current, page));
        }
      })
      .catch((error: unknown) => {
        if (!disposed) {
          setMockUiState((current) =>
            failTimelineLoad(
              current,
              readableError(error, "读取 timeline 失败"),
            ),
          );
        }
      });

    return () => {
      disposed = true;
    };
  }, [
    mockUiState.timeline.kind,
    mockUiState.timeline.open,
    mockUiState.timeline.search,
    selectedSession,
  ]);

  const collapsed = panelUiState.collapsed || panelProbe?.collapsed === true;
  const selectedDraft =
    selectedSession === null
      ? ""
      : (mockUiState.draftsBySessionId[
          sessionKeyToId(selectedSession.session_key)
        ] ?? "");
  const selectedChoiceValues =
    selectedDetail?.pending_interaction_id === null ||
    selectedDetail?.pending_interaction_id === undefined
      ? []
      : (mockUiState.selectedChoicesByInteractionId[
          selectedDetail.pending_interaction_id.value
        ] ?? []);

  const refreshSessions = async (
    session: PanelSessionListItem,
  ): Promise<void> => {
    const nextSessions = await fetchAllSessions(settingsView.settings);
    const nextDetail = await fetchSessionDetail(session);
    setSessions(nextSessions);
    setSelectedDetail(nextDetail);
  };

  const updateSettings = async (
    settings: BuilderPanelSettings,
  ): Promise<void> => {
    const saveVersion = settingsSaveVersion.current + 1;
    settingsSaveVersion.current = saveVersion;
    setSettingsView({
      settings,
      status_message: settingsView.status_message,
    });
    setSettingsSaving(true);
    try {
      const nextView = await savePanelSettings(settings);
      if (
        isLatestSettingsSaveResponse(saveVersion, settingsSaveVersion.current)
      ) {
        setSettingsView(nextView);
      }
    } catch (error: unknown) {
      if (
        isLatestSettingsSaveResponse(saveVersion, settingsSaveVersion.current)
      ) {
        setSettingsView({
          settings,
          status_message: readableError(error, "保存设置失败"),
        });
      }
    } finally {
      if (
        isLatestSettingsSaveResponse(saveVersion, settingsSaveVersion.current)
      ) {
        setSettingsSaving(false);
      }
    }
  };

  const persistPanelCollapsed = (collapsed: boolean): void => {
    setSettingsView((current) => ({
      ...current,
      settings: {
        ...current.settings,
        panel: {
          ...current.settings.panel,
          collapsed,
        },
      },
    }));
    void savePanelWindowState({ collapsed }).catch((error: unknown) => {
      setSettingsView((current) => ({
        ...current,
        status_message: readableError(error, "保存 panel 收缩状态失败"),
      }));
    });
  };

  const schedulePanelWindowStateSave = (
    update: PanelWindowStateUpdate,
  ): void => {
    if (panelGeometrySaveTimer.current !== null) {
      window.clearTimeout(panelGeometrySaveTimer.current);
    }
    pendingPanelWindowUpdate.current = mergePanelWindowStateUpdate(
      pendingPanelWindowUpdate.current,
      update,
    );
    panelGeometrySaveTimer.current = window.setTimeout(() => {
      const nextUpdate = pendingPanelWindowUpdate.current;
      pendingPanelWindowUpdate.current = {};
      void savePanelWindowState(nextUpdate).catch((error: unknown) => {
        setSettingsView((current) => ({
          ...current,
          status_message: readableError(error, "保存 panel 窗口状态失败"),
        }));
      });
    }, 400);
  };

  const toggleHookInstallAgentSelection = (agent: HookInstallAgent): void => {
    setHookInstallState((current) => ({
      ...current,
      selectedAgents: toggleHookInstallAgent(current.selectedAgents, agent),
      previewAgents: null,
      preview: null,
      statusMessage: null,
    }));
  };

  const runHookInstallPreview = async (): Promise<void> => {
    if (hookInstallState.selectedAgents.length === 0) {
      return;
    }

    setHookInstallState((current) => ({
      ...current,
      working: true,
      statusMessage: null,
    }));
    try {
      const preview = await previewHookInstall({
        agents: hookInstallState.selectedAgents,
      });
      setHookInstallState((current) => ({
        ...current,
        preview,
        previewAgents: [...hookInstallState.selectedAgents],
        working: false,
        statusMessage: "已生成 hook 安装预览",
      }));
    } catch (error: unknown) {
      setHookInstallState((current) => ({
        ...current,
        working: false,
        statusMessage: readableError(error, "hook 安装预览失败"),
      }));
    }
  };

  const runHookInstall = async (): Promise<void> => {
    if (!canInstallHooksAfterPreview(hookInstallState)) {
      setHookInstallState((current) => ({
        ...current,
        statusMessage: "请先预览当前 hook 安装目标",
      }));
      return;
    }

    setHookInstallState((current) => ({
      ...current,
      working: true,
      statusMessage: null,
    }));
    try {
      const manifest = await installHooks({
        agents: hookInstallState.selectedAgents,
      });
      setHookInstallState((current) => ({
        ...current,
        preview: null,
        previewAgents: null,
        working: false,
        statusMessage: `hook 已安装：${manifest.entries.length} 个目标`,
      }));
    } catch (error: unknown) {
      setHookInstallState((current) => ({
        ...current,
        working: false,
        statusMessage: readableError(error, "hook 安装失败"),
      }));
    }
  };

  const runHookUninstall = async (): Promise<void> => {
    setHookInstallState((current) => ({
      ...current,
      working: true,
      statusMessage: null,
    }));
    try {
      await uninstallHooks();
      setHookInstallState((current) => ({
        ...current,
        preview: null,
        previewAgents: null,
        working: false,
        statusMessage: "hook 已卸载",
      }));
    } catch (error: unknown) {
      setHookInstallState((current) => ({
        ...current,
        working: false,
        statusMessage: readableError(error, "hook 卸载失败"),
      }));
    }
  };

  const resolveApproval = async (
    decision: ApprovalDecision,
    injectFailure: boolean,
  ): Promise<void> => {
    if (
      selectedSession === null ||
      selectedDetail?.pending_interaction_id === null ||
      selectedDetail?.pending_interaction_id === undefined
    ) {
      return;
    }

    const interactionId = selectedDetail.pending_interaction_id.value;
    setMockUiState((current) => beginSubmit(current, interactionId));
    try {
      if (isCodexCliRuntime(selectedSession)) {
        await submitCodexCliApproval({
          session_key: selectedSession.session_key,
          interaction_id: selectedDetail.pending_interaction_id,
          decision,
        });
      } else {
        await submitMockApproval({
          session_key: selectedSession.session_key,
          interaction_id: selectedDetail.pending_interaction_id,
          decision,
          inject_failure: injectFailure,
        });
      }
      await refreshSessions(selectedSession);
      setMockUiState((current) => endSubmit(current, null));
    } catch (error: unknown) {
      setMockUiState((current) =>
        endSubmit(current, readableError(error, "审批提交失败")),
      );
    }
  };

  const sendReply = async (
    injectFailure: boolean,
    contentOverride: string | null = null,
  ): Promise<void> => {
    if (
      selectedSession === null ||
      selectedDetail?.pending_interaction_id === null ||
      selectedDetail?.pending_interaction_id === undefined
    ) {
      return;
    }

    const interactionId = selectedDetail.pending_interaction_id.value;
    const content = (contentOverride ?? selectedDraft).trim();
    if (isReplyDraftInvalid(content, 1000)) {
      return;
    }

    setMockUiState((current) => beginSubmit(current, interactionId));
    try {
      await submitMockReply({
        session_key: selectedSession.session_key,
        interaction_id: selectedDetail.pending_interaction_id,
        content,
        inject_failure: injectFailure,
      });
      await refreshSessions(selectedSession);
      setMockUiState((current) =>
        endSubmit(clearDraft(current, selectedSession.session_key), null),
      );
    } catch (error: unknown) {
      if (contentOverride !== null) {
        setMockUiState((current) =>
          updateDraft(current, selectedSession.session_key, contentOverride),
        );
      }
      setMockUiState((current) =>
        endSubmit(current, readableError(error, "回复发送失败")),
      );
    }
  };

  const submitChoice = async (injectFailure: boolean): Promise<void> => {
    if (
      selectedSession === null ||
      selectedDetail?.pending_interaction_id === null ||
      selectedDetail?.pending_interaction_id === undefined ||
      selectedChoiceValues.length === 0
    ) {
      return;
    }

    const interactionId = selectedDetail.pending_interaction_id.value;
    setMockUiState((current) => beginSubmit(current, interactionId));
    try {
      await submitMockChoice({
        session_key: selectedSession.session_key,
        interaction_id: selectedDetail.pending_interaction_id,
        selected_values: selectedChoiceValues,
        inject_failure: injectFailure,
      });
      await refreshSessions(selectedSession);
      setMockUiState((current) =>
        endSubmit(clearChoiceSelection(current, interactionId), null),
      );
    } catch (error: unknown) {
      setMockUiState((current) =>
        endSubmit(current, readableError(error, "选项提交失败")),
      );
    }
  };

  return (
    <main className={appSurfaceClassName(settingsView.settings)}>
      <PanelShell
        title="Builder Panel"
        collapsed={collapsed}
        onToggleCollapsed={() => {
          setPanelUiState((current) => {
            const next = togglePanelCollapsed(current);
            persistPanelCollapsed(next.collapsed);
            return next;
          });
        }}
      >
        <div className="panel-summary">
          <span>阶段 7 扩展模式</span>
          <strong>
            {panelProbe?.always_on_top === false ? "普通窗口" : "置顶窗口"}
          </strong>
        </div>
        <PanelStats sessions={sessions} />
        <div className="panel-tabs" aria-label="主视图">
          <button
            className={activeSection === "sessions" ? "tab-active" : ""}
            type="button"
            onClick={() => {
              setActiveSection("sessions");
            }}
          >
            Sessions
          </button>
          <button
            className={activeSection === "settings" ? "tab-active" : ""}
            type="button"
            onClick={() => {
              setActiveSection("settings");
            }}
          >
            Settings
          </button>
        </div>
        {mockUiState.errorMessage !== null && (
          <div className="panel-error" role="alert">
            {mockUiState.errorMessage}
          </div>
        )}
        {activeSection === "sessions" ? (
          <div className="mock-workspace">
            <SessionList
              sessions={sessions}
              selectedSessionId={mockUiState.selectedSessionId}
              showUsage={settingsView.settings.display.show_usage}
              onSelect={(session) => {
                setMockUiState((current) =>
                  selectPanelSession(current, session),
                );
              }}
            />
            <SessionDetail
              detail={selectedDetail}
              selectedSession={selectedSession}
              draft={selectedDraft}
              selectedChoiceValues={selectedChoiceValues}
              settings={settingsView.settings}
              submittingInteractionId={mockUiState.submittingInteractionId}
              onDraftChange={(draft) => {
                if (selectedSession !== null) {
                  setMockUiState((current) =>
                    updateDraft(current, selectedSession.session_key, draft),
                  );
                }
              }}
              onResolveApproval={(decision, injectFailure) => {
                void resolveApproval(decision, injectFailure);
              }}
              onSendReply={(injectFailure) => {
                void sendReply(injectFailure);
              }}
              onUseShortcutReply={(content, injectFailure) => {
                void sendReply(injectFailure, content);
              }}
              onToggleChoice={(choiceValue, allowsMultiple) => {
                const pendingInteractionId =
                  selectedDetail?.pending_interaction_id;
                if (pendingInteractionId != null) {
                  setMockUiState((current) =>
                    toggleChoiceSelection(
                      current,
                      pendingInteractionId.value,
                      choiceValue,
                      allowsMultiple,
                    ),
                  );
                }
              }}
              onSubmitChoice={(injectFailure) => {
                void submitChoice(injectFailure);
              }}
              onOpenTimeline={(sessionKey) => {
                setMockUiState((current) => openTimeline(current, sessionKey));
              }}
            />
          </div>
        ) : (
          <SettingsPanel
            hookInstall={hookInstallState}
            saving={settingsSaving}
            settings={settingsView.settings}
            statusMessage={settingsView.status_message}
            onChange={(settings) => {
              void updateSettings(settings);
            }}
            onInstallHooks={() => {
              void runHookInstall();
            }}
            onPreviewHookInstall={() => {
              void runHookInstallPreview();
            }}
            onToggleHookAgent={toggleHookInstallAgentSelection}
            onUninstallHooks={() => {
              void runHookUninstall();
            }}
          />
        )}
        {mockUiState.timeline.open && selectedSession !== null && (
          <TimelineOverlay
            state={mockUiState}
            selectedSession={selectedSession}
            onClose={() => {
              releaseTimelineCache(selectedSession);
              setMockUiState((current) => closeTimeline(current));
            }}
            onSearch={(search) => {
              setMockUiState((current) =>
                updateTimelineSearch(current, search),
              );
            }}
            onKind={(kind) => {
              setMockUiState((current) => updateTimelineKind(current, kind));
            }}
          />
        )}
      </PanelShell>
    </main>
  );
};

/// 生成应用根节点样式类。
const appSurfaceClassName = (settings: BuilderPanelSettings): string => {
  const classNames = [
    "app-surface",
    `app-theme-${settings.display.theme}`,
    settings.display.density === "compact" ? "app-surface-compact" : null,
  ];

  return classNames.filter((className) => className !== null).join(" ");
};

/// 读取所有当前可展示 session。
const fetchAllSessions = async (
  settings: BuilderPanelSettings,
): Promise<readonly PanelSessionListItem[]> => {
  const [mockSessions, codexSessions] = await Promise.all([
    settings.agents.mock_agent_enabled
      ? fetchMockSessions()
      : Promise.resolve([]),
    settings.agents.codex_cli_enabled
      ? fetchCodexCliSessions()
      : Promise.resolve([]),
  ]);

  return sortPanelSessions([
    ...codexSessions.map((session) => withRuntimeSource(session, "codex_cli")),
    ...mockSessions.map((session) => withRuntimeSource(session, "mock")),
  ]);
};

/// 排序合并后的 session 列表。
export const sortPanelSessions = (
  sessions: readonly PanelSessionListItem[],
): readonly PanelSessionListItem[] => {
  return [...sessions].sort((left, right) => {
    const leftStatusOrder = SESSION_STATUS_ORDER[left.status_kind];
    const rightStatusOrder = SESSION_STATUS_ORDER[right.status_kind];
    if (leftStatusOrder !== rightStatusOrder) {
      return leftStatusOrder - rightStatusOrder;
    }

    const leftUpdatedAt = Number(left.updated_at_label);
    const rightUpdatedAt = Number(right.updated_at_label);
    if (Number.isFinite(leftUpdatedAt) && Number.isFinite(rightUpdatedAt)) {
      return rightUpdatedAt - leftUpdatedAt;
    }

    return panelSessionToId(left).localeCompare(panelSessionToId(right));
  });
};

/// 统计等待和运行中的 session 数量。
export const countSessionsByStatus = (
  sessions: readonly PanelSessionListItem[],
): { readonly waiting: number; readonly running: number } => {
  return sessions.reduce(
    (counts, session) => {
      if (
        session.status_kind === "waiting_for_approval" ||
        session.status_kind === "waiting_for_answer"
      ) {
        return {
          waiting: counts.waiting + 1,
          running: counts.running,
        };
      }
      if (session.status_kind === "running") {
        return {
          waiting: counts.waiting,
          running: counts.running + 1,
        };
      }

      return counts;
    },
    { waiting: 0, running: 0 },
  );
};

/// 判断设置保存响应是否仍是最新请求。
export const isLatestSettingsSaveResponse = (
  responseVersion: number,
  currentVersion: number,
): boolean => {
  return responseVersion === currentVersion;
};

/// 切换 hook 安装目标选择。
export const toggleHookInstallAgent = (
  agents: readonly HookInstallAgent[],
  agent: HookInstallAgent,
): readonly HookInstallAgent[] => {
  if (agents.includes(agent)) {
    return agents.filter((item) => item !== agent);
  }

  return [...agents, agent];
};

/// 判断 hook 安装是否已有当前选择对应的预览。
export const canInstallHooksAfterPreview = (
  state: Pick<
    HookInstallUiState,
    "preview" | "previewAgents" | "selectedAgents"
  >,
): boolean => {
  if (state.preview === null || state.previewAgents === null) {
    return false;
  }

  return sameHookInstallAgents(state.previewAgents, state.selectedAgents);
};

/// 合并 panel 窗口状态局部更新。
export const mergePanelWindowStateUpdate = (
  current: PanelWindowStateUpdate,
  next: PanelWindowStateUpdate,
): PanelWindowStateUpdate => {
  return {
    ...current,
    ...next,
  };
};

/// 判断两个 hook agent 集合是否一致。
const sameHookInstallAgents = (
  left: readonly HookInstallAgent[],
  right: readonly HookInstallAgent[],
): boolean => {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((agent) => right.includes(agent));
};

/// 首次或空选中时自动选择第一条 session。
export const selectFirstSessionWhenMissing = (
  current: MockPanelUiState,
  items: readonly PanelSessionListItem[],
): MockPanelUiState => {
  if (items.length === 0) {
    return current;
  }

  const selectedStillExists =
    current.selectedSessionId !== null &&
    items.some((item) => panelSessionToId(item) === current.selectedSessionId);
  if (selectedStillExists) {
    return current;
  }

  return selectPanelSession(current, items[0]);
};

/// 生成包含运行时来源的前端 session ID。
export const panelSessionToId = (session: PanelSessionListItem): string => {
  return [session.runtimeSource, sessionKeyToId(session.session_key)].join(
    "::",
  );
};

/// 选择合并后的前端 session。
export const selectPanelSession = (
  current: MockPanelUiState,
  session: PanelSessionListItem,
): MockPanelUiState => ({
  ...selectSession(current, session.session_key),
  selectedSessionId: panelSessionToId(session),
});

/// 为列表项附加显式运行时来源。
const withRuntimeSource = (
  session: SessionListItemViewModel,
  runtimeSource: RuntimeSource,
): PanelSessionListItem => ({
  ...session,
  runtimeSource,
});

/// 面板统计属性。
interface PanelStatsProps {
  /// 当前 session 列表。
  readonly sessions: readonly PanelSessionListItem[];
}

/// 面板顶部统计。
const PanelStats = ({ sessions }: PanelStatsProps) => {
  const counts = countSessionsByStatus(sessions);

  return (
    <div className="panel-stats" aria-label="session 统计">
      <span>
        等待 <strong>{counts.waiting}</strong>
      </span>
      <span>
        运行 <strong>{counts.running}</strong>
      </span>
      <span>
        总计 <strong>{sessions.length}</strong>
      </span>
    </div>
  );
};

/// 判断 session 是否来自真实 Codex CLI runtime。
export const isCodexCliRuntime = (session: PanelSessionListItem): boolean => {
  return session.runtimeSource === "codex_cli";
};

/// 判断是否展示 timeline 入口。
export const canShowTimelineEntry = (actions: readonly UiAction[]): boolean => {
  return actions.includes("view_process_timeline");
};

/// 返回 UI 动作标签。
export const actionLabel = (action: UiAction): string => {
  switch (action) {
    case "jump":
      return "跳回";
    case "send_reply":
      return "回复";
    case "resolve_approval":
      return "审批";
    case "create_followup_turn":
      return "新对话";
    case "view_process_timeline":
      return "过程";
  }
};

/// 按运行时来源读取 session 详情。
const fetchSessionDetail = async (
  session: PanelSessionListItem,
): Promise<SessionDetailViewModel | null> => {
  if (isCodexCliRuntime(session)) {
    return fetchCodexCliSessionDetail(session.session_key);
  }

  return fetchMockSessionDetail(session.session_key);
};

/// 按运行时来源读取 timeline。
const fetchTimelinePage = async (
  session: PanelSessionListItem,
  query: TimelineQuery,
): Promise<TimelinePage> => {
  if (isCodexCliRuntime(session)) {
    return fetchCodexCliTimeline(query);
  }

  return fetchMockTimeline(query);
};

/// 按运行时来源释放 timeline 大文本缓存。
const releaseTimelineCache = (session: PanelSessionListItem): void => {
  const releaseTask = isCodexCliRuntime(session)
    ? releaseCodexCliTimelineCache(session.session_key)
    : releaseMockTimelineCache(session.session_key);

  releaseTask.catch(() => {
    // 关闭弹层不能被释放失败阻塞；下次查询仍可重新读取可用缓存。
  });
};

/// Session 列表属性。
interface SessionListProps {
  /// 会话列表。
  readonly sessions: readonly PanelSessionListItem[];
  /// 当前选中 session ID。
  readonly selectedSessionId: string | null;
  /// 是否展示用量。
  readonly showUsage: boolean;
  /// 选择回调。
  readonly onSelect: (session: PanelSessionListItem) => void;
}

/// Session 列表。
const SessionList = ({
  sessions,
  selectedSessionId,
  showUsage,
  onSelect,
}: SessionListProps) => (
  <div className="session-list" aria-label="agent 会话列表">
    {sessions.length === 0 && (
      <div className="session-list-empty">暂无可展示 session</div>
    )}
    {sessions.map((session) => {
      const sessionId = panelSessionToId(session);
      const selected = selectedSessionId === sessionId;

      return (
        <button
          className={
            selected ? "session-row session-row-selected" : "session-row"
          }
          key={sessionId}
          type="button"
          onClick={() => {
            onSelect(session);
          }}
        >
          <div>
            <strong>{session.project_label}</strong>
            <p>{session.summary.text}</p>
            <small>
              {session.agent_label} / {session.conversation_label}
            </small>
            <small className="session-actions">
              {session.actions.map(actionLabel).join(" / ") || "无可执行动作"}
            </small>
          </div>
          <span>{session.status_label}</span>
          {showUsage && <em>{session.usage_5h.value_label}</em>}
        </button>
      );
    })}
  </div>
);

/// Session 详情属性。
interface SessionDetailProps {
  /// Session 详情。
  readonly detail: SessionDetailViewModel | null;
  /// 当前选中 session。
  readonly selectedSession: PanelSessionListItem | null;
  /// 当前 session 回复草稿。
  readonly draft: string;
  /// 当前交互已选选项。
  readonly selectedChoiceValues: readonly string[];
  /// 当前设置。
  readonly settings: BuilderPanelSettings;
  /// 当前提交中的交互 ID。
  readonly submittingInteractionId: string | null;
  /// 草稿更新回调。
  readonly onDraftChange: (draft: string) => void;
  /// 审批回调。
  readonly onResolveApproval: (
    decision: ApprovalDecision,
    injectFailure: boolean,
  ) => void;
  /// 回复回调。
  readonly onSendReply: (injectFailure: boolean) => void;
  /// 快捷回复回调。
  readonly onUseShortcutReply: (
    content: string,
    injectFailure: boolean,
  ) => void;
  /// 切换选项回调。
  readonly onToggleChoice: (
    choiceValue: string,
    allowsMultiple: boolean,
  ) => void;
  /// 提交选项回调。
  readonly onSubmitChoice: (injectFailure: boolean) => void;
  /// 打开时间线回调。
  readonly onOpenTimeline: (sessionKey: SessionKey) => void;
}

/// Session 详情。
const SessionDetail = ({
  detail,
  selectedSession,
  draft,
  selectedChoiceValues,
  settings,
  submittingInteractionId,
  onDraftChange,
  onResolveApproval,
  onSendReply,
  onUseShortcutReply,
  onToggleChoice,
  onSubmitChoice,
  onOpenTimeline,
}: SessionDetailProps) => {
  const [detailOverlayOpen, setDetailOverlayOpen] = useState(false);
  const [replyComposerOpen, setReplyComposerOpen] = useState(false);

  if (selectedSession === null || detail === null) {
    return <section className="session-detail-empty">暂无 session</section>;
  }

  const isMockSession = selectedSession.runtimeSource === "mock";
  const pendingId = detail.pending_interaction_id?.value ?? null;
  const submitting =
    pendingId !== null && submittingInteractionId === pendingId;
  const canResolveApproval =
    detail.toolbar_actions.includes("resolve_approval") &&
    detail.pending_interaction_kind === "approval";
  const canSendReply =
    detail.toolbar_actions.includes("send_reply") &&
    detail.pending_interaction_kind === "text_reply";
  const canSubmitChoice =
    detail.toolbar_actions.includes("send_reply") &&
    detail.pending_interaction_kind === "choice" &&
    detail.choice_box.enabled;
  const canOpenTimeline = canShowTimelineEntry(detail.toolbar_actions);
  const replyCharCount = countReplyChars(draft);
  const replyInvalid = isReplyDraftInvalid(draft, 1000);

  return (
    <>
      <section className="session-detail" aria-label="session 详情">
        <header>
          <div>
            <strong>{detail.header}</strong>
            <p>{detail.identity}</p>
          </div>
          <span>{detail.execution_info}</span>
        </header>
        <div className="detail-summary-line">
          <p className="detail-summary">{detail.summary.text}</p>
          <button
            type="button"
            onClick={() => {
              setDetailOverlayOpen(true);
            }}
          >
            详情
          </button>
        </div>
        {settings.display.show_usage && (
          <div className="detail-metrics">
            <span>{detail.usage}</span>
            <span>{selectedSession.usage_5h.value_label}</span>
          </div>
        )}
        {detail.pending_interaction !== null && (
          <div className="pending-box">
            <strong>{detail.pending_interaction}</strong>
            {canResolveApproval && (
              <div className="button-row">
                <button
                  type="button"
                  disabled={submitting}
                  onClick={() => {
                    onResolveApproval("allow", false);
                  }}
                >
                  允许
                </button>
                <button
                  type="button"
                  disabled={submitting}
                  onClick={() => {
                    onResolveApproval("deny", false);
                  }}
                >
                  拒绝
                </button>
                <button
                  type="button"
                  disabled={submitting}
                  onClick={() => {
                    onResolveApproval("allow_and_remember", false);
                  }}
                >
                  允许并记住
                </button>
                {isMockSession && (
                  <button
                    type="button"
                    disabled={submitting}
                    onClick={() => {
                      onResolveApproval("allow", true);
                    }}
                  >
                    失败演练
                  </button>
                )}
              </div>
            )}
            {canSendReply && (
              <div className="reply-inline">
                <div className="shortcut-row" aria-label="快捷回复">
                  {settings.replies.shortcut_replies_enabled &&
                    DEFAULT_SHORTCUT_REPLIES.map((shortcut) => (
                      <button
                        key={shortcut.id}
                        type="button"
                        disabled={submitting}
                        onClick={() => {
                          onUseShortcutReply(shortcut.content, false);
                        }}
                      >
                        {shortcut.label}
                      </button>
                    ))}
                  <button
                    type="button"
                    disabled={submitting}
                    onClick={() => {
                      setReplyComposerOpen(true);
                    }}
                  >
                    打开回复
                  </button>
                  {isMockSession &&
                    settings.replies.shortcut_replies_enabled && (
                      <button
                        type="button"
                        disabled={submitting}
                        onClick={() => {
                          onUseShortcutReply(
                            DEFAULT_SHORTCUT_REPLIES[0].content,
                            true,
                          );
                        }}
                      >
                        快捷失败演练
                      </button>
                    )}
                </div>
                <span className={replyInvalid ? "reply-count-invalid" : ""}>
                  {replyCharCount}/1000
                </span>
              </div>
            )}
            {canSubmitChoice && (
              <div className="choice-box">
                <div className="choice-list">
                  {detail.choice_box.choices.map((choice) => {
                    const checked = selectedChoiceValues.includes(choice.value);
                    return (
                      <label className="choice-row" key={choice.value}>
                        <input
                          checked={checked}
                          name={pendingId ?? "choice"}
                          type={
                            detail.choice_box.allows_multiple
                              ? "checkbox"
                              : "radio"
                          }
                          value={choice.value}
                          onChange={() => {
                            onToggleChoice(
                              choice.value,
                              detail.choice_box.allows_multiple,
                            );
                          }}
                        />
                        <span>{choice.label}</span>
                      </label>
                    );
                  })}
                </div>
                <div className="button-row">
                  <button
                    type="button"
                    disabled={selectedChoiceValues.length === 0 || submitting}
                    onClick={() => {
                      onSubmitChoice(false);
                    }}
                  >
                    提交选择
                  </button>
                  {isMockSession && (
                    <button
                      type="button"
                      disabled={selectedChoiceValues.length === 0 || submitting}
                      onClick={() => {
                        onSubmitChoice(true);
                      }}
                    >
                      失败演练
                    </button>
                  )}
                </div>
              </div>
            )}
          </div>
        )}
        {canOpenTimeline && (
          <footer>
            <button
              type="button"
              onClick={() => {
                onOpenTimeline(selectedSession.session_key);
              }}
            >
              Timeline
            </button>
          </footer>
        )}
      </section>
      {detailOverlayOpen && (
        <SessionDetailOverlay
          detail={detail}
          selectedSession={selectedSession}
          onClose={() => {
            setDetailOverlayOpen(false);
          }}
        />
      )}
      {replyComposerOpen && canSendReply && (
        <ReplyComposerOverlay
          draft={draft}
          isMockSession={isMockSession}
          replyInvalid={replyInvalid}
          settings={settings}
          submitting={submitting}
          onClose={() => {
            setReplyComposerOpen(false);
          }}
          onDraftChange={onDraftChange}
          onSendReply={onSendReply}
        />
      )}
    </>
  );
};

/// Session 详情弹层属性。
interface SessionDetailOverlayProps {
  /// Session 详情。
  readonly detail: SessionDetailViewModel;
  /// 当前选中 session。
  readonly selectedSession: PanelSessionListItem;
  /// 关闭回调。
  readonly onClose: () => void;
}

/// Session 详情弹层。
const SessionDetailOverlay = ({
  detail,
  selectedSession,
  onClose,
}: SessionDetailOverlayProps) => (
  <div className="overlay-backdrop" role="presentation">
    <section className="overlay-panel session-detail-overlay" aria-label="详情">
      <header>
        <div>
          <strong>{detail.header}</strong>
          <p>
            {selectedSession.agent_label} / {selectedSession.conversation_label}
          </p>
        </div>
        <button type="button" onClick={onClose}>
          关闭
        </button>
      </header>
      <dl>
        <div>
          <dt>身份</dt>
          <dd>{detail.identity}</dd>
        </div>
        <div>
          <dt>执行</dt>
          <dd>{detail.execution_info}</dd>
        </div>
        <div>
          <dt>摘要</dt>
          <dd>{detail.summary.text}</dd>
        </div>
      </dl>
    </section>
  </div>
);

/// 回复输入弹层属性。
interface ReplyComposerOverlayProps {
  /// 当前草稿。
  readonly draft: string;
  /// 是否为 mock session。
  readonly isMockSession: boolean;
  /// 当前草稿是否非法。
  readonly replyInvalid: boolean;
  /// 当前设置。
  readonly settings: BuilderPanelSettings;
  /// 是否正在提交。
  readonly submitting: boolean;
  /// 关闭回调。
  readonly onClose: () => void;
  /// 草稿更新回调。
  readonly onDraftChange: (draft: string) => void;
  /// 回复发送回调。
  readonly onSendReply: (injectFailure: boolean) => void;
}

/// 回复输入弹层。
const ReplyComposerOverlay = ({
  draft,
  isMockSession,
  replyInvalid,
  settings,
  submitting,
  onClose,
  onDraftChange,
  onSendReply,
}: ReplyComposerOverlayProps) => (
  <div className="overlay-backdrop" role="presentation">
    <section className="overlay-panel reply-composer" aria-label="回复输入">
      <header>
        <div>
          <strong>Reply</strong>
          <p>{settings.replies.enter_to_send ? "Enter 发送" : "手动发送"}</p>
        </div>
        <button type="button" onClick={onClose}>
          关闭
        </button>
      </header>
      <textarea
        value={draft}
        autoFocus={true}
        placeholder={
          settings.replies.enter_to_send
            ? "输入回复，Enter 发送，Shift+Enter 换行"
            : "输入回复"
        }
        onChange={(event) => {
          onDraftChange(event.target.value);
        }}
        onKeyDown={(event) => {
          if (
            !settings.replies.enter_to_send ||
            event.key !== "Enter" ||
            event.shiftKey
          ) {
            return;
          }
          event.preventDefault();
          if (!replyInvalid && !submitting) {
            onSendReply(false);
          }
        }}
      />
      <div className="button-row">
        <span className={replyInvalid ? "reply-count-invalid" : ""}>
          {countReplyChars(draft)}/1000
        </span>
        <button
          type="button"
          disabled={replyInvalid || submitting}
          onClick={() => {
            onSendReply(false);
          }}
        >
          发送
        </button>
        {isMockSession && (
          <button
            type="button"
            disabled={replyInvalid || submitting}
            onClick={() => {
              onSendReply(true);
            }}
          >
            失败演练
          </button>
        )}
      </div>
    </section>
  </div>
);

/// Timeline 弹层属性。
interface TimelineOverlayProps {
  /// Mock panel UI 状态。
  readonly state: MockPanelUiState;
  /// 当前选中 session。
  readonly selectedSession: PanelSessionListItem;
  /// 关闭回调。
  readonly onClose: () => void;
  /// 搜索回调。
  readonly onSearch: (search: string) => void;
  /// 类型筛选回调。
  readonly onKind: (kind: TimelineKindFilter) => void;
}

/// Timeline 弹层。
const TimelineOverlay = ({
  state,
  selectedSession,
  onClose,
  onSearch,
  onKind,
}: TimelineOverlayProps) => {
  const listRef = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const items = state.timeline.page?.items ?? [];
  const range = timelineVisibleRange(
    items.length,
    scrollTop,
    TIMELINE_VIEWPORT_HEIGHT,
    TIMELINE_ROW_HEIGHT,
    TIMELINE_OVERSCAN,
  );
  const visibleItems = items.slice(range.start, range.end);
  const topSpacerHeight = range.start * TIMELINE_ROW_HEIGHT;
  const bottomSpacerHeight = (items.length - range.end) * TIMELINE_ROW_HEIGHT;
  const canCopyFiltered =
    state.timeline.page !== null && state.timeline.page.items.length > 0;

  return (
    <div className="overlay-backdrop" role="presentation">
      <section
        className="overlay-panel timeline-overlay"
        aria-label="过程时间线"
      >
        <header>
          <div>
            <strong>Timeline</strong>
            <p>
              {selectedSession.project_label} /{" "}
              {selectedSession.conversation_label}
            </p>
          </div>
          <button type="button" onClick={onClose}>
            关闭
          </button>
        </header>
        <div className="timeline-filters">
          <input
            value={state.timeline.search}
            placeholder="搜索"
            onChange={(event) => {
              onSearch(event.target.value);
            }}
          />
          <select
            value={state.timeline.kind}
            onChange={(event) => {
              onKind(event.target.value as TimelineKindFilter);
            }}
          >
            <option value="all">全部</option>
            <option value="activity">活动</option>
            <option value="tool">工具</option>
            <option value="approval">审批</option>
            <option value="reply">回复</option>
            <option value="system">系统</option>
          </select>
        </div>
        <div className="timeline-actions">
          <span>
            {state.timeline.page === null
              ? "0 条"
              : `${state.timeline.page.total} 条`}
          </span>
          <button
            type="button"
            disabled={!canCopyFiltered}
            onClick={() => {
              if (state.timeline.page !== null) {
                void copyText(timelinePageToCopyText(state.timeline.page));
              }
            }}
          >
            复制筛选结果
          </button>
          <button
            type="button"
            disabled={items.length === 0}
            onClick={() => {
              const list = listRef.current;
              if (list !== null) {
                list.scrollTop = list.scrollHeight;
              }
            }}
          >
            跳到最新
          </button>
        </div>
        {state.timeline.loading && <p className="timeline-empty">读取中</p>}
        {state.timeline.errorMessage !== null && (
          <p className="timeline-empty">{state.timeline.errorMessage}</p>
        )}
        <div
          className="timeline-list"
          ref={listRef}
          onScroll={(event) => {
            setScrollTop(event.currentTarget.scrollTop);
          }}
        >
          <div style={{ height: topSpacerHeight }} />
          {visibleItems.map((item) => (
            <TimelineRow item={item} key={item.item_id} />
          ))}
          <div style={{ height: bottomSpacerHeight }} />
          {state.timeline.page !== null &&
            state.timeline.page.items.length === 0 &&
            !state.timeline.loading && (
              <p className="timeline-empty">没有匹配的过程事件</p>
            )}
        </div>
      </section>
    </div>
  );
};

/// Timeline 行属性。
interface TimelineRowProps {
  /// Timeline 条目。
  readonly item: ProcessTimelineItem;
}

/// Timeline 行。
const TimelineRow = ({ item }: TimelineRowProps) => (
  <article className="timeline-row">
    <div>
      <strong>{item.title}</strong>
      <p>{item.body}</p>
    </div>
    <button
      type="button"
      onClick={() => {
        void copyTimelineItem(item);
      }}
    >
      复制
    </button>
  </article>
);

/// 复制单条 timeline 文本。
const copyTimelineItem = async (item: ProcessTimelineItem): Promise<void> => {
  await copyText(`${item.title}
${item.body}`);
};

/// 复制纯文本。
const copyText = async (text: string): Promise<void> => {
  const clipboard = navigator.clipboard;
  if (clipboard === undefined) {
    return;
  }

  await clipboard.writeText(text);
};

/// 归一错误消息。
const readableError = (error: unknown, fallback: string): string => {
  if (error instanceof Error && error.message.length > 0) {
    return error.message;
  }
  if (typeof error === "string" && error.length > 0) {
    return error;
  }

  return fallback;
};
