// @story 04.3, 11.1
// @implements Shorten Bento API Providers Panel (6-Provider BYOK Grid + Custom BYOS Gateway)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";

export type ApiBentoProviderId = "gemini" | "claude" | "openai" | "openrouter" | "deepseek" | "byos";

export interface ApiVerifyStatus {
  status: "idle" | "verifying" | "success" | "error";
  message?: string;
}

export interface SettingsApiBentoPanelProps {
  ephemeralApiKeys: { gemini: string; claude: string; openai: string; byosToken: string };
  onEphemeralKeyChange: (key: "gemini" | "claude" | "openai" | "byosToken", val: string) => void;
  byosEndpoint: string;
  onByosEndpointChange: (url: string) => void;
  showKeys: boolean;
  onToggleShowKeys: () => void;
  verifyStates: Record<string, ApiVerifyStatus>;
  onVerifyProvider: (provider: ApiBentoProviderId, value: string, optionalToken?: string) => void;
}

interface ApiProviderMeta {
  id: ApiBentoProviderId;
  name: string;
  placeholder: string;
  tag: string;
}

const API_PROVIDER_CATALOG: readonly ApiProviderMeta[] = [
  { id: "gemini", name: "Google Gemini", placeholder: "AIzaSy...", tag: "AI Studio" },
  { id: "claude", name: "Anthropic Claude", placeholder: "sk-ant-api03-...", tag: "Messages API" },
  { id: "openai", name: "OpenAI", placeholder: "sk-...", tag: "Responses API" },
  { id: "openrouter", name: "OpenRouter", placeholder: "sk-or-...", tag: "Multi-LLM" },
  { id: "deepseek", name: "DeepSeek", placeholder: "sk-...", tag: "Reasoning" },
  { id: "byos", name: "BYOS / Ollama", placeholder: "http://localhost:11434/v1", tag: "Custom Gateway" },
];

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

const ApiProviderGridTile: React.FC<{
  meta: ApiProviderMeta;
  selected: boolean;
  configured: boolean;
  verified: boolean;
  onClick: () => void;
}> = ({ meta, selected, configured, verified, onClick }) => {
  const { t } = useTranslation();
  const activeBorder = selected
    ? "1px solid var(--action-primary)"
    : verified
      ? "1px solid var(--state-success)"
      : "1px solid var(--border-subtle)";

  return (
    <button
      type="button"
      data-testid={`api-bento-tile-${meta.id}`}
      onClick={onClick}
      style={{
        padding: "8px 9px",
        borderRadius: "var(--radius-xs)",
        border: activeBorder,
        backgroundColor: selected ? "var(--bg-selected)" : "var(--bg-workspace)",
        textAlign: "left",
        cursor: "pointer",
        display: "flex",
        flexDirection: "column",
        gap: "3px",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "4px" }}>
        <span style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-primary)" }}>{meta.name}</span>
        {(verified || configured) && (
          <span
            style={{
              width: "7px",
              height: "7px",
              borderRadius: "50%",
              backgroundColor: verified ? "var(--state-success)" : "var(--action-primary)",
            }}
          />
        )}
      </div>
      <span style={{ fontSize: "9.5px", color: verified ? "var(--state-success)" : "var(--text-muted)" }}>
        {verified ? t("settings.apiVerifiedShort", "Đã xác thực") : meta.tag}
      </span>
    </button>
  );
};

