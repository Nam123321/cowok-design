// @story 06.5
// @implements AC-03, AC-04 (Interactive 10-Shape SVG Charts with Spotlight Grey-Out, Callout Arrows, Draggable Target & What-If Handle)
import React, { useRef, useState } from "react";
import { useTranslation } from "../../../../hooks/useTranslation";
import { formatMetricValue } from "./studioDataUtils";
import {
  HeatmapMatrixView,
  MovementBarsSvg,
  SegmentedBarSvg,
  SparklineRowsView,
  SvgCalloutArrowsOverlay,
  TickBar2026Svg,
} from "./StudioExtraCharts";
import { FONT_WEIGHT_BOLD, FONT_WEIGHT_SEMIBOLD } from "./studioStyles";
import {
  getPaletteColors,
  type StudioCallout,
  type StudioChartType,
  type StudioDataRow,
  type StudioLegendPos,
  type StudioPaletteId,
  type StudioUnit,
} from "./types";

export interface StudioSvgChartsProps {
  chartType: StudioChartType;
  paletteId: StudioPaletteId;
  rows: readonly StudioDataRow[];
  unit: StudioUnit;
  xAxisLabel: string;
  yAxisLabel: string;
  showComparison: boolean;
  showTargetLine: boolean;
  targetValue: number;
  legendPos: StudioLegendPos;
  highlightedRowId: string | null;
  spotlightRowId: string | null;
  callouts: readonly StudioCallout[];
  onToggleRowHighlight: (rowId: string) => void;
  onPreviewTarget: (nextTarget: number) => void;
  onCommitTarget: (baseTarget: number, nextTarget: number) => void;
  onApplyWhatIf?: (rowId: string, deltaPct: number) => void;
}

const SVG_W = 540;
const SVG_H = 220;
const PAD_L = 46;
const PAD_R = 20;
const PAD_T = 26;
const PLOT_W = 474;
const PLOT_H = 160;
const SCALE_HEADROOM = 1.18;
const MIN_SCALE = 10;
const PCT_SCALE = 100;
const TARGET_STEP = 50;
const DIM_OPACITY = 0.38;
const PIE_CX = 150;
const PIE_CY = 110;
const PIE_R = 76;
const FULL_DEG = 360;
const MAX_ARC_DEG = 359.9;
const HALF_DEG = 180;
const RIGHT_DEG = 90;
const LINE_WIDTH = 2.5;
const TARGET_STROKE = 1.8;
const TAG_W = 86;
const TAG_HALF_W = 43;
const TAG_H = 20;
const TAG_HALF_H = 10;
const TAG_TEXT_DY = 4;
const TAG_RX = 4;
const BAR_MAX_W = 34;
const BAR_STEP_RATIO = 0.42;
const PREV_BAR_OFFSET = 0.85;
const PREV_BAR_RATIO = 0.72;
const CUR_BAR_OFFSET = 0.05;
const BAR_RX = 3;
const DOT_R = 5;
const DOT_HL_R = 7;
const LABEL_Y_OFFSET = 12;
const LABEL_CHARS = 11;
const AXIS_FONT_SIZE = 10.5;
const BULLET_PAD_RATIO = 0.25;
const BULLET_MAX_H = 20;
const BULLET_RATIO = 0.5;
const BULLET_LABEL_DX = 6;
const BULLET_LABEL_Y = 0.72;
const BULLET_CHARS = 7;
const BULLET_INSET_SM = 2;
const BULLET_INSET_MD = 4;
const BULLET_INSET_LG = 8;
const PIE_STROKE = 1.5;
const PIE_HL_STROKE = 3;
const Z_TOOLTIP = 10;
const WHATIF_DRAG_STEP = 10;
const TARGET_TAG_STROKE = 1.4;
const HANDLE_MIN_Y = 4;
const HANDLE_HALF_W = 10;
const HANDLE_W = 20;
const HANDLE_H = 6;
const HANDLE_RX = 3;
const HANDLE_STROKE = 1.2;

