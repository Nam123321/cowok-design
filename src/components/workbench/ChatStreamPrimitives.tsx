// @story 04.4
// @implements AC-02 (Shared Chat Stream Item & Model Constants in components/)
import React from "react";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { useTranslation } from "@/hooks/useTranslation";

export interface ChatMessage {
  id: string;
  sender: "human" | "agent";
  text: string;
  timestamp: string;
}

export const AVAILABLE_CHAT_MODELS = [
  { id: "cowok-swarm", label: "Cowok Swarm · High" },
  { id: "claude-37", label: "Claude 3.7 Sonnet" },
  { id: "gemini-25", label: "Gemini 2.5 Pro" },
];

export const ChatStreamItem: React.FC<{ msg: ChatMessage }> = ({ msg }) => {
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
        <BrandIcon name="ai-agent" size={14} color="var(--agent-accent)" />
        <span style={{ fontWeight: 600, color: "var(--text-secondary)" }}>{t("chat.agentTitle", "cowok Agent")}</span>
        <span>•</span>
        <span style={{ fontFamily: "var(--font-mono)", fontSize: "10px" }}>{msg.timestamp}</span>
      </div>
      <div style={{ paddingLeft: "2px" }}>{msg.text}</div>
    </div>
  );
};
