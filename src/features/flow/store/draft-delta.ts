// @story 04.1
// @implements AC-06, AC-08
import type { DeployDraftInput } from "../../../lib/validations/flow-workbench";
import { normalizeFixtureRef, normalizeViewportPreset } from "../../../lib/validations/screen-fixture";
import {
  edgeTypeToCanonical,
  normalizeSemanticNodeType,
  type StructuralSnapshot,
  type WorkbenchNode,
  type WorkbenchTransition,
} from "./flow-workbench-types";

export interface DraftDeltaSummary {
  payload: DeployDraftInput;
  touchedNodeIds: Set<string>;
  touchedEdgeIds: Set<string>;
}

function hasSemanticNodeChange(base: WorkbenchNode, curr: WorkbenchNode): boolean {
  return (
    base.title !== curr.title ||
    normalizeSemanticNodeType(base.nodeType) !== normalizeSemanticNodeType(curr.nodeType) ||
    (base.routePath || "/") !== (curr.routePath || "/") ||
    normalizeFixtureRef(base.fixtureRef) !== normalizeFixtureRef(curr.fixtureRef) ||
    (base.fixtureJson || "{}") !== (curr.fixtureJson || "{}") ||
    (base.note || "") !== (curr.note || "")
  );
}

function hasSemanticEdgeChange(base: WorkbenchTransition, curr: WorkbenchTransition): boolean {
  return (
    base.sourceNodeId !== curr.sourceNodeId ||
    base.targetNodeId !== curr.targetNodeId ||
    base.triggerEvent !== curr.triggerEvent ||
    base.edgeType !== curr.edgeType ||
    (base.guardCondition || null) !== (curr.guardCondition || null)
  );
}

function buildCreatedNodeDtos(nodes: WorkbenchNode[], baseNodes: Map<string, WorkbenchNode>) {
  return nodes
    .filter((n) => !baseNodes.has(n.id))
    .map((n) => ({
      id: n.id,
      portalId: "default",
      screenKey: n.screenKey,
      title: n.title,
      nodeType: normalizeSemanticNodeType(n.nodeType),
      routePath: n.routePath || "/",
      componentEntryPath: n.componentEntryPath || "",
      fixtureRef: normalizeFixtureRef(n.fixtureRef),
      fixtureJson: n.fixtureJson || "{}",
      note: n.note || "",
      viewportPreset: normalizeViewportPreset(n.viewportPreset),
      posX: n.posX,
      posY: n.posY,
    }));
}

function buildUpdatedNodeDtos(nodes: WorkbenchNode[], baseNodes: Map<string, WorkbenchNode>) {
  return nodes
    .filter((n) => baseNodes.has(n.id) && hasSemanticNodeChange(baseNodes.get(n.id)!, n))
    .map((n) => ({
      nodeId: n.id,
      patch: {
        title: n.title,
        nodeType: normalizeSemanticNodeType(n.nodeType),
        routePath: n.routePath || "/",
        fixtureRef: normalizeFixtureRef(n.fixtureRef),
        fixtureJson: n.fixtureJson || "{}",
        note: n.note || "",
        expectedVersion: baseNodes.get(n.id)!.version,
      },
    }));
}

const VALID_TRIGGERS = new Set(["onClick", "onSubmit", "onNavigate", "onTimeout", "custom"]);

function normalizeTriggerEvent(raw?: string): "onClick" | "onSubmit" | "onNavigate" | "onTimeout" | "custom" {
  return VALID_TRIGGERS.has(raw || "")
    ? (raw as "onClick" | "onSubmit" | "onNavigate" | "onTimeout" | "custom")
    : "custom";
}

function buildUpdatedTransitionDtos(
  transitions: WorkbenchTransition[],
  baseEdges: Map<string, WorkbenchTransition>
) {
  return transitions
    .filter((e) => baseEdges.has(e.id) && hasSemanticEdgeChange(baseEdges.get(e.id)!, e))
    .map((e) => {
      const b = baseEdges.get(e.id)!;
      const reversed = b.sourceNodeId === e.targetNodeId && b.targetNodeId === e.sourceNodeId;
      return {
        transitionId: e.id,
        patch: {
          flowType: edgeTypeToCanonical(e.edgeType),
          triggerEvent: normalizeTriggerEvent(e.triggerEvent),
          guardCondition: e.guardCondition ?? e.conditionLabel ?? null,
          reversed: reversed || undefined,
          expectedVersion: b.version,
        },
      };
    });
}

function buildUpdatedPositionDtos(nodes: WorkbenchNode[], baseNodes: Map<string, WorkbenchNode>) {
  return nodes
    .filter((n) => {
      const b = baseNodes.get(n.id);
      return b !== undefined && (b.posX !== n.posX || b.posY !== n.posY);
    })
    .map((n) => ({
      nodeId: n.id,
      posX: n.posX,
      posY: n.posY,
      expectedLayoutVersion: n.layoutVersion || baseNodes.get(n.id)!.layoutVersion || 1,
    }));
}

