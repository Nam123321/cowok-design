// @story 04.1, 04.5, 05.1
// @implements AC-01, AC-03 (Semantic 5 Canvas Modes & Customer Walkthrough Controls)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import {
  FIXTURE_VARIANTS,
  normalizeFixtureRef,
  type FixtureVariant,
} from "../../../lib/validations/screen-fixture";

const GROUP_WRAP_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  backgroundColor: "var(--bg-workspace)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-xs)",
  padding: "2px",
  gap: "2px",
  flexWrap: "nowrap",
};

const MODE_BTN_BASE: React.CSSProperties = {
  height: "22px",
  padding: "0 7px",
  border: "none",
  borderRadius: "4px",
  cursor: "pointer",
  display: "inline-flex",
  alignItems: "center",
  gap: "4px",
  fontSize: "10.5px",
  whiteSpace: "nowrap",
  transition: "all var(--duration-fast) var(--ease-standard)",
};

export interface PointerModeGroupProps {
  pointerMode: "interact" | "click-dom" | "drag-box";
  onSetPointerMode: (mode: "interact" | "click-dom" | "drag-box") => void;
  isInlineCodeOpen?: boolean;
  onToggleInlineCode?: () => void;
  isSampleDataOpen?: boolean;
  onToggleSampleData?: () => void;
  isWalkthroughActive?: boolean;
  onToggleWalkthrough?: () => void;
  onOpenComments?: () => void;
}

