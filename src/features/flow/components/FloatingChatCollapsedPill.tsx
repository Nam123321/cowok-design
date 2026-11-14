// @story 04.3
// @implements AC-02 (Collapsed Pill Badge & Review Summary for Contextual Floating Chat)
import React from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { WorkbenchNode } from "../store/flow-workbench-store";
import { NodeJobStatus } from "../store/keyed-parallel-coordinator";

export interface FloatingChatCollapsedPillProps {
  containerRef: React.RefObject<HTMLDivElement | null>;
  pillX: number;
  pillY: number;
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
        Tiêu đề: {primaryNode?.title}
        <br />
        Trạng thái: {primaryNode?.status}
        <br />
        Sub-State: {primaryNode?.activeSubState || "Default"}
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
  height: "42px",
  padding: "0 12px",
  borderRadius: "var(--radius-pill)",
  backgroundColor: "var(--bg-elevated)",
  border: "1px solid var(--border-strong)",
  boxShadow: "var(--shadow-lg)",
  display: "flex",
  alignItems: "center",
  gap: "8px",
  zIndex: 55,
  userSelect: "none",
};

const PILL_BADGE_BTN_STYLE: React.CSSProperties = {
  fontSize: "10.5px",
  padding: "2px 8px",
  borderRadius: "var(--radius-pill)",
  backgroundColor: "var(--bg-subtle)",
  border: "1px solid var(--border-default)",
  color: "var(--text-secondary)",
  cursor: "pointer",
};

const PillNodeBadge: React.FC<{
  isMulti: boolean;
  selectedCount: number;
  primaryNode: WorkbenchNode | null;
  primaryNodeStatus: { status: NodeJobStatus; queuePosition: number; jobId?: string } | null;
  onExpand: () => void;
}> = ({ isMulti, selectedCount, primaryNode, primaryNodeStatus, onExpand }) => {
  const isRunning = primaryNodeStatus?.status === "running";
  const isQueued = primaryNodeStatus?.status === "queued";
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "6px" }} onClick={onExpand}>
      <span style={{ fontSize: "12px", fontWeight: 700, color: "var(--action-primary)" }}>
        🎯 {isMulti ? `${selectedCount} Nodes` : primaryNode?.screenKey}
      </span>
      {isRunning && (
        <span style={{ fontSize: "10px", fontWeight: 600, color: "var(--action-primary)", backgroundColor: "var(--bg-selected)", padding: "2px 6px", borderRadius: "var(--radius-xs)" }}>
          ⚡ Đang xử lý
        </span>
      )}
      {isQueued && (
        <span style={{ fontSize: "10px", fontWeight: 600, color: "var(--state-warning)", backgroundColor: "var(--bg-subtle)", padding: "2px 6px", borderRadius: "var(--radius-xs)" }}>
          ⏳ Hàng đợi #{primaryNodeStatus?.queuePosition}
        </span>
      )}
    </div>
  );
};

export const FloatingChatCollapsedPill: React.FC<FloatingChatCollapsedPillProps> = ({
  containerRef,
  pillX,
  pillY,
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
}) => {
  const { t } = useTranslation();
  return (
    <div
      ref={containerRef}
      className="fc-compact-pill"
      data-testid="fc-compact-pill"
      style={{ ...PILL_CONTAINER_STYLE, left: `${pillX}px`, top: `${pillY}px`, cursor: isDragging ? "grabbing" : "pointer" }}
      onMouseDown={onHeaderMouseDown}
    >
      <PillNodeBadge
        isMulti={isMulti}
        selectedCount={selectedCount}
        primaryNode={primaryNode}
        primaryNodeStatus={primaryNodeStatus}
        onExpand={onExpand}
      />
      <button type="button" onClick={onToggleReviewSummary} style={PILL_BADGE_BTN_STYLE}>
        Review Summary
      </button>
      <button type="button" onClick={onExpand} title={t("floatingChat.expandChatTitle", "Mở rộng khung Chat")} style={{ border: "none", background: "transparent", cursor: "pointer", fontSize: "12px", color: "var(--text-muted)", padding: "2px" }}>
        ▲
      </button>
      {showReviewSummary && <ReviewSummaryPopover primaryNode={primaryNode} onClose={onCloseReviewSummary} />}
    </div>
  );
};