function buildCreatedTransitionDtos(transitions: WorkbenchTransition[], baseEdges: Map<string, WorkbenchTransition>) {
  return transitions
    .filter((e) => !baseEdges.has(e.id))
    .map((e) => ({
      id: e.id,
      sourceNodeId: e.sourceNodeId,
      targetNodeId: e.targetNodeId,
      flowType: edgeTypeToCanonical(e.edgeType),
      triggerEvent: normalizeTriggerEvent(e.triggerEvent),
      guardCondition: e.guardCondition ?? e.conditionLabel ?? null,
    }));
}

export function computeDraftDelta(
  baseSnapshot: StructuralSnapshot,
  currentDraft: { nodes: WorkbenchNode[]; transitions: WorkbenchTransition[] },
  draftId: string
): DraftDeltaSummary {
  const baseNodes = new Map(baseSnapshot.nodes.map((n) => [n.id, n]));
  const currNodes = new Map(currentDraft.nodes.map((n) => [n.id, n]));
  const baseEdges = new Map(baseSnapshot.transitions.map((e) => [e.id, e]));
  const currEdges = new Map(currentDraft.transitions.map((e) => [e.id, e]));

  const createdNodes = buildCreatedNodeDtos(currentDraft.nodes, baseNodes);
  const updatedNodes = buildUpdatedNodeDtos(currentDraft.nodes, baseNodes);
  const deletedNodes = baseSnapshot.nodes
    .filter((n) => !currNodes.has(n.id))
    .map((n) => ({ nodeId: n.id, expectedVersion: n.version }));
  const updatedNodePositions = buildUpdatedPositionDtos(currentDraft.nodes, baseNodes);
  const createdTransitions = buildCreatedTransitionDtos(currentDraft.transitions, baseEdges);
  const updatedTransitions = buildUpdatedTransitionDtos(currentDraft.transitions, baseEdges);
  const deletedTransitions = baseSnapshot.transitions
    .filter((e) => !currEdges.has(e.id))
    .map((e) => ({ transitionId: e.id, expectedVersion: e.version }));

  return {
    payload: {
      draftId,
      expectedGraphVersion: Math.max(1, baseSnapshot.graphVersion || 1),
      createdNodes,
      updatedNodes,
      deletedNodes,
      updatedNodePositions,
      createdTransitions,
      updatedTransitions,
      deletedTransitions,
    },
    touchedNodeIds: new Set([...updatedNodes.map((u) => u.nodeId), ...deletedNodes.map((d) => d.nodeId)]),
    touchedEdgeIds: new Set([...updatedTransitions.map((u) => u.transitionId), ...deletedTransitions.map((d) => d.transitionId)]),
  };
}

export function classifyThreeWayConflict(
  baseSnapshot: StructuralSnapshot,
  remoteSnapshot: StructuralSnapshot,
  localDelta: DraftDeltaSummary
): { kind: "NON_OVERLAPPING" | "SEMANTIC_COLLISION"; collidingNodeIds: string[]; collidingEdgeIds: string[] } {
  const remoteNodes = new Map(remoteSnapshot.nodes.map((n) => [n.id, n]));
  const remoteEdges = new Map(remoteSnapshot.transitions.map((e) => [e.id, e]));
  const collidingNodeIds: string[] = [];
  const collidingEdgeIds: string[] = [];

  for (const bNode of baseSnapshot.nodes) {
    const rNode = remoteNodes.get(bNode.id);
    if ((!rNode || rNode.version !== bNode.version) && localDelta.touchedNodeIds.has(bNode.id)) {
      collidingNodeIds.push(bNode.id);
    }
  }
  const createdIds = new Set((localDelta.payload.createdNodes || []).map((n) => n.id));
  for (const edge of localDelta.payload.createdTransitions || []) {
    const srcMissing = !remoteNodes.has(edge.sourceNodeId) && !createdIds.has(edge.sourceNodeId);
    const dstMissing = !remoteNodes.has(edge.targetNodeId) && !createdIds.has(edge.targetNodeId);
    if (srcMissing || dstMissing) collidingEdgeIds.push(edge.id || `${edge.sourceNodeId}->${edge.targetNodeId}`);
  }
  for (const bEdge of baseSnapshot.transitions) {
    const rEdge = remoteEdges.get(bEdge.id);
    if ((!rEdge || rEdge.version !== bEdge.version) && localDelta.touchedEdgeIds.has(bEdge.id)) {
      collidingEdgeIds.push(bEdge.id);
    }
  }

  return {
    kind: collidingNodeIds.length > 0 || collidingEdgeIds.length > 0 ? "SEMANTIC_COLLISION" : "NON_OVERLAPPING",
    collidingNodeIds,
    collidingEdgeIds,
  };
}
