// @story 08.4
// @implements AC-05 (LEARN Human Approval Queue with XSS & Beacon Stripping)
import React, { useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import { MAX_LEARN_TOKENS, type KnowledgeCardDto } from "../../lib/validations/plugins-learn-cron";
import { sanitizeLearnMarkdownPreview } from "./sanitizeMarkdownPreview";
import "./plugins.css";

const QUEUE_ICON_SIZE = 14;
const MIN_RATIONALE_CHARS = 4;
const MAX_RATIONALE_CHARS = 500;
const CHARS_PER_TOKEN = 4;
const XML_CARD_OVERHEAD_TOKENS = 24;
const SHA_PREFIX_LEN = 12;

type CardFilter = "ALL" | "PROPOSED" | "ACCEPTED" | "REJECTED";

export interface LearnCardReviewQueueProps {
  cards: KnowledgeCardDto[];
  loading: boolean;
  error: string | null;
  pendingActionId: string | null;
  actionFeedback?: string | null;
  initialFilter?: CardFilter;
  onRefresh: () => void;
  onApprove: (card: KnowledgeCardDto, reason: string) => void;
  onReject: (card: KnowledgeCardDto, reason: string) => void;
}

function statusBadgeClass(status: KnowledgeCardDto["status"]): string {
  if (status === "ACCEPTED") return "plugin-eco-badge plugin-eco-badge-active";
  if (status === "REJECTED") return "plugin-eco-badge plugin-eco-badge-danger";
  return "plugin-eco-badge plugin-eco-badge-human";
}

function estimateAcceptedTokens(cards: KnowledgeCardDto[]): number {
  const encoder = new TextEncoder();
  return cards
    .filter((c) => c.status === "ACCEPTED")
    .reduce(
      (acc, c) =>
        acc +
        XML_CARD_OVERHEAD_TOKENS +
        Math.ceil(encoder.encode(`${c.title}\n${c.sourceEvidence}\n${c.content}`).length / CHARS_PER_TOKEN),
      0
    );
}

const LearnCardRationaleBar: React.FC<{
  card: KnowledgeCardDto;
  busy: boolean;
  onApprove: (card: KnowledgeCardDto, reason: string) => void;
  onReject: (card: KnowledgeCardDto, reason: string) => void;
}> = ({ card, busy, onApprove, onReject }) => {
  const { t } = useTranslation();
  const [reason, setReason] = useState("");
  const reasonValid = reason.trim().length >= MIN_RATIONALE_CHARS;
  return (
    <>
      <input
        type="text"
        className="plugin-eco-rationale-input"
        data-testid={`input-learn-reason-${card.id}`}
        minLength={MIN_RATIONALE_CHARS}
        maxLength={MAX_RATIONALE_CHARS}
        value={reason}
        onChange={(e) => setReason(e.target.value)}
        placeholder={t("learn.rationalePlaceholder", "Enter approval/rejection rationale (>= 4 chars)...")}
      />
      <div className="plugin-eco-actions">
        <button
          type="button"
          className="plugin-eco-btn plugin-eco-btn-primary"
          data-testid={`btn-approve-learn-${card.id}`}
          disabled={busy || !reasonValid}
          onClick={() => onApprove(card, reason.trim())}
        >
          {t("learn.approveBtn", "Approve & Seal")}
        </button>
        <button
          type="button"
          className="plugin-eco-btn plugin-eco-btn-danger"
          data-testid={`btn-reject-learn-${card.id}`}
          disabled={busy || !reasonValid}
          onClick={() => onReject(card, reason.trim())}
        >
          {t("learn.rejectBtn", "Reject")}
        </button>
      </div>
    </>
  );
};

const LearnCardItem: React.FC<{
  card: KnowledgeCardDto;
  busy: boolean;
  onApprove: (card: KnowledgeCardDto, reason: string) => void;
  onReject: (card: KnowledgeCardDto, reason: string) => void;
}> = ({ card, busy, onApprove, onReject }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <div className="plugin-eco-chip-card" data-testid={`learn-card-${card.id}`}>
      <div className="plugin-eco-card-row">
        <div>
          <div className="plugin-eco-slug">
            [{card.category}] {card.title}
          </div>
          <div className="plugin-eco-meta">
            {t("learn.revLabel", "Revision")} {formatNumber(card.revision)} • {card.contentSha256.slice(0, SHA_PREFIX_LEN)} • {card.sourceEvidence}
          </div>
        </div>
        <span className={statusBadgeClass(card.status)} data-testid={`learn-card-status-${card.id}`}>
          {card.status}
        </span>
      </div>
      <div className="plugin-eco-markdown-preview" data-testid={`learn-card-preview-${card.id}`}>
        {sanitizeLearnMarkdownPreview(card.content)}
      </div>
      {card.status === "PROPOSED" && (
        <LearnCardRationaleBar card={card} busy={busy} onApprove={onApprove} onReject={onReject} />
      )}
    </div>
  );
};

