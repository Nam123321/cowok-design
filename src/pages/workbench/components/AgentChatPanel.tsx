// @story 04.1, 04.4
// @implements AC-02, AC-04 (Left-Open Chat Drawer with Node Scope Chips, Zero useEffect)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { apiChatVault } from "../../../api/chat-vault";
import { DecisionEngineOnboardingCard } from "../../../features/chat-vault/DecisionEngineOnboardingCard";
import { useFormatter } from "../../../hooks/useFormatter";
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

export interface ChatProposalControls {
  selectedNodeId?: string | null;
  selectedNodeKey?: string | null;
  proposalsCount: number;
  isDesignerTalkOpen: boolean;
  isDrawerOpen: boolean;
  onToggleDesignerTalk: () => void;
  onToggleDrawer: () => void;
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
  attachmentSnippets?: Record<string, string>;
  onRemoveAttachmentChip?: (chip: string) => void;
  onSendMessage?: (text: string) => void;
  onAttachClick?: () => void;
  onAttachFiles?: (fileNames: string[], snippetsByName?: Record<string, string>) => void;
  onClearAttachments?: () => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
  proposalControls?: ChatProposalControls;
}

const RAIL_ICON_SIZE = 14;
const SCOPE_PILL_MAX_CHARS = 5;

const CollapsedProposalRailActions: React.FC<{
  proposalControls: ChatProposalControls;
}> = ({ proposalControls }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const talkLabel = `${t("proposals.suggestionLabel", "Gợi ý")} (T)`;
  const propLabel = `${t("proposals.proposalLabel", "Đề xuất")} (${formatNumber(proposalControls.proposalsCount)})`;
  const scopeText = proposalControls.selectedNodeKey || proposalControls.selectedNodeId;
  const batchLabel = t("proposals.scopeBatch", "BATCH");

  return (
    <>
      <button type="button" data-testid="toggle-designer-talk-btn" aria-pressed={proposalControls.isDesignerTalkOpen} title={talkLabel} aria-label={talkLabel} onClick={proposalControls.onToggleDesignerTalk} style={{ width: "28px", height: "28px", padding: 0, display: "flex", alignItems: "center", justifyContent: "center", borderRadius: "var(--radius-xs)", border: proposalControls.isDesignerTalkOpen ? "1px solid var(--action-primary)" : "1px solid var(--border-subtle)", backgroundColor: proposalControls.isDesignerTalkOpen ? "var(--bg-selected)" : "var(--bg-surface)", color: proposalControls.isDesignerTalkOpen ? "var(--text-primary)" : "var(--text-secondary)", cursor: "pointer" }}>
        <BrandIcon name="message-square" size={RAIL_ICON_SIZE} color="currentColor" />
      </button>
      <button type="button" data-testid="toggle-change-proposal-drawer-btn" aria-pressed={proposalControls.isDrawerOpen} title={propLabel} aria-label={propLabel} onClick={proposalControls.onToggleDrawer} style={{ position: "relative", width: "28px", height: "28px", padding: 0, display: "flex", alignItems: "center", justifyContent: "center", borderRadius: "var(--radius-xs)", border: proposalControls.isDrawerOpen ? "1px solid var(--action-primary)" : "1px solid var(--border-subtle)", backgroundColor: proposalControls.isDrawerOpen ? "var(--bg-selected)" : "var(--bg-surface)", color: proposalControls.isDrawerOpen ? "var(--text-primary)" : "var(--text-secondary)", cursor: "pointer" }}>
        <BrandIcon name="layers" size={RAIL_ICON_SIZE} color="currentColor" />
        <span style={{ position: "absolute", top: "-4px", right: "-4px", minWidth: "14px", height: "14px", padding: "0 3px", borderRadius: "var(--radius-pill)", backgroundColor: proposalControls.proposalsCount > 0 ? "var(--action-primary)" : "var(--bg-elevated)", color: proposalControls.proposalsCount > 0 ? "var(--action-primary-fg)" : "var(--text-secondary)", border: "1px solid var(--border-default)", fontSize: "9px", fontWeight: 700, display: "inline-flex", alignItems: "center", justifyContent: "center", lineHeight: 1 }}>
          {formatNumber(proposalControls.proposalsCount)}
        </span>
      </button>
      <span className="cp-scope-pill" data-testid="canvas-active-scope-pill" title={scopeText ? `${t("proposals.scopeNode", "NODE")}: ${scopeText}` : batchLabel} style={{ maxWidth: "34px", padding: "1px 4px", fontSize: "9px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        {scopeText ? String(scopeText).slice(0, SCOPE_PILL_MAX_CHARS) : t("proposals.scopeBatchShort", "ALL")}
      </span>
    </>
  );
};

const ChatCollapsedRail: React.FC<{
  onToggleShorten: () => void;
  proposalControls?: ChatProposalControls;
  attachmentsCount?: number;
}> = ({ onToggleShorten, proposalControls, attachmentsCount = 0 }) => {
  const { t } = useTranslation();
  const expandLabel = t("chat.expandChat", "Mở rộng khung Chat");
  return (
    <aside className="workbench-chat-panel-shortened" data-testid="chat-collapsed-rail" aria-label={t("chat.shortenChat", "Khung trò chuyện thu gọn")} style={{ width: "40px", minWidth: "40px", height: "100%", display: "flex", flexDirection: "column", alignItems: "center", gap: "6px", backgroundColor: "var(--bg-workspace)", borderRight: "1px solid var(--border-subtle)", boxSizing: "border-box", flexShrink: 0, padding: "8px 0", zIndex: 4 }}>
      <button type="button" onClick={onToggleShorten} title={expandLabel} aria-label={expandLabel} className="btn-shorten-chat cowok-btn cowok-btn-ghost" style={{ position: "relative", width: "28px", height: "28px", padding: 0, display: "flex", alignItems: "center", justifyContent: "center", color: "var(--text-secondary)", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-surface)" }}>
        <BrandIcon name="panel-left-open" size={RAIL_ICON_SIZE} color="currentColor" />
        {attachmentsCount > 0 && (
          <span
            data-testid="chat-collapsed-attachments-badge"
            style={{ position: "absolute", top: "-5px", right: "-5px", minWidth: "15px", height: "15px", padding: "0 4px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", fontSize: "9px", fontWeight: 700, display: "inline-flex", alignItems: "center", justifyContent: "center", boxSizing: "border-box" }}
          >
            {attachmentsCount}
          </span>
        )}
      </button>
      {proposalControls && <CollapsedProposalRailActions proposalControls={proposalControls} />}
      <div onClick={onToggleShorten} style={{ writingMode: "vertical-rl", transform: "rotate(180deg)", marginTop: "8px", fontSize: "11px", fontWeight: 600, color: "var(--text-muted)", letterSpacing: "0.04em", userSelect: "none", cursor: "pointer" }}>
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
            <button type="button" onClick={(e) => { e.stopPropagation(); onDeselectNode(n.id); }} aria-label={`${t("floatingChat.deselectNode", "Bỏ chọn")} ${n.screenKey}`} style={{ border: "none", background: "transparent", color: "var(--text-muted)", cursor: "pointer", width: "16px", height: "16px", minWidth: "16px", padding: "2px", display: "inline-flex", alignItems: "center", justifyContent: "center", flexShrink: 0 }}>
              <BrandIcon name="close" size={10} color="currentColor" />
            </button>
          )}
        </span>
      ))}
    </div>
  );
};

