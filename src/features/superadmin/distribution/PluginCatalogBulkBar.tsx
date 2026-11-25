// @story 14.3
// @implements AC-02 (Bulk Action Floating Card for Plugin Catalog Multi-Select Lifecycle Updates)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { PluginLifecycleStatus } from "../../../lib/validations/distribution-ops";

export interface PluginCatalogBulkBarProps {
  selectedCount: number;
  canEdit: boolean;
  submitting: boolean;
  onApplyStatus: (status: PluginLifecycleStatus, reason: string) => Promise<boolean>;
  onClearSelection: () => void;
}

const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const ICON_SIZE_SM = 14;
const MIN_REASON_LEN = 5;
const FLOATING_Z_INDEX = 45;

const FLOATING_CARD_STYLE: React.CSSProperties = {
  position: "fixed",
  bottom: "20px",
  left: "50%",
  transform: "translateX(-50%)",
  zIndex: FLOATING_Z_INDEX,
  boxShadow: "var(--shadow-lg)",
  backgroundColor: "var(--bg-card, var(--bg-surface))",
  border: "1px solid var(--border-strong, var(--border-default))",
  borderRadius: "12px",
  padding: "10px 14px",
  display: "flex",
  flexWrap: "wrap",
  alignItems: "center",
  gap: "10px",
  maxWidth: "calc(100vw - 24px)",
  color: "var(--text-primary)",
};

const COUNT_PILL_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "6px",
  padding: "4px 10px",
  borderRadius: "999px",
  backgroundColor: "var(--bg-selected)",
  color: "var(--action-primary)",
  fontSize: "12px",
  fontWeight: FONT_WEIGHT_BOLD,
  whiteSpace: "nowrap",
};

const REASON_INPUT_STYLE: React.CSSProperties = {
  height: "32px",
  minWidth: "200px",
  flex: "1 1 200px",
  padding: "0 10px",
  borderRadius: "8px",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  fontSize: "12px",
  boxSizing: "border-box",
};

const CLEAR_BTN_STYLE: React.CSSProperties = {
  height: "32px",
  padding: "0 10px",
  borderRadius: "8px",
  border: "1px solid var(--border-default)",
  backgroundColor: "transparent",
  color: "var(--text-secondary)",
  fontSize: "12px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  cursor: "pointer",
  whiteSpace: "nowrap",
};

interface BulkActionSpec {
  status: PluginLifecycleStatus;
  labelKey: string;
  labelFallback: string;
  testId: string;
  bg: string;
  fg: string;
}

const BULK_ACTIONS: readonly BulkActionSpec[] = [
  { status: "ACTIVE", labelKey: "superadmin.ops.bulkActivateBtn", labelFallback: "Kích hoạt", testId: "catalog-bulk-active-btn", bg: "var(--state-success-soft)", fg: "var(--state-success)" },
  { status: "PENDING_APPROVAL", labelKey: "superadmin.ops.statusPendingApproval", labelFallback: "Chờ duyệt", testId: "catalog-bulk-pending-btn", bg: "var(--state-warning-soft)", fg: "var(--state-warning)" },
  { status: "INACTIVE", labelKey: "superadmin.ops.statusInactive", labelFallback: "Tạm dừng", testId: "catalog-bulk-inactive-btn", bg: "var(--bg-subtle)", fg: "var(--text-secondary)" },
  { status: "QUARANTINED", labelKey: "superadmin.ops.statusQuarantinedShort", labelFallback: "Cách ly", testId: "catalog-bulk-quarantine-btn", bg: "var(--state-danger-soft)", fg: "var(--state-danger)" },
];

function BulkActionButtons({
  disabled,
  onSelect,
  onClear,
}: {
  disabled: boolean;
  onSelect: (status: PluginLifecycleStatus) => void;
  onClear: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: "6px" }}>
      {BULK_ACTIONS.map((action) => (
        <button
          key={action.status}
          type="button"
          disabled={disabled}
          data-testid={action.testId}
          onClick={() => onSelect(action.status)}
          style={{
            height: "32px",
            padding: "0 11px",
            borderRadius: "8px",
            border: "1px solid var(--border-subtle)",
            backgroundColor: disabled ? "var(--bg-subtle)" : action.bg,
            color: disabled ? "var(--text-disabled)" : action.fg,
            fontSize: "12px",
            fontWeight: FONT_WEIGHT_SEMIBOLD,
            cursor: disabled ? "not-allowed" : "pointer",
            whiteSpace: "nowrap",
          }}
        >
          {t(action.labelKey, action.labelFallback)}
        </button>
      ))}
      <button type="button" data-testid="catalog-bulk-clear-btn" onClick={onClear} style={CLEAR_BTN_STYLE}>
        {t("superadmin.ops.bulkClearBtn", "Bỏ chọn")}
      </button>
    </div>
  );
}

export const PluginCatalogBulkBar: React.FC<PluginCatalogBulkBarProps> = ({
  selectedCount,
  canEdit,
  submitting,
  onApplyStatus,
  onClearSelection,
}) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const [reason, setReason] = useState("");
  if (selectedCount <= 0) return null;

  const actionsDisabled = !canEdit || submitting || reason.trim().length < MIN_REASON_LEN;
  const handleSelect = async (status: PluginLifecycleStatus) => {
    if (actionsDisabled) return;
    if (await onApplyStatus(status, reason.trim())) {
      setReason("");
      onClearSelection();
    }
  };

  return (
    <div role="region" aria-label={t("superadmin.ops.bulkBarAria", "Thao tác hàng loạt danh mục plugin")} data-testid="catalog-bulk-floating-card" style={FLOATING_CARD_STYLE}>
      <span data-testid="catalog-bulk-count" style={COUNT_PILL_STYLE}>
        <BrandIcon name="plugin" size={ICON_SIZE_SM} color="var(--action-primary)" />
        {t("superadmin.ops.bulkSelectedCount", "Đã chọn {{count}} plugin", {
          count: formatNumber(selectedCount),
        })}
      </span>
      <input
        type="text"
        disabled={!canEdit || submitting}
        value={reason}
        onChange={(e) => setReason(e.target.value)}
        data-testid="catalog-bulk-reason-input"
        aria-label={t("superadmin.ops.bulkReasonAria", "Lý do cập nhật hàng loạt")}
        placeholder={t("superadmin.ops.bulkReasonPlaceholder", "Lý do kiểm toán (>= 5 ký tự)...")}
        style={REASON_INPUT_STYLE}
      />
      <BulkActionButtons
        disabled={actionsDisabled}
        onSelect={(st) => void handleSelect(st)}
        onClear={() => {
          setReason("");
          onClearSelection();
        }}
      />
    </div>
  );
};
