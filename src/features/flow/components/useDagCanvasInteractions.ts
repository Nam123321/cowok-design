// @story 04.1
// @implements AC-04, AC-07, AC-08
import React, { useState, useEffect } from "react";
import {
  WorkbenchNode,
  WorkbenchTransition,
  WorkbenchViewport,
  FlowWorkbenchState,
} from "../store/flow-workbench-store";
import { calculateZoomPan, Point2D, PortType } from "../utils/dag-algorithms";
import { PortMouseDownArgs } from "./DagNodeCard";
import { useDagKeyboardShortcuts } from "./useDagKeyboardShortcuts";

export const NODE_WIDTH = 220;
export const NODE_HEIGHT = 140;
const ZOOM_BUTTON_STEP = 0.2;
const ZOOM_MIN = 0.2;
const ZOOM_MAX = 2.0;
const FIT_VIEW_ZOOM = 0.85;
const FIT_VIEW_PAN = 40;
const WHEEL_ZOOM_SENSITIVITY = 0.005;
const PORT_SNAP_RADIUS = 18;

export interface WiringSourceState {
  nodeId: string;
  portType: PortType;
  startX: number;
  startY: number;
}

const getContainerOffset = (containerRef: React.RefObject<HTMLDivElement | null>): Point2D => {
  const rect = containerRef.current?.getBoundingClientRect();
  return { x: rect?.left ?? 0, y: rect?.top ?? 0 };
};

const clientToWorld = (clientX: number, clientY: number, offset: Point2D, vp: WorkbenchViewport): Point2D => ({
  x: (clientX - offset.x - vp.panX) / vp.zoom,
  y: (clientY - offset.y - vp.panY) / vp.zoom,
});

const findTargetNodeUnderCursor = (nodes: WorkbenchNode[], sourceId: string, mouseX: number, mouseY: number) =>
  nodes.find(
    (n) =>
      n.id !== sourceId &&
      mouseX >= n.posX - PORT_SNAP_RADIUS &&
      mouseX <= n.posX + NODE_WIDTH + PORT_SNAP_RADIUS &&
      mouseY >= n.posY - PORT_SNAP_RADIUS &&
      mouseY <= n.posY + NODE_HEIGHT + PORT_SNAP_RADIUS
  );

export function resolveCanvasCursor(isPanning: boolean, isSpacePressed: boolean, hasDraggedNode: boolean): string {
  if (isPanning || isSpacePressed) return "grabbing";
  if (hasDraggedNode) return "move";
  return "default";
}

export interface UseDagCanvasInteractionsParams {
  state: FlowWorkbenchState;
  containerRef: React.RefObject<HTMLDivElement | null>;
  handleUpdateNodePos: (nodeId: string, posX: number, posY: number) => void;
  handleUpdateVp: (viewport: Partial<WorkbenchViewport>) => void;
  onSelectNode: (nodeId: string | null, isMulti?: boolean) => void;
  onSelectTransition: (transitionId: string | null) => void;
  onAddNode?: (title: string, posX?: number, posY?: number) => void;
  onAddTransition: (sourceNodeId: string, targetNodeId: string, edgeType?: WorkbenchTransition["edgeType"]) => void;
  onUndo?: () => void;
  onRedo?: () => void;
  onDuplicateNode?: (nodeId: string) => void;
  onDeleteNode?: (nodeId: string) => void;
  onDeleteTransition?: (transitionId: string) => void;
  onToggleMinimap?: () => void;
}

interface WindowPointerListenerArgs {
  p: UseDagCanvasInteractionsParams;
  draggedNode: { id: string; offsetX: number; offsetY: number } | null;
  setDraggedNode: React.Dispatch<React.SetStateAction<{ id: string; offsetX: number; offsetY: number } | null>>;
  isPanning: boolean;
  setIsPanning: React.Dispatch<React.SetStateAction<boolean>>;
  panStart: Point2D;
  wiringSource: WiringSourceState | null;
  setWiringSource: React.Dispatch<React.SetStateAction<WiringSourceState | null>>;
  setWiringCurrentPos: React.Dispatch<React.SetStateAction<Point2D | null>>;
}

