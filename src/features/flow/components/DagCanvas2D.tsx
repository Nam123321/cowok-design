// @story 04.1, 04.4, 05.5
// @implements AC-04, AC-07, AC-08, Story 05.5 3-Tier Flow DAG Lens & 4-SubState Node Pills
import React, { useContext, useEffect, useMemo, useRef, useState } from "react";
import { apiFlowSynthesize } from "@/api/flow-synthesize";
import { useTranslation } from "@/hooks/useTranslation";
import type {
  FlowSeverityBand,
  FlowTier,
  StoredFlowSubstate,
  SynthesizedFlowPlan,
  SynthesizedFlowSummary,
} from "@/lib/validations/flow-synthesize";
import { ReadOnlyCanvasContext } from "../hooks/useCompanionCanvasState";
import {
  WorkbenchNode,
  WorkbenchTransition,
  WorkbenchViewport,
  FlowWorkbenchState,
  NodeSubState,
} from "../store/flow-workbench-store";
import { canonicalToEdgeType } from "../store/flow-workbench-types";
import { useKeyedParallelCoordinator } from "../hooks/useKeyedParallelCoordinator";
import { dispatchTransitionToDaemon } from "../hooks/dispatchNodeExecution";
import { NodeExecutionJob, NodeQueueSummary, NodeJobStatus } from "../store/keyed-parallel-coordinator";
import { DagSvgEdges } from "./DagSvgEdges";
import { DagNodeCard, PortMouseDownArgs } from "./DagNodeCard";
import { DagRightVerticalDock } from "./DagRightVerticalDock";
import { DagRadarMinimap } from "./DagRadarMinimap";
import { EdgeConditionInspector } from "./EdgeConditionInspector";
import { DagExecutionSummaryBadge } from "./DagExecutionSummaryBadge";
import { DagEdgeCaseLensBar } from "./DagEdgeCaseLensBar";
import { findOrphanNodes } from "../utils/graph-analysis";
import {
  NODE_WIDTH,
  NODE_HEIGHT,
  resolveCanvasCursor,
  useDagCanvasInteractions,
} from "./useDagCanvasInteractions";

export interface DagCanvas2DProps {
  state: FlowWorkbenchState;
  projectId?: string;
  sessionId?: string;
  onUpdatePosition?: (nodeId: string, posX: number, posY: number) => void;
  onUpdateNodePosition?: (nodeId: string, posX: number, posY: number) => void;
  onSelectNode: (nodeId: string | null, isMulti?: boolean) => void;
  onSelectTransition: (transitionId: string | null) => void;
  onSetViewport?: (viewport: Partial<WorkbenchViewport>) => void;
  onUpdateViewport?: (viewport: Partial<WorkbenchViewport>) => void;
  onAddNode?: (title?: string, posX?: number, posY?: number) => void;
  onAutoLayout?: () => void;
  onSelectSubState?: (nodeId: string, subState: NodeSubState) => void;
  onDuplicateNode?: (nodeId: string) => void;
  onRenameNode?: (nodeId: string, title: string) => void;
  onDeleteNode?: (nodeId: string) => void;
  onAddTransition: (sourceNodeId: string, targetNodeId: string, edgeType?: WorkbenchTransition["edgeType"]) => void;
  onUpdateTransition?: (id: string, updates: Partial<WorkbenchTransition>) => void;
  onReverseTransition?: (id: string) => void;
  onDeleteTransition?: (id: string) => void;
  onUpdateNodeNote: (nodeId: string, note: string) => void;
  onToggleMinimap?: () => void;
  onUndo?: () => void;
  onRedo?: () => void;
  onCommentOnNode?: (screenKey: string) => void;
  onSynthesizedFlow?: (plan: SynthesizedFlowPlan) => void;
}

const CANVAS_GRID_SIZE = 24;
const NOOP = () => {};

interface TierMetadata {
  nodeTiers: Map<string, FlowTier>;
  nodeSeverities: Map<string, FlowSeverityBand>;
  edgeTiers: Map<string, FlowTier>;
  substateFixtures: Map<string, Partial<Record<NodeSubState, string>>>;
}

interface AuxFlowMetadata extends TierMetadata {
  lastSummary: SynthesizedFlowSummary | null;
}

function emptyAuxMetadata(): AuxFlowMetadata {
  return {
    nodeTiers: new Map(),
    nodeSeverities: new Map(),
    edgeTiers: new Map(),
    substateFixtures: new Map(),
    lastSummary: null,
  };
}

