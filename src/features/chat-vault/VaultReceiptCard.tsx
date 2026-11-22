// @story 04.6
// @implements AC-04, AC-05 (VaultReceiptCard [5.15] with Presenter Mask Blur & 44px Controls)
import React, { useState } from "react";
import type { VaultReceiptDto } from "@/lib/validations/chat-vault";
import { useTranslation } from "@/hooks/useTranslation";

export interface VaultReceiptCardProps {
  receipt: VaultReceiptDto;
  onReplace?: (receiptId: string, newSecret: string) => Promise<void>;
  onRevoke?: (receiptId: string) => Promise<void>;
}

const MIN_SECRET_INPUT_LEN = 8;
const WEIGHT_SEMIBOLD = 600;

const BTN_44_STYLE: React.CSSProperties = {
  minHeight: "44px",
  minWidth: "var(--hit-target-min, 44px)",
  padding: "0 12px",
  fontSize: "11px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-subtle)",
  cursor: "pointer",
};

const VaultReplaceRow: React.FC<{
  value: string;
  onChange: (next: string) => void;
  onConfirm: () => void;
}> = ({ value, onChange, onConfirm }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", gap: "6px", alignItems: "center" }}>
      <input
        type="password"
        data-testid="vault-replace-input"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={t("vault.replacePlaceholder", "Enter new API key")}
        style={{ flex: 1, minHeight: "36px", padding: "4px 8px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-canvas)", color: "var(--text-primary)" }}
      />
      <button type="button" data-testid="vault-replace-confirm-btn" onClick={onConfirm} style={{ ...BTN_44_STYLE, backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", border: "none" }}>
        {t("vault.confirm", "Save")}
      </button>
    </div>
  );
};

const VaultReceiptHeader: React.FC<{
  receipt: VaultReceiptDto;
  isWarn: boolean;
}> = ({ receipt, isWarn }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px" }}>
      <span style={{ fontSize: "11px", fontWeight: WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>{receipt.provider}</span>
      <div style={{ display: "flex", alignItems: "center", gap: "4px", flexWrap: "wrap" }}>
        {receipt.localOnly && (
          <span data-testid="vault-receipt-local-only-badge" style={{ fontSize: "10px", padding: "2px 6px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)" }}>
            {t("vault.localOnlyBadge", "Local-Only")}
          </span>
        )}
        {receipt.highEntropyWarning && (
          <span data-testid="vault-receipt-entropy-badge" style={{ fontSize: "10px", padding: "2px 6px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--amber-500)", color: "var(--action-primary-fg)" }}>
            {t("vault.highEntropyBadge", "High-Entropy")}
          </span>
        )}
        <span data-testid="vault-receipt-status-pill" style={{ fontSize: "10px", padding: "2px 6px", borderRadius: "var(--radius-pill)", backgroundColor: isWarn ? "var(--amber-500)" : "var(--bg-subtle)", color: isWarn ? "var(--action-primary-fg)" : "var(--state-success)" }}>
          {receipt.status}
        </span>
      </div>
    </div>
  );
};

export const VaultReceiptCard: React.FC<VaultReceiptCardProps> = ({ receipt, onReplace, onRevoke }) => {
  const { t } = useTranslation();
  const [revealed, setRevealed] = useState(false);
  const [replacing, setReplacing] = useState(false);
  const [replacementSecret, setReplacementSecret] = useState("");
  const isWarn = receipt.status === "RATE_LIMITED_429" || receipt.status === "INVALID";

  const handleConfirmReplace = async () => {
    const trimmed = replacementSecret.trim();
    if (trimmed.length < MIN_SECRET_INPUT_LEN || !onReplace) return;
    setReplacementSecret("");
    setReplacing(false);
    await onReplace(receipt.receiptId, trimmed);
  };

  return (
    <div
      data-ff-id="vault-receipt-card"
      data-testid="vault-receipt-card"
      style={{ padding: "8px 10px", borderRadius: "var(--radius-md)", border: isWarn ? "1px solid var(--amber-500)" : "1px solid var(--border-vault-verified)", backgroundColor: "var(--bg-surface)", display: "flex", flexDirection: "column", gap: "6px" }}
    >
      <VaultReceiptHeader receipt={receipt} isWarn={isWarn} />
      <div data-testid="vault-receipt-mask" onMouseEnter={() => setRevealed(true)} onMouseLeave={() => setRevealed(false)} style={{ fontFamily: "var(--font-mono)", fontSize: "11px", color: "var(--text-secondary)", filter: revealed ? "none" : "var(--privacy-mask-filter, blur(4px))", userSelect: "none" }}>
        {receipt.presenterMask}
      </div>
      {replacing && <VaultReplaceRow value={replacementSecret} onChange={setReplacementSecret} onConfirm={() => void handleConfirmReplace()} />}
      <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
        {onReplace && (
          <button type="button" data-testid="vault-receipt-replace-btn" onClick={() => setReplacing((v) => !v)} style={{ ...BTN_44_STYLE, color: "var(--text-primary)" }}>
            {t("vault.replace", "Replace")}
          </button>
        )}
        {onRevoke && (
          <button type="button" data-testid="vault-receipt-revoke-btn" onClick={() => void onRevoke(receipt.receiptId)} style={{ ...BTN_44_STYLE, color: "var(--state-danger)" }}>
            {t("vault.revoke", "Revoke")}
          </button>
        )}
      </div>
    </div>
  );
};