function useCanvasWindowPointerListeners(args: WindowPointerListenerArgs) {
  const { p, draggedNode, setDraggedNode, isPanning, setIsPanning, panStart, wiringSource, setWiringSource, setWiringCurrentPos } = args;
  useEffect(() => {
    if (!draggedNode && !isPanning && !wiringSource) return;
    const onMove = (e: MouseEvent) => {
      const offset = getContainerOffset(p.containerRef);
      if (draggedNode) {
        const w = clientToWorld(e.clientX, e.clientY, offset, p.state.viewport);
        p.handleUpdateNodePos(draggedNode.id, Math.round(w.x - draggedNode.offsetX), Math.round(w.y - draggedNode.offsetY));
      } else if (wiringSource) {
        setWiringCurrentPos(clientToWorld(e.clientX, e.clientY, offset, p.state.viewport));
      } else if (isPanning) {
        p.handleUpdateVp({ panX: e.clientX - panStart.x, panY: e.clientY - panStart.y });
      }
    };
    const onUp = (e: MouseEvent) => {
      if (draggedNode) setDraggedNode(null);
      if (isPanning) setIsPanning(false);
      if (wiringSource) {
        const w = clientToWorld(e.clientX, e.clientY, getContainerOffset(p.containerRef), p.state.viewport);
        const target = findTargetNodeUnderCursor(p.state.nodes, wiringSource.nodeId, w.x, w.y);
        if (target) p.onAddTransition(wiringSource.nodeId, target.id, wiringSource.portType === "out-branch" ? "fallback" : "main");
        setWiringSource(null);
        setWiringCurrentPos(null);
      }
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => { window.removeEventListener("mousemove", onMove); window.removeEventListener("mouseup", onUp); };
  }, [draggedNode, isPanning, wiringSource, panStart, p, setDraggedNode, setIsPanning, setWiringSource, setWiringCurrentPos]);
}

function buildWheelHandler(p: UseDagCanvasInteractionsParams) {
  return (e: React.WheelEvent) => {
    e.preventDefault();
    const offset = getContainerOffset(p.containerRef);
    if (e.ctrlKey || e.metaKey) {
      p.handleUpdateVp(
        calculateZoomPan({
          oldZoom: p.state.viewport.zoom,
          newZoom: p.state.viewport.zoom * (1 + -e.deltaY * WHEEL_ZOOM_SENSITIVITY),
          panX: p.state.viewport.panX,
          panY: p.state.viewport.panY,
          cursorClientX: e.clientX - offset.x,
          cursorClientY: e.clientY - offset.y,
        })
      );
    } else {
      p.handleUpdateVp({ panX: p.state.viewport.panX - e.deltaX, panY: p.state.viewport.panY - e.deltaY });
    }
  };
}

export function useDagCanvasInteractions(p: UseDagCanvasInteractionsParams) {
  const [isPanning, setIsPanning] = useState(false);
  const [panStart, setPanStart] = useState<Point2D>({ x: 0, y: 0 });
  const [isSpacePressed, setIsSpacePressed] = useState(false);
  const [draggedNode, setDraggedNode] = useState<{ id: string; offsetX: number; offsetY: number } | null>(null);
  const [wiringSource, setWiringSource] = useState<WiringSourceState | null>(null);
  const [wiringCurrentPos, setWiringCurrentPos] = useState<Point2D | null>(null);

  useDagKeyboardShortcuts({
    state: p.state, handleUpdateVp: p.handleUpdateVp, setIsSpacePressed,
    onUndo: p.onUndo, onRedo: p.onRedo, onDuplicateNode: p.onDuplicateNode,
    onDeleteNode: p.onDeleteNode, onDeleteTransition: p.onDeleteTransition, onToggleMinimap: p.onToggleMinimap,
  });
  useCanvasWindowPointerListeners({
    p, draggedNode, setDraggedNode, isPanning, setIsPanning, panStart, wiringSource, setWiringSource, setWiringCurrentPos,
  });
  const isBg = (t: EventTarget | null) => t === p.containerRef.current || (t as HTMLElement)?.classList.contains("dag-canvas-bg");

  return {
    isPanning, isSpacePressed, draggedNode, wiringSource, wiringCurrentPos,
    handleWheel: buildWheelHandler(p),
    handleCanvasMouseDown: (e: React.MouseEvent) => {
      if (!isBg(e.target) && e.button !== 1 && !isSpacePressed) return;
      e.preventDefault();
      setIsPanning(true);
      setPanStart({ x: e.clientX - p.state.viewport.panX, y: e.clientY - p.state.viewport.panY });
      p.onSelectNode(null); p.onSelectTransition(null);
    },
    handleCanvasDoubleClick: (e: React.MouseEvent) => {
      if (!isBg(e.target)) return;
      const w = clientToWorld(e.clientX, e.clientY, getContainerOffset(p.containerRef), p.state.viewport);
      p.onAddNode?.("Màn hình mới", Math.round(w.x), Math.round(w.y));
    },
    handleNodeMouseDown: (e: React.MouseEvent, node: WorkbenchNode) => {
      e.stopPropagation();
      const isMulti = e.shiftKey || e.metaKey || e.ctrlKey;
      p.onSelectNode(node.id, isMulti);
      p.onSelectTransition(null);
      const w = clientToWorld(e.clientX, e.clientY, getContainerOffset(p.containerRef), p.state.viewport);
      setDraggedNode({ id: node.id, offsetX: w.x - node.posX, offsetY: w.y - node.posY });
    },
    handlePortMouseDown: (args: PortMouseDownArgs) => {
      args.e.stopPropagation();
      setWiringSource({ nodeId: args.nodeId, portType: args.portType, startX: args.startX, startY: args.startY });
      setWiringCurrentPos({ x: args.startX, y: args.startY });
    },
    handleZoomIn: () => p.handleUpdateVp({ zoom: Math.min(p.state.viewport.zoom + ZOOM_BUTTON_STEP, ZOOM_MAX) }),
    handleZoomOut: () => p.handleUpdateVp({ zoom: Math.max(p.state.viewport.zoom - ZOOM_BUTTON_STEP, ZOOM_MIN) }),
    handleResetZoom: () => p.handleUpdateVp({ zoom: 1.0, panX: 0, panY: 0 }),
    handleFitView: () => p.handleUpdateVp({ zoom: FIT_VIEW_ZOOM, panX: FIT_VIEW_PAN, panY: FIT_VIEW_PAN }),
  };
}
