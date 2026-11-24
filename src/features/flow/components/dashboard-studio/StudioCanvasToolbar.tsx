// @story 06.5
// @implements AC-02, AC-04 (Unified 2-Row Detail Canvas Toolbar matching HtmlPreviewToolbar UX Pattern)
import React, { useEffect, useRef, useState } from "react";
import { BrandIcon } from "../../../../components/ui/BrandIcon";
import { useTranslation } from "../../../../hooks/useTranslation";
import type { StageshowFramePreset } from "../../../../lib/validations/deck-generation";
import {
  FONT_WEIGHT_BOLD,
  FONT_WEIGHT_MEDIUM,
  FONT_WEIGHT_SEMIBOLD,
} from "./studioStyles";
import {
  ChartPickerPopover,
  PalettePickerPopover,
  QaAuditPopover,
} from "./StudioToolbarPopovers";
import {
  STUDIO_MODE_OPTIONS,
  type StudioAudienceMode,
  type StudioChartType,
  type StudioInteractionMode,
  type StudioPaletteId,
  type StudioQaIssue,
  type StudioViewState,
} from "./types";

export interface StudioCanvasToolbarProps {
  chartType: StudioChartType;
  paletteId: StudioPaletteId;
  interactionMode: StudioInteractionMode;
  viewState: StudioViewState;
  framePreset: StageshowFramePreset;
  audienceMode: StudioAudienceMode;
  pinCount: number;
  qaIssues: readonly StudioQaIssue[];
  onChartTypeChange: (type: StudioChartType) => void;
  onPaletteChange: (id: StudioPaletteId) => void;
  onModeChange: (mode: StudioInteractionMode) => void;
  onViewStateChange: (state: StudioViewState) => void;
  onFrameChange: (preset: StageshowFramePreset) => void;
  onToggleAudienceMode: () => void;
  onAutoFixQa: () => void;
}

type OpenPopover = "chart" | "palette" | "qa" | null;

const ICON_SM = 12;
const VIEW_STATES: readonly StudioViewState[] = ["DEFAULT", "EMPTY", "LOADING", "ERROR"];
const FRAME_PRESETS: ReadonlyArray<{ preset: StageshowFramePreset; icon: "monitor" | "tablet" | "smartphone"; label: string }> = [
  { preset: "Stage", icon: "monitor", label: "Stage (16:9 Desktop)" },
  { preset: "Window", icon: "tablet", label: "Window (Tablet / Card)" },
  { preset: "Phone", icon: "smartphone", label: "Phone (390px Mobile)" },
];

const ROW_STYLE: React.CSSProperties = {
  minHeight: "30px",
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: "6px",
  flexWrap: "wrap",
};

const PILL_GROUP_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  backgroundColor: "var(--bg-workspace)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-xs)",
  padding: "2px",
  gap: "2px",
  flexWrap: "nowrap",
};

function modeButtonStyle(active: boolean): React.CSSProperties {
  return {
    height: "24px",
    padding: "0 7px",
    borderRadius: "4px",
    border: active ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
    backgroundColor: active ? "var(--action-primary)" : "var(--bg-workspace)",
    color: active ? "var(--action-primary-fg)" : "var(--text-primary)",
    fontSize: "10.5px",
    fontWeight: active ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
    display: "inline-flex",
    alignItems: "center",
    gap: "4px",
    cursor: "pointer",
    position: "relative",
    whiteSpace: "nowrap",
  };
}

function subStateBtnStyle(active: boolean): React.CSSProperties {
  return {
    height: "20px",
    padding: "0 6px",
    border: "none",
    borderRadius: "var(--radius-xs)",
    fontSize: "9.5px",
    fontFamily: "var(--font-mono)",
    fontWeight: active ? FONT_WEIGHT_BOLD : FONT_WEIGHT_MEDIUM,
    backgroundColor: active ? "var(--bg-selected)" : "transparent",
    color: active ? "var(--action-primary)" : "var(--text-muted)",
    cursor: "pointer",
    whiteSpace: "nowrap",
  };
}

const RowOneCanvasModes: React.FC<{
  mode: StudioInteractionMode;
  pinCount: number;
  onModeChange: (m: StudioInteractionMode) => void;
}> = ({ mode, pinCount, onModeChange }) => (
  <div role="radiogroup" aria-label="Chế độ tương tác Canvas" style={{ display: "inline-flex", alignItems: "center", gap: "4px", flexWrap: "nowrap" }}>
    {STUDIO_MODE_OPTIONS.map((opt) => {
      const active = opt.id === mode;
      return (
        <button key={opt.id} type="button" role="radio" aria-checked={active} data-testid={`studio-mode-${opt.id}`} onClick={() => onModeChange(opt.id)} style={modeButtonStyle(active)}>
          <BrandIcon name={opt.icon} size={ICON_SM} color="currentColor" />
          <span>{opt.label}</span>
          {opt.id === "pin" && pinCount > 0 && (
            <span data-testid="studio-mode-pin-count" style={{ padding: "0 4px", borderRadius: "999px", fontSize: "9px", fontWeight: FONT_WEIGHT_BOLD, backgroundColor: active ? "var(--bg-surface)" : "var(--action-primary)", color: active ? "var(--action-primary)" : "var(--action-primary-fg)" }}>
              {pinCount}
            </span>
          )}
        </button>
      );
    })}
  </div>
);

