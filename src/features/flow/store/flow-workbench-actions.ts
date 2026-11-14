// @story 04.1
// @implements AC-06, AC-07, AC-08
import { apiFlowWorkbench } from "../../../api/flow-workbench";
import type { FlowGraphSnapshot } from "../../../lib/validations/flow-workbench";
import type { CoordinateSyncTarget } from "./coordinate-sync-queue";
import {
  cloneStructuralSnapshot,
  createDefaultClusters,
  ensurePinnedDraftSession,
  pushHistorySnapshot,
} from "./flow-workbench-mutations";
import { applyServerSnapshotToState, snapshotDtoToStructural } from "./flow-workbench-sync";
import { capUndoStack, clearSessionDraft, loadSessionDraft } from "./session-recovery";
import type { FlowWorkbenchState, WorkbenchNode, WorkbenchTransition } from "./flow-workbench-types";

export function createCoordinateSyncTarget(
  stateRef: () => FlowWorkbenchState,
  notify: () => void
): CoordinateSyncTarget {
  return {
    get projectId() {
      return stateRef().projectId;
    },
    isDeploying: () => stateRef().deployStatus === "deploying",
    isPersistedNode: (nodeId: string) => stateRef().persistedNodeIds.includes(nodeId),
    getNodeLayoutVersion: (nodeId: string) =>
      stateRef().nodes.find((n) => n.id === nodeId)?.layoutVersion || 1,
    onLayoutSynced: (updates, graphLayoutVersion) => {
      const state = stateRef();
      const byId = new Map(updates.map((u) => [u.nodeId, u.layoutVersion]));
      state.nodes = state.nodes.map((n) => (byId.has(n.id) ? { ...n, layoutVersion: byId.get(n.id)! } : n));
      if (graphLayoutVersion !== undefined) state.layoutVersion = graphLayoutVersion;
      notify();
    },
    onStaleLayoutResync: async () => {
      const state = stateRef();
      const snap = await apiFlowWorkbench.getSnapshot(state.projectId);
      const byId = new Map(snap.nodes.map((n) => [n.id, n]));
      state.layoutVersion = snap.layoutVersion;
      state.nodes = state.nodes.map((n) => {
        const r = byId.get(n.id);
        return r ? { ...n, posX: r.posX, posY: r.posY, layoutVersion: r.layoutVersion } : n;
      });
      notify();
    },
  };
}

export function applyNodePositionUpdate(
  state: FlowWorkbenchState,
  nodeId: string,
  posX: number,
  posY: number
): "ephemeral_local_only" | "persisted_changed" | "unchanged" {
  let changed = false;
  state.nodes = state.nodes.map((node) => {
    if (node.id === nodeId && (node.posX !== posX || node.posY !== posY)) {
      changed = true;
      return { ...node, posX, posY };
    }
    return node;
  });
  if (!changed) return "unchanged";
  state.lastLayoutSaveTimestamp = Date.now();
  const isPersisted = state.persistedNodeIds.includes(nodeId);
  if (!isPersisted && state.isDraftDirty) ensurePinnedDraftSession(state);
  return isPersisted ? "persisted_changed" : "ephemeral_local_only";
}

export function applyNodeNoteCoalesced(state: FlowWorkbenchState, nodeId: string, note: string): void {
  if (state.lastCoalescedNoteNodeId !== nodeId) {
    pushHistorySnapshot(state);
    state.lastCoalescedNoteNodeId = nodeId;
  }
  state.nodes = state.nodes.map((n) => (n.id === nodeId ? { ...n, note, status: "draft" } : n));
  ensurePinnedDraftSession(state);
}

export function applyAddNode(
  state: FlowWorkbenchState,
  node: Omit<WorkbenchNode, "version" | "status">
): WorkbenchNode {
  pushHistorySnapshot(state);
  const newNode: WorkbenchNode = {
    nodeType: "screen",
    activeSubState: "DEFAULT",
    fixtureRef: "DEFAULT",
    ...node,
    version: 1,
    layoutVersion: 1,
    status: "draft",
  };
  state.nodes = [...state.nodes, newNode];
  state.ephemeralDraftNodeIds = Array.from(new Set([...state.ephemeralDraftNodeIds, newNode.id]));
  state.selectedNodeId = newNode.id;
  ensurePinnedDraftSession(state);
  return newNode;
}

