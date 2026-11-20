// @story 06.2
// @implements AC-01, AC-04, AC-06, AC-07, AC-08 (Responsive Line-1 Toolbar CMS Undo / Redo / Icon-Only Save Controls)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CmsSyncState } from "../hooks/useCmsSlideEditorController";

const ICON_SIZE_SM = 12;
const ICON_STROKE = 1.85;

export type CmsToolbarDensity = "wide" | "medium" | "narrow" | "min";

export interface HtmlToolbarCmsActions {
  revision: number;
  syncState: CmsSyncState;
  hasPendingSave: boolean;
  canUndo: boolean;
  canRedo: boolean;
  isReadOnly?: boolean;
  onUndo: () => void;
  onRedo: () => void;
  onSave: () => void;
}

const SYNC_STATUS_COPY: Record<CmsSyncState, [string, string]> = {
  SYNCED: ["presentation.cms.statusSynced", "Đã đồng bộ"],
  COMMITTING: ["presentation.cms.statusSaving", "Đang lưu..."],
  FLUSH_FAILED_RETRY: ["presentation.cms.statusRetry", "Chờ thử lại"],
  DIRTY_FAST_PATCHED: ["presentation.cms.statusLive", "Đang sửa"],
};

export function resolveCmsToolbarDensity(toolbarWidth: number): CmsToolbarDensity {
  if (toolbarWidth <= 0 || toolbarWidth >= 740) return "wide";
  if (toolbarWidth >= 520) return "medium";
  if (toolbarWidth >= 380) return "narrow";
  return "min";
}

function buildRevisionBadgeText(
  density: CmsToolbarDensity,
  revPrefix: string,
  revNum: string,
  statusLabel: string
): string {
  if (density === "wide") return `${revPrefix} ${revNum} · ${statusLabel}`;
  return `${revPrefix} ${revNum}`;
}

const CmsIconButton: React.FC<{
  testId: string;
  label: string;
  title: string;
  icon: "undo" | "redo" | "save";
  disabled: boolean;
  isPrimary?: boolean;
  extraClass?: string;
  dataDirty?: "true" | "false";
  dataState?: string;
  showDirtyDot?: boolean;
  onClick: () => void;
}> = (b) => (
  <button
    type="button" data-testid={b.testId} aria-label={b.label} title={b.title} disabled={b.disabled} onClick={b.onClick}
    data-dirty={b.dataDirty} data-state={b.dataState}
    className={`cowok-btn ${b.isPrimary ? "cowok-btn-primary" : "cowok-btn-secondary"} ff-cms-toolbar-icon-btn ${b.extraClass || ""}`}
  >
    <BrandIcon name={b.icon} size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="currentColor" />
    {b.showDirtyDot && <span className="ff-cms-save-dirty-dot" aria-hidden="true" />}
  </button>
);

export const HtmlPreviewCmsActions: React.FC<{
  actions: HtmlToolbarCmsActions;
  density: CmsToolbarDensity;
}> = ({ actions, density }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const [statusKey, statusFallback] = SYNC_STATUS_COPY[actions.syncState];
  const statusLabel = t(statusKey, statusFallback);
  const revPrefix = t("presentation.cms.revisionPrefix", "pb");
  const revNum = formatNumber(actions.revision);
  const fullStatusSummary = `${revPrefix} ${revNum} · ${statusLabel}`;
  const saveLabel = actions.syncState === "COMMITTING" ? t("presentation.cms.statusSaving", "Đang lưu...") : t("presentation.cms.save", "Lưu");
  const saveTitle = `${saveLabel} · ${t("presentation.cms.saveTooltip", "Lưu các thay đổi CMS đang chờ vào SQLite WAL")} (${fullStatusSummary})`;
  const undoLabel = t("presentation.cms.undo", "Hoàn tác");
  const redoLabel = t("presentation.cms.redo", "Làm lại");
  const isDirtyOrRetry = actions.hasPendingSave || actions.syncState === "FLUSH_FAILED_RETRY";
  const isSaveDisabled = Boolean(actions.isReadOnly) || actions.syncState === "COMMITTING" || !isDirtyOrRetry;

  return (
    <div className="ff-cms-toolbar-group" data-density={density} data-testid="toolbar-cms-actions">
      {(density === "wide" || density === "medium") && (
        <span className="ff-metric-line ff-cms-toolbar-rev" data-testid="cms-telemetry-rev" data-state={actions.syncState} title={fullStatusSummary}>
          {buildRevisionBadgeText(density, revPrefix, revNum, statusLabel)}
        </span>
      )}
      <CmsIconButton testId="cms-undo-btn" icon="undo" label={undoLabel} title={`${undoLabel} (⌘Z)`} disabled={!actions.canUndo || Boolean(actions.isReadOnly)} onClick={actions.onUndo} />
      <CmsIconButton testId="cms-redo-btn" icon="redo" label={redoLabel} title={`${redoLabel} (⇧⌘Z)`} disabled={!actions.canRedo || Boolean(actions.isReadOnly)} onClick={actions.onRedo} />
      <CmsIconButton testId="cms-save-btn" icon="save" label={saveTitle} title={saveTitle} disabled={isSaveDisabled} isPrimary={isDirtyOrRetry} extraClass="ff-cms-toolbar-save-btn" dataDirty={isDirtyOrRetry ? "true" : "false"} dataState={actions.syncState} showDirtyDot={isDirtyOrRetry} onClick={actions.onSave} />
    </div>
  );
};
