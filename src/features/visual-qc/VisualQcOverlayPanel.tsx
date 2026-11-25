// @story 07.1
// @implements AC-01, AC-03, AC-04 (Visual QC Coordinate Overlay & Viewport Audit Bar)
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type {
  QcAutoFixResponseData,
  QcIssue,
  QcRunResponseData,
  QcVerdict,
} from "../../lib/validations/visual-qc";
import { LintFixActionButton } from "./LintFixActionButton";

const ICON_SM = 13;
const ICON_MD = 16;
const STROKE_MONO = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const VP_DESKTOP = 1440;
const VP_LAPTOP = 1024;
const VP_TABLET = 768;
const VP_MOBILE = 390;
const VP_SLIDE = 1920;
const SLIDE_HEIGHT_PX = 1080;
const DEFAULT_VIEWPORT_HEIGHT_PX = 900;
const PERCENT_SCALE = 100;
const MAX_OVERLAY_PCT = 92;
const MIN_BOX_W_PX = 12;
const MAX_BOX_W_PX = 100;
const MIN_BOX_H_PX = 22;
const MAX_BOX_H_PX = 80;

function toClampedPercent(coordPx: number, dimensionPx: number): number {
  if (dimensionPx <= 0) return 0;
  const pct = (coordPx / dimensionPx) * PERCENT_SCALE;
  return Number(Math.max(0, Math.min(pct, MAX_OVERLAY_PCT)).toFixed(2));
}

const VIEWPORT_PRESETS: ReadonlyArray<{ width: number; label: string }> = [
  { width: VP_DESKTOP, label: "1440" },
  { width: VP_LAPTOP, label: "1024" },
  { width: VP_TABLET, label: "768" },
  { width: VP_MOBILE, label: "390" },
  { width: VP_SLIDE, label: "16:9" },
];

const VERDICT_PRESENTATION: Record<
  QcVerdict,
  { color: string; bg: string; labelKey: string; labelDefault: string }
> = {
  PASS: {
    color: "var(--state-success)",
    bg: "var(--state-success-soft)",
    labelKey: "visualQc.verdictPassFriendly",
    labelDefault: "Đạt chuẩn hiển thị",
  },
  FAIL: {
    color: "var(--state-danger)",
    bg: "var(--state-danger-soft)",
    labelKey: "visualQc.verdictFailFriendly",
    labelDefault: "Cần sửa trước khi xuất",
  },
  UNVERIFIED_NO_BROWSER: {
    color: "var(--state-warning)",
    bg: "var(--state-warning-soft)",
    labelKey: "visualQc.verdictUnverifiedFriendly",
    labelDefault: "Chưa kiểm tra trình duyệt",
  },
};

export interface VisualQcOverlayPanelProps {
  runResult: QcRunResponseData | null;
  activeViewport: number;
  selectedIssueId: string | null;
  isRunning: boolean;
  isFixing: boolean;
  agentAttemptsUsed: number;
  isAgentFixLocked: boolean;
  canRollback: boolean;
  lastFixResult: QcAutoFixResponseData | null;
  error: string | null;
  onSelectViewport: (width: number) => void;
  onSelectIssue: (id: string | null) => void;
  onRunQc: () => void;
  onDeterministicFix: () => void;
  onAgentFix: () => void;
  onRollback: () => void;
  onOpenReadiness: () => void;
}

const VerdictBadge: React.FC<{ runResult: QcRunResponseData | null }> = ({ runResult }) => {
  const { t } = useTranslation();
  if (!runResult) return null;
  const isPass = runResult.verdict === "PASS";
  const meta = VERDICT_PRESENTATION[runResult.verdict];
  return (
    <span
      data-testid="qc-verdict-badge"
      data-verdict={runResult.verdict}
      style={{
        display: "inline-flex", alignItems: "center", gap: "4px", padding: "2px 8px",
        borderRadius: "var(--radius-xs)", border: `1px solid ${meta.color}`,
        backgroundColor: meta.bg, color: meta.color, fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD,
      }}
    >
      <BrandIcon name={isPass ? "check-circle" : "alert"} size={ICON_SM} strokeWidth={STROKE_MONO} />
      <span>{t(meta.labelKey, meta.labelDefault)}</span>
    </span>
  );
};

