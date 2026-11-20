// @story 04.3, 04.5
// @implements AC-03, AC-04 (Shared Zero-Tailwind ElementCommentItem in components/)
import React from "react";
import { useTranslation } from "../../hooks/useTranslation";
import type { ElementCommentResponse } from "../../lib/validations/element-comment";

export interface ElementCommentItemProps {
  comment: ElementCommentResponse;
  isZeroRectCollapsed?: boolean;
  onResolve?: (commentId: string) => void;
  onReanchor?: (commentId: string) => void;
}

export const ElementCommentItem: React.FC<ElementCommentItemProps> = ({
  comment,
  isZeroRectCollapsed,
  onResolve,
  onReanchor,
}) => {
  const { t } = useTranslation();
  const isStale = comment.anchorStatus === "stale";
  const isResolved = comment.anchorStatus === "resolved";

  return (
    <div
      className="element-comment-item"
      data-testid={`comment-item-${comment.id}`}
      style={{
        padding: "10px",
        backgroundColor: "var(--bg-subtle)",
        border: isStale ? "1px solid var(--agent-accent)" : "1px solid var(--border-default)",
        borderRadius: "var(--radius-sm)",
        display: "flex",
        flexDirection: "column",
        gap: "6px",
        fontSize: "12px",
        boxSizing: "border-box",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap", gap: "6px", minWidth: 0 }}>
        <span style={{ fontWeight: 600, color: "var(--text-primary)" }}>{comment.author}</span>
        <div style={{ display: "flex", alignItems: "center", flexWrap: "wrap", gap: "4px", minWidth: 0, maxWidth: "100%" }}>
          {comment.anchorKind && (
            <span style={{ fontSize: "9.5px", padding: "1px 4px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-subtle)", color: "var(--text-secondary)", fontFamily: "var(--font-mono)" }}>
              {comment.anchorKind}
            </span>
          )}
          {isZeroRectCollapsed && (
            <span data-testid={`zero-rect-badge-${comment.id}`} style={{ fontSize: "9.5px", padding: "1px 4px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--state-warning-soft)", border: "1px solid var(--state-warning)", color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>
              {t("comments.drawerCollapsedRect", "0×0 Drawer")}
            </span>
          )}
          <span style={{ fontSize: "10px", color: "var(--text-muted)", fontFamily: "var(--font-mono)", overflowWrap: "anywhere", wordBreak: "break-word" }}>
            {comment.elementFfId}
          </span>
        </div>
      </div>

      <p style={{ margin: 0, color: "var(--text-secondary)", lineHeight: 1.4 }}>{comment.content}</p>

      {(isStale || isZeroRectCollapsed) && (
        <div className="stale-anchor-banner" data-testid={`stale-banner-${comment.id}`} style={{ padding: "6px 8px", backgroundColor: "var(--state-warning-soft)", border: "1px solid var(--agent-accent)", color: "var(--text-primary)", borderRadius: "var(--radius-xs)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", fontSize: "11px" }}>
          <span>{isStale ? t("comments.staleAnchorWarning", "Phần tử đã đổi cấu trúc (STALE_MAPPING)") : t("comments.zeroRectWarning", "Không tìm thấy khung hiển thị (rect 0)")}</span>
          {onReanchor && (
            <button type="button" data-testid={`reanchor-comment-btn-${comment.id}`} onClick={() => onReanchor(comment.id)} style={{ fontSize: "11px", fontWeight: 600, textDecoration: "underline", color: "var(--action-primary)", background: "transparent", border: "none", cursor: "pointer", whiteSpace: "nowrap" }}>
              {t("comments.reanchorFullBtn", "Gắn lại phần tử (Re-anchor)")}
            </button>
          )}
        </div>
      )}

      {!isResolved && onResolve && (
        <button type="button" data-testid={`resolve-comment-btn-${comment.id}`} onClick={() => onResolve(comment.id)} style={{ alignSelf: "flex-start", fontSize: "11px", color: "var(--text-muted)", background: "transparent", border: "none", cursor: "pointer", fontWeight: 500, padding: "2px 0" }}>
          {t("comments.resolveBtn", "✓ Đánh dấu giải quyết")}
        </button>
      )}
    </div>
  );
};
