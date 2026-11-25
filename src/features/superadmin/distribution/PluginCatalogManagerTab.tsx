// @story 14.3
// @implements AC-02, AC-05 (DESIGN.md [5.21] DataTableView + [5.22] AdvancedFilterBar + [5.25] RightDrawer + Bulk Action Floating Card for Plugin Catalog Lifecycle)
import React, { useEffect, useMemo, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CloudAdminRole } from "../../../lib/validations/cloud-admin";
import type {
  BulkUpdateCatalogRequest,
  CatalogPlugin,
  PluginLifecycleStatus,
  PluginSourceKind,
  SignedCatalogManifest,
  UpsertCatalogPluginRequest,
} from "../../../lib/validations/distribution-ops";
import { PluginCatalogBulkBar } from "./PluginCatalogBulkBar";
import {
  type CatalogDrawerMode,
  formatPluginSourceLabel,
  formatPluginStatusLabel,
  PLUGIN_STATUS_META,
  PluginCatalogDetailDrawer,
  resolveLocalizedTitle,
} from "./PluginCatalogDetailDrawer";
import { resolveBulkCatalogHandler } from "./useDistributionOps";

const STATUS_SORT_ORDER: readonly PluginLifecycleStatus[] = [
  "ACTIVE",
  "PROCESSING",
  "PENDING_APPROVAL",
  "INACTIVE",
  "DEPRECATED",
  "QUARANTINED",
];

export interface PluginCatalogManagerTabProps {
  plugins: CatalogPlugin[];
  signedManifest: SignedCatalogManifest | null;
  operatorRole?: CloudAdminRole;
  submitting: boolean;
  onUpsertPlugin: (input: UpsertCatalogPluginRequest) => Promise<boolean>;
  onBulkUpdateCatalog?: (input: BulkUpdateCatalogRequest) => Promise<boolean>;
}

type SourceFilterValue = "ALL" | PluginSourceKind;
type StatusFilterValue = "ALL" | PluginLifecycleStatus;
type SortColumn = "plugin" | "source" | "status" | "updated";
type SortDirection = "asc" | "desc";

const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const ICON_SIZE_SM = 14;
const ICON_SIZE_MD = 15;
const SIG_PREVIEW_LEN = 22;
const TABLE_COL_SPAN = 6;

const CARD_STYLE: React.CSSProperties = {
  backgroundColor: "var(--bg-card, var(--bg-surface))",
  border: "1px solid var(--border-default)",
  borderRadius: "10px",
  padding: "16px",
  display: "flex",
  flexDirection: "column",
  gap: "12px",
};
const FILTER_BAR_STYLE: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "10px",
  padding: "12px 14px",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  borderRadius: "10px",
};
const SEARCH_INPUT_STYLE: React.CSSProperties = {
  width: "100%",
  height: "36px",
  padding: "0 12px 0 34px",
  borderRadius: "8px",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  fontSize: "13px",
  boxSizing: "border-box",
};
const TH_STYLE: React.CSSProperties = {
  textAlign: "left",
  padding: "10px 12px",
  fontSize: "12px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  color: "var(--text-muted)",
  borderBottom: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-subtle)",
  whiteSpace: "nowrap",
};
const TD_STYLE: React.CSSProperties = {
  padding: "10px 12px",
  fontSize: "13px",
  color: "var(--text-primary)",
  borderBottom: "1px solid var(--border-subtle)",
  verticalAlign: "middle",
};
const SR_ONLY_STYLE: React.CSSProperties = {
  position: "absolute",
  width: "1px",
  height: "1px",
  padding: 0,
  margin: "-1px",
  overflow: "hidden",
  clip: "rect(0, 0, 0, 0)",
  whiteSpace: "nowrap",
  border: 0,
};

const SOURCE_CHIPS: ReadonlyArray<{ value: SourceFilterValue; key: string; fallback: string }> = [
  { value: "ALL", key: "superadmin.ops.filterAll", fallback: "Tất cả" },
  { value: "DEFAULT_BUILTIN", key: "superadmin.ops.sourceDefaultBuiltin", fallback: "Tích hợp Mặc định" },
  { value: "RECOMMENDED_GITHUB", key: "superadmin.ops.sourceRecommendedGithub", fallback: "GitHub Đề xuất" },
];

