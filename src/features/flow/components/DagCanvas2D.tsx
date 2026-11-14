// @story 04.1
// @implements AC-04, AC-07, AC-08
import React, { useRef, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "@/hooks/useFormatter";
import { useTranslation } from "@/hooks/useTranslation";
import {
  WorkbenchNode,
  WorkbenchTransition,
  WorkbenchViewport,
  FlowWorkbenchState,
  NodeSubState,
} from "../store/flow-workbench-store";
import { useKeyedParallelCoordinator } from "../hooks/useKeyedParallelCoordinator";
import { NodeExecutionJob, NodeQueueSummary } from "../store/keyed-parallel-coordinator";
import { DagSvgEdges } from "./DagSvgEdges";
import { DagNodeCard, PortMouseDownArgs } from "./DagNodeCard";
import { DagRightVerticalDock } from "./DagRightVerticalDock";
import { DagRadarMinimap } from "./DagRadarMinimap";
import { EdgeConditionInspector } from "./EdgeConditionInspector";
import { findOrphanNodes } from "../utils/graph-analysis";
import {
  NODE_WIDTH,
  NODE_HEIGHT,
  resolveCanvasCursor,
  useDagCanvasInteractions,
} from "./useDagCanvasInteractions";

export interface DagCanvas2DProps {
  state: FlowWorkbenchState;
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
  onAddTransition: (
    sourceNodeId: string,
    targetNodeId: string,
    edgeType?: WorkbenchTransition["edgeType"]
  ) => void;
  onUpdateTransition?: (id: string, updates: Partial<WorkbenchTransition>) => void;
  onReverseTransition?: (id: string) => void;
  onDeleteTransition?: (id: string) => void;
  onUpdateNodeNote: (nodeId: string, note: string) => void;
  onToggleMinimap?: () => void;
  onUndo?: () => void;
  onRedo?: () => void;
  onCommentOnNode?: (screenKey: string) => void;
}

const CANVAS_GRID_SIZE = 24;
const NOOP = () => {};

const DagNodesList: React.FC<{
  nodes: WorkbenchNode[];
  orphanNodeIds: Set<string>;
  selectedNodeId: string | null;
  onNodeMouseDown: (e: React.MouseEvent, node: WorkbenchNode) => void;
  onPortMouseDown: (args: PortMouseDownArgs) => void;
  props: DagCanvas2DProps;
  getNodeStatus: (nodeId: string) => { status: "running" | "queued" | "paused" | "completed" | "error" | "cancelled"; queuePosition: number } | null;
}> = ({ nodes, orphanNodeIds, selectedNodeId, onNodeMouseDown, onPortMouseDown, props, getNodeStatus }) => (
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
        onSelectSubState={props.onSelectSubState}
        onDuplicateNode={props.onDuplicateNode}
        onRenameNode={props.onRenameNode}
        onDeleteNode={props.onDeleteNode}
        onCommentOnNode={props.onCommentOnNode}
        nodeWidth={NODE_WIDTH}
        nodeHeight={NODE_HEIGHT}
        executionStatus={getNodeStatus(node.id) || undefined}
      />
    ))}
  </div>
);

