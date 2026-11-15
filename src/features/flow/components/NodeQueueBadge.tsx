// @story 04.4
// @implements AC-01, AC-02 (Reusable Node Queue Badge with Localized Formatting)
import React from "react";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { useTranslation } from "@/hooks/useTranslation";
import { useFormatter } from "@/hooks/useFormatter";

export type NodeQueueStatus =
  | "running"
  | "queued"
  | "paused"
  | "completed"
  | "error"
  | "cancelled"
  | "interrupted";

export interface NodeQueueBadgeProps {
  status: NodeQueueStatus;
  queuePosition?: number;
  fencingToken?: number;
  className?: string;
  style?: React.CSSProperties;
}

const BADGE_FONT_WEIGHT = 600;
const BADGE_ICON_SIZE = 10;
const TOKEN_OPACITY = 0.7;

const QueuedBadgeView: React.FC<{
  queuePosition: number;
  fencingToken?: number;
  className: string;
  style: React.CSSProperties;
}> = ({ queuePosition, fencingToken, className, style }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  return (
    <span
      data-testid="node-queue-badge"
      className={`dag-node-queued-badge ${className}`}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "3px",
        fontSize: "10px",
        fontWeight: BADGE_FONT_WEIGHT,
        color: "var(--state-warning)",
        backgroundColor: "var(--bg-subtle)",
        padding: "2px 6px",
        borderRadius: "var(--radius-xs)",
        border: "1px solid var(--border-subtle)",
        ...style,
      }}
    >
      <BrandIcon name="clock" size={BADGE_ICON_SIZE} color="currentColor" />
      <span>
        {t("floatingChat.queueLabel", "Hàng đợi")} #{formatNumber(queuePosition)}
      </span>
      {fencingToken ? (
        <span style={{ opacity: TOKEN_OPACITY, fontFamily: "var(--font-mono)" }}>
          (#{formatNumber(fencingToken)})
        </span>
      ) : null}
    </span>
  );
};

const RunningBadgeView: React.FC<{
  fencingToken?: number;
  className: string;
  style: React.CSSProperties;
}> = ({ fencingToken, className, style }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  return (
    <span
      data-testid="node-running-badge"
      className={`dag-node-running-badge ${className}`}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "3px",
        fontSize: "10px",
        fontWeight: BADGE_FONT_WEIGHT,
        color: "var(--action-primary)",
        backgroundColor: "var(--bg-selected)",
        padding: "2px 6px",
        borderRadius: "var(--radius-xs)",
        ...style,
      }}
    >
      <BrandIcon name="ai-agent" size={BADGE_ICON_SIZE} color="currentColor" />
      <span>{t("floatingChat.runningBadge", "RUNNING")}</span>
      {fencingToken ? (
        <span style={{ opacity: TOKEN_OPACITY, fontFamily: "var(--font-mono)" }}>
          (#{formatNumber(fencingToken)})
        </span>
      ) : null}
    </span>
  );
};

const PausedBadgeView: React.FC<{
  className: string;
  style: React.CSSProperties;
}> = ({ className, style }) => {
  const { t } = useTranslation();
  return (
    <span
      data-testid="node-paused-badge"
      className={`dag-node-paused-badge ${className}`}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "3px",
        fontSize: "10px",
        fontWeight: BADGE_FONT_WEIGHT,
        color: "var(--text-secondary)",
        backgroundColor: "var(--bg-subtle)",
        padding: "2px 6px",
        borderRadius: "var(--radius-xs)",
        ...style,
      }}
    >
      <BrandIcon name="clock" size={BADGE_ICON_SIZE} color="currentColor" />
      <span>{t("floatingChat.jobPaused", "Paused")}</span>
    </span>
  );
};

const InterruptedBadgeView: React.FC<{
  className: string;
  style: React.CSSProperties;
}> = ({ className, style }) => (
  <span
    data-testid="node-interrupted-badge"
    className={`dag-node-interrupted-badge ${className}`}
    style={{
      display: "inline-flex",
      alignItems: "center",
      gap: "3px",
      fontSize: "10px",
      fontWeight: BADGE_FONT_WEIGHT,
      color: "var(--state-danger)",
      backgroundColor: "var(--bg-subtle)",
      padding: "2px 6px",
      borderRadius: "var(--radius-xs)",
      ...style,
    }}
  >
    <BrandIcon name="shield-alert" size={BADGE_ICON_SIZE} color="currentColor" />
    <span>INTERRUPTED</span>
  </span>
);

export const NodeQueueBadge: React.FC<NodeQueueBadgeProps> = ({
  status,
  queuePosition = 0,
  fencingToken,
  className = "",
  style = {},
}) => {
  if (status === "queued") {
    return (
      <QueuedBadgeView
        queuePosition={queuePosition}
        fencingToken={fencingToken}
        className={className}
        style={style}
      />
    );
  }
  if (status === "running") {
    return (
      <RunningBadgeView
        fencingToken={fencingToken}
        className={className}
        style={style}
      />
    );
  }
  if (status === "paused") {
    return <PausedBadgeView className={className} style={style} />;
  }
  if (status === "interrupted") {
    return <InterruptedBadgeView className={className} style={style} />;
  }
  return null;
};
