// @story 06.5
// @implements AC-02, AC-03, AC-04, AC-05 (Data Parsing, Divergence Tracking, Salience Checks & HTML Export)
import {
  DECK_GEN_MAX_PDF_BYTES,
  DECK_GEN_MAX_POINTS,
  DECK_GEN_MAX_SERIES,
  type ChartSourceFormat,
  type ChartSourceInput,
  type DeckChartKind,
  type NormalizeChartSourceResponse,
} from "../../../../lib/validations/deck-generation";
import { DeckGenerationApiError } from "../../../../api/deck-generation";
import {
  BYTES_CHUNK_SIZE,
  type DataSalienceCheck,
  type SourceDivergenceInfo,
  type StudioChartType,
  type StudioDataRow,
  type StudioSnapshot,
  type StudioUnit,
  type UploadedFileState,
} from "./types";

const PERCENT_TARGET = 100;
const PERCENT_TOLERANCE = 0.5;
const MIN_HEALTHY_ROWS = 3;
const COMPARE_FALLBACK_RATIO = 0.85;
const FLOAT_EPSILON = 0.0001;

export function countWords(text: string): number {
  return text.trim().split(/\s+/).filter(Boolean).length;
}

const BACKEND_CHART_KIND_MAP: Record<StudioChartType, DeckChartKind> = {
  bar: "bar",
  movement: "bar",
  bullet: "bar",
  "tick-bar": "bar",
  heatmap: "bar",
  line: "line",
  sparkline: "line",
  pie: "pie",
  segmented: "pie",
  kpi: "kpi",
};

export function toBackendChartKind(chartType: StudioChartType): DeckChartKind {
  return BACKEND_CHART_KIND_MAP[chartType];
}

export function formatMetricValue(value: number, unit: StudioUnit): string {
  const formatted = Number.isFinite(value)
    ? value.toLocaleString("en-US", { maximumFractionDigits: 2 })
    : "0";
  if (unit === "$") return `$${formatted}`;
  if (unit === "%") return `${formatted}%`;
  return `${formatted}₫`;
}

export function encodeBytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i += BYTES_CHUNK_SIZE) {
    const slice = bytes.subarray(i, Math.min(i + BYTES_CHUNK_SIZE, bytes.length));
    binary += String.fromCharCode(...slice);
  }
  return window.btoa(binary);
}

export function detectFormatFromName(filename: string): ChartSourceFormat | null {
  const lower = filename.trim().toLowerCase();
  if (lower.endsWith(".pdf")) return "pdf";
  if (lower.endsWith(".tsv")) return "tsv";
  if (lower.endsWith(".csv")) return "csv";
  return null;
}

export async function readSelectedChartFile(
  file: File
): Promise<{ ok: true; state: UploadedFileState } | { ok: false; key: string; fallback: string }> {
  const fmt = detectFormatFromName(file.name);
  if (!fmt) {
    return { ok: false, key: "deckGen.errorUnsupportedFile", fallback: "Chỉ hỗ trợ tệp .csv, .tsv hoặc .pdf." };
  }
  if (file.size > DECK_GEN_MAX_PDF_BYTES) {
    return { ok: false, key: "deckGen.errorFileTooLarge", fallback: "Tệp vượt giới hạn 5 MB cho phép." };
  }
  if (fmt === "pdf") {
    const buf = new Uint8Array(await file.arrayBuffer());
    return {
      ok: true,
      state: { name: file.name, format: "pdf", inlineBase64: encodeBytesToBase64(buf), byteSize: file.size },
    };
  }
  const text = await file.text();
  return { ok: true, state: { name: file.name, format: fmt, inlineContent: text, byteSize: file.size } };
}

function splitDelimitedLine(line: string, delimiter: string): string[] {
  const cells: string[] = [];
  let current = "";
  let inQuotes = false;
  for (let i = 0; i < line.length; i += 1) {
    const ch = line[i];
    if (ch === '"') {
      inQuotes = !inQuotes;
    } else if (ch === delimiter && !inQuotes) {
      cells.push(current.trim());
      current = "";
    } else {
      current += ch;
    }
  }
  cells.push(current.trim());
  return cells;
}

function parseNumericCell(raw: string | undefined): number {
  if (!raw) return 0;
  const cleaned = raw.replace(/[^0-9.-]/g, "");
  const parsed = Number.parseFloat(cleaned);
  return Number.isFinite(parsed) ? parsed : 0;
}

