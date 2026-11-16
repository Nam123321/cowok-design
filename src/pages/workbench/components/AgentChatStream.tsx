// @story 04.1, 04.4
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { ChatStreamItem, type ChatMessage } from "@/components/workbench/ChatPrimitivesBridge";

export { ChatStreamItem, type ChatMessage };

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
