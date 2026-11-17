// @story 08.4
// @implements AC-01, AC-04, AC-05 (6-State Plugin Registry & Lifecycle Controls)
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type { PluginSummaryDto } from "../../lib/validations/plugins";
import { StagingDiffCard } from "./StagingDiffCard";
import "./plugins.css";

const PANEL_ICON_SIZE = 14;
const REFRESH_ICON_SIZE = 12;

type PluginTargetState = "VALIDATED" | "INSTALLED" | "ACTIVE" | "DEPRECATED" | "ARCHIVED";

export interface PluginRegistryPanelProps {
  plugins: PluginSummaryDto[];
  loading: boolean;
  error: string | null;
  pendingActionId: string | null;
  actionFeedback: string | null;
  onRefresh: () => void;
  onTransition: (plugin: PluginSummaryDto, target: PluginTargetState) => void;
  onPromoteStaging: (plugin: PluginSummaryDto, reason: string) => void;
  onDiscardStaging: (plugin: PluginSummaryDto, reason: string) => void;
}

function badgeClassForState(state: string): string {
  const upper = state.toUpperCase();
  if (upper === "ACTIVE" || upper === "ENABLED") return "plugin-eco-badge plugin-eco-badge-active";
  if (upper === "DEPRECATED" || upper === "DISCOVERED") return "plugin-eco-badge plugin-eco-badge-warning";
  if (upper === "ARCHIVED") return "plugin-eco-badge plugin-eco-badge-danger";
  return "plugin-eco-badge";
}

const PluginLifecycleButtons: React.FC<{
  plugin: PluginSummaryDto;
  busy: boolean;
  onTransition: (plugin: PluginSummaryDto, target: PluginTargetState) => void;
}> = ({ plugin, busy, onTransition }) => {
  const { t } = useTranslation();
  const st = plugin.lifecycleState.toUpperCase();
  return (
    <div className="plugin-eco-actions">
      {st === "DISCOVERED" && (
        <button type="button" className="plugin-eco-btn" disabled={busy} onClick={() => onTransition(plugin, "VALIDATED")}>
          {t("plugins.validateBtn", "Validate")}
        </button>
      )}
      {st === "VALIDATED" && (
        <button type="button" className="plugin-eco-btn" disabled={busy} onClick={() => onTransition(plugin, "INSTALLED")}>
          {t("plugins.installBtn", "Install")}
        </button>
      )}
      {(st === "INSTALLED" || st === "DEPRECATED") && (
        <button type="button" className="plugin-eco-btn plugin-eco-btn-primary" data-testid={`btn-activate-${plugin.pluginSlug}`} disabled={busy} onClick={() => onTransition(plugin, "ACTIVE")}>
          {t("plugins.activateBtn", "Activate (HUMAN_ONLY)")}
        </button>
      )}
      {(st === "ACTIVE" || st === "ENABLED") && (
        <button type="button" className="plugin-eco-btn" data-testid={`btn-deprecate-${plugin.pluginSlug}`} disabled={busy} onClick={() => onTransition(plugin, "DEPRECATED")}>
          {t("plugins.deprecateBtn", "Deprecate")}
        </button>
      )}
    </div>
  );
};

const PluginRegistryBody: React.FC<
  Pick<PluginRegistryPanelProps, "plugins" | "loading" | "error" | "pendingActionId" | "onTransition" | "onPromoteStaging" | "onDiscardStaging">
> = ({ plugins, loading, error, pendingActionId, onTransition, onPromoteStaging, onDiscardStaging }) => {
  const { t } = useTranslation();
  if (loading && plugins.length === 0) {
    return (
      <div className="plugin-eco-state-box" data-testid="plugin-registry-loading">
        {t("plugins.loadingState", "Loading SkillPlugins from SQLite WAL daemon...")}
      </div>
    );
  }
  if (!loading && plugins.length === 0 && !error) {
    return (
      <div className="plugin-eco-state-box" data-testid="plugin-registry-empty">
        {t("plugins.emptyState", "No SkillPlugins registered in this project scope yet.")}
      </div>
    );
  }
  const busy = pendingActionId !== null;
  return (
    <div className="plugin-eco-list">
      {plugins.map((plugin) => (
        <div key={plugin.id} className="plugin-eco-card" data-testid={`plugin-card-${plugin.pluginSlug}`}>
          <div className="plugin-eco-card-row">
            <div>
              <div className="plugin-eco-slug">
                {plugin.pluginSlug} <span className="plugin-eco-meta">v{plugin.version}</span>
              </div>
              <div className="plugin-eco-meta">
                {plugin.sourceType} • {t("plugins.revLabel", "Revision")} {plugin.revision}
              </div>
            </div>
            <div className="plugin-eco-actions">
              {plugin.quarantined && (
                <span className="plugin-eco-badge plugin-eco-badge-danger" title={plugin.quarantineReason || ""}>
                  {t("plugins.quarantinedLabel", "QUARANTINED")}
                </span>
              )}
              <span className={badgeClassForState(plugin.lifecycleState)} data-testid={`plugin-state-${plugin.pluginSlug}`}>
                {plugin.lifecycleState}
              </span>
            </div>
          </div>
          <StagingDiffCard plugin={plugin} pendingActionId={pendingActionId} onPromote={onPromoteStaging} onDiscard={onDiscardStaging} />
          <PluginLifecycleButtons plugin={plugin} busy={busy} onTransition={onTransition} />
        </div>
      ))}
    </div>
  );
};

export const PluginRegistryPanel: React.FC<PluginRegistryPanelProps> = (props) => {
  const { t } = useTranslation();
  const { plugins, loading, error, pendingActionId, actionFeedback, onRefresh, onTransition, onPromoteStaging, onDiscardStaging } = props;
  return (
    <section className="plugin-eco-section" data-ff-id="skill-plugin-manager" data-testid="plugin-registry-panel">
      <div className="plugin-eco-header">
        <div className="plugin-eco-title-group">
          <BrandIcon name="plugin" size={PANEL_ICON_SIZE} color="var(--text-primary)" />
          <h3 className="plugin-eco-title">{t("plugins.registryTitle", "AgentSkills Registry & Staging")}</h3>
        </div>
        <button type="button" className="plugin-eco-btn" data-testid="btn-refresh-plugins" disabled={loading} onClick={onRefresh}>
          <BrandIcon name="refresh" size={REFRESH_ICON_SIZE} color="currentColor" />
        </button>
      </div>
      {actionFeedback && (
        <div className="plugin-eco-badge plugin-eco-badge-active" data-testid="plugin-action-feedback">
          {actionFeedback}
        </div>
      )}
      {error && (
        <div className="plugin-eco-error-box" role="alert" data-testid="plugin-registry-error">
          <span>{error}</span>
          <button type="button" className="plugin-eco-btn" onClick={onRefresh}>
            {t("plugins.retryBtn", "Retry")}
          </button>
        </div>
      )}
      <PluginRegistryBody
        plugins={plugins}
        loading={loading}
        error={error}
        pendingActionId={pendingActionId}
        onTransition={onTransition}
        onPromoteStaging={onPromoteStaging}
        onDiscardStaging={onDiscardStaging}
      />
    </section>
  );
};
