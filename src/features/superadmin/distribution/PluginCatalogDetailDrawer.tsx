// @story 14.3
// @implements AC-02, AC-05 (DESIGN.md [5.25] RightDrawer for Plugin Catalog Create, Detail Inspection & Inline Edit with Async Security Review Lifecycle)
import React, { useEffect, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { getGlobalLocale, translateGlobal, useTranslation } from "../../../hooks/useTranslation";
import type { CloudAdminRole } from "../../../lib/validations/cloud-admin";
import {
  ASCII_IDENTIFIER_REGEX,
  type CatalogPlugin,
  isValidGithubRepoUrl,
  isValidPinnedCommitOrTag,
  type PluginLifecycleStatus,
  type PluginSourceKind,
  type SignedCatalogManifest,
  type UpsertCatalogPluginRequest,
} from "../../../lib/validations/distribution-ops";

export type CatalogDrawerMode = "create" | "detail";

export interface PluginCatalogDetailDrawerProps {
  open: boolean;
  mode: CatalogDrawerMode;
  plugin: CatalogPlugin | null;
  signedManifest: SignedCatalogManifest | null;
  operatorRole?: CloudAdminRole;
  submitting: boolean;
  onClose: () => void;
  onSubmitPlugin: (input: UpsertCatalogPluginRequest) => Promise<boolean>;
}

type TranslateFn = (key: string, fallback?: string, vars?: Record<string, string | number>) => string;

const BACKDROP_Z_INDEX = 9980;
const DRAWER_Z_INDEX = 9981;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const ICON_SIZE_SM = 14;
const MIN_REASON_LEN = 5;
const SIG_PREVIEW_LEN = 24;

export interface StatusMeta {
  labelKey: string;
  labelFallback: string;
  bg: string;
  fg: string;
}

export const PLUGIN_STATUS_META: Record<PluginLifecycleStatus, StatusMeta> = {
  ACTIVE: { labelKey: "superadmin.ops.statusActive", labelFallback: "Đang hoạt động", bg: "var(--state-success-soft)", fg: "var(--state-success)" },
  PROCESSING: { labelKey: "superadmin.ops.statusProcessing", labelFallback: "Đang xử lý", bg: "var(--state-info-soft)", fg: "var(--state-info)" },
  PENDING_APPROVAL: { labelKey: "superadmin.ops.statusPendingApproval", labelFallback: "Chờ duyệt", bg: "var(--state-warning-soft)", fg: "var(--state-warning)" },
  INACTIVE: { labelKey: "superadmin.ops.statusInactive", labelFallback: "Tạm dừng", bg: "var(--bg-subtle)", fg: "var(--text-muted)" },
  DEPRECATED: { labelKey: "superadmin.ops.statusDeprecated", labelFallback: "Ngừng khuyến nghị", bg: "var(--state-warning-soft)", fg: "var(--state-warning)" },
  QUARANTINED: { labelKey: "superadmin.ops.statusQuarantined", labelFallback: "Đang cách ly", bg: "var(--state-danger-soft)", fg: "var(--state-danger)" },
};

export function formatPluginStatusLabel(status: PluginLifecycleStatus, t: TranslateFn = translateGlobal): string {
  const meta = PLUGIN_STATUS_META[status];
  return t(meta.labelKey, meta.labelFallback);
}

const SOURCE_I18N: Record<PluginSourceKind, { key: string; fallback: string }> = {
  DEFAULT_BUILTIN: { key: "superadmin.ops.sourceDefaultBuiltin", fallback: "Tích hợp Mặc định" },
  RECOMMENDED_GITHUB: { key: "superadmin.ops.sourceRecommendedGithub", fallback: "GitHub Đề xuất" },
};

export const formatPluginSourceLabel = (sourceKind: PluginSourceKind, t: TranslateFn = translateGlobal): string =>
  t(SOURCE_I18N[sourceKind].key, SOURCE_I18N[sourceKind].fallback);

function pickLocalizedText(map: Record<string, unknown>, fallback: string, locale?: string): string {
  const langPrefix = (locale ?? getGlobalLocale()).trim().toLowerCase().split(/[-_]/)[0];
  const candidate = [langPrefix, "en", "vi"].map((k) => map[k]).find((v): v is string => typeof v === "string" && v.trim().length > 0);
  return candidate ?? fallback;
}

export const resolveLocalizedTitle = (plugin: CatalogPlugin, locale?: string): string =>
  pickLocalizedText(plugin.titleI18n, plugin.pluginId, locale);

export const resolveLocalizedSummary = (plugin: CatalogPlugin, locale?: string): string =>
  pickLocalizedText(plugin.summaryI18n, "", locale);

const BACKDROP_STYLE: React.CSSProperties = { position: "fixed", inset: 0, backgroundColor: "var(--bg-overlay)", zIndex: BACKDROP_Z_INDEX };
const DRAWER_STYLE: React.CSSProperties = {
  position: "fixed", top: 0, right: 0, bottom: 0, width: "500px", maxWidth: "100vw",
  backgroundColor: "var(--bg-surface)", borderLeft: "1px solid var(--border-default)",
  boxShadow: "var(--shadow-lg)", zIndex: DRAWER_Z_INDEX, display: "flex", flexDirection: "column",
};
const SECTION_STYLE: React.CSSProperties = {
  padding: "12px", borderRadius: "10px", border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-card, var(--bg-subtle))", fontSize: "12px",
  color: "var(--text-secondary)", display: "flex", flexDirection: "column", gap: "8px",
};
const FIELD_STYLE: React.CSSProperties = {
  width: "100%", padding: "8px 10px", borderRadius: "8px", border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", fontSize: "13px", boxSizing: "border-box",
};
const SELECT_STYLE: React.CSSProperties = {
  ...FIELD_STYLE, height: "36px", padding: "0 32px 0 10px", appearance: "none",
  WebkitAppearance: "none", MozAppearance: "none", textOverflow: "ellipsis", overflow: "hidden", whiteSpace: "nowrap",
};
const SELECT_CHEVRON_STYLE: React.CSSProperties = {
  position: "absolute", right: "10px", top: "50%", transform: "translateY(-50%)",
  pointerEvents: "none", display: "inline-flex", alignItems: "center",
};
const LABEL_STYLE: React.CSSProperties = {
  display: "flex", flexDirection: "column", gap: "4px", fontSize: "12px",
  fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-secondary)",
};

