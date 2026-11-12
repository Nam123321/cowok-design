// @story 04.1
// @implements AC-04
import React from "react";
import { WorkbenchNode, WorkbenchTransition } from "../store/flow-workbench-store";
import { calculateCubicBezier, Point2D, PortType } from "../utils/dag-algorithms";

const LABEL_WIDTH = 94;
const LABEL_HEIGHT = 22;
const LABEL_CORNER_RADIUS = 11;
const MAX_CONDITION_LABEL_LENGTH = 14;
const TRUNCATED_LABEL_LENGTH = 12;
const HITBOX_STROKE_WIDTH = 16;

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

export const getEdgeStyle = (edgeType?: WorkbenchTransition["edgeType"], isDraft?: boolean) => {
  if (edgeType === "branch") return { strokeColor: "var(--human-accent)", strokeDasharray: "6,4", markerId: "url(#arrow-branch)" };
  if (edgeType === "fallback") return { strokeColor: "var(--agent-accent)", strokeDasharray: "3,3", markerId: "url(#arrow-fallback)" };
  return { strokeColor: "var(--action-primary)", strokeDasharray: isDraft ? "5,3" : "none", markerId: "url(#arrow-main)" };
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
}> = ({ transition, source, target, isSelected, onSelectTransition, nodeWidth, nodeHeight }) => {
  const sourcePortType: PortType = transition.edgeType === "fallback" ? "out-branch" : "out-main";
  const p0 = getPortCoordinates(source, sourcePortType, nodeWidth, nodeHeight);
  const p3 = getPortCoordinates(target, "in", nodeWidth, nodeHeight);
  const bezier = calculateCubicBezier(p0, p3, sourcePortType, "in");
  const edgeStyle = getEdgeStyle(transition.edgeType, transition.status === "draft");
  const displayLabel = formatConditionLabel(transition.conditionLabel);
  const handleClick = (e: React.MouseEvent) => { e.stopPropagation(); onSelectTransition(transition.id); };

  return (
    <g className="dag-edge-group" data-testid={`dag-edge-${transition.id}`}>
      <path d={bezier.d} fill="none" stroke="transparent" strokeWidth={HITBOX_STROKE_WIDTH} style={{ cursor: "pointer", pointerEvents: "stroke" }} onClick={handleClick} />
      <path d={bezier.d} fill="none" stroke={isSelected ? "var(--action-primary)" : edgeStyle.strokeColor} strokeWidth={isSelected ? "2.5" : "1.8"} strokeDasharray={edgeStyle.strokeDasharray} markerEnd={edgeStyle.markerId} style={{ pointerEvents: "none" }} />
      {displayLabel && (
        <g transform={`translate(${bezier.midpoint.x}, ${bezier.midpoint.y})`} style={{ cursor: "pointer", pointerEvents: "auto" }} onClick={handleClick}>
          <rect x={-LABEL_WIDTH / 2} y={-LABEL_HEIGHT / 2} width={LABEL_WIDTH} height={LABEL_HEIGHT} rx={LABEL_CORNER_RADIUS} fill="var(--bg-surface)" stroke={edgeStyle.strokeColor} strokeWidth="1" />
          <text x="0" y="3" textAnchor="middle" fontSize="10" fontWeight="500" fill="var(--text-secondary)" fontFamily="var(--font-sans)">{displayLabel}</text>
        </g>
      )}
    </g>
  );
};
