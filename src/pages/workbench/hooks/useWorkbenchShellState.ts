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

async function applyRemoteSnapshotOrSeed(
  resolvedProjectId: string,
  store: ReturnType<typeof createNewFlowWorkbenchStore>,
  remoteSnap: Awaited<ReturnType<typeof fetchSnapshotSingleflight>>
): Promise<void> {
  if (remoteSnap.nodes.length > 0 || remoteSnap.transitions.length > 0 || remoteSnap.graphVersion > 1) {
    store.hydrateFromSnapshot(remoteSnap, true);
    return;
  }
  if (store.getState().nodes.length > 0 && resolvedProjectId === DEFAULT_FLOW_PROJECT_ID) {
    store.hydrateFromSnapshot(remoteSnap, false);
    await seedVirginDefaultGraph(resolvedProjectId, store);
  } else {
    store.hydrateFromSnapshot(remoteSnap, true);
  }
}

function useProjectStateInvalidationTick(resolvedProjectId: string, hydratedProjectRef: React.MutableRefObject<string | null>): number {
  const [invalidateTick, setInvalidateTick] = useState(0);
  useEffect(() => {
    if (typeof window === "undefined") return;
    const onInvalidated = (ev: Event) => {
      const detail = (ev as CustomEvent<{ projectId?: string }>).detail;
      if (detail?.projectId && detail.projectId !== resolvedProjectId) return;
      hydratedProjectRef.current = null;
      setInvalidateTick((n) => n + 1);
    };
    window.addEventListener("cowok:project-state-invalidated", onInvalidated);
    return () => window.removeEventListener("cowok:project-state-invalidated", onInvalidated);
  }, [hydratedProjectRef, resolvedProjectId]);
  return invalidateTick;
}

function useRemoteProjectHydration(
  resolvedProjectId: string,
  store: ReturnType<typeof createNewFlowWorkbenchStore>
): boolean {
  const hydratedProjectRef = useRef<string | null>(null);
  const [isFlowHydrated, setIsFlowHydrated] = useState(false);
  const invalidateTick = useProjectStateInvalidationTick(resolvedProjectId, hydratedProjectRef);

  useEffect(() => {
    if (typeof window === "undefined" || typeof fetch !== "function") {
      setIsFlowHydrated(true);
      return;
    }
    if (hydratedProjectRef.current === resolvedProjectId && invalidateTick === 0) {
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
        await applyRemoteSnapshotOrSeed(resolvedProjectId, store, remoteSnap);
        if (!cancelled) setIsFlowHydrated(true);
      })
      .catch(() => {
        if (!cancelled) {
          hydratedProjectRef.current = resolvedProjectId;
          setIsFlowHydrated(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [invalidateTick, resolvedProjectId, store]);

  return isFlowHydrated;
}

function useResponsiveSidebarCollapse(): [boolean, () => void] {
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(false);
  const isSidebarRef = useRef(isSidebarCollapsed);
  isSidebarRef.current = isSidebarCollapsed;
  const wasOpenBeforeInlineRef = useRef(false);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const handleResize = () => {
      if (window.innerWidth < 1024) {
        wasOpenBeforeInlineRef.current = false;
        setIsSidebarCollapsed(true);
      }
    };
    const handleInlineFallback = (ev: Event) => {
      const open = Boolean((ev as CustomEvent<{ open?: boolean }>).detail?.open);
      if (open && !isSidebarRef.current) {
        wasOpenBeforeInlineRef.current = true;
        setIsSidebarCollapsed(true);
      } else if (!open && wasOpenBeforeInlineRef.current) {
        wasOpenBeforeInlineRef.current = false;
        if (window.innerWidth >= 1024) setIsSidebarCollapsed(false);
      }
    };
    handleResize();
    window.addEventListener("resize", handleResize);
    window.addEventListener("cowok:presenter-inline-fallback", handleInlineFallback);
    return () => {
      window.removeEventListener("resize", handleResize);
      window.removeEventListener("cowok:presenter-inline-fallback", handleInlineFallback);
    };
  }, []);
  const toggleSidebar = useCallback(() => {
    wasOpenBeforeInlineRef.current = false;
    setIsSidebarCollapsed((prev) => !prev);
  }, []);
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
  const attachments = useWorkbenchAttachments(sessionManager.activeProjectId || initialProjectId);
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
