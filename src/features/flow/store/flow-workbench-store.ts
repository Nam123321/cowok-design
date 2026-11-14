// @story 04.1
// @implements AC-06, AC-07, AC-08
import type { FlowGraphSnapshot } from "../../../lib/validations/flow-workbench";
import { normalizeFixtureRef } from "../../../lib/validations/screen-fixture";
import { CoordinateSyncQueue } from "./coordinate-sync-queue";
import {
  applyAddNode,
  applyDeleteNode,
  applyDiscardDraft,
  applyHydrateFromSnapshotDto,
  applyHydrateLocal,
  applyNodeNoteCoalesced,
  applyNodePositionUpdate,
  applyRedo,
  applyUndo,
  createCoordinateSyncTarget,
} from "./flow-workbench-actions";
import {
  applyAddTransition,
  applyAutoLayout,
  applyDeployDraft,
  applyDuplicateNode,
  applyReverseTransition,
  createInitialWorkbenchState,
  ensurePinnedDraftSession,
  pushHistorySnapshot,
} from "./flow-workbench-mutations";
import { runResolveConflict, runSyncDeployDraft } from "./flow-workbench-sync";
import type {
  ConflictState,
  FlowEdgeType,
  FlowWorkbenchState,
  NodeDraftStatus,
  NodeSubState,
  SemanticNodeType,
  StructuralSnapshot,
  WorkbenchCluster,
  WorkbenchNode,
  WorkbenchTransition,
  WorkbenchViewport,
} from "./flow-workbench-types";

export type {
  ConflictState,
  FlowEdgeType,
  FlowWorkbenchState,
  NodeDraftStatus,
  NodeSubState,
  SemanticNodeType,
  StructuralSnapshot,
  WorkbenchCluster,
  WorkbenchNode,
  WorkbenchTransition,
  WorkbenchViewport,
};

type Listener = () => void;

const DEFAULT_COORD_DEBOUNCE_MS = 120;

interface StoreRuntimeCtx {
  getState: () => FlowWorkbenchState;
  notify: () => void;
  coordQueue: CoordinateSyncQueue;
  isNetworkSyncEnabled: () => boolean;
}

function createViewportAndSelectionMethods(ctx: StoreRuntimeCtx) {
  const { getState, notify, coordQueue, isNetworkSyncEnabled } = ctx;
  const syncTarget = () => createCoordinateSyncTarget(getState, notify);
  return {
    updateNodePosition(nodeId: string, posX: number, posY: number): "ephemeral_local_only" | "queued_persisted" | "unchanged" {
      const status = applyNodePositionUpdate(getState(), nodeId, posX, posY);
      if (status === "unchanged") return "unchanged";
      notify();
      if (status === "ephemeral_local_only") return "ephemeral_local_only";
      return isNetworkSyncEnabled() ? coordQueue.enqueueMove(syncTarget(), nodeId, posX, posY) : "queued_persisted";
    },
    setNodeSubState(nodeId: string, subState: NodeSubState): void {
      const s = getState();
      const canonical = normalizeFixtureRef(subState);
      const target = s.nodes.find((n) => n.id === nodeId);
      if (!target || (target.fixtureRef === canonical && target.activeSubState === canonical)) return;
      pushHistorySnapshot(s);
      target.activeSubState = canonical;
      target.fixtureRef = canonical;
      if (target.status !== "draft") target.status = "draft";
      s.nodes = [...s.nodes];
      ensurePinnedDraftSession(s);
      notify();
    },
    autoLayoutNodes(): void {
      const s = getState(); applyAutoLayout(s); notify();
      if (isNetworkSyncEnabled()) void coordQueue.flushBatchNow(syncTarget(), s.nodes);
    },
    setViewport(viewport: Partial<WorkbenchViewport>): void { const s = getState(); s.viewport = { ...s.viewport, ...viewport }; notify(); },
    toggleClusterCollapse(clusterId: string): void {
      const s = getState();
      for (const c of s.clusters) if (c.id === clusterId) c.collapsed = !c.collapsed;
      s.clusters = [...s.clusters]; notify();
    },
    toggleMinimap(): void { const s = getState(); s.isMinimapOpen = !s.isMinimapOpen; notify(); },
    selectNode(nodeId: string | null, multi?: boolean): void {
      const s = getState();
      if (!nodeId) {
        s.selectedNodeId = null;
        s.selectedNodeIds = [];
      } else if (multi) {
        const existing = s.selectedNodeIds || (s.selectedNodeId ? [s.selectedNodeId] : []);
        if (existing.includes(nodeId)) {
          s.selectedNodeIds = existing.filter((id) => id !== nodeId);
          s.selectedNodeId = s.selectedNodeIds[0] || null;
        } else {
          s.selectedNodeIds = [...existing, nodeId];
          s.selectedNodeId = nodeId;
        }
      } else {
        s.selectedNodeId = nodeId;
        s.selectedNodeIds = [nodeId];
      }
      if (nodeId) s.selectedTransitionId = null;
      notify();
    },
    selectNodes(nodeIds: string[]): void {
      const s = getState();
      s.selectedNodeIds = nodeIds;
      s.selectedNodeId = nodeIds[0] || null;
      if (nodeIds.length > 0) s.selectedTransitionId = null;
      notify();
    },
    selectTransition(id: string | null): void { const s = getState(); s.selectedTransitionId = id; if (id) s.selectedNodeId = null; notify(); },
    setActiveFormatTab(tab: "dag" | "slide-cms" | "md" | "assets" | "code"): void { getState().activeFormatTab = tab; notify(); },
  };
}

