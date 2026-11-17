// @story 04.4
// @implements AC-02 (Shared Chat Stream Item & Model Constants in components/)
import React from "react";
import { useTranslation } from "@/hooks/useTranslation";

export interface SdiClarificationCardData {
  intentId: string;
  projectId: string;
  status: "clarifying" | "ready" | "executed";
  ambiguityScore: number;
  variants: Array<{ id: string; title: string; description: string }>;
  selectedVariantId?: string;
}

export interface ChatMessage {
  id: string;
  sender: "human" | "agent";
  text: string;
  timestamp: string;
  sdiClarification?: SdiClarificationCardData;
}

export const AVAILABLE_CHAT_MODELS = [
  { id: "cowok-swarm", label: "Cowok Swarm · High" },
  { id: "claude-37", label: "Claude 3.7 Sonnet" },
  { id: "gemini-25", label: "Gemini 2.5 Pro" },
];

export const ChatStreamItem: React.FC<{
  msg: ChatMessage;
  onSelectSdiVariant?: (intentId: string, projectId: string, variantId: string) => void;
}> = ({ msg, onSelectSdiVariant }) => {
  const { t } = useTranslation();
  if (msg.sender === "human") {
    return (
      <div
        className="chat-human-bubble"
        data-testid="chat-human-bubble"
        style={{
          alignSelf: "flex-end",
          maxWidth: "86%",
          backgroundColor: "var(--human-soft)",
          color: "var(--text-primary)",
          border: "1px solid var(--human-border)",
          padding: "8px 12px",
          borderRadius: "12px 12px 2px 12px",
          fontSize: "12.5px",
          lineHeight: "1.5",
          wordBreak: "break-word",
        }}
      >
        {msg.text}
      </div>
    );
  }
  const sdi = msg.sdiClarification;
  return (
    <div
      className="chat-agent-prose"
      data-quotable="true"
      style={{
        alignSelf: "stretch",
        width: "100%",
        color: "var(--text-primary)",
        fontSize: "13px",
        lineHeight: "1.6",
        padding: "4px 0",
        wordBreak: "break-word",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: "6px", marginBottom: "4px", fontSize: "11px", color: "var(--text-muted)" }}>
        <span style={{ fontWeight: 600, color: "var(--text-secondary)" }}>{t("chat.agentTitle", "cowok Agent")}</span>
        <span>•</span>
        <span style={{ fontFamily: "var(--font-mono)", fontSize: "10px" }}>{msg.timestamp}</span>
      </div>
      <div style={{ paddingLeft: "2px" }}>{msg.text}</div>
      {sdi && sdi.variants.length > 0 && (
        <div data-testid="sdi-clarification-card" style={{ marginTop: "8px", padding: "10px", borderRadius: "var(--radius-sm)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-default)", display: "flex", flexDirection: "column", gap: "6px" }}>
          <div style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-secondary)" }}>
            {sdi.status === "ready"
              ? t("sdi.clarificationResolved", "Đã chọn hướng thiết kế:")
              : t("sdi.chooseVariantPrompt", "Chọn 1 phương án để mở khóa thực thi:")}
          </div>
          {sdi.variants.map((v) => {
            const isPicked = sdi.selectedVariantId === v.id;
            return (
              <button
                key={v.id}
                type="button"
                data-testid={`sdi-variant-btn-${v.id}`}
                disabled={sdi.status === "ready"}
                onClick={() => onSelectSdiVariant?.(sdi.intentId, sdi.projectId, v.id)}
                style={{
                  textAlign: "left",
                  padding: "7px 9px",
                  borderRadius: "var(--radius-xs)",
                  border: isPicked ? "1.5px solid var(--action-primary)" : "1px solid var(--border-default)",
                  backgroundColor: isPicked ? "var(--bg-selected)" : "var(--bg-surface)",
                  color: "var(--text-primary)",
                  cursor: sdi.status === "ready" ? "default" : "pointer",
                  display: "flex",
                  flexDirection: "column",
                  gap: "2px",
                }}
              >
                <span style={{ fontSize: "11.5px", fontWeight: 600 }}>{v.title}</span>
                <span style={{ fontSize: "10.5px", color: "var(--text-secondary)" }}>{v.description}</span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
};
