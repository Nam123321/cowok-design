// cowok-design — Canonical Right Vertical Dock (DESIGN.md v4.3-OS & Section 12.4 v5.4 True 2D DAG)
// @story-04.1
// @implements AC-5 @implements AC-8
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";

export interface DagRightVerticalDockProps {
  zoom: number;
  canUndo: boolean;
  canRedo: boolean;
  isMinimapOpen: boolean;
  onAddNode: () => void;
  onAutoLayout?: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetZoom: () => void;
  onFitView: () => void;
  onToggleMinimap: () => void;
}

const DOCK_Z_INDEX = 20;
const FONT_WEIGHT_SEMIBOLD = 600;
const BUTTON_OPACITY_DISABLED = 0.4;
const BUTTON_OPACITY_ENABLED = 1;

const DOCK_CONTAINER_STYLE: React.CSSProperties = {
  position: "absolute",
  right: "16px",
  top: "50%",
  transform: "translateY(-50%)",
  display: "flex",
  flexDirection: "column",
  gap: "3px",
  padding: "5px 4px",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-md)",
  boxShadow: "var(--shadow-md)",
  zIndex: DOCK_Z_INDEX,
};

export const DockDivider: React.FC = () => (
  <div style={{ height: "1px", backgroundColor: "var(--border-default)", margin: "2px 0" }} />
);

interface DockButtonProps {
  onClick: () => void;
  disabled?: boolean;
  title: string;
  ariaLabel: string;
  children: React.ReactNode;
  fontSize?: string;
  fontWeight?: number;
  active?: boolean;
}

export const DockButton: React.FC<DockButtonProps> = ({
  onClick,
  disabled = false,
  title,
  ariaLabel,
  children,
  fontSize = "14px",
  fontWeight,
  active = false,
}) => (
  <button
    type="button"
    className="dag-dock-btn"
    onClick={onClick}
    disabled={disabled}
    title={title}
    aria-label={ariaLabel}
    style={{
      width: "28px",
      height: "28px",
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      border: "none",
      background: active ? "var(--bg-selected)" : "transparent",
      cursor: disabled ? "not-allowed" : "pointer",
      borderRadius: "var(--radius-xs)",
      opacity: disabled ? BUTTON_OPACITY_DISABLED : BUTTON_OPACITY_ENABLED,
      color: active ? "var(--human-accent)" : "var(--text-primary)",
      fontWeight: fontWeight ?? "normal",
      fontSize,
      transition: "all var(--duration-fast) var(--ease-standard)",
    }}
  >
    {children}
  </button>
);

export const DockZoomLabel: React.FC<{ zoomPercentage: number; onResetZoom: () => void }> = ({
  zoomPercentage,
  onResetZoom,
}) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const resetTitle = t("canvas.resetZoomTitle", "Reset 100% (⌘0)");
  const resetAria = t("canvas.resetZoomAria", "Reset 100%");
  return (
    <button
      type="button"
      className="dag-dock-btn dag-dock-zoom-label tabular-nums"
      onClick={onResetZoom}
      title={resetTitle}
      aria-label={resetAria}
      style={{
        width: "28px",
        height: "22px",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        border: "1px solid var(--border-default)",
        background: "var(--bg-subtle)",
        cursor: "pointer",
        borderRadius: "3px",
        color: "var(--text-secondary)",
        fontSize: "9px",
        fontWeight: FONT_WEIGHT_SEMIBOLD,
        fontFamily: "var(--font-mono)",
        padding: "0 2px",
      }}
    >
      {formatNumber(zoomPercentage)}%
    </button>
  );
};

const DOCK_ICON_SIZE = 13;
const DOCK_ICON_SIZE_LG = 14;

export const DockHistoryControls: React.FC<{
  canUndo: boolean;
  canRedo: boolean;
  onUndo: () => void;
  onRedo: () => void;
}> = ({ canUndo, canRedo, onUndo, onRedo }) => {
  const { t } = useTranslation();
  return (
    <>
      <DockButton onClick={onUndo} disabled={!canUndo} title={t("canvas.undoTitle", "Hoàn tác (⌘Z)")} ariaLabel={t("canvas.undo", "Hoàn tác")}>
        <BrandIcon name="undo" size={DOCK_ICON_SIZE} color="currentColor" />
      </DockButton>
      <DockButton onClick={onRedo} disabled={!canRedo} title={t("canvas.redoTitle", "Làm lại (⇧⌘Z)")} ariaLabel={t("canvas.redo", "Làm lại")}>
        <BrandIcon name="redo" size={DOCK_ICON_SIZE} color="currentColor" />
      </DockButton>
    </>
  );
};