export const PointerModeGroup: React.FC<PointerModeGroupProps> = ({
  pointerMode,
  onSetPointerMode,
  isInlineCodeOpen,
  onToggleInlineCode,
  isSampleDataOpen,
  onToggleSampleData,
  isWalkthroughActive,
  onToggleWalkthrough,
  onOpenComments,
}) => {
  const { t } = useTranslation();
  const isInteract = pointerMode === "interact" && !isInlineCodeOpen && !isSampleDataOpen;
  const isInspect = pointerMode === "click-dom";
  const isMarquee = pointerMode === "drag-box";

  return (
    <div style={GROUP_WRAP_STYLE} data-testid="canvas-mode-group">
      {/* 1. Interact Mode */}
      <button
        type="button"
        data-testid="pointer-mode-interact"
        aria-pressed={isInteract}
        onClick={() => onSetPointerMode("interact")}
        title={t("canvas.modeInteract", "Interact")}
        style={{
          ...MODE_BTN_BASE,
          backgroundColor: isInteract ? "var(--bg-selected)" : "transparent",
          color: isInteract ? "var(--text-primary)" : "var(--text-secondary)",
          fontWeight: isInteract ? 600 : 500,
        }}
      >
        <BrandIcon name="cursor-click" size={12} strokeWidth={1.85} color="currentColor" />
        <span>{t("canvas.modeInteractShort", "Interact")}</span>
      </button>

      {/* 2. Inspect data-ff-id Mode */}
      <button
        type="button"
        data-testid="pointer-mode-inspect"
        aria-pressed={isInspect}
        onClick={() => onSetPointerMode("click-dom")}
        title={t("canvas.modeClickComponent", "Inspect data-ff-id (C)")}
        style={{
          ...MODE_BTN_BASE,
          backgroundColor: isInspect ? "var(--action-primary)" : "transparent",
          color: isInspect ? "var(--action-primary-fg)" : "var(--text-secondary)",
          fontWeight: isInspect ? 600 : 500,
        }}
      >
        <BrandIcon name="crosshair" size={12} strokeWidth={1.85} color="currentColor" />
        <span>{t("canvas.modeInspectShort", "Inspect data-ff-id")}</span>
      </button>

      {/* 3. Comment Mode */}
      {onOpenComments && (
        <button
          type="button"
          data-testid="pointer-mode-comment"
          onClick={onOpenComments}
          title={t("canvas.modeComment", "Comment")}
          style={{
            ...MODE_BTN_BASE,
            backgroundColor: "transparent",
            color: "var(--text-secondary)",
            fontWeight: 500,
          }}
        >
          <BrandIcon name="message-square" size={12} strokeWidth={1.85} color="currentColor" />
          <span>{t("canvas.modeComment", "Comment")}</span>
        </button>
      )}

      {/* 4. Inline </> Editor Mode */}
      {onToggleInlineCode && (
        <button
          type="button"
          data-testid="canvas-mode-inline-editor"
          aria-pressed={Boolean(isInlineCodeOpen)}
          onClick={onToggleInlineCode}
          title={t("canvas.modeInlineEditor", "Inline </> Editor")}
          style={{
            ...MODE_BTN_BASE,
            backgroundColor: isInlineCodeOpen ? "var(--bg-selected)" : "transparent",
            color: isInlineCodeOpen ? "var(--text-primary)" : "var(--text-secondary)",
            fontWeight: isInlineCodeOpen ? 600 : 500,
          }}
        >
          <BrandIcon name="code" size={12} strokeWidth={1.85} color="currentColor" />
          <span>{t("canvas.modeInlineEditor", "Inline </> Editor")}</span>
        </button>
      )}

      {/* 5. Sample Data Studio Mode */}
      {onToggleSampleData && (
        <button
          type="button"
          data-testid="canvas-mode-sample-data"
          aria-pressed={Boolean(isSampleDataOpen)}
          onClick={onToggleSampleData}
          title={t("canvas.modeSampleData", "Sample Data Studio")}
          style={{
            ...MODE_BTN_BASE,
            backgroundColor: isSampleDataOpen ? "var(--bg-selected)" : "transparent",
            color: isSampleDataOpen ? "var(--text-primary)" : "var(--text-secondary)",
            fontWeight: isSampleDataOpen ? 600 : 500,
          }}
        >
          <BrandIcon name="database" size={12} strokeWidth={1.85} color="currentColor" />
          <span>{t("canvas.modeSampleData", "Sample Data Studio")}</span>
        </button>
      )}

      {/* Marquee Box Selection */}
      <button
        type="button"
        data-testid="pointer-mode-marquee"
        aria-pressed={isMarquee}
        onClick={() => onSetPointerMode("drag-box")}
        title={t("canvas.modeMarqueeBox", "Kéo khung chọn vùng (>=16x16px) vào Chat")}
        aria-label={t("canvas.modeMarqueeBox", "Kéo khung chọn vùng (>=16x16px) vào Chat")}
        style={{
          width: "22px",
          height: "22px",
          border: "none",
          borderRadius: "4px",
          backgroundColor: isMarquee ? "var(--bg-selected)" : "transparent",
          color: isMarquee ? "var(--text-primary)" : "var(--text-secondary)",
          cursor: "pointer",
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        <BrandIcon name="frame" size={12} strokeWidth={1.85} color="currentColor" />
      </button>

      {/* Customer Walkthrough Playback Control */}
      {onToggleWalkthrough && (
        <button
          type="button"
          data-testid="walkthrough-playback-btn"
          aria-pressed={Boolean(isWalkthroughActive)}
          onClick={onToggleWalkthrough}
          title={
            isWalkthroughActive
              ? t("canvas.pauseWalkthrough", "Tạm dừng Customer Walkthrough")
              : t("canvas.startWalkthrough", "▶ Customer Walkthrough")
          }
          style={{
            ...MODE_BTN_BASE,
            backgroundColor: isWalkthroughActive ? "var(--action-primary)" : "var(--bg-subtle)",
            color: isWalkthroughActive ? "var(--action-primary-fg)" : "var(--text-primary)",
            fontWeight: 600,
          }}
        >
          <BrandIcon
            name={isWalkthroughActive ? "pause" : "play"}
            size={11}
            strokeWidth={1.85}
            color="currentColor"
          />
          <span>{isWalkthroughActive ? "Pause" : "▶ Walkthrough"}</span>
        </button>
      )}
    </div>
  );
};

export const SubStateSwitcher: React.FC<{
  activeSubState?: FixtureVariant;
  onSelectSubState?: (variant: FixtureVariant) => void;
}> = ({ activeSubState, onSelectSubState }) => {
  const current = normalizeFixtureRef(activeSubState);
  return (
    <div data-testid="fixture-substate-switcher" style={GROUP_WRAP_STYLE}>
      {FIXTURE_VARIANTS.map(({ variant }) => {
        const isActive = current === variant;
        return (
          <button
            key={variant}
            type="button"
            data-testid={`fixture-variant-${variant}`}
            aria-pressed={isActive}
            onClick={() => onSelectSubState?.(variant)}
            style={{
              height: "20px",
              padding: "0 6px",
              border: "none",
              borderRadius: "4px",
              fontSize: "9.5px",
              fontFamily: "var(--font-mono)",
              fontWeight: isActive ? 700 : 500,
              backgroundColor: isActive ? "var(--bg-selected)" : "transparent",
              color: isActive ? "var(--action-primary)" : "var(--text-muted)",
              cursor: "pointer",
              whiteSpace: "nowrap",
            }}
          >
            {variant}
          </button>
        );
      })}
    </div>
  );
};
