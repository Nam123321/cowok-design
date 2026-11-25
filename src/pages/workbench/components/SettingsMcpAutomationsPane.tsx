// @story 04.1, 08.4, 11.2
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { apiWorkbenchSettings } from "../../../api/workbench-settings";
import { PluginRegistryPanel } from "../../../features/plugins/PluginRegistryPanel";
import { LearnCardReviewQueue } from "../../../features/plugins/LearnCardReviewQueue";
import { CronSyncStatus } from "../../../features/plugins/CronSyncStatus";
import { validateCronSchedules, validateLearnCards, validatePluginCards } from "../../../features/plugin-center/cards";
import { usePluginRegistry } from "../../../features/plugins/hooks/usePluginRegistry";
import { HostInstallerList } from "../../../features/mcp-gateway/HostInstallerList";
import { useMcpGatewayController } from "../../../features/mcp-gateway/useMcpGatewayController";
import { DEFAULT_FLOW_PROJECT_ID } from "../constants";
import { useAutomationsHealthState, useMcpServersState } from "../hooks/useSettingsModalState";

type McpServerEntryDto = Awaited<ReturnType<typeof apiWorkbenchSettings.upsertMcpServer>>;

interface SettingsMcpAutomationsPaneProps {
  selectedTab: "mcp" | "automations";
  projectId?: string;
}

const FONT_WEIGHT_SEMIBOLD = 600;
const LINK_ICON_SIZE = 12;

const DAEMON_BADGE_MAP: Record<
  "online" | "offline" | "checking",
  { key: string; fallback: string; colors: React.CSSProperties }
> = {
  online: {
    key: "settings.daemonStatusOnline",
    fallback: "ONLINE",
    colors: { backgroundColor: "var(--state-success-soft)", color: "var(--state-success)" },
  },
  offline: {
    key: "settings.daemonStatusOffline",
    fallback: "OFFLINE",
    colors: { backgroundColor: "var(--state-danger-soft)", color: "var(--state-danger)" },
  },
  checking: {
    key: "settings.daemonStatusChecking",
    fallback: "CHECKING",
    colors: { backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)" },
  },
};

function toSafeServerSlug(rawName: string): string {
  const ascii = rawName.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
  return ascii || `mcp-${Date.now()}`;
}

const McpHostNoticeCard: React.FC = () => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)" }}>
      <div style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", marginBottom: "3px" }}>
        {t("settings.mcpHostTitle", "Máy chủ MCP Cowok-Design & Tiện ích mở rộng")}
      </div>
      <div style={{ fontSize: "10.5px", color: "var(--text-muted)", lineHeight: "1.45" }}>
        {t(
          "settings.mcpHostOverviewNotice",
          "Kết nối nhanh ứng dụng AI bên ngoài với Cowok-Design, quản lý máy chủ MCP tùy chỉnh và kiểm soát phân quyền công cụ an toàn."
        )}
      </div>
    </div>
  );
};

const SUMMARY_CARD_STYLE: React.CSSProperties = {
  padding: "12px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-surface)",
  display: "flex",
  flexDirection: "column",
  gap: "10px",
};

const OPEN_CENTER_LINK_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: "6px",
  padding: "7px 12px",
  fontSize: "11px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  borderRadius: "var(--radius-xs)",
  textDecoration: "none",
};

const SummaryMetricTile: React.FC<{ label: string; value: number; color: string }> = ({ label, value, color }) => (
  <div style={{ padding: "8px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-subtle)" }}>
    <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>{label}</div>
    <div style={{ fontSize: "14px", fontWeight: FONT_WEIGHT_SEMIBOLD, color }}>{value}</div>
  </div>
);