interface StyledSelectProps {
  disabled: boolean;
  value: string;
  testId: string;
  onChange: (value: string) => void;
  children: React.ReactNode;
}

function StyledSelect({ disabled, value, testId, onChange, children }: StyledSelectProps) {
  return (
    <div style={{ position: "relative", width: "100%" }}>
      <select disabled={disabled} value={value} onChange={(e) => onChange(e.target.value)} data-testid={testId} style={{ ...SELECT_STYLE, cursor: disabled ? "not-allowed" : "pointer" }}>
        {children}
      </select>
      <span aria-hidden="true" style={SELECT_CHEVRON_STYLE}>
        <BrandIcon name="chevron-down" size={ICON_SIZE_SM} color="var(--text-muted)" />
      </span>
    </div>
  );
}

const QUICK_ACTIONS: ReadonlyArray<{ status: PluginLifecycleStatus; labelKey: string; labelFallback: string; testId: string; bg: string; fg: string }> = [
  { status: "ACTIVE", labelKey: "superadmin.ops.quickApproveBtn", labelFallback: "Phê duyệt & Kích hoạt", testId: "catalog-quick-approve-btn", bg: "var(--state-success-soft)", fg: "var(--state-success)" },
  { status: "PENDING_APPROVAL", labelKey: "superadmin.ops.quickPendingBtn", labelFallback: "Chuyển chờ duyệt", testId: "catalog-quick-pending-btn", bg: "var(--state-warning-soft)", fg: "var(--state-warning)" },
  { status: "INACTIVE", labelKey: "superadmin.ops.quickInactiveBtn", labelFallback: "Tạm dừng", testId: "catalog-quick-inactive-btn", bg: "var(--bg-subtle)", fg: "var(--text-secondary)" },
  { status: "QUARANTINED", labelKey: "superadmin.ops.quickQuarantineBtn", labelFallback: "Cách ly", testId: "catalog-quick-quarantine-btn", bg: "var(--state-danger-soft)", fg: "var(--state-danger)" },
];

interface DrawerFormState {
  pluginId: string;
  titleInput: string;
  summaryInput: string;
  sourceKind: PluginSourceKind;
  lifecycleStatus: PluginLifecycleStatus;
  repoUrl: string;
  pinnedCommitSha: string;
  minAppVersion: string;
  defaultEnabled: boolean;
  reason: string;
}

