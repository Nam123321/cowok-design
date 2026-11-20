// @story 06.3
// @implements AC-01, AC-03
import {
  CHART_MAX_ARIA_LABEL_CHARS,
  CHART_MAX_SVG_PRIMITIVES,
  ChartSpecV12Schema,
  type ChartColorToken,
  type ChartDataPoint,
  type ChartSpecV12,
} from "../../../lib/validations/presentation-chart-specs";

const SVG_NS = "http://www.w3.org/2000/svg";
const DEFAULT_WIDTH = 360;
const DEFAULT_HEIGHT = 200;
const PAD_X = 28;
const PAD_TOP = 24;
const PAD_BOTTOM = 26;
const FULL_CIRCLE_DEG = 360;
const HALF_CIRCLE_DEG = 180;
const RAD_PER_DEG = Math.PI / HALF_CIRCLE_DEG;
const START_ANGLE_OFFSET = -90;
const DONUT_INNER_RATIO = 0.56;
const PIE_RADIUS_RATIO = 0.36;
const BAR_GAP_PX = 4;
const MIN_BAR_HEIGHT_PX = 2;
const STACK_BAR_HEIGHT_PX = 36;
const FUNNEL_MIN_RATIO = 0.28;
const POINT_DOT_RADIUS = 3.5;

const TOKEN_VAR_MAP: Record<ChartColorToken, string> = {
  "human-accent": "var(--human-accent)",
  "agent-accent": "var(--agent-accent)",
  "state-success": "var(--state-success)",
  "state-warning": "var(--state-warning)",
  "state-danger": "var(--state-danger)",
  "state-info": "var(--state-info)",
  "text-secondary": "var(--text-secondary)",
};

const SERIES_PALETTE: readonly string[] = [
  "var(--human-accent)",
  "var(--agent-accent)",
  "var(--state-success)",
  "var(--state-info)",
  "var(--state-warning)",
  "var(--text-secondary)",
];

export type ChartParseResult =
  | { ok: true; spec: ChartSpecV12; ariaLabel: string }
  | { ok: false; code: string; message: string };

export interface RenderedChartSvgResult {
  svg: SVGSVGElement;
  primitiveCount: number;
  renderTimeMs: number;
  ariaLabel: string;
}

function classifyZodChartError(message: string): string {
  if (message.includes("E_CHART_SUM_NOT_100")) return "E_CHART_SUM_NOT_100";
  if (message.includes("E_CHART_NEGATIVE_PROPORTIONAL")) return "E_CHART_NEGATIVE_PROPORTIONAL";
  if (message.includes("E_CHART_DOS_LIMIT")) return "E_CHART_DOS_LIMIT";
  return "VALIDATION_ERROR";
}

export function buildChartAriaLabel(spec: ChartSpecV12): string {
  if (spec.ariaSummary?.trim()) {
    return spec.ariaSummary.trim().slice(0, CHART_MAX_ARIA_LABEL_CHARS);
  }
  const firstPts = spec.series[0]?.points.slice(0, 5) ?? [];
  const summary = firstPts.map((pt) => `${pt.label}: ${pt.value}${spec.unit ?? ""}`).join(", ");
  const raw = summary ? `${spec.title} (${spec.kind}) — ${summary}` : `${spec.title} (${spec.kind})`;
  return raw.slice(0, CHART_MAX_ARIA_LABEL_CHARS);
}

export function parseChartSpecSafely(rawJson: string): ChartParseResult {
  const trimmed = rawJson.trim();
  if (!trimmed || trimmed === "[]") {
    return { ok: false, code: "EMPTY_CHART_SPEC", message: "Chart spec is empty" };
  }
  const byteLen = typeof TextEncoder !== "undefined" ? new TextEncoder().encode(trimmed).byteLength : trimmed.length * 3;
  if (trimmed.length > 65_536 || byteLen > 65_536) {
    return { ok: false, code: "E_CHART_DOS_LIMIT", message: "Chart spec exceeds 65536 byte DoS cap" };
  }
  let parsedJson: unknown;
  try {
    parsedJson = JSON.parse(trimmed);
  } catch (err) {
    return {
      ok: false,
      code: "VALIDATION_ERROR",
      message: err instanceof Error ? err.message : "Invalid JSON",
    };
  }
  const res = ChartSpecV12Schema.safeParse(parsedJson);
  if (!res.success) {
    const firstMsg = res.error.issues[0]?.message || "Invalid ChartSpecV12";
    return { ok: false, code: classifyZodChartError(firstMsg), message: firstMsg };
  }
  return { ok: true, spec: res.data, ariaLabel: buildChartAriaLabel(res.data) };
}

