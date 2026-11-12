// @story-04.1
// @implements AC-5
import React from "react";
import { useTranslation } from "@/hooks/useTranslation";
import { WorkbenchNode, WorkbenchViewport } from "../store/flow-workbench-store";

export interface DagRadarMinimapProps {
  nodes: WorkbenchNode[];
  viewport: WorkbenchViewport;
  onNavigate?: (panX: number, panY: number) => void;
  width?: number;
  height?: number;
}

const DEFAULT_MINIMAP_WIDTH = 160;
const DEFAULT_MINIMAP_HEIGHT = 110;
const NODE_CARD_WIDTH = 220;
const NODE_CARD_HEIGHT = 140;
const MIN_GRAPH_EXTENT = 100;
const MINIMAP_PADDING_BOX = 16;
const MINIMAP_OFFSET_PX = 8;
const VIEWPORT_DEFAULT_W = 800;
const VIEWPORT_DEFAULT_H = 600;
const MIN_NODE_DOT_WIDTH = 6;
const MIN_NODE_DOT_HEIGHT = 4;
const Z_INDEX_RADAR = 15;
const BORDER_RADIUS_CONTAINER = 8;
const BORDER_RADIUS_DOT = 2;
const FONT_WEIGHT_SEMIBOLD = 600;
const DOT_OPACITY = 0.8;
const HEADER_OFFSET_Y = 26;

const calculateGraphScale = (nodes: WorkbenchNode[], width: number, height: number) => {
  const minX = Math.min(...nodes.map((n) => n.posX), 0);
  const maxX = Math.max(...nodes.map((n) => n.posX + NODE_CARD_WIDTH), VIEWPORT_DEFAULT_W);
  const minY = Math.min(...nodes.map((n) => n.posY), 0);
  const maxY = Math.max(...nodes.map((n) => n.posY + NODE_CARD_HEIGHT), VIEWPORT_DEFAULT_H);

  const graphWidth = Math.max(maxX - minX, MIN_GRAPH_EXTENT);
  const graphHeight = Math.max(maxY - minY, MIN_GRAPH_EXTENT);
  const scale = Math.min((width - MINIMAP_PADDING_BOX) / graphWidth, (height - MINIMAP_PADDING_BOX) / graphHeight);

  return { minX, minY, scale };
};

const MinimapHeader: React.FC<{ count: number; title: string }> = ({ count, title }) => (
  <div
    style={{
      padding: "4px 8px",
      fontSize: "10px",
      fontWeight: FONT_WEIGHT_SEMIBOLD,
      color: "var(--cw-text-muted)",
      borderBottom: "1px solid var(--cw-border-subtle)",
      backgroundColor: "var(--bg-subtle)",
      display: "flex",
      justifyContent: "space-between",
    }}
  >
    <span>{title}</span>
    <span>{count} Nodes</span>
  </div>
);

const MinimapNodeDot: React.FC<{
  node: WorkbenchNode;
  minX: number;
  minY: number;
  scale: number;
}> = ({ node, minX, minY, scale }) => {
  const nx = (node.posX - minX) * scale + MINIMAP_OFFSET_PX;
  const ny = (node.posY - minY) * scale + MINIMAP_OFFSET_PX;
  const nw = Math.max(NODE_CARD_WIDTH * scale, MIN_NODE_DOT_WIDTH);
  const nh = Math.max(NODE_CARD_HEIGHT * scale, MIN_NODE_DOT_HEIGHT);

  return (
    <div
      style={{
        position: "absolute",
        left: `${nx}px`,
        top: `${ny}px`,
        width: `${nw}px`,
        height: `${nh}px`,
        backgroundColor: node.status === "draft" ? "var(--agent-accent)" : "var(--human-accent)",
        borderRadius: `${BORDER_RADIUS_DOT}px`,
        opacity: DOT_OPACITY,
      }}
    />
  );
};

const MinimapLens: React.FC<{
  viewX: number;
  viewY: number;
  viewW: number;
  viewH: number;
  width: number;
  height: number;
}> = ({ viewX, viewY, viewW, viewH, width, height }) => (
  <div
    style={{
      position: "absolute",
      left: `${Math.max(0, viewX)}px`,
      top: `${Math.max(0, viewY)}px`,
      width: `${Math.min(width - 2, viewW)}px`,
      height: `${Math.min(height - HEADER_OFFSET_Y, viewH)}px`,
      border: "1px solid var(--action-primary)",
      backgroundColor: "rgba(75, 127, 255, 0.12)",
      borderRadius: `${BORDER_RADIUS_DOT}px`,
    }}
  />
);

export const DagRadarMinimap: React.FC<DagRadarMinimapProps> = ({
  nodes,
  viewport,
  width = DEFAULT_MINIMAP_WIDTH,
  height = DEFAULT_MINIMAP_HEIGHT,
}) => {
  const { t } = useTranslation();
  const { minX, minY, scale } = calculateGraphScale(nodes, width, height);

  const viewX = (-viewport.panX / viewport.zoom - minX) * scale + MINIMAP_OFFSET_PX;
  const viewY = (-viewport.panY / viewport.zoom - minY) * scale + MINIMAP_OFFSET_PX;
  const viewW = (VIEWPORT_DEFAULT_W / viewport.zoom) * scale;
  const viewH = (VIEWPORT_DEFAULT_H / viewport.zoom) * scale;

  return (
    <div
      className="dag-radar-minimap"
      role="region"
      aria-label={t("flow.radarMinimap", "2D Radar Minimap")}
      style={{
        position: "absolute",
        bottom: "16px",
        right: "60px",
        width: `${width}px`,
        height: `${height}px`,
        backgroundColor: "var(--cw-bg-surface)",
        border: "1px solid var(--cw-border-subtle)",
        borderRadius: `${BORDER_RADIUS_CONTAINER}px`,
        boxShadow: "0 4px 16px rgba(8, 30, 58, 0.08)",
        overflow: "hidden",
        zIndex: Z_INDEX_RADAR,
        pointerEvents: "none",
      }}
    >
      <MinimapHeader count={nodes.length} title={t("flow.radarTitle", "RADAR 2D")} />
      <div style={{ position: "relative", width: "100%", height: "calc(100% - 24px)" }}>
        {nodes.map((node) => (
          <MinimapNodeDot key={node.id} node={node} minX={minX} minY={minY} scale={scale} />
        ))}
        <MinimapLens viewX={viewX} viewY={viewY} viewW={viewW} viewH={viewH} width={width} height={height} />
      </div>
    </div>
  );
};
