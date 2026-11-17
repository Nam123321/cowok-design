// @story 04.1, 08.4
import React, { useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { apiWorkbenchSettings } from "../../../api/workbench-settings";
import { PluginRegistryPanel } from "../../../features/plugins/PluginRegistryPanel";
import { LearnCardReviewQueue } from "../../../features/plugins/LearnCardReviewQueue";
import { CronSyncStatus } from "../../../features/plugins/CronSyncStatus";
import { usePluginRegistry } from "../../../features/plugins/hooks/usePluginRegistry";
import { DEFAULT_FLOW_PROJECT_ID } from "../constants";
import { useAutomationsHealthState, useMcpServersState } from "../hooks/useSettingsModalState";

interface SettingsMcpAutomationsPaneProps {
  selectedTab: "mcp" | "automations";
  projectId?: string;
}

const McpHostNoticeCard: React.FC = () => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)" }}>
      <div style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)", marginBottom: "3px" }}>
        {t("settings.mcpHostTitle", "Máy chủ MCP Cowok-Design & AgentSkills Ecosystem")}
      </div>
      <div style={{ fontSize: "10.5px", color: "var(--text-muted)", lineHeight: "1.4" }}>
        {t(
          "settings.mcpHostStory084Notice",
          "Quản lý Client MCPs, AgentSkills Plugin Registry, Isolated Staging (HUMAN_ONLY), và LEARN Knowledge Cards trên SQLite WAL V9."
        )}
      </div>
    </div>
  );
};

