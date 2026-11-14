// @story 04.1, 04.2
import React, { useEffect, useState } from "react";
import { apiWorkbenchSettings } from "../../../api/workbench-settings";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { loadPersistedSettings, savePersistedSettings } from "../hooks/useWorkbenchSessionSettings";
import { FONT_SIZES, SettingsGeneralModelsPane, SUPPORTED_LOCALES } from "./SettingsGeneralModelsPane";
import { SettingsMcpAutomationsPane } from "./SettingsMcpAutomationsPane";
import type { ThemeMode } from "./WorkbenchTopNav";

export { FONT_SIZES, SUPPORTED_LOCALES };
export type SettingsTab = "general" | "models" | "mcp" | "automations";

export interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
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
  display: "flex", alignItems: "center", justifyContent: "center", zIndex: 9999, padding: "16px",
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
    { id: "models", label: t("settings.tabModels", "Mô hình & BYOS"), icon: "llm-config" },
    { id: "mcp", label: t("settings.tabMcp", "MCP & Tiện ích"), icon: "plugin" },
    { id: "automations", label: t("settings.tabAutomations", "Tự động hóa"), icon: "automation" },
  ];
  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", padding: "0 12px", backgroundColor: "var(--bg-subtle)", borderBottom: "1px solid var(--border-subtle)" }}>
      {tabs.map((tab) => {
        const active = selectedTab === tab.id;
        return (
          <button key={tab.id} type="button" onClick={() => onSelectTab(tab.id)} style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: "6px", padding: "10px 8px", fontSize: "11.5px", fontWeight: active ? 600 : 500, color: active ? "var(--text-primary)" : "var(--text-secondary)", borderBottom: active ? "2px solid var(--action-primary)" : "2px solid transparent", backgroundColor: active ? "var(--bg-selected)" : "transparent", borderTop: "none", borderLeft: "none", borderRight: "none", cursor: "pointer" }}>
            <BrandIcon name={tab.icon} size={13} color="currentColor" />
            <span>{tab.label}</span>
          </button>
        );
      })}
    </div>
  );
};

function applyDefaultAgentSettingRow(
  rows: Awaited<ReturnType<typeof apiWorkbenchSettings.listAgentSettings>>,
  setters: {
    setDefaultModel: (v: string) => void;
    setByosEndpoint: (v: string) => void;
    setVaultMode: (v: string) => void;
    setPermissionMode: (v: string) => void;
    setOnboardingDone: (v: boolean) => void;
  }
) {
  if (rows.length === 0) return;
  const def = rows.find((r) => r.isDefault) || rows[0];
  if (def.modelId) setters.setDefaultModel(def.modelId);
  if (def.endpointUrl) setters.setByosEndpoint(def.endpointUrl);
  const nextVault = def.vaultMode || def.credentialRef;
  if (nextVault) setters.setVaultMode(nextVault);
  const nextPerm = def.permissionMode || def.status;
  if (nextPerm) setters.setPermissionMode(nextPerm);
  if (typeof def.onboardingDone === "boolean") setters.setOnboardingDone(def.onboardingDone);
}

function useSettingsDialogState(isOpen: boolean, onClose: () => void, activeTab: SettingsTab) {
  const initial = loadPersistedSettings();
  const [selectedTab, setSelectedTab] = useState<SettingsTab>(activeTab);
  const [defaultModel, setDefaultModel] = useState(initial.defaultModel || "claude-sonnet-4-6");
  const [byosEndpoint, setByosEndpoint] = useState(initial.byosEndpoint || "http://127.0.0.1:43110/v1");
  const [vaultMode, setVaultMode] = useState<string>("vault_0600");
  const [permissionMode, setPermissionMode] = useState<string>("auto_sandbox");
  const [onboardingDone, setOnboardingDone] = useState<boolean>(false);
  const [ephemeralKeys, setEphemeralKeys] = useState({ gemini: "", claude: "", openai: "", byosToken: "" });
  const [showKeys, setShowKeys] = useState(false);
  const [savedFeedback, setSavedFeedback] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [syncWarning, setSyncWarning] = useState<string | null>(null);

  useEffect(() => { if (activeTab) setSelectedTab(activeTab); }, [activeTab]);
  useEffect(() => {
    if (!isOpen) return;
    let active = true;
    void apiWorkbenchSettings.listAgentSettings().then((rows) => {
      if (!active) return;
      setSyncWarning(null);
      applyDefaultAgentSettingRow(rows, { setDefaultModel, setByosEndpoint, setVaultMode, setPermissionMode, setOnboardingDone });
    }).catch((err: unknown) => {
      if (!active) return;
      setSyncWarning(err instanceof Error ? err.message : "daemon_sync_failed");
    });
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKey);
    return () => { active = false; window.removeEventListener("keydown", onKey); };
  }, [isOpen, onClose]);

  return { selectedTab, setSelectedTab, defaultModel, setDefaultModel, byosEndpoint, setByosEndpoint, vaultMode, setVaultMode, permissionMode, setPermissionMode, onboardingDone, setOnboardingDone, ephemeralKeys, setEphemeralKeys, showKeys, setShowKeys, savedFeedback, setSavedFeedback, isSaving, setIsSaving, syncWarning, setSyncWarning };
}

