// @story 06.5
// @implements AC-03, AC-04 (Additional 5 Dashboard-Design Chart Shapes + SVG Callout Arrows Overlay)
import React from "react";
import { useTranslation } from "../../../../hooks/useTranslation";
import { formatMetricValue } from "./studioDataUtils";
import { computeRowDeltaPct } from "./studioStorytellingUtils";
import { FONT_WEIGHT_BOLD, FONT_WEIGHT_SEMIBOLD } from "./studioStyles";
import type { StudioCallout, StudioDataRow, StudioUnit } from "./types";

const SVG_W = 540;
const PAD_L = 74;
const PAD_R = 24;
const PAD_T = 22;
const PLOT_W = 442;
const PLOT_H = 166;
const PCT_SCALE = 100;
const DIM_OPACITY = 0.38;
const CALLOUT_W = 148;
const CALLOUT_H = 22;
const CALLOUT_OFFSET_Y = 34;
const CALLOUT_RX = 4;
const CALLOUT_EDGE_PAD = 10;
const CALLOUT_MIN_Y = 4;
const CALLOUT_ARROW_STROKE = 1.6;
const CALLOUT_DOT_R = 3;
const CALLOUT_BOX_STROKE = 1.2;
const CALLOUT_TEXT_DY = 14;
const CALLOUT_MAX_CHARS = 26;
const BAR_RX = 3;
const BAR_MIN_W = 4;
const MOV_MIN_SCALE = 10;
const MOV_HALF_PAD = 26;
const MOV_BAR_H = 16;
const MOV_LABEL_DX = 8;
const MOV_TEXT_DY = 12;
const MOV_VAL_DX = 6;
const MOV_AXIS_STROKE = 1.5;
const TICK_RIGHT_RESERVE = 44;
const TICK_LABEL_DY = 11;
const TICK_STROKE = 2.2;
const TICK_PAD_RATIO = 0.24;
const TICK_BAR_H = 14;
const TICK_OVERHANG = 4;
const SEG_PAD_X = 24;
const SEG_TOTAL_PAD = 48;
const SEG_Y = 74;
const SEG_H = 44;
const SEG_PCT_DY = 26;
const SEG_LBL_DY = 18;
const SEG_LBL_CHARS = 8;
const MIN_LABEL_PCT = 10;
const LABEL_CHARS = 10;
const MAX_SPARK_POINTS = 5;
const SPARK_W = 96;
const SPARK_H = 24;
const SPARK_PAD = 4;
const SPARK_PAD_DOUBLE = 8;
const HEAT_HIGH = 110;
const HEAT_MID = 85;
const FONT_SM = 10;
const FONT_MD = 10.5;
const FONT_LG = 11;

function resolveRowFill(rowId: string, baseColor: string, spotlightRowId: string | null): string {
  if (!spotlightRowId) return baseColor;
  return spotlightRowId === rowId ? "var(--action-primary)" : "var(--border-strong)";
}

function isRowDimmed(rowId: string, highlightedRowId: string | null, spotlightRowId: string | null): boolean {
  if (spotlightRowId) return spotlightRowId !== rowId;
  return Boolean(highlightedRowId && highlightedRowId !== rowId);
}

export const SvgCalloutArrowsOverlay: React.FC<{
  callouts: readonly StudioCallout[];
  points: ReadonlyArray<{ id: string; x: number; y: number }>;
}> = ({ callouts, points }) => (
  <>
    {callouts.map((c) => {
      const pt = points.find((p) => p.id === c.rowId);
      if (!pt) return null;
      const boxX = Math.max(CALLOUT_EDGE_PAD, Math.min(SVG_W - CALLOUT_W - CALLOUT_EDGE_PAD, pt.x - CALLOUT_W / 2));
      const boxY = Math.max(CALLOUT_MIN_Y, pt.y - CALLOUT_OFFSET_Y);
      const tipY = Math.max(boxY + CALLOUT_H + 2, pt.y - CALLOUT_MIN_Y);
      return (
        <g key={c.id} data-testid={`studio-svg-callout-${c.rowId}`}>
          <line x1={pt.x} y1={boxY + CALLOUT_H} x2={pt.x} y2={tipY} stroke="var(--action-primary)" strokeWidth={CALLOUT_ARROW_STROKE} />
          <circle cx={pt.x} cy={tipY} r={CALLOUT_DOT_R} fill="var(--action-primary)" />
          <rect x={boxX} y={boxY} width={CALLOUT_W} height={CALLOUT_H} rx={CALLOUT_RX} fill="var(--bg-elevated, var(--bg-surface))" stroke="var(--border-strong)" strokeWidth={CALLOUT_BOX_STROKE} />
          <text x={boxX + CALLOUT_W / 2} y={boxY + CALLOUT_TEXT_DY} textAnchor="middle" fill="var(--text-primary)" fontSize={FONT_SM} fontWeight={FONT_WEIGHT_SEMIBOLD}>
            {c.text.slice(0, CALLOUT_MAX_CHARS)}
          </text>
        </g>
      );
    })}
  </>
);

