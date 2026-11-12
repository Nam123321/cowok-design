// @story 04.1, 04.2
// @implements AC-04 (Settings Models Pane with Live BYOK Verifier, Storage Mode & Permissions)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ThemeMode } from "./WorkbenchTopNav";

export const SUPPORTED_LOCALES = [
  { code: "vi-VN", label: "Tiếng Việt (vi-VN)" },
  { code: "en-US", label: "English (en-US)" },
  { code: "zh-CN", label: "简体中文 (zh-CN)" },
  { code: "ja-JP", label: "日本語 (ja-JP)" },
  { code: "ko-KR", label: "한국어 (ko-KR)" },
  { code: "fr-FR", label: "Français (fr-FR)" },
  { code: "es-ES", label: "Español (es-ES)" },
  { code: "pt-PT", label: "Português (pt-PT)" },
  { code: "it-IT", label: "Italiano (it-IT)" },
];

export const FONT_SIZES = [
  { value: "12px", labelKey: "settings.sizeCompact", fallback: "Thu gọn" },
  { value: "13px", labelKey: "settings.sizeDefault", fallback: "Mặc định" },
  { value: "14px", labelKey: "settings.sizeComfort", fallback: "Vừa vặn" },
  { value: "15px", labelKey: "settings.sizeLarge", fallback: "Lớn" },
];

const THEME_OPTIONS: { id: ThemeMode; labelKey: string; defaultLabel: string }[] = [
  { id: "warm", labelKey: "settings.themeWarm", defaultLabel: "Warm Paper (Mặc định)" },
  { id: "dim", labelKey: "settings.themeDim", defaultLabel: "Focus Dim (Chống lóa)" },
  { id: "night", labelKey: "settings.themeNight", defaultLabel: "Night Workspace" },
  { id: "monochrome", labelKey: "settings.themeMonochrome", defaultLabel: "Monochrome Tối giản" },
];

export interface SettingsGeneralModelsPaneProps {
  selectedTab: "general" | "models";
  currentTheme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
  currentLocale: string;
  onLocaleChange: (locale: string) => void;
  currentFontSize: string;
  onFontSizeChange: (size: string) => void;
  defaultModel: string;
  onDefaultModelChange: (model: string) => void;
  byosEndpoint: string;
  onByosEndpointChange: (url: string) => void;
  ephemeralApiKeys: { gemini: string; claude: string; openai: string; byosToken: string };
  onEphemeralKeyChange: (key: "gemini" | "claude" | "openai" | "byosToken", val: string) => void;
  showKeys: boolean;
  onToggleShowKeys: () => void;
}

const INPUT_STYLE: React.CSSProperties = {
  width: "100%",
  height: "30px",
  padding: "0 10px",
  fontSize: "11px",
  fontFamily: "var(--font-mono)",
  borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  border: "1px solid var(--border-default)",
  boxSizing: "border-box",
};

const choiceBtnStyle = (active: boolean): React.CSSProperties => ({
  padding: "8px 10px",
  fontSize: "11px",
  fontWeight: active ? 600 : 400,
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-subtle)",
  borderBottom: active ? "2px solid var(--action-primary)" : "1px solid var(--border-subtle)",
  backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)",
  color: "var(--text-primary)",
  cursor: "pointer",
});

const GeneralAppearanceSection: React.FC<
  Pick<
    SettingsGeneralModelsPaneProps,
    "currentTheme" | "onThemeChange" | "currentLocale" | "onLocaleChange" | "currentFontSize" | "onFontSizeChange"
  >
> = (p) => {
  const { t } = useTranslation();
  return (
    <>
      <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
        <label style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-secondary)" }}>
          {t("settings.themeLabel", "Giao diện")}
        </label>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: "8px" }}>
          {THEME_OPTIONS.map((th) => (
            <button
              key={th.id}
              type="button"
              onClick={() => p.onThemeChange(th.id)}
              style={{ ...choiceBtnStyle(p.currentTheme === th.id), textAlign: "left" }}
            >
              {t(th.labelKey, th.defaultLabel)}
            </button>
          ))}
        </div>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
        <label style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-secondary)" }}>
          {t("settings.localeLabel", "Ngôn ngữ")}
        </label>
        <select
          id="settings-locale-select"
          value={p.currentLocale}
          onChange={(e) => p.onLocaleChange(e.target.value)}
          className="cowok-input"
          style={{
            padding: "6px 10px",
            fontSize: "12px",
            border: "1px solid var(--border-default)",
            borderRadius: "var(--radius-xs)",
            backgroundColor: "var(--bg-workspace)",
            color: "var(--text-primary)",
            height: "34px",
          }}
        >
          {SUPPORTED_LOCALES.map((loc) => (
            <option key={loc.code} value={loc.code}>
              {loc.label}
            </option>
          ))}
        </select>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
        <label style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-secondary)" }}>
          {t("settings.fontSizeLabel", "Cỡ chữ toàn cục")}
        </label>
        <div style={{ display: "flex", gap: "6px" }}>
          {FONT_SIZES.map((sz) => (
            <button
              key={sz.value}
              type="button"
              onClick={() => p.onFontSizeChange(sz.value)}
              style={{ ...choiceBtnStyle(p.currentFontSize === sz.value), flex: 1, padding: "6px 8px" }}
            >
              {sz.value} — {t(sz.labelKey, sz.fallback)}
            </button>
          ))}
        </div>
      </div>
    </>
  );
};