function createSvgNode(
  doc: Document,
  tag: string,
  attrs: Record<string, string | number>,
  counter: { count: number }
): SVGElement {
  counter.count += 1;
  const el = doc.createElementNS(SVG_NS, tag);
  for (const [k, v] of Object.entries(attrs)) {
    el.setAttribute(k, String(v));
  }
  return el;
}

function resolvePointColor(pt: ChartDataPoint, index: number): string {
  if (pt.colorToken && Object.hasOwn(TOKEN_VAR_MAP, pt.colorToken)) {
    return TOKEN_VAR_MAP[pt.colorToken]!;
  }
  return SERIES_PALETTE[index % SERIES_PALETTE.length]!;
}

function polarToCartesian(cx: number, cy: number, radius: number, angleDeg: number): { x: number; y: number } {
  const rad = angleDeg * RAD_PER_DEG;
  return { x: cx + radius * Math.cos(rad), y: cy + radius * Math.sin(rad) };
}

function roundCoord1(n: number): number {
  return Math.round(n * 10) / 10;
}

function roundCoord2(n: number): number {
  return Math.round(n * 100) / 100;
}

function buildArcPath(opts: {
  cx: number;
  cy: number;
  outerR: number;
  innerR: number;
  startDeg: number;
  endDeg: number;
}): string {
  const span = Math.min(FULL_CIRCLE_DEG - 0.01, Math.max(0.1, opts.endDeg - opts.startDeg));
  const largeArc = span > HALF_CIRCLE_DEG ? 1 : 0;
  const p1 = polarToCartesian(opts.cx, opts.cy, opts.outerR, opts.startDeg);
  const p2 = polarToCartesian(opts.cx, opts.cy, opts.outerR, opts.startDeg + span);
  if (opts.innerR <= 0) {
    return `M ${opts.cx} ${opts.cy} L ${roundCoord2(p1.x)} ${roundCoord2(p1.y)} A ${opts.outerR} ${opts.outerR} 0 ${largeArc} 1 ${roundCoord2(p2.x)} ${roundCoord2(p2.y)} Z`;
  }
  const p3 = polarToCartesian(opts.cx, opts.cy, opts.innerR, opts.startDeg + span);
  const p4 = polarToCartesian(opts.cx, opts.cy, opts.innerR, opts.startDeg);
  return `M ${roundCoord2(p1.x)} ${roundCoord2(p1.y)} A ${opts.outerR} ${opts.outerR} 0 ${largeArc} 1 ${roundCoord2(p2.x)} ${roundCoord2(p2.y)} L ${roundCoord2(p3.x)} ${roundCoord2(p3.y)} A ${opts.innerR} ${opts.innerR} 0 ${largeArc} 0 ${roundCoord2(p4.x)} ${roundCoord2(p4.y)} Z`;
}

function renderPieOrDonut(opts: {
  doc: Document;
  svg: SVGSVGElement;
  spec: ChartSpecV12;
  w: number;
  h: number;
  counter: { count: number };
}): void {
  const pts = opts.spec.series[0]?.points ?? [];
  const total = pts.reduce((acc, p) => acc + Math.max(0, p.value), 0) || 100;
  const cx = opts.w * 0.5;
  const cy = opts.h * 0.54;
  const outerR = Math.min(opts.w, opts.h) * PIE_RADIUS_RATIO;
  const innerR = opts.spec.kind === "donut" ? outerR * DONUT_INNER_RATIO : 0;
  let cursorDeg = START_ANGLE_OFFSET;
  pts.forEach((pt, idx) => {
    const sliceDeg = (Math.max(0, pt.value) / total) * FULL_CIRCLE_DEG;
    const d = buildArcPath({ cx, cy, outerR, innerR, startDeg: cursorDeg, endDeg: cursorDeg + sliceDeg });
    cursorDeg += sliceDeg;
    const pathEl = createSvgNode(opts.doc, "path", { d, fill: resolvePointColor(pt, idx), "data-chart-point": pt.label }, opts.counter);
    opts.svg.appendChild(pathEl);
  });
}