const DagExecutionSummaryBadge: React.FC<{
  summary: NodeQueueSummary;
  activeJobs: NodeExecutionJob[];
  onCancelJob: (jobId: string) => void;
}> = ({ summary, activeJobs, onCancelJob }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const [popoverOpen, setPopoverOpen] = useState(false);
  if (summary.totalActive === 0) return null;

  return (
    <div
      className="dag-execution-summary-badge"
      style={{
        position: "absolute",
        top: "14px",
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 25,
      }}
    >
      <button
        type="button"
        onClick={() => setPopoverOpen((p) => !p)}
        style={{
          display: "flex",
          alignItems: "center",
          gap: "6px",
          padding: "5px 12px",
          borderRadius: "var(--radius-pill)",
          backgroundColor: "var(--bg-elevated)",
          border: "1px solid var(--action-primary)",
          color: "var(--text-primary)",
          fontSize: "11px",
          fontWeight: 600,
          boxShadow: "var(--shadow-md)",
          cursor: "pointer",
        }}
      >
        <BrandIcon name="execution-pipeline" size={12} color="var(--action-primary)" />
        <span>{formatNumber(summary.runningCount)} {t("floatingChat.processing", "Đang xử lý")}</span>
        <span style={{ color: "var(--text-muted)" }}>•</span>
        <BrandIcon name="clock" size={11} color="var(--state-warning)" />
        <span style={{ color: "var(--state-warning)" }}>{formatNumber(summary.queuedCount)} {t("floatingChat.queuedStatus", "Chờ")}</span>
        <BrandIcon name="chevron-down" size={10} color="var(--text-muted)" />
      </button>
      {popoverOpen && (
        <div
          role="dialog"
          aria-label={t("flow.runningJobsAria", "Danh sách tác vụ đang chạy")}
          style={{
            position: "absolute",
            top: "34px",
            left: "50%",
            transform: "translateX(-50%)",
            width: "300px",
            backgroundColor: "var(--bg-surface)",
            border: "1px solid var(--border-default)",
            borderRadius: "var(--radius-md)",
            boxShadow: "var(--shadow-lg)",
            padding: "8px",
            display: "flex",
            flexDirection: "column",
            gap: "6px",
            fontSize: "11px",
          }}
        >
          <div style={{ fontWeight: 600, borderBottom: "1px solid var(--border-subtle)", paddingBottom: "4px" }}>
            {t("flow.executionQueueTitle", "Hàng đợi thực thi")} ({formatNumber(activeJobs.length)})
          </div>
          {activeJobs.map((job) => (
            <div
              key={job.id}
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                padding: "4px 6px",
                borderRadius: "var(--radius-xs)",
                backgroundColor: "var(--bg-subtle)",
              }}
            >
              <div style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", flex: 1 }}>
                <span style={{ fontWeight: 600, color: job.status === "running" ? "var(--action-primary)" : "var(--state-warning)" }}>
                  {job.status.toUpperCase()}
                </span>
                <span style={{ marginLeft: "4px", color: "var(--text-secondary)" }}>{job.prompt}</span>
              </div>
              <button
                type="button"
                onClick={() => onCancelJob(job.id)}
                title={t("floatingChat.cancelJob", "Hủy lệnh")}
                style={{
                  border: "none",
                  background: "transparent",
                  color: "var(--state-danger)",
                  cursor: "pointer",
                  fontSize: "11px",
                  padding: "0 4px",
                }}
              >
                <BrandIcon name="close" size={11} color="currentColor" />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

const DagCanvasHudLayer: React.FC<{
  props: DagCanvas2DProps;
  ix: ReturnType<typeof useDagCanvasInteractions>;
  handleUpdateVp: (vp: Partial<WorkbenchViewport>) => void;
  summary: NodeQueueSummary;
  activeJobs: NodeExecutionJob[];
  onCancelJob: (jobId: string) => void;
}> = ({ props, ix, handleUpdateVp, summary, activeJobs, onCancelJob }) => {
  const { t } = useTranslation();
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
        onAddNode={() => (onAddNode ? onAddNode(t("flow.defaultNewNodeTitle", "Màn hình mới")) : undefined)}
        onAutoLayout={onAutoLayout}
        onUndo={onUndo || NOOP}
        onRedo={onRedo || NOOP}
        onZoomIn={ix.handleZoomIn}
        onZoomOut={ix.handleZoomOut}
        onResetZoom={ix.handleResetZoom}
        onFitView={ix.handleFitView}
        onToggleMinimap={onToggleMinimap || NOOP}
      />
      {state.isMinimapOpen && (
        <DagRadarMinimap nodes={state.nodes} viewport={state.viewport} onNavigate={(panX, panY) => handleUpdateVp({ panX, panY })} />
      )}
    </>
  );
};

export const DagCanvas2D: React.FC<DagCanvas2DProps> = (props) => {
  const { state, onSelectNode, onSelectTransition, onAddNode, onAddTransition } = props;
  const { t } = useTranslation();
  const coordinator = useKeyedParallelCoordinator();
  const containerRef = useRef<HTMLDivElement>(null);
  const handleUpdateNodePos = props.onUpdateNodePosition || props.onUpdatePosition || NOOP;
  const handleUpdateVp = props.onUpdateViewport || props.onSetViewport || NOOP;
  const ix = useDagCanvasInteractions({
    state, containerRef, handleUpdateNodePos, handleUpdateVp, onSelectNode, onSelectTransition,
    onAddNode, onAddTransition, onUndo: props.onUndo, onRedo: props.onRedo,
    onDuplicateNode: props.onDuplicateNode, onDeleteNode: props.onDeleteNode,
    onDeleteTransition: props.onDeleteTransition, onToggleMinimap: props.onToggleMinimap,
  });
  const cursor = resolveCanvasCursor(ix.isPanning, ix.isSpacePressed, Boolean(ix.draggedNode));
  const gridPx = `${CANVAS_GRID_SIZE * state.viewport.zoom}px`;
  const stageTransform = `matrix(${state.viewport.zoom}, 0, 0, ${state.viewport.zoom}, ${state.viewport.panX}, ${state.viewport.panY})`;
  const orphanNodeIds = new Set(
    findOrphanNodes(
      state.nodes.map((n) => ({ id: n.id })),
      state.transitions.map((tr) => ({ source: tr.sourceNodeId, target: tr.targetNodeId }))
    )
  );

  return (
    <div ref={containerRef} className="dag-canvas-2d-container" data-testid="dag-canvas-2d" aria-label={t("flow.dagCanvas", "DAG 2D Canvas")} onWheel={ix.handleWheel} onMouseDown={ix.handleCanvasMouseDown} onDoubleClick={ix.handleCanvasDoubleClick} style={{ position: "relative", width: "100%", height: "100%", overflow: "hidden", backgroundColor: "var(--bg-canvas)", cursor, userSelect: "none" }}>
      <div className="dag-canvas-bg" style={{ position: "absolute", inset: 0, backgroundImage: "radial-gradient(circle, var(--border-subtle) 1px, transparent 1px)", backgroundSize: `${gridPx} ${gridPx}`, backgroundPosition: `${state.viewport.panX}px ${state.viewport.panY}px`, pointerEvents: "none" }} />
      <div className="dag-zoom-stage" style={{ position: "absolute", left: 0, top: 0, width: "100%", height: "100%", transformOrigin: "0 0", transform: stageTransform, pointerEvents: "none" }}>
        <DagSvgEdges nodes={state.nodes} transitions={state.transitions} clusters={state.clusters} selectedTransitionId={state.selectedTransitionId} onSelectTransition={onSelectTransition} wiringSource={ix.wiringSource} wiringCurrentPos={ix.wiringCurrentPos} nodeWidth={NODE_WIDTH} nodeHeight={NODE_HEIGHT} />
        <DagNodesList nodes={state.nodes} orphanNodeIds={orphanNodeIds} selectedNodeId={state.selectedNodeId} onNodeMouseDown={ix.handleNodeMouseDown} onPortMouseDown={ix.handlePortMouseDown} props={props} getNodeStatus={coordinator.getNodeStatus} />
      </div>
      <DagCanvasHudLayer props={props} ix={ix} handleUpdateVp={handleUpdateVp} summary={coordinator.summary} activeJobs={coordinator.getActiveJobs()} onCancelJob={coordinator.cancelJob} />
    </div>
  );
};
