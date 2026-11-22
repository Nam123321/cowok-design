// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
import React, { useEffect, useId, useRef, useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import { CONTEXT_LIMITS } from "../../lib/validations/context-errors";
import { ContextMeterPopover } from "./ContextMeterPopover";
import { useContextMeter } from "./useContextMeter";

const TRIGGER_ICON_SIZE_PX = 11;

function getTriggerStyle(hasError: boolean): React.CSSProperties {
  return {
    display: "inline-flex",
    alignItems: "center",
    gap: "6px",
    height: "26px",
    padding: "0 8px",
    fontSize: "11px",
    fontFamily: "var(--font-mono)",
    fontVariantNumeric: "tabular-nums",
    color: hasError ? "var(--state-danger)" : "var(--text-secondary)",
    backgroundColor: hasError ? "var(--state-danger-soft)" : "var(--bg-workspace)",
    border: "1px solid var(--border-subtle)",
    borderRadius: "var(--radius-xs)",
    cursor: "pointer",
  };
}

export interface ContextMeterWidgetProps {
  projectId?: string;
}

function useDismissPopover(
  isOpen: boolean,
  containerRef: React.RefObject<HTMLDivElement | null>,
  onClose: () => void
): void {
  useEffect(() => {
    if (!isOpen) return undefined;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    const onPointerDown = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    document.addEventListener("mousedown", onPointerDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("mousedown", onPointerDown);
    };
  }, [containerRef, isOpen, onClose]);
}

export const ContextMeterWidget: React.FC<ContextMeterWidgetProps> = ({ projectId }) => {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const popoverId = useId();
  const meter = useContextMeter(projectId);
  const textUsed = meter.snapshot?.textTotalTokens ?? 0;
  const triggerLabel = t("contextMeter.triggerLabel", "Context Budget Meter");
  useDismissPopover(isOpen, containerRef, () => setIsOpen(false));

  return (
    <div ref={containerRef} data-ff-id="context-meter-widget" aria-label={triggerLabel} style={{ position: "relative", display: "inline-flex", alignItems: "center" }}>
      <button
        type="button" data-ff-id="context-meter-trigger" data-testid="context-meter-trigger"
        aria-expanded={isOpen} aria-controls={popoverId} aria-label={triggerLabel}
        onClick={() => setIsOpen((prev) => !prev)} style={getTriggerStyle(meter.status === "error")}
      >
        <BrandIcon name="layers" size={TRIGGER_ICON_SIZE_PX} color="currentColor" />
        <span>{textUsed}/{CONTEXT_LIMITS.MAX_TEXT_BPE_TOKENS}</span>
        <span style={{ color: "var(--text-muted)" }}>{meter.modelProfile}</span>
      </button>

      {isOpen && (
        <ContextMeterPopover
          popoverId={popoverId} status={meter.status} portalId={meter.portalId}
          horizon={meter.horizon} modelProfile={meter.modelProfile} snapshot={meter.snapshot}
          error={meter.error} hasValidProject={meter.hasValidProject}
          onPortalChange={meter.setPortalId} onHorizonChange={meter.setHorizon}
          onProfileChange={meter.setModelProfile} onAssemble={() => void meter.assembleNow()}
        />
      )}
    </div>
  );
};
