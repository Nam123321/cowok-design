// @story 04.3, 11.1
// @implements Shorten Bento API Providers Panel (6-Provider Unified BYOK Grid + Live 429 Cooldown Indicators)
import React, { useEffect, useState } from "react";
import { apiBotFleet } from "../../../api/bot-fleet";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ByokProviderCooldown } from "../../../lib/validations/bot-fleet";

export type ApiBentoProviderId = "gemini" | "claude" | "openai" | "openrouter" | "deepseek" | "byos";

export interface ApiVerifyStatus {
  status: "idle" | "verifying" | "success" | "error";
  message?: string;
}

export type EphemeralApiKeyField =
  | "gemini"
  | "claude"
  | "openai"
  | "openrouter"
  | "deepseek"
  | "byosToken";

export interface SettingsApiBentoPanelProps {
  ephemeralApiKeys: {
    gemini: string;
    claude: string;
    openai: string;
    openrouter?: string;
    deepseek?: string;
    byosToken: string;
  };
  onEphemeralKeyChange: (key: EphemeralApiKeyField, val: string) => void;
  byosEndpoint: string;
  onByosEndpointChange: (url: string) => void;
  showKeys: boolean;
  onToggleShowKeys: () => void;
  verifyStates: Record<string, ApiVerifyStatus>;
  onVerifyProvider: (provider: ApiBentoProviderId, value: string, optionalToken?: string) => void;
  byokCooldowns?: ByokProviderCooldown[];
}

interface ApiProviderMeta {
  id: ApiBentoProviderId;
  name: string;
  nameKey?: string;
  placeholder: string;
  tag: string;
  tagKey?: string;
}

const ICON_SIZE_XS = 10;
const FONT_WEIGHT_SEMIBOLD = 600;