const SVG = {
  w: SVG_W, h: SVG_H, padL: PAD_L, padR: PAD_R, padT: PAD_T,
  plotW: PLOT_W, plotH: PLOT_H, scaleHeadroom: SCALE_HEADROOM, minScale: MIN_SCALE,
  pctScale: PCT_SCALE, targetStep: TARGET_STEP, dimOpacity: DIM_OPACITY,
  pieCx: PIE_CX, pieCy: PIE_CY, pieR: PIE_R, fullDeg: FULL_DEG, maxArcDeg: MAX_ARC_DEG, halfDeg: HALF_DEG, rightDeg: RIGHT_DEG,
  lineWidth: LINE_WIDTH, targetStroke: TARGET_STROKE, tagW: TAG_W, tagHalfW: TAG_HALF_W, tagH: TAG_H, tagHalfH: TAG_HALF_H, tagTextDy: TAG_TEXT_DY, tagRx: TAG_RX,
  barMaxW: BAR_MAX_W, barStepRatio: BAR_STEP_RATIO, prevBarOffset: PREV_BAR_OFFSET, prevBarRatio: PREV_BAR_RATIO, curBarOffset: CUR_BAR_OFFSET,
  barRx: BAR_RX, dotR: DOT_R, dotHlR: DOT_HL_R, labelYOffset: LABEL_Y_OFFSET, labelChars: LABEL_CHARS, axisFontSize: AXIS_FONT_SIZE,
  bulletPadRatio: BULLET_PAD_RATIO, bulletMaxH: BULLET_MAX_H, bulletRatio: BULLET_RATIO, bulletLabelDx: BULLET_LABEL_DX, bulletLabelY: BULLET_LABEL_Y, bulletChars: BULLET_CHARS,
  bulletInsetSm: BULLET_INSET_SM, bulletInsetMd: BULLET_INSET_MD, bulletInsetLg: BULLET_INSET_LG, pieStroke: PIE_STROKE, pieHlStroke: PIE_HL_STROKE, zTooltip: Z_TOOLTIP,
} as const;

function computeMaxScale(rows: readonly StudioDataRow[], targetValue: number, showTarget: boolean): number {
  const rowMax = rows.reduce((m, r) => Math.max(m, r.current, r.previous), SVG.minScale);
  const rawMax = showTarget ? Math.max(rowMax, targetValue) : rowMax;
  return Math.max(SVG.minScale, Math.ceil(rawMax * SVG.scaleHeadroom));
}

function resolveColor(rowId: string, baseColor: string, spotlightRowId: string | null): string {
  if (!spotlightRowId) return baseColor;
  return spotlightRowId === rowId ? "var(--action-primary)" : "var(--border-strong)";
}

function polarToCartesian(cx: number, cy: number, r: number, angleDeg: number) {
  const rad = ((angleDeg - SVG.rightDeg) * Math.PI) / SVG.halfDeg;
  return { x: cx + r * Math.cos(rad), y: cy + r * Math.sin(rad) };
}

function buildPieArcPath(startAngle: number, endAngle: number): string {
  const clampedEnd = endAngle - startAngle >= SVG.maxArcDeg ? startAngle + SVG.maxArcDeg : endAngle;
  const start = polarToCartesian(SVG.pieCx, SVG.pieCy, SVG.pieR, clampedEnd);
  const end = polarToCartesian(SVG.pieCx, SVG.pieCy, SVG.pieR, startAngle);
  const largeArc = clampedEnd - startAngle > SVG.halfDeg ? 1 : 0;
  return `M ${SVG.pieCx} ${SVG.pieCy} L ${start.x} ${start.y} A ${SVG.pieR} ${SVG.pieR} 0 ${largeArc} 0 ${end.x} ${end.y} Z`;
}

const ElevatedHoverTooltip: React.FC<{ row: StudioDataRow | null; total: number; unit: StudioUnit }> = ({ row, total, unit }) => {
  if (!row) return null;
  const share = total > 0 ? ((Math.max(0, row.current) / total) * SVG.pctScale).toFixed(1) : "0.0";
  const diff = row.current - row.previous;
  const diffPct = row.previous > 0 ? `${diff >= 0 ? "+" : ""}${Math.round((diff / row.previous) * SVG.pctScale)}%` : "N/A";
  return (
    <div data-testid="studio-chart-tooltip" style={{ position: "absolute", top: "8px", right: "8px", zIndex: SVG.zTooltip, backgroundColor: "var(--bg-elevated, var(--bg-surface))", color: "var(--text-primary)", border: "1px solid var(--border-strong)", padding: "7px 10px", borderRadius: "var(--radius-sm)", fontSize: "11px", boxShadow: "var(--shadow-lg)", pointerEvents: "none", minWidth: "152px" }}>
      <div style={{ fontWeight: FONT_WEIGHT_BOLD, marginBottom: "2px" }}>{row.label}</div>
      <div>Q4: <strong>{formatMetricValue(row.current, unit)}</strong> ({share}%)</div>
      <div style={{ color: "var(--text-secondary)" }}>Q3: {formatMetricValue(row.previous, unit)} ({diffPct})</div>
    </div>
  );
};