const McpCenterSummaryCard: React.FC<{
  gateway: ReturnType<typeof useMcpGatewayController>;
}> = ({ gateway }) => {
  const { t } = useTranslation();
  const activeToolsCount = gateway.policies.filter((p) => !p.lockedHumanOnly && p.permissionTier !== "HUMAN_ONLY" && p.enabled).length;
  const lockedToolsCount = gateway.policies.filter((p) => p.lockedHumanOnly || p.permissionTier === "HUMAN_ONLY").length;

  return (
    <div data-testid="mcp-center-summary-card" style={SUMMARY_CARD_STYLE}>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: "8px" }}>
        <SummaryMetricTile label={t("mcpGateway.summaryActiveTools", "Công cụ đang bật")} value={activeToolsCount} color="var(--state-success)" />
        <SummaryMetricTile label={t("mcpGateway.summaryLockedActions", "Tác vụ khóa bảo mật")} value={lockedToolsCount} color="var(--state-danger)" />
        <SummaryMetricTile label={t("mcpGateway.summaryRecentActivity", "Hoạt động gần đây")} value={gateway.activities.length} color="var(--text-primary)" />
      </div>
      <a href="/plugins" data-testid="open-full-mcp-center-btn" className="cowok-btn cowok-btn-primary" style={OPEN_CENTER_LINK_STYLE}>
        <BrandIcon name="external-link" size={LINK_ICON_SIZE} color="currentColor" />
        <span>{t("mcpGateway.openFullCenterBtn", "Mở Trung tâm MCP & Nhật ký đầy đủ")}</span>
      </a>
    </div>
  );
};

const McpAddServerForm: React.FC<{
  onCreated: (saved: McpServerEntryDto) => void;
  onError: (msg: string | null) => void;
}> = ({ onCreated, onError }) => {
  const { t } = useTranslation();
  const [newName, setNewName] = useState("");
  const [newCommand, setNewCommand] = useState("");

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const cleanName = newName.trim();
    const cleanCmd = newCommand.trim();
    if (!cleanName || !cleanCmd) {
      onError(t("settings.mcpEmptyValidation", "Vui lòng nhập đầy đủ tên và lệnh/URL MCP."));
      return;
    }
    onError(null);
    try {
      const saved = await apiWorkbenchSettings.upsertMcpServer({
        serverSlug: toSafeServerSlug(cleanName),
        displayName: cleanName,
        transportKind: /^https?:\/\//i.test(cleanCmd) ? "http_sse" : "stdio",
        commandOrUrl: cleanCmd,
        permissionTier: "read_only",
        enabled: true,
      });
      setNewName("");
      setNewCommand("");
      onCreated(saved);
    } catch (err) {
      const detail = err instanceof Error ? `: ${err.message}` : "";
      onError(`${t("settings.mcpDaemonError", "Không thể lưu máy chủ MCP")}${detail}`);
    }
  };

  return (
    <form onSubmit={handleSubmit} style={{ display: "flex", gap: "6px", marginBottom: "8px" }}>
      <input type="text" value={newName} onChange={(e) => setNewName(e.target.value)} placeholder={t("settings.mcpNamePlaceholder", "Tên MCP (vd: custom-figma)")} className="cowok-input" style={{ flex: 1, height: "28px", padding: "0 8px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)" }} />
      <input type="text" value={newCommand} onChange={(e) => setNewCommand(e.target.value)} placeholder={t("settings.mcpCmdPlaceholder", "Lệnh / URL (vd: npx -y ...)")} className="cowok-input" style={{ flex: 2, height: "28px", padding: "0 8px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)" }} />
      <button type="submit" className="cowok-btn cowok-btn-primary" style={{ height: "28px", padding: "0 10px", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, borderRadius: "var(--radius-xs)", cursor: "pointer" }}>{t("settings.saveBtn", "Lưu")}</button>
    </form>
  );
};

