// @story 06.5
// @implements AC-02, AC-03, AC-04, AC-05 (Option D v2 Dashboard Studio Types: Storytelling, Drill-Down & Unified Toolbar)
import type {
  ChartSourceFormat,
  GenerateDeckResponse,
  StageshowFramePreset,
} from "../../../../lib/validations/deck-generation";

export type StudioChartType =
  | "bar"
  | "movement"
  | "line"
  | "sparkline"
  | "pie"
  | "segmented"
  | "bullet"
  | "tick-bar"
  | "kpi"
  | "heatmap";

export type StudioChartGroupId = "comparison" | "trend" | "composition" | "target" | "matrix";
export type StudioPaletteId = "ocean" | "emerald" | "amber" | "slate";
export type StudioInteractionMode = "interact" | "select" | "pin" | "text";
export type StudioViewState = "DEFAULT" | "EMPTY" | "LOADING" | "ERROR";
export type StudioLegendPos = "bottom" | "right" | "hidden";
export type StudioUnit = "$" | "%" | "₫";
export type StudioTimeframe = "Q4 2026" | "Q3 2026" | "YTD 2026";
export type StudioMobileTab = "control" | "canvas";
export type StudioControlView = "general" | "chart" | "kpi" | "sharebar";
export type StudioAudienceMode = "analyst" | "ceo";
export type StudioAiLensId = "driver" | "target" | "dynamics";
export type StudioShareSort = "desc" | "asc";

export type StudioWidgetId =
  | "kpiStrip"
  | "shareBar"
  | "targetLine"
  | "timeframe"
  | "footnote";

export interface StudioDataRow {
  id: string;
  label: string;
  current: number;
  previous: number;
  rawDisplay?: string;
}

export interface StudioCommentPin {
  id: string;
  num: number;
  xPct: number;
  yPct: number;
  text: string;
  targetFfId: string;
}

export interface StudioCallout {
  id: string;
  rowId: string;
  text: string;
}

export interface StudioQaIssue {
  id: string;
  severity: "warning" | "danger";
  label: string;
  detail: string;
}

export interface SubChatMessage {
  id: string;
  role: "user" | "assistant";
  text: string;
  targetFfId?: string;
  kind?: "chat" | "comment";
  pinNumber?: number;
}

export interface UploadedFileState {
  name: string;
  format: ChartSourceFormat;
  inlineContent?: string;
  inlineBase64?: string;
  byteSize: number;
}

export interface StudioSnapshot {
  title: string;
  subtitle: string;
  brief: string;
  footnote: string;
  framePreset: StageshowFramePreset;
  chartType: StudioChartType;
  paletteId: StudioPaletteId;
  viewState: StudioViewState;
  audienceMode: StudioAudienceMode;
  activeLens: StudioAiLensId | null;
  spotlightRowId: string | null;
  callouts: StudioCallout[];
  kpiShowTargetTick: boolean;
  kpiShowDelta: boolean;
  shareBarSort: StudioShareSort;
  shareBarShowPct: boolean;
  shareBarShowPrior: boolean;
  whatIfRowId: string | null;
  whatIfDeltaPct: number;
  xAxisLabel: string;
  yAxisLabel: string;
  unit: StudioUnit;
  showComparison: boolean;
  targetValue: number;
  legendPos: StudioLegendPos;
  timeframe: StudioTimeframe;
  widgets: Record<StudioWidgetId, boolean>;
  rows: StudioDataRow[];
  pins: StudioCommentPin[];
}

export interface DeckCsvGeneratorModalProps {
  isOpen: boolean;
  projectId: string;
  aspectRatio?: "16:9" | "16:10" | "Auto";
  onClose: () => void;
  onDeckGenerated?: (response: GenerateDeckResponse) => void;
  onSyncToMainChat?: (summaryText: string) => void;
  onEmbedIntoActiveSlide?: (htmlSnippet: string, title: string) => void;
}

export interface SourceDivergenceInfo {
  isDivergent: boolean;
  changedRowCount: number;
  addedRows: number;
  removedRows: number;
  totalDelta: number;
}

export interface DataSalienceCheck {
  tone: "success" | "warning" | "danger";
  label: string;
}

export interface PaletteConfig {
  id: StudioPaletteId;
  label: string;
  colors: readonly [string, string, string, string];
}

export interface StudioChartOption {
  id: StudioChartType;
  label: string;
  group: StudioChartGroupId;
  groupLabel: string;
  hint: string;
}

export {
  createInitialSnapshot,
  getPaletteColors,
  INITIAL_STUDIO_ROWS,
  STUDIO_AI_LENSES,
  STUDIO_CHART_GROUPS,
  STUDIO_CHART_OPTIONS,
  STUDIO_MODE_OPTIONS,
  STUDIO_PALETTES,
  STUDIO_WIDGET_CHIPS,
} from "./constants";
export {
  BYTES_CHUNK_SIZE,
  DRAWER_DEFAULT_VW,
  DRAWER_MAX_VW,
  DRAWER_MIN_PX,
  DRAWER_MOBILE_MAX_VW,
  DRAWER_Z_INDEX,
  HISTORY_MAX_ENTRIES,
  MASTER_DEFAULT_PX,
  MASTER_MAX_PX,
  MASTER_MIN_PX,
  MOBILE_BREAKPOINT_PX,
  SUBCHAT_COLLAPSED_PX,
  SUBCHAT_DEFAULT_PX,
  SUBCHAT_MAX_PX,
  SUBCHAT_MIN_PX,
} from "./layoutBounds";