const DraggableTargetOverlay: React.FC<{
  svgRef: React.RefObject<SVGSVGElement | null>;
  maxVal: number;
  targetValue: number;
  unit: StudioUnit;
  onPreviewTarget: (val: number) => void;
  onCommitTarget: (base: number, next: number) => void;
}> = ({ svgRef, maxVal, targetValue, unit, onPreviewTarget, onCommitTarget }) => {
  const yPos = SVG.padT + SVG.plotH * (1 - Math.min(1, Math.max(0, targetValue / maxVal)));
  const startDrag = (ev: React.MouseEvent) => {
    ev.preventDefault();
    ev.stopPropagation();
    const baseVal = targetValue;
    let latestVal = baseVal;
    const calcVal = (clientY: number) => {
      const rect = svgRef.current?.getBoundingClientRect();
      if (!rect || rect.height <= 0) return latestVal;
      const svgY = ((clientY - rect.top) / rect.height) * SVG.h;
      return Math.round((1 - Math.min(1, Math.max(0, (svgY - SVG.padT) / SVG.plotH))) * maxVal);
    };
    const onMove = (m: MouseEvent) => { latestVal = calcVal(m.clientY); onPreviewTarget(latestVal); };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      if (latestVal !== baseVal) onCommitTarget(baseVal, latestVal);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  };

  return (
    <g data-testid="studio-target-line">
      <line x1={SVG.padL} y1={yPos} x2={SVG.w - SVG.padR} y2={yPos} stroke="var(--state-warning)" strokeWidth={SVG.targetStroke} strokeDasharray="5 4" />
      <g data-testid="studio-target-drag-handle" role="slider" tabIndex={0} aria-label="Kéo đường mục tiêu" aria-valuenow={targetValue} onMouseDown={startDrag} onKeyDown={(e) => { if (e.key === "ArrowUp") onCommitTarget(targetValue, targetValue + SVG.targetStep); if (e.key === "ArrowDown") onCommitTarget(targetValue, Math.max(0, targetValue - SVG.targetStep)); }} style={{ cursor: "ns-resize" }}>
        <rect x={SVG.w - SVG.padR - SVG.tagW} y={yPos - SVG.tagHalfH} width={SVG.tagW} height={SVG.tagH} rx={SVG.tagRx} fill="var(--bg-elevated, var(--bg-surface))" stroke="var(--state-warning)" strokeWidth={TARGET_TAG_STROKE} />
        <text x={SVG.w - SVG.padR - SVG.tagHalfW} y={yPos + SVG.tagTextDy} textAnchor="middle" fill="var(--text-primary)" fontSize={10} fontWeight={FONT_WEIGHT_SEMIBOLD}>
          Mục tiêu {formatMetricValue(targetValue, unit)}
        </text>
      </g>
    </g>
  );
};

