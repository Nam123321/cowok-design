import React, { useState } from "react";
import { apiSdiEngine } from "../../../api/sdi-engine";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { AMBIGUITY_THRESHOLD, DesignIntentResponse } from "../../../lib/validations/sdi";
import { SDIPromptInput } from "./SDIPromptInput";
import { VisualClarificationModal } from "./VisualClarificationModal";

interface IntentStatusCardProps {
  intent: DesignIntentResponse;
  onOpenModal: () => void;
}

export const IntentStatusCard: React.FC<IntentStatusCardProps> = ({ intent, onOpenModal }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const isAmbiguous = intent.ambiguityScore >= AMBIGUITY_THRESHOLD;
  return (
    <div className="p-3 rounded-lg border border-border bg-muted/30 text-xs space-y-2">
      <div className="flex items-center justify-between">
        <span className="font-semibold text-foreground">{t("sdi.ambiguityScoreLabel", "Điểm mơ hồ:")}</span>
        <span
          className={`px-2 py-0.5 rounded text-[10px] font-bold ${
            isAmbiguous
              ? "bg-secondary text-secondary-foreground border border-border"
              : "bg-emerald-500/20 text-emerald-600 dark:text-emerald-400"
          }`}
        >
          {formatNumber(intent.ambiguityScore)} ({intent.status})
        </span>
      </div>
      {isAmbiguous && intent.status === "clarifying" && (
        <button
          type="button"
          onClick={onOpenModal}
          className="w-full py-1 text-xs font-semibold rounded cowok-btn cowok-btn-primary"
        >
          {t("sdi.reviewClarificationsBtn", "Xem & Chọn Phương Án Làm Rõ")}
        </button>
      )}
    </div>
  );
};

interface SDIInspectorTabProps {
  projectId: string;
  targetNodeId?: string;
}

export const SDIInspectorTab: React.FC<SDIInspectorTabProps> = ({
  projectId,
  targetNodeId,
}) => {
  const [intent, setIntent] = useState<DesignIntentResponse | null>(null);
  const [showModal, setShowModal] = useState(false);

  const handleIntentCompiled = (newIntent: DesignIntentResponse) => {
    setIntent(newIntent);
    if (newIntent.ambiguityScore >= AMBIGUITY_THRESHOLD) {
      setShowModal(true);
    }
  };

  const handleConfirmVariant = async (selectedVariantId: string) => {
    if (!intent) return;
    try {
      const updated = await apiSdiEngine.selectClarification(projectId, {
        intentId: intent.id,
        selectedVariantId,
      });
      setIntent(updated);
      setShowModal(false);
    } catch (err) {
      console.error("Failed to select variant:", err);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <SDIPromptInput
        projectId={projectId}
        targetNodeId={targetNodeId}
        onIntentCompiled={handleIntentCompiled}
      />
      {intent && <IntentStatusCard intent={intent} onOpenModal={() => setShowModal(true)} />}
      {intent?.clarificationVariants && (
        <VisualClarificationModal
          isOpen={showModal}
          variants={intent.clarificationVariants}
          onConfirm={handleConfirmVariant}
          onClose={() => setShowModal(false)}
        />
      )}
    </div>
  );
};
