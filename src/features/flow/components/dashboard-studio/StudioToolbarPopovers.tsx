// @story 06.5
// @implements AC-02, AC-04 (Compact Toolbar Popovers: 10-Shape Visual Chart Picker, Palette Selector & Honest-Data QA Audit)
import React from "react";
import { BrandIcon } from "../../../../components/ui/BrandIcon";
import { useTranslation } from "../../../../hooks/useTranslation";
import { FONT_WEIGHT_BOLD, FONT_WEIGHT_MEDIUM, FONT_WEIGHT_SEMIBOLD } from "./studioStyles";
import {
  STUDIO_CHART_GROUPS,
  STUDIO_CHART_OPTIONS,
  STUDIO_PALETTES,
  type StudioChartType,
  type StudioPaletteId,
  type StudioQaIssue,
} from "./types";

const Z_POPOVER = 35;
const ICON_SM = 12;
const CHEVRON_SM = 10;
const GLYPH_SIZE = 14;
const GLYPH_STROKE = 1.5;
const HINT_LINE_HEIGHT = 1.25;

const POPOVER_SURFACE_STYLE: React.CSSProperties = {
  position: "absolute",
  top: "calc(100% + 4px)",
  right: 0,
  zIndex: Z_POPOVER,
  backgroundColor: "var(--bg-elevated, var(--bg-surface))",
  color: "var(--text-primary)",
  border: "1px solid var(--border-strong)",
  borderRadius: "var(--radius-sm)",
  boxShadow: "var(--shadow-lg)",
  padding: "8px",
};

const COMPACT_TRIGGER_STYLE = (active: boolean): React.CSSProperties => ({
  height: "24px",
  padding: "0 7px",
  borderRadius: "4px",
  border: active ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
  backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)",
  color: active ? "var(--action-primary)" : "var(--text-primary)",
  fontSize: "10.5px",
  fontWeight: active ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
  display: "inline-flex",
  alignItems: "center",
  gap: "4px",
  cursor: "pointer",
  whiteSpace: "nowrap",
  flexShrink: 0,
});

export const ChartMiniGlyph: React.FC<{ type: StudioChartType }> = ({ type }) => {
  const base = { width: GLYPH_SIZE, height: GLYPH_SIZE, viewBox: "0 0 16 16", fill: "none", stroke: "currentColor", strokeWidth: GLYPH_STROKE };
  if (type === "line") return <svg {...base}><polyline points="2,12 6,7 10,9 14,3" /></svg>;
  if (type === "sparkline") return <svg {...base}><polyline points="1,10 4,6 8,11 11,5 15,7" /></svg>;
  if (type === "pie") return <svg {...base}><circle cx="8" cy="8" r="5.5" /><path d="M8 2.5V8H13.5" /></svg>;
  if (type === "segmented") return <svg {...base}><rect x="1.5" y="5.5" width="5" height="5" fill="currentColor" /><rect x="7.5" y="5.5" width="4" height="5" /><rect x="12.5" y="5.5" width="2" height="5" /></svg>;
  if (type === "bullet") return <svg {...base}><rect x="2" y="5" width="12" height="6" rx="1" /><line x1="10" y1="3" x2="10" y2="13" /><rect x="2" y="6.8" width="7" height="2.4" fill="currentColor" /></svg>;
  if (type === "tick-bar") return <svg {...base}><rect x="2" y="6" width="12" height="4" rx="1" /><rect x="2" y="6" width="8" height="4" rx="1" fill="currentColor" /><line x1="11" y1="3" x2="11" y2="13" /></svg>;
  if (type === "movement") return <svg {...base}><line x1="8" y1="2" x2="8" y2="14" /><rect x="8" y="4" width="5" height="3" fill="currentColor" /><rect x="3" y="9" width="5" height="3" /></svg>;
  if (type === "kpi") return <svg {...base}><rect x="2" y="3" width="12" height="10" rx="1.5" /><line x1="4.5" y1="7" x2="11.5" y2="7" /><line x1="4.5" y1="10" x2="8.5" y2="10" /></svg>;
  if (type === "heatmap") return <svg {...base}><rect x="2" y="2" width="5" height="5" fill="currentColor" /><rect x="9" y="2" width="5" height="5" /><rect x="2" y="9" width="5" height="5" /><rect x="9" y="9" width="5" height="5" fill="currentColor" /></svg>;
  return <svg {...base}><rect x="2" y="8" width="3" height="6" fill="currentColor" /><rect x="6.5" y="4" width="3" height="10" fill="currentColor" /><rect x="11" y="6" width="3" height="8" /></svg>;
};