const RowTwoSubStateAndViewport: React.FC<{
  viewState: StudioViewState;
  framePreset: StageshowFramePreset;
  onViewStateChange: (s: StudioViewState) => void;
  onFrameChange: (p: StageshowFramePreset) => void;
}> = ({ viewState, framePreset, onViewStateChange, onFrameChange }) => {
  const { t } = useTranslation();
  return (
    <div style={{ ...ROW_STYLE, borderTop: "1px solid var(--border-subtle)", paddingTop: "3px" }}>
      <div style={PILL_GROUP_STYLE} role="group" aria-label={t("canvas.subStateAria", "Trạng thái dữ liệu màn hình")}>
        {VIEW_STATES.map((st) => (
          <button key={st} type="button" data-testid={`studio-substate-${st}`} aria-pressed={viewState === st} onClick={() => onViewStateChange(st)} style={subStateBtnStyle(viewState === st)}>
            {st}
          </button>
        ))}
      </div>
      <div data-ff-id="stageshow-frame-selector" data-testid="stageshow-frame-selector" role="radiogroup" aria-label="Khung trình diễn stageshow" style={PILL_GROUP_STYLE}>
        {FRAME_PRESETS.map((item) => {
          const active = framePreset === item.preset;
          return (
            <button
              key={item.preset} type="button" role="radio" aria-checked={active}
              data-testid={`stageshow-frame-${item.preset.toLowerCase()}`}
              title={item.label} aria-label={item.label}
              onClick={() => onFrameChange(item.preset)}
              style={{
                width: "24px", height: "20px", padding: 0, border: "none", borderRadius: "var(--radius-xs)",
                backgroundColor: active ? "var(--bg-selected)" : "transparent",
                color: active ? "var(--action-primary)" : "var(--text-muted)",
                display: "inline-flex", alignItems: "center", justifyContent: "center", cursor: "pointer",
              }}
            >
              <BrandIcon name={item.icon} size={ICON_SM} color="currentColor" />
            </button>
          );
        })}
      </div>
    </div>
  );
};

export const StudioCanvasToolbar: React.FC<StudioCanvasToolbarProps> = (props) => {
  const [openPopover, setOpenPopover] = useState<OpenPopover>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!openPopover) return;
    const onDown = (ev: MouseEvent) => { if (ev.target instanceof Node && !rootRef.current?.contains(ev.target)) setOpenPopover(null); };
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") { ev.stopPropagation(); setOpenPopover(null); }
    };
    document.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey, true);
    };
  }, [openPopover]);

  const isCeo = props.audienceMode === "ceo";
  return (
    <div ref={rootRef} data-testid="studio-canvas-toolbar" style={{ padding: "4px 8px", backgroundColor: "var(--bg-surface)", borderBottom: "1px solid var(--border-default)", display: "flex", flexDirection: "column", gap: "3px", flexShrink: 0 }}>
      <div style={ROW_STYLE}>
        <RowOneCanvasModes mode={props.interactionMode} pinCount={props.pinCount} onModeChange={props.onModeChange} />
        <div style={{ display: "inline-flex", alignItems: "center", gap: "4px", flexWrap: "nowrap" }}>
          <ChartPickerPopover chartType={props.chartType} open={openPopover === "chart"} onToggle={() => setOpenPopover((v) => (v === "chart" ? null : "chart"))} onSelect={(type) => { props.onChartTypeChange(type); setOpenPopover(null); }} />
          <PalettePickerPopover paletteId={props.paletteId} open={openPopover === "palette"} onToggle={() => setOpenPopover((v) => (v === "palette" ? null : "palette"))} onSelect={(id) => { props.onPaletteChange(id); setOpenPopover(null); }} />
          <QaAuditPopover issues={props.qaIssues} open={openPopover === "qa"} onToggle={() => setOpenPopover((v) => (v === "qa" ? null : "qa"))} onAutoFixAll={() => { props.onAutoFixQa(); setOpenPopover(null); }} />
          <button
            type="button" data-testid="studio-audience-toggle" aria-pressed={isCeo}
            onClick={props.onToggleAudienceMode}
            title="Chuyển mật độ hiển thị giữa Slide CEO 3s (tối giản) và Chi tiết (phân tích)"
            style={{
              height: "24px", padding: "0 8px", borderRadius: "4px",
              border: isCeo ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
              backgroundColor: isCeo ? "var(--bg-selected)" : "var(--bg-workspace)",
              color: isCeo ? "var(--action-primary)" : "var(--text-primary)",
              fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, cursor: "pointer", whiteSpace: "nowrap",
            }}
          >
            {isCeo ? "Slide CEO 3s" : "Chi tiết"}
          </button>
        </div>
      </div>
      <RowTwoSubStateAndViewport viewState={props.viewState} framePreset={props.framePreset} onViewStateChange={props.onViewStateChange} onFrameChange={props.onFrameChange} />
    </div>
  );
};