const LearnFilterTabs: React.FC<{ filter: CardFilter; onChange: (next: CardFilter) => void }> = ({ filter, onChange }) => {
  const { t } = useTranslation();
  const options: Array<{ id: CardFilter; label: string }> = [
    { id: "ALL", label: t("learn.filterAll", "ALL") },
    { id: "PROPOSED", label: t("learn.filterProposed", "PROPOSED") },
    { id: "ACCEPTED", label: t("learn.filterAccepted", "ACCEPTED") },
    { id: "REJECTED", label: t("learn.filterRejected", "REJECTED") },
  ];
  return (
    <div className="plugin-eco-tabs" role="tablist">
      {options.map((opt) => (
        <button
          key={opt.id}
          type="button"
          className={`plugin-eco-tab ${filter === opt.id ? "plugin-eco-tab-active" : ""}`}
          data-testid={`learn-filter-${opt.id}`}
          onClick={() => onChange(opt.id)}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
};

const LearnQueueBody: React.FC<
  Pick<LearnCardReviewQueueProps, "cards" | "loading" | "error" | "pendingActionId" | "onApprove" | "onReject">
> = ({ cards, loading, error, pendingActionId, onApprove, onReject }) => {
  const { t } = useTranslation();
  if (loading && cards.length === 0) {
    return (
      <div className="plugin-eco-state-box" data-testid="learn-queue-loading">
        {t("learn.loadingState", "Loading LEARN cards...")}
      </div>
    );
  }
  if (!loading && cards.length === 0 && !error) {
    return (
      <div className="plugin-eco-state-box" data-testid="learn-queue-empty">
        {t("learn.emptyState", "No LEARN cards proposed for this project yet.")}
      </div>
    );
  }
  return (
    <div className="plugin-eco-chip-grid">
      {cards.map((card) => (
        <LearnCardItem
          key={card.id}
          card={card}
          busy={pendingActionId === `approve:${card.id}` || pendingActionId === `reject:${card.id}`}
          onApprove={onApprove}
          onReject={onReject}
        />
      ))}
    </div>
  );
};

const LearnQueueHeader: React.FC<{ usedTokens: number }> = ({ usedTokens }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <div className="plugin-eco-header">
      <div className="plugin-eco-title-group">
        <BrandIcon name="tool" size={QUEUE_ICON_SIZE} color="var(--human-accent)" />
        <h3 className="plugin-eco-title">{t("learn.queueTitle", "LEARN Knowledge Review Queue")}</h3>
      </div>
      <div className="plugin-eco-actions">
        <span className="plugin-eco-badge" data-testid="learn-token-budget">
          {t("learn.budgetLabel", "Prompt Budget")}: {formatNumber(usedTokens)}/{formatNumber(MAX_LEARN_TOKENS)}
        </span>
        <span className="plugin-eco-badge plugin-eco-badge-human">
          {t("mcpGateway.tierHumanOnly", "Chỉ người dùng thao tác")}
        </span>
      </div>
    </div>
  );
};

export const LearnCardReviewQueue: React.FC<LearnCardReviewQueueProps> = ({
  cards,
  loading,
  error,
  pendingActionId,
  actionFeedback,
  initialFilter = "ALL",
  onRefresh,
  onApprove,
  onReject,
}) => {
  const { t } = useTranslation();
  const [filter, setFilter] = useState<CardFilter>(initialFilter);
  const visibleCards = filter === "ALL" ? cards : cards.filter((c) => c.status === filter);
  const usedTokens = estimateAcceptedTokens(cards);

  return (
    <section className="plugin-eco-section" data-ff-id="learn-card-review-queue" data-testid="learn-card-queue">
      <LearnQueueHeader usedTokens={usedTokens} />
      <LearnFilterTabs filter={filter} onChange={setFilter} />
      {actionFeedback && (
        <div className="plugin-eco-badge plugin-eco-badge-active" data-testid="learn-action-feedback">
          {actionFeedback}
        </div>
      )}
      {error && (
        <div className="plugin-eco-error-box" role="alert" data-testid="learn-queue-error">
          <span>{error}</span>
          <button type="button" className="plugin-eco-btn" onClick={onRefresh}>
            {t("learn.retryBtn", "Retry")}
          </button>
        </div>
      )}
      <LearnQueueBody
        cards={visibleCards}
        loading={loading}
        error={error}
        pendingActionId={pendingActionId}
        onApprove={onApprove}
        onReject={onReject}
      />
    </section>
  );
};