export function applyDeleteNode(state: FlowWorkbenchState, nodeId: string): void {
  pushHistorySnapshot(state);
  state.nodes = state.nodes.filter((n) => n.id !== nodeId);
  state.ephemeralDraftNodeIds = state.ephemeralDraftNodeIds.filter((id) => id !== nodeId);
  state.transitions = state.transitions.filter((t) => t.sourceNodeId !== nodeId && t.targetNodeId !== nodeId);
  if (state.selectedNodeId === nodeId) state.selectedNodeId = null;
  ensurePinnedDraftSession(state);
}

export function applyUndo(state: FlowWorkbenchState): boolean {
  if (state.undoStack.length === 0) return false;
  state.redoStack = capUndoStack([...state.redoStack, cloneStructuralSnapshot(state)]);
  const prev = state.undoStack.pop()!;
  state.nodes = prev.nodes;
  state.transitions = prev.transitions;
  state.lastCoalescedNoteNodeId = null;
  ensurePinnedDraftSession(state);
  return true;
}

export function applyRedo(state: FlowWorkbenchState): boolean {
  if (state.redoStack.length === 0) return false;
  state.undoStack = capUndoStack([...state.undoStack, cloneStructuralSnapshot(state)]);
  const next = state.redoStack.pop()!;
  state.nodes = next.nodes;
  state.transitions = next.transitions;
  state.lastCoalescedNoteNodeId = null;
  ensurePinnedDraftSession(state);
  return true;
}

export function applyDiscardDraft(state: FlowWorkbenchState): void {
  state.nodes = JSON.parse(JSON.stringify(state.lastSyncedSnapshot.nodes));
  state.transitions = JSON.parse(JSON.stringify(state.lastSyncedSnapshot.transitions));
  state.persistedNodeIds = state.nodes.map((n) => n.id);
  state.ephemeralDraftNodeIds = [];
  state.isDraftDirty = false;
  state.draftId = null;
  state.conflictState = null;
  state.deployStatus = "idle";
  state.undoStack = [];
  state.redoStack = [];
  state.lastCoalescedNoteNodeId = null;
  clearSessionDraft(state.projectId);
}

export function applyHydrateLocal(
  state: FlowWorkbenchState,
  nodes: WorkbenchNode[],
  transitions: WorkbenchTransition[]
): void {
  state.nodes = nodes;
  state.transitions = transitions;
  state.persistedNodeIds = nodes.filter((n) => n.status === "synced").map((n) => n.id);
  state.ephemeralDraftNodeIds = nodes.filter((n) => n.status === "draft").map((n) => n.id);
  state.clusters = createDefaultClusters(nodes);
  state.lastSyncedSnapshot = cloneStructuralSnapshot(state);
  state.isDraftDirty = false;
  state.draftId = null;
  state.undoStack = [];
  state.redoStack = [];
}

export function applyHydrateFromSnapshotDto(
  state: FlowWorkbenchState,
  dto: FlowGraphSnapshot,
  restoreSessionDraft: boolean
): void {
  const serverSnap = snapshotDtoToStructural(dto);
  applyServerSnapshotToState(state, serverSnap);
  if (restoreSessionDraft) {
    const saved = loadSessionDraft(state.projectId);
    if (saved && saved.baseSnapshot.graphVersion === dto.graphVersion) {
      state.draftId = saved.draftId;
      state.nodes = saved.nodes;
      state.transitions = saved.transitions;
      const baseIds = new Set(serverSnap.nodes.map((n) => n.id));
      state.ephemeralDraftNodeIds = saved.nodes.filter((n) => !baseIds.has(n.id)).map((n) => n.id);
      state.isDraftDirty = true;
    }
  }
}
