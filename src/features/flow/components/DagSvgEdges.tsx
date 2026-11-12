// @story 04.1
// @implements AC-04
import React from "react";
import { useTranslation } from "@/hooks/useTranslation";
import {
  WorkbenchNode,
  WorkbenchTransition,
  WorkbenchCluster,
} from "../store/flow-workbench-store";
import { calculateCubicBezier, Point2D, PortType } from "../utils/dag-algorithms";
import { DagSvgDefs, DagTransitionEdge } from "./DagEdgeRenderers";

export interface DagSvgEdgesProps {
  nodes: WorkbenchNode[];
  transitions: WorkbenchTransition[];
  clusters: WorkbenchCluster[];
  selectedTransitionId: string | null;
  onSelectTransition: (transitionId: string) => void;
  wiringSource: { nodeId: string; portType: PortType; startX: number; startY: number } | null;
  wiringCurrentPos: Point2D | null;
  nodeWidth: number;
  nodeHeight: number;
}

const CLUSTER_PADDING_X = 24;
const CLUSTER_PADDING_Y = 24;
const CLUSTER_HEADER_HEIGHT = 44;
const CLUSTER_CORNER_RADIUS = 12;
const SVG_CANVAS_EXTENT = "5000px";

const DagClusterFrame: React.FC<{
  cluster: WorkbenchCluster;
  nodes: WorkbenchNode[];
  nodeWidth: number;
  nodeHeight: number;
}> = ({ cluster, nodes, nodeWidth, nodeHeight }) => {
  const clusterNodes = nodes.filter((n) => cluster.nodeIds.includes(n.id));
  if (clusterNodes.length === 0) return null;
  const cMinX = Math.min(...clusterNodes.map((n) => n.posX)) - CLUSTER_PADDING_X;
  const cMaxX = Math.max(...clusterNodes.map((n) => n.posX + nodeWidth)) + CLUSTER_PADDING_X;
  const cMinY = Math.min(...clusterNodes.map((n) => n.posY)) - CLUSTER_HEADER_HEIGHT;
  const cMaxY = Math.max(...clusterNodes.map((n) => n.posY + nodeHeight)) + CLUSTER_PADDING_Y;
  return (
    <g className="dag-cluster-frame">
      <rect x={cMinX} y={cMinY} width={cMaxX - cMinX} height={cMaxY - cMinY} rx={CLUSTER_CORNER_RADIUS} fill="none" stroke={cluster.color} strokeWidth="1.5" strokeDasharray="4,4" opacity="0.35" />
      <text x={cMinX + 12} y={cMinY + 22} fill={cluster.color} fontSize="12" fontWeight="600" fontFamily="var(--font-sans)" opacity="0.85">{cluster.title}</text>
    </g>
  );
};

export const DagSvgEdges: React.FC<DagSvgEdgesProps> = (p) => {
  const { t } = useTranslation();
  const wirePath =
    p.wiringSource && p.wiringCurrentPos
      ? calculateCubicBezier({ x: p.wiringSource.startX, y: p.wiringSource.startY }, p.wiringCurrentPos, p.wiringSource.portType, "in").d
      : null;

  return (
    <svg className="dag-svg-edges" aria-label={t("flow.dagEdges", "DAG Edges")} style={{ position: "absolute", left: 0, top: 0, width: SVG_CANVAS_EXTENT, height: SVG_CANVAS_EXTENT, overflow: "visible", pointerEvents: "none" }}>
      <DagSvgDefs />
      {p.clusters.map((c) => <DagClusterFrame key={c.id} cluster={c} nodes={p.nodes} nodeWidth={p.nodeWidth} nodeHeight={p.nodeHeight} />)}
      {p.transitions.map((tr) => {
        const source = p.nodes.find((n) => n.id === tr.sourceNodeId);
        const target = p.nodes.find((n) => n.id === tr.targetNodeId);
        if (!source || !target) return null;
        return <DagTransitionEdge key={tr.id} transition={tr} source={source} target={target} isSelected={p.selectedTransitionId === tr.id} onSelectTransition={p.onSelectTransition} nodeWidth={p.nodeWidth} nodeHeight={p.nodeHeight} />;
      })}
      {wirePath && <path d={wirePath} fill="none" stroke="var(--action-primary)" strokeWidth="2" strokeDasharray="4,4" markerEnd="url(#arrow-main)" />}
    </svg>
  );
};
