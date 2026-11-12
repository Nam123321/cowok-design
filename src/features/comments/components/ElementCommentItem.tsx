import React from "react";
import { ElementCommentResponse } from "../../../lib/validations/element-comment";

interface ElementCommentItemProps {
  comment: ElementCommentResponse;
  onResolve?: (commentId: string) => void;
  onReanchor?: (commentId: string) => void;
}

export const ElementCommentItem: React.FC<ElementCommentItemProps> = ({
  comment,
  onResolve,
  onReanchor,
}) => {
  const isStale = comment.anchorStatus === "stale";
  const isResolved = comment.anchorStatus === "resolved";

  return (
    <div
      className="element-comment-item"
      style={{
        padding: "10px",
        backgroundColor: "var(--bg-subtle)",
        border: "1px solid var(--border-default)",
        borderRadius: "var(--radius-sm)",
        display: "flex",
        flexDirection: "column",
        gap: "6px",
        fontSize: "12px",
        boxSizing: "border-box",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <span style={{ fontWeight: 600, color: "var(--text-primary)" }}>{comment.author}</span>
        <span
          style={{
            fontSize: "10px",
            color: "var(--text-muted)",
            fontFamily: "var(--font-mono)",
          }}
        >
          {comment.elementFfId}
        </span>
      </div>
      <p style={{ margin: 0, color: "var(--text-secondary)", lineHeight: 1.4 }}>{comment.content}</p>
      {isStale && (
        <div
          className="stale-anchor-banner"
          style={{
            padding: "6px 8px",
            backgroundColor: "var(--state-warning-soft, rgba(217, 130, 43, 0.12))",
            border: "1px solid var(--state-warning)",
            color: "var(--text-primary)",
            borderRadius: "var(--radius-xs)",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            fontSize: "11px",
          }}
        >
          <span>Phần tử đã đổi cấu trúc (Stale)</span>
          {onReanchor && (
            <button
              type="button"
              onClick={() => onReanchor(comment.id)}
              style={{
                fontSize: "11px",
                fontWeight: 600,
                textDecoration: "underline",
                color: "var(--action-primary)",
                background: "transparent",
                border: "none",
                cursor: "pointer",
              }}
            >
              Gắn lại
            </button>
          )}
        </div>
      )}
      {!isResolved && onResolve && (
        <button
          type="button"
          onClick={() => onResolve(comment.id)}
          style={{
            alignSelf: "flex-start",
            fontSize: "11px",
            color: "var(--text-muted)",
            background: "transparent",
            border: "none",
            cursor: "pointer",
            fontWeight: 500,
            padding: "2px 0",
          }}
        >
          ✓ Đánh dấu giải quyết
        </button>
      )}
    </div>
  );
};
