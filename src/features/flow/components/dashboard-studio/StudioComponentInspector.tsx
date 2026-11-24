// @story 06.5
// @implements AC-02, AC-04 (Level 2 Component Inspector: Chart, KPI & Share-Bar Drill-Down Settings)
import React from "react";
import { BrandIcon } from "../../../../components/ui/BrandIcon";
import { useTranslation } from "../../../../hooks/useTranslation";
import {
  FONT_WEIGHT_SEMIBOLD,
  ICON_SM,
  SECTION_TITLE_STYLE,
  SEGMENT_GROUP_STYLE,
  segmentPillStyle,
  STUDIO_COMPACT_INPUT,
  STUDIO_INPUT_STYLE,
} from "./studioStyles";
import {
  STUDIO_CHART_OPTIONS,
  STUDIO_PALETTES,
  type StudioChartType,
  type StudioControlView,
  type StudioLegendPos,
  type StudioSnapshot,
  type StudioUnit,
  type StudioWidgetId,
} from "./types";

const UNIT_OPTIONS: readonly StudioUnit[] = ["$", "%", "₫"];
const LEGEND_OPTIONS: ReadonlyArray<{ value: StudioLegendPos; label: string }> = [
  { value: "bottom", label: "Dưới biểu đồ" },
  { value: "right", label: "Bên phải" },
  { value: "hidden", label: "Ẩn" },
];

const VIEW_BADGE_MAP: Record<Exclude<StudioControlView, "general">, string> = {
  chart: "Cài đặt Biểu đồ",
  kpi: "Cài đặt Thẻ KPI",
  sharebar: "Cài đặt Share-Bar",
};