const McpServerRows: React.FC<{
  servers: McpServerEntryDto[];
  onDelete: (id: string) => void;
}> = ({ servers, onDelete }) => {
  const { t } = useTranslation();
  if (servers.length === 0) {
    return (
      <div style={{ padding: "10px 12px", borderRadius: "var(--radius-xs)", border: "1px dashed var(--border-subtle)", fontSize: "11px", color: "var(--text-muted)" }}>
        {t("settings.mcpEmptyCustomServers", "Chưa có máy chủ MCP tùy chỉnh nào được đăng ký.")}
      </div>
    );
  }
  return (
    <>
      {servers.map((mcp) => (
        <div key={mcp.id} style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
          <div>
            <div style={{ fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>{mcp.displayName}</div>
            <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>{mcp.transportKind.toUpperCase()} • {mcp.commandOrUrl}</div>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <span style={{ fontSize: "9.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, padding: "2px 6px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--state-success-soft)", color: "var(--state-success)", border: "1px solid var(--state-success)" }}>{mcp.lastHealthStatus.toUpperCase()}</span>
            <button type="button" onClick={() => onDelete(mcp.id)} aria-label={t("common.delete", "Xóa")} style={{ border: "none", background: "transparent", color: "var(--text-muted)", fontSize: "10px", cursor: "pointer" }}>×</button>
          </div>
        </div>
      ))}
    </>
  );
};

const McpClientRegistryList: React.FC = () => {
  const { t } = useTranslation();
  const { servers, setServers, mcpError, setMcpError } = useMcpServersState();
  const [adding, setAdding] = useState(false);

  const handleDeleteServer = async (id: string) => {
    setMcpError(null);
    const prevSnapshot = servers;
    setServers((prev) => prev.filter((s) => s.id !== id));
    try {
      await apiWorkbenchSettings.deleteMcpServer(id);
    } catch (err) {
      setServers(prevSnapshot);
      const detail = err instanceof Error ? `: ${err.message}` : "";
      setMcpError(`${t("settings.mcpDaemonError", "Không thể xóa máy chủ MCP")}${detail}`);
    }
  };

  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: "6px" }}>
        <label style={{ fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-secondary)" }}>
          {t("settings.mcpClientTitle", "Danh sách Máy chủ MCP đã đăng ký")}
        </label>
        <button type="button" onClick={() => setAdding((v) => !v)} className="cowok-btn cowok-btn-primary" style={{ padding: "4px 10px", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, borderRadius: "var(--radius-xs)", cursor: "pointer" }}>
          {adding ? t("settings.cancelBtn", "Hủy") : t("settings.addMcpBtn", "+ Thêm MCP Server")}
        </button>
      </div>
      {mcpError && (
        <div role="alert" data-testid="settings-mcp-error" style={{ padding: "6px 10px", marginBottom: "8px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--state-danger-soft)", border: "1px solid var(--state-danger)", color: "var(--state-danger)", fontSize: "11px" }}>
          {mcpError}
        </div>
      )}
      {adding && (
        <McpAddServerForm
          onError={setMcpError}
          onCreated={(saved) => {
            setServers((prev) => [...prev.filter((s) => s.id !== saved.id), saved]);
            setAdding(false);
          }}
        />
      )}
      <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
        <McpServerRows servers={servers} onDelete={(id) => void handleDeleteServer(id)} />
      </div>
    </div>
  );
};

const AutomationsStatusSummary: React.FC<{ daemonHealth: "online" | "offline" | "checking" }> = ({ daemonHealth }) => {
  const { t } = useTranslation();
  const badgeSpec = DAEMON_BADGE_MAP[daemonHealth];
  return (
    <>
      <div>
        <div style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", marginBottom: "3px" }}>{t("settings.automationsTitle", "Tác vụ nền & Tự động hóa định kỳ")}</div>
        <div style={{ fontSize: "11px", color: "var(--text-muted)", marginBottom: "12px", lineHeight: "1.4" }}>{t("settings.automationsDesc", "Theo dõi tiến trình dịch vụ nền, lịch đồng bộ và webhook callbacks")}</div>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
        <div style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div>
            <div style={{ fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>{t("settings.daemonLoopbackTitle", "Dịch vụ cục bộ Cowok (Loopback)")}</div>
            <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>{t("settings.daemonLoopbackSubtitle", "127.0.0.1 • Kết nối cục bộ bảo mật")}</div>
          </div>
          <span data-testid="automations-daemon-status" style={{ fontSize: "9.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, padding: "2px 6px", borderRadius: "10px", border: "1px solid var(--border-subtle)", ...badgeSpec.colors }}>{t(badgeSpec.key, badgeSpec.fallback)}</span>
        </div>
        <div style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div>
            <div style={{ fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>{t("settings.autoSnapshotTitle", "State & Graph AST Snapshots")}</div>
            <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>{t("settings.autoSnapshotDesc", "On-demand only • Trigger manual snapshot from Toolbar or Flow Workbench")}</div>
          </div>
          <span data-testid="automations-snapshot-status" style={{ fontSize: "9.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, padding: "2px 6px", borderRadius: "10px", backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)" }}>
            {daemonHealth === "online" ? t("settings.snapshotCadenceReady", "MANUAL ONLY") : t("settings.snapshotCadencePaused", "NOT SCHEDULED")}
          </span>
        </div>
      </div>
    </>
  );
};

const McpGatewayCompactSection: React.FC<{
  gateway: ReturnType<typeof useMcpGatewayController>;
}> = ({ gateway }) => (
  <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
    {gateway.error && (
      <div role="alert" data-testid="mcp-gateway-error" style={{ padding: "6px 10px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--state-danger-soft)", border: "1px solid var(--state-danger)", color: "var(--state-danger)", fontSize: "11px" }}>
        {gateway.error}
      </div>
    )}
    {gateway.feedback && (
      <div role="status" data-testid="mcp-gateway-feedback" style={{ padding: "6px 10px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--state-success-soft)", border: "1px solid var(--state-success)", color: "var(--state-success)", fontSize: "11px" }}>
        {gateway.feedback}
      </div>
    )}
    <HostInstallerList
      hosts={gateway.hosts}
      serverUrl={gateway.serverUrl}
      busyHostId={gateway.busyHostId}
      loading={gateway.loading}
      onInstall={(hostId) => void gateway.installHost(hostId)}
      onRollback={(hostId, backupId) => void gateway.rollbackHost(hostId, backupId)}
    />
    <McpCenterSummaryCard gateway={gateway} />
  </div>
);

const SettingsMcpTabContent: React.FC<{
  registry: ReturnType<typeof usePluginRegistry>;
  gateway: ReturnType<typeof useMcpGatewayController>;
}> = ({ registry, gateway }) => {
  const pluginCheck = validatePluginCards(registry.plugins);
  const cardCheck = validateLearnCards(registry.learnCards);
  return (
    <>
      <McpHostNoticeCard />
      <McpGatewayCompactSection gateway={gateway} />
      <McpClientRegistryList />
      <PluginRegistryPanel
        plugins={pluginCheck.valid}
        loading={registry.loading}
        error={registry.pluginError ?? pluginCheck.error}
        pendingActionId={registry.actionPendingId}
        actionFeedback={registry.actionFeedback}
        onRefresh={() => void registry.refreshAll()}
        onTransition={(plugin, target) => void registry.transitionPlugin(plugin, target)}
        onQuarantine={(plugin, reason) => void registry.quarantinePlugin(plugin, reason)}
        onPromoteStaging={(plugin, reason) => void registry.promoteStaging(plugin, reason)}
        onDiscardStaging={(plugin, reason) => void registry.discardStaging(plugin, reason)}
      />
      <LearnCardReviewQueue
        cards={cardCheck.valid}
        loading={registry.loading}
        error={registry.learnError ?? cardCheck.error}
        pendingActionId={registry.actionPendingId}
        actionFeedback={registry.actionFeedback}
        onRefresh={() => void registry.refreshAll()}
        onApprove={(card, reason) => void registry.approveLearnCard(card, reason)}
        onReject={(card, reason) => void registry.rejectLearnCard(card, reason)}
      />
    </>
  );
};

export const SettingsMcpAutomationsPane: React.FC<SettingsMcpAutomationsPaneProps> = ({
  selectedTab,
  projectId = DEFAULT_FLOW_PROJECT_ID,
}) => {
  const daemonHealth = useAutomationsHealthState(selectedTab);
  const pluginRegistry = usePluginRegistry(projectId, selectedTab);
  const mcpGateway = useMcpGatewayController(selectedTab === "mcp");

  if (selectedTab === "mcp") {
    return <SettingsMcpTabContent registry={pluginRegistry} gateway={mcpGateway} />;
  }
  const pluginCheck = validatePluginCards(pluginRegistry.plugins);
  const scheduleCheck = validateCronSchedules(pluginRegistry.schedulesByPlugin);
  const cronPlugins = pluginCheck.valid.filter((p) => p.sourceType !== "builtin" || Boolean(p.upstreamRepoUrl));
  return (
    <>
      <AutomationsStatusSummary daemonHealth={daemonHealth} />
      <CronSyncStatus
        plugins={cronPlugins}
        schedulesByPlugin={scheduleCheck.valid}
        loading={pluginRegistry.loading}
        error={pluginRegistry.cronError ?? pluginCheck.error ?? scheduleCheck.error}
        pendingActionId={pluginRegistry.actionPendingId}
        actionFeedback={pluginRegistry.actionFeedback}
        onRefresh={() => void pluginRegistry.refreshAll()}
        onSyncNow={(plugin, refreshAuth) => void pluginRegistry.triggerSyncNow(plugin, refreshAuth)}
        onSaveSchedule={(plugin, cronExpr, url) => pluginRegistry.saveSchedule(plugin, cronExpr, url)}
      />
    </>
  );
};
