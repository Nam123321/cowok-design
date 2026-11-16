// @story 04.1, 04.4
// @implements AC-02, AC-04 (Left-Open Chat Drawer with Node Scope Chips, Zero useEffect)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { useAgentChatConversation } from "../hooks/useAgentChatConversation";
import { AgentChatComposer } from "./AgentChatComposer";
import { AgentChatStream, type ChatMessage } from "./AgentChatStream";

export type { ChatMessage };

export interface ScopeNodeChip {
  id: string;
  screenKey: string;
  title: string;
}

export interface AgentChatPanelProps {
  projectId?: string;
  activeSessionId?: string;
  selectedNodes?: ScopeNodeChip[];
  onDeselectNode?: (nodeId: string) => void;
  onFocusNode?: (nodeId: string) => void;
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
      className="workbench-chat-panel-shortened"
      data-testid="chat-collapsed-rail"
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

const LeftChatScopeChips: React.FC<{
  projectId?: string;
  sessionId?: string;
  selectedNodes: ScopeNodeChip[];
  onDeselectNode?: (nodeId: string) => void;
  onFocusNode?: (nodeId: string) => void;
}> = ({ projectId = "default-project", sessionId = "session-1", selectedNodes, onDeselectNode, onFocusNode }) => {
  const { t } = useTranslation();
  if (selectedNodes.length === 0) return null;
  return (
    <div
      className="left-chat-scope-chips"
      data-testid="left-chat-scope-chips"
      style={{ padding: "5px 10px", backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-subtle)", display: "flex", alignItems: "center", gap: "5px", flexWrap: "wrap" }}
    >
      {selectedNodes.map((n) => (
        <span
          key={n.id}
          title={`${t("floatingChat.compositeScopePrefix", "Composite Scope")}: ${projectId}::${sessionId}::${n.id}`}
          style={{ display: "inline-flex", alignItems: "center", gap: "4px", fontSize: "10.5px", fontWeight: 500, backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", borderRadius: "var(--radius-pill)", padding: "2px 8px", color: "var(--text-primary)" }}
        >
          <BrandIcon name="crosshair" size={10} color="var(--action-primary)" />
          <span style={{ cursor: "pointer" }} onClick={() => onFocusNode?.(n.id)}>
            {n.screenKey}
          </span>
          {onDeselectNode && (
            <button type="button" onClick={() => onDeselectNode(n.id)} aria-label={`${t("floatingChat.deselectNode", "Bỏ chọn")} ${n.screenKey}`} style={{ border: "none", background: "transparent", color: "var(--text-muted)", cursor: "pointer", padding: 0, display: "inline-flex" }}>
              <BrandIcon name="close" size={10} color="currentColor" />
            </button>
          )}
        </span>
      ))}
    </div>
  );
};

export const AgentChatPanel: React.FC<AgentChatPanelProps> = (props) => {
  const { t } = useTranslation();
  const chat = useAgentChatConversation(props.attachmentChips || [], props.onSendMessage, props.projectId, props.activeSessionId, props.selectedNodes);

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
      <LeftChatScopeChips projectId={props.projectId} sessionId={props.activeSessionId} selectedNodes={props.selectedNodes || []} onDeselectNode={props.onDeselectNode} onFocusNode={props.onFocusNode} />
      <AgentChatStream messages={chat.messages} showScrollBottom={chat.showScrollBottom} onScrollMessages={chat.handleScrollMessages} onScrollToBottom={chat.scrollToBottom} messagesEndRef={chat.messagesEndRef} />
      <AgentChatComposer inputValue={chat.inputValue} onInputChange={chat.setInputValue} onSend={chat.handleSend} attachmentChips={props.attachmentChips || []} onRemoveAttachmentChip={props.onRemoveAttachmentChip} onAttachClick={props.onAttachClick} onOpenSettings={props.onOpenSettings} />
    </aside>
  );
};
