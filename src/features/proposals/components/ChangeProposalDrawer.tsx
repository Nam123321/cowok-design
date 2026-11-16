// @story 05.2
import React, { useState } from "react";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ChangeProposalResponse, FileDiffItem } from "../../../lib/validations/change-proposal";
import { computeProposalDiffStats, formatVisualDiffMetrics } from "../utils/diff-engine";
import { CowokDesignDiffViewer } from "./CowokDesignDiffViewer";
import { DiffViewer } from "./DiffViewer";
import { type ProposalGateErrorBanner, ValidationGateBadge } from "./ValidationGateBadge";

const SSIM_DECIMALS = 3;
const DELTA_DECIMALS = 2;

interface DrawerProps {
  proposal: ChangeProposalResponse | null;
  isOpen: boolean;
  onClose: () => void;
  onApply: (proposalId: string) => void;
  onReject: (proposalId: string) => void;
  onRebaseAndValidate?: (proposalId: string) => void;
  onComputeVisualDiff?: (proposalId: string) => void;
  errorBanner?: ProposalGateErrorBanner | null;
  receiptToast?: { tone: "success" | "danger"; message: string } | null;
  onDismissToast?: () => void;
  isRevalidating?: boolean;
  isComputingDiff?: boolean;
}

const ProposalFileItem: React.FC<{ file: FileDiffItem }> = ({ file }) => {
  const [expanded, setExpanded] = useState(false);
  const { formatNumber } = useFormatter();

  return (
    <div className="cp-file-row">
      <button type="button" onClick={() => setExpanded(!expanded)} className="cp-file-toggle">
        <span className="cp-file-path">{file.path}</span>
        <span className="cp-file-stats">
          <span className="cp-metric-value--add">+{formatNumber(file.additions)}</span>
          <span className="cp-metric-value--del">-{formatNumber(file.deletions)}</span>
          <span>{expanded ? "▲" : "▼"}</span>
        </span>
      </button>
      {expanded && <div className="cp-file-patch-wrap"><DiffViewer patch={file.patch} /></div>}
    </div>
  );
};

const ProposalMetricStrip: React.FC<{ proposal: ChangeProposalResponse }> = ({ proposal }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const stats = computeProposalDiffStats(proposal.files);
  const vMetrics = formatVisualDiffMetrics(proposal.visualDiff);

  return (
    <div className="cp-metric-line" data-testid="proposal-metric-line">
      <span className="cp-metric-item">{t("proposals.metricFiles", "Files")}: <strong className="cp-metric-value">{formatNumber(stats.fileCount)}</strong></span>
      <span className="cp-metric-item"><strong className="cp-metric-value--add">+{formatNumber(stats.additions)}</strong>/<strong className="cp-metric-value--del">-{formatNumber(stats.deletions)}</strong></span>
      <span className="cp-metric-item">{t("proposals.ssimLabel", "SSIM")}: <strong className="cp-metric-value">{vMetrics.ssimValue !== null ? formatNumber(vMetrics.ssimValue, SSIM_DECIMALS) : "—"}</strong></span>
      <span className="cp-metric-item">{t("proposals.deltaLabel", "Delta")}: <strong className="cp-metric-value">{vMetrics.deltaPercent !== null ? `${formatNumber(vMetrics.deltaPercent, DELTA_DECIMALS)}%` : "—"}</strong></span>
      <span className="cp-metric-item">{t("proposals.hmacLabel", "HMAC")}: <strong className="cp-metric-value">{proposal.gateReceipt ? t("proposals.hmacOk", "OK") : "—"}</strong></span>
    </div>
  );
};

const ProposalDrawerHeader: React.FC<{ proposal: ChangeProposalResponse; onClose: () => void }> = ({ proposal, onClose }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const isBatch = Boolean(proposal.batchId || (proposal.nodeIds && proposal.nodeIds.length > 1));
  const scopeLabel = isBatch ? `${t("proposals.scopeBatch", "BATCH")} (${formatNumber(proposal.nodeIds?.length || 1)})` : `${t("proposals.scopeNode", "NODE")}: ${proposal.nodeId || "PROJECT"}`;

  return (
    <div className="cp-drawer-header">
      <div className="cp-drawer-title-wrap">
        <div className="cp-scope-row">
          <span className="cp-scope-pill" data-testid="proposal-scope-pill">{scopeLabel}</span>
          <span className="cp-scope-pill">{proposal.status.toUpperCase()}</span>
        </div>
        <h3 className="cp-drawer-title">{proposal.title}</h3>
        <p className="cp-drawer-desc">{proposal.description || t("proposals.noExtraDesc", "No additional description")}</p>
      </div>
      <button type="button" onClick={onClose} aria-label={t("common.close", "Close")} className="cp-icon-close-btn">✕</button>
    </div>
  );
};

const ProposalApplyConfirmModal: React.FC<{ isOpen: boolean; onCancel: () => void; onConfirm: () => void }> = ({ isOpen, onCancel, onConfirm }) => {
  const { t } = useTranslation();
  if (!isOpen) return null;
  return (
    <div className="cp-modal-backdrop" role="dialog" aria-modal="true" data-testid="proposal-apply-confirm-modal">
      <div className="cp-modal">
        <h4 className="cp-modal-title">{t("proposals.confirmApplyTitle", "Confirm Atomic Apply")}</h4>
        <p className="cp-modal-body">{t("proposals.confirmApplyBody", "A safety snapshot will be captured before writing workspace files. Proceed with applying this ChangeProposal?")}</p>
        <div className="cp-modal-actions">
          <button type="button" className="cp-btn-reject" onClick={onCancel}>{t("common.cancel", "Cancel")}</button>
          <button type="button" className="cp-btn-apply" data-testid="proposal-confirm-apply-btn" onClick={onConfirm}>{t("proposals.confirmApplyBtn", "Confirm & Apply")}</button>
        </div>
      </div>
    </div>
  );
};

