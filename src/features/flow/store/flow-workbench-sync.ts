// @story 04.1
// @implements AC-03, AC-04, AC-06, AC-08
import { apiFlowWorkbench, FlowApiError } from "../../../api/flow-workbench";
import type { FlowGraphSnapshot } from "../../../lib/validations/flow-workbench";
import { classifyThreeWayConflict, computeDraftDelta } from "./draft-delta";
import { cloneStructuralSnapshot, createDefaultClusters } from "./flow-workbench-mutations";
import { clearSessionDraft, generateDraftUuid } from "./session-recovery";
import {
  canonicalToEdgeType,
  type FlowWorkbenchState,
  type StructuralSnapshot,
  type WorkbenchNode,
  type WorkbenchTransition,
} from "./flow-workbench-types";

export function snapshotDtoToStructural(dto: FlowGraphSnapshot): StructuralSnapshot {
  const nodes: WorkbenchNode[] = dto.nodes.map((n) => ({
    id: n.id,
    screenKey: n.screenKey,
    title: n.title,
    nodeType: n.nodeType,
    activeSubState: (n.fixtureRef as "Default" | "Loading" | "Empty" | "Error") || "Default",
    fixtureRef: n.fixtureRef,
    fixtureJson: n.fixtureJson,
    routePath: n.routePath,
    componentEntryPath: n.componentEntryPath,
    viewportPreset: n.viewportPreset,
    posX: n.posX,
    posY: n.posY,
    version: n.version,
    layoutVersion: n.layoutVersion,
    note: n.note,
    status: "synced",
  }));

  const transitions: WorkbenchTransition[] = dto.transitions.map((t) => ({
    id: t.id,
    sourceNodeId: t.sourceNodeId,
    targetNodeId: t.targetNodeId,
    triggerEvent: t.triggerEvent,
    edgeType: canonicalToEdgeType(t.flowType),
    conditionLabel: t.guardCondition || undefined,
    guardCondition: t.guardCondition,
    version: t.version,
    status: "synced",
  }));

  return {
    graphId: dto.graphId,
    graphVersion: dto.graphVersion,
    layoutVersion: dto.layoutVersion,
    nodes,
    transitions,
  };
}

export function applyServerSnapshotToState(state: FlowWorkbenchState, snap: StructuralSnapshot): void {
  state.graphVersion = snap.graphVersion;
  state.layoutVersion = snap.layoutVersion;
  state.nodes = JSON.parse(JSON.stringify(snap.nodes));
  state.transitions = JSON.parse(JSON.stringify(snap.transitions));
  state.persistedNodeIds = snap.nodes.map((n) => n.id);
  state.ephemeralDraftNodeIds = [];
  state.clusters = createDefaultClusters(state.nodes);
  state.lastSyncedSnapshot = cloneStructuralSnapshot(state);
  state.isDraftDirty = false;
  state.draftId = null;
  state.deployStatus = "idle";
  state.conflictState = null;
  state.undoStack = [];
  state.redoStack = [];
  state.lastCoalescedNoteNodeId = null;
  clearSessionDraft(state.projectId);
}

export async function runSyncDeployDraft(
  getState: () => FlowWorkbenchState,
  projectId: string,
  notify: () => void
): Promise<{ ok: boolean; status: "synced" | "conflict" | "error"; message?: string }> {
  const initial = getState();
  initial.projectId = projectId;
  const draftId = initial.draftId || generateDraftUuid();
  initial.draftId = draftId;
  initial.deployStatus = "deploying";
  notify();

  try {
    const preSend = getState();
    const delta = computeDraftDelta(preSend.lastSyncedSnapshot, preSend, draftId);
    const committed = await apiFlowWorkbench.deployDraft(projectId, delta.payload);
    applyServerSnapshotToState(getState(), snapshotDtoToStructural(committed));
    notify();
    return { ok: true, status: "synced" };
  } catch (err) {
    if (err instanceof FlowApiError && err.status === 409) {
      const curr = getState();
      const delta = computeDraftDelta(curr.lastSyncedSnapshot, curr, draftId);
      const remoteDto = await apiFlowWorkbench.getSnapshot(projectId);
      const remoteSnap = snapshotDtoToStructural(remoteDto);
      const classification = classifyThreeWayConflict(curr.lastSyncedSnapshot, remoteSnap, delta);
      const latest = getState();
      latest.deployStatus = "conflict";
      latest.conflictState = {
        kind: classification.kind,
        collidingNodeIds: classification.collidingNodeIds,
        collidingEdgeIds: classification.collidingEdgeIds,
        remoteSnapshot: remoteSnap,
        backupJson: JSON.stringify(delta.payload, null, 2),
      };
      notify();
      return { ok: false, status: "conflict", message: err.message };
    }
    getState().deployStatus = "error";
    const msg = err instanceof Error ? err.message : String(err);
    notify();
    return { ok: false, status: "error", message: msg };
  }
}

export async function runResolveConflict(
  getState: () => FlowWorkbenchState,
  choice: "keep_mine" | "take_agent" | "inspect_collisions",
  notify: () => void
): Promise<{ backupJson?: string }> {
  const state = getState();
  const conflict = state.conflictState;
  if (!conflict) return {};

  if (choice === "inspect_collisions") {
    const firstNode = conflict.collidingNodeIds[0] || null;
    const firstEdge = firstNode ? null : conflict.collidingEdgeIds[0] || null;
    state.selectedNodeId = firstNode;
    state.selectedTransitionId = firstEdge;
    notify();
    return {};
  }

  if (choice === "take_agent") {
    const backupJson = conflict.backupJson;
    applyServerSnapshotToState(state, conflict.remoteSnapshot);
    notify();
    return { backupJson };
  }

  // "keep_mine": rebase baseSnapshot onto remoteSnapshot with bumped versions and retry once with fresh draftId
  const remoteNodesById = new Map(conflict.remoteSnapshot.nodes.map((n) => [n.id, n]));
  const remoteEdgesById = new Map(conflict.remoteSnapshot.transitions.map((e) => [e.id, e]));
  state.nodes = state.nodes.map((n) => {
    const r = remoteNodesById.get(n.id);
    return r ? { ...n, version: r.version, layoutVersion: r.layoutVersion } : n;
  });
  state.transitions = state.transitions.map((e) => {
    const r = remoteEdgesById.get(e.id);
    return r ? { ...e, version: r.version } : e;
  });
  state.lastSyncedSnapshot = conflict.remoteSnapshot;
  state.graphVersion = conflict.remoteSnapshot.graphVersion;
  state.layoutVersion = conflict.remoteSnapshot.layoutVersion;
  state.draftId = generateDraftUuid();
  state.conflictState = null;
  await runSyncDeployDraft(getState, state.projectId, notify);
  return {};
}
