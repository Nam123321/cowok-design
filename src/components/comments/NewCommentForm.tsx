// @story 04.3, 04.5
// @implements AC-03, AC-04 (Shared Zero-Tailwind NewCommentForm without useEffect)
import React, { useState } from "react";
import { useTranslation } from "../../hooks/useTranslation";
import { MAX_COMMENT_LENGTH } from "../../lib/validations/element-comment";

export interface NewCommentFormProps {
  prefillFfId?: string;
  onAddComment?: (content: string, elementFfId: string) => Promise<boolean | void> | boolean | void;
}

export const NewCommentForm: React.FC<NewCommentFormProps> = ({ prefillFfId, onAddComment }) => {
  const { t } = useTranslation();
  const [newContent, setNewContent] = useState("");
  const [editedFfId, setEditedFfId] = useState<{ base: string; value: string } | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const baseFfId = prefillFfId ?? "sec-header";
  const effectiveFfId = editedFfId && editedFfId.base === baseFfId ? editedFfId.value : baseFfId;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = newContent.trim();
    if (!trimmed || trimmed.length > MAX_COMMENT_LENGTH || isSubmitting) return;
    setIsSubmitting(true);
    try {
      const result = await onAddComment?.(trimmed, effectiveFfId.trim() || "sec-header");
      if (result !== false) {
        setNewContent("");
        setEditedFfId(null);
      }
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <form onSubmit={(e) => { void handleSubmit(e); }} className="new-comment-form" data-testid="new-comment-form" style={{ display: "flex", flexDirection: "column", gap: "8px", paddingTop: "8px", borderTop: "1px solid var(--border-subtle)" }}>
      <input
        type="text"
        data-testid="new-comment-input"
        maxLength={MAX_COMMENT_LENGTH}
        disabled={isSubmitting}
        placeholder={t("comments.pinPlaceholder", "Ghim bình luận tại data-ff-id...")}
        value={newContent}
        onChange={(e) => setNewContent(e.target.value)}
        style={{ width: "100%", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-xs)", padding: "6px 10px", fontSize: "12px", color: "var(--text-primary)", boxSizing: "border-box" }}
      />
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
        <input
          type="text"
          data-testid="new-comment-ffid-input"
          disabled={isSubmitting}
          value={effectiveFfId}
          onChange={(e) => setEditedFfId({ base: baseFfId, value: e.target.value })}
          placeholder="data-ff-id"
          style={{ width: "136px", backgroundColor: "var(--bg-canvas)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-xs)", padding: "4px 8px", fontSize: "11px", fontFamily: "var(--font-mono)", color: "var(--text-muted)", boxSizing: "border-box" }}
        />
        <button type="submit" data-testid="submit-comment-btn" disabled={isSubmitting} style={{ padding: "4px 12px", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", borderRadius: "var(--radius-xs)", fontSize: "12px", fontWeight: 600, border: "none", cursor: isSubmitting ? "wait" : "pointer", opacity: isSubmitting ? 0.7 : 1, boxShadow: "var(--shadow-sm)", transition: "opacity var(--duration-fast) var(--ease-standard)" }}>
          {t("common.send", "Gửi")}
        </button>
      </div>
    </form>
  );
};
