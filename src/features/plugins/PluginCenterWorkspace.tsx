// @story 08.4, 11.2
// @implements AC-01, AC-04, AC-05
import React, { useState } from "react";
import { useTranslation } from "../../hooks/useTranslation";
import {
  validateCronSchedules,
  validateLearnCards,
  validatePluginCards,
} from "../plugin-center/cards";
import { CronSyncStatus } from "./CronSyncStatus";
import { LearnCardReviewQueue } from "./LearnCardReviewQueue";
import { PluginRegistryPanel } from "./PluginRegistryPanel";
import { usePluginRegistry } from "./hooks/usePluginRegistry";
import { DeToolsStatusList } from "../plugin-center/DeToolsStatusList";
import { EphemeralToolAuditDrawerPanel } from "../plugin-center/EphemeralToolAuditDrawerPanel";
import { LearnLedgerQueueList } from "../plugin-center/LearnLedgerQueueList";
import { RepoImportForm } from "../plugin-center/RepoImportForm";
import { ExternalActivityTable } from "../mcp-gateway/ExternalActivityTable";
import { HostInstallerList } from "../mcp-gateway/HostInstallerList";
import { McpToolPermissionTable } from "../mcp-gateway/McpToolPermissionTable";
import { useMcpGatewayController } from "../mcp-gateway/useMcpGatewayController";

export interface PluginCenterWorkspaceProps {
  projectId: string;
  hostSlotId: string;
  cardSchemaVersion: number;
  initialScope?: "mcp" | "automations";
  initialLearnFilter?: "ALL" | "PROPOSED" | "ACCEPTED" | "REJECTED";
  headerTabIds?: readonly string[];
}

function updateTabQueryParam(nextScope: "mcp" | "automations", preserveLearnTab: boolean): void {
  if (typeof window === "undefined") return;
  const url = new URL(window.location.href);
  if (url.pathname !== "/plugins" && url.pathname !== "/plugins/") return;
  if (nextScope === "automations") url.searchParams.set("tab", "cron");
  else if (preserveLearnTab) url.searchParams.set("tab", "learn");
  else url.searchParams.delete("tab");
  window.history.replaceState({}, "", `${url.pathname}${url.search}`);
}

const BANNER_ERROR_STYLE: React.CSSProperties = {
  padding: "8px 12px",
  borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--state-danger-soft)",
  border: "1px solid var(--state-danger)",
  color: "var(--state-danger)",
  fontSize: "11.5px",
};

const BANNER_SUCCESS_STYLE: React.CSSProperties = {
  padding: "8px 12px",
  borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--state-success-soft)",
  border: "1px solid var(--state-success)",
  color: "var(--state-success)",
  fontSize: "11.5px",
};