const ApiProviderCredentialEditor: React.FC<{
  meta: ApiProviderMeta;
  value: string;
  byosToken: string;
  showKeys: boolean;
  verifyState?: ApiVerifyStatus;
  onChangeValue: (val: string) => void;
  onChangeByosToken: (val: string) => void;
  onToggleShowKeys: () => void;
  onVerify: () => void;
}> = (p) => {
  const { t } = useTranslation();
  const isByos = p.meta.id === "byos";
  const inputType = p.showKeys ? "text" : "password";

  return (
    <div data-testid={`api-editor-${p.meta.id}`} style={{ padding: "9px 10px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-subtle)", display: "flex", flexDirection: "column", gap: "7px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
        <span style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-primary)" }}>{p.meta.name}</span>
        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <button type="button" onClick={p.onToggleShowKeys} style={{ border: "none", background: "transparent", fontSize: "10px", color: "var(--text-muted)", cursor: "pointer" }}>
            {p.showKeys ? t("settings.hideKeys", "Ẩn khóa") : t("settings.showKeys", "Hiện khóa")}
          </button>
          <button
            type="button"
            data-testid={isByos ? "verify-byos-btn" : `verify-api-btn-${p.meta.id}`}
            onClick={p.onVerify}
            style={{ border: "none", background: "transparent", fontSize: "10.5px", fontWeight: 600, color: "var(--action-primary)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "4px" }}
          >
            <BrandIcon name="play" size={10} color="currentColor" />
            <span>{t("settings.verifyConnection", "Kiểm tra kết nối")}</span>
          </button>
        </div>
      </div>

      <input
        type={isByos ? "text" : inputType}
        value={p.value}
        onChange={(e) => p.onChangeValue(e.target.value)}
        placeholder={p.meta.placeholder}
        className="cowok-input"
        style={INPUT_STYLE}
      />

      {isByos && (
        <input
          type={inputType}
          value={p.byosToken}
          onChange={(e) => p.onChangeByosToken(e.target.value)}
          placeholder={t("settings.bearerPlaceholder", "Bearer token (chỉ giữ trong phiên)")}
          className="cowok-input"
          style={INPUT_STYLE}
        />
      )}

      {p.verifyState && p.verifyState.status !== "idle" && (
        <div style={{ fontSize: "10.5px", color: p.verifyState.status === "success" ? "var(--state-success)" : p.verifyState.status === "verifying" ? "var(--text-muted)" : "var(--state-danger)" }}>
          {p.verifyState.message || (p.verifyState.status === "verifying" ? t("settings.verifyingShort", "Đang kiểm tra kết nối (<3s)...") : "")}
        </div>
      )}
    </div>
  );
};

export const SettingsApiBentoPanel: React.FC<SettingsApiBentoPanelProps> = (p) => {
  const { t } = useTranslation();
  const hasPresetByos = Boolean(p.byosEndpoint.trim());
  const [expanded, setExpanded] = useState<boolean>(hasPresetByos);
  const [activeId, setActiveId] = useState<ApiBentoProviderId>(hasPresetByos ? "byos" : "gemini");
  const [extraKeys, setExtraKeys] = useState<{ openrouter: string; deepseek: string }>({ openrouter: "", deepseek: "" });

  const readProviderVal = (id: ApiBentoProviderId): string => {
    if (id === "byos") return p.byosEndpoint;
    if (id === "openrouter") return extraKeys.openrouter;
    if (id === "deepseek") return extraKeys.deepseek;
    return p.ephemeralApiKeys[id];
  };

  const writeProviderVal = (id: ApiBentoProviderId, val: string) => {
    if (id === "byos") { p.onByosEndpointChange(val); return; }
    if (id === "openrouter" || id === "deepseek") { setExtraKeys((prev) => ({ ...prev, [id]: val })); return; }
    p.onEphemeralKeyChange(id, val);
  };

  const connectedCount = API_PROVIDER_CATALOG.filter((m) => Boolean(readProviderVal(m.id).trim()) || p.verifyStates[m.id]?.status === "success").length;
  const activeMeta = API_PROVIDER_CATALOG.find((m) => m.id === activeId) ?? API_PROVIDER_CATALOG[0];

  return (
    <div data-testid="api-bento-panel" style={{ padding: "10px 12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", display: "flex", flexDirection: "column", gap: "8px" }}>
      <button
        type="button"
        data-testid="toggle-api-bento-btn"
        onClick={() => setExpanded((v) => !v)}
        style={{ border: "none", background: "transparent", padding: 0, cursor: "pointer", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", textAlign: "left", width: "100%" }}
      >
        <div>
          <div style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)" }}>
            {t("settings.apiBentoTitle", "2. Kết nối API Providers (BYOK & Gateway)")}
          </div>
          <div style={{ fontSize: "10.5px", color: "var(--text-muted)", marginTop: "2px" }}>
            {t("settings.apiProvidersList", "Gemini · Claude · OpenAI · OpenRouter · DeepSeek · BYOS/Ollama")}
            {connectedCount > 0 ? ` (${connectedCount} ${t("settings.configuredShort", "đã nhập")})` : ""}
          </div>
        </div>
        <span style={{ fontSize: "11px", fontWeight: 600, color: "var(--action-primary)", whiteSpace: "nowrap" }}>
          {expanded ? `${t("settings.collapseBento", "Thu gọn")} ▲` : `${t("settings.connectApiBtn", "Kết nối API")} ▼`}
        </span>
      </button>

      {expanded && (
        <div style={{ display: "flex", flexDirection: "column", gap: "8px", paddingTop: "2px" }}>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: "6px" }}>
            {API_PROVIDER_CATALOG.map((meta) => (
              <ApiProviderGridTile
                key={meta.id}
                meta={meta}
                selected={activeId === meta.id}
                configured={Boolean(readProviderVal(meta.id).trim())}
                verified={p.verifyStates[meta.id]?.status === "success"}
                onClick={() => setActiveId(meta.id)}
              />
            ))}
          </div>
          <ApiProviderCredentialEditor
            meta={activeMeta}
            value={readProviderVal(activeMeta.id)}
            byosToken={p.ephemeralApiKeys.byosToken}
            showKeys={p.showKeys}
            verifyState={p.verifyStates[activeMeta.id]}
            onChangeValue={(v) => writeProviderVal(activeMeta.id, v)}
            onChangeByosToken={(v) => p.onEphemeralKeyChange("byosToken", v)}
            onToggleShowKeys={p.onToggleShowKeys}
            onVerify={() => p.onVerifyProvider(activeMeta.id, readProviderVal(activeMeta.id), activeMeta.id === "byos" ? p.ephemeralApiKeys.byosToken : undefined)}
          />
          <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>
            {t("settings.story11ApiNote", "Hạ tầng định tuyến Unified BYOK Gateway đầy đủ sẽ mở rộng tại Story 11.1.")}
          </div>
        </div>
      )}
    </div>
  );
};