export const ChartPickerPopover: React.FC<{
  chartType: StudioChartType;
  open: boolean;
  onToggle: () => void;
  onSelect: (type: StudioChartType) => void;
}> = ({ chartType, open, onToggle, onSelect }) => {
  const { t } = useTranslation();
  const activeOption = STUDIO_CHART_OPTIONS.find((o) => o.id === chartType) ?? STUDIO_CHART_OPTIONS[0];
  return (
    <div style={{ position: "relative" }}>
      <button type="button" data-testid="studio-chart-picker-btn" aria-expanded={open} onClick={onToggle} style={COMPACT_TRIGGER_STYLE(open)}>
        <ChartMiniGlyph type={chartType} />
        <span>{t("deckGen.chartPickerPrefix", "Biểu đồ:")} {activeOption.label}</span>
        <BrandIcon name="chevron-down" size={CHEVRON_SM} color="currentColor" />
      </button>
      {open && (
        <div data-testid="studio-chart-picker-popover" style={{ ...POPOVER_SURFACE_STYLE, width: "330px", maxHeight: "340px", overflowY: "auto", display: "flex", flexDirection: "column", gap: "8px" }}>
          {STUDIO_CHART_GROUPS.map((grp) => (
            <div key={grp.id} style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
              <div style={{ fontSize: "10px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.04em" }}>{grp.label}</div>
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "4px" }}>
                {STUDIO_CHART_OPTIONS.filter((o) => o.group === grp.id).map((opt) => {
                  const selected = opt.id === chartType;
                  return (
                    <button key={opt.id} type="button" role="radio" aria-checked={selected} data-testid={`studio-chart-type-${opt.id}`} onClick={() => onSelect(opt.id)} style={{ textAlign: "left", padding: "6px 7px", borderRadius: "var(--radius-xs)", border: selected ? "1.5px solid var(--action-primary)" : "1px solid var(--border-default)", backgroundColor: selected ? "var(--bg-selected)" : "var(--bg-workspace)", color: "var(--text-primary)", cursor: "pointer", display: "flex", flexDirection: "column", gap: "2px" }}>
                      <div style={{ display: "flex", alignItems: "center", gap: "5px", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: selected ? "var(--action-primary)" : "var(--text-primary)" }}>
                        <ChartMiniGlyph type={opt.id} />
                        <span>{opt.label}</span>
                      </div>
                      <div style={{ fontSize: "9.5px", color: "var(--text-secondary)", lineHeight: HINT_LINE_HEIGHT }}>{opt.hint}</div>
                    </button>
                  );
                })}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export const PalettePickerPopover: React.FC<{
  paletteId: StudioPaletteId;
  open: boolean;
  onToggle: () => void;
  onSelect: (id: StudioPaletteId) => void;
}> = ({ paletteId, open, onToggle, onSelect }) => {
  const active = STUDIO_PALETTES.find((p) => p.id === paletteId) ?? STUDIO_PALETTES[0];
  return (
    <div style={{ position: "relative" }}>
      <button type="button" data-testid="studio-palette-picker-btn" aria-label="Bảng màu biểu đồ" aria-expanded={open} onClick={onToggle} title={`Bảng màu: ${active.label}`} style={COMPACT_TRIGGER_STYLE(open)}>
        <span style={{ width: "8px", height: "8px", borderRadius: "999px", backgroundColor: active.colors[0] }} />
        <span style={{ width: "8px", height: "8px", borderRadius: "999px", backgroundColor: active.colors[1] }} />
        <BrandIcon name="chevron-down" size={CHEVRON_SM} color="currentColor" />
      </button>
      {open && (
        <div data-testid="studio-palette-picker-popover" style={{ ...POPOVER_SURFACE_STYLE, width: "176px", display: "flex", flexDirection: "column", gap: "4px" }}>
          {STUDIO_PALETTES.map((pal) => {
            const isCur = pal.id === paletteId;
            return (
              <button key={pal.id} type="button" role="radio" aria-checked={isCur} data-testid={`studio-palette-${pal.id}`} onClick={() => onSelect(pal.id)} style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "5px 7px", borderRadius: "var(--radius-xs)", border: isCur ? "1px solid var(--action-primary)" : "1px solid transparent", backgroundColor: isCur ? "var(--bg-selected)" : "transparent", color: "var(--text-primary)", fontSize: "11px", cursor: "pointer" }}>
                <span>{pal.label}</span>
                <span style={{ display: "inline-flex", gap: "2px" }}>
                  <span style={{ width: "8px", height: "8px", borderRadius: "999px", backgroundColor: pal.colors[0] }} />
                  <span style={{ width: "8px", height: "8px", borderRadius: "999px", backgroundColor: pal.colors[1] }} />
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
};

export const QaAuditPopover: React.FC<{
  issues: readonly StudioQaIssue[];
  open: boolean;
  onToggle: () => void;
  onAutoFixAll: () => void;
}> = ({ issues, open, onToggle, onAutoFixAll }) => (
  <div style={{ position: "relative" }}>
    <button type="button" data-testid="studio-qa-audit-btn" aria-expanded={open} onClick={onToggle} style={{ ...COMPACT_TRIGGER_STYLE(open), color: issues.length > 0 ? "var(--state-warning)" : "var(--state-success)" }}>
      <BrandIcon name="sparkles" size={ICON_SM} color="currentColor" />
      <span>Chuẩn hóa</span>
      {issues.length > 0 && (
        <span data-testid="studio-qa-issue-count" style={{ padding: "0 4px", borderRadius: "999px", backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)", fontSize: "9.5px", fontWeight: FONT_WEIGHT_BOLD }}>
          {issues.length}
        </span>
      )}
    </button>
    {open && (
      <div data-testid="studio-qa-audit-popover" style={{ ...POPOVER_SURFACE_STYLE, width: "290px", display: "flex", flexDirection: "column", gap: "8px" }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
          <span>Kiểm định Dữ liệu Trung thực (QA)</span>
          <span style={{ fontSize: "10px", color: issues.length === 0 ? "var(--state-success)" : "var(--state-warning)" }}>
            {issues.length === 0 ? "Đạt chuẩn" : `${issues.length} gợi ý`}
          </span>
        </div>
        {issues.length === 0 ? (
          <div style={{ fontSize: "11px", color: "var(--state-success)", padding: "4px 0" }}>
            Biểu đồ đã đạt chuẩn Action Title, Chiếu điểm và trung thực số liệu.
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: "5px" }}>
            {issues.map((iss) => (
              <div key={iss.id} style={{ padding: "6px 7px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-subtle)", fontSize: "11px" }}>
                <div style={{ fontWeight: FONT_WEIGHT_SEMIBOLD, color: iss.severity === "danger" ? "var(--state-danger)" : "var(--state-warning)" }}>{iss.label}</div>
                <div style={{ color: "var(--text-secondary)", fontSize: "10.5px", marginTop: "2px" }}>{iss.detail}</div>
              </div>
            ))}
            <button type="button" data-testid="studio-qa-autofix-btn" onClick={onAutoFixAll} style={{ padding: "6px 10px", borderRadius: "var(--radius-xs)", border: "none", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer" }}>
              ✦ Sửa tất cả bằng 1 chạm
            </button>
          </div>
        )}
      </div>
    )}
  </div>
);
