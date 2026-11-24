// @story 06.5
// @implements AC-03, AC-04, AC-05 (AI 3-Lens Strip, 3-Act Narrative Pill, Contextual Point Actions & What-If Direct Manipulation)
import React from "react";
import { BrandIcon } from "../../../../components/ui/BrandIcon";
import { useTranslation } from "../../../../hooks/useTranslation";
import { formatMetricValue } from "./studioDataUtils";
import { computeRowDeltaPct } from "./studioStorytellingUtils";
import { FONT_WEIGHT_BOLD, FONT_WEIGHT_MEDIUM, FONT_WEIGHT_SEMIBOLD } from "./studioStyles";
import {
  STUDIO_AI_LENSES,
  type StudioAiLensId,
  type StudioDataRow,
  type StudioUnit,
} from "./types";

const ICON_XS = 11;
const WHATIF_BIG = 20;
const WHATIF_SMALL = 10;
const WHATIF_STEPS = [-WHATIF_BIG, -WHATIF_SMALL, WHATIF_SMALL, WHATIF_BIG] as const;
const WHATIF_LIMIT = 50;
const WHATIF_MIN = -WHATIF_LIMIT;
const WHATIF_MAX = WHATIF_LIMIT;
const WHATIF_STEP = 5;

const ELEVATED_PILL_BAR: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  alignItems: "center",
  justifyContent: "space-between",
  gap: "6px",
  padding: "7px 10px",
  borderRadius: "var(--radius-sm)",
  backgroundColor: "var(--bg-elevated, var(--bg-surface))",
  color: "var(--text-primary)",
  border: "1px solid var(--border-strong)",
  boxShadow: "var(--shadow-md)",
};

const ACTION_CHIP_STYLE = (active: boolean): React.CSSProperties => ({
  display: "inline-flex",
  alignItems: "center",
  gap: "4px",
  padding: "4px 8px",
  borderRadius: "var(--radius-xs)",
  border: active ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
  backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)",
  color: active ? "var(--action-primary)" : "var(--text-primary)",
  fontSize: "10.5px",
  fontWeight: active ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
  cursor: "pointer",
  whiteSpace: "nowrap",
});