const BarOrLineDatum: React.FC<{
  chartType: "bar" | "line";
  row: StudioDataRow;
  cx: number;
  cy: number;
  py: number;
  barW: number;
  color: string;
  showComparison: boolean;
  isHl: boolean;
  dimmed: boolean;
  onHover: (row: StudioDataRow | null) => void;
  onClickRow: (rowId: string) => void;
  onApplyWhatIf?: (rowId: string, deltaPct: number) => void;
}> = ({ chartType, row, cx, cy, py, barW, color, showComparison, isHl, dimmed, onHover, onClickRow, onApplyWhatIf }) => {
  const hCur = Math.max(2, SVG.padT + SVG.plotH - cy);
  const hPrev = Math.max(2, SVG.padT + SVG.plotH - py);
  const curX = showComparison ? cx - barW * SVG.curBarOffset : cx - barW / 2;
  return (
    <g data-testid={`studio-chart-datum-${row.id}`} onMouseEnter={() => onHover(row)} onMouseLeave={() => onHover(null)} onClick={(e) => { e.stopPropagation(); onClickRow(row.id); }} style={{ cursor: "pointer", opacity: dimmed ? SVG.dimOpacity : 1 }}>
      {chartType === "bar" ? (
        <>
          {showComparison && <rect x={cx - barW * SVG.prevBarOffset} y={py} width={barW * SVG.prevBarRatio} height={hPrev} rx={SVG.barRx} fill="var(--border-strong)" />}
          <rect x={curX} y={cy} width={barW} height={hCur} rx={SVG.barRx} fill={color} stroke={isHl ? "var(--text-primary)" : "none"} strokeWidth={isHl ? 2 : 0} />
          {isHl && onApplyWhatIf && (
            <rect
              data-testid="studio-whatif-drag-handle"
              role="slider" tabIndex={0} aria-label={`Kéo What-If ${row.label}`} aria-valuenow={row.current}
              x={curX + barW / 2 - HANDLE_HALF_W} y={Math.max(HANDLE_MIN_Y, cy - HANDLE_H)} width={HANDLE_W} height={HANDLE_H} rx={HANDLE_RX}
              fill="var(--action-primary)" stroke="var(--bg-surface)" strokeWidth={HANDLE_STROKE}
              onClick={(e) => e.stopPropagation()}
              onKeyDown={(e) => { if (e.key === "ArrowUp") onApplyWhatIf(row.id, WHATIF_DRAG_STEP); if (e.key === "ArrowDown") onApplyWhatIf(row.id, -WHATIF_DRAG_STEP); }}
              style={{ cursor: "ns-resize" }}
            />
          )}
        </>
      ) : (
        <circle cx={cx} cy={cy} r={isHl ? SVG.dotHlR : SVG.dotR} fill={color} stroke="var(--bg-surface)" strokeWidth={2} />
      )}
      <text x={cx} y={SVG.h - SVG.labelYOffset} textAnchor="middle" fill="var(--text-secondary)" fontSize={SVG.axisFontSize}>{row.label.slice(0, SVG.labelChars)}</text>
    </g>
  );
};

const BarOrLineSvg: React.FC<{
  chartType: "bar" | "line";
  rows: readonly StudioDataRow[];
  colors: readonly string[];
  maxVal: number;
  showComparison: boolean;
  highlightedRowId: string | null;
  spotlightRowId: string | null;
  callouts: readonly StudioCallout[];
  onHover: (row: StudioDataRow | null) => void;
  onClickRow: (rowId: string) => void;
  onApplyWhatIf?: (rowId: string, deltaPct: number) => void;
}> = ({ chartType, rows, colors, maxVal, showComparison, highlightedRowId, spotlightRowId, callouts, onHover, onClickRow, onApplyWhatIf }) => {
  const step = SVG.plotW / Math.max(1, rows.length);
  const barW = Math.min(SVG.barMaxW, step * SVG.barStepRatio);
  const points = rows.map((row, i) => ({
    row,
    cx: SVG.padL + step * i + step / 2,
    cy: SVG.padT + SVG.plotH * (1 - Math.max(0, row.current) / maxVal),
    py: SVG.padT + SVG.plotH * (1 - Math.max(0, row.previous) / maxVal),
    color: resolveColor(row.id, colors[i % colors.length], spotlightRowId),
  }));
  return (
    <>
      {chartType === "line" && points.length > 1 && <polyline fill="none" stroke={spotlightRowId ? "var(--border-strong)" : colors[0]} strokeWidth={SVG.lineWidth} points={points.map((p) => `${p.cx},${p.cy}`).join(" ")} />}
      {points.map((pt) => {
        const dimmed = spotlightRowId ? spotlightRowId !== pt.row.id : Boolean(highlightedRowId && highlightedRowId !== pt.row.id);
        return (
          <BarOrLineDatum key={pt.row.id} chartType={chartType} row={pt.row} cx={pt.cx} cy={pt.cy} py={pt.py} barW={barW} color={pt.color} showComparison={showComparison} isHl={highlightedRowId === pt.row.id || spotlightRowId === pt.row.id} dimmed={dimmed} onHover={onHover} onClickRow={onClickRow} onApplyWhatIf={onApplyWhatIf} />
        );
      })}
      <SvgCalloutArrowsOverlay callouts={callouts} points={points.map((p) => ({ id: p.row.id, x: p.cx, y: p.cy }))} />
    </>
  );
};

