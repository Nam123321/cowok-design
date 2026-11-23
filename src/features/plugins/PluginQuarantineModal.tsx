// @story 08.4, 08.5
// @implements Compact Quarantine Reason Popup & Bulk Action Floating Card
import React, { useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { PluginSummaryDto } from "../../lib/validations/plugins";

const MIN_QUARANTINE_REASON_LEN = 4;
const MAX_QUARANTINE_REASON_LEN = 500;
const ICON_SIZE_SM = 13;

export interface PluginQuarantineModalProps {
  targets: PluginSummaryDto[];
  busy: boolean;
  onConfirm: (targets: PluginSummaryDto[], reason: string) => void;
  onClose: () => void;
}

const QuarantineModalFooter: React.FC<{
  canSubmit: boolean;
  onClose: () => void;
  onSubmit: () => void;
}> = ({ canSubmit, onClose, onSubmit }) => {
  const { t } = useTranslation();
  return (
    <div className="plugin-eco-actions" style={{ justifyContent: "flex-end" }}>
      <button type="button" className="plugin-eco-btn" data-testid="btn-cancel-quarantine-modal" onClick={onClose}>
        {t("common.cancel", "Hủy")}
      </button>
      <button
        type="button"
        className="plugin-eco-btn plugin-eco-btn-danger"
        data-testid="btn-confirm-quarantine-modal"
        disabled={!canSubmit}
        onClick={onSubmit}
      >
        {t("plugins.confirmQuarantineBtn", "Xác nhận cách ly")}
      </button>
    </div>
  );
};

const QuarantineModalHeader: React.FC<{ targetSummary: string }> = ({ targetSummary }) => {
  const { t } = useTranslation();
  return (
    <div className="plugin-eco-header">
      <div className="plugin-eco-title-group">
        <BrandIcon name="security" size={ICON_SIZE_SM} color="var(--state-danger)" />
        <h4 className="plugin-eco-title">{t("plugins.quarantineModalTitle", "Cách ly kỹ năng")}</h4>
      </div>
      <span className="plugin-eco-badge plugin-eco-badge-danger">{targetSummary}</span>
    </div>
  );
};

export const PluginQuarantineModal: React.FC<PluginQuarantineModalProps> = ({
  targets,
  busy,
  onConfirm,
  onClose,
}) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const [reason, setReason] = useState("");
  if (targets.length === 0) return null;

  const cleanReason = reason.trim();
  const canSubmit = cleanReason.length >= MIN_QUARANTINE_REASON_LEN && !busy;
  const targetSummary =
    targets.length === 1 && targets[0]
      ? targets[0].pluginSlug
      : t("plugins.bulkSelectedCount", "{{count}} kỹ năng đã chọn").replace("{{count}}", formatNumber(targets.length));

  return (
    <div className="plugin-eco-modal-backdrop" role="dialog" aria-modal="true" data-testid="quarantine-reason-modal">
      <div className="plugin-eco-modal-card">
        <QuarantineModalHeader targetSummary={targetSummary} />
        <p className="plugin-eco-subtitle" style={{ margin: 0 }}>
          {t("plugins.quarantineModalDesc", "Nhập lý do cách ly (tối thiểu 4 ký tự). Kỹ năng bị cách ly sẽ bị khóa kích hoạt cho tới khi được kiểm tra lại.")}
        </p>
        <input
          type="text"
          className="plugin-eco-rationale-input"
          data-testid="input-quarantine-reason-modal"
          minLength={MIN_QUARANTINE_REASON_LEN}
          maxLength={MAX_QUARANTINE_REASON_LEN}
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          placeholder={t("plugins.quarantinePlaceholder", "Lý do cách ly (>= 4 ký tự)...")}
          autoFocus
        />
        <QuarantineModalFooter
          canSubmit={canSubmit}
          onClose={onClose}
          onSubmit={() => {
            onConfirm(targets, cleanReason);
            setReason("");
          }}
        />
      </div>
    </div>
  );
};

export interface PluginBulkFloatingBarProps {
  selectedPlugins: PluginSummaryDto[];
  busy: boolean;
  onBulkActivate: () => void;
  onBulkDeactivate: () => void;
  onBulkQuarantine: () => void;
  onClearSelection: () => void;
}

const BulkBarActionButtons: React.FC<{
  canActivateCount: number;
  canDeactivateCount: number;
  canQuarantineCount: number;
  busy: boolean;
  onBulkActivate: () => void;
  onBulkDeactivate: () => void;
  onBulkQuarantine: () => void;
  onClearSelection: () => void;
}> = (p) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <div className="plugin-eco-actions">
      {p.canActivateCount > 0 && (
        <button type="button" className="plugin-eco-toggle-chip plugin-eco-toggle-chip--active" data-testid="btn-bulk-activate" disabled={p.busy} onClick={p.onBulkActivate}>
          {t("plugins.bulkActivateBtn", "Bật")} ({formatNumber(p.canActivateCount)})
        </button>
      )}
      {p.canDeactivateCount > 0 && (
        <button type="button" className="plugin-eco-toggle-chip" data-testid="btn-bulk-deactivate" disabled={p.busy} onClick={p.onBulkDeactivate}>
          {t("plugins.bulkDeactivateBtn", "Tắt")} ({formatNumber(p.canDeactivateCount)})
        </button>
      )}
      {p.canQuarantineCount > 0 && (
        <button type="button" className="plugin-eco-btn plugin-eco-btn-danger" data-testid="btn-bulk-quarantine" disabled={p.busy} onClick={p.onBulkQuarantine}>
          {t("plugins.quarantineBtn", "Cách ly")} ({formatNumber(p.canQuarantineCount)})
        </button>
      )}
      <button type="button" className="plugin-eco-btn" data-testid="btn-bulk-clear" onClick={p.onClearSelection}>
        {t("plugins.clearSelectionBtn", "Bỏ chọn")}
      </button>
    </div>
  );
};

export const PluginBulkFloatingBar: React.FC<PluginBulkFloatingBarProps> = ({
  selectedPlugins,
  busy,
  onBulkActivate,
  onBulkDeactivate,
  onBulkQuarantine,
  onClearSelection,
}) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  if (selectedPlugins.length === 0) return null;

  const canActivateCount = selectedPlugins.filter(
    (p) => !p.quarantined && ["INSTALLED", "DEPRECATED"].includes(p.lifecycleState.toUpperCase())
  ).length;
  const canDeactivateCount = selectedPlugins.filter((p) => p.lifecycleState.toUpperCase() === "ACTIVE").length;
  const canQuarantineCount = selectedPlugins.filter(
    (p) => !p.quarantined && p.lifecycleState.toUpperCase() !== "ARCHIVED"
  ).length;

  return (
    <div className="plugin-eco-bulk-float" role="region" aria-label={t("plugins.bulkBarLabel", "Thao tác hàng loạt kỹ năng")} data-testid="plugin-bulk-floating-bar">
      <span className="plugin-eco-bulk-count">
        {t("plugins.bulkSelectedCount", "{{count}} kỹ năng đã chọn").replace("{{count}}", formatNumber(selectedPlugins.length))}
      </span>
      <BulkBarActionButtons
        canActivateCount={canActivateCount}
        canDeactivateCount={canDeactivateCount}
        canQuarantineCount={canQuarantineCount}
        busy={busy}
        onBulkActivate={onBulkActivate}
        onBulkDeactivate={onBulkDeactivate}
        onBulkQuarantine={onBulkQuarantine}
        onClearSelection={onClearSelection}
      />
    </div>
  );
};
