// @story 04.4
// @implements AC-02 (Shared Chat Stream Item & Model Constants in components/)
import React from "react";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { useTranslation } from "@/hooks/useTranslation";
import { VaultReceiptCard } from "@/features/chat-vault/VaultReceiptCard";
import type { VaultReceiptDto } from "@/lib/validations/chat-vault";

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
  vaultReceipt?: VaultReceiptDto | null;
  sdiClarification?: SdiClarificationCardData;
}

export const AVAILABLE_CHAT_MODELS = [
  { id: "cowok-swarm", label: "Cowok Swarm · High" },
  { id: "claude-37", label: "Claude 3.7 Sonnet" },
  { id: "gemini-25", label: "Gemini 2.5 Pro" },
];

const FORK_ICON_SIZE = 11;
const WEIGHT_SEMIBOLD = 600;

const ForkFromTurnButton: React.FC<{
  msg: ChatMessage;
  onForkFromMessage?: (msg: ChatMessage) => void;
}> = ({ msg, onForkFromMessage }) => {
  const { t } = useTranslation();
  if (!onForkFromMessage || msg.id === "msg-1") return null;
  const label = t("chat.forkFromHere", "Rẽ nhánh hội thoại từ đây");
  return (
    <button
      type="button"
      data-testid={`chat-fork-msg-btn-${msg.id}`}
      title={label}
      aria-label={label}
      onClick={(e) => {
        e.stopPropagation();
        onForkFromMessage(msg);
      }}
      style={{
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        width: "20px",
        height: "20px",
        padding: 0,
        borderRadius: "var(--radius-xs)",
        border: "1px solid transparent",
        background: "transparent",
        color: "var(--text-muted)",
        cursor: "pointer",
        flexShrink: 0,
      }}
    >
      <BrandIcon name="git-branch" size={FORK_ICON_SIZE} color="currentColor" />
    </button>
  );
};

const SdiClarificationBox: React.FC<{
  sdi: SdiClarificationCardData;
  onSelectSdiVariant?: (intentId: string, projectId: string, variantId: string) => void;
}> = ({ sdi, onSelectSdiVariant }) => {
  const { t } = useTranslation();
  if (!sdi.variants.length) return null;
  return (
    <div data-testid="sdi-clarification-card" style={{ marginTop: "8px", padding: "10px", borderRadius: "var(--radius-sm)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-default)", display: "flex", flexDirection: "column", gap: "6px" }}>
      <div style={{ fontSize: "11px", fontWeight: WEIGHT_SEMIBOLD, color: "var(--text-secondary)" }}>
        {sdi.status === "ready" ? t("sdi.clarificationResolved", "Đã chọn hướng thiết kế:") : t("sdi.chooseVariantPrompt", "Chọn 1 phương án để mở khóa thực thi:")}
      </div>
      {sdi.variants.map((v) => {
        const isPicked = sdi.selectedVariantId === v.id;
        return (
          <button key={v.id} type="button" data-testid={`sdi-variant-btn-${v.id}`} disabled={sdi.status === "ready"} onClick={() => onSelectSdiVariant?.(sdi.intentId, sdi.projectId, v.id)} style={{ textAlign: "left", padding: "7px 9px", borderRadius: "var(--radius-xs)", border: isPicked ? "1.5px solid var(--action-primary)" : "1px solid var(--border-default)", backgroundColor: isPicked ? "var(--bg-selected)" : "var(--bg-surface)", color: "var(--text-primary)", cursor: sdi.status === "ready" ? "default" : "pointer", display: "flex", flexDirection: "column", gap: "2px" }}>
            <span style={{ fontSize: "11.5px", fontWeight: WEIGHT_SEMIBOLD }}>{v.title}</span>
            <span style={{ fontSize: "10.5px", color: "var(--text-secondary)" }}>{v.description}</span>
          </button>
        );
      })}
    </div>
  );
};

export const ChatStreamItem: React.FC<{
  msg: ChatMessage;
  onSelectSdiVariant?: (intentId: string, projectId: string, variantId: string) => void;
  onForkFromMessage?: (msg: ChatMessage) => void;
  onReplaceVaultReceipt?: (receiptId: string, newSecret: string) => Promise<void>;
  onRevokeVaultReceipt?: (receiptId: string) => Promise<void>;
}> = ({ msg, onSelectSdiVariant, onForkFromMessage, onReplaceVaultReceipt, onRevokeVaultReceipt }) => {
  const { t } = useTranslation();
  if (msg.sender === "human") {
    return (
      <div style={{ alignSelf: "flex-end", maxWidth: "86%", display: "flex", flexDirection: "column", alignItems: "flex-end", gap: "4px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
          <ForkFromTurnButton msg={msg} onForkFromMessage={onForkFromMessage} />
          <div className="chat-human-bubble" data-testid="chat-human-bubble" style={{ backgroundColor: "var(--human-soft)", color: "var(--text-primary)", border: "1px solid var(--human-border)", padding: "8px 12px", borderRadius: "12px 12px 2px 12px", fontSize: "12.5px", lineHeight: "1.5", wordBreak: "break-word" }}>
            {msg.text}
          </div>
        </div>
        {msg.vaultReceipt && <VaultReceiptCard receipt={msg.vaultReceipt} onReplace={onReplaceVaultReceipt} onRevoke={onRevokeVaultReceipt} />}
      </div>
    );
  }
  return (
    <div className="chat-agent-prose" data-quotable="true" style={{ alignSelf: "stretch", width: "100%", color: "var(--text-primary)", fontSize: "13px", lineHeight: "1.6", padding: "4px 0", wordBreak: "break-word" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "6px", marginBottom: "4px", fontSize: "11px", color: "var(--text-muted)" }}>
        <span style={{ fontWeight: WEIGHT_SEMIBOLD, color: "var(--text-secondary)" }}>{t("chat.agentTitle", "cowok Agent")}</span>
        <span>•</span>
        <span style={{ fontFamily: "var(--font-mono)", fontSize: "10px" }}>{msg.timestamp}</span>
        <span style={{ marginLeft: "auto" }}><ForkFromTurnButton msg={msg} onForkFromMessage={onForkFromMessage} /></span>
      </div>
      <div style={{ paddingLeft: "2px" }}>{msg.text}</div>
      {msg.vaultReceipt && <VaultReceiptCard receipt={msg.vaultReceipt} onReplace={onReplaceVaultReceipt} onRevoke={onRevokeVaultReceipt} />}
      {msg.sdiClarification && <SdiClarificationBox sdi={msg.sdiClarification} onSelectSdiVariant={onSelectSdiVariant} />}
    </div>
  );
};
