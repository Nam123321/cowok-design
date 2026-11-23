// @story 04.1, 04.2, 08.4
import React, { useEffect, useRef } from "react";
import { apiWorkbenchSettings } from "../../../api/workbench-settings";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { DEFAULT_FLOW_PROJECT_ID } from "../constants";
import { useSettingsDialogState, type SettingsTab } from "../hooks/useSettingsModalState";
import { savePersistedSettings } from "../hooks/useWorkbenchSessionSettings";
import { FONT_SIZES, SettingsGeneralModelsPane, SUPPORTED_LOCALES } from "./SettingsGeneralModelsPane";
import { SettingsMcpAutomationsPane } from "./SettingsMcpAutomationsPane";
import type { ThemeMode } from "./WorkbenchTopNav";

export { FONT_SIZES, SUPPORTED_LOCALES };
export type { SettingsTab };

const Z_INDEX_MODAL = 9999;
const TAB_ICON_SIZE = 13;
const TITLE_ICON_SIZE = 16;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_MEDIUM = 500;
const SAVE_CLOSE_DELAY_MS = 260;
const DISABLED_BTN_OPACITY = 0.7;

export interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  projectId?: string;
  currentTheme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
  currentLocale: string;
  onLocaleChange: (locale: string) => void;
  currentFontSize: string;
  onFontSizeChange: (fontSize: string) => void;
  activeTab?: SettingsTab;
}

const OVERLAY_STYLE: React.CSSProperties = {
  position: "fixed", inset: 0, backgroundColor: "var(--bg-overlay)", backdropFilter: "blur(4px)",
  display: "flex", alignItems: "center", justifyContent: "center", zIndex: Z_INDEX_MODAL, padding: "16px",
};

const DIALOG_STYLE: React.CSSProperties = {
  width: "100%", maxWidth: "620px", backgroundColor: "var(--bg-surface)", borderRadius: "var(--radius-lg)",
  border: "1px solid var(--border-default)", boxShadow: "var(--shadow-lg)", overflow: "hidden",
  display: "flex", flexDirection: "column", fontFamily: "var(--font-sans)",
};

const SettingsTabStrip: React.FC<{
  selectedTab: SettingsTab;
  onSelectTab: (tab: SettingsTab) => void;
}> = ({ selectedTab, onSelectTab }) => {
  const { t } = useTranslation();
  const tabs: { id: SettingsTab; label: string; icon: string }[] = [
    { id: "general", label: t("settings.tabGeneral", "Chung"), icon: "settings" },
    { id: "models", label: t("settings.tabModels", "Mô hình AI"), icon: "llm-config" },
    { id: "mcp", label: t("settings.tabMcp", "MCP & Tiện ích"), icon: "plugin" },
    { id: "automations", label: t("settings.tabAutomations", "Tự động hóa"), icon: "automation" },
  ];
  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", padding: "0 12px", backgroundColor: "var(--bg-subtle)", borderBottom: "1px solid var(--border-subtle)" }}>
      {tabs.map((tab) => {
        const active = selectedTab === tab.id;
        return (
          <button key={tab.id} type="button" onClick={() => onSelectTab(tab.id)} style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: "6px", padding: "10px 8px", fontSize: "11.5px", fontWeight: active ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM, color: active ? "var(--text-primary)" : "var(--text-secondary)", borderBottom: active ? "2px solid var(--action-primary)" : "2px solid transparent", backgroundColor: active ? "var(--bg-selected)" : "transparent", borderTop: "none", borderLeft: "none", borderRight: "none", cursor: "pointer" }}>
            <BrandIcon name={tab.icon} size={TAB_ICON_SIZE} color="currentColor" />
            <span>{tab.label}</span>
          </button>
        );
      })}
    </div>
  );
};

