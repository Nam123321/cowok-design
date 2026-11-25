// @story 10.3
// @implements AC-02 (Visual Cross-Variant Element Borrow Form with Multi-Block Picker & Sub-Chat Shortcut)
import React, { useEffect, useMemo, useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { GeneratedVariant } from "../../lib/validations/tournament";
import {
  CrossVariantZonePickerModal,
  type DonorVisualZone,
  extractDonorVisualZones,
  getZoneLabelFallback,
} from "./CrossVariantZonePickerModal";

const FONT_SEMIBOLD = 600;
const CHECK_ICON_SIZE = 13;

export interface CrossVariantBorrowFormProps {
  activeVariant: GeneratedVariant;
  donorVariants: ReadonlyArray<GeneratedVariant>;
  isCommitting: boolean;
  onConfirm: (payload: {
    borrowFromVariantId?: string;
    borrowSelectorOrInstruction?: string;
  }) => void;
  onFocusSubChat?: () => void;
}

const FIELD_STYLE: React.CSSProperties = {
  padding: "8px 10px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-card)",
  color: "var(--text-primary)",
  fontSize: "12.5px",
};

const FORM_STYLE: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "10px",
  padding: "14px",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-md)",
};

const CONFIRM_BTN_BASE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  flex: "1 0 auto",
  gap: "6px",
  padding: "8px 14px",
  borderRadius: "var(--radius-xs)",
  border: "none",
  backgroundColor: "var(--action-primary)",
  color: "var(--text-on-action)",
  fontSize: "12.5px",
  fontWeight: FONT_SEMIBOLD,
  whiteSpace: "nowrap",
};

const PICKER_BTN_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: "6px",
  padding: "8px 11px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--action-primary)",
  backgroundColor: "var(--bg-selected)",
  color: "var(--action-primary)",
  fontSize: "12px",
  fontWeight: FONT_SEMIBOLD,
  cursor: "pointer",
  whiteSpace: "nowrap",
};

const SelectedZoneChipsRow: React.FC<{
  selectedFfIds: ReadonlyArray<string>;
  donorZones: ReadonlyArray<DonorVisualZone>;
  onRemove: (ffId: string) => void;
}> = ({ selectedFfIds, donorZones, onRemove }) => {
  const { t } = useTranslation();
  if (selectedFfIds.length === 0) return null;

  return (
    <div data-testid="cross-variant-selected-chips" style={{ display: "flex", flexWrap: "wrap", gap: "6px" }}>
      {selectedFfIds.map((ffId) => {
        const zone = donorZones.find((z) => z.ffId === ffId);
        const label = zone ? t(zone.labelKey, getZoneLabelFallback(zone.labelKey)) : ffId;
        return (
          <span
            key={ffId}
            data-testid={`cross-variant-selected-chip-${ffId}`}
            style={{
              display: "inline-flex", alignItems: "center", gap: "5px", padding: "3px 8px",
              borderRadius: "999px", backgroundColor: "var(--bg-selected)", border: "1px solid var(--border-default)",
              color: "var(--action-primary)", fontSize: "11px", fontWeight: FONT_SEMIBOLD,
            }}
          >
            <span>{label}</span>
            <button
              type="button"
              data-testid={`cross-variant-remove-chip-${ffId}`}
              aria-label={t("tournament.removeSelectedZoneAria", "Bỏ chọn khối {{label}}", { label })}
              onClick={() => onRemove(ffId)}
              style={{ border: "none", background: "transparent", color: "currentColor", cursor: "pointer", padding: 0, lineHeight: 1, fontSize: "13px" }}
            >
              ×
            </button>
          </span>
        );
      })}
    </div>
  );
};

const BorrowControlsRow: React.FC<{
  donorId: string;
  selectedCount: number;
  donorVariants: ReadonlyArray<GeneratedVariant>;
  isCommitting: boolean;
  onDonorChange: (id: string) => void;
  onOpenZonePicker: () => void;
}> = ({ donorId, selectedCount, donorVariants, isCommitting, onDonorChange, onOpenZonePicker }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const pickerLabel = selectedCount > 0
    ? t("tournament.openZonePickerWithCount", "Chọn khối trực quan ({{count}})", { count: formatNumber(selectedCount) })
    : t("tournament.openZonePickerBtn", "Chọn khối trực quan");
  return (
    <div style={{ display: "flex", flexWrap: "wrap", gap: "8px", alignItems: "center" }}>
      <select
        aria-label={t("tournament.borrowSourceAria", "Chọn phương án nguồn để mượn")}
        data-testid="cross-variant-donor-select"
        value={donorId}
        onChange={(e) => onDonorChange(e.target.value)}
        style={{ ...FIELD_STYLE, flex: "1 1 140px", minWidth: 0 }}
      >
        <option value="">{t("tournament.borrowNone", "Không mượn thêm")}</option>
        {donorVariants.map((v) => (
          <option key={v.id} value={v.id}>
            {t("tournament.borrowFromOption", "Mượn từ {{label}}", { label: v.displayLabel })}
          </option>
        ))}
      </select>
      {donorId && (
        <button type="button" data-testid="cross-variant-open-zone-picker-btn" onClick={onOpenZonePicker} style={PICKER_BTN_STYLE}>
          <BrandIcon name="layers" size={CHECK_ICON_SIZE} color="currentColor" />
          <span>{pickerLabel}</span>
        </button>
      )}
      <button type="submit" disabled={isCommitting} data-testid="tournament-confirm-winner-btn" style={{ ...CONFIRM_BTN_BASE, cursor: isCommitting ? "wait" : "pointer" }}>
        <BrandIcon name="check" size={CHECK_ICON_SIZE} color="currentColor" />
        <span>{isCommitting ? t("tournament.committingBtn", "Đang áp dụng...") : t("tournament.confirmWinnerBtn", "Chốt phương án này")}</span>
      </button>
    </div>
  );
};

