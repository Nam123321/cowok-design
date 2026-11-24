// @story 06.5
// @implements AC-03, AC-04 (Detail Canvas Widgets: Honest States, KPI Strip with 2026 Target Tick Bar, Ranked Share-Bar, Inline Text Edit & Pins)
import React, { useState } from "react";
import { BrandIcon } from "../../../../components/ui/BrandIcon";
import { useTranslation } from "../../../../hooks/useTranslation";
import { computeDataSalience, formatMetricValue } from "./studioDataUtils";
import { computeRowDeltaPct } from "./studioStorytellingUtils";
import {
  FONT_WEIGHT_BOLD,
  FONT_WEIGHT_SEMIBOLD,
  STUDIO_COMPACT_INPUT,
  STUDIO_SECONDARY_BTN,
} from "./studioStyles";
import {
  getPaletteColors,
  type DataSalienceCheck,
  type StudioCommentPin,
  type StudioDataRow,
  type StudioInteractionMode,
  type StudioShareSort,
  type StudioSnapshot,
  type StudioTimeframe,
  type StudioViewState,
} from "./types";

const PERCENT_SCALE = 100;
const Z_INDEX_BADGE = 5;
const Z_INDEX_PIN = 12;
const ICON_SIZE_EMPTY = 24;
const POPOVER_META_OPACITY = 0.85;
const TICK_CAP_PCT = 100;
const TICK_MARKER_PCT = 78;
const SKELETON_KEYS = ["sk-1", "sk-2", "sk-3"] as const;
const TIMEFRAME_OPTIONS: readonly StudioTimeframe[] = ["Q4 2026", "Q3 2026", "YTD 2026"];

const SALIENCE_TONE_STYLES: Record<DataSalienceCheck["tone"], { color: string; bg: string }> = {
  success: { color: "var(--state-success)", bg: "var(--state-success-soft)" },
  warning: { color: "var(--state-warning)", bg: "var(--state-warning-soft)" },
  danger: { color: "var(--state-danger)", bg: "var(--state-danger-soft)" },
};

export function selectableBoxStyle(isSelected: boolean): React.CSSProperties {
  return { position: "relative", borderRadius: "var(--radius-sm)", outline: isSelected ? "2px solid var(--action-primary)" : "none", outlineOffset: "2px" };
}