const BulletSvg: React.FC<{
  rows: readonly StudioDataRow[];
  colors: readonly string[];
  maxVal: number;
  targetValue: number;
  highlightedRowId: string | null;
  spotlightRowId: string | null;
  onHover: (row: StudioDataRow | null) => void;
  onClickRow: (rowId: string) => void;
}> = ({ rows, colors, maxVal, targetValue, highlightedRowId, spotlightRowId, onHover, onClickRow }) => {
  const laneH = SVG.plotH / Math.max(1, rows.length);
  const targetX = SVG.padL + SVG.plotW * Math.min(1, Math.max(0, targetValue / maxVal));
  return (
    <>
      {rows.map((r, i) => {
        const y = SVG.padT + i * laneH + laneH * SVG.bulletPadRatio;
        const barH = Math.min(SVG.bulletMaxH, laneH * SVG.bulletRatio);
        const wCur = SVG.plotW * Math.min(1, Math.max(0, r.current / maxVal));
        const wPrev = SVG.plotW * Math.min(1, Math.max(0, r.previous / maxVal));
        const dimmed = spotlightRowId ? spotlightRowId !== r.id : Boolean(highlightedRowId && highlightedRowId !== r.id);
        const fill = resolveColor(r.id, colors[i % colors.length], spotlightRowId);
        return (
          <g key={r.id} data-testid={`studio-chart-datum-${r.id}`} onMouseEnter={() => onHover(r)} onMouseLeave={() => onHover(null)} onClick={(e) => { e.stopPropagation(); onClickRow(r.id); }} style={{ cursor: "pointer", opacity: dimmed ? SVG.dimOpacity : 1 }}>
            <text x={SVG.padL - SVG.bulletLabelDx} y={y + barH * SVG.bulletLabelY} textAnchor="end" fill="var(--text-secondary)" fontSize={10}>{r.label.slice(0, SVG.bulletChars)}</text>
            <rect x={SVG.padL} y={y} width={SVG.plotW} height={barH} rx={SVG.barRx} fill="var(--bg-subtle)" />
            <rect x={SVG.padL} y={y + SVG.bulletInsetSm} width={wPrev} height={barH - SVG.bulletInsetMd} rx={SVG.bulletInsetSm} fill="var(--border-strong)" />
            <rect x={SVG.padL} y={y + SVG.bulletInsetMd} width={wCur} height={Math.max(SVG.bulletInsetMd, barH - SVG.bulletInsetLg)} rx={SVG.bulletInsetSm} fill={fill} />
          </g>
        );
      })}
      <line x1={targetX} y1={SVG.padT} x2={targetX} y2={SVG.padT + SVG.plotH} stroke="var(--state-warning)" strokeWidth={2} strokeDasharray="4 3" />
    </>
  );
};

const PieSvg: React.FC<{
  rows: readonly StudioDataRow[];
  colors: readonly string[];
  total: number;
  highlightedRowId: string | null;
  spotlightRowId: string | null;
  onHover: (row: StudioDataRow | null) => void;
  onClickRow: (rowId: string) => void;
}> = ({ rows, colors, total, highlightedRowId, spotlightRowId, onHover, onClickRow }) => {
  let angle = 0;
  return (
    <>
      {rows.map((r, i) => {
        const sweep = total > 0 ? (Math.max(0, r.current) / total) * SVG.fullDeg : 0;
        const start = angle;
        angle += sweep;
        const isHl = highlightedRowId === r.id || spotlightRowId === r.id;
        const dimmed = spotlightRowId ? spotlightRowId !== r.id : Boolean(highlightedRowId && !isHl);
        return (
          <path key={r.id} data-testid={`studio-chart-datum-${r.id}`} d={buildPieArcPath(start, angle)} fill={resolveColor(r.id, colors[i % colors.length], spotlightRowId)} stroke="var(--bg-surface)" strokeWidth={isHl ? SVG.pieHlStroke : SVG.pieStroke} onMouseEnter={() => onHover(r)} onMouseLeave={() => onHover(null)} onClick={(e) => { e.stopPropagation(); onClickRow(r.id); }} style={{ cursor: "pointer", opacity: dimmed ? SVG.dimOpacity : 1 }} />
        );
      })}
    </>
  );
};

