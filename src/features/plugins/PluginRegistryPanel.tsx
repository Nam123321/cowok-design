// @story 08.4, 08.5
// @implements Compact Chip-Grid 6-State Plugin Registry with Active/Inactive Swap, Quarantine Popup & Multi-Select Bulk Floating Card
import React, { useMemo, useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { PluginLifecycleTarget, PluginSummaryDto } from "../../lib/validations/plugins";
import { PluginBulkFloatingBar, PluginQuarantineModal } from "./PluginQuarantineModal";
import { StagingDiffCard } from "./StagingDiffCard";
import "./plugins.css";

const PANEL_ICON_SIZE = 14;
const REFRESH_ICON_SIZE = 12;
const CHIP_ICON_SIZE = 11;

export interface PluginRegistryPanelProps {
  plugins: PluginSummaryDto[];
  loading: boolean;
  error: string | null;
  pendingActionId: string | null;
  actionFeedback: string | null;
  onRefresh: () => void;
  onTransition: (plugin: PluginSummaryDto, target: PluginLifecycleTarget) => void;
  onQuarantine?: (plugin: PluginSummaryDto, reason: string) => void;
  onPromoteStaging: (plugin: PluginSummaryDto, reason: string) => void;
  onDiscardStaging: (plugin: PluginSummaryDto, reason: string) => void;
}

function resolveNextLifecycleTarget(st: string): PluginLifecycleTarget | null {
  if (st === "DISCOVERED") return "VALIDATED";
  if (st === "VALIDATED") return "INSTALLED";
  return null;
}

const PluginStateToggleChip: React.FC<{
  plugin: PluginSummaryDto;
  busy: boolean;
  onTransition: (plugin: PluginSummaryDto, target: PluginLifecycleTarget) => void;
}> = ({ plugin, busy, onTransition }) => {
  const { t } = useTranslation();
  const st = plugin.lifecycleState.toUpperCase();
  if (plugin.quarantined) {
    return (
      <span className="plugin-eco-badge plugin-eco-badge-danger" data-testid={`plugin-state-${plugin.pluginSlug}`}>
        {t("plugins.quarantinedLabel", "Đã cách ly")}
      </span>
    );
  }
  if (st === "ACTIVE") {
    return (
      <button type="button" className="plugin-eco-toggle-chip plugin-eco-toggle-chip--active" data-testid={`btn-deprecate-${plugin.pluginSlug}`} title={t("plugins.clickToDeactivateHint", "Nhấn để tạm tắt kỹ năng này")} disabled={busy} onClick={() => onTransition(plugin, "DEPRECATED")}>
        <span className="plugin-eco-status-dot plugin-eco-status-dot--active" aria-hidden="true" />
        <span data-testid={`plugin-state-${plugin.pluginSlug}`}>{t("plugins.stateActiveChip", "Đang bật")}</span>
      </button>
    );
  }
  if (st === "INSTALLED" || st === "DEPRECATED") {
    return (
      <button type="button" className="plugin-eco-toggle-chip" data-testid={`btn-activate-${plugin.pluginSlug}`} title={t("plugins.clickToActivateHint", "Nhấn để kích hoạt kỹ năng này")} disabled={busy} onClick={() => onTransition(plugin, "ACTIVE")}>
        <span className="plugin-eco-status-dot" aria-hidden="true" />
        <span data-testid={`plugin-state-${plugin.pluginSlug}`}>{t("plugins.stateInactiveChip", "Đang tắt")}</span>
      </button>
    );
  }
  const nextTarget = resolveNextLifecycleTarget(st);
  return (
    <span className="plugin-eco-actions">
      <span className="plugin-eco-badge" data-testid={`plugin-state-${plugin.pluginSlug}`}>{st}</span>
      {nextTarget && (
        <button type="button" className="plugin-eco-toggle-chip" disabled={busy} onClick={() => onTransition(plugin, nextTarget)}>
          {nextTarget === "VALIDATED" ? t("plugins.validateBtn", "Xác thực") : t("plugins.installBtn", "Cài đặt")}
        </button>
      )}
    </span>
  );
};

const PluginChipActionGroup: React.FC<{
  plugin: PluginSummaryDto;
  busy: boolean;
  onTransition: (plugin: PluginSummaryDto, target: PluginLifecycleTarget) => void;
  onRequestQuarantine?: (plugin: PluginSummaryDto) => void;
}> = ({ plugin, busy, onTransition, onRequestQuarantine }) => {
  const { t } = useTranslation();
  const st = plugin.lifecycleState.toUpperCase();
  const canQuarantine = Boolean(onRequestQuarantine) && !plugin.quarantined && st !== "ARCHIVED";
  return (
    <div className="plugin-eco-actions">
      <PluginStateToggleChip plugin={plugin} busy={busy} onTransition={onTransition} />
      {st === "DEPRECATED" && (
        <button
          type="button"
          className="plugin-eco-toggle-chip"
          data-testid={`btn-archive-${plugin.pluginSlug}`}
          disabled={busy}
          onClick={() => onTransition(plugin, "ARCHIVED")}
        >
          {t("plugins.archiveBtn", "Lưu trữ")}
        </button>
      )}
      {canQuarantine && (
        <button
          type="button"
          className="plugin-eco-quarantine-chip"
          data-testid={`btn-quarantine-${plugin.pluginSlug}`}
          disabled={busy}
          onClick={() => onRequestQuarantine?.(plugin)}
        >
          <BrandIcon name="security" size={CHIP_ICON_SIZE} color="currentColor" />
          <span>{t("plugins.quarantineBtn", "Cách ly")}</span>
        </button>
      )}
    </div>
  );
};

const PluginChipCard: React.FC<{
  plugin: PluginSummaryDto;
  selected: boolean;
  pendingActionId: string | null;
  onToggleSelect: (id: string) => void;
  onTransition: (plugin: PluginSummaryDto, target: PluginLifecycleTarget) => void;
  onRequestQuarantine?: (plugin: PluginSummaryDto) => void;
  onPromoteStaging: (plugin: PluginSummaryDto, reason: string) => void;
  onDiscardStaging: (plugin: PluginSummaryDto, reason: string) => void;
}> = (props) => {
  const { plugin, selected, pendingActionId, onToggleSelect, onTransition, onRequestQuarantine } = props;
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const busy =
    pendingActionId === plugin.id ||
    pendingActionId === `quarantine:${plugin.id}` ||
    pendingActionId === `promote:${plugin.id}` ||
    pendingActionId === `discard:${plugin.id}`;

  return (
    <div className={selected ? "plugin-eco-chip-card plugin-eco-chip-card--selected" : "plugin-eco-chip-card"} data-testid={`plugin-card-${plugin.pluginSlug}`}>
      <div className="plugin-eco-card-row">
        <label className="plugin-eco-check-label">
          <input type="checkbox" checked={selected} data-testid={`checkbox-plugin-${plugin.pluginSlug}`} onChange={() => onToggleSelect(plugin.id)} />
          <span className="plugin-eco-slug">{plugin.pluginSlug}</span>
          <span className="plugin-eco-meta">v{plugin.version}</span>
        </label>
        <PluginChipActionGroup plugin={plugin} busy={busy} onTransition={onTransition} onRequestQuarantine={onRequestQuarantine} />
      </div>
      <div className="plugin-eco-meta">
        {plugin.sourceType} • {t("plugins.revLabel", "Phiên bản")} {formatNumber(plugin.revision)}
      </div>
      {plugin.quarantined && (
        <div className="plugin-eco-error-box" role="status" data-testid={`plugin-quarantine-reason-${plugin.pluginSlug}`}>
          <span>
            {t("plugins.quarantineReasonLabel", "Lý do cách ly")}:{" "}
            {plugin.quarantineReason || t("plugins.quarantinedLabel", "Đã cách ly")}
          </span>
        </div>
      )}
      <StagingDiffCard plugin={plugin} pendingActionId={pendingActionId} onPromote={props.onPromoteStaging} onDiscard={props.onDiscardStaging} />
    </div>
  );
};

const PluginRegistryHeader: React.FC<{
  totalCount: number;
  allSelected: boolean;
  loading: boolean;
  onToggleSelectAll: () => void;
  onRefresh: () => void;
}> = ({ totalCount, allSelected, loading, onToggleSelectAll, onRefresh }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const refreshLabel = t("plugins.refreshBtn", "Làm mới");
  return (
    <div className="plugin-eco-header">
      <div className="plugin-eco-title-group">
        <BrandIcon name="plugin" size={PANEL_ICON_SIZE} color="var(--text-primary)" />
        <h3 className="plugin-eco-title">{t("plugins.registryTitle", "Kho AgentSkills & Hàng chờ Staging")}</h3>
        <span className="plugin-eco-badge">{formatNumber(totalCount)}</span>
      </div>
      <div className="plugin-eco-actions">
        {totalCount > 0 && (
          <button type="button" className="plugin-eco-btn" data-testid="btn-select-all-plugins" onClick={onToggleSelectAll}>
            {allSelected ? t("plugins.clearSelectionBtn", "Bỏ chọn") : t("plugins.selectAllBtn", "Chọn tất cả")}
          </button>
        )}
        <button type="button" className="plugin-eco-btn" data-testid="btn-refresh-plugins" aria-label={refreshLabel} title={refreshLabel} disabled={loading} onClick={onRefresh}>
          <BrandIcon name="refresh" size={REFRESH_ICON_SIZE} color="currentColor" />
        </button>
      </div>
    </div>
  );
};

const PluginRegistryListBody: React.FC<{
  plugins: PluginSummaryDto[];
  loading: boolean;
  error: string | null;
  selectedIds: string[];
  pendingActionId: string | null;
  onToggleSelect: (id: string) => void;
  onTransition: (plugin: PluginSummaryDto, target: PluginLifecycleTarget) => void;
  onRequestQuarantine?: (plugin: PluginSummaryDto) => void;
  onPromoteStaging: (plugin: PluginSummaryDto, reason: string) => void;
  onDiscardStaging: (plugin: PluginSummaryDto, reason: string) => void;
}> = (p) => {
  const { t } = useTranslation();
  if (p.loading && p.plugins.length === 0) {
    return <div className="plugin-eco-state-box" data-testid="plugin-registry-loading">{t("plugins.loadingState", "Đang tải danh sách kỹ năng...")}</div>;
  }
  if (!p.loading && p.plugins.length === 0 && !p.error) {
    return <div className="plugin-eco-state-box" data-testid="plugin-registry-empty">{t("plugins.emptyState", "Chưa có kỹ năng nào trong dự án này.")}</div>;
  }
  return (
    <div className="plugin-eco-chip-grid">
      {p.plugins.map((plugin) => (
        <PluginChipCard
          key={plugin.id}
          plugin={plugin}
          selected={p.selectedIds.includes(plugin.id)}
          pendingActionId={p.pendingActionId}
          onToggleSelect={p.onToggleSelect}
          onTransition={p.onTransition}
          onRequestQuarantine={p.onRequestQuarantine}
          onPromoteStaging={p.onPromoteStaging}
          onDiscardStaging={p.onDiscardStaging}
        />
      ))}
    </div>
  );
};

export const PluginRegistryPanel: React.FC<PluginRegistryPanelProps> = (props) => {
  const { t } = useTranslation();
  const { plugins, loading, error, pendingActionId, actionFeedback, onRefresh, onTransition, onQuarantine } = props;
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [quarantineTargets, setQuarantineTargets] = useState<PluginSummaryDto[]>([]);

  const selectedPlugins = useMemo(() => plugins.filter((p) => selectedIds.includes(p.id)), [plugins, selectedIds]);
  const allSelected = plugins.length > 0 && selectedPlugins.length === plugins.length;
  const toggleSelect = (id: string) =>
    setSelectedIds((prev) => (prev.includes(id) ? prev.filter((item) => item !== id) : [...prev, id]));
  const runBulkTransition = (target: PluginLifecycleTarget, predicate: (p: PluginSummaryDto) => boolean) => {
    selectedPlugins.filter(predicate).forEach((plugin) => onTransition(plugin, target));
    setSelectedIds([]);
  };

  return (
    <section className="plugin-eco-section" data-ff-id="skill-plugin-manager" data-testid="plugin-registry-panel">
      <PluginRegistryHeader totalCount={plugins.length} allSelected={allSelected} loading={loading} onToggleSelectAll={() => setSelectedIds(allSelected ? [] : plugins.map((p) => p.id))} onRefresh={onRefresh} />
      {actionFeedback && <div className="plugin-eco-badge plugin-eco-badge-active" data-testid="plugin-action-feedback">{actionFeedback}</div>}
      {error && (
        <div className="plugin-eco-error-box" role="alert" data-testid="plugin-registry-error">
          <span>{error}</span>
          <button type="button" className="plugin-eco-btn" onClick={onRefresh}>{t("plugins.retryBtn", "Thử lại")}</button>
        </div>
      )}
      <PluginRegistryListBody
        plugins={plugins} loading={loading} error={error} selectedIds={selectedIds} pendingActionId={pendingActionId}
        onToggleSelect={toggleSelect} onTransition={onTransition}
        onRequestQuarantine={onQuarantine ? (p) => setQuarantineTargets([p]) : undefined}
        onPromoteStaging={props.onPromoteStaging} onDiscardStaging={props.onDiscardStaging}
      />
      <PluginBulkFloatingBar
        selectedPlugins={selectedPlugins} busy={Boolean(pendingActionId)}
        onBulkActivate={() => runBulkTransition("ACTIVE", (p) => !p.quarantined && ["INSTALLED", "DEPRECATED"].includes(p.lifecycleState.toUpperCase()))}
        onBulkDeactivate={() => runBulkTransition("DEPRECATED", (p) => p.lifecycleState.toUpperCase() === "ACTIVE")}
        onBulkQuarantine={() => setQuarantineTargets(selectedPlugins.filter((p) => !p.quarantined && p.lifecycleState.toUpperCase() !== "ARCHIVED"))}
        onClearSelection={() => setSelectedIds([])}
      />
      <PluginQuarantineModal
        targets={quarantineTargets} busy={Boolean(pendingActionId)}
        onConfirm={(targets, reason) => { targets.forEach((p) => onQuarantine?.(p, reason)); setQuarantineTargets([]); setSelectedIds([]); }}
        onClose={() => setQuarantineTargets([])}
      />
    </section>
  );
};