function createInitialFormState(mode: CatalogDrawerMode, plugin: CatalogPlugin | null, locale: string): DrawerFormState {
  if (mode === "detail" && plugin) {
    return {
      pluginId: plugin.pluginId, titleInput: resolveLocalizedTitle(plugin, locale),
      summaryInput: resolveLocalizedSummary(plugin, locale), sourceKind: plugin.sourceKind,
      lifecycleStatus: plugin.lifecycleStatus, repoUrl: plugin.repoUrl ?? "",
      pinnedCommitSha: plugin.pinnedCommitSha ?? "", minAppVersion: plugin.minAppVersion || "0.1.0",
      defaultEnabled: plugin.defaultEnabled, reason: "",
    };
  }
  return {
    pluginId: "", titleInput: "", summaryInput: "", sourceKind: "RECOMMENDED_GITHUB",
    lifecycleStatus: "PROCESSING", repoUrl: "", pinnedCommitSha: "", minAppVersion: "0.1.0", defaultEnabled: false, reason: "",
  };
}

function isDrawerFormValid(form: DrawerFormState): boolean {
  const baseValid = ASCII_IDENTIFIER_REGEX.test(form.pluginId.trim()) && form.reason.trim().length >= MIN_REASON_LEN;
  return baseValid && (form.sourceKind !== "RECOMMENDED_GITHUB" || (isValidGithubRepoUrl(form.repoUrl) && isValidPinnedCommitOrTag(form.pinnedCommitSha)));
}

function buildUpsertPayload(form: DrawerFormState, locale: string): UpsertCatalogPluginRequest {
  const isGithub = form.sourceKind === "RECOMMENDED_GITHUB";
  const title = form.titleInput.trim() || form.pluginId.trim();
  const summary = form.summaryInput.trim() || translateGlobal("superadmin.ops.catalogDefaultSummary", "Plugin được quản lý tập trung trong danh mục phân phối.");
  const langPrefix = locale.trim().toLowerCase().split(/[-_]/)[0] || "vi";
  return {
    pluginId: form.pluginId.trim(), sourceKind: form.sourceKind, lifecycleStatus: form.lifecycleStatus,
    asyncSecurityScan: form.lifecycleStatus === "PROCESSING",
    repoUrl: isGithub ? form.repoUrl.trim() : null, pinnedCommitSha: isGithub ? form.pinnedCommitSha.trim() : null,
    minAppVersion: form.minAppVersion.trim() || "0.1.0", defaultEnabled: isGithub ? false : form.defaultEnabled,
    titleI18n: { vi: title, en: title, [langPrefix]: title },
    summaryI18n: { vi: summary, en: summary, [langPrefix]: summary },
    reason: form.reason.trim(),
  };
}

const ASYNC_BANNER_COPY: Partial<Record<PluginLifecycleStatus, { titleKey: string; titleFallback: string; bodyKey: string; bodyFallback: string; bg: string; fg: string }>> = {
  PROCESSING: {
    titleKey: "superadmin.ops.asyncProcessingTitle", titleFallback: "Đang quét bảo mật bất đồng bộ",
    bodyKey: "superadmin.ops.asyncProcessingBody",
    bodyFallback: "Plugin đã vào bảng theo dõi. Thiết bị người dùng sẽ không tự động kích hoạt cho đến khi được phê duyệt.",
    bg: "var(--state-info-soft)", fg: "var(--state-info)",
  },
  PENDING_APPROVAL: {
    titleKey: "superadmin.ops.asyncPendingTitle", titleFallback: "Đã hoàn tất quét — Chờ phê duyệt",
    bodyKey: "superadmin.ops.asyncPendingBody",
    bodyFallback: "Nhấn \"Phê duyệt & Kích hoạt\" khi sẵn sàng phân phối tới thiết bị người dùng.",
    bg: "var(--state-warning-soft)", fg: "var(--state-warning)",
  },
};

