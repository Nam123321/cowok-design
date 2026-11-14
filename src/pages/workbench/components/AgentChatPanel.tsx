// @story 04.1
// cowok-design — Canonical Agent Chat Panel (PD-02=A 40px Rail + Seamless 6-Line Rolling Composer)
import React, { useEffect, useMemo, useRef, useState } from "react";
import { apiSdiEngine } from "../../../api/sdi-engine";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { AgentChatComposer } from "./AgentChatComposer";
import { AgentChatStream, type ChatMessage } from "./AgentChatStream";

export type { ChatMessage };

export interface AgentChatPanelProps {
  projectId?: string;
  activeSessionId?: string;
  isShortened: boolean;
  onToggleShorten: () => void;
  width: number;
  attachmentChips?: string[];
  onRemoveAttachmentChip?: (chip: string) => void;
  onSendMessage?: (text: string) => void;
  onAttachClick?: () => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
}

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

function buildCompiledIntentReply(
  intent: {
    intentType: string;
    status: string;
    ambiguityScore: number;
    clarificationVariants?: Array<{ title: string }>;
    targetScope: { screenKeys: string[] };
  },
  formatNumber: (n: number) => string,
  t: (k: string, defVal?: string) => string
): string {
  const ambiguityPct = formatNumber(Math.round(intent.ambiguityScore * 100));
  const variants = intent.clarificationVariants?.map((v) => v.title).join(" / ") || t("chat.selectOptionPrompt", "Vui lòng chọn phương án");
  const scopeKeys = intent.targetScope.screenKeys.join(", ") || "project";
  if (intent.status === "clarifying") {
    return `${t("chat.clarifyPrefix", "Cần làm rõ yêu cầu")} (${intent.intentType}, ${t("chat.ambiguityLabel", "ambiguity")} ${ambiguityPct}%): ${variants}`;
  }
  return `${t("chat.compiledIntentPrefix", "Đã biên dịch intent")} ${intent.intentType} (${intent.status}) — ${t("chat.scopeLabel", "phạm vi")}: ${scopeKeys}.`;
}

function useAgentChatConversation(
  attachmentChips: string[],
  onSendMessage?: (text: string) => void,
  projectId?: string,
  activeSessionId?: string
) {
  const { t, locale } = useTranslation();
  const { formatTime, formatNumber } = useFormatter({ locale });
  const sessionKey = activeSessionId || (projectId ? `project:${projectId}` : "default");
  const initialGreeting: ChatMessage = useMemo(
    () => ({
      id: "msg-1",
      sender: "agent",
      text: t(
        "chat.welcomeGreeting",
        "Chào bạn! Tôi là cowok Agent. Tôi đã sẵn sàng đồng hành cùng bạn xây dựng hệ thống thiết kế và luồng người dùng 2D DAG. Bạn có thể kéo thả đỉnh trên đồ thị, kết nối các màn hình hoặc bôi đen chữ để Quote nhanh vào khung nhập liệu."
      ),
      timestamp: formatTime(new Date().setHours(10, 0, 0, 0)),
    }),
    [t, formatTime]
  );
  const [messagesBySession, setMessagesBySession] = useState<Record<string, ChatMessage[]>>({});
  const [draftBySession, setDraftBySession] = useState<Record<string, string>>({});
  const [showScrollBottom, setShowScrollBottom] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const messages = useMemo(
    () => messagesBySession[sessionKey] ?? [initialGreeting],
    [messagesBySession, sessionKey, initialGreeting]
  );
  const inputValue = draftBySession[sessionKey] ?? "";
  const setInputValue = (val: string) => {
    setDraftBySession((prev) => ({ ...prev, [sessionKey]: val }));
  };

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView?.({ behavior: "smooth" });
  }, [messages, sessionKey]);

  const appendMessageToSession = (targetKey: string, msg: ChatMessage) => {
    setMessagesBySession((prev) => {
      const base = prev[targetKey] ?? [initialGreeting];
      return { ...prev, [targetKey]: [...base, msg] };
    });
  };

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
    const targetSessionKey = sessionKey;
    const sentPrompt = inputValue.trim() || attachmentChips.join(", ") || t("chat.attachedFallback", "Đính kèm");
    const humanMsg: ChatMessage = { id: `msg-${Date.now()}`, sender: "human", text: inputValue || sentPrompt, timestamp: formatTime(new Date()) };
    appendMessageToSession(targetSessionKey, humanMsg);
    onSendMessage?.(inputValue);
    setDraftBySession((prev) => ({ ...prev, [targetSessionKey]: "" }));

    if (!projectId) return;
    void apiSdiEngine
      .compileIntent(projectId, { rawPrompt: sentPrompt })
      .then((intent) => {
        const replyText = buildCompiledIntentReply(intent, formatNumber, t);
        appendMessageToSession(targetSessionKey, {
          id: `msg-agent-${intent.id}`,
          sender: "agent",
          text: replyText,
          timestamp: formatTime(new Date()),
        });
      })
      .catch((err) => {
        const errText = err instanceof Error ? err.message : t("chat.daemonConnectError", "Không thể kết nối Rust daemon");
        appendMessageToSession(targetSessionKey, {
          id: `msg-err-${Date.now()}`,
          sender: "agent",
          text: `${t("chat.daemonSyncError", "Lỗi đồng bộ SDI daemon")}: ${errText}`,
          timestamp: formatTime(new Date()),
        });
      });
  };

  return { messages, inputValue, setInputValue, showScrollBottom, messagesEndRef, handleScrollMessages, scrollToBottom, handleSend };
}

export const AgentChatPanel: React.FC<AgentChatPanelProps> = (props) => {
  const { t } = useTranslation();
  const chat = useAgentChatConversation(props.attachmentChips || [], props.onSendMessage, props.projectId, props.activeSessionId);

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
