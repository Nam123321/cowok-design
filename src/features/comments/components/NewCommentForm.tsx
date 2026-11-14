import React, { useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";

interface NewCommentFormProps {
  onAddComment?: (content: string, elementFfId: string) => void;
}

export const NewCommentForm: React.FC<NewCommentFormProps> = ({ onAddComment }) => {
  const { t } = useTranslation();
  const [newContent, setNewContent] = useState("");
  const [selectedFfId, setSelectedFfId] = useState("ff-btn-submit");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newContent.trim()) return;
    onAddComment?.(newContent.trim(), selectedFfId);
    setNewContent("");
  };

  return (
    <form
      onSubmit={handleSubmit}
      className="new-comment-form"
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "8px",
        paddingTop: "8px",
        borderTop: "1px solid var(--border-subtle)",
      }}
    >
      <input
        type="text"
        placeholder={t("comments.pinPlaceholder", "Ghim bình luận tại data-ff-id...")}
        value={newContent}
        onChange={(e) => setNewContent(e.target.value)}
        style={{
          width: "100%",
          backgroundColor: "var(--bg-subtle)",
          border: "1px solid var(--border-default)",
          borderRadius: "var(--radius-xs)",
          padding: "6px 10px",
          fontSize: "12px",
          color: "var(--text-primary)",
          boxSizing: "border-box",
        }}
      />
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <input
          type="text"
          value={selectedFfId}
          onChange={(e) => setSelectedFfId(e.target.value)}
          placeholder="data-ff-id"
          style={{
            width: "128px",
            backgroundColor: "var(--bg-canvas)",
            border: "1px solid var(--border-default)",
            borderRadius: "var(--radius-xs)",
            padding: "4px 8px",
            fontSize: "11px",
            fontFamily: "var(--font-mono)",
            color: "var(--text-muted)",
            boxSizing: "border-box",
          }}
        />
        <button
          type="submit"
          style={{
            padding: "4px 12px",
            backgroundColor: "var(--action-primary)",
            color: "var(--action-primary-fg)",
            borderRadius: "var(--radius-xs)",
            fontSize: "12px",
            fontWeight: 600,
            border: "none",
            cursor: "pointer",
            boxShadow: "var(--shadow-sm)",
            transition: "opacity var(--duration-fast) var(--ease-standard)",
          }}
        >
          {t("common.send", "Gửi")}
        </button>
      </div>
    </form>
  );
};
