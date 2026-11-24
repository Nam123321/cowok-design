// @story 06.5
// @implements AC-03, AC-04, AC-05 (Data Storytelling Engine: 3 AI Lenses, Honest-Data QA Auto-Fix, What-If & 3-Act Arc)
import { formatMetricValue } from "./studioDataUtils";
import type {
  StudioAiLensId,
  StudioCallout,
  StudioControlView,
  StudioDataRow,
  StudioQaIssue,
  StudioSnapshot,
  SubChatMessage,
} from "./types";

const PCT_SCALE = 100;
const PCT_TOLERANCE = 0.5;
const MIN_BASE_ROWS = 3;
const CLUTTER_THRESHOLD = 4;
const MAX_CHAT_HISTORY_NOTES = 3;

function findTopRow(rows: readonly StudioDataRow[]): StudioDataRow | null {
  if (rows.length === 0) return null;
  return rows.reduce((best, r) => (r.current > best.current ? r : best), rows[0]);
}

function findFastestGrowthRow(rows: readonly StudioDataRow[]): StudioDataRow | null {
  if (rows.length === 0) return null;
  const pct = (r: StudioDataRow) => (r.previous > 0 ? (r.current - r.previous) / r.previous : 0);
  return rows.reduce((best, r) => (pct(r) > pct(best) ? r : best), rows[0]);
}

export function computeRowDeltaPct(row: StudioDataRow): number {
  if (row.previous <= 0) return 0;
  return Math.round(((row.current - row.previous) / row.previous) * PCT_SCALE);
}

export function buildRowCallout(row: StudioDataRow, unit: StudioSnapshot["unit"]): StudioCallout {
  const delta = computeRowDeltaPct(row);
  const sign = delta >= 0 ? "+" : "";
  return {
    id: `callout-${row.id}`,
    rowId: row.id,
    text: `${row.label}: ${formatMetricValue(row.current, unit)} (${sign}${delta}%)`,
  };
}

function buildDriverLens(snapshot: StudioSnapshot, top: StudioDataRow | null): StudioSnapshot {
  const pct = top ? computeRowDeltaPct(top) : 0;
  const title = top ? `${top.label} dẫn đầu tăng trưởng (+${pct}% vs kỳ trước)` : snapshot.title;
  return {
    ...snapshot, activeLens: "driver", chartType: "bar", showComparison: true,
    spotlightRowId: top?.id ?? null, title,
    callouts: top ? [buildRowCallout(top, snapshot.unit)] : snapshot.callouts,
  };
}

function buildTargetLens(snapshot: StudioSnapshot, top: StudioDataRow | null): StudioSnapshot {
  const aboveCount = snapshot.rows.filter((r) => r.current >= snapshot.targetValue).length;
  const title = `${aboveCount}/${snapshot.rows.length} kênh vượt ngưỡng mục tiêu ${formatMetricValue(snapshot.targetValue, snapshot.unit)}`;
  const pct = top ? Math.round((top.current / Math.max(1, snapshot.targetValue)) * PCT_SCALE) : 0;
  return {
    ...snapshot, activeLens: "target", chartType: "tick-bar", kpiShowTargetTick: true,
    spotlightRowId: top?.id ?? null, title,
    widgets: { ...snapshot.widgets, targetLine: true },
    callouts: top ? [{ id: `callout-${top.id}`, rowId: top.id, text: `Đạt ${pct}% mục tiêu` }] : [],
  };
}

