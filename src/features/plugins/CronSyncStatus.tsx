// @story 08.4
// @implements AC-04 (Cron Sync Schedule Monitor, Lease/Retry Backoff, & AUTH_REQUIRED Recovery)
import React, { useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type { CronScheduleStatusDto, PluginSummaryDto } from "../../lib/validations/plugins";
import "./plugins.css";

const CRON_ICON_SIZE = 14;
const DEFAULT_CRON_EXPR = "0 */5 * * * *";

export interface CronSyncStatusProps {
  plugins: PluginSummaryDto[];
  schedulesByPlugin: Record<string, CronScheduleStatusDto | null>;
  loading: boolean;
  error: string | null;
  pendingActionId: string | null;
  onRefresh: () => void;
  onSyncNow: (plugin: PluginSummaryDto, refreshAuth?: boolean) => void;
  onSaveSchedule?: (plugin: PluginSummaryDto, cronExpression: string, upstreamRepoUrl: string) => void;
}

function cronBadgeClass(syncStatus: string): string {
  const upper = syncStatus.toUpperCase();
  if (upper === "SUCCEEDED" || upper === "IDLE") return "plugin-eco-badge plugin-eco-badge-active";
  if (upper === "AUTH_REQUIRED" || upper === "FAILED") return "plugin-eco-badge plugin-eco-badge-danger";
  return "plugin-eco-badge plugin-eco-badge-warning";
}

const CronScheduleEditor: React.FC<{
  plugin: PluginSummaryDto;
  initialCron: string;
  initialUrl: string;
  busy: boolean;
  onSave: (plugin: PluginSummaryDto, cronExpression: string, upstreamRepoUrl: string) => void;
}> = ({ plugin, initialCron, initialUrl, busy, onSave }) => {
  const { t } = useTranslation();
  const [cronExpr, setCronExpr] = useState(initialCron || DEFAULT_CRON_EXPR);
  const [repoUrl, setRepoUrl] = useState(initialUrl || plugin.sourceRepoUrl);

  return (
    <form
      className="plugin-eco-schedule-form"
      data-testid={`cron-schedule-form-${plugin.pluginSlug}`}
      onSubmit={(e) => {
        e.preventDefault();
        if (cronExpr.trim() && repoUrl.trim()) onSave(plugin, cronExpr, repoUrl);
      }}
    >
      <input
        type="text"
        className="plugin-eco-input"
        data-testid={`input-cron-expr-${plugin.pluginSlug}`}
        value={cronExpr}
        onChange={(e) => setCronExpr(e.target.value)}
        placeholder={t("cron.cronPlaceholder", "0 */5 * * * *")}
      />
      <input
        type="url"
        className="plugin-eco-input"
        style={{ flex: 1 }}
        data-testid={`input-cron-url-${plugin.pluginSlug}`}
        value={repoUrl}
        onChange={(e) => setRepoUrl(e.target.value)}
        placeholder={t("cron.upstreamUrlPlaceholder", "https://github.com/org/repo")}
      />
      <button type="submit" className="plugin-eco-btn" disabled={busy || !cronExpr.trim() || !repoUrl.trim()}>
        {t("cron.saveScheduleBtn", "Save Schedule")}
      </button>
    </form>
  );
};

const CronAuthBanner: React.FC<{
  plugin: PluginSummaryDto;
  lastErrorMessage?: string | null;
  busy: boolean;
  onRefreshSync: () => void;
}> = ({ plugin, lastErrorMessage, busy, onRefreshSync }) => {
  const { t } = useTranslation();
  return (
    <div className="plugin-eco-error-box" role="alert" data-testid={`cron-auth-banner-${plugin.pluginSlug}`}>
      <span>
        {t("cron.authRequiredNotice", "Upstream 401/403")}: {lastErrorMessage || "Vault github token required"}
      </span>
      <button
        type="button"
        className="plugin-eco-btn plugin-eco-btn-primary"
        data-testid={`btn-refresh-auth-sync-${plugin.pluginSlug}`}
        disabled={busy}
        onClick={onRefreshSync}
      >
        {t("cron.refreshAuthSync", "Refresh Auth & Sync")}
      </button>
    </div>
  );
};

const CronScheduleCard: React.FC<{
  plugin: PluginSummaryDto;
  schedule: CronScheduleStatusDto;
  busy: boolean;
  onSyncNow: (plugin: PluginSummaryDto, refreshAuth?: boolean) => void;
  onSaveSchedule?: (plugin: PluginSummaryDto, cronExpression: string, upstreamRepoUrl: string) => void;
}> = ({ plugin, schedule, busy, onSyncNow, onSaveSchedule }) => {
  const { t } = useTranslation();
  const isAuthRequired = schedule.syncStatus === "AUTH_REQUIRED";
  return (
    <div className="plugin-eco-card" data-testid={`cron-card-${plugin.pluginSlug}`}>
      <div className="plugin-eco-card-row">
        <div>
          <div className="plugin-eco-slug">
            {plugin.pluginSlug} ({schedule.cronExpression})
          </div>
          <div className="plugin-eco-meta">
            {schedule.upstreamRepoUrl} • {t("cron.failuresLabel", "Failures")}: {schedule.consecutiveFailures}
          </div>
        </div>
        <span className={cronBadgeClass(schedule.syncStatus)} data-testid={`cron-status-${plugin.pluginSlug}`}>
          {schedule.syncStatus}
        </span>
      </div>
      {isAuthRequired && (
        <CronAuthBanner
          plugin={plugin}
          lastErrorMessage={schedule.lastErrorMessage}
          busy={busy}
          onRefreshSync={() => onSyncNow(plugin, true)}
        />
      )}
      {onSaveSchedule && (
        <CronScheduleEditor
          plugin={plugin}
          initialCron={schedule.cronExpression}
          initialUrl={schedule.upstreamRepoUrl}
          busy={busy}
          onSave={onSaveSchedule}
        />
      )}
      <div className="plugin-eco-actions">
        <button
          type="button"
          className="plugin-eco-btn"
          data-testid={`btn-sync-now-${plugin.pluginSlug}`}
          disabled={busy}
          onClick={() => onSyncNow(plugin, false)}
        >
          {t("cron.syncNowBtn", "Sync Now")}
        </button>
      </div>
    </div>
  );
};

const CronSyncBody: React.FC<{
  entries: Array<{ plugin: PluginSummaryDto; schedule: CronScheduleStatusDto }>;
  loading: boolean;
  error: string | null;
  busy: boolean;
  onSyncNow: (plugin: PluginSummaryDto, refreshAuth?: boolean) => void;
  onSaveSchedule?: (plugin: PluginSummaryDto, cronExpression: string, upstreamRepoUrl: string) => void;
}> = ({ entries, loading, error, busy, onSyncNow, onSaveSchedule }) => {
  const { t } = useTranslation();
  if (loading && entries.length === 0) {
    return (
      <div className="plugin-eco-state-box" data-testid="cron-sync-loading">
        {t("cron.loadingState", "Checking cron sync schedules...")}
      </div>
    );
  }
  if (!loading && entries.length === 0 && !error) {
    return (
      <div className="plugin-eco-state-box" data-testid="cron-sync-empty">
        {t("cron.emptyState", "No upstream cron sync schedules active.")}
      </div>
    );
  }
  return (
    <div className="plugin-eco-list">
      {entries.map(({ plugin, schedule }) => (
        <CronScheduleCard
          key={schedule.id}
          plugin={plugin}
          schedule={schedule}
          busy={busy}
          onSyncNow={onSyncNow}
          onSaveSchedule={onSaveSchedule}
        />
      ))}
    </div>
  );
};

export const CronSyncStatus: React.FC<CronSyncStatusProps> = ({
  plugins,
  schedulesByPlugin,
  loading,
  error,
  pendingActionId,
  onRefresh,
  onSyncNow,
  onSaveSchedule,
}) => {
  const { t } = useTranslation();
  const entries = plugins
    .map((plugin) => ({ plugin, schedule: schedulesByPlugin[plugin.id] }))
    .filter((entry): entry is { plugin: PluginSummaryDto; schedule: CronScheduleStatusDto } => Boolean(entry.schedule));

  return (
    <section className="plugin-eco-section" data-ff-id="cron-sync-status" data-testid="cron-sync-status">
      <div className="plugin-eco-header">
        <div className="plugin-eco-title-group">
          <BrandIcon name="automation" size={CRON_ICON_SIZE} color="var(--text-primary)" />
          <h3 className="plugin-eco-title">{t("cron.statusTitle", "Plugin Upstream Cron Sync")}</h3>
        </div>
        <button type="button" className="plugin-eco-btn" disabled={loading} onClick={onRefresh}>
          {t("cron.refreshBtn", "Refresh")}
        </button>
      </div>
      {error && (
        <div className="plugin-eco-error-box" role="alert" data-testid="cron-sync-error">
          <span>{error}</span>
          <button type="button" className="plugin-eco-btn" onClick={onRefresh}>
            {t("cron.retryBtn", "Retry")}
          </button>
        </div>
      )}
      <CronSyncBody
        entries={entries}
        loading={loading}
        error={error}
        busy={pendingActionId !== null}
        onSyncNow={onSyncNow}
        onSaveSchedule={onSaveSchedule}
      />
    </section>
  );
};