export function parseRawDelimitedRows(rawText: string): StudioDataRow[] {
  const lines = rawText.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);
  if (lines.length < 2) return [];
  const delimiter = lines[0].includes("\t") && !lines[0].includes(",") ? "\t" : ",";
  return lines.slice(1).map((line, idx) => {
    const cols = splitDelimitedLine(line, delimiter);
    const label = cols[0] || `Mục ${idx + 1}`;
    const current = parseNumericCell(cols[1]);
    const previous = cols[2] !== undefined ? parseNumericCell(cols[2]) : Math.round(current * COMPARE_FALLBACK_RATIO);
    return { id: `src-${idx + 1}`, label, current, previous, rawDisplay: cols[1] };
  });
}

export function serializeRowsToCsv(
  rows: readonly StudioDataRow[],
  yAxisLabel: string,
  showComparison: boolean
): string {
  const safeY = yAxisLabel.trim() || "Value";
  const header = showComparison ? `Category,${safeY},Previous` : `Category,${safeY}`;
  const body = rows.map((row) => {
    const escaped = row.label.includes(",") ? `"${row.label.replace(/"/g, '""')}"` : row.label;
    return showComparison ? `${escaped},${row.current},${row.previous}` : `${escaped},${row.current}`;
  });
  return [header, ...body].join("\n");
}

export function normalizeResponseToRows(resp: NormalizeChartSourceResponse): StudioDataRow[] {
  const points = resp.spec.series[0]?.points ?? [];
  return points.map((pt, idx) => {
    const raw = resp.rawPoints[idx];
    const current = raw ? raw.rawValue : pt.value;
    return {
      id: `norm-${idx + 1}`,
      label: raw ? raw.category : pt.label,
      current,
      previous: Math.round(current * COMPARE_FALLBACK_RATIO),
      rawDisplay: raw?.rawDisplay,
    };
  });
}

export function computeSourceDivergence(
  sourceRows: readonly StudioDataRow[],
  currentRows: readonly StudioDataRow[]
): SourceDivergenceInfo {
  const maxLen = Math.max(sourceRows.length, currentRows.length);
  let changedRowCount = 0;
  for (let i = 0; i < maxLen; i += 1) {
    const src = sourceRows[i];
    const cur = currentRows[i];
    if (!src || !cur) {
      changedRowCount += 1;
      continue;
    }
    const diffLabel = src.label.trim() !== cur.label.trim();
    const diffCur = Math.abs(src.current - cur.current) > FLOAT_EPSILON;
    const diffPrev = Math.abs(src.previous - cur.previous) > FLOAT_EPSILON;
    if (diffLabel || diffCur || diffPrev) changedRowCount += 1;
  }
  const srcSum = sourceRows.reduce((acc, r) => acc + r.current, 0);
  const curSum = currentRows.reduce((acc, r) => acc + r.current, 0);
  const addedRows = Math.max(0, currentRows.length - sourceRows.length);
  const removedRows = Math.max(0, sourceRows.length - currentRows.length);
  return {
    isDivergent: changedRowCount > 0,
    changedRowCount,
    addedRows,
    removedRows,
    totalDelta: Number((curSum - srcSum).toFixed(2)),
  };
}

export function computeDataSalience(
  rows: readonly StudioDataRow[],
  chartType: StudioChartType,
  unit: StudioUnit
): DataSalienceCheck {
  if (rows.length === 0) {
    return { tone: "danger", label: "Thiếu dữ liệu: 0 dòng" };
  }
  const emptyLabels = rows.filter((r) => !r.label.trim()).length;
  if (emptyLabels > 0) {
    return { tone: "warning", label: `Cảnh báo: ${emptyLabels} ô nhãn trống` };
  }
  if (rows.length < MIN_HEALTHY_ROWS) {
    return { tone: "warning", label: `Mẫu nhỏ (${rows.length} điểm dữ liệu)` };
  }
  if ((chartType === "pie" || chartType === "segmented") && unit === "%") {
    const sum = rows.reduce((acc, r) => acc + Math.max(0, r.current), 0);
    if (Math.abs(sum - PERCENT_TARGET) > PERCENT_TOLERANCE) {
      return { tone: "warning", label: `Tổng tỷ trọng = ${sum.toFixed(1)}% (≠ 100%)` };
    }
  }
  return { tone: "success", label: `Dữ liệu nhất quán · ${rows.length} điểm` };
}