function createStructuralDraftMethods(ctx: Pick<StoreRuntimeCtx, "getState" | "notify">) {
  const { getState, notify } = ctx;
  return {
    addNode(node: Omit<WorkbenchNode, "version" | "status">): WorkbenchNode { const created = applyAddNode(getState(), node); notify(); return created; },
    renameNode(nodeId: string, title: string): void {
      const s = getState(); pushHistorySnapshot(s);
      for (const n of s.nodes) if (n.id === nodeId) { n.title = title; n.status = "draft"; }
      s.nodes = [...s.nodes]; ensurePinnedDraftSession(s); notify();
    },
    duplicateNode(nodeId: string): WorkbenchNode | null { const cloned = applyDuplicateNode(getState(), nodeId); if (cloned) notify(); return cloned; },
    deleteNode(nodeId: string): void { applyDeleteNode(getState(), nodeId); notify(); },
    addTransition(transition: Omit<WorkbenchTransition, "id" | "version" | "status"> & { id?: string }) {
      const res = applyAddTransition(getState(), transition); if (res.success) notify(); return res;
    },
    deleteTransition(transitionId: string): void {
      const s = getState(); pushHistorySnapshot(s);
      s.transitions = s.transitions.filter((t) => t.id !== transitionId);
      if (s.selectedTransitionId === transitionId) s.selectedTransitionId = null;
      ensurePinnedDraftSession(s); notify();
    },
    reverseTransition(transitionId: string) { const res = applyReverseTransition(getState(), transitionId); if (res.success) notify(); return res; },
    updateTransition(transitionId: string, updates: Partial<Pick<WorkbenchTransition, "edgeType" | "conditionLabel" | "guardCondition" | "triggerEvent">>): void {
      const s = getState(); pushHistorySnapshot(s);
      for (const t of s.transitions) if (t.id === transitionId) Object.assign(t, updates, { status: "draft" });
      s.transitions = [...s.transitions]; ensurePinnedDraftSession(s); notify();
    },
    updateNodeNote(nodeId: string, note: string): void { applyNodeNoteCoalesced(getState(), nodeId, note); notify(); },
  };
}

function createLifecycleSyncMethods(ctx: Pick<StoreRuntimeCtx, "getState" | "notify">) {
  const { getState, notify } = ctx;
  return {
    undo(): void { if (applyUndo(getState())) notify(); },
    redo(): void { if (applyRedo(getState())) notify(); },
    deployDraft() { const res = applyDeployDraft(getState()); notify(); return res; },
    syncDeployDraft: (projectId?: string) => runSyncDeployDraft(getState, projectId ?? getState().projectId, notify),
    resolveConflict: (choice: "keep_mine" | "take_agent" | "inspect_collisions") => runResolveConflict(getState, choice, notify),
    discardDraft(): void { applyDiscardDraft(getState()); notify(); },
    hydrate(nodes: WorkbenchNode[], transitions: WorkbenchTransition[]): void { applyHydrateLocal(getState(), nodes, transitions); notify(); },
    hydrateFromSnapshot(dto: FlowGraphSnapshot, restoreSessionDraft = true): void { applyHydrateFromSnapshotDto(getState(), dto, restoreSessionDraft); notify(); },
  };
}

export type FlowWorkbenchStore = ReturnType<typeof createNewFlowWorkbenchStore>;

export function createNewFlowWorkbenchStore(
  initialNodes: WorkbenchNode[] = [],
  initialTransitions: WorkbenchTransition[] = [],
  options?: { projectId?: string; enableNetworkLayoutSync?: boolean; debounceMs?: number }
) {
  let state: FlowWorkbenchState = createInitialWorkbenchState(initialNodes, initialTransitions, options?.projectId);
  const listeners = new Set<Listener>();
  const coordQueue = new CoordinateSyncQueue(options?.debounceMs ?? DEFAULT_COORD_DEBOUNCE_MS);
  let enableNetworkSync = Boolean(options?.enableNetworkLayoutSync);

  const getState = (): FlowWorkbenchState => state;
  const notify = () => {
    state = { ...state };
    for (const fn of listeners) fn();
  };
  const ctx: StoreRuntimeCtx = { getState, notify, coordQueue, isNetworkSyncEnabled: () => enableNetworkSync };

  return {
    setNetworkSyncEnabled(enabled: boolean, projectId?: string): void {
      enableNetworkSync = enabled;
      if (projectId) state.projectId = projectId;
    },
    getState,
    subscribe: (listener: Listener): (() => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    ...createViewportAndSelectionMethods(ctx),
    ...createStructuralDraftMethods(ctx),
    ...createLifecycleSyncMethods(ctx),
  };
}