function inferNodeTier(node: WorkbenchNode, explicit?: FlowTier): FlowTier {
  if (explicit) return explicit;
  if (node.screenKey.startsWith("fmea-") || node.note?.includes("tier:EDGE_CASE")) {
    return "EDGE_CASE";
  }
  return "PRIMARY";
}

function inferNodeSeverity(node: WorkbenchNode, explicit?: FlowSeverityBand): FlowSeverityBand | undefined {
  if (explicit) return explicit;
  const note = node.note || "";
  if (note.includes("severity:CRITICAL")) return "CRITICAL";
  if (note.includes("severity:HIGH")) return "HIGH";
  if (note.includes("severity:MEDIUM")) return "MEDIUM";
  if (note.includes("severity:LOW")) return "LOW";
  return undefined;
}

export function mapSynthesizedPlanToWorkbenchGraph(plan: SynthesizedFlowPlan): {
  nodes: WorkbenchNode[];
  transitions: WorkbenchTransition[];
} {
  const nodes: WorkbenchNode[] = plan.nodes.map((n) => ({
    id: n.id,
    screenKey: n.screenKey,
    title: n.title,
    nodeType: n.nodeType === "condition" ? "condition" : "screen",
    activeSubState: "DEFAULT",
    fixtureRef: "DEFAULT",
    routePath: n.routePath,
    posX: n.posX,
    posY: n.posY,
    version: 1,
    clusterId: n.clusterId,
    note: `tier:${n.flowTier} severity:${n.severityBand}`,
    status: "synced",
  }));

  const transitions: WorkbenchTransition[] = plan.edges.map((e) => ({
    id: e.id,
    sourceNodeId: e.sourceNodeId,
    targetNodeId: e.targetNodeId,
    triggerEvent: e.triggerEvent,
    edgeType: canonicalToEdgeType(e.flowType),
    conditionLabel: e.guardCondition || undefined,
    guardCondition: e.guardCondition,
    version: 1,
    status: "synced",
  }));

  return { nodes, transitions };
}

function buildAuxMetadataFromPlan(plan: SynthesizedFlowPlan): AuxFlowMetadata {
  const nodeTiers = new Map<string, FlowTier>();
  const nodeSeverities = new Map<string, FlowSeverityBand>();
  const edgeTiers = new Map<string, FlowTier>();
  const substateFixtures = new Map<string, Partial<Record<NodeSubState, string>>>();

  for (const n of plan.nodes) {
    nodeTiers.set(n.id, n.flowTier);
    nodeSeverities.set(n.id, n.severityBand);
  }
  for (const e of plan.edges) {
    edgeTiers.set(e.id, e.flowTier);
  }
  for (const b of plan.substates) {
    const existing = substateFixtures.get(b.nodeId) || {};
    existing[b.substate] = b.fixtureId;
    substateFixtures.set(b.nodeId, existing);
  }

  return { nodeTiers, nodeSeverities, edgeTiers, substateFixtures, lastSummary: plan.summary };
}

function buildAuxMetadataFromStoredSubstates(items: StoredFlowSubstate[]): AuxFlowMetadata {
  const nodeTiers = new Map<string, FlowTier>();
  const nodeSeverities = new Map<string, FlowSeverityBand>();
  const substateFixtures = new Map<string, Partial<Record<NodeSubState, string>>>();

  for (const item of items) {
    nodeTiers.set(item.nodeId, item.flowTier);
    nodeSeverities.set(item.nodeId, item.severityBand);
    const existing = substateFixtures.get(item.nodeId) || {};
    existing[item.substate] = item.fixtureId;
    substateFixtures.set(item.nodeId, existing);
  }

  return { nodeTiers, nodeSeverities, edgeTiers: new Map(), substateFixtures, lastSummary: null };
}