const STATUS_CHIPS: ReadonlyArray<{ value: StatusFilterValue; key: string; fallback: string }> = [
  { value: "ALL", key: "superadmin.ops.filterAll", fallback: "Tất cả" },
  { value: "ACTIVE", key: "superadmin.ops.statusActive", fallback: "Đang hoạt động" },
  { value: "PROCESSING", key: "superadmin.ops.statusProcessing", fallback: "Đang xử lý" },
  { value: "PENDING_APPROVAL", key: "superadmin.ops.statusPendingApproval", fallback: "Chờ duyệt" },
  { value: "INACTIVE", key: "superadmin.ops.statusInactive", fallback: "Tạm dừng" },
  { value: "DEPRECATED", key: "superadmin.ops.statusDeprecated", fallback: "Ngừng khuyến nghị" },
  { value: "QUARANTINED", key: "superadmin.ops.statusQuarantinedShort", fallback: "Cách ly" },
];

function matchesSearchQuery(plugin: CatalogPlugin, query: string): boolean {
  if (!query) return true;
  const localizedTitles = Object.values(plugin.titleI18n)
    .filter((v): v is string => typeof v === "string")
    .join(" ");
  return `${plugin.pluginId} ${localizedTitles} ${plugin.repoUrl ?? ""} ${plugin.pinnedCommitSha ?? ""}`
    .toLowerCase()
    .includes(query.toLowerCase());
}

function comparePlugins(
  a: CatalogPlugin,
  b: CatalogPlugin,
  opts: { col: SortColumn; dir: SortDirection; locale: string }
): number {
  let diff = a.updatedAt - b.updatedAt;
  if (opts.col === "plugin") {
    diff = resolveLocalizedTitle(a, opts.locale).localeCompare(resolveLocalizedTitle(b, opts.locale), opts.locale);
  } else if (opts.col === "source") {
    diff = formatPluginSourceLabel(a.sourceKind).localeCompare(formatPluginSourceLabel(b.sourceKind), opts.locale);
  } else if (opts.col === "status") {
    diff = STATUS_SORT_ORDER.indexOf(a.lifecycleStatus) - STATUS_SORT_ORDER.indexOf(b.lifecycleStatus);
  }
  return opts.dir === "asc" ? diff : -diff;
}

function CatalogPolicyBadges({ signedManifest }: { signedManifest: SignedCatalogManifest | null }) {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  return (
    <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: "8px", fontSize: "11px", color: "var(--text-secondary)" }}>
      <span style={{ padding: "3px 10px", borderRadius: "999px", backgroundColor: "var(--state-success-soft)", color: "var(--state-success)", fontWeight: FONT_WEIGHT_BOLD }}>
        {signedManifest
          ? t("superadmin.ops.catalogManifestSeqBadge", "Ed25519 · catalog.json · chuỗi #{{seq}}", {
              seq: formatNumber(signedManifest.manifestSeq),
            })
          : t("superadmin.ops.catalogManifestReadyBadge", "Ed25519 · catalog.json · đã sẵn sàng")}
      </span>
      <span style={{ padding: "3px 8px", borderRadius: "6px", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)" }}>
        {t("superadmin.ops.catalogLocalOverrideBadge", "Ưu tiên cấu hình cục bộ (Local Override > Cloud Default)")}
      </span>
      {signedManifest && (
        <code data-testid="catalog-ed25519-sig" style={{ padding: "3px 8px", borderRadius: "6px", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)" }}>
          {signedManifest.keyId}: {signedManifest.signatureBase64.slice(0, SIG_PREVIEW_LEN)}…
        </code>
      )}
      <span style={SR_ONLY_STYLE}>ACTIVE | INACTIVE | DEPRECATED | QUARANTINED | PROCESSING | PENDING_APPROVAL</span>
    </div>
  );
}