const ProposalBodyTabs: React.FC<{
  proposal: ChangeProposalResponse;
  activeTab: "visual" | "code";
  onTabChange: (tab: "visual" | "code") => void;
  onComputeVisualDiff?: (id: string) => void;
  isComputingDiff: boolean;
}> = ({ proposal, activeTab, onTabChange, onComputeVisualDiff, isComputingDiff }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();

  return (
    <>
      <div className="cp-tabs" role="tablist">
        <button type="button" role="tab" aria-selected={activeTab === "visual"} className={`cp-tab${activeTab === "visual" ? " cp-tab--active" : ""}`} data-testid="proposal-tab-visual" onClick={() => onTabChange("visual")}>{t("proposals.visualDiffTab", "Visual Diff")}</button>
        <button type="button" role="tab" aria-selected={activeTab === "code"} className={`cp-tab${activeTab === "code" ? " cp-tab--active" : ""}`} data-testid="proposal-tab-code" onClick={() => onTabChange("code")}>{t("proposals.codeDiffTab", "Code Diff")}</button>
      </div>
      {activeTab === "visual" ? (
        <CowokDesignDiffViewer visualDiff={proposal.visualDiff} isComputing={isComputingDiff} onComputeDiff={onComputeVisualDiff ? () => onComputeVisualDiff(proposal.id) : undefined} />
      ) : (
        <div className="cp-file-section">
          <span className="cp-section-label">{t("proposals.modifiedFilesLabel", "Modified Files")} ({formatNumber(proposal.files.length)})</span>
          <div className="cp-file-table">{proposal.files.map((f) => <ProposalFileItem key={f.path} file={f} />)}</div>
        </div>
      )}
    </>
  );
};

function canApplyProposal(proposal: ChangeProposalResponse, errorBanner?: ProposalGateErrorBanner | null): boolean {
  const hasFailedCheck = proposal.validationChecks.some((c) => c.status === "failed");
  const isBlocked = errorBanner?.code === "UNVERIFIED_GATE_CLAIM" || errorBanner?.code === "PROPOSAL_STALE_BASE_HASH";
  return proposal.status === "ready" && proposal.validationStatus === "passed" && !hasFailedCheck && !isBlocked;
}

export const ChangeProposalDrawer: React.FC<DrawerProps> = ({
  proposal, isOpen, onClose, onApply, onReject, onRebaseAndValidate, onComputeVisualDiff,
  errorBanner, receiptToast, onDismissToast, isRevalidating = false, isComputingDiff = false,
}) => {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<"visual" | "code">("code");
  const [isConfirmModalOpen, setIsConfirmModalOpen] = useState(false);

  const toastNode = receiptToast ? (
    <div className={`cp-toast cp-toast--${receiptToast.tone}`} role="status" data-testid="proposal-receipt-toast">
      <span>{receiptToast.message}</span>
      {onDismissToast && <button type="button" className="cp-icon-close-btn" aria-label={t("common.close", "Close")} onClick={onDismissToast}>✕</button>}
    </div>
  ) : null;

  if (!isOpen || !proposal) return toastNode;
  const canApply = canApplyProposal(proposal, errorBanner);

  return (
    <>
      <aside className="cp-drawer" role="complementary" aria-label={proposal.title} data-testid="change-proposal-drawer">
        <div className="cp-drawer-body">
          <ProposalDrawerHeader proposal={proposal} onClose={onClose} />
          <ProposalMetricStrip proposal={proposal} />
          <ValidationGateBadge checks={proposal.validationChecks} gateReceipt={proposal.gateReceipt} errorBanner={errorBanner} isRevalidating={isRevalidating} onRebaseAndValidate={onRebaseAndValidate ? () => onRebaseAndValidate(proposal.id) : undefined} />
          <ProposalBodyTabs proposal={proposal} activeTab={activeTab} onTabChange={setActiveTab} onComputeVisualDiff={onComputeVisualDiff} isComputingDiff={isComputingDiff} />
        </div>
        <div className="cp-drawer-footer">
          <button type="button" onClick={() => onReject(proposal.id)} className="cp-btn-reject" data-testid="proposal-reject-btn">{t("proposals.rejectBtn", "Reject")}</button>
          <button type="button" disabled={!canApply} onClick={() => setIsConfirmModalOpen(true)} title={!canApply ? t("proposals.applyBlockedHint", "Apply is locked until safety checks pass [AC-03]") : ""} className="cp-btn-apply" data-testid="proposal-apply-btn">{t("proposals.applyBtn", "Apply Changes")}</button>
        </div>
      </aside>
      <ProposalApplyConfirmModal isOpen={isConfirmModalOpen} onCancel={() => setIsConfirmModalOpen(false)} onConfirm={() => { setIsConfirmModalOpen(false); onApply(proposal.id); }} />
      {toastNode}
    </>
  );
};
