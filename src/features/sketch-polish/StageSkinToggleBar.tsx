// @story 05.4
// @implements AC-01, AC-02 (Cognitive Progressive Phase Stepper + <16ms Designer Wireframe Lens + OKLCH AA Guard)
import React, { useEffect, useRef, useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";

export type StageSkinMode = "sketch-paper" | "brand-color";
export type CognitivePhase = "sketch" | "style" | "polish" | "approved";

const ICON_SIZE_SM = 13;
const ICON_SIZE_XS = 11;
const STROKE_WIDTH_MONO = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;
const PHASE_MENU_Z_INDEX = 45;

const PHASE_ORDER: readonly CognitivePhase[] = ["sketch", "style", "polish", "approved"];

export interface StageSkinToggleBarProps {
  skin: StageSkinMode;
  cognitivePhase?: CognitivePhase;
  onChangeSkin: (nextSkin: StageSkinMode) => void;
  onSelectPhase?: (phase: CognitivePhase) => void;
  onAdvanceToStyle?: () => void;
  onApproveV1?: () => void;
  onToggleSketchLens?: () => void;
  hasExistingBrandTokens?: boolean;
  oklchWarning?: string | null;
  lastSwapMs?: number | null;
  onOpenClarify?: () => void;
}

function getPhaseLabel(
  phase: CognitivePhase,
  t: (key: string, fallback: string) => string
): string {
  switch (phase) {
    case "sketch":
      return t("sketchPolish.phaseSketch", "1. Phác thảo");
    case "style":
      return t("sketchPolish.phaseStyle", "2. Diện mạo");
    case "polish":
      return t("sketchPolish.phasePolish", "3. Hoàn thiện");
    case "approved":
      return t("sketchPolish.phaseApproved", "4. Duyệt v1");
  }
}

function getPhaseStepClass(isActive: boolean, isDone: boolean): string {
  if (isActive) return "sp-phase-step sp-phase-step--active";
  if (isDone) return "sp-phase-step sp-phase-step--done";
  return "sp-phase-step";
}

const PHASE_MENU_STYLE: React.CSSProperties = {
  position: "absolute",
  top: "calc(100% + 4px)",
  left: 0,
  minWidth: "160px",
  padding: "4px",
  borderRadius: "var(--radius-sm)",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  boxShadow: "var(--shadow-lg)",
  zIndex: PHASE_MENU_Z_INDEX,
  display: "flex",
  flexDirection: "column",
  gap: "2px",
};

const PHASE_DROPDOWN_BTN_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
  height: "24px",
  padding: "0 8px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-surface)",
  color: "var(--text-primary)",
  fontSize: "11px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  whiteSpace: "nowrap",
  cursor: "pointer",
};