const InspectorBreadcrumbHeader: React.FC<{
  view: Exclude<StudioControlView, "general">;
  onBack: () => void;
}> = ({ view, onBack }) => {
  const { t } = useTranslation();
  return (
    <div style={{ position: "sticky", top: 0, zIndex: 4, display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", padding: "8px 12px", backgroundColor: "var(--bg-subtle)", borderBottom: "1px solid var(--border-default)" }}>
      <button
        type="button"
        data-testid="studio-inspector-back-btn"
        onClick={onBack}
        style={{ display: "inline-flex", alignItems: "center", gap: "5px", border: "none", background: "transparent", color: "var(--action-primary)", fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer", padding: "2px 4px" }}
      >
        <BrandIcon name="arrow-left" size={ICON_SM} color="currentColor" />
        <span>{t("deckGen.generalDataBack", "Dữ liệu chung")}</span>
      </button>
      <span data-testid="studio-inspector-badge" style={{ padding: "2px 8px", borderRadius: "999px", backgroundColor: "var(--bg-selected)", color: "var(--action-primary)", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
        {VIEW_BADGE_MAP[view]}
      </span>
    </div>
  );
};

const ChartAxisAndTargetFields: React.FC<{
  snapshot: StudioSnapshot;
  onPatch: (patch: Partial<StudioSnapshot>) => void;
  onToggleWidget: (id: StudioWidgetId) => void;
}> = ({ snapshot, onPatch, onToggleWidget }) => (
  <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
    <span style={SECTION_TITLE_STYLE}>Trục, Đơn vị &amp; Mục tiêu</span>
    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 64px", gap: "6px" }}>
      <input type="text" aria-label="Nhãn trục X" value={snapshot.xAxisLabel} onChange={(e) => onPatch({ xAxisLabel: e.target.value })} placeholder="Trục X" style={STUDIO_COMPACT_INPUT} />
      <input type="text" aria-label="Nhãn trục Y" value={snapshot.yAxisLabel} onChange={(e) => onPatch({ yAxisLabel: e.target.value })} placeholder="Trục Y" style={STUDIO_COMPACT_INPUT} />
      <select aria-label="Đơn vị" data-testid="studio-unit-select" value={snapshot.unit} onChange={(e) => onPatch({ unit: e.target.value as StudioUnit })} style={STUDIO_COMPACT_INPUT}>
        {UNIT_OPTIONS.map((u) => <option key={u} value={u}>{u}</option>)}
      </select>
    </div>
    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px" }}>
      <label style={{ display: "flex", flexDirection: "column", gap: "2px", fontSize: "11px", color: "var(--text-secondary)" }}>
        <span>Ngưỡng mục tiêu</span>
        <input type="number" data-testid="studio-target-input" value={snapshot.targetValue} onChange={(e) => onPatch({ targetValue: Number(e.target.value) || 0 })} style={STUDIO_COMPACT_INPUT} />
      </label>
      <label style={{ display: "flex", flexDirection: "column", gap: "2px", fontSize: "11px", color: "var(--text-secondary)" }}>
        <span>Vị trí Legend</span>
        <select data-testid="studio-legend-select" value={snapshot.legendPos} onChange={(e) => onPatch({ legendPos: e.target.value as StudioLegendPos })} style={STUDIO_COMPACT_INPUT}>
          {LEGEND_OPTIONS.map((opt) => <option key={opt.value} value={opt.value}>{opt.label}</option>)}
        </select>
      </label>
    </div>
    <label style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "11.5px", color: "var(--text-secondary)", cursor: "pointer" }}>
      <input type="checkbox" data-testid="studio-widget-chip-targetLine" checked={snapshot.widgets.targetLine} onChange={() => onToggleWidget("targetLine")} />
      <span>Hiển thị đường mục tiêu trên biểu đồ</span>
    </label>
    <label style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "11.5px", color: "var(--text-secondary)", cursor: "pointer" }}>
      <input type="checkbox" data-testid="studio-comparison-toggle" checked={snapshot.showComparison} onChange={(e) => onPatch({ showComparison: e.target.checked })} />
      <span>Hiển thị chuỗi so sánh (vs kỳ trước)</span>
    </label>
  </div>
);

const ChartSpotlightAndPaletteFields: React.FC<{
  snapshot: StudioSnapshot;
  onPatch: (patch: Partial<StudioSnapshot>) => void;
}> = ({ snapshot, onPatch }) => {
  const defaultFocusId = snapshot.rows[0]?.id ?? null;
  const spotlightOn = Boolean(snapshot.spotlightRowId);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
      <span style={SECTION_TITLE_STYLE}>Bảng màu &amp; Chiếu điểm (Knaflic)</span>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: "5px" }}>
        {STUDIO_PALETTES.map((pal) => {
          const active = pal.id === snapshot.paletteId;
          return (
            <button key={pal.id} type="button" onClick={() => onPatch({ paletteId: pal.id })} style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "5px 8px", borderRadius: "var(--radius-xs)", border: active ? "1.5px solid var(--action-primary)" : "1px solid var(--border-default)", backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)", color: "var(--text-primary)", fontSize: "11px", cursor: "pointer" }}>
              <span>{pal.label}</span>
              <span style={{ display: "inline-flex", gap: "2px" }}>
                <span style={{ width: "8px", height: "8px", borderRadius: "999px", backgroundColor: pal.colors[0] }} />
                <span style={{ width: "8px", height: "8px", borderRadius: "999px", backgroundColor: pal.colors[1] }} />
              </span>
            </button>
          );
        })}
      </div>
      <label style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "11.5px", color: "var(--text-secondary)", cursor: "pointer" }}>
        <input type="checkbox" data-testid="studio-inspector-spotlight-toggle" checked={spotlightOn} onChange={(e) => onPatch({ spotlightRowId: e.target.checked ? defaultFocusId : null })} />
        <span>Bật Chiếu điểm (Grey-out chuỗi nền)</span>
      </label>
      {spotlightOn && (
        <select aria-label="Chuỗi chiếu điểm" data-testid="studio-inspector-spotlight-select" value={snapshot.spotlightRowId ?? ""} onChange={(e) => onPatch({ spotlightRowId: e.target.value || null })} style={STUDIO_COMPACT_INPUT}>
          {snapshot.rows.map((r) => <option key={r.id} value={r.id}>Chiếu điểm: {r.label}</option>)}
        </select>
      )}
    </div>
  );
};

const ChartInspectorBody: React.FC<{
  snapshot: StudioSnapshot;
  onPatch: (patch: Partial<StudioSnapshot>) => void;
  onToggleWidget: (id: StudioWidgetId) => void;
}> = ({ snapshot, onPatch, onToggleWidget }) => (
  <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
    <label style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
      <span style={SECTION_TITLE_STYLE}>Dạng biểu đồ (10 mẫu chuẩn)</span>
      <select data-testid="studio-inspector-chart-select" value={snapshot.chartType} onChange={(e) => onPatch({ chartType: e.target.value as StudioChartType })} style={STUDIO_INPUT_STYLE}>
        {STUDIO_CHART_OPTIONS.map((opt) => (
          <option key={opt.id} value={opt.id}>{opt.groupLabel} · {opt.label}</option>
        ))}
      </select>
    </label>
    <ChartAxisAndTargetFields snapshot={snapshot} onPatch={onPatch} onToggleWidget={onToggleWidget} />
    <ChartSpotlightAndPaletteFields snapshot={snapshot} onPatch={onPatch} />
  </div>
);