function resolveLiveTierMetadata(
  nodes: WorkbenchNode[],
  transitions: WorkbenchTransition[],
  aux: AuxFlowMetadata
): TierMetadata {
  const nodeTiers = new Map<string, FlowTier>();
  const nodeSeverities = new Map<string, FlowSeverityBand>();
  const edgeTiers = new Map<string, FlowTier>();

  for (const n of nodes) {
    nodeTiers.set(n.id, inferNodeTier(n, aux.nodeTiers.get(n.id)));
    const sev = inferNodeSeverity(n, aux.nodeSeverities.get(n.id));
    if (sev) nodeSeverities.set(n.id, sev);
  }
  for (const tr of transitions) {
    const explicit = aux.edgeTiers.get(tr.id);
    if (explicit) {
      edgeTiers.set(tr.id, explicit);
    } else {
      const isEdgeCase =
        tr.edgeType === "fallback" ||
        tr.edgeType === "loopback" ||
        nodeTiers.get(tr.sourceNodeId) === "EDGE_CASE" ||
        nodeTiers.get(tr.targetNodeId) === "EDGE_CASE";
      edgeTiers.set(tr.id, isEdgeCase ? "EDGE_CASE" : "PRIMARY");
    }
  }
  return { nodeTiers, nodeSeverities, edgeTiers, substateFixtures: aux.substateFixtures };
}

function useHydrateProjectSubstates(
  activeProjectId: string,
  setAuxMeta: React.Dispatch<React.SetStateAction<AuxFlowMetadata>>,
  setShowEdgeCaseFlows: React.Dispatch<React.SetStateAction<boolean>>
) {
  const reqSeqRef = useRef(0);
  useEffect(() => {
    const seq = ++reqSeqRef.current;
    setAuxMeta(emptyAuxMetadata());
    setShowEdgeCaseFlows(false);
    if (!activeProjectId) return undefined;

    const controller = new AbortController();
    void apiFlowSynthesize
      .listSubstates({ projectId: activeProjectId }, controller.signal)
      .then((resp) => {
        if (seq !== reqSeqRef.current || controller.signal.aborted) return;
        setAuxMeta((prev) => {
          const hydrated = buildAuxMetadataFromStoredSubstates(resp.data.items);
          return { ...hydrated, edgeTiers: prev.edgeTiers, lastSummary: prev.lastSummary };
        });
      })
      .catch(() => undefined);

    return () => {
      controller.abort();
    };
  }, [activeProjectId, setAuxMeta, setShowEdgeCaseFlows]);
}

const DagNodesList: React.FC<{
  nodes: WorkbenchNode[];
  orphanNodeIds: Set<string>;
  selectedNodeId: string | null;
  onNodeMouseDown: (e: React.MouseEvent, node: WorkbenchNode) => void;
  onPortMouseDown: (args: PortMouseDownArgs) => void;
  onSelectSubState: (nodeId: string, subState: NodeSubState) => void;
  props: DagCanvas2DProps;
  tierMeta: TierMetadata;
  getNodeStatus: (nodeId: string) => { status: NodeJobStatus; queuePosition: number } | null;
}> = ({
  nodes,
  orphanNodeIds,
  selectedNodeId,
  onNodeMouseDown,
  onPortMouseDown,
  onSelectSubState,
  props,
  tierMeta,
  getNodeStatus,
}) => (
  <div style={{ position: "relative", width: "100%", height: "100%", pointerEvents: "none" }}>
    {nodes.map((node) => (
      <DagNodeCard
        key={node.id}
        node={node}
        isSelected={selectedNodeId === node.id}
        isOrphan={orphanNodeIds.has(node.id)}
        onNodeMouseDown={onNodeMouseDown}
        onPortMouseDown={onPortMouseDown}
        onUpdateNote={props.onUpdateNodeNote}
        onSelectSubState={onSelectSubState}
        onDuplicateNode={props.onDuplicateNode}
        onRenameNode={props.onRenameNode}
        onDeleteNode={props.onDeleteNode}
        onCommentOnNode={props.onCommentOnNode}
        nodeWidth={NODE_WIDTH}
        nodeHeight={NODE_HEIGHT}
        substateFixtures={tierMeta.substateFixtures.get(node.id)}
        flowTier={tierMeta.nodeTiers.get(node.id) || "PRIMARY"}
        severityBand={tierMeta.nodeSeverities.get(node.id)}
        executionStatus={getNodeStatus(node.id) || undefined}
      />
    ))}
  </div>
);

