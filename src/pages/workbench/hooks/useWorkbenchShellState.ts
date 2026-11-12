// @story 04.1
import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { apiFlowWorkbench } from "../../../api/flow-workbench";
import {
  createNewFlowWorkbenchStore,
  FlowWorkbenchState,
  WorkbenchNode,
  WorkbenchTransition,
} from "../../../features/flow/store/flow-workbench-store";
import { calculateGraphMetrics } from "../../../features/flow/utils/graph-analysis";
import {
  DEFAULT_FLOW_PROJECT_ID,
  DEFAULT_INITIAL_NODES,
  DEFAULT_INITIAL_TRANSITIONS,
} from "../constants";
import { useWorkbenchAttachments } from "./useWorkbenchAttachments";
import { useWorkbenchGraphHandlers } from "./useWorkbenchGraphHandlers";
import { useWorkbenchProjectsAndSessions, useWorkbenchSettings } from "./useWorkbenchSessionSettings";
import { useWorkbenchSplitter } from "./useWorkbenchSplitter";

export { useWorkbenchProjectsAndSessions, useWorkbenchSettings };

function seedVirginDefaultGraph(
  resolvedProjectId: string,
  store: ReturnType<typeof createNewFlowWorkbenchStore>
): void {
  for (const n of DEFAULT_INITIAL_NODES) {
    store.addNode({
      id: n.id,
      screenKey: n.screenKey,
      title: n.title,
      routePath: n.routePath,
      posX: n.posX,
      posY: n.posY,
      clusterId: n.clusterId,
      note: n.note,
    });
  }
  for (const t of DEFAULT_INITIAL_TRANSITIONS) {
    store.addTransition({
      id: t.id,
      sourceNodeId: t.sourceNodeId,
      targetNodeId: t.targetNodeId,
      triggerEvent: t.triggerEvent,
      edgeType: t.edgeType,
      conditionLabel: t.conditionLabel,
    });
  }
  store.selectTransition(null);
  store.selectNode(null);
  void store.syncDeployDraft(resolvedProjectId);
}

function useRemoteProjectHydration(
  resolvedProjectId: string,
  store: ReturnType<typeof createNewFlowWorkbenchStore>
): void {
  const hydratedProjectRef = useRef<string | null>(null);
  useEffect(() => {
    if (typeof window === "undefined" || typeof fetch !== "function") return;
    if (hydratedProjectRef.current === resolvedProjectId) return;

    let cancelled = false;
    void apiFlowWorkbench
      .getSnapshot(resolvedProjectId)
      .then((remoteSnap) => {
        if (cancelled) return;
        hydratedProjectRef.current = resolvedProjectId;
        store.setNetworkSyncEnabled(true, resolvedProjectId);
        if (remoteSnap.nodes.length > 0 || remoteSnap.transitions.length > 0 || remoteSnap.graphVersion > 1) {
          store.hydrateFromSnapshot(remoteSnap, true);
          return;
        }
        if (store.getState().nodes.length > 0) {
          store.hydrateFromSnapshot(remoteSnap, false);
          seedVirginDefaultGraph(resolvedProjectId, store);
        }
      })
      .catch(() => {
        // Keep hydratedProjectRef unset when cancelled; mark attempted on non-cancelled offline fallback
        if (!cancelled) hydratedProjectRef.current = resolvedProjectId;
      });
    return () => {
      cancelled = true;
    };
  }, [resolvedProjectId, store]);
}

function useResponsiveSidebarCollapse(): [boolean, () => void] {
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(false);
  useEffect(() => {
    if (typeof window === "undefined") return;
    const handleResize = () => {
      if (window.innerWidth < 1024) setIsSidebarCollapsed(true);
    };
    handleResize();
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);
  const toggleSidebar = useCallback(() => setIsSidebarCollapsed((prev) => !prev), []);
  return [isSidebarCollapsed, toggleSidebar];
}

export function useWorkbenchShellState(
  initialNodes: WorkbenchNode[] = DEFAULT_INITIAL_NODES,
  initialTransitions: WorkbenchTransition[] = DEFAULT_INITIAL_TRANSITIONS,
  initialProjectId: string = DEFAULT_FLOW_PROJECT_ID
) {
  const store = useMemo(
    () => createNewFlowWorkbenchStore(initialNodes, initialTransitions, { projectId: initialProjectId, enableNetworkLayoutSync: false }),
    [initialNodes, initialTransitions, initialProjectId]
  );
  const state: FlowWorkbenchState = useSyncExternalStore(store.subscribe, store.getState);
  const settings = useWorkbenchSettings();
  const sessionManager = useWorkbenchProjectsAndSessions();
  const splitter = useWorkbenchSplitter();
  const attachments = useWorkbenchAttachments();
  const graphHandlers = useWorkbenchGraphHandlers(store, state.nodes.length);
  const [isSidebarCollapsed, toggleSidebar] = useResponsiveSidebarCollapse();
  const [activeTabId, setActiveTabId] = useState("tab-userflow");

  useRemoteProjectHydration(sessionManager.activeProjectId || initialProjectId, store);

  const metrics = useMemo(
    () => calculateGraphMetrics(state.nodes.map((n) => ({ id: n.id })), state.transitions.map((t) => ({ source: t.sourceNodeId, target: t.targetNodeId }))),
    [state.nodes, state.transitions]
  );
  const activeSession = useMemo(
    () => sessionManager.sessions.find((s) => s.id === sessionManager.activeSessionId),
    [sessionManager.sessions, sessionManager.activeSessionId]
  );

  return {
    store, state, settings, sessionManager, splitter, attachments, graphHandlers, metrics,
    activeSession, isSidebarCollapsed, toggleSidebar, isChatCollapsed: splitter.isChatShortened,
    toggleChat: splitter.handleToggleShortenChat, activeTabId, setActiveTabId,
  };
}