const KpiInspectorBody: React.FC<{
  snapshot: StudioSnapshot;
  onPatch: (patch: Partial<StudioSnapshot>) => void;
}> = ({ snapshot, onPatch }) => (
  <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
    <label style={{ display: "flex", flexDirection: "column", gap: "3px" }}>
      <span style={SECTION_TITLE_STYLE}>Action Title / Tiêu đề chính</span>
      <input type="text" data-testid="deck-gen-title-input" value={snapshot.title} onChange={(e) => onPatch({ title: e.target.value })} placeholder="VD: Direct dẫn đầu tăng trưởng Q4 (+20%)" style={STUDIO_INPUT_STYLE} />
    </label>
    <label style={{ display: "flex", flexDirection: "column", gap: "3px" }}>
      <span style={SECTION_TITLE_STYLE}>Phụ đề giải thích</span>
      <input type="text" data-testid="studio-inspector-subtitle-input" value={snapshot.subtitle} onChange={(e) => onPatch({ subtitle: e.target.value })} style={STUDIO_INPUT_STYLE} />
    </label>
    <label style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "11.5px", color: "var(--text-secondary)", cursor: "pointer" }}>
      <input type="checkbox" data-testid="studio-kpi-tick-toggle" checked={snapshot.kpiShowTargetTick} onChange={(e) => onPatch({ kpiShowTargetTick: e.target.checked })} />
      <span>Hiển thị Target Tick Bar (phong cách 2026)</span>
    </label>
    <label style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "11.5px", color: "var(--text-secondary)", cursor: "pointer" }}>
      <input type="checkbox" data-testid="studio-kpi-delta-toggle" checked={snapshot.kpiShowDelta} onChange={(e) => onPatch({ kpiShowDelta: e.target.checked })} />
      <span>Hiển thị chênh lệch so với kỳ trước (Delta %)</span>
    </label>
  </div>
);

const ShareBarInspectorBody: React.FC<{
  snapshot: StudioSnapshot;
  onPatch: (patch: Partial<StudioSnapshot>) => void;
}> = ({ snapshot, onPatch }) => (
  <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
    <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
      <span style={SECTION_TITLE_STYLE}>Thứ tự xếp hạng</span>
      <div style={SEGMENT_GROUP_STYLE} role="radiogroup" aria-label="Thứ tự Share-Bar">
        <button type="button" role="radio" aria-checked={snapshot.shareBarSort === "desc"} data-testid="studio-sharebar-sort-desc" onClick={() => onPatch({ shareBarSort: "desc" })} style={segmentPillStyle(snapshot.shareBarSort === "desc")}>
          Cao → Thấp
        </button>
        <button type="button" role="radio" aria-checked={snapshot.shareBarSort === "asc"} data-testid="studio-sharebar-sort-asc" onClick={() => onPatch({ shareBarSort: "asc" })} style={segmentPillStyle(snapshot.shareBarSort === "asc")}>
          Thấp → Cao
        </button>
      </div>
    </div>
    <label style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "11.5px", color: "var(--text-secondary)", cursor: "pointer" }}>
      <input type="checkbox" data-testid="studio-sharebar-pct-toggle" checked={snapshot.shareBarShowPct} onChange={(e) => onPatch({ shareBarShowPct: e.target.checked })} />
      <span>Hiển thị tỷ trọng phần trăm (%)</span>
    </label>
    <label style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "11.5px", color: "var(--text-secondary)", cursor: "pointer" }}>
      <input type="checkbox" data-testid="studio-sharebar-prior-toggle" checked={snapshot.shareBarShowPrior} onChange={(e) => onPatch({ shareBarShowPrior: e.target.checked })} />
      <span>Hiển thị mức tăng/giảm so với kỳ trước</span>
    </label>
  </div>
);

export const StudioComponentInspector: React.FC<{
  view: Exclude<StudioControlView, "general">;
  snapshot: StudioSnapshot;
  onBack: () => void;
  onPatch: (patch: Partial<StudioSnapshot>) => void;
  onToggleWidget: (id: StudioWidgetId) => void;
}> = ({ view, snapshot, onBack, onPatch, onToggleWidget }) => (
  <div data-testid="studio-control-inspector" style={{ display: "flex", flexDirection: "column", minHeight: 0 }}>
    <InspectorBreadcrumbHeader view={view} onBack={onBack} />
    <div style={{ padding: "12px", display: "flex", flexDirection: "column", gap: "12px" }}>
      {view === "chart" && <ChartInspectorBody snapshot={snapshot} onPatch={onPatch} onToggleWidget={onToggleWidget} />}
      {view === "kpi" && <KpiInspectorBody snapshot={snapshot} onPatch={onPatch} />}
      {view === "sharebar" && <ShareBarInspectorBody snapshot={snapshot} onPatch={onPatch} />}
    </div>
  </div>
);