const DagCanvasHudLayer: React.FC<{
  props: DagCanvas2DProps;
  ix: ReturnType<typeof useDagCanvasInteractions>;
  handleUpdateVp: (vp: Partial<WorkbenchViewport>) => void;
  summary: NodeQueueSummary;
  activeJobs: NodeExecutionJob[];
  onCancelJob: (jobId: string) => void;
}> = ({ props, ix, handleUpdateVp, summary, activeJobs, onCancelJob }) => {
  const { t } = useTranslation();
  const isReadOnly = useContext(ReadOnlyCanvasContext);
  const { state, onAddNode, onAutoLayout, onUndo, onRedo, onToggleMinimap } = props;
  const selectedTransition = state.transitions.find((tr) => tr.id === state.selectedTransitionId) || null;
  return (
    <>
      <DagExecutionSummaryBadge summary={summary} activeJobs={activeJobs} onCancelJob={onCancelJob} />
      <EdgeConditionInspector
        transition={selectedTransition}
        onUpdate={(id, updates) => props.onUpdateTransition?.(id, updates)}
        onReverse={(id) => props.onReverseTransition?.(id)}
        onDelete={(id) => props.onDeleteTransition?.(id)}
        onClose={() => props.onSelectTransition(null)}
      />
      <DagRightVerticalDock
        zoom={state.viewport?.zoom || 1}
        canUndo={(state.undoStack?.length || 0) > 0}
        canRedo={(state.redoStack?.length || 0) > 0}
        isMinimapOpen={Boolean(state.isMinimapOpen)}
        isReadOnly={isReadOnly}
        onAddNode={() => (onAddNode ? onAddNode(t("flow.defaultNewNodeTitle", "Màn hình mới")) : undefined)}
        onAutoLayout={onAutoLayout} onUndo={onUndo || NOOP} onRedo={onRedo || NOOP}
        onZoomIn={ix.handleZoomIn} onZoomOut={ix.handleZoomOut} onResetZoom={ix.handleResetZoom}
        onFitView={ix.handleFitView} onToggleMinimap={onToggleMinimap || NOOP}
      />
      {state.isMinimapOpen && (
        <DagRadarMinimap nodes={state.nodes} viewport={state.viewport} onNavigate={(panX, panY) => handleUpdateVp({ panX, panY })} />
      )}
    </>
  );
};

function useThreeTierCanvasView(
  state: FlowWorkbenchState,
  showEdgeCaseFlows: boolean,
  auxMeta: AuxFlowMetadata
) {
  return useMemo(() => {
    const meta = resolveLiveTierMetadata(state.nodes, state.transitions, auxMeta);
    const visibleNodes = showEdgeCaseFlows
      ? state.nodes
      : state.nodes.filter((n) => meta.nodeTiers.get(n.id) !== "EDGE_CASE");
    const visibleNodeIds = new Set(visibleNodes.map((n) => n.id));
    const visibleTransitions = showEdgeCaseFlows
      ? state.transitions
      : state.transitions.filter(
          (tr) =>
            meta.edgeTiers.get(tr.id) !== "EDGE_CASE" &&
            tr.edgeType !== "fallback" &&
            tr.edgeType !== "loopback" &&
            visibleNodeIds.has(tr.sourceNodeId) &&
            visibleNodeIds.has(tr.targetNodeId)
        );
    const primaryCount = state.nodes.filter((n) => meta.nodeTiers.get(n.id) !== "EDGE_CASE").length;
    const edgeCaseCount = state.nodes.length - primaryCount;
    return { baseNodes: state.nodes, visibleNodes, visibleTransitions, meta, primaryCount, edgeCaseCount };
  }, [state.nodes, state.transitions, showEdgeCaseFlows, auxMeta]);
}