const KpiGridMode: React.FC<{
  rows: readonly StudioDataRow[];
  colors: readonly string[];
  unit: StudioUnit;
  targetValue: number;
  highlightedRowId: string | null;
  spotlightRowId: string | null;
  onClickRow: (rowId: string) => void;
}> = ({ rows, colors, unit, targetValue, highlightedRowId, spotlightRowId, onClickRow }) => {
  const { t } = useTranslation();
  return (
    <div data-testid="studio-kpi-chart-view" style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(120px, 1fr))", gap: "8px", padding: "8px 0" }}>
      {rows.map((r, i) => {
        const isHl = highlightedRowId === r.id || spotlightRowId === r.id;
        const pct = r.previous > 0 ? Math.round(((r.current - r.previous) / r.previous) * SVG.pctScale) : 0;
        const targetPct = Math.min(SVG.pctScale, Math.round((r.current / Math.max(1, targetValue)) * SVG.pctScale));
        return (
          <button key={r.id} type="button" data-testid={`studio-chart-datum-${r.id}`} onClick={(e) => { e.stopPropagation(); onClickRow(r.id); }} style={{ textAlign: "left", padding: "10px", borderRadius: "var(--radius-sm)", border: isHl ? "2px solid var(--action-primary)" : "1px solid var(--border-default)", backgroundColor: isHl ? "var(--bg-selected)" : "var(--bg-workspace)", cursor: "pointer" }}>
            <div style={{ fontSize: "11px", color: "var(--text-muted)", marginBottom: "4px" }}>{r.label}</div>
            <div style={{ fontSize: "17px", fontWeight: FONT_WEIGHT_BOLD, color: resolveColor(r.id, colors[i % colors.length], spotlightRowId) }}>{formatMetricValue(r.current, unit)}</div>
            <div style={{ height: "4px", borderRadius: "999px", backgroundColor: "var(--bg-subtle)", margin: "6px 0 4px", overflow: "hidden" }}>
              <div style={{ height: "100%", width: `${targetPct}%`, backgroundColor: "var(--action-primary)" }} />
            </div>
            <div style={{ fontSize: "10.5px", color: pct >= 0 ? "var(--state-success)" : "var(--state-danger)" }}>{pct >= 0 ? "+" : ""}{pct}% {t("deckGen.vsPrior", "vs kỳ trước")}</div>
          </button>
        );
      })}
    </div>
  );
};

const SvgChartContentLayer: React.FC<{
  props: StudioSvgChartsProps;
  colors: readonly string[];
  total: number;
  maxVal: number;
  onHover: (row: StudioDataRow | null) => void;
}> = ({ props, colors, total, maxVal, onHover }) => {
  if (props.chartType === "bar" || props.chartType === "line") {
    return (
      <>
        <line x1={SVG.padL} y1={SVG.padT + SVG.plotH} x2={SVG.w - SVG.padR} y2={SVG.padT + SVG.plotH} stroke="var(--border-default)" />
        <BarOrLineSvg chartType={props.chartType} rows={props.rows} colors={colors} maxVal={maxVal} showComparison={props.showComparison} highlightedRowId={props.highlightedRowId} spotlightRowId={props.spotlightRowId} callouts={props.callouts} onHover={onHover} onClickRow={props.onToggleRowHighlight} onApplyWhatIf={props.onApplyWhatIf} />
      </>
    );
  }
  if (props.chartType === "movement") return <MovementBarsSvg rows={props.rows} unit={props.unit} highlightedRowId={props.highlightedRowId} spotlightRowId={props.spotlightRowId} onHover={onHover} onClickRow={props.onToggleRowHighlight} />;
  if (props.chartType === "bullet") return <BulletSvg rows={props.rows} colors={colors} maxVal={maxVal} targetValue={props.targetValue} highlightedRowId={props.highlightedRowId} spotlightRowId={props.spotlightRowId} onHover={onHover} onClickRow={props.onToggleRowHighlight} />;
  if (props.chartType === "tick-bar") return <TickBar2026Svg rows={props.rows} colors={colors} maxVal={maxVal} targetValue={props.targetValue} highlightedRowId={props.highlightedRowId} spotlightRowId={props.spotlightRowId} onHover={onHover} onClickRow={props.onToggleRowHighlight} />;
  if (props.chartType === "segmented") return <SegmentedBarSvg rows={props.rows} colors={colors} total={total} highlightedRowId={props.highlightedRowId} spotlightRowId={props.spotlightRowId} onHover={onHover} onClickRow={props.onToggleRowHighlight} />;
  return <PieSvg rows={props.rows} colors={colors} total={total} highlightedRowId={props.highlightedRowId} spotlightRowId={props.spotlightRowId} onHover={onHover} onClickRow={props.onToggleRowHighlight} />;
};