const PluginCenterMcpGatewaySection: React.FC<{
  gateway: ReturnType<typeof useMcpGatewayController>;
}> = ({ gateway }) => (
  <div className="plugin-eco-section" data-testid="plugin-center-mcp-gateway">
    {gateway.error && (
      <div role="alert" data-testid="mcp-gateway-error" style={BANNER_ERROR_STYLE}>
        {gateway.error}
      </div>
    )}
    {gateway.feedback && (
      <div role="status" data-testid="mcp-gateway-feedback" style={BANNER_SUCCESS_STYLE}>
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
    <McpToolPermissionTable
      policies={gateway.policies}
      busyToolName={gateway.busyToolName}
      onToggle={(policy, nextEnabled) => void gateway.toggleTool(policy, nextEnabled)}
    />
    <ExternalActivityTable
      activities={gateway.activities}
      loading={gateway.loading}
      statusFilter={gateway.statusFilter}
      onFilterChange={gateway.setStatusFilter}
      onRefresh={() => void gateway.refresh()}
    />
  </div>
);

const PluginCenterMcpContent: React.FC<{
  projectId: string;
  registry: ReturnType<typeof usePluginRegistry>;
  gateway: ReturnType<typeof useMcpGatewayController>;
  initialLearnFilter: "ALL" | "PROPOSED" | "ACCEPTED" | "REJECTED";
}> = ({ projectId, registry, gateway, initialLearnFilter }) => {
  const pluginCheck = validatePluginCards(registry.plugins);
  const cardCheck = validateLearnCards(registry.learnCards);
  return (
    <>
      <PluginCenterMcpGatewaySection gateway={gateway} />
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
      <DeToolsStatusList projectId={projectId} />
      <RepoImportForm projectId={projectId} />
      <EphemeralToolAuditDrawerPanel projectId={projectId} />
      <LearnLedgerQueueList
        projectId={projectId}
        onCardProposed={() => void registry.refreshAll()}
      />
      <LearnCardReviewQueue
        cards={cardCheck.valid}
        loading={registry.loading}
        error={registry.learnError ?? cardCheck.error}
        pendingActionId={registry.actionPendingId}
        actionFeedback={registry.actionFeedback}
        initialFilter={initialLearnFilter}
        onRefresh={() => void registry.refreshAll()}
        onApprove={(card, reason) => void registry.approveLearnCard(card, reason)}
        onReject={(card, reason) => void registry.rejectLearnCard(card, reason)}
      />
    </>
  );
};

const PluginCenterHeader: React.FC<{
  scope: "mcp" | "automations";
  onSwitchScope: (next: "mcp" | "automations") => void;
}> = ({ scope, onSwitchScope }) => {
  const { t } = useTranslation();
  return (
    <div className="plugin-eco-header">
      <div className="plugin-eco-title-group">
        <a href="/" className="plugin-eco-btn" data-testid="btn-back-to-workbench" style={{ textDecoration: "none" }}>
          {t("plugins.backToWorkbench", "Workbench")}
        </a>
        <h1 className="plugin-eco-title">{t("plugins.registryTitle", "AgentSkills Registry & Staging")}</h1>
      </div>
      <div className="plugin-eco-tabs" role="tablist">
        <button type="button" className={`plugin-eco-tab ${scope === "mcp" ? "plugin-eco-tab-active" : ""}`} onClick={() => onSwitchScope("mcp")}>
          {t("settings.tabMcp", "MCP & Tiện ích")}
        </button>
        <button type="button" className={`plugin-eco-tab ${scope === "automations" ? "plugin-eco-tab-active" : ""}`} onClick={() => onSwitchScope("automations")}>
          {t("settings.tabAutomations", "Tự động hóa")}
        </button>
      </div>
    </div>
  );
};

const PluginCenterAutomationsBody: React.FC<{
  registry: ReturnType<typeof usePluginRegistry>;
}> = ({ registry }) => {
  const cronPlugins = registry.plugins.filter((p) => p.sourceType !== "builtin" || Boolean(p.upstreamRepoUrl));
  const cronScheduleCheck = validateCronSchedules(registry.schedulesByPlugin);
  return (
    <CronSyncStatus
      plugins={cronPlugins}
      schedulesByPlugin={cronScheduleCheck.valid}
      loading={registry.loading}
      error={registry.cronError ?? cronScheduleCheck.error}
      pendingActionId={registry.actionPendingId}
      actionFeedback={registry.actionFeedback}
      onRefresh={() => void registry.refreshAll()}
      onSyncNow={(plugin, refreshAuth) => void registry.triggerSyncNow(plugin, refreshAuth)}
      onSaveSchedule={(plugin, cronExpr, url) => registry.saveSchedule(plugin, cronExpr, url)}
    />
  );
};

export const PluginCenterWorkspace: React.FC<PluginCenterWorkspaceProps> = ({
  projectId,
  hostSlotId,
  cardSchemaVersion,
  initialScope = "mcp",
  initialLearnFilter = "ALL",
  headerTabIds = [],
}) => {
  const [scope, setScope] = useState<"mcp" | "automations">(initialScope);
  const registry = usePluginRegistry(projectId, scope);
  const mcpGateway = useMcpGatewayController(scope === "mcp");
  const switchScope = (next: "mcp" | "automations") => {
    setScope(next);
    updateTabQueryParam(next, initialLearnFilter === "PROPOSED");
  };

  return (
    <main
      className="plugin-eco-section"
      data-host-slot={hostSlotId}
      data-host-tabs={headerTabIds.join(",")}
      data-card-schema={cardSchemaVersion}
      data-project-id={projectId}
      style={{ maxWidth: "1120px", margin: "24px auto" }}
    >
      <PluginCenterHeader scope={scope} onSwitchScope={switchScope} />
      {scope === "mcp" ? (
        <PluginCenterMcpContent
          projectId={projectId}
          registry={registry}
          gateway={mcpGateway}
          initialLearnFilter={initialLearnFilter}
        />
      ) : (
        <PluginCenterAutomationsBody registry={registry} />
      )}
    </main>
  );
};
