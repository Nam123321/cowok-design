// @story 04.1, 04.5
// @implements AC-01, AC-03 (Semantic Pointer Mode & 4-SubState Switcher Controls)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { FIXTURE_VARIANTS, normalizeFixtureRef, type FixtureVariant } from "../../../lib/validations/screen-fixture";

const GROUP_WRAP_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  backgroundColor: "var(--bg-workspace)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-xs)",
  padding: "2px",
  gap: "2px",
};

export const PointerModeGroup: React.FC<{
  pointerMode: "interact" | "click-dom" | "drag-box";
  onSetPointerMode: (mode: "interact" | "click-dom" | "drag-box") => void;
}> = ({ pointerMode, onSetPointerMode }) => {
  const { t } = useTranslation();
  const isInspect = pointerMode === "click-dom";
  const isMarquee = pointerMode === "drag-box";
  const modeBtnBase: React.CSSProperties = {
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
  };

  return (
    <div style={GROUP_WRAP_STYLE}>
      <button
        type="button"
        data-testid="pointer-mode-interact"
        aria-pressed={pointerMode === "interact"}
        onClick={() => onSetPointerMode("interact")}
        title={t("canvas.modeInteract", "Interact")}
        style={{
          ...modeBtnBase,
          backgroundColor: pointerMode === "interact" ? "var(--bg-selected)" : "transparent",
          color: pointerMode === "interact" ? "var(--text-primary)" : "var(--text-secondary)",
          fontWeight: pointerMode === "interact" ? 600 : 500,
        }}
      >
        <BrandIcon name="cursor-click" size={12} color="currentColor" />
        <span>{t("canvas.modeInteractShort", "Interact")}</span>
      </button>
      <button
        type="button"
        data-testid="pointer-mode-inspect"
        aria-pressed={isInspect}
        onClick={() => onSetPointerMode("click-dom")}
        title={t("canvas.modeClickComponent", "Inspect / Comment (C)")}
        style={{
          ...modeBtnBase,
          backgroundColor: isInspect ? "var(--action-primary)" : "transparent",
          color: isInspect ? "var(--action-primary-fg)" : "var(--text-secondary)",
          fontWeight: isInspect ? 600 : 500,
        }}
      >
        <BrandIcon name="crosshair" size={12} color="currentColor" />
        <span>{t("canvas.modeInspectShort", "Inspect / Comment (C)")}</span>
      </button>
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
        <BrandIcon name="frame" size={12} color="currentColor" />
      </button>
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