const McpClientRegistryList: React.FC = () => {
  const { t } = useTranslation();
  const { servers, setServers, mcpError, setMcpError } = useMcpServersState();
  const [adding, setAdding] = useState(false);
  const [newName, setNewName] = useState("");
  const [newCommand, setNewCommand] = useState("");

  const handleAddServer = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newName.trim() || !newCommand.trim()) {
      setMcpError(t("settings.mcpEmptyValidation", "Vui lòng nhập đầy đủ tên và lệnh/URL MCP."));
      return;
    }
    setMcpError(null);
    try {
      const saved = await apiWorkbenchSettings.upsertMcpServer({
        serverSlug: newName.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-"),
        displayName: newName.trim(),
        transportKind: newCommand.trim().startsWith("http") ? "http_sse" : "stdio",
        commandOrUrl: newCommand.trim(),
        permissionTier: "read_only",
        enabled: true,
      });
      setServers((prev) => [...prev.filter((s) => s.id !== saved.id), saved]);
      setNewName("");
      setNewCommand("");
      setAdding(false);
    } catch (err) {
      const detail = err instanceof Error ? `: ${err.message}` : "";
      setMcpError(`${t("settings.mcpDaemonError", "Không thể lưu MCP server lên daemon")}${detail}`);
    }
  };

  const handleDeleteServer = async (id: string) => {
    setMcpError(null);
    const prevSnapshot = servers;
    setServers((prev) => prev.filter((s) => s.id !== id));
    try {
      await apiWorkbenchSettings.deleteMcpServer(id);
    } catch (err) {
      setServers(prevSnapshot);
      const detail = err instanceof Error ? `: ${err.message}` : "";
      setMcpError(`${t("settings.mcpDaemonError", "Không thể xóa MCP server trên daemon")}${detail}`);
    }
  };

  const displayedRows = servers.map((s) => ({
    id: s.id,
    name: s.displayName,
    desc: `${s.transportKind.toUpperCase()} • ${s.commandOrUrl}`,
    status: s.lastHealthStatus.toUpperCase(),
    removable: true,
  }));

  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: "6px" }}>
        <label style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-secondary)" }}>
          {t("settings.mcpClientTitle", "Danh sách Máy chủ MCP đã đăng ký (Client MCPs)")}
        </label>
        <div style={{ display: "flex", gap: "6px" }}>
          <button type="button" onClick={() => setAdding((v) => !v)} className="cowok-btn cowok-btn-primary" style={{ padding: "4px 10px", fontSize: "10.5px", fontWeight: 600, borderRadius: "var(--radius-xs)", cursor: "pointer" }}>
            {adding ? t("settings.cancelBtn", "Hủy") : t("settings.addMcpBtn", "+ Thêm MCP Server")}
          </button>
        </div>
      </div>
      {mcpError && (
        <div
          role="alert"
          data-testid="settings-mcp-error"
          style={{
            padding: "6px 10px",
            marginBottom: "8px",
            borderRadius: "var(--radius-xs)",
            backgroundColor: "var(--bg-subtle)",
            border: "1px solid var(--state-danger)",
            color: "var(--state-danger)",
            fontSize: "11px",
          }}
        >
          {mcpError}
        </div>
      )}
      {adding && (
        <form onSubmit={handleAddServer} style={{ display: "flex", gap: "6px", marginBottom: "8px" }}>
          <input type="text" value={newName} onChange={(e) => setNewName(e.target.value)} placeholder={t("settings.mcpNamePlaceholder", "Tên MCP (vd: custom-figma)")} className="cowok-input" style={{ flex: 1, height: "28px", padding: "0 8px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)" }} />
          <input type="text" value={newCommand} onChange={(e) => setNewCommand(e.target.value)} placeholder={t("settings.mcpCmdPlaceholder", "Lệnh / URL (vd: npx -y ...)")} className="cowok-input" style={{ flex: 2, height: "28px", padding: "0 8px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)" }} />
          <button type="submit" className="cowok-btn cowok-btn-primary" style={{ height: "28px", padding: "0 10px", fontSize: "10.5px", fontWeight: 600, borderRadius: "var(--radius-xs)", cursor: "pointer" }}>{t("settings.saveBtn", "Lưu")}</button>
        </form>
      )}
      <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
        {displayedRows.length === 0 && (
          <div style={{ padding: "10px 12px", borderRadius: "var(--radius-xs)", border: "1px dashed var(--border-subtle)", fontSize: "11px", color: "var(--text-muted)" }}>
            {t("settings.mcpEmptyRegistry", "Chưa có MCP Server nào được cấu hình trong Daemon SQLite.")}
          </div>
        )}
        {displayedRows.map((mcp) => (
          <div key={mcp.id} style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
            <div>
              <div style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>{mcp.name}</div>
              <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>{mcp.desc}</div>
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
              <span style={{ fontSize: "9.5px", fontWeight: 600, padding: "2px 6px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--state-success-soft)", color: "var(--state-success)", border: "1px solid var(--border-subtle)" }}>{mcp.status}</span>
              {mcp.removable && (
                <button type="button" onClick={() => void handleDeleteServer(mcp.id)} aria-label={t("common.delete", "Xóa")} style={{ border: "none", background: "transparent", color: "var(--text-muted)", fontSize: "10px", cursor: "pointer" }}>×</button>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

export const SettingsMcpAutomationsPane: React.FC<SettingsMcpAutomationsPaneProps> = ({
  selectedTab,
  projectId = DEFAULT_FLOW_PROJECT_ID,
}) => {
  const { t } = useTranslation();
  const daemonHealth = useAutomationsHealthState(selectedTab);
  const pluginRegistry = usePluginRegistry(projectId, selectedTab);

  if (selectedTab === "mcp") {
    return (
      <>
        <McpHostNoticeCard />
        <McpClientRegistryList />
        <PluginRegistryPanel
          plugins={pluginRegistry.plugins}
          loading={pluginRegistry.loading}
          error={pluginRegistry.error}
          pendingActionId={pluginRegistry.actionPendingId}
          actionFeedback={pluginRegistry.actionFeedback}
          onRefresh={() => void pluginRegistry.refreshAll()}
          onTransition={(plugin, target) => void pluginRegistry.transitionPlugin(plugin, target)}
          onPromoteStaging={(plugin, reason) => void pluginRegistry.promoteStaging(plugin, reason)}
          onDiscardStaging={(plugin, reason) => void pluginRegistry.discardStaging(plugin, reason)}
        />
        <LearnCardReviewQueue
          cards={pluginRegistry.learnCards}
          loading={pluginRegistry.loading}
          error={pluginRegistry.error}
          pendingActionId={pluginRegistry.actionPendingId}
          onRefresh={() => void pluginRegistry.refreshAll()}
          onApprove={(card, reason) => void pluginRegistry.approveLearnCard(card, reason)}
          onReject={(card, reason) => void pluginRegistry.rejectLearnCard(card, reason)}
        />
      </>
    );
  }
  const daemonBadgeLabel =
    daemonHealth === "online"
      ? t("settings.daemonStatusOnline", "ONLINE")
      : daemonHealth === "offline"
      ? t("settings.daemonStatusOffline", "OFFLINE")
      : t("settings.daemonStatusChecking", "CHECKING");
  const daemonBadgeColors =
    daemonHealth === "online"
      ? { backgroundColor: "var(--state-success-soft)", color: "var(--state-success)" }
      : daemonHealth === "offline"
      ? { backgroundColor: "var(--bg-subtle)", color: "var(--state-danger)" }
      : { backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)" };
  return (
    <>
      <div>
        <div style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)", marginBottom: "3px" }}>{t("settings.automationsTitle", "Tác vụ nền & Tự động hóa định kỳ")}</div>
        <div style={{ fontSize: "11px", color: "var(--text-muted)", marginBottom: "12px", lineHeight: "1.4" }}>{t("settings.automationsDesc", "Theo dõi tiến trình Rust SQLite WAL Daemon, cron triggers và webhook callbacks")}</div>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
        <div style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div>
            <div style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-primary)" }}>{t("settings.daemonLoopbackTitle", "Cowok Rust SQLite WAL Daemon (Loopback)")}</div>
            <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>{t("settings.daemonLoopbackSubtitle", "127.0.0.1 • Dual-Ingress HMAC-SHA256 v2")}</div>
          </div>
          <span data-testid="automations-daemon-status" style={{ fontSize: "9.5px", fontWeight: 600, padding: "2px 6px", borderRadius: "10px", border: "1px solid var(--border-subtle)", ...daemonBadgeColors }}>{daemonBadgeLabel}</span>
        </div>
        <div style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div>
            <div style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-primary)" }}>{t("settings.autoSnapshotTitle", "State & Graph AST Snapshots")}</div>
            <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>{t("settings.autoSnapshotDesc", "On-demand only • Trigger manual snapshot from Toolbar or Flow Workbench")}</div>
          </div>
          <span data-testid="automations-snapshot-status" style={{ fontSize: "9.5px", fontWeight: 600, padding: "2px 6px", borderRadius: "10px", backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)" }}>
            {daemonHealth === "online" ? t("settings.snapshotCadenceReady", "MANUAL ONLY") : t("settings.snapshotCadencePaused", "NOT SCHEDULED")}
          </span>
        </div>
      </div>
      <CronSyncStatus
        plugins={pluginRegistry.plugins}
        schedulesByPlugin={pluginRegistry.schedulesByPlugin}
        loading={pluginRegistry.loading}
        error={pluginRegistry.error}
        pendingActionId={pluginRegistry.actionPendingId}
        onRefresh={() => void pluginRegistry.refreshAll()}
        onSyncNow={(plugin, refreshAuth) => void pluginRegistry.triggerSyncNow(plugin, refreshAuth)}
        onSaveSchedule={(plugin, cronExpr, url) => void pluginRegistry.saveSchedule(plugin, cronExpr, url)}
      />
    </>
  );
};
