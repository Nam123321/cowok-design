// @story 05.3
import React from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { type VersionSnapshotResponse } from "../../../lib/validations/version-snapshot";

export interface RevertConfirmModalProps {
  isOpen: boolean;
  targetSnapshot: VersionSnapshotResponse | null;
  isReverting: boolean;
  requiresGraphRestore?: boolean;
  restoreMissingNodes?: boolean;
  errorMessage?: string | null;
  onToggleRestoreMissingNodes?: (checked: boolean) => void;
  onConfirm: () => void;
  onClose: () => void;
}

const SafetyNotice: React.FC = () => {
  const { t } = useTranslation();
  return (
    <div className="snap-safety-banner">
      <p className="snap-safety-banner-title">{t("snapshots.safetyTitle", "Safety First Mechanism")}</p>
      <p className="snap-modal-desc">
        {t("snapshots.safetyDescPrefix", "The system automatically captures a safety snapshot")}{" "}
        <code>pre_revert_safety</code>{" "}
        {t("snapshots.safetyDescSuffix", "before overwriting files.")}
      </p>
    </div>
  );
};

const DeletedNodeRestoreBanner: React.FC<{
  restoreMissingNodes: boolean;
  onToggleRestoreMissingNodes?: (checked: boolean) => void;
}> = ({ restoreMissingNodes, onToggleRestoreMissingNodes }) => {
  const { t } = useTranslation();
  return (
    <div className="snap-node-restore-banner" data-testid="node-deleted-restore-banner">
      <strong>{t("snapshots.deletedNodeGuardTitle", "Referenced FlowNode was deleted from the Canvas")}</strong>
      <span>
        {t("snapshots.deletedNodeGuardDesc", "Restoring this turn requires re-attaching the missing Canvas node or confirming graph restore.")}
      </span>
      {onToggleRestoreMissingNodes && (
        <label className="snap-node-restore-checkbox">
          <input
            type="checkbox"
            checked={restoreMissingNodes}
            data-testid="restore-missing-nodes-checkbox"
            onChange={(e) => onToggleRestoreMissingNodes(e.target.checked)}
          />
          <span>{t("snapshots.restoreMissingNodesLabel", "Restore deleted Canvas node & revert")}</span>
        </label>
      )}
    </div>
  );
};

const RevertModalActions: React.FC<{
  isReverting: boolean;
  requiresGraphRestore: boolean;
  restoreMissingNodes: boolean;
  onClose: () => void;
  onConfirm: () => void;
}> = ({ isReverting, requiresGraphRestore, restoreMissingNodes, onClose, onConfirm }) => {
  const { t } = useTranslation();
  let label = t("snapshots.confirmRevertBtn", "Confirm Restore");
  if (isReverting) {
    label = t("snapshots.revertingBtn", "Restoring...");
  } else if (requiresGraphRestore) {
    label = t("snapshots.confirmRestoreWithGraphBtn", "Restore Node & Revert");
  }

  return (
    <div className="snap-modal-actions">
      <button type="button" onClick={onClose} disabled={isReverting} data-testid="revert-cancel-btn" className="snap-btn snap-btn--ghost">
        {t("common.cancel", "Cancel")}
      </button>
      <button
        type="button"
        onClick={onConfirm}
        disabled={isReverting || (requiresGraphRestore && !restoreMissingNodes)}
        data-testid="revert-confirm-btn"
        className="snap-btn snap-btn--primary"
      >
        {label}
      </button>
    </div>
  );
};

export const RevertConfirmModal: React.FC<RevertConfirmModalProps> = ({
  isOpen,
  targetSnapshot,
  isReverting,
  requiresGraphRestore = false,
  restoreMissingNodes = false,
  errorMessage = null,
  onToggleRestoreMissingNodes,
  onConfirm,
  onClose,
}) => {
  const { t } = useTranslation();
  if (!isOpen || !targetSnapshot) return null;

  return (
    <div className="snap-modal-overlay" role="dialog" aria-modal="true" data-testid="revert-confirm-modal">
      <div className="snap-modal-card">
        <h3 className="snap-modal-title">{t("snapshots.revertModalTitle", "Confirm Version Restore")}</h3>
        <p className="snap-modal-desc">
          {t("snapshots.revertToPrefix", "Restore to")}{" "}
          <span className="snap-item-tag">{targetSnapshot.versionTag}</span> ({targetSnapshot.label}).
        </p>
        <SafetyNotice />
        {requiresGraphRestore && (
          <DeletedNodeRestoreBanner restoreMissingNodes={restoreMissingNodes} onToggleRestoreMissingNodes={onToggleRestoreMissingNodes} />
        )}
        {errorMessage && !requiresGraphRestore && (
          <p className="snap-form-error" data-testid="revert-error-banner">{errorMessage}</p>
        )}
        <RevertModalActions
          isReverting={isReverting}
          requiresGraphRestore={requiresGraphRestore}
          restoreMissingNodes={restoreMissingNodes}
          onClose={onClose}
          onConfirm={onConfirm}
        />
      </div>
    </div>
  );
};