const SubChatSuggestionStrip: React.FC<{ onFocusSubChat?: () => void }> = ({ onFocusSubChat }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", flexWrap: "wrap", paddingTop: "6px", borderTop: "1px solid var(--border-subtle)", fontSize: "11.5px" }}>
      <span style={{ color: "var(--text-muted)" }}>{t("tournament.subChatSuggestPrompt", "Muốn chỉnh sửa tự do hơn?")}</span>
      <button
        type="button"
        data-testid="cross-variant-chat-suggest-btn"
        onClick={() => onFocusSubChat?.()}
        style={{ border: "none", background: "transparent", padding: 0, color: "var(--action-primary)", fontSize: "11.5px", fontWeight: FONT_SEMIBOLD, cursor: "pointer" }}
      >
        {t("tournament.subChatSuggestAction", "Nhắn Agent ở Sub-Chat ↓")}
      </button>
    </div>
  );
};

function useBorrowFormState(activeVariantId: string, donorVariants: ReadonlyArray<GeneratedVariant>) {
  const [donorId, setDonorId] = useState<string>("");
  const [selectedFfIds, setSelectedFfIds] = useState<string[]>([]);
  const [isPickerOpen, setIsPickerOpen] = useState<boolean>(false);

  const selectedDonor = useMemo(() => donorVariants.find((v) => v.id === donorId) ?? null, [donorId, donorVariants]);
  const donorZones = useMemo(() => (selectedDonor ? extractDonorVisualZones(selectedDonor.html) : []), [selectedDonor]);

  useEffect(() => {
    if (donorId && (donorId === activeVariantId || !donorVariants.some((v) => v.id === donorId))) {
      setDonorId("");
      setSelectedFfIds([]);
      setIsPickerOpen(false);
    }
  }, [activeVariantId, donorId, donorVariants]);

  const handleDonorChange = (nextId: string) => {
    setDonorId(nextId);
    setSelectedFfIds([]);
    setIsPickerOpen(false);
  };

  return {
    donorId, selectedDonor, donorZones, selectedFfIds, setSelectedFfIds,
    isPickerOpen, setIsPickerOpen, handleDonorChange,
  };
}

export const CrossVariantBorrowForm: React.FC<CrossVariantBorrowFormProps> = ({
  activeVariant,
  donorVariants,
  isCommitting,
  onConfirm,
  onFocusSubChat,
}) => {
  const { t } = useTranslation();
  const ctrl = useBorrowFormState(activeVariant.id, donorVariants);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const resolvedDonor = ctrl.selectedDonor?.id;
    onConfirm({
      borrowFromVariantId: resolvedDonor,
      borrowSelectorOrInstruction: resolvedDonor && ctrl.selectedFfIds.length > 0 ? ctrl.selectedFfIds.join(",") : undefined,
    });
  };

  return (
    <>
      <form onSubmit={handleSubmit} data-testid="cross-variant-borrow-form" style={FORM_STYLE}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", flexWrap: "wrap" }}>
          <span style={{ fontSize: "12.5px", fontWeight: FONT_SEMIBOLD, color: "var(--text-primary)" }}>
            {t("tournament.borrowTitle", "Mượn chi tiết từ phương án khác (tùy chọn)")}
          </span>
          <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>
            {t("tournament.activeWinnerHint", "Đang chọn")}: {activeVariant.displayLabel}
          </span>
        </div>
        <BorrowControlsRow donorId={ctrl.donorId} selectedCount={ctrl.selectedFfIds.length} donorVariants={donorVariants} isCommitting={isCommitting} onDonorChange={ctrl.handleDonorChange} onOpenZonePicker={() => ctrl.setIsPickerOpen(true)} />
        <SelectedZoneChipsRow selectedFfIds={ctrl.selectedFfIds} donorZones={ctrl.donorZones} onRemove={(id) => ctrl.setSelectedFfIds((prev) => prev.filter((item) => item !== id))} />
        <SubChatSuggestionStrip onFocusSubChat={onFocusSubChat} />
      </form>
      <CrossVariantZonePickerModal isOpen={ctrl.isPickerOpen} donorVariant={ctrl.selectedDonor} initialSelectedFfIds={ctrl.selectedFfIds} onClose={() => ctrl.setIsPickerOpen(false)} onApply={(nextIds) => { ctrl.setSelectedFfIds(nextIds); ctrl.setIsPickerOpen(false); }} />
    </>
  );
};