function renderStackedBar(opts: {
  doc: Document;
  svg: SVGSVGElement;
  spec: ChartSpecV12;
  w: number;
  h: number;
  counter: { count: number };
}): void {
  const pts = opts.spec.series[0]?.points ?? [];
  const total = pts.reduce((acc, p) => acc + Math.max(0, p.value), 0) || 100;
  const innerW = opts.w - PAD_X * 2;
  const y = (opts.h - STACK_BAR_HEIGHT_PX) * 0.52;
  let xCursor = PAD_X;
  pts.forEach((pt, idx) => {
    const segW = Math.max(1, (Math.max(0, pt.value) / total) * innerW);
    const rect = createSvgNode(
      opts.doc,
      "rect",
      { x: roundCoord1(xCursor), y: roundCoord1(y), width: roundCoord1(segW), height: STACK_BAR_HEIGHT_PX, fill: resolvePointColor(pt, idx), rx: 3 },
      opts.counter
    );
    xCursor += segW;
    opts.svg.appendChild(rect);
  });
}

function renderFunnelChart(opts: {
  doc: Document;
  svg: SVGSVGElement;
  spec: ChartSpecV12;
  w: number;
  h: number;
  counter: { count: number };
}): void {
  const pts = opts.spec.series[0]?.points ?? [];
  const innerW = opts.w - PAD_X * 2;
  const innerH = opts.h - PAD_TOP - PAD_BOTTOM;
  const stepH = Math.max(8, innerH / Math.max(1, pts.length) - BAR_GAP_PX);
  const maxVal = Math.max(1, ...pts.map((p) => Math.max(0, p.value)));
  pts.forEach((pt, idx) => {
    const ratio = Math.max(FUNNEL_MIN_RATIO, Math.max(0, pt.value) / maxVal);
    const segW = innerW * ratio;
    const x = PAD_X + (innerW - segW) * 0.5;
    const y = PAD_TOP + idx * (stepH + BAR_GAP_PX);
    const rect = createSvgNode(
      opts.doc,
      "rect",
      { x: roundCoord1(x), y: roundCoord1(y), width: roundCoord1(segW), height: roundCoord1(stepH), fill: resolvePointColor(pt, idx), rx: 4 },
      opts.counter
    );
    opts.svg.appendChild(rect);
  });
}

function computeCartesianRange(spec: ChartSpecV12): { minVal: number; maxVal: number; span: number } {
  const values = spec.series.flatMap((s) => s.points.map((p) => p.value));
  const minVal = Math.min(0, ...values);
  const maxVal = Math.max(0, ...values);
  const span = Math.max(1, maxVal - minVal);
  return { minVal, maxVal, span };
}

function renderBarChart(opts: {
  doc: Document;
  svg: SVGSVGElement;
  spec: ChartSpecV12;
  w: number;
  h: number;
  counter: { count: number };
}): void {
  const innerW = opts.w - PAD_X * 2;
  const innerH = opts.h - PAD_TOP - PAD_BOTTOM;
  const { maxVal, span } = computeCartesianRange(opts.spec);
  const baselineY = PAD_TOP + (maxVal / span) * innerH;
  const axis = createSvgNode(opts.doc, "line", { x1: PAD_X, y1: roundCoord1(baselineY), x2: opts.w - PAD_X, y2: roundCoord1(baselineY), stroke: "var(--border-strong)", "stroke-width": 1 }, opts.counter);
  opts.svg.appendChild(axis);

  const pts = opts.spec.series[0]?.points ?? [];
  const slotW = innerW / Math.max(1, pts.length);
  const barW = Math.max(4, slotW - BAR_GAP_PX);
  pts.forEach((pt, idx) => {
    const valH = Math.max(MIN_BAR_HEIGHT_PX, (Math.abs(pt.value) / span) * innerH);
    const y = pt.value >= 0 ? baselineY - valH : baselineY;
    const x = PAD_X + idx * slotW + BAR_GAP_PX * 0.5;
    const rect = createSvgNode(opts.doc, "rect", { x: roundCoord1(x), y: roundCoord1(y), width: roundCoord1(barW), height: roundCoord1(valH), fill: resolvePointColor(pt, idx), rx: 3 }, opts.counter);
    opts.svg.appendChild(rect);
  });
}