export const AiLensStrip: React.FC<{
  activeLens: StudioAiLensId | null;
  onApplyLens: (lensId: StudioAiLensId) => void;
}> = ({ activeLens, onApplyLens }) => {
  const { t } = useTranslation();
  return (
    <div data-testid="studio-ai-lens-strip" style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", flexWrap: "wrap", marginBottom: "8px" }}>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "5px", flexWrap: "wrap" }}>
        <span style={{ display: "inline-flex", alignItems: "center", gap: "3px", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--action-primary)" }}>
          <BrandIcon name="sparkles" size={ICON_XS} color="currentColor" />
          <span>{t("deckGen.aiLensTitle", "3 Góc nhìn AI:")}</span>
        </span>
        {STUDIO_AI_LENSES.map((lens) => {
          const active = activeLens === lens.id;
          return (
            <button key={lens.id} type="button" data-testid={`studio-lens-${lens.id}`} aria-pressed={active} title={lens.summary} onClick={() => onApplyLens(lens.id)} style={{ padding: "3px 8px", borderRadius: "999px", border: active ? "1px solid var(--action-primary)" : "1px solid var(--border-default)", backgroundColor: active ? "var(--bg-selected)" : "var(--bg-surface)", color: active ? "var(--action-primary)" : "var(--text-secondary)", fontSize: "10.5px", fontWeight: active ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM, cursor: "pointer", whiteSpace: "nowrap" }}>
              {lens.label}
            </button>
          );
        })}
      </div>
      <span data-testid="studio-story-arc-pill" title="Cấu trúc 3 hồi khi xuất bộ Slide đa trang" style={{ padding: "2px 8px", borderRadius: "999px", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", fontSize: "10px", color: "var(--text-secondary)", whiteSpace: "nowrap" }}>
        {t("deckGen.storyArcPill", "Hồi 1: Bối cảnh → Hồi 2: Động lực → Hồi 3: Hành động")}
      </span>
    </div>
  );
};

export const WhatIfScenarioStrip: React.FC<{
  row: StudioDataRow;
  unit: StudioUnit;
  whatIfDeltaPct: number;
  isWhatIfActive: boolean;
  onApplyWhatIf: (rowId: string, deltaPct: number) => void;
  onRevertSource: () => void;
}> = ({ row, unit, whatIfDeltaPct, isWhatIfActive, onApplyWhatIf, onRevertSource }) => (
  <div data-testid="studio-whatif-bar" style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", flexWrap: "wrap", paddingTop: "6px", borderTop: "1px solid var(--border-subtle)", width: "100%" }}>
    <div style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "10.5px", color: "var(--text-secondary)" }}>
      <strong style={{ color: "var(--text-primary)" }}>Giả lập What-If ({row.label}):</strong>
      <span>{formatMetricValue(row.current, unit)}</span>
      <input
        type="range" min={WHATIF_MIN} max={WHATIF_MAX} step={WHATIF_STEP}
        aria-label={`Điều chỉnh What-If ${row.label}`}
        data-testid="studio-whatif-slider"
        value={isWhatIfActive ? whatIfDeltaPct : 0}
        onChange={(e) => onApplyWhatIf(row.id, Number(e.target.value))}
        style={{ width: "86px", cursor: "pointer" }}
      />
    </div>
    <div style={{ display: "inline-flex", alignItems: "center", gap: "4px", flexWrap: "wrap" }}>
      {WHATIF_STEPS.map((pct) => {
        const sign = pct > 0 ? "+" : "";
        const testSuffix = pct > 0 ? `plus${pct}` : `minus${Math.abs(pct)}`;
        return (
          <button key={pct} type="button" data-testid={`studio-whatif-${testSuffix}-btn`} onClick={() => onApplyWhatIf(row.id, pct)} style={ACTION_CHIP_STYLE(isWhatIfActive && whatIfDeltaPct === pct)}>
            {sign}{pct}%
          </button>
        );
      })}
      {isWhatIfActive && (
        <button type="button" data-testid="studio-whatif-reset-btn" onClick={onRevertSource} style={{ ...ACTION_CHIP_STYLE(false), color: "var(--state-warning)", borderColor: "var(--state-warning)" }}>
          Hoàn nguyên
        </button>
      )}
    </div>
  </div>
);

export const ContextualPointActionsBar: React.FC<{
  row: StudioDataRow | null;
  unit: StudioUnit;
  hasCallout: boolean;
  isSpotlight: boolean;
  whatIfRowId: string | null;
  whatIfDeltaPct: number;
  onToggleCallout: (row: StudioDataRow) => void;
  onToggleSpotlight: (rowId: string) => void;
  onAskAiExplain: (row: StudioDataRow) => void;
  onApplyWhatIf: (rowId: string, deltaPct: number) => void;
  onRevertSource: () => void;
  onDismiss: () => void;
}> = (p) => {
  if (!p.row) return null;
  const delta = computeRowDeltaPct(p.row);
  const isWhatIf = p.whatIfRowId === p.row.id && p.whatIfDeltaPct !== 0;
  return (
    <div data-testid="studio-point-action-bar" style={ELEVATED_PILL_BAR}>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "6px", fontSize: "11px" }}>
        <span style={{ fontWeight: FONT_WEIGHT_BOLD }}>{p.row.label}</span>
        <span style={{ color: "var(--text-secondary)" }}>
          {formatMetricValue(p.row.current, p.unit)} ({delta >= 0 ? "+" : ""}{delta}%)
        </span>
      </div>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "5px", flexWrap: "wrap" }}>
        <button type="button" data-testid="studio-point-add-callout-btn" onClick={() => p.onToggleCallout(p.row!)} style={ACTION_CHIP_STYLE(p.hasCallout)}>
          <span>✦ {p.hasCallout ? "Gỡ mũi tên chú thích" : "Gắn mũi tên chú thích"}</span>
        </button>
        <button type="button" data-testid="studio-point-spotlight-btn" onClick={() => p.onToggleSpotlight(p.row!.id)} style={ACTION_CHIP_STYLE(p.isSpotlight)}>
          <span>✦ {p.isSpotlight ? "Tắt Chiếu điểm" : "Chiếu điểm (Grey-out nền)"}</span>
        </button>
        <button type="button" data-testid="studio-point-ask-ai-btn" onClick={() => p.onAskAiExplain(p.row!)} style={ACTION_CHIP_STYLE(false)}>
          <span>✦ Hỏi Agent giải thích</span>
        </button>
        <button type="button" aria-label="Đóng thanh thao tác điểm dữ liệu" onClick={p.onDismiss} style={{ border: "none", background: "transparent", color: "var(--text-muted)", cursor: "pointer", padding: "2px 4px" }}>
          <BrandIcon name="close" size={ICON_XS} color="currentColor" />
        </button>
      </div>
      <WhatIfScenarioStrip row={p.row} unit={p.unit} whatIfDeltaPct={p.whatIfDeltaPct} isWhatIfActive={isWhatIf} onApplyWhatIf={p.onApplyWhatIf} onRevertSource={p.onRevertSource} />
    </div>
  );
};
