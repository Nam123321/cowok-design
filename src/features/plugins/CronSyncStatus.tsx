// @story 08.4
// @implements AC-04 (Cron Sync Schedule Monitor, Lease/Retry Backoff, & AUTH_REQUIRED Recovery)
import React, { useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type { PluginSummaryDto } from "../../lib/validations/plugins";
import {
  MAX_CRON_EXPR_LEN,
  MIN_CRON_EXPR_LEN,
  type CronScheduleStatusDto,
} from "../../lib/validations/plugins-learn-cron";
import "./plugins.css";

const CRON_ICON_SIZE = 14;
const DEFAULT_CRON_EXPR = "0 */5 * * * *";
const MAX_URL_LEN = 1024;

export interface CronSyncStatusProps {
  plugins: PluginSummaryDto[];
  schedulesByPlugin: Record<string, CronScheduleStatusDto | null>;
  loading: boolean;
  error: string | null;
  pendingActionId: string | null;
  actionFeedback?: string | null;
  onRefresh: () => void;
  onSyncNow: (plugin: PluginSummaryDto, refreshAuth?: boolean) => void;
  onSaveSchedule?: (plugin: PluginSummaryDto, cronExpression: string, upstreamRepoUrl: string) => boolean | Promise<boolean> | void | Promise<void>;
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
  onSave: (plugin: PluginSummaryDto, cronExpression: string, upstreamRepoUrl: string) => boolean | Promise<boolean> | void | Promise<void>;
}> = ({ plugin, initialCron, initialUrl, busy, onSave }) => {
  const { t } = useTranslation();
  const [draftCron, setDraftCron] = useState<string | null>(null);
  const [draftUrl, setDraftUrl] = useState<string | null>(null);
  const cronExpr = draftCron ?? (initialCron || DEFAULT_CRON_EXPR);
  const repoUrl = draftUrl ?? (initialUrl || plugin.upstreamRepoUrl || plugin.sourceRepoUrl);

  return (
    <form
      className="plugin-eco-schedule-form"
      data-testid={`cron-schedule-form-${plugin.pluginSlug}`}
      onSubmit={(e) => {
        e.preventDefault();
        if (!cronExpr.trim() || !repoUrl.trim()) return;
        void Promise.resolve(onSave(plugin, cronExpr, repoUrl)).then((ok) => {
          if (ok !== false) {
            setDraftCron(null);
            setDraftUrl(null);
          }
        });
      }}
    >
      <input
        type="text"
        className="plugin-eco-input"
        data-testid={`input-cron-expr-${plugin.pluginSlug}`}
        minLength={MIN_CRON_EXPR_LEN}
        maxLength={MAX_CRON_EXPR_LEN}
        value={cronExpr}
        onChange={(e) => setDraftCron(e.target.value)}
        placeholder={t("cron.cronPlaceholder", "0 */5 * * * *")}
      />
      <input
        type="url"
        className="plugin-eco-input"
        style={{ flex: 1 }}
        data-testid={`input-cron-url-${plugin.pluginSlug}`}
        maxLength={MAX_URL_LEN}
        value={repoUrl}
        onChange={(e) => setDraftUrl(e.target.value)}
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
        {t("cron.authRequiredNotice", "Upstream 401/403")}:{" "}
        {lastErrorMessage || t("cron.vaultTokenRequired", "Vault github token required")}
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

const CronCardHeader: React.FC<{
  plugin: PluginSummaryDto;
  schedule: CronScheduleStatusDto | null;
  statusLabel: string;
}> = ({ plugin, schedule, statusLabel }) => {
  const { t } = useTranslation();
  return (
    <div className="plugin-eco-card-row">
      <div>
        <div className="plugin-eco-slug">
          {plugin.pluginSlug} ({schedule ? schedule.cronExpression : DEFAULT_CRON_EXPR})
        </div>
        <div className="plugin-eco-meta">
          {schedule ? schedule.upstreamRepoUrl : plugin.upstreamRepoUrl || plugin.sourceRepoUrl} • {t("cron.failuresLabel", "Failures")}:{" "}
          {schedule ? schedule.consecutiveFailures : 0}
        </div>
      </div>
      <span className={cronBadgeClass(statusLabel)} data-testid={`cron-status-${plugin.pluginSlug}`}>
        {statusLabel}
      </span>
    </div>
  );
};

const CronScheduleCard: React.FC<{
  plugin: PluginSummaryDto;
  schedule: CronScheduleStatusDto | null;
  busy: boolean;
  onSyncNow: (plugin: PluginSummaryDto, refreshAuth?: boolean) => void;
  onSaveSchedule?: (plugin: PluginSummaryDto, cronExpression: string, upstreamRepoUrl: string) => boolean | Promise<boolean> | void | Promise<void>;
}> = ({ plugin, schedule, busy, onSyncNow, onSaveSchedule }) => {
  const { t } = useTranslation();
  const statusLabel = schedule ? schedule.syncStatus : "UNSCHEDULED";
  return (
    <div className="plugin-eco-card" data-testid={`cron-card-${plugin.pluginSlug}`}>
      <CronCardHeader plugin={plugin} schedule={schedule} statusLabel={statusLabel} />
      {schedule?.syncStatus === "AUTH_REQUIRED" && (
        <CronAuthBanner
          plugin={plugin}
          lastErrorMessage={schedule.lastErrorMessage}
          busy={busy}
          onRefreshSync={() => onSyncNow(plugin, true)}
        />
      )}
      {onSaveSchedule && (
        <CronScheduleEditor
          key={plugin.id}
          plugin={plugin}
          initialCron={schedule?.cronExpression || DEFAULT_CRON_EXPR}
          initialUrl={schedule?.upstreamRepoUrl || plugin.upstreamRepoUrl || plugin.sourceRepoUrl}
          busy={busy}
          onSave={onSaveSchedule}
        />
      )}
      {schedule && (
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
      )}
    </div>
  );
};

const CronSyncBody: React.FC<{
  entries: Array<{ plugin: PluginSummaryDto; schedule: CronScheduleStatusDto | null }>;
  loading: boolean;
  error: string | null;
  pendingActionId: string | null;
  onSyncNow: (plugin: PluginSummaryDto, refreshAuth?: boolean) => void;
  onSaveSchedule?: (plugin: PluginSummaryDto, cronExpression: string, upstreamRepoUrl: string) => boolean | Promise<boolean> | void | Promise<void>;
}> = ({ entries, loading, error, pendingActionId, onSyncNow, onSaveSchedule }) => {
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
      {entries.map(({ plugin, schedule }) => {
        const pluginBusy =
          pendingActionId === `sync:${plugin.id}` || pendingActionId === `schedule:${plugin.id}`;
        return (
          <CronScheduleCard
            key={schedule ? schedule.id : `unscheduled-${plugin.id}`}
            plugin={plugin}
            schedule={schedule}
            busy={pluginBusy}
            onSyncNow={onSyncNow}
            onSaveSchedule={onSaveSchedule}
          />
        );
      })}
    </div>
  );
};

const CronSyncHeader: React.FC<{ loading: boolean; onRefresh: () => void }> = ({ loading, onRefresh }) => {
  const { t } = useTranslation();
  return (
    <div className="plugin-eco-header">
      <div className="plugin-eco-title-group">
        <BrandIcon name="automation" size={CRON_ICON_SIZE} color="var(--text-primary)" />
        <h3 className="plugin-eco-title">{t("cron.statusTitle", "Plugin Upstream Cron Sync")}</h3>
      </div>
      <button type="button" className="plugin-eco-btn" disabled={loading} onClick={onRefresh}>
        {t("cron.refreshBtn", "Refresh")}
      </button>
    </div>
  );
};

export const CronSyncStatus: React.FC<CronSyncStatusProps> = ({
  plugins,
  schedulesByPlugin,
  loading,
  error,
  pendingActionId,
  actionFeedback,
  onRefresh,
  onSyncNow,
  onSaveSchedule,
}) => {
  const { t } = useTranslation();
  const cronPlugins = plugins.filter((plugin) => plugin.sourceType !== "builtin" || Boolean(plugin.upstreamRepoUrl));
  const entries = cronPlugins.map((plugin) => ({ plugin, schedule: schedulesByPlugin[plugin.id] ?? null }));

  return (
    <section className="plugin-eco-section" data-ff-id="cron-sync-status" data-testid="cron-sync-status">
      <CronSyncHeader loading={loading} onRefresh={onRefresh} />
      {actionFeedback && (
        <div className="plugin-eco-badge plugin-eco-badge-active" data-testid="cron-action-feedback">
          {actionFeedback}
        </div>
      )}
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
        pendingActionId={pendingActionId}
        onSyncNow={onSyncNow}
        onSaveSchedule={onSaveSchedule}
      />
    </section>
  );
};
