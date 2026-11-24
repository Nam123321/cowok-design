// @story 06.5
// @implements AC-02, AC-03, AC-04 (Option D v2 Dashboard Studio Palettes, 10 Chart Shapes, AI Lenses & Initial Snapshot)
import type {
  PaletteConfig,
  StudioAiLensId,
  StudioChartGroupId,
  StudioChartOption,
  StudioDataRow,
  StudioInteractionMode,
  StudioPaletteId,
  StudioSnapshot,
  StudioWidgetId,
} from "./types";

const DIRECT_CUR = 1420;
const DIRECT_PREV = 1180;
const PARTNER_CUR = 980;
const PARTNER_PREV = 860;
const ORGANIC_CUR = 640;
const ORGANIC_PREV = 590;
const ENTERPRISE_CUR = 1160;
const ENTERPRISE_PREV = 910;
const DEFAULT_TARGET = 1000;

export const STUDIO_PALETTES: readonly PaletteConfig[] = [
  { id: "ocean", label: "Ocean Blue", colors: ["var(--action-primary)", "var(--state-info)", "var(--state-success)", "var(--agent-accent)"] },
  { id: "emerald", label: "Emerald Growth", colors: ["var(--state-success)", "var(--action-primary)", "var(--agent-accent)", "var(--text-secondary)"] },
  { id: "amber", label: "Warm Amber", colors: ["var(--agent-accent)", "var(--state-warning)", "var(--action-primary)", "var(--state-danger)"] },
  { id: "slate", label: "Executive Slate", colors: ["var(--text-primary)", "var(--text-secondary)", "var(--action-primary)", "var(--border-strong)"] },
];

export const STUDIO_CHART_GROUPS: ReadonlyArray<{ id: StudioChartGroupId; label: string }> = [
  { id: "comparison", label: "So sánh" },
  { id: "trend", label: "Xu hướng" },
  { id: "composition", label: "Tỷ trọng" },
  { id: "target", label: "Mục tiêu" },
  { id: "matrix", label: "Ma trận" },
];

export const STUDIO_CHART_OPTIONS: readonly StudioChartOption[] = [
  { id: "bar", label: "Cột", group: "comparison", groupLabel: "So sánh", hint: "So sánh giá trị trực tiếp giữa các danh mục" },
  { id: "movement", label: "Biến động ±", group: "comparison", groupLabel: "So sánh", hint: "Làm nổi bật mức tăng/giảm so với kỳ trước" },
  { id: "line", label: "Đường", group: "trend", groupLabel: "Xu hướng", hint: "Theo dõi diễn biến và độ dốc xu hướng" },
  { id: "sparkline", label: "Sparkline", group: "trend", groupLabel: "Xu hướng", hint: "Đường xu hướng siêu gọn kèm số liệu cạnh nhau" },
  { id: "pie", label: "Tròn", group: "composition", groupLabel: "Tỷ trọng", hint: "Hiển thị cơ cấu phần trăm trong tổng 100%" },
  { id: "segmented", label: "Phân đoạn", group: "composition", groupLabel: "Tỷ trọng", hint: "Thanh tỷ trọng 100% gọn gàng cho 3–5 nhóm" },
  { id: "bullet", label: "Bullet", group: "target", groupLabel: "Mục tiêu", hint: "Đối chiếu thực tế với kỳ trước & ngưỡng mục tiêu" },
  { id: "tick-bar", label: "Tick-Bar", group: "target", groupLabel: "Mục tiêu", hint: "Thanh tiến độ 2026 kèm vạch mục tiêu sắc nét" },
  { id: "kpi", label: "Thẻ KPI", group: "target", groupLabel: "Mục tiêu", hint: "Lưới chỉ số điều hành đọc nhanh trong 3 giây" },
  { id: "heatmap", label: "Ma trận nhiệt", group: "matrix", groupLabel: "Ma trận", hint: "Soi cường độ Q3, Q4, tăng trưởng & chênh mục tiêu" },
];

export const STUDIO_MODE_OPTIONS: ReadonlyArray<{
  id: StudioInteractionMode;
  icon: "cursor-click" | "crosshair" | "message-square" | "edit";
  label: string;
}> = [
  { id: "interact", icon: "cursor-click", label: "Tương tác" },
  { id: "select", icon: "crosshair", label: "Chọn vùng" },
  { id: "pin", icon: "message-square", label: "Ghim" },
  { id: "text", icon: "edit", label: "Sửa chữ" },
];

export const STUDIO_AI_LENSES: ReadonlyArray<{
  id: StudioAiLensId;
  label: string;
  summary: string;
}> = [
  { id: "driver", label: "1. Động lực tăng trưởng", summary: "Cột + Chiếu điểm dẫn đầu + Callout" },
  { id: "target", label: "2. Tiến độ mục tiêu", summary: "Tick-Bar / Bullet + Ngưỡng mục tiêu" },
  { id: "dynamics", label: "3. Cơ cấu & Biến động", summary: "Biến động ± + Chiếu điểm bứt phá" },
];

export const STUDIO_WIDGET_CHIPS: ReadonlyArray<{ id: StudioWidgetId; label: string }> = [
  { id: "kpiStrip", label: "Thẻ KPI (3)" },
  { id: "shareBar", label: "Xếp hạng Share-Bar" },
  { id: "targetLine", label: "Đường mục tiêu" },
  { id: "timeframe", label: "Bộ lọc thời gian" },
  { id: "footnote", label: "Ghi chú nguồn" },
];

export const INITIAL_STUDIO_ROWS: readonly StudioDataRow[] = [
  { id: "row-1", label: "Direct", current: DIRECT_CUR, previous: DIRECT_PREV },
  { id: "row-2", label: "Partner", current: PARTNER_CUR, previous: PARTNER_PREV },
  { id: "row-3", label: "Organic", current: ORGANIC_CUR, previous: ORGANIC_PREV },
  { id: "row-4", label: "Enterprise", current: ENTERPRISE_CUR, previous: ENTERPRISE_PREV },
];

export function createInitialSnapshot(): StudioSnapshot {
  return {
    title: "Báo cáo Tăng trưởng Doanh thu Q4",
    subtitle: "Hiệu suất theo kênh phân phối so với kỳ trước",
    brief: "Tổng hợp doanh thu Q4 theo kênh Direct, Partner, Organic và Enterprise vượt mục tiêu.",
    footnote: "Nguồn: Báo cáo tài chính nội bộ Q4 2026 (đã kiểm toán sơ bộ).",
    framePreset: "Stage",
    chartType: "bar",
    paletteId: "ocean",
    viewState: "DEFAULT",
    audienceMode: "analyst",
    activeLens: null,
    spotlightRowId: null,
    callouts: [],
    kpiShowTargetTick: true,
    kpiShowDelta: true,
    shareBarSort: "desc",
    shareBarShowPct: true,
    shareBarShowPrior: true,
    whatIfRowId: null,
    whatIfDeltaPct: 0,
    xAxisLabel: "Kênh bán hàng",
    yAxisLabel: "Doanh thu",
    unit: "$",
    showComparison: true,
    targetValue: DEFAULT_TARGET,
    legendPos: "bottom",
    timeframe: "Q4 2026",
    widgets: { kpiStrip: true, shareBar: true, targetLine: true, timeframe: true, footnote: true },
    rows: INITIAL_STUDIO_ROWS.map((r) => ({ ...r })),
    pins: [],
  };
}

export function getPaletteColors(paletteId: StudioPaletteId): readonly string[] {
  const found = STUDIO_PALETTES.find((p) => p.id === paletteId);
  return found ? found.colors : STUDIO_PALETTES[0].colors;
}