export const MovementBarsSvg: React.FC<{
  rows: readonly StudioDataRow[];
  unit: StudioUnit;
  highlightedRowId: string | null;
  spotlightRowId: string | null;
  onHover: (row: StudioDataRow | null) => void;
  onClickRow: (rowId: string) => void;
}> = ({ rows, unit, highlightedRowId, spotlightRowId, onHover, onClickRow }) => {
  const deltas = rows.map((r) => r.current - r.previous);
  const maxAbs = Math.max(MOV_MIN_SCALE, ...deltas.map((d) => Math.abs(d)));
  const zeroX = PAD_L + PLOT_W / 2;
  const laneH = PLOT_H / Math.max(1, rows.length);
  return (
    <>
      <line x1={zeroX} y1={PAD_T} x2={zeroX} y2={PAD_T + PLOT_H} stroke="var(--border-strong)" strokeWidth={MOV_AXIS_STROKE} />
      {rows.map((r, i) => {
        const diff = deltas[i];
        const w = (Math.abs(diff) / maxAbs) * (PLOT_W / 2 - MOV_HALF_PAD);
        const y = PAD_T + i * laneH + laneH * TICK_PAD_RATIO;
        const x = diff >= 0 ? zeroX : zeroX - w;
        const baseColor = diff >= 0 ? "var(--state-success)" : "var(--state-danger)";
        const fill = resolveRowFill(r.id, baseColor, spotlightRowId);
        const dim = isRowDimmed(r.id, highlightedRowId, spotlightRowId);
        return (
          <g key={r.id} data-testid={`studio-chart-datum-${r.id}`} onMouseEnter={() => onHover(r)} onMouseLeave={() => onHover(null)} onClick={(e) => { e.stopPropagation(); onClickRow(r.id); }} style={{ cursor: "pointer", opacity: dim ? DIM_OPACITY : 1 }}>
            <text x={PAD_L - MOV_LABEL_DX} y={y + MOV_TEXT_DY} textAnchor="end" fill="var(--text-secondary)" fontSize={FONT_MD}>{r.label.slice(0, LABEL_CHARS)}</text>
            <rect x={x} y={y} width={Math.max(BAR_MIN_W, w)} height={MOV_BAR_H} rx={BAR_RX} fill={fill} />
            <text x={diff >= 0 ? x + w + MOV_VAL_DX : x - MOV_VAL_DX} y={y + MOV_TEXT_DY} textAnchor={diff >= 0 ? "start" : "end"} fill="var(--text-primary)" fontSize={FONT_SM} fontWeight={FONT_WEIGHT_SEMIBOLD}>
              {diff >= 0 ? "+" : "-"}{formatMetricValue(Math.abs(diff), unit)}
            </text>
          </g>
        );
      })}
    </>
  );
};

export const TickBar2026Svg: React.FC<{
  rows: readonly StudioDataRow[];
  colors: readonly string[];
  maxVal: number;
  targetValue: number;
  highlightedRowId: string | null;
  spotlightRowId: string | null;
  onHover: (row: StudioDataRow | null) => void;
  onClickRow: (rowId: string) => void;
}> = ({ rows, colors, maxVal, targetValue, highlightedRowId, spotlightRowId, onHover, onClickRow }) => {
  const laneH = PLOT_H / Math.max(1, rows.length);
  const trackW = PLOT_W - TICK_RIGHT_RESERVE;
  const tickX = PAD_L + trackW * Math.min(1, Math.max(0, targetValue / maxVal));
  return (
    <>
      {rows.map((r, i) => {
        const y = PAD_T + i * laneH + laneH * TICK_PAD_RATIO;
        const barW = trackW * Math.min(1, Math.max(0, r.current / maxVal));
        const pct = Math.round((r.current / Math.max(1, targetValue)) * PCT_SCALE);
        const fill = resolveRowFill(r.id, colors[i % colors.length], spotlightRowId);
        const dim = isRowDimmed(r.id, highlightedRowId, spotlightRowId);
        return (
          <g key={r.id} data-testid={`studio-chart-datum-${r.id}`} onMouseEnter={() => onHover(r)} onMouseLeave={() => onHover(null)} onClick={(e) => { e.stopPropagation(); onClickRow(r.id); }} style={{ cursor: "pointer", opacity: dim ? DIM_OPACITY : 1 }}>
            <text x={PAD_L - MOV_LABEL_DX} y={y + TICK_LABEL_DY} textAnchor="end" fill="var(--text-secondary)" fontSize={FONT_MD}>{r.label.slice(0, LABEL_CHARS)}</text>
            <rect x={PAD_L} y={y} width={trackW} height={TICK_BAR_H} rx={BAR_RX} fill="var(--bg-subtle)" />
            <rect x={PAD_L} y={y} width={Math.max(BAR_MIN_W, barW)} height={TICK_BAR_H} rx={BAR_RX} fill={fill} />
            <line x1={tickX} y1={y - TICK_OVERHANG} x2={tickX} y2={y + TICK_BAR_H + TICK_OVERHANG} stroke="var(--text-primary)" strokeWidth={TICK_STROKE} />
            <text x={SVG_W - PAD_R} y={y + TICK_LABEL_DY} textAnchor="end" fill={pct >= PCT_SCALE ? "var(--state-success)" : "var(--text-secondary)"} fontSize={FONT_MD} fontWeight={FONT_WEIGHT_SEMIBOLD}>{pct}%</text>
          </g>
        );
      })}
    </>
  );
};

