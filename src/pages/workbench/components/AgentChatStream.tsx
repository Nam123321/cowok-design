// @story 04.1
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";

export interface ChatMessage {
  id: string;
  sender: "human" | "agent";
  text: string;
  timestamp: string;
}

interface AgentChatStreamProps {
  messages: ChatMessage[];
  showScrollBottom: boolean;
  onScrollMessages: (e: React.UIEvent<HTMLDivElement>) => void;
  onScrollToBottom: () => void;
  messagesEndRef: React.RefObject<HTMLDivElement | null>;
}

const STREAM_CONTAINER_STYLE: React.CSSProperties = {
  flex: 1, overflowY: "auto", padding: "14px 14px", display: "flex", flexDirection: "column",
  gap: "14px", backgroundColor: "var(--bg-canvas)", position: "relative",
};

const SCROLL_BTN_STYLE: React.CSSProperties = {
  position: "sticky", bottom: "8px", alignSelf: "center", width: "28px", height: "28px",
  borderRadius: "50%", backgroundColor: "var(--bg-elevated)", border: "1px solid var(--border-subtle)",
  color: "var(--text-primary)", boxShadow: "var(--shadow-md)", display: "flex", alignItems: "center",
  justifyContent: "center", cursor: "pointer", zIndex: 10, transition: "all var(--duration-fast)",
};

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
    <div className="chat-agent-prose" data-quotable="true" style={{ alignSelf: "stretch", width: "100%", color: "var(--text-primary)", fontSize: "13px", lineHeight: "1.6", padding: "4px 0", wordBreak: "break-word" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "6px", marginBottom: "4px", fontSize: "11px", color: "var(--text-muted)" }}>
        <BrandIcon name="sparkles" size={12} color="var(--agent-accent)" />
        <span style={{ fontWeight: 600, color: "var(--text-secondary)" }}>{t("chat.agentTitle", "cowok Agent")}</span>
        <span>•</span>
        <span style={{ fontFamily: "var(--font-mono)", fontSize: "10px" }}>{msg.timestamp}</span>
      </div>
      <div style={{ paddingLeft: "2px" }}>{msg.text}</div>
    </div>
  );
};

export const AgentChatStream: React.FC<AgentChatStreamProps> = ({
  messages,
  showScrollBottom,
  onScrollMessages,
  onScrollToBottom,
  messagesEndRef,
}) => {
  const { t } = useTranslation();
  const scrollLabel = t("chat.scrollToBottom", "Cuộn xuống cuối trang");

  return (
    <div onScroll={onScrollMessages} className="chat-message-stream" data-testid="chat-message-stream" style={STREAM_CONTAINER_STYLE}>
      {messages.map((msg) => <ChatStreamItem key={msg.id} msg={msg} />)}
      {showScrollBottom && (
        <button type="button" data-testid="chat-scroll-bottom-btn" onClick={onScrollToBottom} title={scrollLabel} aria-label={scrollLabel} style={SCROLL_BTN_STYLE}>
          <BrandIcon name="arrow-down" size={13} color="currentColor" />
        </button>
      )}
      <div ref={messagesEndRef} />
    </div>
  );
};
