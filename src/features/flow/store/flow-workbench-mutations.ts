// @story 04.1
// @implements AC-06, AC-07, AC-08
import { detectCycleDfs, buildAdjacencyList, computeTopologicalAutoLayout } from "../utils/dag-algorithms";
import { computeDraftDelta } from "./draft-delta";
import { capUndoStack, clearSessionDraft, generateDraftUuid, saveSessionDraft } from "./session-recovery";
import type {
  FlowWorkbenchState,
  StructuralSnapshot,
  WorkbenchCluster,
  WorkbenchNode,
  WorkbenchTransition,
} from "./flow-workbench-types";

const DUPLICATE_OFFSET_PX = 36;

export function cloneStructuralSnapshot(state: Pick<FlowWorkbenchState, "graphVersion" | "layoutVersion" | "nodes" | "transitions">): StructuralSnapshot {
  return {
    graphVersion: state.graphVersion,
    layoutVersion: state.layoutVersion,
    nodes: JSON.parse(JSON.stringify(state.nodes)),
    transitions: JSON.parse(JSON.stringify(state.transitions)),
  };
}

export function ensurePinnedDraftSession(state: FlowWorkbenchState): string {
  if (!state.draftId) {
    state.draftId = generateDraftUuid();
  }
  state.isDraftDirty = true;
  saveSessionDraft({
    schema: "cowok-design.draft-session.v2",
    projectId: state.projectId,
    draftId: state.draftId,
    baseSnapshot: state.lastSyncedSnapshot,
    nodes: state.nodes,
    transitions: state.transitions,
    savedAt: Date.now(),
  });
  return state.draftId;
}

export function pushHistorySnapshot(state: FlowWorkbenchState): void {
  state.undoStack = capUndoStack([...state.undoStack, cloneStructuralSnapshot(state)]);
  state.redoStack = [];
  state.lastCoalescedNoteNodeId = null;
}

const FALLBACK_META_RE = /fallback|3ds|otp|retry|error/i;

function classifyNodeClusterId(node: WorkbenchNode): string {
  const explicit = node.clusterId?.trim();
  if (explicit) return explicit;
  if (node.nodeType === "condition" || node.nodeType === "decision") return "cluster-fallback";
  const metaText = `${node.screenKey} ${node.routePath} ${node.title}`;
  return FALLBACK_META_RE.test(metaText) ? "cluster-fallback" : "cluster-main";
}

export function createDefaultClusters(initialNodes: WorkbenchNode[]): WorkbenchCluster[] {
  const grouped = new Map<string, string[]>([
    ["cluster-main", []],
    ["cluster-fallback", []],
  ]);

  for (const node of initialNodes) {
    const cid = classifyNodeClusterId(node);
    const bucket = grouped.get(cid) ?? [];
    bucket.push(node.id);
    grouped.set(cid, bucket);
  }

  const metaByCluster: Record<string, { title: string; color: string }> = {
    "cluster-main": { title: "Luồng Chính (Main Flow)", color: "var(--action-primary)" },
    "cluster-fallback": { title: "Luồng Dự Phòng (IF NOT / Fallback)", color: "var(--state-warning)" },
  };

  return Array.from(grouped.entries()).map(([id, nodeIds]) => ({
    id,
    title: metaByCluster[id]?.title ?? id,
    color: metaByCluster[id]?.color ?? "var(--human-accent)",
    nodeIds,
    collapsed: false,
  }));
}

export function createInitialWorkbenchState(
  initialNodes: WorkbenchNode[],
  initialTransitions: WorkbenchTransition[],
  projectId = "11111111-1111-4111-8111-111111111111"
): FlowWorkbenchState {
  const syncedIds = initialNodes.filter((n) => n.status === "synced").map((n) => n.id);
  const draftIds = initialNodes.filter((n) => n.status === "draft").map((n) => n.id);
  const initialSnapshot: StructuralSnapshot = {
    graphVersion: 1,
    layoutVersion: 1,
    nodes: JSON.parse(JSON.stringify(initialNodes)),
    transitions: JSON.parse(JSON.stringify(initialTransitions)),
  };
  return {
    projectId,
    graphVersion: 1,
    layoutVersion: 1,
    draftId: null,
    deployStatus: "idle",
    conflictState: null,
    persistedNodeIds: syncedIds,
    ephemeralDraftNodeIds: draftIds,
    nodes: initialNodes,
    transitions: initialTransitions,
    clusters: createDefaultClusters(initialNodes),
    viewport: { zoom: 1.0, panX: 0, panY: 0 },
    selectedNodeId: null,
    selectedTransitionId: null,
    isDraftDirty: false,
    isMinimapOpen: true,
    activeFormatTab: "dag",
    undoStack: [],
    redoStack: [],
    lastSyncedSnapshot: initialSnapshot,
    lastLayoutSaveTimestamp: Date.now(),
    lastCoalescedNoteNodeId: null,
  };
}