export const SegmentedBarSvg: React.FC<{
  rows: readonly StudioDataRow[];
  colors: readonly string[];
  total: number;
  highlightedRowId: string | null;
  spotlightRowId: string | null;
  onHover: (row: StudioDataRow | null) => void;
  onClickRow: (rowId: string) => void;
}> = ({ rows, colors, total, highlightedRowId, spotlightRowId, onHover, onClickRow }) => {
  let cursorX = SEG_PAD_X;
  const barTotalW = SVG_W - SEG_TOTAL_PAD;
  return (
    <>
      {rows.map((r, i) => {
        const pct = total > 0 ? Math.round((Math.max(0, r.current) / total) * PCT_SCALE) : 0;
        const w = total > 0 ? (Math.max(0, r.current) / total) * barTotalW : 0;
        const x = cursorX;
        cursorX += w;
        const fill = resolveRowFill(r.id, colors[i % colors.length], spotlightRowId);
        const dim = isRowDimmed(r.id, highlightedRowId, spotlightRowId);
        return (
          <g key={r.id} data-testid={`studio-chart-datum-${r.id}`} onMouseEnter={() => onHover(r)} onMouseLeave={() => onHover(null)} onClick={(e) => { e.stopPropagation(); onClickRow(r.id); }} style={{ cursor: "pointer", opacity: dim ? DIM_OPACITY : 1 }}>
            <rect x={x} y={SEG_Y} width={Math.max(2, w - 2)} height={SEG_H} rx={BAR_RX} fill={fill} />
            {pct >= MIN_LABEL_PCT && (
              <text x={x + w / 2} y={SEG_Y + SEG_PCT_DY} textAnchor="middle" fill="var(--action-primary-fg)" fontSize={FONT_LG} fontWeight={FONT_WEIGHT_BOLD}>{pct}%</text>
            )}
            <text x={x + w / 2} y={SEG_Y + SEG_H + SEG_LBL_DY} textAnchor="middle" fill="var(--text-secondary)" fontSize={FONT_SM}>{r.label.slice(0, SEG_LBL_CHARS)}</text>
          </g>
        );
      })}
    </>
  );
};

