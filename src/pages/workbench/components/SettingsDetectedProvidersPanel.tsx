// @story 04.3, 11.1
// @implements Bento 1: Auto-Detected Local BYOS / CLI Catalog + Live POSIX 0o600 FD / -PGID Sandbox Status
import React, { useEffect, useState } from "react";
import { apiBotFleet } from "../../../api/bot-fleet";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type {
  BotFleetProvidersStatusData,
  CliAdapterStatus,
} from "../../../lib/validations/bot-fleet";
import type { CliDiagnosticItem } from "../../../lib/validations/workbench-settings";

export interface SettingsDetectedProvidersPanelProps {
  readyTools: CliDiagnosticItem[];
  setupTools: CliDiagnosticItem[];
  moreTools: CliDiagnosticItem[];
  connectedIds: string[];
  loading: boolean;
  probingId: string | null;
  onToggleConnect: (tool: CliDiagnosticItem) => void;
  onRecheckAll: () => void;
  onProbeTool: (cliId: string) => void;
  botFleetStatus?: BotFleetProvidersStatusData | null;
}

const ICON_SIZE_SM = 11;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_MEDIUM = 500;
const LINE_HEIGHT_BODY = 1.4;

const SETUP_HINT_CMD: Record<string, string> = {
  codex: "codex login",
  claude: "claude",
  gemini: "gemini",
  agy: "agy",
  ollama: "ollama serve",
  copilot: "gh auth login",
  opencode: "opencode",
  qwen: "qwen",
  pi: "pi",
};

