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
): Promise<unknown> {
  const isDefaultProj = resolvedProjectId === DEFAULT_FLOW_PROJECT_ID;
  const genId = () =>
    typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
      ? crypto.randomUUID()
      : `node-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;

  const nodeIdMap = new Map<string, string>();
  for (const n of DEFAULT_INITIAL_NODES) {
    const actualNodeId = isDefaultProj ? n.id : genId();
    nodeIdMap.set(n.id, actualNodeId);
    store.addNode({
      id: actualNodeId,
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
    const actualEdgeId = isDefaultProj ? t.id : genId();
    const sourceNodeId = nodeIdMap.get(t.sourceNodeId) || t.sourceNodeId;
    const targetNodeId = nodeIdMap.get(t.targetNodeId) || t.targetNodeId;
    store.addTransition({
      id: actualEdgeId,
      sourceNodeId,
      targetNodeId,
      triggerEvent: t.triggerEvent,
      edgeType: t.edgeType,
      conditionLabel: t.conditionLabel,
    });
  }
  store.selectTransition(null);
  store.selectNode(null);
  return store.syncDeployDraft(resolvedProjectId);
}

const inFlightSnapshotRequests = new Map<string, ReturnType<typeof apiFlowWorkbench.getSnapshot>>();

function fetchSnapshotSingleflight(projectId: string) {
  const existing = inFlightSnapshotRequests.get(projectId);
  if (existing) return existing;
  const request = apiFlowWorkbench.getSnapshot(projectId).finally(() => {
    inFlightSnapshotRequests.delete(projectId);
  });
  inFlightSnapshotRequests.set(projectId, request);
  return request;
}

function useRemoteProjectHydration(
  resolvedProjectId: string,
  store: ReturnType<typeof createNewFlowWorkbenchStore>
): boolean {
  const hydratedProjectRef = useRef<string | null>(null);
  const [isFlowHydrated, setIsFlowHydrated] = useState(false);

  useEffect(() => {
    if (typeof window === "undefined" || typeof fetch !== "function") {
      setIsFlowHydrated(true);
      return;
    }
    if (hydratedProjectRef.current === resolvedProjectId) {
      setIsFlowHydrated(true);
      return;
    }

    let cancelled = false;
    setIsFlowHydrated(false);
    void fetchSnapshotSingleflight(resolvedProjectId)
      .then(async (remoteSnap) => {
        if (cancelled) return;
        hydratedProjectRef.current = resolvedProjectId;
        store.setNetworkSyncEnabled(true, resolvedProjectId);
        if (remoteSnap.nodes.length > 0 || remoteSnap.transitions.length > 0 || remoteSnap.graphVersion > 1) {
          store.hydrateFromSnapshot(remoteSnap, true);
          if (!cancelled) setIsFlowHydrated(true);
          return;
        }
        if (store.getState().nodes.length > 0) {
          store.hydrateFromSnapshot(remoteSnap, false);
          await seedVirginDefaultGraph(resolvedProjectId, store);
        }
        if (!cancelled) setIsFlowHydrated(true);
      })
      .catch(() => {
        // Keep hydratedProjectRef unset when cancelled; mark attempted on non-cancelled offline fallback
        if (!cancelled) {
          hydratedProjectRef.current = resolvedProjectId;
          setIsFlowHydrated(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [resolvedProjectId, store]);

  return isFlowHydrated;
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

  const isFlowHydrated = useRemoteProjectHydration(sessionManager.activeProjectId || initialProjectId, store);

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
    toggleChat: splitter.handleToggleShortenChat, activeTabId, setActiveTabId, isFlowHydrated,
  };
}