export const DagCanvas2D: React.FC<DagCanvas2DProps> = (props) => {
  const { state, onSelectNode, onSelectTransition, onAddNode, onAddTransition } = props;
  const { t } = useTranslation();
  const isReadOnly = useContext(ReadOnlyCanvasContext);
  const coordinator = useKeyedParallelCoordinator();
  const containerRef = useRef<HTMLDivElement>(null);
  const activeProjectId = (props.projectId || state.projectId || "").trim();
  const [showEdgeCaseFlows, setShowEdgeCaseFlows] = useState(false);
  const [auxMeta, setAuxMeta] = useState<AuxFlowMetadata>(emptyAuxMetadata);
  useHydrateProjectSubstates(activeProjectId, setAuxMeta, setShowEdgeCaseFlows);

  const handleUpdateNodePos = props.onUpdateNodePosition || props.onUpdatePosition || NOOP;
  const handleUpdateVp = props.onUpdateViewport || props.onSetViewport || NOOP;
  const ix = useDagCanvasInteractions({
    state, containerRef, handleUpdateNodePos, handleUpdateVp, onSelectNode, onSelectTransition,
    onAddNode, onAddTransition, onUndo: props.onUndo, onRedo: props.onRedo,
    onDuplicateNode: props.onDuplicateNode, onDeleteNode: props.onDeleteNode,
    onDeleteTransition: props.onDeleteTransition, onToggleMinimap: props.onToggleMinimap,
  });
  const view = useThreeTierCanvasView(state, showEdgeCaseFlows, auxMeta);
  const cursor = resolveCanvasCursor(ix.isPanning, ix.isSpacePressed, Boolean(ix.draggedNode));
  const gridPx = `${CANVAS_GRID_SIZE * state.viewport.zoom}px`;
  const stageTransform = `matrix(${state.viewport.zoom}, 0, 0, ${state.viewport.zoom}, ${state.viewport.panX}, ${state.viewport.panY})`;
  const orphanNodeIds = new Set(findOrphanNodes(view.visibleNodes.map((n) => ({ id: n.id })), view.visibleTransitions.map((tr) => ({ source: tr.sourceNodeId, target: tr.targetNodeId }))));
  const activeJobs = coordinator.getActiveJobs();
  const handleCancelJob = (jobId: string) => {
    const job = activeJobs.find((j) => j.id === jobId);
    dispatchTransitionToDaemon({ projectId: job?.projectId || activeProjectId, kind: "cancel", jobId, fence: job?.fencingToken ?? 1, coordinator });
  };
  const scopedStatus = (nodeId: string) => coordinator.getNodeStatus(nodeId, { projectId: props.projectId, sessionId: props.sessionId });
  const handleSelectSubState = (nodeId: string, subState: NodeSubState) => props.onSelectSubState?.(nodeId, subState);
  const handleSynthesized = (plan: SynthesizedFlowPlan) => { setAuxMeta(buildAuxMetadataFromPlan(plan)); props.onSynthesizedFlow?.(plan); };

  return (
    <div ref={containerRef} className="dag-canvas-2d-container" data-testid="dag-canvas-2d" aria-label={t("flow.dagCanvas", "DAG 2D Canvas")} onWheel={ix.handleWheel} onMouseDown={ix.handleCanvasMouseDown} onDoubleClick={ix.handleCanvasDoubleClick} style={{ position: "relative", width: "100%", height: "100%", overflow: "hidden", backgroundColor: "var(--bg-canvas)", cursor, userSelect: "none" }}>
      <div className="dag-canvas-bg" style={{ position: "absolute", inset: 0, backgroundImage: "radial-gradient(circle, var(--border-subtle) 1px, transparent 1px)", backgroundSize: `${gridPx} ${gridPx}`, backgroundPosition: `${state.viewport.panX}px ${state.viewport.panY}px`, pointerEvents: "none" }} />
      <div className="dag-zoom-stage" style={{ position: "absolute", left: 0, top: 0, width: "100%", height: "100%", transformOrigin: "0 0", transform: stageTransform, pointerEvents: "none" }}>
        <DagSvgEdges nodes={view.visibleNodes} transitions={view.visibleTransitions} clusters={state.clusters} selectedTransitionId={state.selectedTransitionId} onSelectTransition={onSelectTransition} wiringSource={ix.wiringSource} wiringCurrentPos={ix.wiringCurrentPos} nodeWidth={NODE_WIDTH} nodeHeight={NODE_HEIGHT} edgeTiers={view.meta.edgeTiers} />
        <DagNodesList nodes={view.visibleNodes} orphanNodeIds={orphanNodeIds} selectedNodeId={state.selectedNodeId} onNodeMouseDown={ix.handleNodeMouseDown} onPortMouseDown={ix.handlePortMouseDown} onSelectSubState={handleSelectSubState} props={props} tierMeta={view.meta} getNodeStatus={scopedStatus} />
      </div>
      <DagEdgeCaseLensBar projectId={activeProjectId} nodes={view.baseNodes} showEdgeCaseFlows={showEdgeCaseFlows} onToggleEdgeCaseFlows={setShowEdgeCaseFlows} primaryClusterCount={view.primaryCount} edgeCaseBranchCount={view.edgeCaseCount} lastSummary={auxMeta.lastSummary} onSynthesized={handleSynthesized} isReadOnly={isReadOnly} />
      <DagCanvasHudLayer props={props} ix={ix} handleUpdateVp={handleUpdateVp} summary={coordinator.summary} activeJobs={activeJobs} onCancelJob={handleCancelJob} />
    </div>
  );
};
