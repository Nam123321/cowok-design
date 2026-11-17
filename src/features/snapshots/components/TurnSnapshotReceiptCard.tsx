// @story 05.3
import React from "react";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { type ChatArtifactSnapshotResponse } from "../../../lib/validations/version-snapshot";

const SHORT_HASH_LEN = 8;

export interface TurnSnapshotReceiptCardProps {
  turnSnapshot: ChatArtifactSnapshotResponse;
  isPreviewActive?: boolean;
  isBusy?: boolean;
  forkedSessionId?: string | null;
  detachedNodeIds?: string[];
  onPreview: (snapshot: ChatArtifactSnapshotResponse) => void;
  onUndoTurn: (snapshot: ChatArtifactSnapshotResponse) => void;
  onForkSession: (snapshot: ChatArtifactSnapshotResponse) => void;
}

const TurnReceiptMeta: React.FC<{
  turnSnapshot: ChatArtifactSnapshotResponse;
  isPreviewActive: boolean;
  forkedSessionId: string | null;
  detachedCount: number;
}> = ({ turnSnapshot, isPreviewActive, forkedSessionId, detachedCount }) => {
  const { t } = useTranslation();
  const { formatNumber, formatTime } = useFormatter();
  const shortHash = turnSnapshot.contentHash.slice(0, SHORT_HASH_LEN);
  const titleText = turnSnapshot.label || `${t("snapshots.turnReceiptPrefix", "Turn")} #${shortHash}`;

  return (
    <div className="snap-turn-receipt-meta">
      <span className="snap-badge snap-badge-turn">{shortHash}</span>
      <span>{titleText}</span>
      <span>{formatTime(turnSnapshot.createdAt)}</span>
      {isPreviewActive && (
        <span className="snap-badge snap-badge-historical" data-testid="turn-historical-readonly-badge">
          {t("snapshots.historicalReadOnlyBadge", "Read-Only Preview")}
        </span>
      )}
      {forkedSessionId && (
        <span className="snap-fork-indicator" data-testid="turn-fork-session-indicator">
          {t("snapshots.forkedSessionBadge", "Forked")}: {forkedSessionId.slice(0, SHORT_HASH_LEN)}
        </span>
      )}
      {detachedCount > 0 && (
        <span className="snap-badge snap-badge-auto" data-testid="turn-detached-nodes-badge">
          {formatNumber(detachedCount)} {t("snapshots.detachedNodesUnit", "detached")}
        </span>
      )}
    </div>
  );
};

const TurnReceiptActions: React.FC<{
  turnSnapshot: ChatArtifactSnapshotResponse;
  isPreviewActive: boolean;
  isBusy: boolean;
  onPreview: (s: ChatArtifactSnapshotResponse) => void;
  onUndoTurn: (s: ChatArtifactSnapshotResponse) => void;
  onForkSession: (s: ChatArtifactSnapshotResponse) => void;
}> = ({ turnSnapshot, isPreviewActive, isBusy, onPreview, onUndoTurn, onForkSession }) => {
  const { t } = useTranslation();
  return (
    <div className="snap-turn-receipt-actions">
      <button
        type="button"
        className="snap-btn snap-btn--ghost snap-btn--compact"
        data-testid={`turn-preview-btn-${turnSnapshot.id}`}
        disabled={isBusy}
        aria-pressed={isPreviewActive}
        onClick={() => onPreview(turnSnapshot)}
      >
        {isPreviewActive
          ? t("snapshots.exitPreviewBtn", "Exit Preview")
          : t("snapshots.historicalPreviewBtn", "Historical Preview")}
      </button>
      <button
        type="button"
        className="snap-btn snap-btn--compact"
        data-testid={`turn-undo-btn-${turnSnapshot.id}`}
        disabled={isBusy}
        onClick={() => onUndoTurn(turnSnapshot)}
      >
        {t("snapshots.undoTurnBtn", "Undo Turn")}
      </button>
      <button
        type="button"
        className="snap-btn snap-btn--primary snap-btn--compact"
        data-testid={`turn-fork-btn-${turnSnapshot.id}`}
        disabled={isBusy}
        onClick={() => onForkSession(turnSnapshot)}
      >
        {t("snapshots.forkSessionBtn", "Fork Session")}
      </button>
    </div>
  );
};

export const TurnSnapshotReceiptCard: React.FC<TurnSnapshotReceiptCardProps> = ({
  turnSnapshot,
  isPreviewActive = false,
  isBusy = false,
  forkedSessionId = null,
  detachedNodeIds = [],
  onPreview,
  onUndoTurn,
  onForkSession,
}) => (
  <div className="snap-turn-receipt" data-testid={`turn-receipt-card-${turnSnapshot.id}`}>
    <TurnReceiptMeta
      turnSnapshot={turnSnapshot}
      isPreviewActive={isPreviewActive}
      forkedSessionId={forkedSessionId}
      detachedCount={detachedNodeIds.length}
    />
    <TurnReceiptActions
      turnSnapshot={turnSnapshot}
      isPreviewActive={isPreviewActive}
      isBusy={isBusy}
      onPreview={onPreview}
      onUndoTurn={onUndoTurn}
      onForkSession={onForkSession}
    />
  </div>
);