const SettingsModalFooter: React.FC<{
  savedFeedback: boolean;
  isSaving: boolean;
  syncWarning: string | null;
  closeText: string;
  onClose: () => void;
  onSave: () => void;
}> = ({ savedFeedback, isSaving, syncWarning, closeText, onClose, onSave }) => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "10px 18px", borderTop: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <span data-testid="settings-saved-toast" style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--state-success)" }}>{savedFeedback ? t("settings.savedConfirm", "Đã lưu cấu hình thành công") : ""}</span>
        {syncWarning && !savedFeedback && (
          <span data-testid="settings-sync-warning" style={{ fontSize: "11px", color: "var(--state-warning)" }}>{t("settings.daemonSyncWarning", "Không thể đồng bộ cấu hình với Rust daemon; đang dùng dữ liệu cục bộ.")}</span>
        )}
      </div>
      <div style={{ display: "flex", gap: "8px" }}>
        <button type="button" onClick={onClose} className="cowok-btn cowok-btn-secondary" style={{ padding: "6px 14px", fontSize: "12px", fontWeight: 500, borderRadius: "var(--radius-xs)", cursor: "pointer" }}>{closeText}</button>
        <button type="button" data-testid="settings-save-btn" disabled={isSaving} onClick={onSave} className="cowok-btn cowok-btn-primary" style={{ padding: "6px 16px", fontSize: "12px", fontWeight: 600, borderRadius: "var(--radius-xs)", cursor: isSaving ? "wait" : "pointer", opacity: isSaving ? 0.7 : 1 }}>{t("settings.saveBtn", "Lưu thay đổi")}</button>
      </div>
    </div>
  );
};

function buildSavePayload(s: ReturnType<typeof useSettingsDialogState>) {
  return {
    providerId: s.defaultModel || "cowok-swarm",
    modelId: s.defaultModel || "cowok-swarm",
    endpointUrl: s.byosEndpoint || "",
    credentialRef: s.vaultMode || "vault_0600",
    status: s.permissionMode || "auto_sandbox",
    permissionMode: s.permissionMode || "auto_sandbox",
    vaultMode: s.vaultMode || "vault_0600",
    onboardingDone: s.onboardingDone,
    isDefault: true,
  };
}

async function executeSaveSettings(p: SettingsModalProps, s: ReturnType<typeof useSettingsDialogState>) {
  if (s.isSaving) return;
  s.setIsSaving(true);
  savePersistedSettings({ theme: p.currentTheme, locale: p.currentLocale, fontSize: p.currentFontSize, defaultModel: s.defaultModel, byosEndpoint: s.byosEndpoint });
  try {
    await apiWorkbenchSettings.upsertAgentSetting(buildSavePayload(s));
    s.setSyncWarning(null);
    s.setSavedFeedback(true);
    setTimeout(() => { s.setSavedFeedback(false); p.onClose(); }, 260);
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
  if (!p.isOpen) return null;

  const titleText = t("settings.title", "Cài đặt & Tùy biến Hệ thống");
  const closeText = t("settings.closeBtn", "Đóng");

  return (
    <div role="dialog" aria-modal="true" aria-label={titleText} style={OVERLAY_STYLE} onClick={p.onClose}>
      <div style={DIALOG_STYLE} onClick={(e) => e.stopPropagation()}>
        <div style={{ padding: "13px 18px", borderBottom: "1px solid var(--border-subtle)", display: "flex", alignItems: "center", justifyContent: "space-between", backgroundColor: "var(--bg-workspace)" }}>
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <BrandIcon name="settings" size={16} color="currentColor" />
            <h2 style={{ margin: 0, fontSize: "14px", fontWeight: 600, color: "var(--text-primary)" }}>{titleText}</h2>
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
            <SettingsMcpAutomationsPane selectedTab={s.selectedTab} />
          )}
        </div>
        <SettingsModalFooter savedFeedback={s.savedFeedback} isSaving={s.isSaving} syncWarning={s.syncWarning} closeText={closeText} onClose={p.onClose} onSave={() => void executeSaveSettings(p, s)} />
      </div>
    </div>
  );
};
