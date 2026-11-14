// @story 04.3
// @implements AC-02 (Collapsed Floating Pill Bar & Review Summary for Contextual Floating Chat)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import { WorkbenchNode } from "../store/flow-workbench-store";
import { NodeJobStatus } from "../store/keyed-parallel-coordinator";

export interface FloatingChatCollapsedPillProps {
  containerRef: React.RefObject<HTMLDivElement | null>;
  pillX: number;
  pillBottom: number;
  isDragging: boolean;
  isMulti: boolean;
  selectedCount: number;
  primaryNode: WorkbenchNode | null;
  primaryNodeStatus: { status: NodeJobStatus; queuePosition: number; jobId?: string } | null;
  showReviewSummary: boolean;
  onHeaderMouseDown: (e: React.MouseEvent) => void;
  onExpand: () => void;
  onToggleReviewSummary: () => void;
  onCloseReviewSummary: () => void;
  onClearSelection?: () => void;
}

const ReviewSummaryPopover: React.FC<{
  primaryNode: WorkbenchNode | null;
  onClose: () => void;
}> = ({ primaryNode, onClose }) => {
  const { t } = useTranslation();
  return (
    <div
      role="dialog"
      aria-label={t("floatingChat.reviewSummaryAria", "Tóm tắt công việc Node")}
      style={{
        position: "absolute",
        bottom: "48px",
        left: 0,
        width: "280px",
        backgroundColor: "var(--bg-surface)",
        border: "1px solid var(--border-default)",
        borderRadius: "var(--radius-md)",
        padding: "10px",
        boxShadow: "var(--shadow-lg)",
        zIndex: 60,
        fontSize: "11px",
        color: "var(--text-primary)",
      }}
    >
      <div style={{ fontWeight: 600, marginBottom: "4px" }}>
        {t("floatingChat.nodeSummaryTitle", "Tóm tắt Node")} {primaryNode?.screenKey}
      </div>
      <div style={{ color: "var(--text-secondary)", marginBottom: "8px" }}>
        {t("floatingChat.nodeTitleLabel", "Tiêu đề")}: {primaryNode?.title}
        <br />
        {t("floatingChat.nodeStatusLabel", "Trạng thái")}: {primaryNode?.status}
        <br />
        {t("floatingChat.subStateLabel", "Sub-State")}: {primaryNode?.activeSubState || "DEFAULT"}
      </div>
      <button
        type="button"
        onClick={onClose}
        style={{
          fontSize: "10px",
          padding: "2px 8px",
          borderRadius: "var(--radius-xs)",
          border: "1px solid var(--border-default)",
          backgroundColor: "var(--bg-subtle)",
          color: "var(--text-primary)",
          cursor: "pointer",
        }}
      >
        {t("common.close", "Đóng")}
      </button>
    </div>
  );
};

const PILL_CONTAINER_STYLE: React.CSSProperties = {
  position: "fixed",
  height: "38px",
  padding: "0 10px",
  borderRadius: "var(--radius-pill)",
  backgroundColor: "var(--bg-elevated)",
  border: "1px solid var(--border-default)",
  boxShadow: "var(--shadow-lg)",
  display: "flex",
  alignItems: "center",
  gap: "8px",
  zIndex: 55,
  userSelect: "none",
};

const PILL_ACTION_BTN_STYLE: React.CSSProperties = {
  fontSize: "10.5px",
  padding: "3px 8px",
  borderRadius: "var(--radius-pill)",
  backgroundColor: "var(--bg-subtle)",
  border: "1px solid var(--border-subtle)",
  color: "var(--text-secondary)",
  cursor: "pointer",
  display: "inline-flex",
  alignItems: "center",
  gap: "4px",
};