const SettingsModalFooter: React.FC<{
  savedFeedback: boolean;
  isSaving: boolean;
  syncWarning: string | null;
  showSaveButton: boolean;
  closeText: string;
  onClose: () => void;
  onSave: () => void;
}> = ({ savedFeedback, isSaving, syncWarning, showSaveButton, closeText, onClose, onSave }) => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "10px 18px", borderTop: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <span data-testid="settings-saved-toast" style={{ fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--state-success)" }}>{savedFeedback ? t("settings.savedConfirm", "Đã lưu cấu hình thành công") : ""}</span>
        {syncWarning && !savedFeedback && (
          <span data-testid="settings-sync-warning" style={{ fontSize: "11px", color: "var(--state-warning)" }}>{t("settings.daemonSyncWarning", "Không thể đồng bộ cấu hình với Rust daemon; đang dùng dữ liệu cục bộ.")}</span>
        )}
      </div>
      <div style={{ display: "flex", gap: "8px" }}>
        <button type="button" onClick={onClose} className="cowok-btn cowok-btn-secondary" style={{ padding: "6px 14px", fontSize: "12px", fontWeight: FONT_WEIGHT_MEDIUM, borderRadius: "var(--radius-xs)", cursor: "pointer" }}>{closeText}</button>
        {showSaveButton && (
          <button type="button" data-testid="settings-save-btn" disabled={isSaving} onClick={onSave} className="cowok-btn cowok-btn-primary" style={{ padding: "6px 16px", fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, borderRadius: "var(--radius-xs)", cursor: isSaving ? "wait" : "pointer", opacity: isSaving ? DISABLED_BTN_OPACITY : 1 }}>{t("settings.saveBtn", "Lưu thay đổi")}</button>
        )}
      </div>
    </div>
  );
};

const VAULT_MODE_OPTIONS = new Set<"keychain" | "vault_0600" | "ram_only" | "ephemeral">([
  "keychain",
  "vault_0600",
  "ram_only",
  "ephemeral",
]);

function normalizeVaultMode(raw: string): "keychain" | "vault_0600" | "ram_only" | "ephemeral" {
  return VAULT_MODE_OPTIONS.has(raw as "keychain")
    ? (raw as "keychain" | "vault_0600" | "ram_only" | "ephemeral")
    : "vault_0600";
}

function buildSavePayload(s: ReturnType<typeof useSettingsDialogState>, resolvedCredentialRef?: string) {
  return {
    providerId: s.defaultModel || "cowok-swarm",
    modelId: s.defaultModel || "cowok-swarm",
    endpointUrl: s.byosEndpoint || "",
    credentialRef: resolvedCredentialRef || (s.vaultMode ? `vault://${normalizeVaultMode(s.vaultMode)}/${s.defaultModel || "cowok-swarm"}` : "vault://vault_0600/cowok-swarm"),
    status: s.permissionMode || "auto_sandbox",
    permissionMode: s.permissionMode || "auto_sandbox",
    vaultMode: normalizeVaultMode(s.vaultMode || "vault_0600"),
    onboardingDone: s.onboardingDone,
    isDefault: true,
  };
}

async function executeSaveSettings(
  p: SettingsModalProps,
  s: ReturnType<typeof useSettingsDialogState>,
  closeTimerRef: React.MutableRefObject<ReturnType<typeof setTimeout> | null>
) {
  if (s.isSaving) return;
  s.setIsSaving(true);
  savePersistedSettings({ theme: p.currentTheme, locale: p.currentLocale, fontSize: p.currentFontSize, defaultModel: s.defaultModel, byosEndpoint: s.byosEndpoint });
  try {
    const resolvedVaultMode = normalizeVaultMode(s.vaultMode || "vault_0600");
    await apiWorkbenchSettings.updateVaultConfig({ vaultMode: resolvedVaultMode });

    let primaryVaultRef: string | undefined;
    const providerKeyEntries: Array<{ providerId: string; secret: string }> = [
      { providerId: "gemini", secret: s.ephemeralKeys.gemini.trim() },
      { providerId: "anthropic", secret: s.ephemeralKeys.claude.trim() },
      { providerId: "openai", secret: s.ephemeralKeys.openai.trim() },
      { providerId: "openrouter", secret: (s.ephemeralKeys.openrouter ?? "").trim() },
      { providerId: "deepseek", secret: (s.ephemeralKeys.deepseek ?? "").trim() },
      { providerId: "byos", secret: s.ephemeralKeys.byosToken.trim() },
    ];
    for (const entry of providerKeyEntries) {
      if (!entry.secret) continue;
      const stored = await apiWorkbenchSettings.upsertVaultKey({
        providerId: entry.providerId,
        apiKey: entry.secret,
        vaultMode: resolvedVaultMode,
      });
      if (!primaryVaultRef && stored.vaultRef) {
        primaryVaultRef = stored.vaultRef;
      }
    }

    await apiWorkbenchSettings.upsertAgentSetting(buildSavePayload(s, primaryVaultRef));
    s.setSyncWarning(null);
    s.setSavedFeedback(true);
    if (closeTimerRef.current) clearTimeout(closeTimerRef.current);
    closeTimerRef.current = setTimeout(() => { s.setSavedFeedback(false); p.onClose(); }, SAVE_CLOSE_DELAY_MS);
  } catch (err: unknown) {
    s.setSavedFeedback(false);
    s.setSyncWarning(err instanceof Error ? err.message : "save_daemon_failed");
  } finally {
    s.setIsSaving(false);
  }
}