export const SelectionTagBadge: React.FC<{ ffId: string; active: boolean }> = ({ ffId, active }) => {
  if (!active) return null;
  return (
    <span data-testid={`studio-selection-badge-${ffId}`} style={{ position: "absolute", top: "-10px", left: "8px", zIndex: Z_INDEX_BADGE, backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", padding: "1px 6px", borderRadius: "var(--radius-xs)", fontSize: "9.5px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
      {ffId}
    </span>
  );
};

export const CanvasNonDefaultState: React.FC<{
  viewState: Exclude<StudioViewState, "DEFAULT">;
  onResetDefault: () => void;
}> = ({ viewState, onResetDefault }) => {
  const { t } = useTranslation();
  if (viewState === "LOADING") {
    return (
      <div data-testid="studio-state-loading" style={{ padding: "24px", display: "flex", flexDirection: "column", gap: "12px" }}>
        <div style={{ height: "20px", width: "45%", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-subtle)" }} />
        <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: "10px" }}>
          {SKELETON_KEYS.map((k) => <div key={k} style={{ height: "64px", borderRadius: "var(--radius-sm)", backgroundColor: "var(--bg-subtle)" }} />)}
        </div>
        <div style={{ height: "180px", borderRadius: "var(--radius-sm)", backgroundColor: "var(--bg-subtle)" }} />
      </div>
    );
  }
  const isError = viewState === "ERROR";
  return (
    <div data-testid={isError ? "studio-state-error" : "studio-state-empty"} style={{ padding: "36px 20px", textAlign: "center", display: "flex", flexDirection: "column", alignItems: "center", gap: "10px" }}>
      <BrandIcon name={isError ? "warning" : "file-text"} size={ICON_SIZE_EMPTY} color={isError ? "var(--state-danger)" : "var(--text-muted)"} />
      <div style={{ fontSize: "13px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
        {isError ? t("deckGen.stateErrorTitle", "Không thể tải dữ liệu biểu đồ từ nguồn ngoài") : t("deckGen.stateEmptyTitle", "Chưa có dữ liệu hiển thị cho Dashboard")}
      </div>
      <div style={{ fontSize: "12px", color: "var(--text-secondary)" }}>
        {isError ? t("deckGen.stateErrorDesc", "Kiểm tra lại định dạng tệp CSV/PDF hoặc chuyển về trạng thái mặc định.") : t("deckGen.stateEmptyDesc", "Tải tệp CSV/TSV/PDF hoặc thêm dòng trong bảng bên trái để bắt đầu.")}
      </div>
      <button type="button" onClick={onResetDefault} style={STUDIO_SECONDARY_BTN}>Quay về trạng thái DEFAULT</button>
    </div>
  );
};

const InlineEditableText: React.FC<{
  testId: string;
  value: string;
  interactionMode: StudioInteractionMode;
  style: React.CSSProperties;
  onCommit: (nextVal: string) => void;
}> = ({ testId, value, interactionMode, style, onCommit }) => {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  const beginEdit = () => { setDraft(value); setEditing(true); };
  const finishEdit = () => { setEditing(false); if (draft.trim() && draft !== value) onCommit(draft.trim()); };

  if (editing) {
    return (
      <input autoFocus data-testid={`${testId}-input`} value={draft} onChange={(e) => setDraft(e.target.value)} onBlur={finishEdit} onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); finishEdit(); } if (e.key === "Escape") setEditing(false); }} style={{ ...STUDIO_COMPACT_INPUT, ...style }} />
    );
  }
  return (
    <div data-testid={testId} onClick={(e) => { if (interactionMode === "text") { e.stopPropagation(); beginEdit(); } }} onDoubleClick={(e) => { e.stopPropagation(); beginEdit(); }} title={interactionMode === "text" ? "Nhấp để sửa trực tiếp" : "Nhấp đúp để sửa chữ"} style={{ ...style, cursor: interactionMode === "text" ? "text" : "inherit" }}>
      {value}
    </div>
  );
};

export const CanvasDashboardHeader: React.FC<{
  snapshot: StudioSnapshot;
  interactionMode: StudioInteractionMode;
  selectedFfId: string | null;
  onSelectElement: (ffId: string) => void;
  onCommitPatch: (patch: Partial<StudioSnapshot>) => void;
}> = ({ snapshot, interactionMode, selectedFfId, onSelectElement, onCommitPatch }) => {
  const salience = computeDataSalience(snapshot.rows, snapshot.chartType, snapshot.unit);
  const toneStyle = SALIENCE_TONE_STYLES[salience.tone];
  const isCeo = snapshot.audienceMode === "ceo";
  const isWhatIf = Boolean(snapshot.whatIfRowId && snapshot.whatIfDeltaPct !== 0);

  return (
    <div data-ff-id="dashboard-header" onClick={() => interactionMode === "select" && onSelectElement("dashboard-header")} style={{ ...selectableBoxStyle(selectedFfId === "dashboard-header"), display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: "10px", flexWrap: "wrap", padding: "4px" }}>
      <SelectionTagBadge ffId="dashboard-header" active={selectedFfId === "dashboard-header"} />
      <div style={{ flex: 1, minWidth: "180px" }}>
        <InlineEditableText testId="studio-canvas-title" value={snapshot.title} interactionMode={interactionMode} style={{ fontSize: "16px", fontWeight: FONT_WEIGHT_BOLD }} onCommit={(title) => onCommitPatch({ title })} />
        <InlineEditableText testId="studio-canvas-subtitle" value={snapshot.subtitle} interactionMode={interactionMode} style={{ fontSize: "12px", color: "var(--text-secondary)", marginTop: "2px" }} onCommit={(subtitle) => onCommitPatch({ subtitle })} />
      </div>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "6px", flexWrap: "wrap" }}>
        {isWhatIf && (
          <span data-testid="studio-whatif-badge" style={{ padding: "3px 8px", borderRadius: "999px", fontSize: "10.5px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--state-warning)", backgroundColor: "var(--state-warning-soft)", border: "1px solid var(--state-warning)" }}>
            Kịch bản giả lập ({snapshot.whatIfDeltaPct > 0 ? "+" : ""}{snapshot.whatIfDeltaPct}%)
          </span>
        )}
        <span data-testid="studio-salience-badge" style={{ padding: "3px 8px", borderRadius: "999px", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: toneStyle.color, backgroundColor: toneStyle.bg, border: `1px solid ${toneStyle.color}` }}>
          {salience.label}
        </span>
        {!isCeo && snapshot.widgets.timeframe && (
          <select data-testid="studio-timeframe-select" value={snapshot.timeframe} onChange={(e) => onCommitPatch({ timeframe: e.target.value as StudioTimeframe })} style={{ ...STUDIO_COMPACT_INPUT, width: "auto" }}>
            {TIMEFRAME_OPTIONS.map((tf) => <option key={tf} value={tf}>{tf}</option>)}
          </select>
        )}
      </div>
    </div>
  );
};

