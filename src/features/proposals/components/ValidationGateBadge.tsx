// @story 05.2
import React from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ProposalGateReceipt, ValidationCheckResult } from "../../../lib/validations/change-proposal";

const HTTP_UNVERIFIED_GATE_STATUS = 412;
const HTTP_STALE_BASE_HASH_STATUS = 409;
const HMAC_PREVIEW_LENGTH = 8;

export interface ProposalGateErrorBanner {
  status?: number;
  code: string;
  message: string;
  staleFiles?: string[];
}

interface ValidationGateBadgeProps {
  checks: ValidationCheckResult[];
  gateReceipt?: ProposalGateReceipt | null;
  errorBanner?: ProposalGateErrorBanner | null;
  isStaleStatus?: boolean;
  onRebaseAndValidate?: () => void;
  isRevalidating?: boolean;
}

function getStatusBadge(status: "passed" | "failed" | "skipped", t: (k: string, f?: string) => string) {
  if (status === "passed") return { className: "cp-gate-status cp-gate-status--passed", icon: "✓", label: t("proposals.gatePass", "PASS") };
  if (status === "failed") return { className: "cp-gate-status cp-gate-status--failed", icon: "✕", label: t("proposals.gateFail", "FAIL") };
  return { className: "cp-gate-status cp-gate-status--skipped", icon: "•", label: t("proposals.gateSkip", "SKIP") };
}

const GateErrorAlerts: React.FC<{
  errorBanner?: ProposalGateErrorBanner | null;
  isStaleStatus?: boolean;
  onRebaseAndValidate?: () => void;
  isRevalidating: boolean;
}> = ({ errorBanner, isStaleStatus, onRebaseAndValidate, isRevalidating }) => {
  const { t } = useTranslation();
  const isUnverifiedClaim = errorBanner?.status === HTTP_UNVERIFIED_GATE_STATUS || errorBanner?.code === "UNVERIFIED_GATE_CLAIM";
  const isStaleBaseHash =
    Boolean(isStaleStatus) || errorBanner?.status === HTTP_STALE_BASE_HASH_STATUS || errorBanner?.code === "PROPOSAL_STALE_BASE_HASH";

  return (
    <>
      {isUnverifiedClaim && (
        <div className="cp-gate-banner" role="alert" data-testid="gate-error-412">
          <span className="cp-gate-banner-code">{t("proposals.error412Code", "412 UNVERIFIED_GATE_CLAIM")}</span>
          <span>{t("proposals.unverifiedGateClaimMsg", "Server blocked Apply: missing or invalid daemon HMAC gate receipt. Re-run validation before applying.")}</span>
        </div>
      )}
      {isStaleBaseHash && (
        <div className="cp-gate-banner" role="alert" data-testid="gate-error-409">
          <span className="cp-gate-banner-code">{t("proposals.error409Code", "409 PROPOSAL_STALE_BASE_HASH")}</span>
          <span>{t("proposals.staleBaseHashMsg", "Workspace files changed since this proposal snapshot was captured. Rebase snapshot and re-validate to mint a fresh receipt.")}</span>
          {errorBanner?.staleFiles && errorBanner.staleFiles.length > 0 && (
            <span className="cp-gate-check-name">{errorBanner.staleFiles.join(", ")}</span>
          )}
          {onRebaseAndValidate && (
            <button type="button" className="cp-btn-rebase" data-testid="rebase-revalidate-btn" disabled={isRevalidating} onClick={onRebaseAndValidate}>
              {isRevalidating ? t("proposals.revalidating", "Re-validating...") : t("proposals.rebaseAndValidateBtn", "Rebase Snapshot & Re-Validate")}
            </button>
          )}
        </div>
      )}
    </>
  );
};

export const ValidationGateBadge: React.FC<ValidationGateBadgeProps> = ({
  checks,
  gateReceipt,
  errorBanner,
  isStaleStatus = false,
  onRebaseAndValidate,
  isRevalidating = false,
}) => {
  const { t } = useTranslation();
  const isReceiptVerified = Boolean(gateReceipt && gateReceipt.exitCode === 0 && gateReceipt.receiptHmac);
  const receiptLabel = isReceiptVerified
    ? `${t("proposals.hmacVerified", "HMAC VERIFIED")} · ${gateReceipt!.receiptHmac.slice(0, HMAC_PREVIEW_LENGTH)}`
    : t("proposals.hmacUnverified", "NO RECEIPT");

  return (
    <div className="cp-gate-card" data-testid="validation-gate-badge">
      <div className="cp-gate-header">
        <span className="cp-gate-title">{t("proposals.validationGateTitle", "Automated Validation Gate")}</span>
        <span className={`cp-gate-receipt${isReceiptVerified ? " cp-gate-receipt--verified" : ""}`} data-testid="gate-receipt-pill" title={gateReceipt?.receiptHmac || ""}>
          {receiptLabel}
        </span>
      </div>
      <GateErrorAlerts errorBanner={errorBanner} isStaleStatus={isStaleStatus} onRebaseAndValidate={onRebaseAndValidate} isRevalidating={isRevalidating} />
      <div className="cp-gate-list">
        {checks.map((c) => {
          const badge = getStatusBadge(c.status, t);
          return (
            <div key={c.checkType} className="cp-gate-row">
              <span className="cp-gate-check-name">{c.checkType}</span>
              <span className={badge.className}>
                <span>{badge.icon}</span>
                <span>{badge.label}</span>
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
};
