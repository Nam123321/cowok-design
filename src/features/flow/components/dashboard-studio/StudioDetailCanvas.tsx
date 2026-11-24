// @story 06.5
// @implements AC-02, AC-03, AC-04, AC-05 (Interactive HTML/SVG Right Detail Dashboard Canvas v2 with 5 Storytelling Patterns)
import React, { useMemo, useState } from "react";
import {
  StudioMarqueeOverlay,
  StudioSpatialActionBar,
  useStudioMarqueeSelection,
  type StudioSpatialCoreAction,
  type StudioSpatialTarget,
} from "../../../../components/ui/studio-drawer";
import { useTranslation } from "../../../../hooks/useTranslation";
import type { StageshowFramePreset } from "../../../../lib/validations/deck-generation";
import { StudioCanvasToolbar } from "./StudioCanvasToolbar";
import {
  CanvasDashboardHeader,
  CanvasFootnoteBar,
  CanvasKpiStrip,
  CanvasNonDefaultState,
  CanvasPinsOverlay,
  CanvasShareBarPanel,
  selectableBoxStyle,
  SelectionTagBadge,
} from "./StudioCanvasWidgets";
import {
  AiLensStrip,
  ContextualPointActionsBar,
} from "./StudioStorytellingBar";
import { runHonestDataQaAudit } from "./studioStorytellingUtils";
import { StudioSvgCharts } from "./StudioSvgCharts";
import type {
  StudioAiLensId,
  StudioCommentPin,
  StudioDataRow,
  StudioInteractionMode,
  StudioSnapshot,
} from "./types";

export interface StudioDetailCanvasProps {
  snapshot: StudioSnapshot;
  interactionMode: StudioInteractionMode;
  selectedFfId: string | null;
  highlightedRowId: string | null;
  onModeChange: (mode: StudioInteractionMode) => void;
  onSelectElement: (ffId: string) => void;
  onToggleRowHighlight: (rowId: string) => void;
  onClearRowHighlight: () => void;
  onCommitPatch: (patch: Partial<StudioSnapshot>) => void;
  onPreviewTarget: (nextTarget: number) => void;
  onCommitTarget: (baseTarget: number, nextTarget: number) => void;
  onAddPin: (pin: StudioCommentPin) => void;
  onUpdatePinText: (id: string, text: string) => void;
  onDeletePin: (id: string) => void;
  onApplyPinToChat: (pin: StudioCommentPin) => void;
  onApplyAiLens: (lensId: StudioAiLensId) => void;
  onAutoFixQa: () => void;
  onToggleCallout: (row: StudioDataRow) => void;
  onToggleSpotlight: (rowId: string) => void;
  onAskAiExplainRow: (row: StudioDataRow) => void;
  onApplyWhatIf: (rowId: string, deltaPct: number) => void;
  onRevertToSource: () => void;
  onFocusSubChat?: (prefill: string) => void;
  onCommentSubmitted?: (targetLabel: string, commentText: string, pinNum: number) => void;
}

const PIN_MIN_X = 8;
const PIN_MAX_X = 92;
const PIN_MIN_Y = 12;
const PIN_MAX_Y = 88;
const PIN_FALLBACK_X = 50;
const PIN_FALLBACK_Y = 45;
const PIN_PCT_SCALE = 100;

function getFrameContainerStyle(preset: StageshowFramePreset): React.CSSProperties {
  if (preset === "Phone") {
    return { width: "min(390px, 100%)", margin: "0 auto", borderRadius: "20px", border: "2px solid var(--border-strong)", backgroundColor: "var(--bg-surface)", boxShadow: "var(--shadow-md)", overflow: "hidden" };
  }
  if (preset === "Window") {
    return { width: "100%", borderRadius: "var(--radius-lg)", border: "1px solid var(--border-strong)", backgroundColor: "var(--bg-surface)", boxShadow: "var(--shadow-md)", overflow: "hidden" };
  }
  return { width: "100%", borderRadius: "var(--radius-md)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", overflow: "hidden" };
}

