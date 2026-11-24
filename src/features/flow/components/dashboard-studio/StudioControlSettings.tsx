// @story 06.5
// @implements AC-02, AC-03, AC-04 (Level 1 Control Plane: Component List with On/Off Toggle + Drill-Down & Backend Sync Preview)
import React from "react";
import { BrandIcon } from "../../../../components/ui/BrandIcon";
import { useFormatter } from "../../../../hooks/useFormatter";
import { useTranslation } from "../../../../hooks/useTranslation";
import {
  DECK_GEN_MAX_WORDS_PER_SLIDE,
  type GenerateDeckResponse,
  type NormalizeChartSourceResponse,
} from "../../../../lib/validations/deck-generation";
import {
  FONT_WEIGHT_MEDIUM,
  FONT_WEIGHT_SEMIBOLD,
  ICON_SM,
  SECTION_TITLE_STYLE,
  STUDIO_SECONDARY_BTN,
} from "./studioStyles";
import type {
  StudioControlView,
  StudioSnapshot,
  StudioWidgetId,
} from "./types";

const PREVIEW_POINTS_LIMIT = 4;

interface ComponentRowConfig {
  id: string;
  label: string;
  hint: string;
  widgetId?: StudioWidgetId;
  inspectTarget?: Exclude<StudioControlView, "general">;
}

const COMPONENT_ROWS: readonly ComponentRowConfig[] = [
  { id: "chart", label: "Biểu đồ chính", hint: "Loại biểu đồ, trục, mục tiêu, Chiếu điểm", inspectTarget: "chart" },
  { id: "kpi", label: "Thẻ KPI (3)", hint: "Action Title, Tick-Bar 2026 & Delta", widgetId: "kpiStrip", inspectTarget: "kpi" },
  { id: "sharebar", label: "Xếp hạng Share-Bar", hint: "Thứ tự sắp xếp, tỷ trọng %, kỳ trước", widgetId: "shareBar", inspectTarget: "sharebar" },
  { id: "timeframe", label: "Bộ lọc thời gian", hint: "Hiển thị chọn kỳ Q4 / Q3 / YTD", widgetId: "timeframe" },
  { id: "footnote", label: "Ghi chú nguồn", hint: "Chú thích nguồn dữ liệu cuối thẻ", widgetId: "footnote" },
];

const RowToggleButton: React.FC<{
  widgetId: StudioWidgetId;
  enabled: boolean;
  label: string;
  onToggle: () => void;
}> = ({ widgetId, enabled, label, onToggle }) => (
  <button
    type="button"
    role="switch"
    aria-checked={enabled}
    aria-pressed={enabled}
    aria-label={`Bật/tắt ${label}`}
    data-testid={`studio-widget-chip-${widgetId}`}
    onClick={(e) => { e.stopPropagation(); onToggle(); }}
    style={{
      padding: "2px 7px",
      borderRadius: "999px",
      border: enabled ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
      backgroundColor: enabled ? "var(--bg-selected)" : "var(--bg-surface)",
      color: enabled ? "var(--action-primary)" : "var(--text-muted)",
      fontSize: "10px",
      fontWeight: FONT_WEIGHT_SEMIBOLD,
      cursor: "pointer",
      flexShrink: 0,
    }}
  >
    {enabled ? "Bật" : "Tắt"}
  </button>
);