const KpiTargetTickBar: React.FC<{ ratioPct: number }> = ({ ratioPct }) => (
  <div data-testid="studio-kpi-tick-bar" style={{ position: "relative", height: "5px", borderRadius: "999px", backgroundColor: "var(--bg-subtle)", marginTop: "6px" }}>
    <div style={{ height: "100%", width: `${Math.min(TICK_CAP_PCT, Math.max(0, ratioPct))}%`, borderRadius: "999px", backgroundColor: ratioPct >= TICK_CAP_PCT ? "var(--state-success)" : "var(--action-primary)" }} />
    <span style={{ position: "absolute", top: "-2px", left: `${TICK_MARKER_PCT}%`, width: "2px", height: "9px", backgroundColor: "var(--text-primary)", borderRadius: "1px" }} />
  </div>
);

export const CanvasKpiStrip: React.FC<{
  snapshot: StudioSnapshot;
  selectedFfId: string | null;
  onSelectElement: (ffId: string) => void;
}> = ({ snapshot, selectedFfId, onSelectElement }) => {
  if (!snapshot.widgets.kpiStrip) return null;
  const totalCur = snapshot.rows.reduce((a, r) => a + r.current, 0);
  const totalPrev = snapshot.rows.reduce((a, r) => a + r.previous, 0);
  const growth = totalPrev > 0 ? Math.round(((totalCur - totalPrev) / totalPrev) * PERCENT_SCALE) : 0;
  const avg = snapshot.rows.length > 0 ? Math.round(totalCur / snapshot.rows.length) : 0;
  const targetRatio = snapshot.targetValue > 0 ? Math.round((avg / snapshot.targetValue) * PERCENT_SCALE) : PERCENT_SCALE;
  const cards = [
    { id: "kpi-revenue-card", label: `Tổng ${snapshot.yAxisLabel}`, value: formatMetricValue(totalCur, snapshot.unit), meta: `${snapshot.rows.length} kênh hoạt động`, tickPct: targetRatio },
    { id: "kpi-growth-card", label: "Tăng trưởng kỳ trước", value: `${growth >= 0 ? "+" : ""}${growth}%`, meta: `Kỳ trước: ${formatMetricValue(totalPrev, snapshot.unit)}`, tickPct: Math.max(0, Math.min(TICK_CAP_PCT, growth + TICK_MARKER_PCT)) },
    { id: "kpi-target-card", label: "Đạt mục tiêu TB", value: `${targetRatio}%`, meta: `TB/kênh ${formatMetricValue(avg, snapshot.unit)} vs ${formatMetricValue(snapshot.targetValue, snapshot.unit)}`, tickPct: targetRatio },
  ];

  return (
    <div data-testid="studio-kpi-strip" style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(140px, 1fr))", gap: "8px" }}>
      {cards.map((c) => {
        const active = selectedFfId === c.id;
        return (
          <div key={c.id} data-ff-id={c.id} data-testid={`studio-card-${c.id}`} onClick={(e) => { e.stopPropagation(); onSelectElement(c.id); }} style={{ ...selectableBoxStyle(active), padding: "10px 12px", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-default)", cursor: "pointer" }}>
            <SelectionTagBadge ffId={c.id} active={active} />
            <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>{c.label}</div>
            <div style={{ fontSize: "17px", fontWeight: FONT_WEIGHT_BOLD, marginTop: "2px" }}>{c.value}</div>
            {snapshot.kpiShowDelta && <div style={{ fontSize: "10.5px", color: "var(--text-secondary)", marginTop: "2px" }}>{c.meta}</div>}
            {snapshot.kpiShowTargetTick && <KpiTargetTickBar ratioPct={c.tickPct} />}
          </div>
        );
      })}
    </div>
  );
};