export function applyAiLens(
  snapshot: StudioSnapshot,
  lensId: StudioAiLensId
): { nextSnapshot: StudioSnapshot; highlightRowId: string | null } {
  const top = findTopRow(snapshot.rows);
  if (lensId === "driver") return { highlightRowId: top?.id ?? null, nextSnapshot: buildDriverLens(snapshot, top) };
  if (lensId === "target") return { highlightRowId: top?.id ?? null, nextSnapshot: buildTargetLens(snapshot, top) };
  const fastest = findFastestGrowthRow(snapshot.rows);
  const fastPct = fastest ? computeRowDeltaPct(fastest) : 0;
  const title = fastest ? `${fastest.label} bứt phá mạnh nhất (+${fastPct}%), dẫn dắt cơ cấu kỳ này` : snapshot.title;
  return {
    highlightRowId: fastest?.id ?? null,
    nextSnapshot: {
      ...snapshot, activeLens: "dynamics", chartType: "movement", showComparison: true,
      spotlightRowId: fastest?.id ?? null, title,
      callouts: fastest ? [buildRowCallout(fastest, snapshot.unit)] : [],
    },
  };
}

export function isActionTitle(title: string): boolean {
  const trimmed = title.trim();
  if (!trimmed) return false;
  return /[0-9%+\-$₫]/.test(trimmed) && /(dẫn|vượt|tăng|giảm|đạt|bứt phá|chiếm|lead|grow|exceed)/i.test(trimmed);
}

function checkDataIntegrityIssues(snapshot: StudioSnapshot): StudioQaIssue[] {
  const issues: StudioQaIssue[] = [];
  if (snapshot.rows.some((r) => !r.label.trim() || !Number.isFinite(r.current))) {
    issues.push({ id: "missing-values", severity: "danger", label: "Có dòng thiếu nhãn hoặc giá trị rỗng", detail: "Điền đầy đủ nhãn danh mục để tránh mất ngữ cảnh trên biểu đồ." });
  }
  if (snapshot.rows.some((r) => r.current < 0) && snapshot.chartType !== "movement") {
    issues.push({ id: "negative-baseline", severity: "danger", label: "Giá trị âm trên biểu đồ nền 0", detail: "Chuyển sang biểu đồ Biến động ± hoặc chuẩn hóa giá trị không âm." });
  }
  if (snapshot.rows.length < MIN_BASE_ROWS) {
    issues.push({ id: "small-base", severity: "warning", label: `Mẫu nhỏ (${snapshot.rows.length} điểm dữ liệu)`, detail: "Ghi chú cỡ mẫu nhỏ (n < 3) để người xem không hiểu lầm xu hướng." });
  }
  if ((snapshot.chartType === "pie" || snapshot.chartType === "segmented") && snapshot.unit === "%") {
    const sum = snapshot.rows.reduce((a, r) => a + Math.max(0, r.current), 0);
    if (Math.abs(sum - PCT_SCALE) > PCT_TOLERANCE) {
      issues.push({ id: "pie-sum", severity: "warning", label: `Tổng tỷ trọng = ${sum.toFixed(1)}% (≠ 100%)`, detail: "Chuẩn hóa tự động các lát về đúng tổng 100%." });
    }
  }
  return issues;
}

export function runHonestDataQaAudit(snapshot: StudioSnapshot): StudioQaIssue[] {
  const issues: StudioQaIssue[] = [];
  if (!isActionTitle(snapshot.title)) {
    issues.push({ id: "label-title", severity: "warning", label: "Tiêu đề dạng nhãn (chưa nêu kết luận)", detail: "Nên viết Action Title nêu rõ kênh dẫn đầu và con số chênh lệch để đọc trong 3 giây." });
  }
  issues.push(...checkDataIntegrityIssues(snapshot));
  if (snapshot.rows.length >= CLUTTER_THRESHOLD && !snapshot.spotlightRowId) {
    issues.push({ id: "no-spotlight", severity: "warning", label: "Chưa bật Chiếu điểm (Grey-out nền)", detail: "Nhấn nổi 1 chuỗi chính và làm dịu chuỗi nền để giảm nhiễu thị giác." });
  }
  return issues;
}