function buildCommentPinFromClick(ev: React.MouseEvent<HTMLDivElement>, existingPins: readonly StudioCommentPin[]): StudioCommentPin {
  const rect = ev.currentTarget.getBoundingClientRect();
  const xPct = rect.width > 0 ? Math.min(PIN_MAX_X, Math.max(PIN_MIN_X, Math.round(((ev.clientX - rect.left) / rect.width) * PIN_PCT_SCALE))) : PIN_FALLBACK_X;
  const yPct = rect.height > 0 ? Math.min(PIN_MAX_Y, Math.max(PIN_MIN_Y, Math.round(((ev.clientY - rect.top) / rect.height) * PIN_PCT_SCALE))) : PIN_FALLBACK_Y;
  const nextNum = existingPins.reduce((m, p) => Math.max(m, p.num), 0) + 1;
  return { id: `pin-${Date.now()}-${nextNum}`, num: nextNum, xPct, yPct, text: "", targetFfId: "main-chart-panel" };
}

const WindowFrameChrome: React.FC<{ preset: StageshowFramePreset; title: string }> = ({ preset, title }) => {
  const { t } = useTranslation();
  if (preset === "Stage") return null;
  return (
    <div data-testid={`studio-frame-chrome-${preset.toLowerCase()}`} style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "6px 12px", backgroundColor: "var(--bg-subtle)", borderBottom: "1px solid var(--border-subtle)", fontSize: "10.5px", color: "var(--text-muted)" }}>
      <span>{preset === "Window" ? t("deckGen.windowChrome", "● ● ●  Window Preview") : t("deckGen.phoneChrome", "Phone Preview · 390px")}</span>
      <span>{title}</span>
    </div>
  );
};

