// @story 06.5
// @implements AC-04, AC-05 (Deterministic Scoped Sub-Chat State Refinement & Contextual Explanation Engine)
import { formatMetricValue } from "./studioDataUtils";
import { computeRowDeltaPct } from "./studioStorytellingUtils";
import {
  type StudioChartType,
  type StudioCommentPin,
  type StudioPaletteId,
  type StudioSnapshot,
} from "./types";

export interface SubChatRefinementResult {
  nextSnapshot: StudioSnapshot;
  reply: string;
  highlightRowId?: string | null;
}

const PCT_SCALE = 100;

const CHART_INTENT_RULES: ReadonlyArray<readonly [StudioChartType, readonly string[]]> = [
  ["sparkline", ["sparkline"]],
  ["heatmap", ["heatmap", "nhiệt"]],
  ["tick-bar", ["tick"]],
  ["segmented", ["phân đoạn", "segmented"]],
  ["movement", ["biến động", "movement"]],
  ["line", ["đường", "line"]],
  ["pie", ["tròn", "pie"]],
  ["bullet", ["bullet"]],
  ["kpi", ["kpi"]],
  ["bar", ["cột", "bar"]],
];

function detectChartTypeIntent(lower: string): StudioChartType | null {
  const matched = CHART_INTENT_RULES.find(([, keywords]) => keywords.some((kw) => lower.includes(kw)));
  return matched ? matched[0] : null;
}

function detectPaletteIntent(lower: string): StudioPaletteId | null {
  if (lower.includes("emerald") || lower.includes("xanh lá")) return "emerald";
  if (lower.includes("amber") || lower.includes("vàng") || lower.includes("cam")) return "amber";
  if (lower.includes("slate") || lower.includes("xám")) return "slate";
  if (lower.includes("ocean") || lower.includes("xanh dương")) return "ocean";
  return null;
}

function extractTargetNumber(prompt: string): number | null {
  const lower = prompt.toLowerCase();
  if (!lower.includes("mục tiêu") && !lower.includes("target")) return null;
  const match = prompt.match(/(\d+(?:[.,]\d+)?)/);
  if (!match) return null;
  const parsed = Number.parseFloat(match[1].replace(",", ""));
  return Number.isFinite(parsed) ? parsed : null;
}

function extractTitleUpdate(prompt: string): string | null {
  const match = prompt.match(/(?:đổi tiêu đề|tiêu đề|title)\s*[:\-]\s*(.+)/i);
  return match ? match[1].trim() : null;
}

function applyWidgetIntents(lower: string, snapshot: StudioSnapshot, notes: string[]): StudioSnapshot {
  const widgets = { ...snapshot.widgets };
  let showComparison = snapshot.showComparison;
  let audienceMode = snapshot.audienceMode;
  if (lower.includes("ẩn kpi") || lower.includes("hide kpi")) { widgets.kpiStrip = false; notes.push("ẩn thẻ KPI"); }
  else if (lower.includes("bật kpi") || lower.includes("show kpi")) { widgets.kpiStrip = true; notes.push("bật thẻ KPI"); }
  if (lower.includes("ẩn mục tiêu") || lower.includes("hide target")) { widgets.targetLine = false; notes.push("ẩn đường mục tiêu"); }
  else if (lower.includes("bật mục tiêu") || lower.includes("show target")) { widgets.targetLine = true; notes.push("bật đường mục tiêu"); }
  if (lower.includes("tắt so sánh")) { showComparison = false; notes.push("tắt chuỗi so sánh kỳ trước"); }
  else if (lower.includes("bật so sánh")) { showComparison = true; notes.push("bật chuỗi so sánh kỳ trước"); }
  if (lower.includes("ceo") || lower.includes("3s")) { audienceMode = "ceo"; notes.push("chuyển chế độ Slide CEO 3s"); }
  else if (lower.includes("chi tiết") || lower.includes("analyst")) { audienceMode = "analyst"; notes.push("chuyển chế độ Chi tiết"); }
  return { ...snapshot, widgets, showComparison, audienceMode };
}

function applySemanticUpdates(
  trimmed: string,
  lower: string,
  base: StudioSnapshot,
  notes: string[]
): { next: StudioSnapshot; highlightRowId?: string | null; explainReply?: string } {
  let next = base;
  let highlightRowId: string | null | undefined;
  let explainReply: string | undefined;
  const requestedChart = detectChartTypeIntent(lower);
  if (requestedChart && requestedChart !== next.chartType) {
    next = { ...next, chartType: requestedChart };
    notes.push(`chuyển biểu đồ sang ${requestedChart.toUpperCase()}`);
  }
  const requestedPalette = detectPaletteIntent(lower);
  if (requestedPalette && requestedPalette !== next.paletteId) {
    next = { ...next, paletteId: requestedPalette };
    notes.push(`đổi bảng màu sang ${requestedPalette}`);
  }
  const newTarget = extractTargetNumber(trimmed);
  if (newTarget !== null) {
    next = { ...next, targetValue: newTarget, widgets: { ...next.widgets, targetLine: true } };
    notes.push(`cập nhật mục tiêu = ${newTarget}`);
  }
  const newTitle = extractTitleUpdate(trimmed);
  if (newTitle) {
    next = { ...next, title: newTitle };
    notes.push(`đổi tiêu đề thành "${newTitle}"`);
  }
  const matchedRow = next.rows.find((r) => r.label.trim() && lower.includes(r.label.toLowerCase()));
  if (matchedRow) {
    highlightRowId = matchedRow.id;
    if (lower.includes("giải thích") || lower.includes("explain")) {
      const total = next.rows.reduce((a, r) => a + Math.max(0, r.current), 0);
      const share = total > 0 ? Math.round((Math.max(0, matchedRow.current) / total) * PCT_SCALE) : 0;
      const pct = computeRowDeltaPct(matchedRow);
      explainReply = `Phân tích ${matchedRow.label}: đạt ${formatMetricValue(matchedRow.current, next.unit)} (${share}% tổng), ${pct >= 0 ? "tăng" : "giảm"} ${Math.abs(pct)}% so với kỳ trước (${formatMetricValue(matchedRow.previous, next.unit)}).`;
    } else {
      notes.push(`tô sáng kênh "${matchedRow.label}"`);
    }
  }
  return { next, highlightRowId, explainReply };
}

export function applySubChatCommand(
  prompt: string,
  snapshot: StudioSnapshot,
  selectedFfId: string | null,
  pins: readonly StudioCommentPin[]
): SubChatRefinementResult {
  const trimmed = prompt.trim();
  const lower = trimmed.toLowerCase();
  const notes: string[] = [];
  const afterWidgets = applyWidgetIntents(lower, snapshot, notes);
  const { next: afterSemantic, highlightRowId, explainReply } = applySemanticUpdates(trimmed, lower, afterWidgets, notes);
  let next = afterSemantic;

  if (pins.length > 0 && (lower.includes("ghim") || lower.includes("pin") || lower.includes("xử lý"))) {
    next = { ...next, pins: [] };
    notes.push(`đã áp dụng và đóng ${pins.length} ghim nhận xét`);
  }
  const scopeTag = selectedFfId ? `[${selectedFfId}] ` : "";
  next = { ...next, brief: `${next.brief.trim()} ${scopeTag}${trimmed}`.trim() };

  const reply = explainReply
    ?? (notes.length > 0
      ? `Đã cập nhật ${scopeTag}${notes.join(", ")}.`
      : `Đã ghi nhận yêu cầu ${scopeTag}vào cấu hình dashboard.`);
  return { nextSnapshot: next, reply, highlightRowId };
}