export const StudioSvgCharts: React.FC<StudioSvgChartsProps> = (props) => {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [hoveredRow, setHoveredRow] = useState<StudioDataRow | null>(null);
  const activeSpotlightId =
    props.spotlightRowId && props.rows.some((r) => r.id === props.spotlightRowId) ? props.spotlightRowId : null;
  const effectiveProps = activeSpotlightId === props.spotlightRowId ? props : { ...props, spotlightRowId: activeSpotlightId };
  const colors = getPaletteColors(effectiveProps.paletteId);
  const total = effectiveProps.rows.reduce((acc, r) => acc + Math.max(0, r.current), 0);
  const maxVal = computeMaxScale(effectiveProps.rows, effectiveProps.targetValue, effectiveProps.showTargetLine);

  if (effectiveProps.chartType === "kpi") {
    return <KpiGridMode rows={effectiveProps.rows} colors={colors} unit={effectiveProps.unit} targetValue={effectiveProps.targetValue} highlightedRowId={effectiveProps.highlightedRowId} spotlightRowId={activeSpotlightId} onClickRow={effectiveProps.onToggleRowHighlight} />;
  }
  if (effectiveProps.chartType === "sparkline") {
    return <SparklineRowsView rows={effectiveProps.rows} colors={colors} unit={effectiveProps.unit} highlightedRowId={effectiveProps.highlightedRowId} spotlightRowId={activeSpotlightId} onClickRow={effectiveProps.onToggleRowHighlight} />;
  }
  if (effectiveProps.chartType === "heatmap") {
    return <HeatmapMatrixView rows={effectiveProps.rows} unit={effectiveProps.unit} targetValue={effectiveProps.targetValue} highlightedRowId={effectiveProps.highlightedRowId} spotlightRowId={activeSpotlightId} onClickRow={effectiveProps.onToggleRowHighlight} />;
  }

  const showTarget = effectiveProps.showTargetLine && (effectiveProps.chartType === "bar" || effectiveProps.chartType === "line");
  return (
    <div style={{ position: "relative", display: "flex", flexDirection: effectiveProps.legendPos === "right" ? "row" : "column", gap: "8px", alignItems: "stretch" }}>
      <ElevatedHoverTooltip row={hoveredRow} total={total} unit={effectiveProps.unit} />
      <svg ref={svgRef} viewBox={`0 0 ${SVG.w} ${SVG.h}`} data-testid={`studio-svg-${effectiveProps.chartType}`} style={{ width: "100%", height: "220px", display: "block" }}>
        <SvgChartContentLayer props={effectiveProps} colors={colors} total={total} maxVal={maxVal} onHover={setHoveredRow} />
        {showTarget && <DraggableTargetOverlay svgRef={svgRef} maxVal={maxVal} targetValue={effectiveProps.targetValue} unit={effectiveProps.unit} onPreviewTarget={effectiveProps.onPreviewTarget} onCommitTarget={effectiveProps.onCommitTarget} />}
      </svg>
      {effectiveProps.legendPos !== "hidden" && (
        <div data-testid="studio-chart-legend" style={{ display: "flex", flexDirection: effectiveProps.legendPos === "right" ? "column" : "row", flexWrap: "wrap", gap: "8px", fontSize: "11px", color: "var(--text-secondary)", justifyContent: "center" }}>
          {effectiveProps.rows.map((r, i) => (
            <span key={r.id} style={{ display: "inline-flex", alignItems: "center", gap: "4px" }}>
              <span style={{ width: "8px", height: "8px", borderRadius: "2px", backgroundColor: resolveColor(r.id, colors[i % colors.length], activeSpotlightId) }} />
              <span>{r.label}</span>
            </span>
          ))}
        </div>
      )}
    </div>
  );
};
