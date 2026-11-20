// @story 06.3
// @implements AC-01, AC-03 (Offline CSP-B 2D Live Chart Editor & E2 Recovery)
import React, { useMemo, useRef, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import {
  ChartKind2DSchema,
  type ChartKind2D,
  type ChartSpecV12,
  type ValidateChartResponse,
} from "../../../lib/validations/presentation-chart-specs";
import {
  buildChartAriaLabel,
  parseChartSpecSafely,
  renderOffline2DChartSvg,
} from "../utils/offline-chart-2d-renderer";

const CHART_KINDS: readonly ChartKind2D[] = ChartKind2DSchema.options;
const FALLBACK_SPEC: ChartSpecV12 = {
  kind: "bar",
  title: "Chart",
  series: [{ name: "Series 1", points: [{ label: "A", value: 50 }, { label: "B", value: 50 }] }],
};

export interface OfflineChart2DRendererProps {
  ffId: string;
  rawJson: string;
  disabled?: boolean;
  externalError?: { code: string; message: string } | null;
  onChangeChartsJson: (nextJson: string) => void;
  onValidateChartSpec?: (ffId: string, spec: ChartSpecV12) => Promise<ValidateChartResponse | null>;
  onRenderChartStage?: (ffId: string, spec: ChartSpecV12, ariaLabel: string) => void;
  onRetryWithCopilot?: (ffId: string, errors?: string[]) => void;
}

function buildInitialDraftSpec(rawJson: string): { spec: ChartSpecV12; validSpec: ChartSpecV12 | null; error: { code: string; message: string } | null } {
  const parsed = parseChartSpecSafely(rawJson);
  if (parsed.ok) return { spec: parsed.spec, validSpec: parsed.spec, error: null };
  try {
    const rawObj = JSON.parse(rawJson);
    if (rawObj && typeof rawObj === "object" && Array.isArray((rawObj as ChartSpecV12).series)) {
      return { spec: rawObj as ChartSpecV12, validSpec: null, error: { code: parsed.code, message: parsed.message } };
    }
  } catch {
    // Keep fallback editable table spec while surfacing syntax error.
  }
  return { spec: FALLBACK_SPEC, validSpec: null, error: rawJson.trim() && rawJson.trim() !== "[]" ? { code: parsed.code, message: parsed.message } : null };
}

function ChartValidationRecoveryBanner(props: {
  ffId: string;
  error: { code: string; message: string };
  isHelpOpen: boolean;
  disabled: boolean;
  onToggleHelp: () => void;
  onEditInTable: () => void;
  onRetryWithCopilot?: (ffId: string, errors?: string[]) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="ff-chart2d-recovery-banner" data-testid="chart-validation-retry" role="alert">
      <div className="ff-chart2d-recovery-header">
        <span className="ff-chart2d-recovery-title">
          <BrandIcon name="shield-alert" size={12} color="var(--state-danger)" />
          <span>{props.error.code}</span>
        </span>
        <div className="ff-chart2d-recovery-badges">
          <span className="ff-chart2d-intact-badge" data-testid="chart-stage-intact-badge">
            {t("presentation.charts.liveStageIntact", "Live Stage Intact")}
          </span>
          <button type="button" className="ff-chart2d-help-btn" aria-expanded={props.isHelpOpen} aria-label={t("presentation.charts.zasfHelpAria", "Zero-Sum Chart Rules Help")} onClick={props.onToggleHelp}>
            <BrandIcon name="info" size={12} color="currentColor" />
          </button>
        </div>
      </div>
      <div className="ff-chart2d-recovery-msg">{props.error.message}</div>
      {props.isHelpOpen && (
        <div className="ff-chart2d-help-popover" role="note" data-testid="chart-zasf-help-popover">
          {t("presentation.charts.zasfHelpBody", "Proportional/Funnel charts require non-negative values summing to 100.0 +/- 0.05; cartesian charts accept [-1e6, 1e6].")}
        </div>
      )}
      <div className="ff-chart2d-recovery-actions">
        <button type="button" className="cowok-btn ff-cms-mini-btn" data-testid="chart-edit-in-table-btn" disabled={props.disabled} onClick={props.onEditInTable}>
          <BrandIcon name="edit" size={11} color="currentColor" />
          <span>{t("presentation.charts.editInTable", "Edit in Table")}</span>
        </button>
        {props.onRetryWithCopilot && (
          <button type="button" className="cowok-btn cowok-btn-primary ff-cms-mini-btn" data-testid="chart-retry-copilot-btn" disabled={props.disabled} onClick={() => props.onRetryWithCopilot?.(props.ffId, [props.error.message])}>
            <BrandIcon name="sparkles" size={11} color="currentColor" />
            <span>{t("presentation.charts.retryWithCopilot", "Retry with Copilot")}</span>
          </button>
        )}
      </div>
    </div>
  );
}

function ChartDataPointRow(props: {
  pt: ChartSpecV12["series"][number]["points"][number];
  idx: number;
  disabled: boolean;
  canRemove: boolean;
  firstInputRef: React.RefObject<HTMLInputElement | null>;
  onUpdatePoint: (idx: number, patch: { label?: string; value?: number }) => void;
  onRemovePoint: (idx: number) => void;
}) {
  const { t } = useTranslation();
  return (
    <tr>
      <td>
        <input type="text" className="ff-cms-input ff-chart2d-cell-input" value={props.pt.label} disabled={props.disabled} onChange={(e) => props.onUpdatePoint(props.idx, { label: e.target.value })} />
      </td>
      <td>
        <input ref={props.idx === 0 ? props.firstInputRef : undefined} type="number" step="any" className="ff-cms-input ff-chart2d-cell-input" data-testid={`chart-point-value-${props.idx}`} value={Number.isFinite(props.pt.value) ? props.pt.value : 0} disabled={props.disabled} onChange={(e) => props.onUpdatePoint(props.idx, { value: Number(e.target.value) })} />
      </td>
      <td>
        <button type="button" className="ff-cms-mini-btn" aria-label={t("presentation.charts.removeRow", "Remove row")} disabled={props.disabled || !props.canRemove} onClick={() => props.onRemovePoint(props.idx)}>
          <BrandIcon name="close" size={11} color="currentColor" />
        </button>
      </td>
    </tr>
  );
}

function ChartDataPointsTable(props: {
  spec: ChartSpecV12;
  disabled: boolean;
  firstInputRef: React.RefObject<HTMLInputElement | null>;
  onUpdatePoint: (idx: number, patch: { label?: string; value?: number }) => void;
  onAddPoint: () => void;
  onRemovePoint: (idx: number) => void;
}) {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const points = props.spec.series[0]?.points ?? [];
  const total = points.reduce((acc, pt) => acc + (Number.isFinite(pt.value) ? pt.value : 0), 0);
  return (
    <div className="ff-chart2d-table-wrap" data-pattern="DataTableView-5.21">
      <div className="overflow-x-auto">
        <table className="ff-chart2d-table" aria-label={t("presentation.charts.dataTableAria", "2D Chart Data Table")}>
          <thead>
            <tr><th>{t("presentation.charts.labelCol", "Label")}</th><th>{t("presentation.charts.valueCol", "Value")}</th><th aria-label={t("presentation.charts.actionsCol", "Actions")} /></tr>
          </thead>
          <tbody>
            {points.map((pt, idx) => <ChartDataPointRow key={idx} pt={pt} idx={idx} disabled={props.disabled} canRemove={points.length > 1} firstInputRef={props.firstInputRef} onUpdatePoint={props.onUpdatePoint} onRemovePoint={props.onRemovePoint} />)}
          </tbody>
        </table>
      </div>
      <div className="ff-chart2d-table-footer">
        <span className="ff-chart2d-sum-chip">{t("presentation.charts.sumLabel", "Sum")}: {formatNumber(Math.round(total * 100) / 100)}</span>
        <button type="button" className="cowok-btn ff-cms-mini-btn" disabled={props.disabled} onClick={props.onAddPoint}><BrandIcon name="plus" size={11} color="currentColor" /><span>{t("presentation.charts.addPoint", "Add Point")}</span></button>
      </div>
    </div>
  );
}

function replaceFirstSeriesPoints(
  spec: ChartSpecV12,
  updater: (pts: ChartSpecV12["series"][number]["points"]) => ChartSpecV12["series"][number]["points"]
): ChartSpecV12 {
  const first = spec.series[0];
  const nextPts = updater(first?.points ?? []);
  return { ...spec, series: [{ name: first?.name || "Series 1", points: nextPts }, ...spec.series.slice(1)] };
}

function buildChartSvgSafe(spec: ChartSpecV12) {
  if (typeof document === "undefined") return null;
  try {
    return renderOffline2DChartSvg(document, spec);
  } catch {
    return null;
  }
}

function useSyncedChartDraftState(props: OfflineChart2DRendererProps) {
  const initial = useMemo(() => buildInitialDraftSpec(props.rawJson), [props.rawJson]);
  const [prevPropJson, setPrevPropJson] = useState(props.rawJson);
  const [draftSpec, setDraftSpec] = useState<ChartSpecV12>(initial.spec);
  const [lastValidSpec, setLastValidSpec] = useState<ChartSpecV12>(initial.validSpec ?? FALLBACK_SPEC);
  const [localError, setLocalError] = useState<{ code: string; message: string } | null>(initial.error);
  if (props.rawJson !== prevPropJson) {
    setPrevPropJson(props.rawJson);
    const nextInit = buildInitialDraftSpec(props.rawJson);
    setDraftSpec(nextInit.spec);
    if (nextInit.validSpec) setLastValidSpec(nextInit.validSpec);
    setLocalError(nextInit.error);
  }
  const applyDraftMutation = (nextSpec: ChartSpecV12) => {
    setDraftSpec(nextSpec);
    const nextJson = JSON.stringify(nextSpec);
    const parsed = parseChartSpecSafely(nextJson);
    if (!parsed.ok) { setLocalError({ code: parsed.code, message: parsed.message }); return; }
    setLocalError(null);
    setLastValidSpec(parsed.spec);
    props.onChangeChartsJson(nextJson);
    props.onRenderChartStage?.(props.ffId, parsed.spec, buildChartAriaLabel(parsed.spec));
    void props.onValidateChartSpec?.(props.ffId, parsed.spec);
  };
  return { draftSpec, lastValidSpec, localError, applyDraftMutation };
}

export const OfflineChart2DRenderer: React.FC<OfflineChart2DRendererProps> = (props) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const { draftSpec, lastValidSpec, localError, applyDraftMutation } = useSyncedChartDraftState(props);
  const [isHelpOpen, setIsHelpOpen] = useState(false);
  const firstInputRef = useRef<HTMLInputElement | null>(null);
  const renderedSvg = useMemo(() => buildChartSvgSafe(lastValidSpec), [lastValidSpec]);
  const activeError = localError ?? props.externalError ?? null;
  const firstPoints = draftSpec.series[0]?.points ?? [];
  return (
    <div className="ff-chart2d-editor" data-testid={`offline-chart-editor-${props.ffId}`}>
      <div className="ff-chart2d-toolbar">
        <span className="ff-chart2d-kind-badge"><BrandIcon name="trending-up" size={12} color="var(--human-accent)" /><span>{t("presentation.charts.offlineSvgLabel", "2D SVG Chart")}</span></span>
        <select className="ff-cms-input ff-chart2d-kind-select" aria-label={t("presentation.charts.kindSelectAria", "Chart Kind")} value={draftSpec.kind} disabled={Boolean(props.disabled)} onChange={(e) => applyDraftMutation({ ...draftSpec, kind: e.target.value as ChartKind2D })}>
          {CHART_KINDS.map((k) => <option key={k} value={k}>{t(`presentation.charts.kinds.${k}`, k)}</option>)}
        </select>
      </div>
      {activeError && <ChartValidationRecoveryBanner ffId={props.ffId} error={activeError} isHelpOpen={isHelpOpen} disabled={Boolean(props.disabled)} onToggleHelp={() => setIsHelpOpen((v) => !v)} onEditInTable={() => firstInputRef.current?.focus()} onRetryWithCopilot={props.onRetryWithCopilot} />}
      <div className="ff-chart2d-preview-box" data-testid="chart-last-valid-preview" ref={(el) => { if (el && renderedSvg && el.firstChild !== renderedSvg.svg) el.replaceChildren(renderedSvg.svg); }} />
      <ChartDataPointsTable spec={draftSpec} disabled={Boolean(props.disabled)} firstInputRef={firstInputRef} onUpdatePoint={(idx, patch) => applyDraftMutation(replaceFirstSeriesPoints(draftSpec, (pts) => pts.map((pt, i) => (i === idx ? { ...pt, ...patch } : pt))))} onAddPoint={() => applyDraftMutation(replaceFirstSeriesPoints(draftSpec, (pts) => [...pts, { label: `${t("presentation.charts.defaultItemPrefix", "Item")} ${formatNumber(pts.length + 1)}`, value: 0 }]))} onRemovePoint={(idx) => { if (firstPoints.length > 1) applyDraftMutation(replaceFirstSeriesPoints(draftSpec, (pts) => pts.filter((_, i) => i !== idx))); }} />
    </div>
  );
};
