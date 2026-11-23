// @story 08.5
// @implements AC-04, AC-05 (V20 gate_failure_ledger -> System-1 LEARN Queue with hit_count>=3 & hysteresis)
import React, { useCallback, useEffect, useState } from "react";
import { ZodError } from "zod";
import {
  DeRuntimeApiError,
  listLearnLedger,
  recordLearnLedgerFailure,
} from "../../api/de-runtime";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import {
  LEARN_MIN_HIT_COUNT,
  LEARN_PROMOTE_SLPR,
  LEARN_RETIRE_SLPR,
  type GateFailureLedgerEntryDto,
} from "../../lib/validations/de-runtime";
import "../plugins/plugins.css";

const ICON_SIZE = 14;
const SHA_PREFIX_LEN = 12;
const DEFAULT_SAMPLE_SLPR = 0.96;

export interface LearnLedgerQueueListProps {
  projectId: string;
  onCardProposed?: () => void;
}

function ledgerStatusBadgeClass(status: GateFailureLedgerEntryDto["status"]): string {
  if (status === "PROMOTED") return "plugin-eco-badge plugin-eco-badge-active";
  if (status === "PROPOSED") return "plugin-eco-badge plugin-eco-badge-human";
  if (status === "RETIRED") return "plugin-eco-badge plugin-eco-badge-danger";
  return "plugin-eco-badge plugin-eco-badge-warning";
}

const SLPR_DECIMALS = 2;

const LedgerEntryRow: React.FC<{ entry: GateFailureLedgerEntryDto }> = ({ entry }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <div className="plugin-eco-chip-card" data-testid={`learn-ledger-row-${entry.id}`}>
      <div className="plugin-eco-card-row">
        <div>
          <div className="plugin-eco-slug">
            {entry.ruleId} • {t("deRuntime.hitsLabel", "Lượt gặp")}: {formatNumber(entry.hitCount)} •{" "}
            SLPR: {formatNumber(entry.slprScore, SLPR_DECIMALS)}
          </div>
          <div className="plugin-eco-meta">
            noul:{entry.fingerprintSha256.slice(0, SHA_PREFIX_LEN)} • {entry.sampleMessage}
          </div>
        </div>
        <span className={ledgerStatusBadgeClass(entry.status)} data-testid={`ledger-status-${entry.id}`}>
          {entry.status}
        </span>
      </div>
      {entry.promotedCardId && (
        <div className="plugin-eco-meta" data-testid={`ledger-promoted-card-${entry.id}`}>
          {t("deRuntime.proposedCardLabel", "Thẻ đề xuất")}: {entry.promotedCardId} (
          {t("deRuntime.awaitsHumanApprovalNote", "chờ người dùng phê duyệt")})
        </div>
      )}
    </div>
  );
};

const LedgerHeader: React.FC<{
  qualifyingOnly: boolean;
  loading: boolean;
  onToggleQualifying: () => void;
  onRefresh: () => void;
}> = ({ qualifyingOnly, loading, onToggleQualifying, onRefresh }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <div className="plugin-eco-header">
      <div className="plugin-eco-title-group">
        <BrandIcon name="file-text" size={ICON_SIZE} color="var(--human-accent)" />
        <h3 className="plugin-eco-title">
          {t("deRuntime.learnLedgerTitle", "System-1 LEARN Gate Failure Ledger")}
        </h3>
      </div>
      <div className="plugin-eco-actions">
        <span className="plugin-eco-badge" data-testid="ledger-hysteresis-badge">
          {t("deRuntime.hitsLabel", "Lượt gặp")}&gt;={formatNumber(LEARN_MIN_HIT_COUNT)} •{" "}
          {t("deRuntime.promoteLabel", "đề xuất")}&gt;={formatNumber(LEARN_PROMOTE_SLPR, SLPR_DECIMALS)} •{" "}
          {t("deRuntime.retireLabel", "thu hồi")}&lt;{formatNumber(LEARN_RETIRE_SLPR, SLPR_DECIMALS)}
        </span>
        <button
          type="button"
          className="plugin-eco-btn"
          data-testid="btn-toggle-qualifying"
          onClick={onToggleQualifying}
        >
          {qualifyingOnly
            ? t("deRuntime.showAllLedger", "Show All")
            : t("deRuntime.showQualifyingLedger", "Qualifying Only")}
        </button>
        <button
          type="button"
          className="plugin-eco-btn"
          data-testid="btn-refresh-ledger"
          disabled={loading}
          onClick={onRefresh}
        >
          {t("deRuntime.refreshBtn", "Refresh")}
        </button>
      </div>
    </div>
  );
};