const CanvasChartAndShareGrid: React.FC<{
  props: StudioDetailCanvasProps;
  activePinId: string | null;
  setActivePinId: (id: string | null) => void;
  onChartStageClick: (ev: React.MouseEvent<HTMLDivElement>) => void;
}> = ({ props, activePinId, setActivePinId, onChartStageClick }) => {
  const snap = props.snapshot;
  const showShareBar = snap.widgets.shareBar && snap.audienceMode !== "ceo";
  const gridCols = showShareBar && snap.framePreset !== "Phone" ? "minmax(0, 1.55fr) minmax(200px, 1fr)" : "1fr";
  return (
    <div style={{ display: "grid", gridTemplateColumns: gridCols, gap: "10px", alignItems: "stretch" }}>
      <div data-ff-id="main-chart-panel" data-testid="studio-chart-stage" onClick={onChartStageClick} style={{ ...selectableBoxStyle(props.selectedFfId === "main-chart-panel"), padding: "10px", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-default)", cursor: props.interactionMode === "pin" ? "crosshair" : "default" }}>
        <SelectionTagBadge ffId="main-chart-panel" active={props.selectedFfId === "main-chart-panel"} />
        <StudioSvgCharts
          chartType={snap.chartType} paletteId={snap.paletteId} rows={snap.rows} unit={snap.unit}
          xAxisLabel={snap.xAxisLabel} yAxisLabel={snap.yAxisLabel} showComparison={snap.showComparison}
          showTargetLine={snap.widgets.targetLine} targetValue={snap.targetValue} legendPos={snap.legendPos}
          highlightedRowId={props.highlightedRowId} spotlightRowId={snap.spotlightRowId} callouts={snap.callouts}
          onToggleRowHighlight={props.onToggleRowHighlight} onPreviewTarget={props.onPreviewTarget}
          onCommitTarget={props.onCommitTarget} onApplyWhatIf={props.onApplyWhatIf}
        />
        <CanvasPinsOverlay
          pins={snap.pins} activePinId={activePinId} onSelectPin={setActivePinId} onUpdatePinText={props.onUpdatePinText}
          onDeletePin={(id) => { props.onDeletePin(id); if (activePinId === id) setActivePinId(null); }}
          onApplyPinToChat={(pin) => { props.onApplyPinToChat(pin); setActivePinId(null); }}
        />
      </div>
      {showShareBar && (
        <CanvasShareBarPanel
          rows={snap.rows} paletteId={snap.paletteId} unit={snap.unit} sortOrder={snap.shareBarSort}
          showPct={snap.shareBarShowPct} showPrior={snap.shareBarShowPrior} highlightedRowId={props.highlightedRowId}
          selectedFfId={props.selectedFfId} onSelectElement={props.onSelectElement} onToggleRowHighlight={props.onToggleRowHighlight}
        />
      )}
    </div>
  );
};

const CanvasFrameBody: React.FC<{
  props: StudioDetailCanvasProps;
  activeRow: StudioDataRow | null;
  hasCallout: boolean;
  activePinId: string | null;
  setActivePinId: (id: string | null) => void;
  onChartStageClick: (ev: React.MouseEvent<HTMLDivElement>) => void;
}> = ({ props, activeRow, hasCallout, activePinId, setActivePinId, onChartStageClick }) => {
  const snap = props.snapshot;
  if (snap.viewState !== "DEFAULT") {
    return <CanvasNonDefaultState viewState={snap.viewState} onResetDefault={() => props.onCommitPatch({ viewState: "DEFAULT" })} />;
  }
  return (
    <div style={{ padding: "14px", display: "flex", flexDirection: "column", gap: "10px" }}>
      <CanvasDashboardHeader snapshot={snap} interactionMode={props.interactionMode} selectedFfId={props.selectedFfId} onSelectElement={props.onSelectElement} onCommitPatch={props.onCommitPatch} />
      <CanvasKpiStrip snapshot={snap} selectedFfId={props.selectedFfId} onSelectElement={props.onSelectElement} />
      <ContextualPointActionsBar
        row={activeRow} unit={snap.unit} hasCallout={hasCallout} isSpotlight={Boolean(activeRow && snap.spotlightRowId === activeRow.id)}
        whatIfRowId={snap.whatIfRowId} whatIfDeltaPct={snap.whatIfDeltaPct}
        onToggleCallout={props.onToggleCallout} onToggleSpotlight={props.onToggleSpotlight}
        onAskAiExplain={props.onAskAiExplainRow} onApplyWhatIf={props.onApplyWhatIf}
        onRevertSource={props.onRevertToSource} onDismiss={props.onClearRowHighlight}
      />
      <CanvasChartAndShareGrid props={props} activePinId={activePinId} setActivePinId={setActivePinId} onChartStageClick={onChartStageClick} />
      {snap.widgets.footnote && snap.audienceMode !== "ceo" && (
        <CanvasFootnoteBar footnote={snap.footnote} interactionMode={props.interactionMode} selectedFfId={props.selectedFfId} onSelectElement={props.onSelectElement} onCommitFootnote={(footnote) => props.onCommitPatch({ footnote })} />
      )}
    </div>
  );
};

function useDashboardStageSpatialHandlers(
  props: StudioDetailCanvasProps,
  setActivePinId: (id: string | null) => void,
  setSpatialTarget: (t: StudioSpatialTarget | null) => void
) {
  const handleChartStageClick = (ev: React.MouseEvent<HTMLDivElement>) => {
    if (props.interactionMode === "pin") {
      const pin = buildCommentPinFromClick(ev, props.snapshot.pins);
      props.onAddPin(pin);
      setActivePinId(pin.id);
      return;
    }
    props.onSelectElement("main-chart-panel");
  };

  const handleSpatialAction = (action: StudioSpatialCoreAction, target: StudioSpatialTarget) => {
    if (action === "comment") {
      const pin: StudioCommentPin = { id: `pin-${Date.now()}`, num: props.snapshot.pins.length + 1, xPct: PIN_FALLBACK_X, yPct: PIN_FALLBACK_Y, text: "", targetFfId: target.ffId };
      props.onAddPin(pin);
      setActivePinId(pin.id);
    } else if (action === "ask-subchat") props.onFocusSubChat?.(`Tinh chỉnh ${target.label ?? target.ffId}: `);
    else if (action === "edit") props.onModeChange("text");
    else props.onSelectElement(target.ffId);
    setSpatialTarget(null);
  };

  const handleSubmitSpatialComment = (target: StudioSpatialTarget, commentText: string) => {
    const nextNum = props.snapshot.pins.length + 1;
    props.onAddPin({ id: `pin-${Date.now()}-${nextNum}`, num: nextNum, xPct: PIN_FALLBACK_X, yPct: PIN_FALLBACK_Y, text: commentText, targetFfId: target.ffId });
    props.onCommentSubmitted?.(target.label ?? target.ffId, commentText, nextNum);
    setSpatialTarget(null);
  };

  return { handleChartStageClick, handleSpatialAction, handleSubmitSpatialComment };
}

export const StudioDetailCanvas: React.FC<StudioDetailCanvasProps> = (props) => {
  const [activePinId, setActivePinId] = useState<string | null>(null);
  const [spatialTarget, setSpatialTarget] = useState<StudioSpatialTarget | null>(null);
  const snap = props.snapshot;
  const qaIssues = useMemo(() => runHonestDataQaAudit(snap), [snap]);
  const activeRow = useMemo(() => snap.rows.find((r) => r.id === props.highlightedRowId) ?? null, [snap.rows, props.highlightedRowId]);
  const hasCallout = Boolean(activeRow && snap.callouts.some((c) => c.rowId === activeRow.id));
  const { handleChartStageClick, handleSpatialAction, handleSubmitSpatialComment } = useDashboardStageSpatialHandlers(props, setActivePinId, setSpatialTarget);
  const marquee = useStudioMarqueeSelection({
    pointerMode: props.interactionMode === "select" ? "select-region" : "interact",
    onSelectTarget: (target) => { props.onSelectElement(target.ffId); setSpatialTarget(target); },
  });

  return (
    <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", backgroundColor: "var(--bg-canvas)", overflow: "hidden" }}>
      <StudioCanvasToolbar
        chartType={snap.chartType} paletteId={snap.paletteId} interactionMode={props.interactionMode} viewState={snap.viewState}
        framePreset={snap.framePreset} audienceMode={snap.audienceMode} pinCount={snap.pins.length} qaIssues={qaIssues}
        onChartTypeChange={(chartType) => props.onCommitPatch({ chartType })} onPaletteChange={(paletteId) => props.onCommitPatch({ paletteId })}
        onModeChange={props.onModeChange} onViewStateChange={(viewState) => props.onCommitPatch({ viewState })} onFrameChange={(framePreset) => props.onCommitPatch({ framePreset })}
        onToggleAudienceMode={() => props.onCommitPatch({ audienceMode: snap.audienceMode === "ceo" ? "analyst" : "ceo" })}
        onAutoFixQa={props.onAutoFixQa} tier3Strip={<AiLensStrip activeLens={snap.activeLens} onApplyLens={props.onApplyAiLens} />}
      />
      <div ref={marquee.containerRef} data-testid="studio-detail-canvas" onMouseDown={marquee.handleCanvasMouseDown} onContextMenu={marquee.handleCanvasContextMenu} style={{ flex: 1, overflowY: "auto", padding: "12px 14px", position: "relative", userSelect: props.interactionMode === "select" ? "none" : "auto" }}>
        <StudioMarqueeOverlay rect={marquee.marqueeRect} />
        <StudioSpatialActionBar target={spatialTarget} onCoreAction={handleSpatialAction} onSubmitComment={handleSubmitSpatialComment} onClose={() => setSpatialTarget(null)} />
        <div style={getFrameContainerStyle(snap.framePreset)}>
          <WindowFrameChrome preset={snap.framePreset} title={snap.title} />
          <CanvasFrameBody props={props} activeRow={activeRow} hasCallout={hasCallout} activePinId={activePinId} setActivePinId={setActivePinId} onChartStageClick={handleChartStageClick} />
        </div>
      </div>
    </div>
  );
};