export const SparklineRowsView: React.FC<{
  rows: readonly StudioDataRow[];
  colors: readonly string[];
  unit: StudioUnit;
  highlightedRowId: string | null;
  spotlightRowId: string | null;
  onClickRow: (rowId: string) => void;
}> = ({ rows, colors, unit, highlightedRowId, spotlightRowId, onClickRow }) => (
  <div data-testid="studio-svg-sparkline" style={{ display: "flex", flexDirection: "column", gap: "6px", padding: "6px 2px" }}>
    {rows.map((r, i) => {
      const pct = computeRowDeltaPct(r);
      const pts = Array.from({ length: MAX_SPARK_POINTS }, (_, idx) => {
        const t = idx / (MAX_SPARK_POINTS - 1);
        const val = r.previous + (r.current - r.previous) * t;
        const maxV = Math.max(1, r.current, r.previous);
        const x = Math.round(t * (SPARK_W - SPARK_PAD_DOUBLE) + SPARK_PAD);
        const y = Math.round(SPARK_H - SPARK_PAD - (val / maxV) * (SPARK_H - SPARK_PAD_DOUBLE));
        return `${x},${y}`;
      }).join(" ");
      const stroke = resolveRowFill(r.id, colors[i % colors.length], spotlightRowId);
      const dim = isRowDimmed(r.id, highlightedRowId, spotlightRowId);
      return (
        <button key={r.id} type="button" data-testid={`studio-chart-datum-${r.id}`} onClick={(e) => { e.stopPropagation(); onClickRow(r.id); }} style={{ display: "grid", gridTemplateColumns: "1fr auto auto", alignItems: "center", gap: "10px", padding: "6px 8px", borderRadius: "var(--radius-xs)", border: highlightedRowId === r.id ? "1.5px solid var(--action-primary)" : "1px solid var(--border-subtle)", backgroundColor: highlightedRowId === r.id ? "var(--bg-selected)" : "var(--bg-surface)", color: "var(--text-primary)", cursor: "pointer", opacity: dim ? DIM_OPACITY : 1, textAlign: "left" }}>
          <span style={{ fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{r.label}</span>
          <svg width={SPARK_W} height={SPARK_H} viewBox={`0 0 ${SPARK_W} ${SPARK_H}`}><polyline fill="none" stroke={stroke} strokeWidth={2} points={pts} /></svg>
          <span style={{ fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD, whiteSpace: "nowrap" }}>
            {formatMetricValue(r.current, unit)} <span style={{ color: pct >= 0 ? "var(--state-success)" : "var(--state-danger)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>({pct >= 0 ? "+" : ""}{pct}%)</span>
          </span>
        </button>
      );
    })}
  </div>
);

function resolveHeatCellBg(targetPct: number): string {
  if (targetPct >= HEAT_HIGH) return "var(--state-success-soft)";
  if (targetPct >= HEAT_MID) return "var(--bg-selected)";
  return "var(--state-warning-soft)";
}

export const HeatmapMatrixView: React.FC<{
  rows: readonly StudioDataRow[];
  unit: StudioUnit;
  targetValue: number;
  highlightedRowId: string | null;
  spotlightRowId: string | null;
  onClickRow: (rowId: string) => void;
}> = ({ rows, unit, targetValue, highlightedRowId, spotlightRowId, onClickRow }) => {
  const { t } = useTranslation();
  return (
    <div data-testid="studio-svg-heatmap" style={{ overflowX: "auto", padding: "4px 0" }}>
      <table style={{ width: "100%", borderCollapse: "separate", borderSpacing: "4px", fontSize: "11px" }}>
        <thead>
          <tr style={{ color: "var(--text-muted)", textAlign: "left" }}>
            <th style={{ padding: "4px 6px" }}>{t("deckGen.heatmapColChannel", "Kênh")}</th>
            <th style={{ padding: "4px 6px" }}>Q3</th>
            <th style={{ padding: "4px 6px" }}>Q4</th>
            <th style={{ padding: "4px 6px" }}>{t("deckGen.heatmapColGrowth", "Tăng trưởng")}</th>
            <th style={{ padding: "4px 6px" }}>{t("deckGen.heatmapColTarget", "Đạt mục tiêu")}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => {
            const growth = computeRowDeltaPct(r);
            const targetPct = Math.round((r.current / Math.max(1, targetValue)) * PCT_SCALE);
            const dim = isRowDimmed(r.id, highlightedRowId, spotlightRowId);
            const cellBg = resolveHeatCellBg(targetPct);
            return (
              <tr key={r.id} data-testid={`studio-chart-datum-${r.id}`} onClick={(e) => { e.stopPropagation(); onClickRow(r.id); }} style={{ cursor: "pointer", opacity: dim ? DIM_OPACITY : 1 }}>
                <td style={{ padding: "6px 8px", fontWeight: FONT_WEIGHT_SEMIBOLD, backgroundColor: "var(--bg-surface)", borderRadius: "var(--radius-xs)" }}>{r.label}</td>
                <td style={{ padding: "6px 8px", backgroundColor: "var(--bg-subtle)", borderRadius: "var(--radius-xs)" }}>{formatMetricValue(r.previous, unit)}</td>
                <td style={{ padding: "6px 8px", fontWeight: FONT_WEIGHT_BOLD, backgroundColor: cellBg, borderRadius: "var(--radius-xs)" }}>{formatMetricValue(r.current, unit)}</td>
                <td style={{ padding: "6px 8px", color: growth >= 0 ? "var(--state-success)" : "var(--state-danger)", backgroundColor: "var(--bg-surface)", borderRadius: "var(--radius-xs)" }}>{growth >= 0 ? "+" : ""}{growth}%</td>
                <td style={{ padding: "6px 8px", fontWeight: FONT_WEIGHT_SEMIBOLD, backgroundColor: cellBg, borderRadius: "var(--radius-xs)" }}>{targetPct}%</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
};