const LedgerRecordBar: React.FC<{
  ruleId: string;
  failureMessage: string;
  submitting: boolean;
  onRuleChange: (v: string) => void;
  onMessageChange: (v: string) => void;
  onRecord: () => void;
}> = ({ ruleId, failureMessage, submitting, onRuleChange, onMessageChange, onRecord }) => {
  const { t } = useTranslation();
  return (
    <div className="plugin-eco-inline-bar">
      <input
        type="text"
        className="plugin-eco-input"
        data-testid="input-ledger-rule-id"
        value={ruleId}
        onChange={(e) => onRuleChange(e.target.value)}
        placeholder={t("deRuntime.ruleIdPlaceholder", "Rule ID")}
      />
      <input
        type="text"
        className="plugin-eco-rationale-input"
        data-testid="input-ledger-message"
        value={failureMessage}
        onChange={(e) => onMessageChange(e.target.value)}
        placeholder={t("deRuntime.failureMsgPlaceholder", "Observed DE failure message")}
      />
      <button
        type="button"
        className="plugin-eco-btn plugin-eco-btn-primary"
        data-testid="btn-record-ledger-failure"
        disabled={submitting || !failureMessage.trim()}
        onClick={onRecord}
      >
        {t("deRuntime.recordFailureBtn", "Record DE Failure")}
      </button>
    </div>
  );
};

function formatLedgerErr(err: unknown): string {
  if (err instanceof DeRuntimeApiError) return `[${err.code}] ${err.message}`;
  if (err instanceof ZodError) return err.issues[0]?.message || "Invalid input";
  return String(err);
}

const DEFAULT_DRAFT = {
  ruleId: "TELLS-001",
  failureMessage: "Raw indigo gradient detected in hero card",
};

function buildLedgerPayload(projectId: string, draft: typeof DEFAULT_DRAFT) {
  return {
    projectId,
    ruleId: draft.ruleId.trim() || "TELLS-001",
    failureMessage: draft.failureMessage.trim(),
    slprScore: DEFAULT_SAMPLE_SLPR,
  };
}

function useLearnLedgerController(projectId: string, onCardProposed?: () => void) {
  const [entries, setEntries] = useState<GateFailureLedgerEntryDto[]>([]);
  const [qualifyingOnly, setQualifyingOnly] = useState(false);
  const [loading, setLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState(DEFAULT_DRAFT);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setEntries((await listLearnLedger(projectId, qualifyingOnly)).data.entries);
    } catch (err) {
      setError(formatLedgerErr(err));
    } finally {
      setLoading(false);
    }
  }, [projectId, qualifyingOnly]);

  useEffect(() => { void refresh(); }, [refresh]);

  const recordSample = async () => {
    if (submitting) return;
    setSubmitting(true);
    setError(null);
    try {
      const res = await recordLearnLedgerFailure(buildLedgerPayload(projectId, draft));
      await refresh();
      if (res.data.status === "PROPOSED") onCardProposed?.();
    } catch (err) {
      setError(formatLedgerErr(err));
    } finally {
      setSubmitting(false);
    }
  };

  return {
    entries, qualifyingOnly, loading, submitting, error, draft,
    setRuleId: (ruleId: string) => setDraft((p) => ({ ...p, ruleId })),
    setFailureMessage: (failureMessage: string) => setDraft((p) => ({ ...p, failureMessage })),
    toggleQualifying: () => setQualifyingOnly((prev) => !prev),
    refresh, recordSample,
  };
}

export const LearnLedgerQueueList: React.FC<LearnLedgerQueueListProps> = ({
  projectId,
  onCardProposed,
}) => {
  const { t } = useTranslation();
  const ctrl = useLearnLedgerController(projectId, onCardProposed);

  return (
    <section
      className="plugin-eco-section"
      data-ff-id="learn-ledger-queue"
      data-testid="learn-ledger-queue"
    >
      <LedgerHeader
        qualifyingOnly={ctrl.qualifyingOnly}
        loading={ctrl.loading}
        onToggleQualifying={ctrl.toggleQualifying}
        onRefresh={() => void ctrl.refresh()}
      />
      <LedgerRecordBar
        ruleId={ctrl.draft.ruleId}
        failureMessage={ctrl.draft.failureMessage}
        submitting={ctrl.submitting}
        onRuleChange={ctrl.setRuleId}
        onMessageChange={ctrl.setFailureMessage}
        onRecord={() => void ctrl.recordSample()}
      />
      {ctrl.error && (
        <div className="plugin-eco-error-box" role="alert" data-testid="learn-ledger-error">
          <span>{ctrl.error}</span>
        </div>
      )}
      {ctrl.entries.length === 0 && !ctrl.loading ? (
        <div className="plugin-eco-state-box" data-testid="learn-ledger-empty">
          {t("deRuntime.ledgerEmpty", "No gate failures recorded in V20 gate_failure_ledger yet.")}
        </div>
      ) : (
        <div className="plugin-eco-chip-grid">
          {ctrl.entries.map((entry) => (
            <LedgerEntryRow key={entry.id} entry={entry} />
          ))}
        </div>
      )}
    </section>
  );
};
