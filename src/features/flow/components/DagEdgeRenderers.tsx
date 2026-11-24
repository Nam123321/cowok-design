// @story 04.1, 05.1, 05.5
// @implements AC-04, Story 05.5 Native EDGE_CASE Dashed & Loopback Arc Rendering
import React from "react";
import { useTranslation } from "@/hooks/useTranslation";
import { WorkbenchNode, WorkbenchTransition } from "../store/flow-workbench-store";
import { calculateCubicBezier, Point2D, PortType } from "../utils/dag-algorithms";

const LABEL_WIDTH = 94;
const LABEL_HEIGHT = 22;
const LABEL_CORNER_RADIUS = 11;
const MAX_CONDITION_LABEL_LENGTH = 14;
const TRUNCATED_LABEL_LENGTH = 12;
const HITBOX_STROKE_WIDTH = 16;
const LOOPBACK_ARC_DROP_PX = 76;
const LOOPBACK_HALF_OFFSET = 0.5;
const CUBIC_END_WEIGHT = 0.125;
const CUBIC_CTRL_WEIGHT = 0.375;

export const getPortCoordinates = (
  node: WorkbenchNode,
  portType: PortType,
  nodeWidth: number,
  nodeHeight: number
): Point2D => {
  if (portType === "in") return { x: node.posX, y: node.posY + nodeHeight / 2 };
  if (portType === "out-branch") return { x: node.posX + nodeWidth / 2, y: node.posY + nodeHeight };
  return { x: node.posX + nodeWidth, y: node.posY + nodeHeight / 2 };
};

export const getEdgeStyle = (
  edgeType?: WorkbenchTransition["edgeType"],
  isDraft?: boolean,
  flowTier?: string
) => {
  if (edgeType === "loopback") return { strokeColor: "var(--agent-accent)", strokeDasharray: "4,3", markerId: "url(#arrow-loopback)" };
  if (flowTier === "EDGE_CASE" || edgeType === "fallback") return { strokeColor: "var(--state-warning)", strokeDasharray: "6,4", markerId: "url(#arrow-edge-case)" };
  if (edgeType === "branch") return { strokeColor: "var(--human-accent)", strokeDasharray: "6,4", markerId: "url(#arrow-branch)" };
  return { strokeColor: "var(--action-primary)", strokeDasharray: isDraft ? "5,3" : "none", markerId: "url(#arrow-main)" };
};

const buildLoopbackArc = (p0: Point2D, p3: Point2D): { d: string; midpoint: Point2D } => {
  const dropY = Math.max(p0.y, p3.y) + LOOPBACK_ARC_DROP_PX;
  const spanX = Math.max(Math.abs(p0.x - p3.x) * LOOPBACK_HALF_OFFSET, LOOPBACK_ARC_DROP_PX);
  const c1 = { x: p0.x + spanX, y: dropY };
  const c2 = { x: p3.x - spanX, y: dropY };
  return {
    d: `M ${p0.x} ${p0.y} C ${c1.x} ${c1.y}, ${c2.x} ${c2.y}, ${p3.x} ${p3.y}`,
    midpoint: {
      x: CUBIC_END_WEIGHT * (p0.x + p3.x) + CUBIC_CTRL_WEIGHT * (c1.x + c2.x),
      y: CUBIC_END_WEIGHT * (p0.y + p3.y) + CUBIC_CTRL_WEIGHT * (c1.y + c2.y),
    },
  };
};

const formatConditionLabel = (label?: string): string | null => {
  if (!label) return null;
  return label.length > MAX_CONDITION_LABEL_LENGTH ? `${label.slice(0, TRUNCATED_LABEL_LENGTH)}…` : label;
};

export const DagSvgDefs: React.FC = () => (
  <defs>
    <marker id="arrow-main" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M 0 1 L 10 5 L 0 9 z" fill="var(--action-primary)" />
    </marker>
    <marker id="arrow-branch" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M 0 1 L 10 5 L 0 9 z" fill="var(--human-accent)" />
    </marker>
    <marker id="arrow-fallback" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M 0 1 L 10 5 L 0 9 z" fill="var(--agent-accent)" />
    </marker>
    <marker id="arrow-edge-case" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M 0 1 L 10 5 L 0 9 z" fill="var(--state-warning)" />
    </marker>
    <marker id="arrow-loopback" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M 0 1 L 10 5 L 0 9 z" fill="var(--agent-accent)" />
    </marker>
  </defs>
);

export const DagTransitionEdge: React.FC<{
  transition: WorkbenchTransition;
  source: WorkbenchNode;
  target: WorkbenchNode;
  isSelected: boolean;
  onSelectTransition: (transitionId: string) => void;
  nodeWidth: number;
  nodeHeight: number;
  flowTier?: string;
}> = ({ transition, source, target, isSelected, onSelectTransition, nodeWidth, nodeHeight, flowTier }) => {
  const { t } = useTranslation();
  const isLoopback = transition.edgeType === "loopback";
  const resolvedTier = flowTier || (transition.edgeType === "fallback" || isLoopback ? "EDGE_CASE" : "PRIMARY");
  const sourcePortType: PortType = transition.edgeType === "fallback" || resolvedTier === "EDGE_CASE" ? "out-branch" : "out-main";
  const p0 = getPortCoordinates(source, sourcePortType, nodeWidth, nodeHeight);
  const p3 = getPortCoordinates(target, "in", nodeWidth, nodeHeight);
  const curve = isLoopback ? buildLoopbackArc(p0, p3) : calculateCubicBezier(p0, p3, sourcePortType, "in");
  const edgeStyle = getEdgeStyle(transition.edgeType, transition.status === "draft", resolvedTier);
  const displayLabel = formatConditionLabel(transition.conditionLabel);
  const handleClick = (e: React.MouseEvent) => { e.stopPropagation(); onSelectTransition(transition.id); };

  return (
    <g className="dag-edge-group" data-testid={`dag-edge-${transition.id}`} data-flow-tier={resolvedTier} data-edge-type={transition.edgeType} aria-label={t("flow.dagEdge", `Transition ${transition.triggerEvent}`)}>
      <path d={curve.d} fill="none" stroke="transparent" strokeWidth={HITBOX_STROKE_WIDTH} style={{ cursor: "pointer", pointerEvents: "stroke" }} onClick={handleClick} />
      <path d={curve.d} fill="none" stroke={isSelected ? "var(--action-primary)" : edgeStyle.strokeColor} strokeWidth={isSelected ? "2.5" : "1.8"} strokeDasharray={edgeStyle.strokeDasharray} markerEnd={edgeStyle.markerId} style={{ pointerEvents: "none" }} />
      {displayLabel && (
        <g transform={`translate(${curve.midpoint.x}, ${curve.midpoint.y})`} style={{ cursor: "pointer", pointerEvents: "auto" }} onClick={handleClick}>
          <rect x={-LABEL_WIDTH / 2} y={-LABEL_HEIGHT / 2} width={LABEL_WIDTH} height={LABEL_HEIGHT} rx={LABEL_CORNER_RADIUS} fill="var(--bg-surface)" stroke={edgeStyle.strokeColor} strokeWidth="1" />
          <text x="0" y="3" textAnchor="middle" fontSize="10" fontWeight="500" fill="var(--text-secondary)" fontFamily="var(--font-sans)">{displayLabel}</text>
        </g>
      )}
    </g>
  );
};
