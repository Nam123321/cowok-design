// @story 04.3, 04.5
// @implements AC-03, AC-04 (Shared Zero-Tailwind Spatial Comment Pin Marker in components/)
import React from "react";
import type { AnchorStatus } from "../../lib/validations/element-comment";

export interface CommentPinMarkerProps {
  index: number;
  anchorStatus: AnchorStatus;
  elementFfId?: string;
  onClick?: () => void;
}

export const CommentPinMarker: React.FC<CommentPinMarkerProps> = ({
  index,
  anchorStatus,
  elementFfId,
  onClick,
}) => {
  const getBadgeStyle = (status: AnchorStatus): React.CSSProperties => {
    switch (status) {
      case "stale":
        return { backgroundColor: "var(--agent-accent)", color: "var(--action-primary-fg)", borderColor: "var(--border-strong)" };
      case "resolved":
        return { backgroundColor: "var(--bg-subtle)", color: "var(--text-muted)", textDecoration: "line-through", borderColor: "var(--border-subtle)" };
      case "active":
      default:
        return { backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", borderColor: "var(--action-primary)" };
    }
  };

  return (
    <button
      type="button"
      onClick={onClick}
      className="comment-pin-marker"
      data-testid={`comment-pin-${index}`}
      data-ff-id={elementFfId}
      style={{
        width: "24px", height: "24px", borderRadius: "var(--radius-pill)",
        display: "flex", alignItems: "center", justifyContent: "center",
        fontSize: "12px", fontWeight: 700, boxShadow: "var(--shadow-md)",
        border: "1px solid", cursor: "pointer",
        transition: "transform var(--duration-fast) var(--ease-standard)",
        ...getBadgeStyle(anchorStatus),
      }}
      title={`Comment #${index} (${anchorStatus})`}
    >
      {index}
    </button>
  );
};