export function applyDuplicateNode(state: FlowWorkbenchState, nodeId: string): WorkbenchNode | null {
  const source = state.nodes.find((n) => n.id === nodeId);
  if (!source) return null;
  pushHistorySnapshot(state);
  const copyId = generateDraftUuid();
  const clonedNode: WorkbenchNode = {
    ...source,
    id: copyId,
    screenKey: `${source.screenKey}-COPY-${copyId.slice(0, 4)}`,
    title: `${source.title} (Copy)`,
    posX: source.posX + DUPLICATE_OFFSET_PX,
    posY: source.posY + DUPLICATE_OFFSET_PX,
    version: 1,
    layoutVersion: 1,
    status: "draft",
  };
  state.nodes = [...state.nodes, clonedNode];
  state.ephemeralDraftNodeIds = Array.from(new Set([...state.ephemeralDraftNodeIds, copyId]));
  state.selectedNodeId = clonedNode.id;
  ensurePinnedDraftSession(state);
  return clonedNode;
}

export function applyAddTransition(
  state: FlowWorkbenchState,
  transition: Omit<WorkbenchTransition, "id" | "version" | "status"> & { id?: string }
): { success: boolean; error?: string; transition?: WorkbenchTransition } {
  if (transition.sourceNodeId === transition.targetNodeId) {
    return { success: false, error: "Không cho phép nối tự vòng (Self-loop forbidden)" };
  }
  const proposedEdge = { sourceNodeId: transition.sourceNodeId, targetNodeId: transition.targetNodeId };
  const adj = buildAdjacencyList(state.transitions, proposedEdge);
  const cycleRes = detectCycleDfs(adj);
  if (cycleRes.hasCycle) {
    return { success: false, error: `Chu trình phát hiện: ${cycleRes.cyclePath.join(" -> ")}` };
  }

  pushHistorySnapshot(state);
  const newTransition: WorkbenchTransition = {
    id: transition.id || generateDraftUuid(),
    ...transition,
    version: 1,
    status: "draft",
  };
  state.transitions = [...state.transitions, newTransition];
  state.selectedTransitionId = newTransition.id;
  ensurePinnedDraftSession(state);
  return { success: true, transition: newTransition };
}

export function applyReverseTransition(
  state: FlowWorkbenchState,
  transitionId: string
): { success: boolean; error?: string } {
  const target = state.transitions.find((t) => t.id === transitionId);
  if (!target) return { success: false, error: "Transition not found" };

  const otherEdges = state.transitions.filter((t) => t.id !== transitionId);
  const proposed = { sourceNodeId: target.targetNodeId, targetNodeId: target.sourceNodeId };
  const cycleRes = detectCycleDfs(buildAdjacencyList(otherEdges, proposed));
  if (cycleRes.hasCycle) {
    return { success: false, error: `Đảo chiều tạo chu trình: ${cycleRes.cyclePath.join(" -> ")}` };
  }

  pushHistorySnapshot(state);
  state.transitions = state.transitions.map((t) =>
    t.id === transitionId
      ? { ...t, sourceNodeId: target.targetNodeId, targetNodeId: target.sourceNodeId, status: "draft" }
      : t
  );
  ensurePinnedDraftSession(state);
  return { success: true };
}

export function applyDeployDraft(state: FlowWorkbenchState): { schema: string; draftPayload: object } {
  const draftId = state.draftId || generateDraftUuid();
  const delta = computeDraftDelta(state.lastSyncedSnapshot, state, draftId);
  state.nodes = state.nodes.map((n) => ({ ...n, status: "synced" }));
  state.transitions = state.transitions.map((t) => ({ ...t, status: "synced" }));
  state.persistedNodeIds = state.nodes.map((n) => n.id);
  state.ephemeralDraftNodeIds = [];
  state.isDraftDirty = false;
  state.draftId = null;
  state.deployStatus = "idle";
  state.conflictState = null;
  state.graphVersion += 1;
  state.lastSyncedSnapshot = cloneStructuralSnapshot(state);
  state.undoStack = [];
  state.redoStack = [];
  state.lastCoalescedNoteNodeId = null;
  clearSessionDraft(state.projectId);
  return { schema: "cowok-design.flow-diff.v1", draftPayload: delta.payload };
}

export function applyAutoLayout(state: FlowWorkbenchState): void {
  state.nodes = computeTopologicalAutoLayout(state.nodes, state.transitions);
  state.lastLayoutSaveTimestamp = Date.now();
}
