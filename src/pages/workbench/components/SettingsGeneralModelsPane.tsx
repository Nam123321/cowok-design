// @story 04.1, 04.2
// @implements AC-04 (Settings Models Pane with Live BYOK Verifier, Storage Mode & Permissions)
import React, { useEffect, useState } from "react";
import { CowokSelect } from "../../../components/ui/CowokSelect";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { useDoctorProvidersUsage } from "../../../hooks/useDoctorProvidersUsage";
import { apiSupport } from "../../../api/support";
import { apiWorkbenchSettings } from "../../../api/workbench-settings";
import type { TelemetryPreferences } from "../../../lib/validations/support";
import type { CliDiagnosticItem } from "../../../lib/validations/workbench-settings";
import { SettingsDetectedProvidersPanel } from "./SettingsDetectedProvidersPanel";
import {
  SettingsApiBentoPanel,
  type ApiBentoProviderId,
  type ApiVerifyStatus,
} from "./SettingsApiBentoPanel";
import { SettingsRuntimeSummaryBento } from "./SettingsRuntimeSummaryBento";
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
  ephemeralApiKeys: {
    gemini: string;
    claude: string;
    openai: string;
    openrouter?: string;
    deepseek?: string;
    byosToken: string;
  };
  onEphemeralKeyChange: (
    key: "gemini" | "claude" | "openai" | "openrouter" | "deepseek" | "byosToken",
    val: string
  ) => void;
  showKeys: boolean;
  onToggleShowKeys: () => void;
  onAgentPrefsChange?: (vaultMode: string, permissionMode: string) => void;
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