export function buildChartSourcePayload(params: {
  uploadedFile: UploadedFileState | null;
  pastedData: string;
  rows: readonly StudioDataRow[];
  isDivergent: boolean;
  yAxisLabel: string;
  showComparison: boolean;
  chartType: StudioChartType;
  title: string;
}): ChartSourceInput | undefined {
  const chartKind = toBackendChartKind(params.chartType);
  if (params.uploadedFile && !params.isDivergent) {
    return {
      sourceFormat: params.uploadedFile.format,
      inlineContent: params.uploadedFile.inlineContent,
      inlineBase64: params.uploadedFile.inlineBase64,
      chartKind,
      title: params.title.trim() || undefined,
      maxSeries: DECK_GEN_MAX_SERIES,
      maxPoints: DECK_GEN_MAX_POINTS,
    };
  }
  const trimmedPaste = params.pastedData.trim();
  if (trimmedPaste && !params.isDivergent) {
    const fmt: ChartSourceFormat = trimmedPaste.includes("\t") && !trimmedPaste.includes(",") ? "tsv" : "csv";
    return {
      sourceFormat: fmt,
      inlineContent: trimmedPaste,
      chartKind,
      title: params.title.trim() || undefined,
      maxSeries: DECK_GEN_MAX_SERIES,
      maxPoints: DECK_GEN_MAX_POINTS,
    };
  }
  if (params.rows.length === 0) return undefined;
  return {
    sourceFormat: "csv",
    inlineContent: serializeRowsToCsv(params.rows, params.yAxisLabel, params.showComparison),
    chartKind,
    title: params.title.trim() || undefined,
    maxSeries: DECK_GEN_MAX_SERIES,
    maxPoints: DECK_GEN_MAX_POINTS,
  };
}

export function formatDeckGenError(err: unknown, fallback: string): string {
  if (err instanceof DeckGenerationApiError) {
    return err.guidance ? `${err.code}: ${err.guidance}` : `${err.code}: ${err.message}`;
  }
  return err instanceof Error ? err.message : fallback;
}

function escapeHtml(raw: string): string {
  return raw.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

export function buildEmbeddableDashboardHtml(snapshot: StudioSnapshot): string {
  const total = snapshot.rows.reduce((acc, r) => acc + r.current, 0);
  const rowsMarkup = snapshot.rows
    .map((r) => {
      const share = total > 0 ? Math.round((r.current / total) * PERCENT_TARGET) : 0;
      return `<li><strong>${escapeHtml(r.label)}</strong>: ${escapeHtml(formatMetricValue(r.current, snapshot.unit))} (${share}%)</li>`;
    })
    .join("");
  const footnote = snapshot.widgets.footnote && snapshot.audienceMode !== "ceo"
    ? `<footer>${escapeHtml(snapshot.footnote)}</footer>`
    : "";
  return [
    `<section data-studio-chart="${snapshot.chartType}" data-frame="${snapshot.framePreset}" data-audience="${snapshot.audienceMode}">`,
    `<h3>${escapeHtml(snapshot.title)}</h3>`,
    `<p>${escapeHtml(snapshot.subtitle)}</p>`,
    `<ul>${rowsMarkup}</ul>`,
    footnote,
    `</section>`,
  ].join("");
}

export function buildMainChatSyncSummary(
  snapshot: StudioSnapshot,
  actionLabel: string,
  slidesCount?: number
): string {
  const total = snapshot.rows.reduce((acc, r) => acc + r.current, 0);
  const slideNote = typeof slidesCount === "number" ? ` (${slidesCount} slides)` : "";
  const activeWidgets = Object.entries(snapshot.widgets)
    .filter(([, enabled]) => enabled)
    .map(([key]) => key)
    .join(", ");
  return `[Dashboard Studio] ${actionLabel}: "${snapshot.title}"${slideNote} · Biểu đồ ${snapshot.chartType.toUpperCase()} · Tổng ${formatMetricValue(total, snapshot.unit)} (${snapshot.rows.length} dòng) · Mục tiêu ${formatMetricValue(snapshot.targetValue, snapshot.unit)} · Widgets: ${activeWidgets || "none"}.`;
}
