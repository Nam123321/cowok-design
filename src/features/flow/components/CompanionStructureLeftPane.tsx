// @story 04.1
// @implements AC-03, AC-06
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ConflictState } from "../store/flow-workbench-types";
import type { SlidePhaseItem } from "./companion-canvas-data";
import { DraftDeployBanner } from "./DraftDeployBanner";

const ICON_SIZE_XS = 10;
const ICON_SIZE_MD = 12;
const ICON_SIZE_LG = 13;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_MEDIUM = 500;
const TEXTAREA_ROWS = 3;

const buildLensButtonStyle = (isActive: boolean): React.CSSProperties => ({
  height: "24px",
  padding: "0 10px",
  borderRadius: "4px 4px 0 0",
  border: "none",
  borderBottom: isActive ? "2px solid var(--action-primary)" : "2px solid transparent",
  backgroundColor: isActive ? "var(--bg-selected)" : "transparent",
  color: isActive ? "var(--text-primary)" : "var(--text-secondary)",
  fontWeight: isActive ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
  fontSize: "11.5px",
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
  cursor: "pointer",
  transition: "all var(--duration-fast) var(--ease-standard)",
});

const SlideListItemCard: React.FC<{
  slide: SlidePhaseItem;
  isCurrent: boolean;
  onSelect: () => void;
}> = ({ slide, isCurrent, onSelect }) => (
  <div
    role="button"
    onClick={onSelect}
    style={{
      padding: "10px 12px",
      borderRadius: "var(--radius-sm)",
      backgroundColor: "var(--bg-surface)",
      border: isCurrent ? "1.5px solid var(--action-primary)" : "1px solid var(--border-default)",
      cursor: "pointer",
      display: "flex",
      flexDirection: "column",
      gap: "4px",
      boxShadow: isCurrent ? "var(--shadow-sm)" : "none",
    }}
  >
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
      <span style={{ fontFamily: "var(--font-mono)", fontSize: "10px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--human-accent)" }}>
        {slide.code}
      </span>
      <span style={{ fontSize: "10px", color: "var(--text-muted)", display: "inline-flex", alignItems: "center", gap: "4px" }}>
        <span>Sửa nội dung</span>
        <BrandIcon name="edit" size={ICON_SIZE_XS} color="currentColor" />
      </span>
    </div>
    <div style={{ fontSize: "12.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>{slide.title}</div>
    <div style={{ fontSize: "11px", color: "var(--text-secondary)" }}>{slide.subtitle}</div>
  </div>
);

const SlideCmsDrawerEditor: React.FC<{
  activeSlide: SlidePhaseItem;
  onCloseDrawer: () => void;
  onUpdateSlide: (patch: Partial<SlidePhaseItem>) => void;
}> = ({ activeSlide, onCloseDrawer, onUpdateSlide }) => (
  <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
      <button type="button" onClick={onCloseDrawer} className="cowok-btn cowok-btn-secondary" style={{ height: "24px", padding: "0 8px", fontSize: "11px", display: "inline-flex", alignItems: "center", gap: "4px", cursor: "pointer" }}>
        <BrandIcon name="arrow-left" size={ICON_SIZE_MD} color="currentColor" />
        <span>Danh sách Slide</span>
      </button>
      <span style={{ fontFamily: "var(--font-mono)", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--human-accent)" }}>
        {activeSlide.code} • Zero-Code CMS
      </span>
    </div>
    <div style={{ padding: "10px", borderRadius: "var(--radius-sm)", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", display: "flex", flexDirection: "column", gap: "6px" }}>
      <label style={{ fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-muted)" }}>Headline</label>
      <input type="text" value={activeSlide.headline} onChange={(e) => onUpdateSlide({ headline: e.target.value })} style={{ width: "100%", padding: "6px 8px", fontSize: "12px", borderRadius: "4px", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", boxSizing: "border-box" }} />
    </div>
    <div style={{ padding: "10px", borderRadius: "var(--radius-sm)", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", display: "flex", flexDirection: "column", gap: "6px" }}>
      <label style={{ fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-muted)" }}>Key Takeaway / Subtitle</label>
      <textarea rows={TEXTAREA_ROWS} value={activeSlide.subtitle} onChange={(e) => onUpdateSlide({ subtitle: e.target.value })} style={{ width: "100%", padding: "6px 8px", fontSize: "12px", borderRadius: "4px", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", boxSizing: "border-box", resize: "vertical" }} />
    </div>
  </div>
);

const StructurePaneHeader: React.FC<{
  structureLens: "userflow" | "slide-cms";
  onSetLens: (lens: "userflow" | "slide-cms") => void;
  isRightPreviewHidden: boolean;
  onToggleRightPreviewHidden: () => void;
}> = ({ structureLens, onSetLens, isRightPreviewHidden, onToggleRightPreviewHidden }) => {
  const { t } = useTranslation();
  const toggleTitle = isRightPreviewHidden
    ? t("canvas.restoreSplitView", "Thu gọn về chế độ Chia đôi (Hiện lại Live HTML Preview)")
    : t("canvas.expandStructurePane", "Mở rộng toàn khung Cấu trúc (Thu gọn Live HTML Preview)");

  return (
    <div className="structure-pane-header" style={{ height: "35px", minHeight: "35px", padding: "0 8px", backgroundColor: "var(--bg-surface)", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
      <div className="btn-1click-view-toggle" role="tablist" style={{ display: "inline-flex", alignItems: "center", gap: "4px" }}>
        <button type="button" role="tab" aria-selected={structureLens === "userflow"} data-testid="lens-userflow-btn" onClick={() => onSetLens("userflow")} style={buildLensButtonStyle(structureLens === "userflow")}>
          <BrandIcon name="workflow" size={ICON_SIZE_MD} color="currentColor" />
          <span>{t("canvas.userflowLens", "Userflow")}</span>
        </button>
        <button type="button" role="tab" aria-selected={structureLens === "slide-cms"} data-testid="lens-slide-cms-btn" onClick={() => onSetLens("slide-cms")} style={buildLensButtonStyle(structureLens === "slide-cms")}>
          <BrandIcon name="layers" size={ICON_SIZE_MD} color="currentColor" />
          <span>{t("canvas.slideCmsLens", "Slide Deck / CMS")}</span>
        </button>
      </div>
      <button
        type="button"
        onClick={onToggleRightPreviewHidden}
        title={toggleTitle}
        aria-label={t("canvas.expandStructurePane", "Thu gọn / Mở rộng khung Cấu trúc")}
        className="cowok-btn cowok-btn-ghost"
        style={{ width: "28px", height: "28px", padding: 0, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: isRightPreviewHidden ? "var(--bg-selected)" : "var(--bg-workspace)", color: isRightPreviewHidden ? "var(--text-primary)" : "var(--text-secondary)" }}
      >
        <BrandIcon name={isRightPreviewHidden ? "minimize" : "maximize"} size={ICON_SIZE_LG} color="currentColor" />
      </button>
    </div>
  );
};

export const CompanionStructureLeftPane: React.FC<{
  widthPercent: number;
  isRightPreviewHidden: boolean;
  structureLens: "userflow" | "slide-cms";
  onSetLens: (lens: "userflow" | "slide-cms") => void;
  onToggleRightPreviewHidden: () => void;
  isDraftDirty: boolean;
  conflictState?: ConflictState | null;
  onDeployDraft?: () => void;
  onDiscardDraft?: () => void;
  onResolveConflict?: (choice: "keep_mine" | "take_agent" | "inspect_collisions") => void;
  slides: SlidePhaseItem[];
  activeSlideIndex: number;
  isCmsDrawerOpen: boolean;
  onSelectSlide: (idx: number) => void;
  onCloseCmsDrawer: () => void;
  onUpdateActiveSlide: (patch: Partial<SlidePhaseItem>) => void;
  children: React.ReactNode;
}> = (props) => {
  const { t } = useTranslation();
  const activeSlide = props.slides[props.activeSlideIndex] || props.slides[0];

  return (
    <div
      className="canvas-structure-left-pane"
      aria-label={t("canvas.structurePane", "Khung Cấu trúc Userflow & Slide CMS")}
      style={{
        width: props.isRightPreviewHidden ? "100%" : `${props.widthPercent}%`,
        minWidth: "260px",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        backgroundColor: "var(--bg-canvas)",
        overflow: "hidden",
        position: "relative",
      }}
    >
      <StructurePaneHeader
        structureLens={props.structureLens}
        onSetLens={props.onSetLens}
        isRightPreviewHidden={props.isRightPreviewHidden}
        onToggleRightPreviewHidden={props.onToggleRightPreviewHidden}
      />
      <DraftDeployBanner
        isDraftDirty={props.isDraftDirty}
        conflictState={props.conflictState}
        onDeployDraft={props.onDeployDraft}
        onDiscardDraft={props.onDiscardDraft}
        onResolveConflict={props.onResolveConflict}
      />
      <div style={{ flex: 1, position: "relative", overflow: "hidden" }}>
        {props.structureLens === "userflow" ? (
          props.children
        ) : (
          <div className="slide-deck-cms-studio" style={{ width: "100%", height: "100%", overflowY: "auto", padding: "12px", boxSizing: "border-box", display: "flex", flexDirection: "column", gap: "10px", backgroundColor: "var(--bg-canvas)" }}>
            {!props.isCmsDrawerOpen ? (
              props.slides.map((sld, idx) => (
                <SlideListItemCard key={sld.id} slide={sld} isCurrent={idx === props.activeSlideIndex} onSelect={() => props.onSelectSlide(idx)} />
              ))
            ) : (
              <SlideCmsDrawerEditor activeSlide={activeSlide} onCloseDrawer={props.onCloseCmsDrawer} onUpdateSlide={props.onUpdateActiveSlide} />
            )}
          </div>
        )}
      </div>
    </div>
  );
};