const TelemetryPrivacyCard: React.FC<{ locale: string }> = ({ locale }) => {
  const { t } = useTranslation(locale);
  const fmt = useFormatter({ locale });
  const [prefs, setPrefs] = useState<TelemetryPreferences | null>(null);
  const [loadState, setLoadState] = useState<"loading" | "ready" | "offline">("loading");
  const [saving, setSaving] = useState(false);
  const [statusNotice, setStatusNotice] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    setLoadState("loading");
    void apiSupport
      .getTelemetryPreferences()
      .then((res) => {
        if (!active) return;
        setPrefs(res.data);
        setLoadState("ready");
      })
      .catch(() => {
        if (!active) return;
        setLoadState("offline");
      });
    return () => {
      active = false;
    };
  }, []);

  const isReady = loadState === "ready" && prefs !== null;
  const isEnabled = isReady && prefs.effectiveEnabled;
  const isLocked = Boolean(prefs?.envLocked);
  const isDisabled = !isReady || isLocked || saving;

  const handleToggle = async () => {
    if (isDisabled || !prefs) return;
    const nextEnabled = !prefs.telemetryEnabled;
    setSaving(true);
    setStatusNotice(null);
    try {
      const res = await apiSupport.updateTelemetryPreferences({ telemetryEnabled: nextEnabled });
      setPrefs(res.data);
      setStatusNotice(
        res.data.effectiveEnabled
          ? t("support.telemetrySavedEnabled", "Đã bật gửi thống kê ẩn danh cải thiện sản phẩm.")
          : t("support.telemetrySavedDisabled", "Đã tắt hoàn toàn telemetry ra ngoài (0 kết nối).")
      );
    } catch (err) {
      setStatusNotice(
        err instanceof Error
          ? err.message
          : t("support.telemetrySaveFailed", "Không thể cập nhật thiết lập quyền riêng tư.")
      );
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      data-testid="settings-telemetry-privacy-card"
      style={{
        padding: "10px 12px",
        borderRadius: "var(--radius-sm)",
        border: "1px solid var(--border-subtle)",
        backgroundColor: "var(--bg-workspace)",
        display: "flex",
        flexDirection: "column",
        gap: "8px",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "10px" }}>
        <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
          <span style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-primary)" }}>
            {t("support.privacySectionTitle", "Quyền riêng tư & Telemetry")}
          </span>
          <span style={{ fontSize: "10.5px", color: "var(--text-muted)", lineHeight: 1.4 }}>
            {t(
              "support.privacySectionDesc",
              "Chia sẻ sự kiện kích hoạt ẩn danh giúp cải thiện cowok-design. Tự động xóa API key, mật khẩu và đường dẫn cá nhân."
            )}
          </span>
        </div>

        <button
          type="button"
          role="switch"
          aria-checked={isEnabled}
          aria-busy={loadState === "loading"}
          aria-label={t("support.privacyToggleAria", "Bật hoặc tắt gửi telemetry ẩn danh")}
          data-testid="settings-telemetry-optin-toggle"
          disabled={isDisabled}
          onClick={() => void handleToggle()}
          style={{
            flexShrink: 0,
            minWidth: "42px",
            height: "24px",
            padding: "2px",
            borderRadius: "var(--radius-pill)",
            border: "1px solid var(--border-default)",
            backgroundColor: isEnabled ? "var(--action-primary)" : "var(--bg-subtle)",
            display: "inline-flex",
            alignItems: "center",
            justifyContent: isEnabled ? "flex-end" : "flex-start",
            cursor: isDisabled ? "not-allowed" : "pointer",
            opacity: isDisabled ? 0.6 : 1,
          }}
        >
          <span
            style={{
              width: "18px",
              height: "18px",
              borderRadius: "50%",
              backgroundColor: isEnabled ? "var(--action-primary-fg)" : "var(--text-muted)",
              display: "inline-block",
            }}
          />
        </button>
      </div>

      {loadState === "loading" && (
        <div data-testid="settings-telemetry-loading" style={{ fontSize: "10.5px", color: "var(--text-muted)" }}>
          {t("support.telemetryLoading", "Đang tải trạng thái quyền riêng tư từ daemon...")}
        </div>
      )}

      {loadState === "offline" && (
        <div data-testid="settings-telemetry-offline" style={{ fontSize: "10.5px", color: "var(--state-warning)" }}>
          {t("support.telemetryOffline", "Daemon đang ngoại tuyến — không thể đọc hoặc thay đổi thiết lập telemetry.")}
        </div>
      )}

      {isLocked && (
        <div
          data-testid="settings-telemetry-env-locked-badge"
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: "6px",
            padding: "4px 8px",
            borderRadius: "var(--radius-xs)",
            backgroundColor: "var(--state-warning-soft)",
            color: "var(--state-warning)",
            fontSize: "10.5px",
            fontWeight: 600,
          }}
        >
          <BrandIcon name="warning" size={12} color="currentColor" />
          <span>
            {`${t("support.telemetryEnvLocked", "Bị khóa tắt bởi biến môi trường")}: ${prefs?.lockedReason ?? "COWOK_TELEMETRY=0"}`}
          </span>
        </div>
      )}

      {prefs && (
        <div style={{ display: "flex", alignItems: "center", gap: "10px", fontSize: "10px", color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
          <span>{`${t("support.installIdLabel", "Install ID")}: ${prefs.installId}`}</span>
          <span>•</span>
          <span>{`${t("support.outboxPendingLabel", "Hàng đợi chờ gửi")}: ${fmt.formatNumber(prefs.pendingOutboxCount)}`}</span>
        </div>
      )}

      {statusNotice && (
        <div role="status" data-testid="settings-telemetry-status" style={{ fontSize: "10.5px", color: "var(--text-secondary)" }}>
          {statusNotice}
        </div>
      )}
    </div>
  );
};

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
        <label htmlFor="settings-locale-select" style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-secondary)" }}>
          {t("settings.localeLabel", "Ngôn ngữ")}
        </label>
        <CowokSelect
          id="settings-locale-select"
          testId="settings-locale-select"
          ariaLabel={t("settings.localeLabel", "Ngôn ngữ")}
          value={p.currentLocale}
          onChange={p.onLocaleChange}
          options={SUPPORTED_LOCALES.map((loc) => ({ value: loc.code, label: loc.label }))}
        />
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
              aria-label={`${sz.value} — ${t(sz.labelKey, sz.fallback)}`}
              onClick={() => p.onFontSizeChange(sz.value)}
              style={{ ...choiceBtnStyle(p.currentFontSize === sz.value), flex: 1, padding: "6px 8px" }}
            >
              {sz.value}
            </button>
          ))}
        </div>
      </div>
      <TelemetryPrivacyCard locale={p.currentLocale} />
    </>
  );
};

const ModelsByosSection: React.FC<
  Omit<
    SettingsGeneralModelsPaneProps,
    "selectedTab" | "currentTheme" | "onThemeChange" | "currentLocale" | "onLocaleChange" | "currentFontSize" | "onFontSizeChange"
  >