function renderLineOrAreaChart(opts: {
  doc: Document;
  svg: SVGSVGElement;
  spec: ChartSpecV12;
  w: number;
  h: number;
  counter: { count: number };
}): void {
  const innerW = opts.w - PAD_X * 2;
  const innerH = opts.h - PAD_TOP - PAD_BOTTOM;
  const { maxVal, span } = computeCartesianRange(opts.spec);
  const baselineY = PAD_TOP + (maxVal / span) * innerH;
  opts.svg.appendChild(createSvgNode(opts.doc, "line", { x1: PAD_X, y1: roundCoord1(baselineY), x2: opts.w - PAD_X, y2: roundCoord1(baselineY), stroke: "var(--border-strong)", "stroke-width": 1 }, opts.counter));

  opts.spec.series.forEach((series, sIdx) => {
    const stroke = SERIES_PALETTE[sIdx % SERIES_PALETTE.length]!;
    const coords = series.points.map((pt, pIdx) => {
      const x = series.points.length === 1 ? PAD_X + innerW * 0.5 : PAD_X + (pIdx / (series.points.length - 1)) * innerW;
      const y = PAD_TOP + ((maxVal - pt.value) / span) * innerH;
      return { x, y };
    });
    const ptAttr = coords.map((c) => `${roundCoord1(c.x)},${roundCoord1(c.y)}`).join(" ");
    if (opts.spec.kind === "area" && coords.length > 0) {
      const areaPts = `${roundCoord1(coords[0]!.x)},${roundCoord1(baselineY)} ${ptAttr} ${roundCoord1(coords[coords.length - 1]!.x)},${roundCoord1(baselineY)}`;
      opts.svg.appendChild(createSvgNode(opts.doc, "polygon", { points: areaPts, fill: stroke, "fill-opacity": 0.22 }, opts.counter));
    }
    opts.svg.appendChild(createSvgNode(opts.doc, "polyline", { points: ptAttr, fill: "none", stroke, "stroke-width": 2 }, opts.counter));
    coords.forEach((c) => {
      opts.svg.appendChild(createSvgNode(opts.doc, "circle", { cx: roundCoord1(c.x), cy: roundCoord1(c.y), r: POINT_DOT_RADIUS, fill: stroke }, opts.counter));
    });
  });
}

export function renderOffline2DChartSvg(
  doc: Document,
  spec: ChartSpecV12,
  opts: { width?: number; height?: number; ariaLabel?: string } = {}
): RenderedChartSvgResult {
  const started = typeof performance !== "undefined" ? performance.now() : Date.now();
  const w = opts.width ?? DEFAULT_WIDTH;
  const h = opts.height ?? DEFAULT_HEIGHT;
  const ariaLabel = opts.ariaLabel || buildChartAriaLabel(spec);
  const counter = { count: 0 };
  const svg = doc.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", `0 0 ${w} ${h}`);
  svg.setAttribute("role", "img");
  svg.setAttribute("aria-label", ariaLabel);
  svg.setAttribute("data-chart-kind", spec.kind);

  const titleNode = createSvgNode(doc, "text", { x: PAD_X, y: 15, fill: "var(--text-primary)", "font-size": 11, "font-weight": 600 }, counter);
  titleNode.textContent = spec.title;
  svg.appendChild(titleNode);

  if (spec.kind === "pie" || spec.kind === "donut") renderPieOrDonut({ doc, svg, spec, w, h, counter });
  else if (spec.kind === "100_stacked") renderStackedBar({ doc, svg, spec, w, h, counter });
  else if (spec.kind === "funnel") renderFunnelChart({ doc, svg, spec, w, h, counter });
  else if (spec.kind === "bar") renderBarChart({ doc, svg, spec, w, h, counter });
  else renderLineOrAreaChart({ doc, svg, spec, w, h, counter });

  if (counter.count > CHART_MAX_SVG_PRIMITIVES) {
    throw new Error(`E_CHART_DOS_LIMIT: Rendered ${counter.count} primitives (> ${CHART_MAX_SVG_PRIMITIVES})`);
  }
  const ended = typeof performance !== "undefined" ? performance.now() : Date.now();
  return { svg, primitiveCount: counter.count, renderTimeMs: Math.max(0, ended - started), ariaLabel };
}