export const DockZoomControls: React.FC<{
  zoomPercentage: number;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetZoom: () => void;
  onFitView: () => void;
}> = ({ zoomPercentage, onZoomIn, onZoomOut, onResetZoom, onFitView }) => {
  const { t } = useTranslation();
  return (
    <>
      <DockButton onClick={onZoomIn} title={t("canvas.zoomInTitle", "Phóng to (⌘+)")} ariaLabel={t("canvas.zoomIn", "Phóng to")}>
        <BrandIcon name="zoom-in" size={DOCK_ICON_SIZE} color="currentColor" />
      </DockButton>
      <DockZoomLabel zoomPercentage={zoomPercentage} onResetZoom={onResetZoom} />
      <DockButton onClick={onZoomOut} title={t("canvas.zoomOutTitle", "Thu nhỏ (⌘-)")} ariaLabel={t("canvas.zoomOut", "Thu nhỏ")}>
        <BrandIcon name="zoom-out" size={DOCK_ICON_SIZE} color="currentColor" />
      </DockButton>
      <DockButton onClick={onFitView} title={t("canvas.fitViewTitle", "Căn vừa toàn đồ thị (Phím F)")} ariaLabel={t("canvas.fitView", "Căn vừa toàn đồ thị")}>
        <BrandIcon name="fit-view" size={DOCK_ICON_SIZE} color="currentColor" />
      </DockButton>
    </>
  );
};

export const DagRightVerticalDock: React.FC<DagRightVerticalDockProps> = ({
  zoom,
  canUndo,
  canRedo,
  isMinimapOpen,
  onAddNode,
  onAutoLayout,
  onUndo,
  onRedo,
  onZoomIn,
  onZoomOut,
  onResetZoom,
  onFitView,
  onToggleMinimap,
}) => {
  const { t } = useTranslation();
  const zoomPercentage = Math.round(zoom * 100);
  const addNodeLabel = t("flow.addNodeButton", "Thêm Node mới");
  const autoLayoutTitle = t("flow.autoLayoutTitle", "Tự động sắp xếp bố cục (Auto-Layout)");
  const autoLayoutAria = t("flow.autoLayout", "Tự động sắp xếp bố cục");
  const minimapTitle = t("flow.toggleMinimapTitle", "Bật/Tắt 2D Radar Minimap (Phím M)");
  const minimapAria = t("flow.toggleMinimap", "Bật/Tắt Minimap");

  return (
    <aside className="dag-right-vertical-dock" aria-label={t("flow.dagDockAria", "Thanh công cụ đồ thị 2D")} style={DOCK_CONTAINER_STYLE}>
      <DockButton onClick={onAddNode} title={addNodeLabel} ariaLabel={addNodeLabel} fontWeight={FONT_WEIGHT_SEMIBOLD}>
        <BrandIcon name="plus" size={DOCK_ICON_SIZE_LG} color="currentColor" />
      </DockButton>
      {onAutoLayout && (
        <DockButton onClick={onAutoLayout} title={autoLayoutTitle} ariaLabel={autoLayoutAria}>
          <BrandIcon name="layout" size={DOCK_ICON_SIZE} color="currentColor" />
        </DockButton>
      )}
      <DockDivider />
      <DockHistoryControls canUndo={canUndo} canRedo={canRedo} onUndo={onUndo} onRedo={onRedo} />
      <DockDivider />
      <DockZoomControls zoomPercentage={zoomPercentage} onZoomIn={onZoomIn} onZoomOut={onZoomOut} onResetZoom={onResetZoom} onFitView={onFitView} />
      <DockDivider />
      <DockButton onClick={onToggleMinimap} title={minimapTitle} ariaLabel={minimapAria} active={isMinimapOpen}>
        <BrandIcon name="map" size={DOCK_ICON_SIZE} color="currentColor" />
      </DockButton>
    </aside>
  );
};
