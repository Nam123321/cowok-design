// @story 04.6
// @implements AC-04 (DecisionEngineOnboardingCard [5.29] with Official Guide Links)
import React from "react";
import { useTranslation } from "@/hooks/useTranslation";

export interface DecisionEngineOnboardingCardProps {
  enterpriseLocalOnlyLock: boolean;
  onToggleLocalOnlyLock: (next: boolean) => void;
  onSelectProviderHint?: (provider: "google_ai_studio" | "typesafe_jev") => void;
}

const WEIGHT_SEMIBOLD = 600;
const RELAXED_LINE_HEIGHT = 1.4;

const CARD_STYLE: React.CSSProperties = {
  padding: "10px 12px",
  borderRadius: "var(--radius-md)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-surface)",
  display: "flex",
  flexDirection: "column",
  gap: "8px",
};

const LINK_BTN_STYLE: React.CSSProperties = {
  minHeight: "32px",
  padding: "4px 10px",
  fontSize: "11px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-subtle)",
  color: "var(--action-primary)",
  textDecoration: "none",
  display: "inline-flex",
  alignItems: "center",
};

const ProviderGuideLinks: React.FC<{
  onSelectProviderHint?: (provider: "google_ai_studio" | "typesafe_jev") => void;
}> = ({ onSelectProviderHint }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", flexWrap: "wrap", gap: "8px" }}>
      <a href="https://aistudio.google.com/app/apikey" target="_blank" rel="noopener noreferrer" onClick={() => onSelectProviderHint?.("google_ai_studio")} style={LINK_BTN_STYLE}>
        {t("vault.googleAiStudioKey", "Google AI Studio Key")}
      </a>
      <a href="https://console.typesafe.ai" target="_blank" rel="noopener noreferrer" onClick={() => onSelectProviderHint?.("typesafe_jev")} style={LINK_BTN_STYLE}>
        {t("vault.typesafeJevKey", "TypeSafe JEV Key")}
      </a>
    </div>
  );
};

export const DecisionEngineOnboardingCard: React.FC<DecisionEngineOnboardingCardProps> = ({
  enterpriseLocalOnlyLock,
  onToggleLocalOnlyLock,
  onSelectProviderHint,
}) => {
  const { t } = useTranslation();
  return (
    <section data-ff-id="decision-engine-onboarding-card" data-testid="decision-engine-onboarding-card" style={CARD_STYLE}>
      <div style={{ fontSize: "12px", fontWeight: WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
        {t("vault.onboardingTitle", "Decision Engine API Key Onboarding (Zero-Leak Vault)")}
      </div>
      <div style={{ fontSize: "11px", color: "var(--text-secondary)", lineHeight: RELAXED_LINE_HEIGHT }}>
        {t("vault.onboardingDesc", "Get an API key from Google AI Studio or TypeSafe JEV and paste it into chat. Keys are encrypted with AES-256-GCM before storage.")}
      </div>
      <ProviderGuideLinks onSelectProviderHint={onSelectProviderHint} />
      <label style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "11px", color: "var(--text-secondary)", cursor: "pointer" }}>
        <input type="checkbox" data-testid="enterprise-local-only-toggle" checked={enterpriseLocalOnlyLock} onChange={(e) => onToggleLocalOnlyLock(e.target.checked)} />
        <span>{t("vault.localOnlyLabel", "Enterprise Local-Only Lock (no outbound probe)")}</span>
      </label>
    </section>
  );
};