function useLiveBotFleetStatus(
  overrideStatus: BotFleetProvidersStatusData | null | undefined,
  refreshTrigger: boolean
): BotFleetProvidersStatusData | null {
  const [liveStatus, setLiveStatus] = useState<BotFleetProvidersStatusData | null>(
    overrideStatus ?? null
  );

  useEffect(() => {
    if (overrideStatus !== undefined) {
      setLiveStatus(overrideStatus);
      return;
    }
    let active = true;
    apiBotFleet
      .getProvidersStatus()
      .then((res) => {
        if (active) setLiveStatus(res.data);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [overrideStatus, refreshTrigger]);

  return liveStatus;
}

const DetectedPanelHeader: React.FC<{ loading: boolean; onRecheckAll: () => void }> = ({
  loading,
  onRecheckAll,
}) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
      <div>
        <div style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
          {t("settings.detectedByosTitle", "1. Công cụ AI có sẵn trên máy (Chọn 1 chạm)")}
        </div>
        <div style={{ fontSize: "10.5px", color: "var(--text-muted)" }}>
          {t("settings.detectedByosDesc", "Tự động nhận diện công cụ AI đã cài đặt và đăng nhập — bấm chọn để dùng ngay.")}
        </div>
      </div>
      <button
        type="button"
        data-testid="settings-recheck-providers-btn"
        onClick={onRecheckAll}
        disabled={loading}
        style={{ height: "26px", padding: "0 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", fontSize: "11px", fontWeight: FONT_WEIGHT_MEDIUM, cursor: loading ? "wait" : "pointer", display: "inline-flex", alignItems: "center", gap: "5px", flexShrink: 0 }}
      >
        <BrandIcon name="refresh" size={ICON_SIZE_SM} color="currentColor" />
        <span>{loading ? t("settings.checkingProviders", "Đang quét...") : t("settings.checkAgain", "Kiểm tra lại")}</span>
      </button>
    </div>
  );
};

const SandboxStatusStrip: React.FC<{ status: BotFleetProvidersStatusData }> = ({ status }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <div
      data-testid="byos-sandbox-status-badge"
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        gap: "8px",
        padding: "5px 8px",
        borderRadius: "var(--radius-xs)",
        backgroundColor: "var(--state-success-soft)",
        border: "1px solid var(--state-success)",
        fontSize: "10px",
        color: "var(--text-primary)",
      }}
    >
      <span style={{ display: "inline-flex", alignItems: "center", gap: "5px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
        <BrandIcon name="security" size={ICON_SIZE_SM} color="var(--state-success)" />
        <span>
          {t("settings.sandboxBadgeLabel", "Hộp cát POSIX 0o600 FD + -PGID")} ·{" "}
          {formatNumber(status.adapterCount)} {t("settings.adaptersCountLabel", "bộ điều hợp CLI")}
        </span>
      </span>
      <span style={{ color: "var(--state-success)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
        {formatNumber(status.readyAdapterCount)}/{formatNumber(status.adapterCount)}{" "}
        {t("settings.sandboxReadyShort", "sẵn sàng")}
      </span>
    </div>
  );
};

const ReadyBentoTile: React.FC<{
  tool: CliDiagnosticItem;
  connected: boolean;
  adapterMeta?: CliAdapterStatus;
  onClick: () => void;
}> = ({ tool, connected, adapterMeta, onClick }) => {
  const { t } = useTranslation();
  const statusLabel = connected
    ? `✓ ${t("settings.connectedShort", "Đã nối")}`
    : t("settings.providerReady", "Sẵn sàng");

  return (
    <button
      type="button"
      data-testid={`provider-row-${tool.id}`}
      onClick={onClick}
      style={{
        padding: "8px 10px",
        borderRadius: "var(--radius-xs)",
        border: connected ? "1px solid var(--state-success)" : "1px solid var(--border-default)",
        backgroundColor: connected ? "var(--state-success-soft)" : "var(--bg-workspace)",
        textAlign: "left",
        cursor: "pointer",
        display: "flex",
        flexDirection: "column",
        gap: "3px",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
        <span style={{ fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {tool.name}
        </span>
        <span style={{ fontSize: "10px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: connected ? "var(--state-success)" : "var(--text-muted)", flexShrink: 0 }}>
          {statusLabel}
        </span>
      </div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
        <span style={{ fontSize: "10px", color: "var(--text-muted)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {tool.accountLabel || `${tool.command} (${t("settings.detectedLocal", "Phát hiện trên máy")})`}
        </span>
        {adapterMeta && (
          <span data-testid={`adapter-transport-${tool.id}`} style={{ fontSize: "9px", fontFamily: "var(--font-mono)", color: "var(--text-secondary)", flexShrink: 0 }}>
            0600 FD
          </span>
        )}
      </div>
    </button>
  );
};

const SetupDrawerCard: React.FC<{
  tool: CliDiagnosticItem;
  probing: boolean;
  onProbe: () => void;
  onClose: () => void;
}> = ({ tool, probing, onProbe, onClose }) => {
  const { t } = useTranslation();
  const cmdHint = SETUP_HINT_CMD[tool.id] || tool.command;

  return (
    <div data-testid={`setup-flow-${tool.id}`} style={{ padding: "9px 10px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-default)", display: "flex", flexDirection: "column", gap: "6px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
        <span style={{ fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
          {t("settings.setupTitlePrefix", "Thiết lập")} {tool.name}
        </span>
        <button type="button" onClick={onClose} style={{ border: "none", background: "transparent", color: "var(--text-muted)", fontSize: "12px", cursor: "pointer" }}>×</button>
      </div>
      <div style={{ fontSize: "10.5px", color: "var(--text-secondary)", lineHeight: LINE_HEIGHT_BODY }}>
        {tool.authReason || t("settings.setupStepInstruction", "Chạy lệnh bên dưới trên terminal rồi bấm Kiểm tra lại để tự động kết nối.")}
      </div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", padding: "5px 8px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-canvas)", border: "1px solid var(--border-subtle)" }}>
        <code style={{ fontSize: "10.5px", fontFamily: "var(--font-mono)", color: "var(--text-primary)" }}>{cmdHint}</code>
        <button
          type="button"
          data-testid={`probe-setup-btn-${tool.id}`}
          onClick={onProbe}
          disabled={probing}
          style={{ height: "22px", padding: "0 9px", borderRadius: "var(--radius-pill)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--action-primary)", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer" }}
        >
          {probing ? "..." : t("settings.probeBtn", "Kiểm tra")}
        </button>
      </div>
      <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>
        {t("settings.sandboxActiveNote", "Đã kích hoạt cô lập tiến trình $COWOK_SESSION_DIR (O_NOFOLLOW + 0o600 FD + -PGID).")}
      </div>
    </div>
  );
};

const PendingToolsCollapse: React.FC<{
  pendingTools: CliDiagnosticItem[];
  showUnconfigured: boolean;
  onToggle: () => void;
  onSelect: (tool: CliDiagnosticItem) => void;
}> = ({ pendingTools, showUnconfigured, onToggle, onSelect }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <div>
      <button
        type="button"
        data-testid="toggle-more-providers-btn"
        onClick={onToggle}
        style={{ width: "100%", padding: "6px 9px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", color: "var(--text-secondary)", fontSize: "11px", fontWeight: FONT_WEIGHT_MEDIUM, display: "flex", alignItems: "center", justifyContent: "space-between", cursor: "pointer" }}
      >
        <span>{t("settings.moreAiProviders", "Thiết lập thêm công cụ AI khác")} ({formatNumber(pendingTools.length)})</span>
        <span>{showUnconfigured ? "▲" : "▼"}</span>
      </button>
      {showUnconfigured && (
        <div data-testid="setup-byos-list" style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))", gap: "5px", marginTop: "6px" }}>
          {pendingTools.map((tool) => (
            <button
              key={tool.id}
              type="button"
              data-testid={`provider-row-${tool.id}`}
              onClick={() => onSelect(tool)}
              style={{ padding: "6px 8px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-subtle)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", fontSize: "10.5px", color: "var(--text-primary)", cursor: "pointer", textAlign: "left" }}
            >
              <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{tool.name}</span>
              <span style={{ fontSize: "9.5px", color: tool.installed ? "var(--state-warning)" : "var(--text-muted)", flexShrink: 0 }}>
                {tool.installed ? t("settings.setUpBtn", "Thiết lập") : tool.command}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
};

export const SettingsDetectedProvidersPanel: React.FC<SettingsDetectedProvidersPanelProps> = (p) => {
  const { t } = useTranslation();
  const [showUnconfigured, setShowUnconfigured] = useState(false);
  const [setupTarget, setSetupTarget] = useState<CliDiagnosticItem | null>(null);
  const fleetStatus = useLiveBotFleetStatus(p.botFleetStatus, p.loading);
  const adapterMap = new Map((fleetStatus?.adapters ?? []).map((a) => [a.id, a]));
  const pendingTools = [...p.setupTools, ...p.moreTools];

  return (
    <div data-testid="detected-byos-panel" style={{ padding: "10px 12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", display: "flex", flexDirection: "column", gap: "8px" }}>
      <DetectedPanelHeader loading={p.loading} onRecheckAll={p.onRecheckAll} />
      {fleetStatus && <SandboxStatusStrip status={fleetStatus} />}
      {p.readyTools.length > 0 ? (
        <div data-testid="ready-byos-list" style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(210px, 1fr))", gap: "6px" }}>
          {p.readyTools.map((tool) => (
            <ReadyBentoTile key={tool.id} tool={tool} connected={p.connectedIds.includes(tool.id)} adapterMeta={adapterMap.get(tool.id)} onClick={() => p.onToggleConnect(tool)} />
          ))}
        </div>
      ) : (
        !p.loading && (
          <div style={{ fontSize: "11px", color: "var(--text-muted)", padding: "2px" }}>
            {t("settings.noLocalByosDetected", "Chưa phát hiện CLI AI nào sẵn sàng trên máy; mở danh mục bên dưới để thiết lập hoặc dùng API Gateway.")}
          </div>
        )
      )}
      {pendingTools.length > 0 && (
        <PendingToolsCollapse pendingTools={pendingTools} showUnconfigured={showUnconfigured} onToggle={() => setShowUnconfigured((v) => !v)} onSelect={setSetupTarget} />
      )}
      {setupTarget && (
        <SetupDrawerCard tool={setupTarget} probing={p.probingId === setupTarget.id} onProbe={() => p.onProbeTool(setupTarget.id)} onClose={() => setSetupTarget(null)} />
      )}
    </div>
  );
};