const OverlayHeaderBar: React.FC<{
  runResult: QcRunResponseData | null;
  isRunning: boolean;
  onRunQc: () => void;
  onOpenReadiness: () => void;
}> = ({ runResult, isRunning, onRunQc, onOpenReadiness }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <BrandIcon name="eye" size={ICON_MD} strokeWidth={STROKE_MONO} />
        <strong style={{ fontSize: "13px" }}>{t("visualQc.overlayTitle", "Kiểm tra hiển thị")}</strong>
        <VerdictBadge runResult={runResult} />
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
        <button
          type="button"
          data-testid="qc-run-button"
          disabled={isRunning}
          onClick={onRunQc}
          style={{
            height: "28px", padding: "0 10px", borderRadius: "var(--radius-xs)",
            border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)",
            color: "var(--text-primary)", fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD,
            cursor: isRunning ? "wait" : "pointer",
          }}
        >
          {isRunning ? t("visualQc.running", "Đang kiểm tra...") : t("visualQc.runAudit", "Kiểm tra ngay")}
        </button>
        <button
          type="button"
          data-testid="open-readiness-modal-button"
          onClick={onOpenReadiness}
          style={{
            height: "28px", padding: "0 10px", borderRadius: "var(--radius-xs)",
            border: "1px solid var(--border-default)", backgroundColor: "var(--bg-subtle)",
            color: "var(--text-primary)", fontSize: "11.5px", cursor: "pointer",
          }}
        >
          {t("visualQc.openReadiness", "Sẵn sàng")}
        </button>
      </div>
    </div>
  );
};

const ViewportAuditBar: React.FC<{
  activeViewport: number;
  onSelectViewport: (width: number) => void;
}> = ({ activeViewport, onSelectViewport }) => {
  const { t } = useTranslation();
  return (
    <div role="tablist" aria-label={t("visualQc.viewportBarLabel", "Chuyển đổi khung hình kiểm tra")} style={{ display: "flex", gap: "4px", flexWrap: "wrap" }}>
      {VIEWPORT_PRESETS.map((vp) => {
        const active = activeViewport === vp.width;
        return (
          <button
            key={vp.width}
            type="button"
            role="tab"
            aria-selected={active}
            data-testid={`qc-viewport-${vp.width}`}
            onClick={() => onSelectViewport(vp.width)}
            style={{
              height: "26px", padding: "0 8px", borderRadius: "var(--radius-xs)",
              border: active ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
              backgroundColor: active ? "var(--action-primary)" : "var(--bg-surface)",
              color: active ? "var(--action-primary-fg)" : "var(--text-secondary)",
              fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer",
            }}
          >
            {vp.label}
          </button>
        );
      })}
    </div>
  );
};

const BoundingBoxMarker: React.FC<{
  issue: QcIssue;
  activeViewport: number;
  viewportHeight: number;
  isSelected: boolean;
  onSelectIssue: (id: string | null) => void;
}> = ({ issue, activeViewport, viewportHeight, isSelected, onSelectIssue }) => {
  const { t } = useTranslation();
  const box = issue.bbox!;
  const isCritical = issue.severity === "P0";
  const borderColor = isCritical ? "var(--state-danger)" : "var(--state-warning)";
  const selectedBg = isCritical ? "var(--state-danger-soft)" : "var(--state-warning-soft)";
  const severityLabel = isCritical
    ? t("visualQc.severityP0Label", "Lỗi cần sửa")
    : t("visualQc.severityP1Label", "Gợi ý cải thiện");
  return (
    <button
      type="button"
      data-testid={`qc-bbox-${issue.id}`}
      data-rule-id={issue.ruleId}
      title={`${issue.severity} · ${issue.ruleId}: ${issue.message}`}
      onClick={() => onSelectIssue(isSelected ? null : issue.id)}
      style={{
        position: "absolute",
        left: `${toClampedPercent(box.x, activeViewport)}%`,
        top: `${toClampedPercent(box.y, viewportHeight)}%`,
        width: `${Math.max(MIN_BOX_W_PX, Math.min(box.width, MAX_BOX_W_PX))}px`,
        height: `${Math.max(MIN_BOX_H_PX, Math.min(box.height, MAX_BOX_H_PX))}px`,
        border: `2px solid ${borderColor}`,
        backgroundColor: isSelected ? selectedBg : "transparent",
        color: borderColor, fontSize: "10px", fontWeight: FONT_WEIGHT_SEMIBOLD,
        cursor: "pointer", padding: "2px 4px", textAlign: "left",
        overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
      }}
    >
      {severityLabel}
    </button>
  );
};