function sanitizeRowsForQa(snapshot: StudioSnapshot): StudioDataRow[] {
  const allowNeg = snapshot.chartType === "movement";
  let rows = snapshot.rows.map((r, idx) => {
    const safeVal = Number.isFinite(r.current) ? r.current : 0;
    return {
      ...r,
      label: r.label.trim() || `Kênh ${idx + 1}`,
      current: allowNeg ? safeVal : Math.max(0, safeVal),
    };
  });
  const isProportionalPct = (snapshot.chartType === "pie" || snapshot.chartType === "segmented") && snapshot.unit === "%";
  if (isProportionalPct) {
    const sum = rows.reduce((a, r) => a + Math.max(0, r.current), 0);
    if (sum > 0 && Math.abs(sum - PCT_SCALE) > PCT_TOLERANCE) {
      rows = rows.map((r) => ({ ...r, current: Math.round((Math.max(0, r.current) / sum) * PCT_SCALE) }));
    }
  }
  return rows;
}

function buildAutoFixedTitle(snapshot: StudioSnapshot, top: StudioDataRow | null): string {
  if (isActionTitle(snapshot.title) || !top) return snapshot.title;
  const topPct = computeRowDeltaPct(top);
  return `${top.label} dẫn đầu ${snapshot.yAxisLabel.toLowerCase()} (+${topPct}% vs kỳ trước, đạt ${formatMetricValue(top.current, snapshot.unit)})`;
}

export function autoFixHonestDataIssues(snapshot: StudioSnapshot): StudioSnapshot {
  const rows = sanitizeRowsForQa(snapshot);
  const top = findTopRow(rows);
  const nextTitle = buildAutoFixedTitle(snapshot, top);
  const spotlightRowId = snapshot.spotlightRowId ?? top?.id ?? null;
  let callouts = snapshot.callouts;
  if (callouts.length === 0 && top) {
    callouts = [buildRowCallout(top, snapshot.unit)];
  }
  return { ...snapshot, rows, title: nextTitle, spotlightRowId, callouts };
}

export function applyWhatIfDelta(
  sourceRows: readonly StudioDataRow[],
  snapshot: StudioSnapshot,
  rowId: string,
  deltaPct: number
): StudioSnapshot {
  const baseRow = sourceRows.find((r) => r.id === rowId) ?? snapshot.rows.find((r) => r.id === rowId);
  if (!baseRow) return snapshot;
  const nextVal = Math.max(0, Math.round(baseRow.current * (1 + deltaPct / PCT_SCALE)));
  const rows = snapshot.rows.map((r) => (r.id === rowId ? { ...r, current: nextVal } : r));
  return {
    ...snapshot,
    rows,
    whatIfRowId: deltaPct === 0 ? null : rowId,
    whatIfDeltaPct: deltaPct,
  };
}

export function deriveBackendBrief(
  snapshot: StudioSnapshot,
  messages: readonly SubChatMessage[]
): string {
  const top = findTopRow(snapshot.rows);
  const total = snapshot.rows.reduce((a, r) => a + r.current, 0);
  const userNotes = messages
    .filter((m) => m.role === "user")
    .slice(-MAX_CHAT_HISTORY_NOTES)
    .map((m) => m.text.trim())
    .filter(Boolean)
    .join(" | ");
  const arc = `Hồi 1: Bối cảnh tổng ${formatMetricValue(total, snapshot.unit)} · Hồi 2: Động lực ${top ? `${top.label} (${formatMetricValue(top.current, snapshot.unit)})` : "đa kênh"} · Hồi 3: Hành động mục tiêu ${formatMetricValue(snapshot.targetValue, snapshot.unit)}.`;
  return [snapshot.title.trim(), arc, userNotes].filter(Boolean).join(" ").trim();
}

export function mapFfIdToControlView(ffId: string): StudioControlView {
  if (ffId === "main-chart-panel") return "chart";
  if (ffId.startsWith("kpi-")) return "kpi";
  if (ffId === "ranked-share-bar") return "sharebar";
  return "general";
}
