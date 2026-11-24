// @story 06.5
// @implements AC-02, AC-04 (Resizable RightDrawer Studio Header with Undo/Redo, Master Collapse, Embed & Single Submit CTA)
import React from "react";
import { BrandIcon } from "../../../../components/ui/BrandIcon";
import { useTranslation } from "../../../../hooks/useTranslation";
import {
  FONT_WEIGHT_SEMIBOLD,
  ICON_LG,
  ICON_MD,
  ICON_SM,
  STUDIO_ICON_BTN,
  STUDIO_PRIMARY_BTN,
  STUDIO_SECONDARY_BTN,
} from "./studioStyles";

export interface StudioHeaderProps {
  titleId: string;
  canUndo: boolean;
  canRedo: boolean;
  masterCollapsed: boolean;
  submitting: boolean;
  onUndo: () => void;
  onRedo: () => void;
  onToggleMaster: () => void;
  onEmbedIntoSlide: () => void;
  onClose: () => void;
}

const DISABLED_OPACITY = 0.45;
const SUBMITTING_OPACITY = 0.7;

const HEADER_BAR_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: "8px",
  padding: "10px 14px",
  borderBottom: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-surface)",
  flexShrink: 0,
  flexWrap: "wrap",
};

const HistoryAndLayoutControls: React.FC<{
  canUndo: boolean;
  canRedo: boolean;
  masterCollapsed: boolean;
  onUndo: () => void;
  onRedo: () => void;
  onToggleMaster: () => void;
}> = ({ canUndo, canRedo, masterCollapsed, onUndo, onRedo, onToggleMaster }) => (
  <div style={{ display: "inline-flex", alignItems: "center", gap: "4px" }}>
    <button
      type="button"
      data-testid="studio-undo-btn"
      disabled={!canUndo}
      onClick={onUndo}
      title="Hoàn tác (Cmd+Z)"
      style={{ ...STUDIO_ICON_BTN, opacity: canUndo ? 1 : DISABLED_OPACITY, cursor: canUndo ? "pointer" : "not-allowed" }}
    >
      <BrandIcon name="undo" size={ICON_SM} color="currentColor" />
      <span>Undo</span>
    </button>
    <button
      type="button"
      data-testid="studio-redo-btn"
      disabled={!canRedo}
      onClick={onRedo}
      title="Làm lại (Cmd+Shift+Z)"
      style={{ ...STUDIO_ICON_BTN, opacity: canRedo ? 1 : DISABLED_OPACITY, cursor: canRedo ? "pointer" : "not-allowed" }}
    >
      <BrandIcon name="redo" size={ICON_SM} color="currentColor" />
      <span>Redo</span>
    </button>
    <button
      type="button"
      data-testid="studio-toggle-master-btn"
      onClick={onToggleMaster}
      aria-pressed={masterCollapsed}
      title={masterCollapsed ? "Mở rộng bảng điều khiển" : "Thu gọn bảng điều khiển"}
      style={STUDIO_ICON_BTN}
    >
      <BrandIcon name={masterCollapsed ? "panel-right" : "panel-left-close"} size={ICON_SM} color="currentColor" />
      <span>{masterCollapsed ? "Mở bảng trái" : "Thu gọn trái"}</span>
    </button>
  </div>
);

export const StudioHeader: React.FC<StudioHeaderProps> = (props) => {
  const { t } = useTranslation();
  const submitStyle: React.CSSProperties = {
    ...STUDIO_PRIMARY_BTN,
    opacity: props.submitting ? SUBMITTING_OPACITY : 1,
    cursor: props.submitting ? "wait" : "pointer",
  };
  return (
    <div style={HEADER_BAR_STYLE}>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "8px", minWidth: 0 }}>
        <BrandIcon name="sparkles" size={ICON_LG} color="var(--action-primary)" />
        <h2 id={props.titleId} data-testid="studio-drawer-title" style={{ margin: 0, fontSize: "14px", fontWeight: FONT_WEIGHT_SEMIBOLD, whiteSpace: "nowrap" }}>
          {t("deckGen.studioTitle", "Studio Biểu đồ & Dashboard")}
        </h2>
      </div>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "6px", flexWrap: "wrap" }}>
        <HistoryAndLayoutControls
          canUndo={props.canUndo}
          canRedo={props.canRedo}
          masterCollapsed={props.masterCollapsed}
          onUndo={props.onUndo}
          onRedo={props.onRedo}
          onToggleMaster={props.onToggleMaster}
        />
        <button type="button" data-testid="studio-embed-slide-btn" onClick={props.onEmbedIntoSlide} style={STUDIO_SECONDARY_BTN}>
          <BrandIcon name="layers" size={ICON_SM} color="currentColor" />
          <span>Gắn vào Slide hiện tại</span>
        </button>
        <button type="submit" disabled={props.submitting} data-testid="deck-generate-submit-btn" style={submitStyle}>
          <BrandIcon name="sparkles" size={ICON_MD} color="currentColor" />
          <span data-testid="deck-gen-primary-cta">
            {props.submitting ? t("deckGen.generatingCta", "Đang tạo bộ Slide...") : "Lưu & Tạo Bộ Slide"}
          </span>
        </button>
        <button type="button" onClick={props.onClose} aria-label={t("common.close", "Đóng")} data-testid="deck-close-btn" style={{ ...STUDIO_ICON_BTN, padding: "5px" }}>
          <span data-testid="deck-gen-close-btn" style={{ display: "inline-flex", alignItems: "center" }}>
            <BrandIcon name="close" size={ICON_MD} color="currentColor" />
          </span>
        </button>
      </div>
    </div>
  );
};
