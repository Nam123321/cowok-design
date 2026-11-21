// @story 04.6
// @implements AC-01, AC-02, AC-04, AC-05 (Persistent Chat Stream, Secret Interceptor & Vault Receipt Composer Bridge)
import React, { useEffect, useState } from "react";
import { apiChatVault } from "@/api/chat-vault";
import { useTranslation } from "@/hooks/useTranslation";
import type { CasAttachmentDto, ChatMessageDto, VaultReceiptDto } from "@/lib/validations/chat-vault";
import { CasDropzonePanel } from "./CasDropzonePanel";
import { DecisionEngineOnboardingCard } from "./DecisionEngineOnboardingCard";
import { VaultReceiptCard } from "./VaultReceiptCard";
import { inspectChatSecretCandidate } from "./secret-interceptor";

export interface ChatVaultBridgeProps {
  sessionId: string;
  projectId?: string | null;
  showOnboarding?: boolean;
}

const INPUT_STYLE: React.CSSProperties = {
  width: "100%",
  minHeight: "44px",
  padding: "8px",
  borderRadius: "var(--radius-sm)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-surface)",
  color: "var(--text-primary)",
};

const BridgeStreamView: React.FC<{
  receipts: VaultReceiptDto[];
  messages: ChatMessageDto[];
  attachments: CasAttachmentDto[];
  onReplaceReceipt: (rid: string, nextSecret: string) => Promise<void>;
  onRevokeReceipt: (rid: string) => Promise<void>;
}> = ({ receipts, messages, attachments, onReplaceReceipt, onRevokeReceipt }) => (
  <>
    {receipts.map((receipt) => (
      <VaultReceiptCard key={receipt.receiptId} receipt={receipt} onReplace={onReplaceReceipt} onRevoke={onRevokeReceipt} />
    ))}
    {messages.map((msg) => (
      <div key={msg.id} data-testid={`sqlite-chat-msg-${msg.id}`} style={{ fontSize: "12px", color: "var(--text-primary)" }}>{msg.proseMarkdown}</div>
    ))}
    {attachments.map((att) => (
      <div key={att.id} data-testid={`cas-attachment-pill-${att.id}`} style={{ fontSize: "11px", color: "var(--text-secondary)" }}>{att.filename} ({att.fileExt})</div>
    ))}
  </>
);

async function submitChatBridgeTurn(params: {
  sessionId: string;
  projectId?: string | null;
  rawText: string;
  localOnly: boolean;
  onReceipt: (r: VaultReceiptDto) => void;
  onMessage: (m: ChatMessageDto) => void;
}): Promise<void> {
  const inspected = inspectChatSecretCandidate(params.rawText);
  if (inspected.detected && inspected.detectedSecrets.length > 0) {
    const receipts: VaultReceiptDto[] = [];
    for (const entry of inspected.detectedSecrets) {
      const receipt = await apiChatVault.ingestVaultSecret({
        sessionId: params.sessionId,
        projectId: params.projectId ?? null,
        provider: entry.provider,
        rawSecretEnvelope: entry.rawSecretEnvelope,
        enterpriseLocalOnlyLock: params.localOnly,
      });
      receipts.push(receipt);
      params.onReceipt(receipt);
    }
    const persisted = await apiChatVault.appendMessage(params.sessionId, {
      role: "user",
      proseMarkdown: inspected.redactedPreview,
      projectId: params.projectId ?? null,
      vaultReceipt: receipts[0],
    });
    params.onMessage(persisted);
    for (const extra of receipts.slice(1)) {
      const follow = await apiChatVault.appendMessage(params.sessionId, {
        role: "user",
        proseMarkdown: `[VAULTED:${extra.provider}] ${extra.presenterMask}`,
        projectId: params.projectId ?? null,
        vaultReceipt: extra,
      });
      params.onMessage(follow);
    }
    return;
  }
  const saved = await apiChatVault.appendMessage(params.sessionId, { role: "user", proseMarkdown: params.rawText, projectId: params.projectId ?? null });
  params.onMessage(saved);
}

