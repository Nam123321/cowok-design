// @story 04.1
// cowok-design — Canonical Agent Chat Panel (PD-02=A 40px Rail + Seamless 6-Line Rolling Composer)
import React, { useEffect, useRef, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { AgentChatComposer } from "./AgentChatComposer";
import { AgentChatStream, type ChatMessage } from "./AgentChatStream";

export type { ChatMessage };

export interface AgentChatPanelProps {
  isShortened: boolean;
  onToggleShorten: () => void;
  width: number;
  attachmentChips?: string[];
  onRemoveAttachmentChip?: (chip: string) => void;
  onSendMessage?: (text: string) => void;
  onAttachClick?: () => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
}

const DEFAULT_MESSAGES: ChatMessage[] = [
  {
    id: "msg-1",
    sender: "agent",
    text: "Chào bạn! Tôi là cowok Agent. Tôi đã sẵn sàng đồng hành cùng bạn xây dựng hệ thống thiết kế và luồng người dùng 2D DAG. Bạn có thể kéo thả đỉnh trên đồ thị, kết nối các màn hình hoặc bôi đen chữ để Quote nhanh vào khung nhập liệu.",
    timestamp: "10:00",
  },
];

const ChatCollapsedRail: React.FC<{ onToggleShorten: () => void }> = ({ onToggleShorten }) => {
  const { t } = useTranslation();
  const expandLabel = t("chat.expandChat", "Mở rộng khung Chat");
  return (
    <aside
      className="workbench-chat-panel-shortened" data-testid="chat-collapsed-rail"
      aria-label={t("chat.shortenChat", "Khung trò chuyện thu gọn")}
      style={{ width: "40px", minWidth: "40px", height: "100%", display: "flex", flexDirection: "column", alignItems: "center", backgroundColor: "var(--bg-workspace)", borderRight: "1px solid var(--border-subtle)", boxSizing: "border-box", flexShrink: 0, padding: "8px 0", zIndex: 4 }}
    >
      <button type="button" onClick={onToggleShorten} title={expandLabel} aria-label={expandLabel} className="btn-shorten-chat cowok-btn cowok-btn-ghost" style={{ width: "28px", height: "28px", padding: 0, display: "flex", alignItems: "center", justifyContent: "center", color: "var(--text-secondary)", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-surface)" }}>
        <BrandIcon name="panel-left-open" size={14} color="currentColor" />
      </button>
      <div onClick={onToggleShorten} style={{ writingMode: "vertical-rl", transform: "rotate(180deg)", marginTop: "14px", fontSize: "11px", fontWeight: 600, color: "var(--text-muted)", letterSpacing: "0.04em", userSelect: "none", cursor: "pointer" }}>
        {t("chat.agentTitle", "cowok Agent")}
      </div>
    </aside>
  );
};

function useAgentChatConversation(
  attachmentChips: string[],
  onSendMessage?: (text: string) => void
) {
  const { formatTime } = useFormatter();
  const [messages, setMessages] = useState<ChatMessage[]>(DEFAULT_MESSAGES);
  const [inputValue, setInputValue] = useState("");
  const [showScrollBottom, setShowScrollBottom] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => { messagesEndRef.current?.scrollIntoView?.({ behavior: "smooth" }); }, [messages]);

  const handleScrollMessages = (e: React.UIEvent<HTMLDivElement>) => {
    const { scrollTop, scrollHeight, clientHeight } = e.currentTarget;
    setShowScrollBottom(scrollHeight - scrollTop - clientHeight > 60);
  };
  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView?.({ behavior: "smooth" });
    setShowScrollBottom(false);
  };
  const handleSend = () => {
    if (!inputValue.trim() && attachmentChips.length === 0) return;
    const humanMsg: ChatMessage = { id: `msg-${Date.now()}`, sender: "human", text: inputValue, timestamp: formatTime(new Date()) };
    setMessages((prev) => [...prev, humanMsg]);
    onSendMessage?.(inputValue);
    const sentPrompt = inputValue || "Đính kèm";
    setInputValue("");
    setTimeout(() => {
      setMessages((prev) => [
        ...prev,
        { id: `msg-agent-${Date.now()}`, sender: "agent", text: `Đã ghi nhận yêu cầu: "${sentPrompt}". Tôi đang phân tích đồ thị 2D DAG và cập nhật trực tiếp lên khung Live HTML Preview bên phải.`, timestamp: formatTime(new Date()) },
      ]);
    }, 450);
  };

  return { messages, inputValue, setInputValue, showScrollBottom, messagesEndRef, handleScrollMessages, scrollToBottom, handleSend };
}

export const AgentChatPanel: React.FC<AgentChatPanelProps> = (props) => {
  const { t } = useTranslation();
  const chat = useAgentChatConversation(props.attachmentChips || [], props.onSendMessage);

  if (props.isShortened) return <ChatCollapsedRail onToggleShorten={props.onToggleShorten} />;
  const shortenLabel = t("chat.shortenChat", "Thu gọn khung Chat");

  return (
    <aside className="workbench-chat-panel" aria-label={t("chat.agentTitle", "cowok Agent")} style={{ width: `${props.width}px`, height: "100%", display: "flex", flexDirection: "column", backgroundColor: "var(--bg-canvas)", borderRight: "1px solid var(--border-subtle)", boxSizing: "border-box", flexShrink: 0, position: "relative", fontFamily: "var(--font-sans)" }}>
      <div style={{ height: "36px", minHeight: "36px", padding: "0 10px 0 12px", display: "flex", alignItems: "center", justifyContent: "space-between", borderBottom: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)" }}>
        <span style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)" }}>{t("chat.agentTitle", "cowok Agent")}</span>
        <button type="button" onClick={props.onToggleShorten} className="btn-shorten-chat cowok-btn cowok-btn-ghost" title={shortenLabel} aria-label={shortenLabel} style={{ width: "28px", height: "28px", minWidth: "28px", padding: 0, display: "flex", alignItems: "center", justifyContent: "center", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", color: "var(--text-secondary)", borderRadius: "var(--radius-xs)", cursor: "pointer" }}>
          <BrandIcon name="panel-left-close" size={14} color="currentColor" />
        </button>
      </div>
      <AgentChatStream messages={chat.messages} showScrollBottom={chat.showScrollBottom} onScrollMessages={chat.handleScrollMessages} onScrollToBottom={chat.scrollToBottom} messagesEndRef={chat.messagesEndRef} />
      <AgentChatComposer inputValue={chat.inputValue} onInputChange={chat.setInputValue} onSend={chat.handleSend} attachmentChips={props.attachmentChips || []} onRemoveAttachmentChip={props.onRemoveAttachmentChip} onAttachClick={props.onAttachClick} onOpenSettings={props.onOpenSettings} />
    </aside>
  );
};
