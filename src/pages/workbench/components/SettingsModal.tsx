// @story 04.1
import React, { useEffect, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { savePersistedSettings } from "../hooks/useWorkbenchSessionSettings";
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

function useSettingsDialogState(isOpen: boolean, onClose: () => void, activeTab: SettingsTab) {
  const [selectedTab, setSelectedTab] = useState<SettingsTab>(activeTab);
  const [defaultModel, setDefaultModel] = useState("cowok-swarm");
  const [byosEndpoint, setByosEndpoint] = useState("http://localhost:11434/v1");
  const [ephemeralKeys, setEphemeralKeys] = useState({ gemini: "", claude: "", openai: "", byosToken: "" });
  const [showKeys, setShowKeys] = useState(false);
  const [savedFeedback, setSavedFeedback] = useState(false);

  useEffect(() => { if (activeTab) setSelectedTab(activeTab); }, [activeTab]);
  useEffect(() => {
    if (!isOpen) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [isOpen, onClose]);

  return { selectedTab, setSelectedTab, defaultModel, setDefaultModel, byosEndpoint, setByosEndpoint, ephemeralKeys, setEphemeralKeys, showKeys, setShowKeys, savedFeedback, setSavedFeedback };
}

export const SettingsModal: React.FC<SettingsModalProps> = (p) => {
  const { t } = useTranslation();
  const s = useSettingsDialogState(p.isOpen, p.onClose, p.activeTab || "general");
  if (!p.isOpen) return null;

  const handleSave = () => {
    savePersistedSettings({ theme: p.currentTheme, locale: p.currentLocale, fontSize: p.currentFontSize, defaultModel: s.defaultModel, byosEndpoint: s.byosEndpoint });
    s.setSavedFeedback(true);
    setTimeout(() => { s.setSavedFeedback(false); p.onClose(); }, 260);
  };
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
            />
          ) : (
            <SettingsMcpAutomationsPane selectedTab={s.selectedTab} />
          )}
        </div>
        <div style={{ padding: "10px 18px", borderTop: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
          <span data-testid="settings-saved-toast" style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--state-success)" }}>{s.savedFeedback ? t("settings.savedConfirm", "Đã lưu cấu hình thành công") : ""}</span>
          <div style={{ display: "flex", gap: "8px" }}>
            <button type="button" onClick={p.onClose} className="cowok-btn cowok-btn-secondary" style={{ padding: "6px 14px", fontSize: "12px", fontWeight: 500, borderRadius: "var(--radius-xs)", cursor: "pointer" }}>{closeText}</button>
            <button type="button" data-testid="settings-save-btn" onClick={handleSave} className="cowok-btn cowok-btn-primary" style={{ padding: "6px 16px", fontSize: "12px", fontWeight: 600, borderRadius: "var(--radius-xs)", cursor: "pointer" }}>{t("settings.saveBtn", "Lưu thay đổi")}</button>
          </div>
        </div>
      </div>
    </div>
  );
};