const AgentChatHeader: React.FC<{
  onToggleShorten: () => void;
  onToggleVaultOnboarding: () => void;
  isVaultOnboardingOpen: boolean;
  proposalControls?: ChatProposalControls;
}> = ({ onToggleShorten, onToggleVaultOnboarding, isVaultOnboardingOpen, proposalControls }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const shortenLabel = t("chat.shortenChat", "Thu gọn khung Chat");
  const scopeText = proposalControls?.selectedNodeKey || proposalControls?.selectedNodeId;
  return (
    <div style={{ display: "flex", flexDirection: "column", borderBottom: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)" }}>
      <div style={{ height: "34px", padding: "0 8px 0 10px", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", flexWrap: "nowrap" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "6px", minWidth: 0, overflow: "hidden" }}>
          <span style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)", whiteSpace: "nowrap" }}>{t("chat.agentTitle", "cowok Agent")}</span>
          {proposalControls && (
            <span className="cp-scope-pill" data-testid="canvas-active-scope-pill" style={{ whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
              {scopeText ? `${t("proposals.scopeNode", "NODE")}: ${scopeText}` : t("proposals.scopeBatch", "BATCH")}
            </span>
          )}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
          <button type="button" onClick={onToggleVaultOnboarding} data-testid="toggle-vault-onboarding-btn" aria-pressed={isVaultOnboardingOpen} title={t("vault.onboardingToggle", "API Key Vault")} style={{ height: "26px", padding: "0 6px", fontSize: "10.5px", border: "1px solid var(--border-subtle)", backgroundColor: isVaultOnboardingOpen ? "var(--bg-selected)" : "var(--bg-surface)", color: "var(--text-secondary)", borderRadius: "var(--radius-xs)", cursor: "pointer" }}>
            {t("vault.onboardingToggle", "API Key Vault")}
          </button>
          <button type="button" onClick={onToggleShorten} className="btn-shorten-chat cowok-btn cowok-btn-ghost" title={shortenLabel} aria-label={shortenLabel} style={{ width: "26px", height: "26px", minWidth: "26px", flexShrink: 0, padding: 0, display: "flex", alignItems: "center", justifyContent: "center", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-surface)", color: "var(--text-secondary)", borderRadius: "var(--radius-xs)", cursor: "pointer" }}>
            <BrandIcon name="panel-left-close" size={RAIL_ICON_SIZE} color="currentColor" />
          </button>
        </div>
      </div>
      {proposalControls && (
        <div style={{ padding: "4px 10px 6px", display: "flex", alignItems: "center", gap: "6px", borderTop: "1px solid var(--border-subtle)", flexWrap: "nowrap" }}>
          <button type="button" className={`cp-toolbar-btn${proposalControls.isDesignerTalkOpen ? " cp-toolbar-btn--active" : ""}`} data-testid="toggle-designer-talk-btn" aria-pressed={proposalControls.isDesignerTalkOpen} title={`${t("proposals.suggestionTooltip", "Gợi ý thiết kế & Đánh đổi")} (T)`} onClick={proposalControls.onToggleDesignerTalk}>
            <span>{t("proposals.suggestionLabel", "Gợi ý")}</span>
          </button>
          <button type="button" className={`cp-toolbar-btn${proposalControls.isDrawerOpen ? " cp-toolbar-btn--active" : ""}`} data-testid="toggle-change-proposal-drawer-btn" aria-pressed={proposalControls.isDrawerOpen} title={`${t("proposals.proposalTooltip", "Danh sách Đề xuất thay đổi")} (${formatNumber(proposalControls.proposalsCount)})`} onClick={proposalControls.onToggleDrawer}>
            <span>{t("proposals.proposalLabel", "Đề xuất")}</span>
          </button>
        </div>
      )}
    </div>
  );
};

export const AgentChatPanel: React.FC<AgentChatPanelProps> = (props) => {
  const { t } = useTranslation();
  const [showVaultOnboarding, setShowVaultOnboarding] = useState(false);
  const chat = useAgentChatConversation({
    attachmentChips: props.attachmentChips || [],
    onSendMessage: props.onSendMessage,
    projectId: props.projectId,
    activeSessionId: props.activeSessionId,
    selectedNodes: props.selectedNodes,
    attachmentSnippets: props.attachmentSnippets,
    onClearAttachments: props.onClearAttachments,
  });

  if (props.isShortened) {
    return <ChatCollapsedRail onToggleShorten={props.onToggleShorten} proposalControls={props.proposalControls} attachmentsCount={props.attachmentChips?.length || 0} />;
  }

  return (
    <aside className="workbench-chat-panel" aria-label={t("chat.agentTitle", "cowok Agent")} style={{ width: `${props.width}px`, height: "100%", display: "flex", flexDirection: "column", backgroundColor: "var(--bg-canvas)", borderRight: "1px solid var(--border-subtle)", boxSizing: "border-box", flexShrink: 0, position: "relative", fontFamily: "var(--font-sans)" }}>
      <AgentChatHeader onToggleShorten={props.onToggleShorten} onToggleVaultOnboarding={() => setShowVaultOnboarding((v) => !v)} isVaultOnboardingOpen={showVaultOnboarding} proposalControls={props.proposalControls} />
      {showVaultOnboarding && (
        <div style={{ padding: "8px 10px", borderBottom: "1px solid var(--border-subtle)" }}>
          <DecisionEngineOnboardingCard enterpriseLocalOnlyLock={chat.enterpriseLocalOnlyLock} onToggleLocalOnlyLock={chat.setEnterpriseLocalOnlyLock} />
        </div>
      )}
      <LeftChatScopeChips projectId={props.projectId} sessionId={props.activeSessionId} selectedNodes={props.selectedNodes || []} onDeselectNode={props.onDeselectNode} onFocusNode={props.onFocusNode} />
      <AgentChatStream messages={chat.messages} showScrollBottom={chat.showScrollBottom} onScrollMessages={chat.handleScrollMessages} onScrollToBottom={chat.scrollToBottom} messagesEndRef={chat.messagesEndRef} onSelectSdiVariant={(intentId, pid, variantId) => { void chat.handleSelectSdiVariant(intentId, pid, variantId); }} onForkFromMessage={(msg) => { void chat.handleForkFromMessage(msg); }} onReplaceVaultReceipt={chat.handleReplaceVaultReceipt} onRevokeVaultReceipt={chat.handleRevokeVaultReceipt} />
      {chat.chatVaultError && (
        <div data-testid="agent-chat-vault-error" style={{ padding: "6px 12px", fontSize: "11px", color: "var(--state-danger)", backgroundColor: "var(--bg-subtle)", borderTop: "1px solid var(--border-subtle)" }}>
          {chat.chatVaultError}
        </div>
      )}
      <AgentChatComposer
        inputValue={chat.inputValue}
        onInputChange={chat.setInputValue}
        onSend={chat.handleSend}
        attachmentChips={props.attachmentChips || []}
        onRemoveAttachmentChip={props.onRemoveAttachmentChip}
        onAttachClick={props.onAttachClick}
        onAttachFiles={props.onAttachFiles}
        onPasteBinaryFiles={(files) => {
          const sid = props.activeSessionId || "session-1";
          chat.setChatVaultError(null);
          for (const f of files) {
            void apiChatVault
              .uploadAttachment({ sessionId: sid, projectId: props.projectId ?? null, file: f, filename: f.name })
              .catch((err) => { chat.setChatVaultError(err instanceof Error ? err.message : t("cas.uploadFailed", "Upload failed")); });
          }
        }}
        onOpenSettings={props.onOpenSettings}
      />
    </aside>
  );
};