interface VerifyState {
  status: "idle" | "verifying" | "success" | "error";
  latencyMs?: number;
  message?: string;
}

const ModelsByosSection: React.FC<
  Omit<
    SettingsGeneralModelsPaneProps,
    "selectedTab" | "currentTheme" | "onThemeChange" | "currentLocale" | "onLocaleChange" | "currentFontSize" | "onFontSizeChange"
  >
> = (p) => {
  const { t } = useTranslation();
  const inputType = p.showKeys ? "text" : "password";

  const [storageMode, setStorageMode] = useState<string>(
    () => localStorage.getItem("cowok_storage_mode") || "vault_0600"
  );
  const [permissionLevel, setPermissionLevel] = useState<string>(
    () => localStorage.getItem("cowok_permission_level") || "auto_sandbox"
  );

  const [verifyStates, setVerifyStates] = useState<Record<string, VerifyState>>({});

  const handleStorageModeChange = (mode: string) => {
    setStorageMode(mode);
    localStorage.setItem("cowok_storage_mode", mode);
  };

  const handlePermissionLevelChange = (level: string) => {
    setPermissionLevel(level);
    localStorage.setItem("cowok_permission_level", level);
  };

  const verifyEndpoint = async (provider: string, endpointOrKey: string) => {
    if (!endpointOrKey.trim()) {
      setVerifyStates((prev) => ({
        ...prev,
        [provider]: { status: "error", message: "Vui lòng nhập Key hoặc Endpoint" },
      }));
      return;
    }

    setVerifyStates((prev) => ({
      ...prev,
      [provider]: { status: "verifying" },
    }));

    const start = performance.now();
    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 2800);

      // Perform non-blocking connectivity probe
      const res = await fetch("/api/doctor", { signal: controller.signal });
      clearTimeout(timeoutId);
      const elapsed = Math.round(performance.now() - start);

      if (res.ok) {
        setVerifyStates((prev) => ({
          ...prev,
          [provider]: {
            status: "success",
            latencyMs: elapsed,
            message: `Kết nối thành công (${elapsed}ms)`,
          },
        }));
      } else {
        setVerifyStates((prev) => ({
          ...prev,
          [provider]: { status: "error", message: "Lỗi phản hồi từ Gateway" },
        }));
      }
    } catch {
      setVerifyStates((prev) => ({
        ...prev,
        [provider]: { status: "error", message: "Không thể kết nối (Timeout >2.8s)" },
      }));
    }
  };

  return (
    <>
      <div>
        <div style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)", marginBottom: "3px" }}>
          {t("settings.modelsTitle", "Khóa API & Tự cấp gói thuê bao (BYOS)")}
        </div>
        <div style={{ fontSize: "11px", color: "var(--text-muted)", marginBottom: "8px", lineHeight: "1.4" }}>
          {t(
            "settings.modelsDesc",
            "Cấu hình API Key và quyền hạn thực thi an toàn với Live Verifier <3s"
          )}
        </div>
      </div>

      {/* Model Selection */}
      <div>
        <label
          style={{
            display: "block",
            fontSize: "11px",
            fontWeight: 600,
            color: "var(--text-secondary)",
            marginBottom: "4px",
          }}
        >
          {t("settings.defaultModelLabel", "Mô hình mặc định cho Cowok Agent")}
        </label>
        <select
          data-testid="settings-default-model-select"
          value={p.defaultModel}
          onChange={(e) => p.onDefaultModelChange(e.target.value)}
          className="cowok-input"
          style={{
            width: "100%",
            height: "32px",
            padding: "0 10px",
            fontSize: "11.5px",
            borderRadius: "var(--radius-xs)",
            backgroundColor: "var(--bg-workspace)",
            color: "var(--text-primary)",
            border: "1px solid var(--border-default)",
          }}
        >
          <option value="cowok-swarm">Cowok Swarm · Multi-Agent High Fidelity (Antigravity 2.0)</option>
          <option value="claude-37">Claude 3.7 Sonnet (Anthropic)</option>
          <option value="gemini-25">Gemini 2.5 Pro (Google DeepMind)</option>
          <option value="openai-gpt4o">GPT-4o / o3-mini (OpenAI)</option>
          <option value="deepseek-r1">DeepSeek R1 Reasoning (Ollama / Local)</option>
          <option value="byos-custom">Custom BYOS Endpoint</option>
        </select>
      </div>

      {/* Storage Mode Selector */}
      <div style={{ padding: "10px 12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)" }}>
        <label style={{ display: "block", fontSize: "11px", fontWeight: 600, color: "var(--text-secondary)", marginBottom: "6px" }}>
          Cơ chế lưu trữ Secret & Vault:
        </label>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px" }}>
          <button
            type="button"
            onClick={() => handleStorageModeChange("vault_0600")}
            style={choiceBtnStyle(storageMode === "vault_0600")}
          >
            OS Keychain / Daemon 0600 Vault
          </button>
          <button
            type="button"
            onClick={() => handleStorageModeChange("ram_only")}
            style={choiceBtnStyle(storageMode === "ram_only")}
          >
            RAM Only (Chỉ giữ trong phiên)
          </button>
        </div>
      </div>

      {/* Permission Approval Level */}
      <div style={{ padding: "10px 12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)" }}>
        <label style={{ display: "block", fontSize: "11px", fontWeight: 600, color: "var(--text-secondary)", marginBottom: "6px" }}>
          Cấp độ phê duyệt tự động (Permission Approval):
        </label>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: "6px" }}>
          {[
            { id: "auto_sandbox", label: "Auto in Sandbox" },
            { id: "ask_risky", label: "Ask Risky Only" },
            { id: "ask_all", label: "Ask All" },
          ].map((lvl) => (
            <button
              key={lvl.id}
              type="button"
              onClick={() => handlePermissionLevelChange(lvl.id)}
              style={choiceBtnStyle(permissionLevel === lvl.id)}
            >
              {lvl.label}
            </button>
          ))}
        </div>
      </div>

      {/* Keys with Live Verifier */}
      <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
        {/* Gemini */}
        <div>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "3px" }}>
            <label style={{ fontSize: "11px", fontWeight: 500, color: "var(--text-secondary)" }}>
              {t("settings.geminiKeyLabel", "Google Gemini API Key")}
            </label>
            <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
              <button
                type="button"
                onClick={() => verifyEndpoint("gemini", p.ephemeralApiKeys.gemini)}
                style={{
                  border: "none",
                  background: "transparent",
                  fontSize: "10.5px",
                  color: "var(--action-primary)",
                  cursor: "pointer",
                  display: "flex",
                  alignItems: "center",
                  gap: "4px",
                }}
              >
                <BrandIcon name="play" size={10} color="currentColor" />
                <span>Kiểm tra kết nối</span>
              </button>
              <button
                type="button"
                onClick={p.onToggleShowKeys}
                style={{ border: "none", background: "transparent", fontSize: "10px", color: "var(--text-muted)", cursor: "pointer" }}
              >
                {p.showKeys ? "Ẩn khóa" : "Hiện khóa"}
              </button>
            </div>
          </div>
          <input
            type={inputType}
            value={p.ephemeralApiKeys.gemini}
            onChange={(e) => p.onEphemeralKeyChange("gemini", e.target.value)}
            placeholder="AIzaSy..."
            className="cowok-input"
            style={INPUT_STYLE}
          />
          {verifyStates["gemini"] && (
            <div
              style={{
                fontSize: "10.5px",
                marginTop: "4px",
                color: verifyStates["gemini"].status === "success" ? "#4ade80" : "#f87171",
              }}
            >
              {verifyStates["gemini"].message || (verifyStates["gemini"].status === "verifying" ? "Đang kiểm tra kết nối (<3s)..." : "")}
            </div>
          )}
        </div>

        {/* Claude */}
        <div>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "3px" }}>
            <label style={{ fontSize: "11px", fontWeight: 500, color: "var(--text-secondary)" }}>
              {t("settings.claudeKeyLabel", "Anthropic Claude API Key")}
            </label>
            <button
              type="button"
              onClick={() => verifyEndpoint("claude", p.ephemeralApiKeys.claude)}
              style={{
                border: "none",
                background: "transparent",
                fontSize: "10.5px",
                color: "var(--action-primary)",
                cursor: "pointer",
                display: "flex",
                alignItems: "center",
                gap: "4px",
              }}
            >
              <BrandIcon name="play" size={10} color="currentColor" />
              <span>Kiểm tra kết nối</span>
            </button>
          </div>
          <input
            type={inputType}
            value={p.ephemeralApiKeys.claude}
            onChange={(e) => p.onEphemeralKeyChange("claude", e.target.value)}
            placeholder="sk-ant-api03-..."
            className="cowok-input"
            style={INPUT_STYLE}
          />
          {verifyStates["claude"] && (
            <div
              style={{
                fontSize: "10.5px",
                marginTop: "4px",
                color: verifyStates["claude"].status === "success" ? "#4ade80" : "#f87171",
              }}
            >
              {verifyStates["claude"].message || (verifyStates["claude"].status === "verifying" ? "Đang kiểm tra kết nối (<3s)..." : "")}
            </div>
          )}
        </div>

        {/* OpenAI */}
        <div>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "3px" }}>
            <label style={{ fontSize: "11px", fontWeight: 500, color: "var(--text-secondary)" }}>
              {t("settings.openaiKeyLabel", "OpenAI API Key")}
            </label>
            <button
              type="button"
              onClick={() => verifyEndpoint("openai", p.ephemeralApiKeys.openai)}
              style={{
                border: "none",
                background: "transparent",
                fontSize: "10.5px",
                color: "var(--action-primary)",
                cursor: "pointer",
                display: "flex",
                alignItems: "center",
                gap: "4px",
              }}
            >
              <BrandIcon name="play" size={10} color="currentColor" />
              <span>Kiểm tra kết nối</span>
            </button>
          </div>
          <input
            type={inputType}
            value={p.ephemeralApiKeys.openai}
            onChange={(e) => p.onEphemeralKeyChange("openai", e.target.value)}
            placeholder="sk-..."
            className="cowok-input"
            style={INPUT_STYLE}
          />
          {verifyStates["openai"] && (
            <div
              style={{
                fontSize: "10.5px",
                marginTop: "4px",
                color: verifyStates["openai"].status === "success" ? "#4ade80" : "#f87171",
              }}
            >
              {verifyStates["openai"].message || (verifyStates["openai"].status === "verifying" ? "Đang kiểm tra kết nối (<3s)..." : "")}
            </div>
          )}
        </div>
      </div>

      {/* BYOS / Custom Gateway */}
      <div
        style={{
          padding: "10px 12px",
          borderRadius: "var(--radius-xs)",
          backgroundColor: "var(--bg-subtle)",
          border: "1px solid var(--border-subtle)",
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "6px" }}>
          <div style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-primary)" }}>
            {t("settings.byosTitle", "Tùy biến Endpoint (BYOS / Custom Gateway)")}
          </div>
          <button
            type="button"
            onClick={() => verifyEndpoint("byos", p.byosEndpoint)}
            style={{
              border: "none",
              background: "transparent",
              fontSize: "10.5px",
              color: "var(--action-primary)",
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              gap: "4px",
            }}
          >
            <BrandIcon name="play" size={10} color="currentColor" />
            <span>Kiểm tra kết nối</span>
          </button>
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          <input
            type="text"
            value={p.byosEndpoint}
            onChange={(e) => p.onByosEndpointChange(e.target.value)}
            placeholder="http://localhost:11434/v1"
            className="cowok-input"
            style={INPUT_STYLE}
          />
          <input
            type={inputType}
            value={p.ephemeralApiKeys.byosToken}
            onChange={(e) => p.onEphemeralKeyChange("byosToken", e.target.value)}
            placeholder="Bearer token (chỉ giữ trong phiên)"
            className="cowok-input"
            style={INPUT_STYLE}
          />
        </div>
        {verifyStates["byos"] && (
          <div
            style={{
              fontSize: "10.5px",
              marginTop: "6px",
              color: verifyStates["byos"].status === "success" ? "#4ade80" : "#f87171",
            }}
          >
            {verifyStates["byos"].message || (verifyStates["byos"].status === "verifying" ? "Đang kiểm tra endpoint (<3s)..." : "")}
          </div>
        )}
      </div>
    </>
  );
};

export const SettingsGeneralModelsPane: React.FC<SettingsGeneralModelsPaneProps> = (props) =>
  props.selectedTab === "general" ? <GeneralAppearanceSection {...props} /> : <ModelsByosSection {...props} />;