const BoundingBoxLayer: React.FC<{
  issues: QcIssue[];
  activeViewport: number;
  selectedIssueId: string | null;
  onSelectIssue: (id: string | null) => void;
}> = ({ issues, activeViewport, selectedIssueId, onSelectIssue }) => {
  const boxed = issues.filter((i) => i.bbox !== null && i.bbox !== undefined);
  if (boxed.length === 0) return null;
  const viewportHeight = activeViewport === VP_SLIDE ? SLIDE_HEIGHT_PX : DEFAULT_VIEWPORT_HEIGHT_PX;
  return (
    <div
      data-testid="qc-bounding-box-layer"
      style={{
        position: "relative", minHeight: "120px", borderRadius: "var(--radius-xs)",
        border: "1px dashed var(--border-default)", backgroundColor: "var(--bg-surface)", overflow: "hidden",
      }}
    >
      {boxed.map((issue) => (
        <BoundingBoxMarker
          key={issue.id}
          issue={issue}
          activeViewport={activeViewport}
          viewportHeight={viewportHeight}
          isSelected={selectedIssueId === issue.id}
          onSelectIssue={onSelectIssue}
        />
      ))}
    </div>
  );
};

const QcErrorBanner: React.FC<{ error: string | null }> = ({ error }) => {
  if (!error) return null;
  return (
    <div
      role="alert"
      data-testid="qc-error-banner"
      style={{
        padding: "8px 10px", borderRadius: "var(--radius-xs)",
        border: "1px solid var(--state-danger)", backgroundColor: "var(--state-danger-soft)",
        color: "var(--state-danger)", fontSize: "11.5px",
      }}
    >
      {error}
    </div>
  );
};

export const VisualQcOverlayPanel: React.FC<VisualQcOverlayPanelProps> = (props) => {
  const { t } = useTranslation();
  const issues = (props.runResult?.issues ?? []).filter(
    (issue) => issue.viewportWidth === props.activeViewport || issue.category === "flow"
  );
  const deterministicCount = issues.filter((i) => i.deterministicFixable).length;
  return (
    <section
      data-ff-id="visual-qc-overlay"
      data-testid="visual-qc-overlay"
      aria-label={t("visualQc.overlayAriaLabel", "Lớp phủ kiểm tra hiển thị")}
      style={{
        display: "flex", flexDirection: "column", gap: "10px", padding: "12px",
        borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)",
        backgroundColor: "var(--bg-surface)", color: "var(--text-primary)",
      }}
    >
      <OverlayHeaderBar runResult={props.runResult} isRunning={props.isRunning} onRunQc={props.onRunQc} onOpenReadiness={props.onOpenReadiness} />
      <ViewportAuditBar activeViewport={props.activeViewport} onSelectViewport={props.onSelectViewport} />
      <QcErrorBanner error={props.error} />
      <BoundingBoxLayer
        issues={issues}
        activeViewport={props.activeViewport}
        selectedIssueId={props.selectedIssueId}
        onSelectIssue={props.onSelectIssue}
      />
      <LintFixActionButton
        deterministicCount={deterministicCount}
        totalIssueCount={issues.length}
        isFixing={props.isFixing}
        agentAttemptsUsed={props.agentAttemptsUsed}
        isAgentFixLocked={props.isAgentFixLocked}
        canRollback={props.canRollback}
        lastFixResult={props.lastFixResult}
        onDeterministicFix={props.onDeterministicFix}
        onAgentFix={props.onAgentFix}
        onRollback={props.onRollback}
      />
    </section>
  );
};