const API_PROVIDER_CATALOG: readonly ApiProviderMeta[] = [
  { id: "gemini", name: "Google Gemini", placeholder: "AIzaSy...", tag: "AI Studio" },
  { id: "claude", name: "Anthropic Claude", placeholder: "sk-ant-api03-...", tag: "Messages API" },
  { id: "openai", name: "OpenAI", placeholder: "sk-...", tag: "Responses API" },
  { id: "openrouter", name: "OpenRouter", placeholder: "sk-or-...", tag: "Multi-LLM" },
  { id: "deepseek", name: "DeepSeek", placeholder: "sk-...", tag: "Reasoning" },
  {
    id: "byos",
    name: "Máy chủ riêng / Ollama",
    nameKey: "settings.customServerProviderName",
    placeholder: "http://localhost:11434/v1",
    tag: "Máy chủ tùy chỉnh",
    tagKey: "settings.customServerProviderTag",
  },
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

function resolveTileBorder(selected: boolean, inCooldown: boolean, verified: boolean): string {
  if (selected) return "1px solid var(--action-primary)";
  if (inCooldown) return "1px solid var(--state-warning)";
  if (verified) return "1px solid var(--state-success)";
  return "1px solid var(--border-subtle)";
}

function resolveTileDotColor(inCooldown: boolean, verified: boolean): string {
  if (inCooldown) return "var(--state-warning)";
  if (verified) return "var(--state-success)";
  return "var(--action-primary)";
}

function resolveTileSubColor(inCooldown: boolean, verified: boolean): string {
  if (inCooldown) return "var(--state-warning)";
  if (verified) return "var(--state-success)";
  return "var(--text-muted)";
}

function resolveVerifyColor(status: ApiVerifyStatus["status"]): string {
  if (status === "success") return "var(--state-success)";
  if (status === "verifying") return "var(--text-muted)";
  return "var(--state-danger)";
}

function resolveTileSubLabel(params: {
  inCooldown: boolean;
  verified: boolean;
  retryAfterSecs: number;
  tag: string;
  t: (key: string, fallback: string) => string;
  formatNumber: (val: number) => string;
}): string {
  if (params.inCooldown) {
    return `${params.t("settings.cooldown429Short", "Tạm nghỉ 429")} (${params.formatNumber(params.retryAfterSecs)}s)`;
  }
  if (params.verified) return params.t("settings.apiVerifiedShort", "Đã xác thực");
  return params.tag;
}

function useLiveByokCooldowns(
  overrideCooldowns: ByokProviderCooldown[] | undefined,
  expanded: boolean
): ByokProviderCooldown[] {
  const [cooldowns, setCooldowns] = useState<ByokProviderCooldown[]>(overrideCooldowns ?? []);

  useEffect(() => {
    if (overrideCooldowns !== undefined) {
      setCooldowns(overrideCooldowns);
      return;
    }
    let active = true;
    apiBotFleet
      .getProvidersStatus()
      .then((res) => {
        if (active) setCooldowns(res.data.byokProviders);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [overrideCooldowns, expanded]);

  return cooldowns;
}

const ApiProviderGridTile: React.FC<{
  meta: ApiProviderMeta;
  selected: boolean;
  configured: boolean;
  verified: boolean;
  cooldown?: ByokProviderCooldown;
  onClick: () => void;
}> = ({ meta, selected, configured, verified, cooldown, onClick }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const inCooldown = Boolean(cooldown?.inCooldown);
  const localizedName = meta.nameKey ? t(meta.nameKey, meta.name) : meta.name;
  const localizedTag = meta.tagKey ? t(meta.tagKey, meta.tag) : meta.tag;
  const subLabel = resolveTileSubLabel({
    inCooldown,
    verified,
    retryAfterSecs: cooldown?.retryAfterSecs ?? 0,
    tag: localizedTag,
    t,
    formatNumber,
  });

  return (
    <button
      type="button"
      data-testid={`api-bento-tile-${meta.id}`}
      onClick={onClick}
      style={{
        padding: "8px 9px",
        borderRadius: "var(--radius-xs)",
        border: resolveTileBorder(selected, inCooldown, verified),
        backgroundColor: selected ? "var(--bg-selected)" : "var(--bg-workspace)",
        textAlign: "left",
        cursor: "pointer",
        display: "flex",
        flexDirection: "column",
        gap: "3px",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "4px" }}>
        <span style={{ fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>{localizedName}</span>
        {(verified || configured || inCooldown) && (
          <span
            data-testid={inCooldown ? `byok-cooldown-dot-${meta.id}` : undefined}
            style={{ width: "7px", height: "7px", borderRadius: "50%", backgroundColor: resolveTileDotColor(inCooldown, verified) }}
          />
        )}
      </div>
      <span style={{ fontSize: "9.5px", color: resolveTileSubColor(inCooldown, verified), display: "inline-flex", alignItems: "center", gap: "3px" }}>
        {inCooldown && <BrandIcon name="shield-alert" size={ICON_SIZE_XS} color="var(--state-warning)" />}
        <span>{subLabel}</span>
      </span>
    </button>
  );
};

const CooldownBanner: React.FC<{ providerId: string; retryAfterSecs: number }> = ({
  providerId,
  retryAfterSecs,
}) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <div
      data-testid={`byok-cooldown-banner-${providerId}`}
      style={{
        padding: "5px 8px",
        borderRadius: "var(--radius-xs)",
        backgroundColor: "var(--state-warning-soft)",
        border: "1px solid var(--state-warning)",
        color: "var(--state-warning)",
        fontSize: "10px",
        display: "flex",
        alignItems: "center",
        gap: "5px",
      }}
    >
      <BrandIcon name="shield-alert" size={ICON_SIZE_XS} color="var(--state-warning)" />
      <span>
        {t("settings.byokCooldownActive", "Nhà cung cấp đang tạm nghỉ HTTP 429 — tự động chuyển sang dự phòng ({{secs}}s).").replace(
          "{{secs}}",
          formatNumber(retryAfterSecs)
        )}
      </span>
    </div>
  );
};

const ApiProviderCredentialEditor: React.FC<{
  meta: ApiProviderMeta;
  value: string;
  byosToken: string;
  showKeys: boolean;
  verifyState?: ApiVerifyStatus;
  cooldown?: ByokProviderCooldown;
  onChangeValue: (val: string) => void;
  onChangeByosToken: (val: string) => void;
  onToggleShowKeys: () => void;
  onVerify: () => void;
}> = (p) => {
  const { t } = useTranslation();
  const isByos = p.meta.id === "byos";
  const localizedName = p.meta.nameKey ? t(p.meta.nameKey, p.meta.name) : p.meta.name;
  const inputType = p.showKeys ? "text" : "password";
  const verifyMsg =
    p.verifyState?.message ||
    (p.verifyState?.status === "verifying" ? t("settings.verifyingShort", "Đang kiểm tra kết nối (<3s)...") : "");

  return (
    <div data-testid={`api-editor-${p.meta.id}`} style={{ padding: "9px 10px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-subtle)", display: "flex", flexDirection: "column", gap: "7px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
        <span style={{ fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>{localizedName}</span>
        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <button type="button" onClick={p.onToggleShowKeys} style={{ border: "none", background: "transparent", fontSize: "10px", color: "var(--text-muted)", cursor: "pointer" }}>
            {p.showKeys ? t("settings.hideKeys", "Ẩn khóa") : t("settings.showKeys", "Hiện khóa")}
          </button>
          <button type="button" data-testid={isByos ? "verify-byos-btn" : `verify-api-btn-${p.meta.id}`} onClick={p.onVerify} style={{ border: "none", background: "transparent", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--action-primary)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "4px" }}>
            <BrandIcon name="play" size={ICON_SIZE_XS} color="currentColor" />
            <span>{t("settings.verifyConnection", "Kiểm tra kết nối")}</span>
          </button>
        </div>
      </div>
      <input type={isByos ? "text" : inputType} value={p.value} onChange={(e) => p.onChangeValue(e.target.value)} placeholder={p.meta.placeholder} className="cowok-input" style={INPUT_STYLE} />
      {isByos && (
        <input type={inputType} value={p.byosToken} onChange={(e) => p.onChangeByosToken(e.target.value)} placeholder={t("settings.bearerPlaceholder", "Bearer token (chỉ giữ trong phiên)")} className="cowok-input" style={INPUT_STYLE} />
      )}
      {p.cooldown?.inCooldown && <CooldownBanner providerId={p.meta.id} retryAfterSecs={p.cooldown.retryAfterSecs} />}
      {p.verifyState && p.verifyState.status !== "idle" && (
        <div style={{ fontSize: "10.5px", color: resolveVerifyColor(p.verifyState.status) }}>{verifyMsg}</div>
      )}
    </div>
  );
};

const ApiBentoHeaderButton: React.FC<{
  expanded: boolean;
  connectedCount: number;
  coolingCount: number;
  onToggle: () => void;
}> = ({ expanded, connectedCount, coolingCount, onToggle }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <button
      type="button"
      data-testid="toggle-api-bento-btn"
      onClick={onToggle}
      style={{ border: "none", background: "transparent", padding: 0, cursor: "pointer", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", textAlign: "left", width: "100%" }}
    >
      <div>
        <div style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", display: "flex", alignItems: "center", gap: "6px" }}>
          <span>{t("settings.apiBentoTitle", "2. Kết nối Khóa API nhà cung cấp")}</span>
          {coolingCount > 0 && (
            <span data-testid="byok-cooling-summary-badge" style={{ fontSize: "9.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, padding: "1px 6px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)" }}>
              {formatNumber(coolingCount)} {t("settings.cooldown429Short", "Tạm nghỉ 429")}
            </span>
          )}
        </div>
        <div style={{ fontSize: "10.5px", color: "var(--text-muted)", marginTop: "2px" }}>
          {t("settings.apiProvidersList", "Gemini · Claude · OpenAI · OpenRouter · DeepSeek · Máy chủ riêng/Ollama")}
          {connectedCount > 0 ? ` (${formatNumber(connectedCount)} ${t("settings.configuredShort", "đã nhập")})` : ""}
        </div>
      </div>
      <span style={{ fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--action-primary)", whiteSpace: "nowrap" }}>
        {expanded ? `${t("settings.collapseBento", "Thu gọn")} ▲` : `${t("settings.connectApiBtn", "Kết nối API")} ▼`}
      </span>
    </button>
  );
};

export const SettingsApiBentoPanel: React.FC<SettingsApiBentoPanelProps> = (p) => {
  const { t } = useTranslation();
  const hasPresetByos = Boolean(p.byosEndpoint.trim());
  const [expanded, setExpanded] = useState<boolean>(hasPresetByos);
  const [activeId, setActiveId] = useState<ApiBentoProviderId>(hasPresetByos ? "byos" : "gemini");
  const liveCooldowns = useLiveByokCooldowns(p.byokCooldowns, expanded);
  const cooldownMap = new Map(liveCooldowns.map((c) => [c.providerId, c]));
  const coolingCount = liveCooldowns.filter((c) => c.inCooldown).length;

  const readProviderVal = (id: ApiBentoProviderId): string => {
    if (id === "byos") return p.byosEndpoint;
    return p.ephemeralApiKeys[id] ?? "";
  };
  const writeProviderVal = (id: ApiBentoProviderId, val: string) => {
    if (id === "byos") { p.onByosEndpointChange(val); return; }
    p.onEphemeralKeyChange(id, val);
  };
  const connectedCount = API_PROVIDER_CATALOG.filter(
    (m) => Boolean(readProviderVal(m.id).trim()) || p.verifyStates[m.id]?.status === "success" || Boolean(cooldownMap.get(m.id)?.configured)
  ).length;
  const activeMeta = API_PROVIDER_CATALOG.find((m) => m.id === activeId) ?? API_PROVIDER_CATALOG[0];

  return (
    <div data-testid="api-bento-panel" style={{ padding: "10px 12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", display: "flex", flexDirection: "column", gap: "8px" }}>
      <ApiBentoHeaderButton expanded={expanded} connectedCount={connectedCount} coolingCount={coolingCount} onToggle={() => setExpanded((v) => !v)} />
      {expanded && (
        <div style={{ display: "flex", flexDirection: "column", gap: "8px", paddingTop: "2px" }}>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))", gap: "6px" }}>
            {API_PROVIDER_CATALOG.map((meta) => (
              <ApiProviderGridTile key={meta.id} meta={meta} selected={activeId === meta.id} configured={Boolean(readProviderVal(meta.id).trim()) || Boolean(cooldownMap.get(meta.id)?.configured)} verified={p.verifyStates[meta.id]?.status === "success"} cooldown={cooldownMap.get(meta.id)} onClick={() => setActiveId(meta.id)} />
            ))}
          </div>
          <ApiProviderCredentialEditor meta={activeMeta} value={readProviderVal(activeMeta.id)} byosToken={p.ephemeralApiKeys.byosToken} showKeys={p.showKeys} verifyState={p.verifyStates[activeMeta.id]} cooldown={cooldownMap.get(activeMeta.id)} onChangeValue={(v) => writeProviderVal(activeMeta.id, v)} onChangeByosToken={(v) => p.onEphemeralKeyChange("byosToken", v)} onToggleShowKeys={p.onToggleShowKeys} onVerify={() => p.onVerifyProvider(activeMeta.id, readProviderVal(activeMeta.id), activeMeta.id === "byos" ? p.ephemeralApiKeys.byosToken : undefined)} />
          <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>
            {t("settings.byokGatewayActiveNote", "Cổng kết nối Khóa API đang hoạt động — đọc khóa từ Két an toàn 0o600 và tự động chuyển nhà cung cấp dự phòng khi gặp HTTP 429.")}
          </div>
        </div>
      )}
    </div>
  );
};
