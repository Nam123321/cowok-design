// @story 04.1
// @implements AC-06, AC-07, AC-08
export type FlowEdgeType = "main" | "branch" | "fallback";
export type CanonicalFlowType = "main" | "if_else" | "if_not";
export type NodeDraftStatus = "synced" | "draft";
export type SemanticNodeType = "screen" | "modal" | "condition" | "decision" | "action";
export type NodeSubState = "Default" | "Loading" | "Empty" | "Error";

export interface WorkbenchNode {
  id: string;
  screenKey: string;
  title: string;
  nodeType?: SemanticNodeType;
  activeSubState?: NodeSubState;
  fixtureRef?: string;
  fixtureJson?: string;
  routePath?: string;
  componentEntryPath?: string;
  viewportPreset?: string;
  posX: number;
  posY: number;
  version: number;
  layoutVersion?: number;
  clusterId?: string;
  note?: string;
  status: NodeDraftStatus;
}

export interface WorkbenchTransition {
  id: string;
  sourceNodeId: string;
  targetNodeId: string;
  triggerEvent: string;
  edgeType: FlowEdgeType;
  conditionLabel?: string;
  guardCondition?: string | null;
  version: number;
  status: NodeDraftStatus;
}

export interface WorkbenchCluster {
  id: string;
  title: string;
  color: string;
  nodeIds: string[];
  collapsed: boolean;
}

export interface WorkbenchViewport {
  zoom: number;
  panX: number;
  panY: number;
}

export interface StructuralSnapshot {
  graphId?: string;
  graphVersion: number;
  layoutVersion: number;
  nodes: WorkbenchNode[];
  transitions: WorkbenchTransition[];
}

export interface ConflictState {
  kind: "NON_OVERLAPPING" | "SEMANTIC_COLLISION";
  collidingNodeIds: string[];
  collidingEdgeIds: string[];
  remoteSnapshot: StructuralSnapshot;
  backupJson: string;
}

export interface FlowWorkbenchState {
  projectId: string;
  graphVersion: number;
  layoutVersion: number;
  draftId: string | null;
  deployStatus: "idle" | "deploying" | "conflict" | "error";
  conflictState: ConflictState | null;
  persistedNodeIds: string[];
  ephemeralDraftNodeIds: string[];
  nodes: WorkbenchNode[];
  transitions: WorkbenchTransition[];
  clusters: WorkbenchCluster[];
  viewport: WorkbenchViewport;
  selectedNodeId: string | null;
  selectedNodeIds?: string[];
  selectedTransitionId: string | null;
  isDraftDirty: boolean;
  isMinimapOpen: boolean;
  activeFormatTab: "dag" | "slide-cms" | "md" | "assets" | "code";
  undoStack: StructuralSnapshot[];
  redoStack: StructuralSnapshot[];
  lastSyncedSnapshot: StructuralSnapshot;
  lastLayoutSaveTimestamp: number | null;
  lastCoalescedNoteNodeId: string | null;
}

export function edgeTypeToCanonical(edgeType: FlowEdgeType): CanonicalFlowType {
  if (edgeType === "branch") return "if_else";
  if (edgeType === "fallback") return "if_not";
  return "main";
}

export function canonicalToEdgeType(flowType: string): FlowEdgeType {
  if (flowType === "if_else") return "branch";
  if (flowType === "if_not") return "fallback";
  return "main";
}

export function normalizeSemanticNodeType(t?: SemanticNodeType | string): "screen" | "modal" | "condition" | "action" {
  if (t === "modal") return "modal";
  if (t === "condition" || t === "decision") return "condition";
  if (t === "action") return "action";
  return "screen";
}