export const SettingsModal: React.FC<SettingsModalProps> = (p) => {
  const { t } = useTranslation();
  const s = useSettingsDialogState(p.isOpen, p.onClose, p.activeTab || "general");
  const closeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => {
    if (closeTimerRef.current) clearTimeout(closeTimerRef.current);
  }, []);
  if (!p.isOpen) return null;

  const titleText = t("settings.title", "Cài đặt & Tùy biến Hệ thống");
  const closeText = t("settings.closeBtn", "Đóng");
  const isFormTab = s.selectedTab === "general" || s.selectedTab === "models";

  return (
    <div role="dialog" aria-modal="true" aria-label={titleText} data-ff-id="settings-modal" style={OVERLAY_STYLE} onClick={p.onClose}>
      <div style={DIALOG_STYLE} onClick={(e) => e.stopPropagation()}>
        <div style={{ padding: "13px 18px", borderBottom: "1px solid var(--border-subtle)", display: "flex", alignItems: "center", justifyContent: "space-between", backgroundColor: "var(--bg-workspace)" }}>
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <BrandIcon name="settings" size={TITLE_ICON_SIZE} color="currentColor" />
            <h2 style={{ margin: 0, fontSize: "14px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>{titleText}</h2>
          </div>
          <button type="button" onClick={p.onClose} aria-label={closeText} className="cowok-btn-ghost" style={{ border: "none", background: "transparent", cursor: "pointer", fontSize: "18px", color: "var(--text-muted)", lineHeight: 1, padding: "4px" }}>×</button>
        </div>
        <SettingsTabStrip selectedTab={s.selectedTab} onSelectTab={s.setSelectedTab} />
        <div style={{ padding: "18px 20px", maxHeight: "480px", overflowY: "auto", display: "flex", flexDirection: "column", gap: "16px" }}>
          {(s.selectedTab === "general" || s.selectedTab === "models") ? (
            <SettingsGeneralModelsPane
              selectedTab={s.selectedTab} currentTheme={p.currentTheme} onThemeChange={p.onThemeChange}
              currentLocale={p.currentLocale} onLocaleChange={p.onLocaleChange}
              currentFontSize={p.currentFontSize} onFontSizeChange={p.onFontSizeChange}
              defaultModel={s.defaultModel} onDefaultModelChange={s.setDefaultModel}
              byosEndpoint={s.byosEndpoint} onByosEndpointChange={s.setByosEndpoint}
              ephemeralApiKeys={s.ephemeralKeys} onEphemeralKeyChange={(k, v) => s.setEphemeralKeys((prev) => ({ ...prev, [k]: v }))}
              showKeys={s.showKeys} onToggleShowKeys={() => s.setShowKeys((v) => !v)}
              onAgentPrefsChange={(v, pm) => { s.setVaultMode(v); s.setPermissionMode(pm); }}
            />
          ) : (
            <SettingsMcpAutomationsPane
              selectedTab={s.selectedTab}
              projectId={p.projectId || DEFAULT_FLOW_PROJECT_ID}
            />
          )}
        </div>
        <SettingsModalFooter savedFeedback={s.savedFeedback} isSaving={s.isSaving} syncWarning={s.syncWarning} showSaveButton={isFormTab} closeText={closeText} onClose={p.onClose} onSave={() => void executeSaveSettings(p, s, closeTimerRef)} />
      </div>
    </div>
  );
};
