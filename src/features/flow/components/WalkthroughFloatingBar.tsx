// @story 05.1, 06.1
// @implements AC-03, AC-06 (Shared Walkthrough & Presentation Floating Pill Controller)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";

const WALKTHROUGH_BAR_Z_INDEX = 25;
const WALKTHROUGH_ICON_SIZE = 11;
const FONT_WEIGHT_SEMIBOLD = 600;

export interface WalkthroughFloatingBarProps {
  stepLabel: string;
  autoPlay?: boolean;
  viewMode?: "dual" | "full";
  showViewModeToggle?: boolean;
  onPrev?: () => void;
  onToggleAuto?: () => void;
  onNext?: () => void;
  onSetViewMode?: (mode: "dual" | "full") => void;
  onExit?: () => void;
}

const CIRCLE_NAV_BTN_STYLE: React.CSSProperties = {
  width: "24px",
  height: "24px",
  borderRadius: "50%",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  cursor: "pointer",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
};

const ViewModeSwitchGroup: React.FC<{
  viewMode?: "dual" | "full";
  onSetViewMode?: (mode: "dual" | "full") => void;
}> = ({ viewMode, onSetViewMode }) => {
  const { t } = useTranslation();
  const mode = viewMode ?? "dual";
  const btnStyle = (active: boolean): React.CSSProperties => ({
    height: "22px",
    padding: "0 8px",
    borderRadius: "var(--radius-pill)",
    border: "none",
    fontSize: "10.5px",
    fontWeight: FONT_WEIGHT_SEMIBOLD,
    cursor: "pointer",
    backgroundColor: active ? "var(--bg-selected)" : "transparent",
    color: active ? "var(--action-primary)" : "var(--text-secondary)",
  });
  return (
    <div style={{ display: "inline-flex", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-pill)", padding: "2px", gap: "2px" }}>
      <button type="button" data-testid="walkthrough-mode-dual" onClick={() => onSetViewMode?.("dual")} style={btnStyle(mode === "dual")}>
        {t("canvas.walkthroughDualMode", "Dual Screen")}
      </button>
      <button type="button" data-testid="walkthrough-mode-full" onClick={() => onSetViewMode?.("full")} style={btnStyle(mode === "full")}>
        {t("canvas.walkthroughFullMode", "Toàn bộ thiết kế")}
      </button>
    </div>
  );
};

export const WalkthroughFloatingBar: React.FC<WalkthroughFloatingBarProps> = ({
  stepLabel,
  autoPlay,
  viewMode,
  showViewModeToggle = true,
  onPrev,
  onToggleAuto,
  onNext,
  onSetViewMode,
  onExit,
}) => {
  const { t } = useTranslation();
  return (
    <div
      data-testid="walkthrough-floating-bar"
      style={{
        position: "absolute",
        bottom: "14px",
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: WALKTHROUGH_BAR_Z_INDEX,
        display: "inline-flex",
        alignItems: "center",
        gap: "6px",
        padding: "5px 10px",
        backgroundColor: "var(--bg-surface)",
        border: "1px solid var(--border-default)",
        borderRadius: "var(--radius-pill)",
        boxShadow: "var(--shadow-lg)",
        whiteSpace: "nowrap",
      }}
    >
      <span style={{ fontFamily: "var(--font-mono)", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--action-primary)" }}>
        {stepLabel}
      </span>
      <button type="button" data-testid="walkthrough-prev-btn" onClick={onPrev} title={t("canvas.walkthroughPrev", "Màn hình trước")} style={CIRCLE_NAV_BTN_STYLE}>
        <BrandIcon name="arrow-left" size={WALKTHROUGH_ICON_SIZE} color="currentColor" />
      </button>
      <button
        type="button"
        data-testid="walkthrough-auto-toggle"
        onClick={onToggleAuto}
        title={autoPlay ? t("canvas.pauseWalkthrough", "Tạm dừng tự động") : t("canvas.resumeWalkthrough", "Chạy tự động")}
        style={{
          width: "26px",
          height: "26px",
          borderRadius: "50%",
          border: "none",
          backgroundColor: "var(--action-primary)",
          color: "var(--action-primary-fg)",
          cursor: "pointer",
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        <BrandIcon name={autoPlay ? "pause" : "play"} size={WALKTHROUGH_ICON_SIZE} color="currentColor" />
      </button>
      <button type="button" data-testid="walkthrough-next-btn" onClick={onNext} title={t("canvas.walkthroughNext", "Màn hình tiếp theo")} style={CIRCLE_NAV_BTN_STYLE}>
        <BrandIcon name="chevron-right" size={WALKTHROUGH_ICON_SIZE} color="currentColor" />
      </button>
      {showViewModeToggle && <ViewModeSwitchGroup viewMode={viewMode} onSetViewMode={onSetViewMode} />}
      <button
        type="button"
        data-testid="walkthrough-exit-btn"
        onClick={onExit}
        title={t("canvas.exitWalkthrough", "Thoát Walkthrough (Esc)")}
        style={{ ...CIRCLE_NAV_BTN_STYLE, backgroundColor: "transparent", color: "var(--text-secondary)" }}
      >
        <BrandIcon name="close" size={WALKTHROUGH_ICON_SIZE} color="currentColor" />
      </button>
    </div>
  );
};
