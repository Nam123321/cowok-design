// @story 05.1
// @implements Zero-Tailwind Semantic Pure CSS SDI Inspector Tab
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { apiSdiEngine } from "../../../api/sdi-engine";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { AMBIGUITY_THRESHOLD, DesignIntentResponse } from "../../../lib/validations/sdi";
import { SDIPromptInput } from "./SDIPromptInput";
import { VisualClarificationModal } from "./VisualClarificationModal";

const ICON_SIZE_SM = 12;
const STROKE_WIDTH_MONO = 1.85;
const FONT_WEIGHT_BOLD = 700;
const FONT_WEIGHT_SEMIBOLD = 600;

const CARD_STYLE: React.CSSProperties = {
  padding: "12px",
  borderRadius: "var(--radius-md)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  fontSize: "12px",
  display: "flex",
  flexDirection: "column",
  gap: "8px",
};

const ACTION_BTN_STYLE: React.CSSProperties = {
  width: "100%",
  padding: "6px 12px",
  fontSize: "11.5px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  borderRadius: "var(--radius-xs)",
  border: "none",
  backgroundColor: "var(--action-primary)",
  color: "var(--action-primary-fg)",
  cursor: "pointer",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: "5px",
};

interface IntentStatusCardProps {
  intent: DesignIntentResponse;
  onOpenModal: () => void;
}

export const IntentStatusCard: React.FC<IntentStatusCardProps> = ({ intent, onOpenModal }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const isAmbiguous = intent.ambiguityScore >= AMBIGUITY_THRESHOLD;
  const scoreDisplay = typeof intent.ambiguityScore === "number" ? formatNumber(intent.ambiguityScore, 2) : String(intent.ambiguityScore);

  return (
    <div data-testid="intent-status-card" style={CARD_STYLE}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <span style={{ fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
          {t("sdi.ambiguityScoreLabel", "Điểm mơ hồ:")}
        </span>
        <span
          data-testid="intent-ambiguity-badge"
          style={{
            padding: "2px 8px",
            borderRadius: "var(--radius-xs)",
            fontSize: "10.5px",
            fontFamily: "var(--font-mono)",
            fontWeight: FONT_WEIGHT_BOLD,
            border: isAmbiguous ? "1px solid var(--state-warning)" : "1px solid var(--state-success)",
            backgroundColor: isAmbiguous ? "var(--state-warning-soft)" : "var(--state-success-soft)",
            color: isAmbiguous ? "var(--state-warning)" : "var(--state-success)",
          }}
        >
          {scoreDisplay} ({intent.status})
        </span>
      </div>
      {isAmbiguous && intent.status === "clarifying" && (
        <button type="button" data-testid="open-clarifications-btn" onClick={onOpenModal} style={ACTION_BTN_STYLE}>
          <BrandIcon name="help-circle" size={ICON_SIZE_SM} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
          <span>{t("sdi.reviewClarificationsBtn", "Xem & Chọn Phương Án Làm Rõ")}</span>
        </button>
      )}
    </div>
  );
};

interface SDIInspectorTabProps {
  projectId: string;
  targetNodeId?: string;
}

export const SDIInspectorTab: React.FC<SDIInspectorTabProps> = ({ projectId, targetNodeId }) => {
  const [intent, setIntent] = useState<DesignIntentResponse | null>(null);
  const [showModal, setShowModal] = useState(false);
  const [selectionError, setSelectionError] = useState<string | null>(null);

  const handleIntentCompiled = (newIntent: DesignIntentResponse) => {
    setIntent(newIntent);
    setSelectionError(null);
    if (newIntent.ambiguityScore >= AMBIGUITY_THRESHOLD) setShowModal(true);
  };

  const handleConfirmVariant = async (selectedVariantId: string) => {
    if (!intent) return;
    try {
      const updated = await apiSdiEngine.selectClarification(projectId, { intentId: intent.id, selectedVariantId });
      setIntent(updated);
      setShowModal(false);
    } catch (err) {
      setSelectionError(err instanceof Error ? err.message : "Failed to select clarification variant.");
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
      <SDIPromptInput projectId={projectId} targetNodeId={targetNodeId} onIntentCompiled={handleIntentCompiled} />
      {selectionError && <div role="alert" style={{ fontSize: "11px", color: "var(--state-danger)" }}>{selectionError}</div>}
      {intent && <IntentStatusCard intent={intent} onOpenModal={() => setShowModal(true)} />}
      {intent?.clarificationVariants && (
        <VisualClarificationModal key={intent.id} isOpen={showModal} variants={intent.clarificationVariants} onConfirm={handleConfirmVariant} onClose={() => setShowModal(false)} />
      )}
    </div>
  );
};