function AsyncSecurityBanner({ status }: { status: PluginLifecycleStatus }) {
  const { t } = useTranslation();
  const copy = ASYNC_BANNER_COPY[status];
  if (!copy) return null;
  return (
    <div data-testid="catalog-async-security-pill" style={{ padding: "10px 12px", borderRadius: "8px", backgroundColor: copy.bg, color: copy.fg, fontSize: "12px" }}>
      <strong>{t(copy.titleKey, copy.titleFallback)}</strong>
      <div>{t(copy.bodyKey, copy.bodyFallback)}</div>
    </div>
  );
}

function PluginMetadataCard({ plugin, signedManifest }: { plugin: CatalogPlugin; signedManifest: SignedCatalogManifest | null }) {
  const { t, locale } = useTranslation();
  const { formatDate, formatTime, formatNumber } = useFormatter({ locale });
  const statusMeta = PLUGIN_STATUS_META[plugin.lifecycleStatus];
  const summary = resolveLocalizedSummary(plugin, locale);
  return (
    <div style={SECTION_STYLE} data-testid="catalog-drawer-metadata">
      <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: "8px" }}>
        <span style={{ padding: "3px 9px", borderRadius: "999px", backgroundColor: "var(--bg-subtle)", color: "var(--text-primary)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{formatPluginSourceLabel(plugin.sourceKind, t)}</span>
        <span data-testid="catalog-drawer-status-pill" style={{ padding: "3px 9px", borderRadius: "999px", backgroundColor: statusMeta.bg, color: statusMeta.fg, fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD }}>{formatPluginStatusLabel(plugin.lifecycleStatus, t)}</span>
        <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>{t("superadmin.ops.minAppVersionShort", "Tối thiểu v{{version}}", { version: plugin.minAppVersion })}</span>
      </div>
      {summary && <div style={{ color: "var(--text-primary)" }}>{summary}</div>}
      <AsyncSecurityBanner status={plugin.lifecycleStatus} />
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px", fontSize: "11px" }}>
        <div><div style={{ color: "var(--text-muted)" }}>{t("superadmin.ops.lastUpdatedLabel", "Cập nhật lần cuối")}</div><div style={{ color: "var(--text-primary)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{formatDate(plugin.updatedAt)} {formatTime(plugin.updatedAt)}</div></div>
        <div><div style={{ color: "var(--text-muted)" }}>{t("superadmin.ops.updatedByLabel", "Người cập nhật")}</div><div style={{ color: "var(--text-primary)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{plugin.updatedBy}</div></div>
      </div>
      {plugin.sourceKind === "RECOMMENDED_GITHUB" && (
        <div style={{ fontSize: "11px", wordBreak: "break-all" }}>
          <div style={{ color: "var(--text-muted)" }}>{t("superadmin.ops.repoAndCommitLabel", "Kho nguồn & Commit ghim")}</div>
          <div style={{ color: "var(--text-primary)" }}>{plugin.repoUrl ?? "—"}</div>
          <code style={{ color: "var(--text-secondary)" }}>@{plugin.pinnedCommitSha ?? "—"}</code>
        </div>
      )}
      {signedManifest && (
        <div data-testid="catalog-drawer-ed25519-meta" style={{ paddingTop: "6px", borderTop: "1px solid var(--border-subtle)", fontSize: "11px" }}>
          <div style={{ color: "var(--text-muted)" }}>{t("superadmin.ops.catalogManifestSeqBadge", "Ed25519 · catalog.json · chuỗi #{{seq}}", { seq: formatNumber(signedManifest.manifestSeq) })}</div>
          <code style={{ color: "var(--text-secondary)", wordBreak: "break-all" }}>{signedManifest.keyId}: {signedManifest.signatureBase64.slice(0, SIG_PREVIEW_LEN)}…</code>
        </div>
      )}
    </div>
  );
}

const QUICK_BTN_DISABLED_STYLE: React.CSSProperties = {
  height: "32px", padding: "0 11px", borderRadius: "8px", border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-subtle)", color: "var(--text-disabled)", fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "not-allowed",
};

function QuickStatusStrip({ plugin, canEdit, submitting, reason, onTrigger }: { plugin: CatalogPlugin; canEdit: boolean; submitting: boolean; reason: string; onTrigger: (nextStatus: PluginLifecycleStatus, auditReason: string) => void }) {
  const { t } = useTranslation();
  return (
    <div style={SECTION_STYLE} data-testid="catalog-quick-actions">
      <div style={{ fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)" }}>{t("superadmin.ops.quickStatusTitle", "Chuyển trạng thái nhanh")}</div>
      <div style={{ display: "flex", flexWrap: "wrap", gap: "8px" }}>
        {QUICK_ACTIONS.map((act) => {
          const disabled = !canEdit || submitting || plugin.lifecycleStatus === act.status;
          const label = t(act.labelKey, act.labelFallback);
          const auditReason = reason.trim().length >= MIN_REASON_LEN ? reason.trim() : t("superadmin.ops.quickActionDefaultReason", "{{action}} cho plugin {{pluginId}}", { action: label, pluginId: plugin.pluginId });
          const btnStyle: React.CSSProperties = disabled ? QUICK_BTN_DISABLED_STYLE : { ...QUICK_BTN_DISABLED_STYLE, backgroundColor: act.bg, color: act.fg, cursor: "pointer" };
          return (
            <button key={act.status} type="button" disabled={disabled} data-testid={act.testId} onClick={() => onTrigger(act.status, auditReason)} style={btnStyle}>
              {label}
            </button>
          );
        })}
      </div>
    </div>
  );
}

function DrawerIdentityFields({ form, isCreate, canEdit, onPatch, onSourceChange }: { form: DrawerFormState; isCreate: boolean; canEdit: boolean; onPatch: (p: Partial<DrawerFormState>) => void; onSourceChange: (s: PluginSourceKind) => void }) {
  const { t } = useTranslation();
  return (
    <>
      <label style={LABEL_STYLE}>
        <span>{t("superadmin.ops.fieldPluginId", "Mã định danh Plugin")}</span>
        <input type="text" disabled={!canEdit || !isCreate} value={form.pluginId} onChange={(e) => onPatch({ pluginId: e.target.value })} data-testid="catalog-plugin-id-input" placeholder={t("superadmin.ops.pluginIdPlaceholder", "vd: figma-tokens-sync")} style={FIELD_STYLE} />
      </label>
      <label style={LABEL_STYLE}>
        <span>{t("superadmin.ops.fieldDisplayName", "Tên hiển thị")}</span>
        <input type="text" disabled={!canEdit} value={form.titleInput} onChange={(e) => onPatch({ titleInput: e.target.value })} data-testid="catalog-plugin-title-input" placeholder={t("superadmin.ops.pluginTitlePlaceholder", "vd: Đồng bộ Token Figma")} style={FIELD_STYLE} />
      </label>
      <label style={LABEL_STYLE}>
        <span>{t("superadmin.ops.fieldSummary", "Mô tả ngắn")}</span>
        <input type="text" disabled={!canEdit} value={form.summaryInput} onChange={(e) => onPatch({ summaryInput: e.target.value })} data-testid="catalog-plugin-summary-input" placeholder={t("superadmin.ops.pluginSummaryPlaceholder", "Mô tả công dụng chính")} style={FIELD_STYLE} />
      </label>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "10px" }}>
        <label style={LABEL_STYLE}>
          <span>{t("superadmin.ops.fieldSourceKind", "Nguồn phân phối")}</span>
          <StyledSelect disabled={!canEdit} value={form.sourceKind} testId="catalog-source-kind-select" onChange={(v) => onSourceChange(v as PluginSourceKind)}>
            <option value="RECOMMENDED_GITHUB">{formatPluginSourceLabel("RECOMMENDED_GITHUB", t)}</option>
            <option value="DEFAULT_BUILTIN">{formatPluginSourceLabel("DEFAULT_BUILTIN", t)}</option>
          </StyledSelect>
        </label>
        <label style={LABEL_STYLE}>
          <span>{t("superadmin.ops.fieldLifecycleStatus", "Trạng thái vòng đời")}</span>
          <StyledSelect disabled={!canEdit} value={form.lifecycleStatus} testId="catalog-status-select" onChange={(v) => onPatch({ lifecycleStatus: v as PluginLifecycleStatus })}>
            <option value="PROCESSING">{t("superadmin.ops.statusProcessingOption", "Đang xử lý — Đang quét bảo mật")}</option>
            <option value="PENDING_APPROVAL">{formatPluginStatusLabel("PENDING_APPROVAL", t)}</option>
            <option value="ACTIVE">{formatPluginStatusLabel("ACTIVE", t)}</option>
            <option value="INACTIVE">{formatPluginStatusLabel("INACTIVE", t)}</option>
            <option value="DEPRECATED">{formatPluginStatusLabel("DEPRECATED", t)}</option>
            <option value="QUARANTINED">{formatPluginStatusLabel("QUARANTINED", t)}</option>
          </StyledSelect>
        </label>
      </div>
    </>
  );
}

function DrawerSourceAndAuditFields({ form, canEdit, onPatch }: { form: DrawerFormState; canEdit: boolean; onPatch: (p: Partial<DrawerFormState>) => void }) {
  const { t } = useTranslation();
  const isGithub = form.sourceKind === "RECOMMENDED_GITHUB";
  return (
    <>
      {isGithub && (
        <>
          <label style={LABEL_STYLE}>
            <span>{t("superadmin.ops.fieldRepoUrl", "Đường dẫn kho nguồn (URL HTTPS)")}</span>
            <input type="url" disabled={!canEdit} value={form.repoUrl} onChange={(e) => onPatch({ repoUrl: e.target.value })} data-testid="catalog-repo-url-input" placeholder="https://github.com/cowok-design/figma-sync" style={FIELD_STYLE} />
          </label>
          <label style={LABEL_STYLE}>
            <span>{t("superadmin.ops.fieldPinnedCommit", "Commit SHA (40 ký tự hex) hoặc thẻ phát hành cố định")}</span>
            <input type="text" disabled={!canEdit} value={form.pinnedCommitSha} onChange={(e) => onPatch({ pinnedCommitSha: e.target.value })} data-testid="catalog-commit-sha-input" placeholder={t("superadmin.ops.commitShaPlaceholder", "Commit SHA 40 ký tự hoặc thẻ semver")} style={FIELD_STYLE} />
          </label>
        </>
      )}
      <label style={LABEL_STYLE}>
        <span>{t("superadmin.ops.fieldMinAppVersion", "Phiên bản ứng dụng tối thiểu")}</span>
        <input type="text" disabled={!canEdit} value={form.minAppVersion} onChange={(e) => onPatch({ minAppVersion: e.target.value })} data-testid="catalog-min-ver-input" placeholder="0.1.0" style={FIELD_STYLE} />
      </label>
      {!isGithub && (
        <label style={{ display: "inline-flex", alignItems: "center", gap: "8px", fontSize: "12px", color: "var(--text-secondary)" }}>
          <input type="checkbox" disabled={!canEdit} checked={form.defaultEnabled} onChange={(e) => onPatch({ defaultEnabled: e.target.checked })} data-testid="catalog-default-enabled-check" />
          <span>{t("superadmin.ops.defaultEnabledLabel", "Bật mặc định cho bản cài đặt mới (không ghi đè cấu hình cục bộ)")}</span>
        </label>
      )}
      <label style={LABEL_STYLE}>
        <span>{t("superadmin.ops.fieldAuditReason", "Lý do kiểm toán bắt buộc (>= 5 ký tự)")}</span>
        <input type="text" disabled={!canEdit} value={form.reason} onChange={(e) => onPatch({ reason: e.target.value })} data-testid="catalog-reason-input" placeholder={t("superadmin.ops.catalogReasonPlaceholder", "Nhập lý do thay đổi để ghi nhận kiểm toán...")} style={FIELD_STYLE} />
      </label>
    </>
  );
}

const SUBMIT_BTN_STYLES: Record<"enabled" | "disabled", React.CSSProperties> = {
  enabled: { height: "36px", padding: "0 16px", borderRadius: "8px", border: "none", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", fontSize: "13px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer" },
  disabled: { height: "36px", padding: "0 16px", borderRadius: "8px", border: "none", backgroundColor: "var(--bg-subtle)", color: "var(--text-disabled)", fontSize: "13px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "not-allowed" },
};

function DrawerFormFooter({ isCreate, canSubmit, onClose }: { isCreate: boolean; canSubmit: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "4px" }}>
      <button type="button" onClick={onClose} style={{ height: "36px", padding: "0 14px", borderRadius: "8px", border: "1px solid var(--border-default)", backgroundColor: "transparent", color: "var(--text-secondary)", fontSize: "13px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer" }}>
        {t("common.cancel", "Hủy")}
      </button>
      <button type="submit" disabled={!canSubmit} data-testid="catalog-submit-btn" style={canSubmit ? SUBMIT_BTN_STYLES.enabled : SUBMIT_BTN_STYLES.disabled}>
        {isCreate ? t("superadmin.ops.catalogSubmitCreateBtn", "Lưu & Đưa vào bảng theo dõi") : t("superadmin.ops.catalogSubmitEditBtn", "Lưu thay đổi & Ký số")}
      </button>
    </div>
  );
}

function resolveHeaderCopy(mode: CatalogDrawerMode, plugin: CatalogPlugin | null, t: TranslateFn, locale: string): { title: string; subtitle: string } {
  if (mode === "create") {
    return { title: t("superadmin.ops.catalogCreateDrawerTitle", "Thêm Plugin vào Danh mục"), subtitle: t("superadmin.ops.catalogCreateDrawerSubtitle", "Đăng ký mới vào bảng theo dõi & ký số catalog.json") };
  }
  if (plugin) {
    return { title: resolveLocalizedTitle(plugin, locale), subtitle: `${plugin.pluginId} · ${formatPluginSourceLabel(plugin.sourceKind, t)}` };
  }
  return { title: t("superadmin.ops.catalogDetailDrawerTitle", "Chi tiết Plugin"), subtitle: "" };
}

function DrawerHeader({ mode, plugin, onClose }: { mode: CatalogDrawerMode; plugin: CatalogPlugin | null; onClose: () => void }) {
  const { t, locale } = useTranslation();
  const copy = resolveHeaderCopy(mode, plugin, t, locale);
  return (
    <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "14px 16px", borderBottom: "1px solid var(--border-default)", gap: "10px" }}>
      <div>
        <div style={{ fontSize: "15px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)" }}>{copy.title}</div>
        <div style={{ fontSize: "12px", color: "var(--text-muted)" }}>{copy.subtitle}</div>
      </div>
      <button type="button" onClick={onClose} aria-label={t("common.close", "Đóng")} data-testid="catalog-drawer-close-btn" style={{ background: "transparent", border: "1px solid var(--border-default)", borderRadius: "6px", padding: "6px", cursor: "pointer", color: "var(--text-secondary)" }}>
        <BrandIcon name="close" size={ICON_SIZE_SM} />
      </button>
    </header>
  );
}