function CatalogHeaderBanner({
  signedManifest,
  canEdit,
  totalCount,
  onOpenCreate,
}: {
  signedManifest: SignedCatalogManifest | null;
  canEdit: boolean;
  totalCount: number;
  onOpenCreate: () => void;
}) {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  return (
    <div style={CARD_STYLE} data-testid="catalog-policy-banner">
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "10px", flexWrap: "wrap" }}>
        <div style={{ display: "inline-flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
          <BrandIcon name="plugin" size={ICON_SIZE_MD} color="var(--action-primary)" />
          <h2 style={{ margin: 0, fontSize: "15px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)" }}>
            {t("superadmin.ops.catalogTitle", "Quản trị Danh mục Plugin")}
          </h2>
          <span style={{ padding: "2px 9px", borderRadius: "999px", backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
            {t("superadmin.ops.catalogPluginCount", "{{count}} plugin", { count: formatNumber(totalCount) })}
          </span>
        </div>
        <div style={{ display: "inline-flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
          {!canEdit && (
            <span data-testid="catalog-readonly-lock" style={{ padding: "4px 10px", borderRadius: "999px", backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>
              {t("superadmin.ops.readOnlyBadge", "Chế độ chỉ đọc (RBAC)")}
            </span>
          )}
          <button type="button" disabled={!canEdit} data-testid="catalog-open-create-drawer-btn" onClick={onOpenCreate} style={{ height: "34px", padding: "0 14px", borderRadius: "8px", border: "none", backgroundColor: canEdit ? "var(--action-primary)" : "var(--bg-subtle)", color: canEdit ? "var(--action-primary-fg)" : "var(--text-disabled)", fontSize: "13px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: canEdit ? "pointer" : "not-allowed", display: "inline-flex", alignItems: "center", gap: "6px" }}>
            <BrandIcon name="plus" size={ICON_SIZE_SM} color="currentColor" />
            <span>{t("superadmin.ops.catalogAddBtn", "Thêm Plugin")}</span>
          </button>
        </div>
      </div>
      <CatalogPolicyBadges signedManifest={signedManifest} />
    </div>
  );
}

function FilterChipButton({ label, active, testId, onClick }: { label: string; active: boolean; testId: string; onClick: () => void }) {
  return (
    <button type="button" data-testid={testId} aria-pressed={active} onClick={onClick} style={{ height: "28px", padding: "0 10px", borderRadius: "999px", border: `1px solid ${active ? "var(--action-primary)" : "var(--border-default)"}`, backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)", color: active ? "var(--action-primary)" : "var(--text-secondary)", fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer", whiteSpace: "nowrap" }}>
      {label}
    </button>
  );
}

function CatalogFilterBar({
  searchQuery,
  sourceFilter,
  statusFilter,
  onSearchChange,
  onSourceChange,
  onStatusChange,
}: {
  searchQuery: string;
  sourceFilter: SourceFilterValue;
  statusFilter: StatusFilterValue;
  onSearchChange: (q: string) => void;
  onSourceChange: (s: SourceFilterValue) => void;
  onStatusChange: (st: StatusFilterValue) => void;
}) {
  const { t } = useTranslation();
  return (
    <div style={FILTER_BAR_STYLE} data-testid="catalog-filter-bar">
      <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: "10px" }}>
        <div style={{ position: "relative", flex: "1 1 260px", display: "flex", alignItems: "center" }}>
          <span style={{ position: "absolute", left: "10px", pointerEvents: "none" }}><BrandIcon name="search" size={ICON_SIZE_SM} color="var(--text-muted)" /></span>
          <input
            type="search"
            value={searchQuery}
            onChange={(e) => onSearchChange(e.target.value)}
            data-testid="catalog-search-input"
            placeholder={t("superadmin.ops.catalogSearchPlaceholder", "Tìm theo mã plugin, tên hiển thị, kho nguồn hoặc Commit SHA...")}
            style={SEARCH_INPUT_STYLE}
          />
        </div>
        <div style={{ display: "flex", flexWrap: "wrap", gap: "6px" }} role="group" aria-label={t("superadmin.ops.filterBySourceAria", "Lọc theo nguồn")}>
          {SOURCE_CHIPS.map((c) => <FilterChipButton key={c.value} label={t(c.key, c.fallback)} active={sourceFilter === c.value} testId={`catalog-source-chip-${c.value}`} onClick={() => onSourceChange(c.value)} />)}
        </div>
      </div>
      <div style={{ display: "flex", flexWrap: "wrap", gap: "6px" }} role="group" aria-label={t("superadmin.ops.filterByStatusAria", "Lọc theo trạng thái")}>
        {STATUS_CHIPS.map((c) => <FilterChipButton key={c.value} label={t(c.key, c.fallback)} active={statusFilter === c.value} testId={`catalog-status-chip-${c.value}`} onClick={() => onStatusChange(c.value)} />)}
      </div>
    </div>
  );
}

function SortHeaderButton({ label, column, activeCol, direction, testId, onSort }: { label: string; column: SortColumn; activeCol: SortColumn; direction: SortDirection; testId: string; onSort: (col: SortColumn) => void }) {
  const isActive = activeCol === column;
  let indicator = "↕";
  if (isActive) indicator = direction === "asc" ? "↑" : "↓";
  return (
    <button type="button" data-testid={testId} onClick={() => onSort(column)} style={{ background: "transparent", border: "none", padding: 0, display: "inline-flex", alignItems: "center", gap: "5px", color: isActive ? "var(--text-primary)" : "var(--text-muted)", fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer" }}>
      <span>{label}</span>
      <span aria-hidden="true" style={{ fontSize: "11px" }}>{indicator}</span>
    </button>
  );
}

function CatalogPluginRow({ plugin, isSelected, isActiveDrawer, onToggleSelect, onOpenDetail }: { plugin: CatalogPlugin; isSelected: boolean; isActiveDrawer: boolean; onToggleSelect: (id: string) => void; onOpenDetail: (id: string) => void }) {
  const { t, locale } = useTranslation();
  const { formatDate, formatTime } = useFormatter({ locale });
  const statusMeta = PLUGIN_STATUS_META[plugin.lifecycleStatus];
  const localizedTitle = resolveLocalizedTitle(plugin, locale);
  return (
    <tr data-testid={`catalog-row-${plugin.pluginId}`} onClick={() => onOpenDetail(plugin.pluginId)} style={{ backgroundColor: isSelected || isActiveDrawer ? "var(--bg-selected)" : "transparent", cursor: "pointer" }}>
      <td style={{ ...TD_STYLE, width: "36px" }} onClick={(e) => e.stopPropagation()}>
        <input type="checkbox" aria-label={t("superadmin.ops.selectPluginAria", "Chọn plugin {{title}}", { title: localizedTitle })} data-testid={`catalog-row-select-${plugin.pluginId}`} checked={isSelected} onChange={() => onToggleSelect(plugin.pluginId)} />
      </td>
      <td style={TD_STYLE}>
        <div style={{ fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)" }}>{localizedTitle}</div>
        <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>{t("superadmin.ops.pluginIdMinVersion", "{{pluginId}} · tối thiểu v{{version}}", { pluginId: plugin.pluginId, version: plugin.minAppVersion })}</div>
      </td>
      <td style={TD_STYLE}>
        <span style={{ display: "inline-block", padding: "2px 8px", borderRadius: "6px", backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, whiteSpace: "nowrap" }}>{formatPluginSourceLabel(plugin.sourceKind, t)}</span>
      </td>
      <td style={TD_STYLE}>
        <span data-testid={`catalog-status-badge-${plugin.pluginId}`} style={{ display: "inline-flex", padding: "3px 9px", borderRadius: "999px", backgroundColor: statusMeta.bg, color: statusMeta.fg, fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD, whiteSpace: "nowrap" }}>{formatPluginStatusLabel(plugin.lifecycleStatus, t)}</span>
      </td>
      <td style={TD_STYLE}>
        {plugin.repoUrl ? (
          <div style={{ fontSize: "11px", wordBreak: "break-all" }}><div>{plugin.repoUrl}</div><code style={{ color: "var(--text-secondary)" }}>@{plugin.pinnedCommitSha ?? "—"}</code></div>
        ) : (
          <span style={{ fontSize: "12px", color: "var(--text-muted)" }}>{t("superadmin.ops.builtinSource", "Tích hợp lõi")}</span>
        )}
      </td>
      <td style={{ ...TD_STYLE, whiteSpace: "nowrap", fontSize: "12px", color: "var(--text-secondary)" }}>
        <div>{formatDate(plugin.updatedAt)}</div>
        <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>{formatTime(plugin.updatedAt)}</div>
      </td>
    </tr>
  );
}

interface CatalogTableProps {
  plugins: CatalogPlugin[];
  totalCount: number;
  sortCol: SortColumn;
  sortDir: SortDirection;
  selectedIds: Set<string>;
  allVisibleSelected: boolean;
  activeDetailId: string | null;
  onSort: (col: SortColumn) => void;
  onToggleSelectAll: () => void;
  onToggleRow: (id: string) => void;
  onOpenDetail: (id: string) => void;
}

function CatalogDataTable(props: CatalogTableProps) {
  const { plugins, totalCount, sortCol, sortDir, selectedIds, allVisibleSelected, activeDetailId, onSort, onToggleSelectAll, onToggleRow, onOpenDetail } = props;
  const { t } = useTranslation();
  return (
    <div style={{ overflowX: "auto", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "10px" }} data-ff-id="superadmin-catalog-table-card">
      <table style={{ width: "100%", borderCollapse: "collapse", minWidth: "660px" }} data-testid="catalog-plugins-table">
        <thead>
          <tr>
            <th style={{ ...TH_STYLE, width: "36px" }}>
              <input type="checkbox" aria-label={t("superadmin.ops.selectAllVisibleAria", "Chọn tất cả plugin đang hiển thị")} data-testid="catalog-select-all" checked={allVisibleSelected} onChange={onToggleSelectAll} />
            </th>
            <th style={TH_STYLE}><SortHeaderButton label={t("superadmin.ops.colPlugin", "Plugin")} column="plugin" activeCol={sortCol} direction={sortDir} testId="catalog-sort-plugin" onSort={onSort} /></th>
            <th style={TH_STYLE}><SortHeaderButton label={t("superadmin.ops.colSource", "Nguồn")} column="source" activeCol={sortCol} direction={sortDir} testId="catalog-sort-source" onSort={onSort} /></th>
            <th style={TH_STYLE}><SortHeaderButton label={t("superadmin.ops.colStatus", "Trạng thái")} column="status" activeCol={sortCol} direction={sortDir} testId="catalog-sort-status" onSort={onSort} /></th>
            <th style={TH_STYLE}>{t("superadmin.ops.colPinnedRef", "Kho nguồn / Commit ghim")}</th>
            <th style={TH_STYLE}><SortHeaderButton label={t("superadmin.ops.colUpdated", "Cập nhật")} column="updated" activeCol={sortCol} direction={sortDir} testId="catalog-sort-updated" onSort={onSort} /></th>
          </tr>
        </thead>
        <tbody>
          {plugins.length === 0 ? (
            <tr>
              <td colSpan={TABLE_COL_SPAN} style={{ ...TD_STYLE, textAlign: "center", color: "var(--text-muted)", padding: "24px 12px" }}>
                {totalCount === 0
                  ? t("superadmin.ops.catalogEmpty", "Chưa có dữ liệu danh mục plugin.")
                  : t("superadmin.ops.catalogFilteredEmpty", "Không có plugin nào khớp với bộ lọc hiện tại.")}
              </td>
            </tr>
          ) : (
            plugins.map((p) => <CatalogPluginRow key={p.pluginId} plugin={p} isSelected={selectedIds.has(p.pluginId)} isActiveDrawer={activeDetailId === p.pluginId} onToggleSelect={onToggleRow} onOpenDetail={onOpenDetail} />)
          )}
        </tbody>
      </table>
    </div>
  );
}

interface BulkFallbackInput {
  ids: string[];
  plugins: CatalogPlugin[];
  lifecycleStatus: PluginLifecycleStatus;
  reason: string;
  onUpsertPlugin: (input: UpsertCatalogPluginRequest) => Promise<boolean>;
}

async function applyBulkFallback({ ids, plugins, lifecycleStatus, reason, onUpsertPlugin }: BulkFallbackInput): Promise<boolean> {
  let allOk = true;
  for (const id of ids) {
    const target = plugins.find((p) => p.pluginId === id);
    if (!target) continue;
    const ok = await onUpsertPlugin({
      pluginId: target.pluginId,
      sourceKind: target.sourceKind,
      lifecycleStatus,
      repoUrl: target.repoUrl ?? null,
      pinnedCommitSha: target.pinnedCommitSha ?? null,
      minAppVersion: target.minAppVersion,
      defaultEnabled: target.sourceKind === "RECOMMENDED_GITHUB" ? false : target.defaultEnabled,
      titleI18n: target.titleI18n,
      summaryI18n: target.summaryI18n,
      reason,
    });
    if (!ok) allOk = false;
  }
  return allOk;
}

function useCatalogTableState(plugins: CatalogPlugin[], locale: string) {
  const [searchQuery, setSearchQuery] = useState("");
  const [sourceFilter, setSourceFilter] = useState<SourceFilterValue>("ALL");
  const [statusFilter, setStatusFilter] = useState<StatusFilterValue>("ALL");
  const [sortCol, setSortCol] = useState<SortColumn>("updated");
  const [sortDir, setSortDir] = useState<SortDirection>("desc");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    const validIds = new Set(plugins.map((p) => p.pluginId));
    setSelectedIds((prev) => new Set(Array.from(prev).filter((id) => validIds.has(id))));
  }, [plugins]);

  const visiblePlugins = useMemo(
    () =>
      plugins
        .filter((p) => (sourceFilter === "ALL" || p.sourceKind === sourceFilter) && (statusFilter === "ALL" || p.lifecycleStatus === statusFilter) && matchesSearchQuery(p, searchQuery.trim()))
        .sort((a, b) => comparePlugins(a, b, { col: sortCol, dir: sortDir, locale })),
    [plugins, searchQuery, sourceFilter, statusFilter, sortCol, sortDir, locale]
  );

  const handleSort = (col: SortColumn) => {
    if (sortCol === col) {
      setSortDir((prev) => (prev === "asc" ? "desc" : "asc"));
      return;
    }
    setSortCol(col);
    setSortDir(col === "updated" ? "desc" : "asc");
  };

  const toggleRow = (id: string) => setSelectedIds((prev) => {
    const next = new Set(prev);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    return next;
  });

  const allVisibleSelected = visiblePlugins.length > 0 && visiblePlugins.every((p) => selectedIds.has(p.pluginId));
  const toggleAllVisible = () => setSelectedIds((prev) => {
    const next = new Set(prev);
    for (const p of visiblePlugins) {
      if (allVisibleSelected) next.delete(p.pluginId);
      else next.add(p.pluginId);
    }
    return next;
  });

  return { searchQuery, setSearchQuery, sourceFilter, setSourceFilter, statusFilter, setStatusFilter, sortCol, sortDir, handleSort, selectedIds, setSelectedIds, visiblePlugins, allVisibleSelected, toggleRow, toggleAllVisible };
}

export const PluginCatalogManagerTab: React.FC<PluginCatalogManagerTabProps> = ({
  plugins,
  signedManifest,
  operatorRole,
  submitting,
  onUpsertPlugin,
  onBulkUpdateCatalog,
}) => {
  const { locale } = useTranslation();
  const canEdit = operatorRole === "SUPER_ADMIN" || operatorRole === "SKILL_CURATOR";
  const table = useCatalogTableState(plugins, locale);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [drawerMode, setDrawerMode] = useState<CatalogDrawerMode>("create");
  const [activePluginId, setActivePluginId] = useState<string | null>(null);

  const activePlugin = useMemo(() => (activePluginId ? plugins.find((p) => p.pluginId === activePluginId) ?? null : null), [activePluginId, plugins]);
  const resolvedBulkHandler = onBulkUpdateCatalog ?? resolveBulkCatalogHandler(onUpsertPlugin);

  const handleBulkApply = async (lifecycleStatus: PluginLifecycleStatus, reason: string): Promise<boolean> => {
    const ids = Array.from(table.selectedIds);
    if (ids.length === 0) return false;
    if (resolvedBulkHandler) return resolvedBulkHandler({ pluginIds: ids, lifecycleStatus, reason });
    return applyBulkFallback({ ids, plugins, lifecycleStatus, reason, onUpsertPlugin });
  };

  return (
    <section data-ff-id="superadmin-plugin-catalog-panel" data-testid="superadmin-panel-plugin-catalog" style={{ display: "flex", flexDirection: "column", gap: "12px", paddingBottom: table.selectedIds.size > 0 ? "76px" : "0" }}>
      <CatalogHeaderBanner signedManifest={signedManifest} canEdit={canEdit} totalCount={plugins.length} onOpenCreate={() => { if (!canEdit) return; setActivePluginId(null); setDrawerMode("create"); setDrawerOpen(true); }} />
      <CatalogFilterBar searchQuery={table.searchQuery} sourceFilter={table.sourceFilter} statusFilter={table.statusFilter} onSearchChange={table.setSearchQuery} onSourceChange={table.setSourceFilter} onStatusChange={table.setStatusFilter} />
      <CatalogDataTable plugins={table.visiblePlugins} totalCount={plugins.length} sortCol={table.sortCol} sortDir={table.sortDir} selectedIds={table.selectedIds} allVisibleSelected={table.allVisibleSelected} activeDetailId={drawerOpen && drawerMode === "detail" ? activePluginId : null} onSort={table.handleSort} onToggleSelectAll={table.toggleAllVisible} onToggleRow={table.toggleRow} onOpenDetail={(id) => { setActivePluginId(id); setDrawerMode("detail"); setDrawerOpen(true); }} />
      <PluginCatalogBulkBar selectedCount={table.selectedIds.size} canEdit={canEdit} submitting={submitting} onApplyStatus={handleBulkApply} onClearSelection={() => table.setSelectedIds(new Set())} />
      <PluginCatalogDetailDrawer open={drawerOpen} mode={drawerMode} plugin={activePlugin} signedManifest={signedManifest} operatorRole={operatorRole} submitting={submitting} onClose={() => setDrawerOpen(false)} onSubmitPlugin={onUpsertPlugin} />
    </section>
  );
};