export const CanvasShareBarPanel: React.FC<{
  rows: readonly StudioDataRow[];
  paletteId: StudioSnapshot["paletteId"];
  unit: StudioSnapshot["unit"];
  sortOrder: StudioShareSort;
  showPct: boolean;
  showPrior: boolean;
  highlightedRowId: string | null;
  selectedFfId: string | null;
  onSelectElement: (ffId: string) => void;
  onToggleRowHighlight: (rowId: string) => void;
}> = ({ rows, paletteId, unit, sortOrder, showPct, showPrior, highlightedRowId, selectedFfId, onSelectElement, onToggleRowHighlight }) => {
  const colors = getPaletteColors(paletteId);
  const total = rows.reduce((a, r) => a + Math.max(0, r.current), 0);
  const ranked = [...rows].sort((a, b) => (sortOrder === "asc" ? a.current - b.current : b.current - a.current));
  const active = selectedFfId === "ranked-share-bar";

  return (
    <div data-ff-id="ranked-share-bar" data-testid="studio-share-bar-panel" onClick={() => onSelectElement("ranked-share-bar")} style={{ ...selectableBoxStyle(active), padding: "10px 12px", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-default)", display: "flex", flexDirection: "column", gap: "6px" }}>
      <SelectionTagBadge ffId="ranked-share-bar" active={active} />
      <div style={{ fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-secondary)" }}>Xếp hạng tỷ trọng (Share-Bar)</div>
      {ranked.map((r, i) => {
        const pct = total > 0 ? Math.round((Math.max(0, r.current) / total) * PERCENT_SCALE) : 0;
        const delta = computeRowDeltaPct(r);
        const isHl = highlightedRowId === r.id;
        return (
          <button key={r.id} type="button" data-testid={`studio-share-row-${r.id}`} onClick={(e) => { e.stopPropagation(); onToggleRowHighlight(r.id); }} style={{ textAlign: "left", border: "none", padding: "4px 6px", borderRadius: "var(--radius-xs)", backgroundColor: isHl ? "var(--bg-selected)" : "transparent", color: "var(--text-primary)", cursor: "pointer" }}>
            <div style={{ display: "flex", justifyContent: "space-between", fontSize: "11px", marginBottom: "3px", gap: "4px" }}>
              <span style={{ fontWeight: isHl ? FONT_WEIGHT_BOLD : FONT_WEIGHT_SEMIBOLD }}>{r.label}</span>
              <span style={{ color: "var(--text-secondary)" }}>
                {formatMetricValue(r.current, unit)}
                {showPct ? ` (${pct}%)` : ""}
                {showPrior ? ` · ${delta >= 0 ? "+" : ""}${delta}%` : ""}
              </span>
            </div>
            <div style={{ height: "6px", borderRadius: "999px", backgroundColor: "var(--bg-subtle)", overflow: "hidden" }}>
              <div style={{ height: "100%", width: `${pct}%`, backgroundColor: colors[i % colors.length], borderRadius: "999px" }} />
            </div>
          </button>
        );
      })}
    </div>
  );
};

export const CanvasFootnoteBar: React.FC<{
  footnote: string;
  interactionMode: StudioInteractionMode;
  selectedFfId: string | null;
  onSelectElement: (ffId: string) => void;
  onCommitFootnote: (next: string) => void;
}> = ({ footnote, interactionMode, selectedFfId, onSelectElement, onCommitFootnote }) => (
  <div data-ff-id="dashboard-footnote" onClick={() => interactionMode === "select" && onSelectElement("dashboard-footnote")} style={{ ...selectableBoxStyle(selectedFfId === "dashboard-footnote"), padding: "6px 8px", borderTop: "1px solid var(--border-subtle)" }}>
    <SelectionTagBadge ffId="dashboard-footnote" active={selectedFfId === "dashboard-footnote"} />
    <InlineEditableText testId="studio-canvas-footnote" value={footnote} interactionMode={interactionMode} style={{ fontSize: "11px", color: "var(--text-muted)" }} onCommit={onCommitFootnote} />
  </div>
);

export const CanvasPinsOverlay: React.FC<{
  pins: readonly StudioCommentPin[];
  activePinId: string | null;
  onSelectPin: (id: string | null) => void;
  onUpdatePinText: (id: string, text: string) => void;
  onDeletePin: (id: string) => void;
  onApplyPinToChat: (pin: StudioCommentPin) => void;
}> = ({ pins, activePinId, onSelectPin, onUpdatePinText, onDeletePin, onApplyPinToChat }) => (
  <>
    {pins.map((pin) => {
      const open = activePinId === pin.id;
      return (
        <div key={pin.id} style={{ position: "absolute", left: `${pin.xPct}%`, top: `${pin.yPct}%`, transform: "translate(-50%, -50%)", zIndex: Z_INDEX_PIN }}>
          <button type="button" data-testid={`studio-comment-pin-${pin.num}`} onClick={(e) => { e.stopPropagation(); onSelectPin(open ? null : pin.id); }} style={{ width: "22px", height: "22px", borderRadius: "999px", border: "2px solid var(--bg-surface)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD, cursor: "pointer" }}>
            {pin.num}
          </button>
          {open && (
            <div data-testid={`studio-pin-popover-${pin.num}`} onClick={(e) => e.stopPropagation()} style={{ position: "absolute", top: "26px", left: "0", width: "210px", padding: "8px", borderRadius: "var(--radius-sm)", backgroundColor: "var(--bg-elevated, var(--bg-surface))", color: "var(--text-primary)", border: "1px solid var(--border-strong)", boxShadow: "var(--shadow-lg)", display: "flex", flexDirection: "column", gap: "6px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", fontSize: "10.5px", opacity: POPOVER_META_OPACITY }}>
                <span>@human · Ghim #{pin.num}</span>
                <button type="button" data-testid={`studio-pin-delete-${pin.num}`} onClick={() => onDeletePin(pin.id)} style={{ border: "none", background: "transparent", color: "var(--text-secondary)", cursor: "pointer", fontSize: "11px" }}>Xóa</button>
              </div>
              <input type="text" data-testid={`studio-pin-input-${pin.num}`} value={pin.text} onChange={(e) => onUpdatePinText(pin.id, e.target.value)} placeholder="Nhập ghi chú ghim..." style={{ width: "100%", boxSizing: "border-box", padding: "4px 6px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-strong)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)" }} />
              <button type="button" data-testid={`studio-pin-apply-${pin.num}`} onClick={() => onApplyPinToChat(pin)} style={{ border: "none", borderRadius: "var(--radius-xs)", padding: "4px 6px", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer" }}>
                Gửi vào Sub-Chat
              </button>
            </div>
          )}
        </div>
      );
    })}
  </>
);