export const StudioComponentListSection: React.FC<{
  snapshot: StudioSnapshot;
  onToggleWidget: (id: StudioWidgetId) => void;
  onDrillInto: (view: Exclude<StudioControlView, "general">) => void;
}> = ({ snapshot, onToggleWidget, onDrillInto }) => (
  <div data-testid="studio-component-list" style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
    <span style={SECTION_TITLE_STYLE}>Các thành phần trên Dashboard</span>
    <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
      {COMPONENT_ROWS.map((row) => {
        const enabled = row.widgetId ? snapshot.widgets[row.widgetId] : true;
        const drillable = Boolean(row.inspectTarget);
        return (
          <div
            key={row.id}
            data-testid={`studio-component-row-${row.id}`}
            role={drillable ? "button" : undefined}
            tabIndex={drillable ? 0 : undefined}
            onClick={() => row.inspectTarget && onDrillInto(row.inspectTarget)}
            onKeyDown={(e) => { if (row.inspectTarget && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); onDrillInto(row.inspectTarget); } }}
            style={{
              display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px",
              padding: "7px 9px", borderRadius: "var(--radius-sm)",
              border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)",
              cursor: drillable ? "pointer" : "default",
            }}
          >
            <div style={{ minWidth: 0, flex: 1 }}>
              <div style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_MEDIUM, color: "var(--text-primary)" }}>{row.label}</div>
              <div style={{ fontSize: "10.5px", color: "var(--text-muted)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{row.hint}</div>
            </div>
            <div style={{ display: "inline-flex", alignItems: "center", gap: "6px", flexShrink: 0 }}>
              {row.widgetId && (
                <RowToggleButton widgetId={row.widgetId} enabled={enabled} label={row.label} onToggle={() => onToggleWidget(row.widgetId!)} />
              )}
              {drillable && <BrandIcon name="chevron-right" size={ICON_SM} color="var(--text-muted)" />}
            </div>
          </div>
        );
      })}
    </div>
  </div>
);

export const StudioBackendSyncPreview: React.FC<{
  hasSource: boolean;
  submitting: boolean;
  normalizedChart: NormalizeChartSourceResponse | null;
  generatedSummary: GenerateDeckResponse | null;
  onPreviewChart: () => void;
}> = ({ hasSource, submitting, normalizedChart, generatedSummary, onPreviewChart }) => {
  const { locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const points = normalizedChart?.spec.series[0]?.points.slice(0, PREVIEW_POINTS_LIMIT) ?? [];

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
      {hasSource && (
        <button type="button" disabled={submitting} data-testid="deck-gen-preview-chart-btn" onClick={onPreviewChart} style={{ ...STUDIO_SECONDARY_BTN, justifyContent: "center" }}>
          <BrandIcon name="trending-up" size={ICON_SM} color="currentColor" />
          <span>Chuẩn hóa từ Backend (ChartSpecV12)</span>
        </button>
      )}
      {normalizedChart && (
        <div data-testid="normalized-chart-preview" style={{ padding: "8px 10px", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-sm)", fontSize: "11.5px", display: "flex", flexDirection: "column", gap: "5px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", gap: "6px" }}>
            <strong style={{ fontWeight: FONT_WEIGHT_SEMIBOLD }}>{normalizedChart.spec.title}</strong>
            {typeof normalizedChart.proportionalSum === "number" && (
              <span data-testid="chart-proportional-sum" style={{ color: "var(--state-success)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
                Tổng tỷ trọng: {formatNumber(normalizedChart.proportionalSum)}%
              </span>
            )}
          </div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "4px" }}>
            {points.map((pt, idx) => (
              <span key={`${pt.label}-${idx}`} style={{ padding: "2px 6px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)" }}>
                {pt.label}: {formatNumber(pt.value)}{normalizedChart.spec.unit ? ` ${normalizedChart.spec.unit}` : ""}
              </span>
            ))}
          </div>
        </div>
      )}
      {generatedSummary && (
        <div data-testid="deck-gen-result-summary" style={{ padding: "7px 9px", backgroundColor: "var(--state-success-soft)", border: "1px solid var(--state-success)", borderRadius: "var(--radius-xs)", fontSize: "11.5px", color: "var(--text-primary)" }}>
          Đã lưu V15: {formatNumber(generatedSummary.deck.slides.length)} slides · Tự tách: {formatNumber(generatedSummary.autoSplitCount)} · Tối đa: {formatNumber(generatedSummary.maxWordsPerSlide)}/{formatNumber(DECK_GEN_MAX_WORDS_PER_SLIDE)}
        </div>
      )}
    </div>
  );
};