> = (p) => {
  const { t } = useTranslation();
  const [storageMode, setStorageMode] = useState<string>("vault_0600");
  const [permissionLevel, setPermissionLevel] = useState<string>("auto_sandbox");
  const [verifyStates, setVerifyStates] = useState<Record<string, ApiVerifyStatus>>({});
  const [daemonNotice, setDaemonNotice] = useState<string | null>(null);
  const [disconnectedIds, setDisconnectedIds] = useState<string[]>([]);
  const doctor = useDoctorProvidersUsage(true);
  const connectedIds = doctor.readyTools
    .map((tool) => tool.id)
    .filter((id) => !disconnectedIds.includes(id));

  useEffect(() => {
    let active = true;
    void apiWorkbenchSettings
      .listAgentSettings()
      .then((rows) => {
        if (!active || rows.length === 0) return;
        setDaemonNotice(null);
        const def = rows.find((r) => r.isDefault) || rows[0];
        const vMode = def.vaultMode || def.credentialRef;
        if (vMode === "keychain" || vMode === "vault_0600" || vMode === "ram_only") {
          setStorageMode(vMode);
        }
        const pMode = def.permissionMode || def.status;
        if (pMode === "auto_sandbox" || pMode === "ask_risky" || pMode === "ask_all" || pMode === "accept_edits" || pMode === "ask-every-time") {
          setPermissionLevel(pMode);
        }
      })
      .catch((err) => {
        if (!active) return;
        const detail = err instanceof Error ? `: ${err.message}` : "";
        setDaemonNotice(
          `${t("settings.daemonSyncWarning", "Daemon offline — chỉ dùng trạng thái trong phiên")}${detail}`
        );
      });
    return () => {
      active = false;
    };
  }, [t]);

  const persistAgentPrefs = async (opts: {
    nextStorage: string;
    nextPerm: string;
    prevStorage?: string;
    prevPerm?: string;
    modelId: string;
    endpoint: string;
  }) => {
    const { nextStorage, nextPerm, prevStorage, prevPerm, modelId, endpoint } = opts;
    const vaultMap: Record<string, "ram_only" | "keychain" | "vault_0600"> = { ram_only: "ram_only", keychain: "keychain", vault_0600: "vault_0600" };
    const resolvedVault = vaultMap[nextStorage] ?? "vault_0600";
    const resolvedPerm = nextPerm === "ask_risky" || nextPerm === "ask_all" || nextPerm === "accept_edits" || nextPerm === "ask-every-time"
      ? nextPerm
      : "auto_sandbox";
    try {
      await apiWorkbenchSettings.updateVaultConfig({ vaultMode: resolvedVault });
      await apiWorkbenchSettings.upsertAgentSetting({
        providerId: modelId || "cowok-swarm",
        modelId: modelId || "cowok-swarm",
        endpointUrl: endpoint || "",
        credentialRef: `vault://${resolvedVault}/${modelId || "cowok-swarm"}`,
        status: resolvedPerm,
        permissionMode: resolvedPerm,
        vaultMode: resolvedVault,
        isDefault: true,
      });
      setDaemonNotice(null);
      p.onAgentPrefsChange?.(resolvedVault, resolvedPerm);
    } catch (err) {
      if (prevStorage !== undefined) setStorageMode(prevStorage);
      if (prevPerm !== undefined) setPermissionLevel(prevPerm);
      const detail = err instanceof Error ? `: ${err.message}` : "";
      setDaemonNotice(
        `${t("settings.saveDaemonWarning", "Không thể lưu cấu hình lên daemon SQLite V7")}${detail}`
      );
    }
  };

  const handleStorageModeChange = (mode: string) => {
    const prevStorage = storageMode;
    setStorageMode(mode);
    void persistAgentPrefs({ nextStorage: mode, nextPerm: permissionLevel, prevStorage, modelId: p.defaultModel, endpoint: p.byosEndpoint });
  };

  const handlePermissionLevelChange = (level: string) => {
    const prevPerm = permissionLevel;
    setPermissionLevel(level);
    void persistAgentPrefs({ nextStorage: storageMode, nextPerm: level, prevPerm, modelId: p.defaultModel, endpoint: p.byosEndpoint });
  };

  const verifyEndpoint = async (
    provider: ApiBentoProviderId,
    endpointOrKey: string,
    optionalToken?: string
  ) => {
    if (!endpointOrKey.trim()) {
      setVerifyStates((prev) => ({
        ...prev,
        [provider]: { status: "error", message: t("settings.enterKeyOrEndpoint", "Vui lòng nhập Key hoặc Endpoint") },
      }));
      return;
    }

    setVerifyStates((prev) => ({
      ...prev,
      [provider]: { status: "verifying" },
    }));

    try {
      const result = await apiWorkbenchSettings.verifyProviderKey({
        providerId: provider,
        apiKey: provider === "byos" ? (optionalToken ?? "").trim() : endpointOrKey.trim(),
        endpointUrl: provider === "byos" ? endpointOrKey.trim() : "",
      });
      setVerifyStates((prev) => ({
        ...prev,
        [provider]: {
          status: result.valid ? "success" : "error",
          message: result.message || (result.valid ? `OK (${result.status})` : "Verification failed"),
        },
      }));
    } catch (err) {
      setVerifyStates((prev) => ({
        ...prev,
        [provider]: {
          status: "error",
          message: err instanceof Error ? err.message : t("settings.verifyTimeout", "Không thể kết nối tới Gateway"),
        },
      }));
    }
  };

  const handleToggleConnectTool = (tool: CliDiagnosticItem) => {
    const isCurrentlyConnected = connectedIds.includes(tool.id);
    setDisconnectedIds((prev) =>
      isCurrentlyConnected ? [...prev, tool.id] : prev.filter((id) => id !== tool.id)
    );
    const primaryModel = tool.models?.[0]?.id;
    if (!isCurrentlyConnected && primaryModel) {
      p.onDefaultModelChange(primaryModel);
    }
  };

  return (
    <>
      <div>
        <div style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)", marginBottom: "2px" }}>
          {t("settings.modelsTitle", "Kết nối Mô hình AI & Quyền Runtime")}
        </div>
        <div style={{ fontSize: "11px", color: "var(--text-muted)", marginBottom: "4px", lineHeight: "1.4" }}>
          {t("settings.modelsDesc", "Tự động phát hiện công cụ AI trên máy, khóa API nhà cung cấp và chính sách Runtime. Chọn mô hình theo từng tác vụ ngay tại khung Chat.")}
        </div>
        {daemonNotice && (
          <div
            role="status"
            data-testid="settings-models-daemon-notice"
            style={{
              padding: "6px 10px",
              marginTop: "6px",
              borderRadius: "var(--radius-xs)",
              backgroundColor: "var(--bg-subtle)",
              border: "1px solid var(--state-warning)",
              color: "var(--state-warning)",
              fontSize: "11px",
            }}
          >
            {daemonNotice}
          </div>
        )}
      </div>

      <SettingsDetectedProvidersPanel
        readyTools={doctor.readyTools}
        setupTools={doctor.setupTools}
        moreTools={doctor.moreTools}
        connectedIds={connectedIds}
        loading={doctor.loading}
        probingId={doctor.probingId}
        onToggleConnect={handleToggleConnectTool}
        onRecheckAll={() => { void doctor.refresh({ active: true, refreshUsage: true }); }}
        onProbeTool={(cliId) => { void doctor.probeTool(cliId); }}
      />

      <SettingsApiBentoPanel
        ephemeralApiKeys={p.ephemeralApiKeys}
        onEphemeralKeyChange={p.onEphemeralKeyChange}
        byosEndpoint={p.byosEndpoint}
        onByosEndpointChange={p.onByosEndpointChange}
        showKeys={p.showKeys}
        onToggleShowKeys={p.onToggleShowKeys}
        verifyStates={verifyStates}
        onVerifyProvider={(provider, val, tok) => { void verifyEndpoint(provider, val, tok); }}
      />

      <SettingsRuntimeSummaryBento
        storageMode={storageMode}
        permissionLevel={permissionLevel}
        onChangeStorageMode={handleStorageModeChange}
        onChangePermissionLevel={handlePermissionLevelChange}
      />
    </>
  );
};

export const SettingsGeneralModelsPane: React.FC<SettingsGeneralModelsPaneProps> = (props) =>
  props.selectedTab === "general" ? <GeneralAppearanceSection {...props} /> : <ModelsByosSection {...props} />;