const PillNodeBadge: React.FC<{
  isMulti: boolean;
  selectedCount: number;
  primaryNode: WorkbenchNode | null;
  primaryNodeStatus: { status: NodeJobStatus; queuePosition: number; jobId?: string } | null;
  onExpand: () => void;
}> = ({ isMulti, selectedCount, primaryNode, primaryNodeStatus, onExpand }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const isRunning = primaryNodeStatus?.status === "running";
  const isQueued = primaryNodeStatus?.status === "queued";
  const openTitle = t("floatingChat.openContextChat", "Mở khung chat ngữ cảnh");
  return (
    <button
      type="button"
      data-testid="floating-chat-open-trigger"
      onClick={onExpand}
      title={openTitle}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "6px",
        border: "none",
        background: "transparent",
        padding: "2px 4px",
        cursor: "pointer",
      }}
    >
      <BrandIcon name="crosshair" size={13} color="var(--action-primary)" />
      <span style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-primary)" }}>
        {isMulti ? `${formatNumber(selectedCount)} ${t("floatingChat.nodesUnit", "Nodes")}` : primaryNode?.screenKey}
      </span>
      <span style={{ fontSize: "11px", color: "var(--text-muted)", display: "inline-flex", alignItems: "center", gap: "4px" }}>
        <BrandIcon name="message-square" size={11} color="var(--text-secondary)" />
        <span>{t("floatingChat.chatShortLabel", "Chat")}</span>
      </span>
      {isRunning && (
        <span style={{ display: "inline-flex", alignItems: "center", gap: "3px", fontSize: "10px", fontWeight: 600, color: "var(--action-primary)", backgroundColor: "var(--bg-selected)", padding: "2px 6px", borderRadius: "var(--radius-xs)" }}>
          <BrandIcon name="execution-pipeline" size={10} color="currentColor" />
          <span>{t("floatingChat.processing", "Đang xử lý")}</span>
        </span>
      )}
      {isQueued && (
        <span style={{ display: "inline-flex", alignItems: "center", gap: "3px", fontSize: "10px", fontWeight: 600, color: "var(--state-warning)", backgroundColor: "var(--bg-subtle)", padding: "2px 6px", borderRadius: "var(--radius-xs)" }}>
          <BrandIcon name="clock" size={10} color="currentColor" />
          <span>{t("floatingChat.queueLabel", "Hàng đợi")} #{formatNumber(primaryNodeStatus?.queuePosition ?? 0)}</span>
        </span>
      )}
    </button>
  );
};

export const FloatingChatCollapsedPill: React.FC<FloatingChatCollapsedPillProps> = ({
  containerRef,
  pillX,
  pillBottom,
  isDragging,
  isMulti,
  selectedCount,
  primaryNode,
  primaryNodeStatus,
  showReviewSummary,
  onHeaderMouseDown,
  onExpand,
  onToggleReviewSummary,
  onCloseReviewSummary,
  onClearSelection,
}) => {
  const { t } = useTranslation();
  return (
    <div
      ref={containerRef}
      className="fc-compact-pill"
      data-testid="fc-compact-pill"
      style={{ ...PILL_CONTAINER_STYLE, left: `${pillX}px`, bottom: `${pillBottom}px`, cursor: isDragging ? "grabbing" : "grab" }}
      onMouseDown={onHeaderMouseDown}
    >
      <PillNodeBadge isMulti={isMulti} selectedCount={selectedCount} primaryNode={primaryNode} primaryNodeStatus={primaryNodeStatus} onExpand={onExpand} />
      <button type="button" onClick={onToggleReviewSummary} style={PILL_ACTION_BTN_STYLE}>
        {t("floatingChat.reviewSummaryBtn", "Review Summary")}
      </button>
      <button type="button" onClick={onExpand} title={t("floatingChat.expandChatTitle", "Mở rộng khung Chat")} aria-label={t("floatingChat.expandChatTitle", "Mở rộng khung Chat")} style={{ width: "24px", height: "24px", border: "none", background: "transparent", cursor: "pointer", color: "var(--text-secondary)", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "var(--radius-xs)" }}>
        <BrandIcon name="maximize" size={12} color="currentColor" />
      </button>
      {onClearSelection && (
        <button type="button" onClick={onClearSelection} title={t("floatingChat.closeChat", "Đóng")} aria-label={t("floatingChat.closeChat", "Đóng")} style={{ width: "22px", height: "22px", border: "none", background: "transparent", cursor: "pointer", color: "var(--text-muted)", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "var(--radius-xs)" }}>
          <BrandIcon name="close" size={12} color="currentColor" />
        </button>
      )}
      {showReviewSummary && <ReviewSummaryPopover primaryNode={primaryNode} onClose={onCloseReviewSummary} />}
    </div>
  );
};
