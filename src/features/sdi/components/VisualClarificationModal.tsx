// @story 05.1
// @implements Zero-Tailwind Semantic Pure CSS Visual Clarification Modal
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { AMBIGUITY_THRESHOLD, ClarificationVariant } from "../../../lib/validations/sdi";
import { ClarificationCard } from "./ClarificationCard";

const MODAL_Z_INDEX = 50;
const DISABLED_OPACITY = 0.5;
const ICON_HEADER_SIZE = 16;
const ICON_CHECK_SIZE = 12;
const STROKE_WIDTH_MONO = 1.85;
const FONT_WEIGHT_BOLD = 700;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_MEDIUM = 500;

const MODAL_OVERLAY_STYLE: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  zIndex: MODAL_Z_INDEX,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  backgroundColor: "var(--bg-overlay)",
  padding: "16px",
};

const MODAL_CARD_STYLE: React.CSSProperties = {
  width: "100%",
  maxWidth: "680px",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-lg)",
  padding: "20px",
  boxShadow: "var(--shadow-lg)",
  display: "flex",
  flexDirection: "column",
  gap: "16px",
};

interface VisualClarificationModalProps {
  isOpen: boolean;
  variants: ClarificationVariant[];
  onConfirm: (selectedVariantId: string) => void;
  onClose: () => void;
}

const ModalHeader: React.FC = () => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <div>
      <h2 style={{ margin: 0, fontSize: "15px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)", display: "flex", alignItems: "center", gap: "8px" }}>
        <BrandIcon name="help-circle" size={ICON_HEADER_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="var(--action-primary)" />
        <span>{t("sdi.clarificationTitle", "Làm Rõ Ý Định Thiết Kế (Visual Clarification)")}</span>
      </h2>
      <p style={{ margin: "4px 0 0", fontSize: "11px", color: "var(--text-muted)" }}>
        {t("sdi.clarificationDescPrefix", "Yêu cầu có độ mơ hồ cao (≥")} {formatNumber(AMBIGUITY_THRESHOLD)}
        {t("sdi.clarificationDescSuffix", "). Vui lòng chọn 1 phương án định hướng trước khi sinh mã.")}
      </p>
    </div>
  );
};

interface ModalFooterProps {
  selectedId: string | null;
  onCancel: () => void;
  onConfirm: () => void;
}

const ModalFooter: React.FC<ModalFooterProps> = ({ selectedId, onCancel, onConfirm }) => {
  const { t } = useTranslation();
  const isDisabled = !selectedId;
  return (
    <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", paddingTop: "12px", borderTop: "1px solid var(--border-default)" }}>
      <button type="button" onClick={onCancel} style={{ padding: "6px 14px", fontSize: "12px", fontWeight: FONT_WEIGHT_MEDIUM, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "transparent", color: "var(--text-secondary)", cursor: "pointer" }}>
        {t("common.cancel", "Hủy Bỏ")}
      </button>
      <button type="button" disabled={isDisabled} onClick={onConfirm} style={{ padding: "6px 16px", fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, borderRadius: "var(--radius-xs)", border: "none", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", cursor: isDisabled ? "not-allowed" : "pointer", opacity: isDisabled ? DISABLED_OPACITY : 1, display: "inline-flex", alignItems: "center", gap: "5px" }}>
        <BrandIcon name="check" size={ICON_CHECK_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
        <span>{t("sdi.confirmContinueBtn", "Xác Nhận & Tiếp Tục")}</span>
      </button>
    </div>
  );
};

export const VisualClarificationModal: React.FC<VisualClarificationModalProps> = ({ isOpen, variants, onConfirm, onClose }) => {
  const [selectedId, setSelectedId] = useState<string | null>(null);

  if (!isOpen) return null;
  return (
    <div role="dialog" aria-modal="true" data-testid="visual-clarification-modal" style={MODAL_OVERLAY_STYLE}>
      <div style={MODAL_CARD_STYLE}>
        <ModalHeader />
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: "10px" }}>
          {variants.map((v) => (
            <ClarificationCard key={v.id} variant={v} isSelected={selectedId === v.id} onSelect={setSelectedId} />
          ))}
        </div>
        <ModalFooter selectedId={selectedId} onCancel={() => { setSelectedId(null); onClose(); }} onConfirm={() => selectedId && onConfirm(selectedId)} />
      </div>
    </div>
  );
};