function useDismissMenu(
  isOpen: boolean,
  ref: React.RefObject<HTMLDivElement | null>,
  onClose: () => void
): void {
  useEffect(() => {
    if (!isOpen) return;
    const onPointerDown = (evt: MouseEvent) => {
      if (ref.current && !ref.current.contains(evt.target as Node)) onClose();
    };
    const onKeyDown = (evt: KeyboardEvent) => {
      if (evt.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [isOpen, ref, onClose]);
}

const PhaseDropdownMenu: React.FC<{
  phase: CognitivePhase;
  onChoose: (next: CognitivePhase) => void;
}> = ({ phase, onChoose }) => {
  const { t } = useTranslation();
  const activeIdx = PHASE_ORDER.indexOf(phase);
  return (
    <div role="menu" data-testid="cognitive-phase-menu" style={PHASE_MENU_STYLE}>
      {PHASE_ORDER.map((step, idx) => {
        const isActive = step === phase;
        const isDone = idx < activeIdx;
        return (
          <button
            key={step}
            type="button"
            role="menuitem"
            className={getPhaseStepClass(isActive, isDone)}
            aria-current={isActive ? "step" : undefined}
            data-testid={`phase-step-${step}`}
            onClick={() => onChoose(step)}
            style={{ width: "100%", justifyContent: "flex-start", whiteSpace: "nowrap" }}
          >
            {isDone && <BrandIcon name="check" size={ICON_SIZE_SM} strokeWidth={STROKE_WIDTH_MONO} />}
            <span>{getPhaseLabel(step, t)}</span>
          </button>
        );
      })}
    </div>
  );
};

const CognitivePhaseStepper: React.FC<{
  phase: CognitivePhase;
  onSelectPhase?: (next: CognitivePhase) => void;
}> = ({ phase, onSelectPhase }) => {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement | null>(null);
  useDismissMenu(isOpen, containerRef, () => setIsOpen(false));

  return (
    <div
      ref={containerRef}
      role="group"
      aria-label={t("sketchPolish.phaseStepperLabel", "Tiến trình thiết kế")}
      data-testid="cognitive-phase-stepper"
      style={{ position: "relative", display: "inline-flex", alignItems: "center", flexShrink: 0 }}
    >
      <button
        type="button"
        aria-haspopup="menu"
        aria-expanded={isOpen}
        data-testid="cognitive-phase-dropdown"
        onClick={() => setIsOpen((prev) => !prev)}
        style={PHASE_DROPDOWN_BTN_STYLE}
      >
        <span>{getPhaseLabel(phase, t)}</span>
        <BrandIcon name="chevron-down" size={ICON_SIZE_XS} strokeWidth={STROKE_WIDTH_MONO} color="var(--text-secondary)" />
      </button>
      {isOpen && (
        <PhaseDropdownMenu
          phase={phase}
          onChoose={(step) => { onSelectPhase?.(step); setIsOpen(false); }}
        />
      )}
    </div>
  );
};

const PolishPhaseActions: React.FC<{
  phase: "polish" | "approved";
  isSketchLens: boolean;
  onOpenClarify?: () => void;
  onApproveV1?: () => void;
  onToggleSketchLens?: () => void;
}> = ({ phase, isSketchLens, onOpenClarify, onApproveV1, onToggleSketchLens }) => {
  const { t } = useTranslation();
  return (
    <>
      {onToggleSketchLens && (
        <button
          type="button"
          className={`sp-chip-btn${isSketchLens ? " sp-chip-btn--active" : ""}`}
          aria-pressed={isSketchLens}
          data-testid="toggle-sketch-lens-btn"
          onClick={onToggleSketchLens}
        >
          <BrandIcon name="file-text" size={ICON_SIZE_SM} strokeWidth={STROKE_WIDTH_MONO} />
          <span>{t("sketchPolish.sketchLensBtn", "Soi nét phác")}</span>
        </button>
      )}
      {phase === "polish" && onOpenClarify && (
        <button type="button" className="sp-chip-btn" data-testid="open-live-direction-btn" onClick={onOpenClarify}>
          <BrandIcon name="brand-agent" size={ICON_SIZE_SM} strokeWidth={STROKE_WIDTH_MONO} />
          <span>{t("sketchPolish.changeStyleBtn", "Đổi diện mạo")}</span>
        </button>
      )}
      {phase === "polish" && onApproveV1 && (
        <button type="button" className="sp-phase-cta" data-testid="approve-v1-btn" onClick={onApproveV1}>
          <BrandIcon name="check-circle" size={ICON_SIZE_SM} strokeWidth={STROKE_WIDTH_MONO} />
          <span>{t("sketchPolish.approveV1Btn", "Duyệt bản v1")}</span>
        </button>
      )}
      {phase === "approved" && (
        <span className="sp-approved-badge" data-testid="approved-v1-badge">
          <BrandIcon name="check-circle" size={ICON_SIZE_SM} strokeWidth={STROKE_WIDTH_MONO} />
          <span>{t("sketchPolish.approvedV1Badge", "Đã duyệt bản v1")}</span>
        </span>
      )}
    </>
  );
};

const PhaseActionControls: React.FC<{
  phase: CognitivePhase;
  isSketchLens: boolean;
  onAdvanceToStyle?: () => void;
  onOpenClarify?: () => void;
  onApproveV1?: () => void;
  onToggleSketchLens?: () => void;
}> = ({ phase, isSketchLens, onAdvanceToStyle, onOpenClarify, onApproveV1, onToggleSketchLens }) => {
  const { t } = useTranslation();
  if (phase === "sketch") {
    return (
      <button type="button" className="sp-phase-cta" data-testid="promote-sketch-to-style-btn" onClick={onAdvanceToStyle ?? onOpenClarify}>
        <BrandIcon name="sparkles" size={ICON_SIZE_SM} strokeWidth={STROKE_WIDTH_MONO} />
        <span>{t("sketchPolish.promoteToStyleBtn", "Chốt phác thảo → Chọn diện mạo")}</span>
      </button>
    );
  }
  if (phase === "style") {
    return (
      <button type="button" className="sp-phase-cta" data-testid="open-live-direction-btn" onClick={onOpenClarify ?? onAdvanceToStyle}>
        <BrandIcon name="brand-agent" size={ICON_SIZE_SM} strokeWidth={STROKE_WIDTH_MONO} />
        <span>{t("sketchPolish.chooseStyleBtn", "Chọn diện mạo & màu sắc")}</span>
      </button>
    );
  }
  return (
    <PolishPhaseActions
      phase={phase}
      isSketchLens={isSketchLens}
      onOpenClarify={onOpenClarify}
      onApproveV1={onApproveV1}
      onToggleSketchLens={onToggleSketchLens}
    />
  );
};

const StageSkinStatusBadges: React.FC<{
  showBrandBypass: boolean;
  oklchWarning: string | null;
  lastSwapMs: number | null;
}> = ({ showBrandBypass, oklchWarning, lastSwapMs }) => {
  const { t } = useTranslation();
  return (
    <>
      {showBrandBypass && (
        <span className="sp-chip-btn" data-testid="brand-bypass-badge">
          <BrandIcon name="check-circle" size={ICON_SIZE_SM} strokeWidth={STROKE_WIDTH_MONO} />
          <span>{t("sketchPolish.brandDetected", "Đã nhận diện màu thương hiệu")}</span>
        </span>
      )}
      {oklchWarning && (
        <span role="status" className="sp-oklch-warn" data-testid="oklch-contrast-warning">
          <BrandIcon name="shield-alert" size={ICON_SIZE_SM} strokeWidth={STROKE_WIDTH_MONO} />
          <span>{oklchWarning}</span>
        </span>
      )}
      {typeof lastSwapMs === "number" && (
        <span
          data-testid="skin-swap-latency"
          aria-label="skin-swap-latency"
          title={`${lastSwapMs.toFixed(1)}ms`}
          style={{ fontSize: "10.5px", color: "var(--text-muted)", padding: "0 4px" }}
        >
          {`${lastSwapMs.toFixed(1)}ms`}
        </span>
      )}
    </>
  );
};

export const StageSkinToggleBar: React.FC<StageSkinToggleBarProps> = ({
  skin,
  cognitivePhase,
  onChangeSkin,
  onSelectPhase,
  onAdvanceToStyle,
  onApproveV1,
  onToggleSketchLens,
  hasExistingBrandTokens = false,
  oklchWarning = null,
  lastSwapMs = null,
  onOpenClarify,
}) => {
  const isSketch = skin === "sketch-paper";
  const effectivePhase: CognitivePhase = cognitivePhase ?? (isSketch ? "sketch" : "polish");
  const handleToggleLens = onToggleSketchLens ?? (() => onChangeSkin(isSketch ? "brand-color" : "sketch-paper"));

  return (
    <div
      className="sp-toolbar-group"
      data-ff-id="stage-skin-toggle"
      data-testid="stage-skin-toggle"
      style={{ display: "inline-flex", flexDirection: "row", flexWrap: "nowrap", alignItems: "center", gap: "6px", whiteSpace: "nowrap", minWidth: 0 }}
    >
      <CognitivePhaseStepper phase={effectivePhase} onSelectPhase={onSelectPhase} />
      <PhaseActionControls
        phase={effectivePhase}
        isSketchLens={isSketch}
        onAdvanceToStyle={onAdvanceToStyle}
        onOpenClarify={onOpenClarify}
        onApproveV1={onApproveV1}
        onToggleSketchLens={effectivePhase === "polish" || effectivePhase === "approved" ? handleToggleLens : undefined}
      />
      <StageSkinStatusBadges
        showBrandBypass={hasExistingBrandTokens && !isSketch}
        oklchWarning={oklchWarning}
        lastSwapMs={lastSwapMs}
      />
    </div>
  );
};
