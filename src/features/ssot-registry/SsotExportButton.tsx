// @story 02.4
// @implements AC-01, AC-02, AC-05
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";

const EXPORT_ICON_SIZE = 11;
const EXPORT_ICON_STROKE = 1.85;

export interface SsotExportButtonProps {
  isExporting: boolean;
  disabled?: boolean;
  onExport: () => void;
}

export const SsotExportButton: React.FC<SsotExportButtonProps> = ({
  isExporting,
  disabled = false,
  onExport,
}) => {
  const { t } = useTranslation();

  return (
    <button
      type="button"
      data-ff-id="ssot-export-button"
      data-testid="ssot-export-trigger"
      aria-busy={isExporting}
      disabled={disabled || isExporting}
      onClick={onExport}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "5px",
        height: "26px",
        padding: "0 8px",
        fontSize: "11px",
        fontWeight: 500,
        color: "var(--text-secondary)",
        backgroundColor: "var(--bg-workspace)",
        border: "1px solid var(--border-subtle)",
        borderRadius: "var(--radius-xs)",
        cursor: isExporting ? "wait" : "pointer",
      }}
    >
      <BrandIcon name="deploy" size={EXPORT_ICON_SIZE} strokeWidth={EXPORT_ICON_STROKE} color="currentColor" />
      <span>{isExporting ? t("ssot.export.busy", "Exporting...") : t("ssot.export.cta", "Export Design Kit")}</span>
    </button>
  );
};
