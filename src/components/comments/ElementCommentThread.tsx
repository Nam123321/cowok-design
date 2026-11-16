// @story 04.3, 04.5
// @implements AC-03, AC-04 (Shared Zero-Tailwind ElementCommentThread in components/)
import React from "react";
import { useTranslation } from "../../hooks/useTranslation";
import type { ElementCommentResponse } from "../../lib/validations/element-comment";
import { ElementCommentItem } from "./ElementCommentItem";
import { NewCommentForm } from "./NewCommentForm";

export interface ElementCommentThreadProps {
  comments: ElementCommentResponse[];
  nodeId: string;
  prefillFfId?: string;
  collapsedCommentIds?: string[];
  staleToastMessage?: string | null;
  toastTone?: "warning" | "danger";
  onDismissStaleToast?: () => void;
  onAddComment?: (content: string, elementFfId: string) => Promise<boolean | void> | boolean | void;
  onResolveComment?: (commentId: string) => void;
  onReanchorComment?: (commentId: string) => void;
}

export const ElementCommentThread: React.FC<ElementCommentThreadProps> = ({
  comments,
  prefillFfId,
  collapsedCommentIds = [],
  staleToastMessage,
  toastTone = "warning",
  onDismissStaleToast,
  onAddComment,
  onResolveComment,
  onReanchorComment,
}) => {
  const { t } = useTranslation();
  const collapsedSet = new Set(collapsedCommentIds);
  const isDanger = toastTone === "danger";
  const toastBg = isDanger ? "var(--state-danger-soft)" : "var(--state-warning-soft)";
  const toastBorder = isDanger ? "var(--state-danger)" : "var(--state-warning)";

  return (
    <div className="element-comment-thread" data-testid="element-comment-thread" style={{ display: "flex", flexDirection: "column", gap: "10px", width: "100%", boxSizing: "border-box" }}>
      {staleToastMessage && (
        <div role="alert" data-testid="stale-mapping-toast" style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", backgroundColor: toastBg, border: `1px solid ${toastBorder}`, color: "var(--text-primary)", fontSize: "11px", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", lineHeight: 1.4 }}>
          <span>{staleToastMessage}</span>
          {onDismissStaleToast && (
            <button type="button" onClick={onDismissStaleToast} aria-label={t("common.close", "Đóng")} style={{ border: "none", background: "transparent", color: "var(--text-muted)", cursor: "pointer", fontWeight: 700 }}>×</button>
          )}
        </div>
      )}
      <div className="comment-items-list" style={{ display: "flex", flexDirection: "column", gap: "8px", maxHeight: "320px", overflowY: "auto", paddingRight: "4px" }}>
        {comments.length === 0 ? (
          <div style={{ fontSize: "11px", color: "var(--text-muted)", padding: "8px 0" }}>
            {t("comments.emptyThread", "Chưa có bình luận ghim trên node này.")}
          </div>
        ) : (
          comments.map((comment) => (
            <ElementCommentItem
              key={comment.id}
              comment={comment}
              isZeroRectCollapsed={collapsedSet.has(comment.id)}
              onResolve={onResolveComment}
              onReanchor={onReanchorComment}
            />
          ))
        )}
      </div>
      <NewCommentForm prefillFfId={prefillFfId} onAddComment={onAddComment} />
    </div>
  );
};