function useChatBridgeSessionState(sessionId: string, projectId: string | null | undefined, fallbackErr: string) {
  const [messages, setMessages] = useState<ChatMessageDto[]>([]);
  const [attachments, setAttachments] = useState<CasAttachmentDto[]>([]);
  const [receipts, setReceipts] = useState<VaultReceiptDto[]>([]);
  const [draft, setDraft] = useState("");
  const [localOnly, setLocalOnly] = useState(false);
  const [statusError, setStatusError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void apiChatVault.getMessages(sessionId).then((res) => {
      if (!active) return;
      setMessages(res.messages);
      setReceipts(res.messages.map((m) => m.vaultReceipt).filter((r): r is VaultReceiptDto => Boolean(r)));
    }).catch(() => { if (active) setStatusError(null); });
    return () => { active = false; };
  }, [sessionId]);

  const sendDraft = async (rawText: string) => {
    const trimmed = rawText.trim();
    if (!trimmed) return;
    setDraft("");
    setStatusError(null);
    try {
      await submitChatBridgeTurn({
        sessionId,
        projectId,
        rawText: trimmed,
        localOnly,
        onReceipt: (r) => setReceipts((p) => [...p, r]),
        onMessage: (m) => setMessages((p) => [...p, m]),
      });
    } catch (err) {
      setStatusError(err instanceof Error ? err.message : fallbackErr);
    }
  };
  return { messages, attachments, setAttachments, receipts, setReceipts, draft, setDraft, localOnly, setLocalOnly, statusError, setStatusError, sendDraft };
}

const ChatComposerForm: React.FC<{
  draft: string;
  placeholder: string;
  onChange: (value: string) => void;
  onPaste: (e: React.ClipboardEvent<HTMLTextAreaElement>) => void;
  onSend: (rawText: string) => Promise<void>;
}> = ({ draft, placeholder, onChange, onPaste, onSend }) => (
  <form data-testid="chat-composer-form" onSubmit={(e) => { e.preventDefault(); void onSend(draft); }}>
    <textarea
      data-ff-id="chat-composer-input"
      data-testid="chat-vault-composer-input"
      placeholder={placeholder}
      value={draft}
      onChange={(e) => onChange(e.target.value)}
      onPaste={onPaste}
      onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); void onSend(draft); } }}
      style={INPUT_STYLE}
    />
  </form>
);

export const ChatPrimitivesBridge: React.FC<ChatVaultBridgeProps> = ({ sessionId, projectId, showOnboarding = false }) => {
  const { t } = useTranslation();
  const st = useChatBridgeSessionState(sessionId, projectId, t("chat.persistFailed", "Persistence failed"));
  const handlePaste = (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
    const files = Array.from(e.clipboardData?.files || []);
    if (files.length === 0) return;
    e.preventDefault();
    files.forEach((f) => void apiChatVault.uploadAttachment({ sessionId, projectId: projectId ?? null, file: f, filename: f.name }).then((dto) => st.setAttachments((p) => [...p, dto])).catch((err) => st.setStatusError(err instanceof Error ? err.message : t("cas.uploadFailed", "Upload failed"))));
  };
  return (
    <div data-testid="chat-vault-bridge" style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
      {showOnboarding && <DecisionEngineOnboardingCard enterpriseLocalOnlyLock={st.localOnly} onToggleLocalOnlyLock={st.setLocalOnly} />}
      <BridgeStreamView
        receipts={st.receipts}
        messages={st.messages}
        attachments={st.attachments}
        onReplaceReceipt={async (rid, nextSecret) => {
          const cur = st.receipts.find((r) => r.receiptId === rid);
          const inferred = inspectChatSecretCandidate(nextSecret).provider;
          const provider = (inferred || cur?.provider || "custom") as "google_ai_studio" | "typesafe_jev" | "anthropic" | "openai" | "custom";
          const updated = await apiChatVault.replaceVaultReceipt(rid, { provider, rawSecretEnvelope: nextSecret, enterpriseLocalOnlyLock: st.localOnly });
          st.setReceipts((p) => p.map((r) => (r.receiptId === rid ? updated : r)));
        }}
        onRevokeReceipt={async (rid) => {
          await apiChatVault.revokeVaultReceipt(rid);
          st.setReceipts((p) => p.filter((r) => r.receiptId !== rid));
        }}
      />
      {st.statusError && <div data-testid="chat-vault-error" style={{ fontSize: "11px", color: "var(--state-danger)" }}>{st.statusError}</div>}
      <CasDropzonePanel sessionId={sessionId} projectId={projectId} onUploaded={(dto) => st.setAttachments((p) => [...p, dto])} onError={st.setStatusError} />
      <ChatComposerForm
        draft={st.draft}
        placeholder={t("chat.vaultComposerPlaceholder", "Ask cowok Agent or paste API key...")}
        onChange={st.setDraft}
        onPaste={handlePaste}
        onSend={st.sendDraft}
      />
    </div>
  );
};