function buildQuickStatusPayload(plugin: CatalogPlugin, nextStatus: PluginLifecycleStatus, auditReason: string): UpsertCatalogPluginRequest {
  return {
    pluginId: plugin.pluginId, sourceKind: plugin.sourceKind, lifecycleStatus: nextStatus,
    repoUrl: plugin.repoUrl ?? null, pinnedCommitSha: plugin.pinnedCommitSha ?? null,
    minAppVersion: plugin.minAppVersion, defaultEnabled: plugin.sourceKind === "DEFAULT_BUILTIN" && plugin.defaultEnabled,
    titleI18n: plugin.titleI18n, summaryI18n: plugin.summaryI18n, reason: auditReason,
  };
}

function DrawerStatusSections({ isCreate, plugin, signedManifest, canEdit, submitting, sourceKind, reason, onQuickStatus }: { isCreate: boolean; plugin: CatalogPlugin | null; signedManifest: SignedCatalogManifest | null; canEdit: boolean; submitting: boolean; sourceKind: PluginSourceKind; reason: string; onQuickStatus: (st: PluginLifecycleStatus, rsn: string) => void }) {
  const { t } = useTranslation();
  return (
    <>
      {!canEdit && (
        <div role="note" data-testid="catalog-drawer-readonly-lock" style={{ padding: "8px 10px", borderRadius: "8px", backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)", fontSize: "12px", display: "flex", alignItems: "center", gap: "6px" }}>
          <BrandIcon name="lock" size={ICON_SIZE_SM} color="var(--state-warning)" />
          <span>{t("superadmin.ops.catalogReadOnlyNotice", "Chế độ chỉ đọc (RBAC): vai trò hiện tại không có quyền chỉnh sửa danh mục plugin.")}</span>
        </div>
      )}
      {!isCreate && plugin && (
        <>
          <PluginMetadataCard plugin={plugin} signedManifest={signedManifest} />
          <QuickStatusStrip plugin={plugin} canEdit={canEdit} submitting={submitting} reason={reason} onTrigger={onQuickStatus} />
        </>
      )}
      {isCreate && sourceKind === "RECOMMENDED_GITHUB" && (
        <div data-testid="catalog-create-async-hint" style={{ padding: "10px 12px", borderRadius: "8px", backgroundColor: "var(--state-info-soft)", color: "var(--state-info)", fontSize: "12px" }}>
          {t("superadmin.ops.catalogCreateAsyncHintPrefix", "Mặc định plugin nguồn ngoài mới lưu ở trạng thái ")}
          <strong>{t("superadmin.ops.statusProcessing", "Đang xử lý")}</strong>
          {t("superadmin.ops.catalogCreateAsyncHintSuffix", " (quét bảo mật bất đồng bộ) để đưa vào bảng theo dõi ngay.")}
        </div>
      )}
    </>
  );
}

function nextStateForSource(prev: DrawerFormState, nextSource: PluginSourceKind, isCreate: boolean): DrawerFormState {
  const isGithub = nextSource === "RECOMMENDED_GITHUB";
  const defaultStatus: PluginLifecycleStatus = isGithub ? "PROCESSING" : "ACTIVE";
  return { ...prev, sourceKind: nextSource, lifecycleStatus: isCreate ? defaultStatus : prev.lifecycleStatus, defaultEnabled: !isGithub && prev.defaultEnabled };
}

export const PluginCatalogDetailDrawer: React.FC<PluginCatalogDetailDrawerProps> = (props) => {
  const { open, mode, plugin, signedManifest, operatorRole, submitting, onClose, onSubmitPlugin } = props;
  const { t, locale } = useTranslation();
  const canEdit = operatorRole === "SUPER_ADMIN" || operatorRole === "SKILL_CURATOR";
  const [form, setForm] = useState<DrawerFormState>(() => createInitialFormState(mode, plugin, locale));

  useEffect(() => {
    if (open) setForm(createInitialFormState(mode, plugin, locale));
  }, [open, mode, plugin, locale]);

  if (!open) return null;

  const isCreate = mode === "create";
  const canSubmit = canEdit && !submitting && isDrawerFormValid(form);
  const patchForm = (patch: Partial<DrawerFormState>) => setForm((prev) => ({ ...prev, ...patch }));
  const handleQuick = (st: PluginLifecycleStatus, rsn: string) => { if (plugin) void onSubmitPlugin(buildQuickStatusPayload(plugin, st, rsn)); };
  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canSubmit || !(await onSubmitPlugin(buildUpsertPayload(form, locale)))) return;
    if (isCreate) { onClose(); return; }
    patchForm({ reason: "" });
  };

  return (
    <>
      <div style={BACKDROP_STYLE} onClick={onClose} aria-hidden="true" />
      <aside role="dialog" aria-modal="true" aria-label={isCreate ? t("superadmin.ops.catalogAddBtn", "Thêm Plugin") : t("superadmin.ops.catalogDetailDrawerTitle", "Chi tiết Plugin")} data-ff-id="superadmin-catalog-right-drawer" data-testid="catalog-right-drawer" style={DRAWER_STYLE}>
        <DrawerHeader mode={mode} plugin={plugin} onClose={onClose} />
        <div style={{ flex: 1, overflowY: "auto", padding: "16px", display: "flex", flexDirection: "column", gap: "14px" }}>
          <DrawerStatusSections isCreate={isCreate} plugin={plugin} signedManifest={signedManifest} canEdit={canEdit} submitting={submitting} sourceKind={form.sourceKind} reason={form.reason} onQuickStatus={handleQuick} />
          <form onSubmit={(ev) => void handleSubmit(ev)} data-testid="catalog-drawer-form" style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
            <strong style={{ fontSize: "13px", color: "var(--text-primary)" }}>{isCreate ? t("superadmin.ops.catalogCreateFormHeading", "Thông tin đăng ký Plugin") : t("superadmin.ops.catalogEditFormHeading", "Chỉnh sửa thông tin Plugin")}</strong>
            <DrawerIdentityFields form={form} isCreate={isCreate} canEdit={canEdit} onPatch={patchForm} onSourceChange={(s) => setForm((p) => nextStateForSource(p, s, isCreate))} />
            <DrawerSourceAndAuditFields form={form} canEdit={canEdit} onPatch={patchForm} />
            <DrawerFormFooter isCreate={isCreate} canSubmit={canSubmit} onClose={onClose} />
          </form>
        </div>
      </aside>
    </>
  );
};
