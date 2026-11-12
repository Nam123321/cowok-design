// @story 04.1
// @implements AC-03, AC-04, AC-05, AC-06
export {
  DfsColor,
  buildAdjacencyList,
  classifyHtmlDocument,
  detectCycleDfs,
  findOrphanNodes,
} from "./dag-cycle-classifier";
export type { CycleDetectionResult } from "./dag-cycle-classifier";
export { computeTopologicalAutoLayout } from "./dag-auto-layout";
export type { AutoLayoutOptions } from "./dag-auto-layout";

export interface Point2D {
  x: number;
  y: number;
}

export interface BezierCurvePath {
  d: string;
  midpoint: Point2D;
  sourcePoint: Point2D;
  targetPoint: Point2D;
  controlPoint1: Point2D;
  controlPoint2: Point2D;
}

export type PortType = "in" | "out-main" | "out-branch";

export interface NodePortPosition {
  type: PortType;
  x: number;
  y: number;
}

const HALF_RATIO = 0.5;
const MIN_HORIZONTAL_OFFSET = 40;
const MIN_VERTICAL_OFFSET = 50;
const DEFAULT_TARGET_OFFSET = 100;
const BERNSTEIN_CUBIC_WEIGHT = 3;
const ROUNDING_PRECISION = 10;
const ZOOM_MIN_BOUND = 0.25;
const ZOOM_MAX_BOUND = 3.0;
const ZOOM_PRECISION = 100;

function computeControlPoints(
  coords: { safeX1: number; safeY1: number; safeX2: number; safeY2: number },
  dy: number,
  offset: number,
  ports: { source: PortType; target: PortType }
): { p1: Point2D; p2: Point2D } {
  const { safeX1, safeY1, safeX2, safeY2 } = coords;
  const p1: Point2D =
    ports.source === "out-branch"
      ? { x: safeX1, y: safeY1 + Math.max(Math.abs(dy) * HALF_RATIO, MIN_VERTICAL_OFFSET) }
      : { x: safeX1 + offset, y: safeY1 };
  const p2: Point2D =
    ports.target === "in"
      ? { x: safeX2 - offset, y: safeY2 }
      : { x: safeX2, y: safeY2 - offset };
  return { p1, p2 };
}

function computeBernsteinMidpoint(p0: Point2D, p1: Point2D, p2: Point2D, p3: Point2D): Point2D {
  const t = HALF_RATIO;
  const mt = 1 - t;
  const c0 = mt * mt * mt;
  const c1 = BERNSTEIN_CUBIC_WEIGHT * mt * mt * t;
  const c2 = BERNSTEIN_CUBIC_WEIGHT * mt * t * t;
  const c3 = t * t * t;
  return {
    x: Math.round((c0 * p0.x + c1 * p1.x + c2 * p2.x + c3 * p3.x) * ROUNDING_PRECISION) / ROUNDING_PRECISION,
    y: Math.round((c0 * p0.y + c1 * p1.y + c2 * p2.y + c3 * p3.y) * ROUNDING_PRECISION) / ROUNDING_PRECISION,
  };
}

export function calculateCubicBezier(
  p0: Point2D,
  p3: Point2D,
  sourcePortType: PortType = "out-main",
  targetPortType: PortType = "in"
): BezierCurvePath {
  const safeX1 = Number.isFinite(p0.x) ? p0.x : 0;
  const safeY1 = Number.isFinite(p0.y) ? p0.y : 0;
  const safeX2 = Number.isFinite(p3.x) ? p3.x : safeX1 + DEFAULT_TARGET_OFFSET;
  const safeY2 = Number.isFinite(p3.y) ? p3.y : safeY1;

  const dx = safeX2 - safeX1;
  const dy = safeY2 - safeY1;
  const offset = Math.max(Math.abs(dx) * HALF_RATIO, MIN_HORIZONTAL_OFFSET);
  const { p1, p2 } = computeControlPoints(
    { safeX1, safeY1, safeX2, safeY2 },
    dy,
    offset,
    { source: sourcePortType, target: targetPortType }
  );
  const midpoint = computeBernsteinMidpoint({ x: safeX1, y: safeY1 }, p1, p2, { x: safeX2, y: safeY2 });
  return {
    d: `M ${safeX1} ${safeY1} C ${p1.x} ${p1.y}, ${p2.x} ${p2.y}, ${safeX2} ${safeY2}`,
    midpoint,
    sourcePoint: { x: safeX1, y: safeY1 },
    targetPoint: { x: safeX2, y: safeY2 },
    controlPoint1: p1,
    controlPoint2: p2,
  };
}

export interface ZoomPanParams {
  oldZoom: number;
  newZoom: number;
  panX: number;
  panY: number;
  cursorClientX: number;
  cursorClientY: number;
  containerRect?: { left: number; top: number };
}

export function calculateZoomPan(params: ZoomPanParams): { panX: number; panY: number; zoom: number } {
  const boundedZoom = Math.min(Math.max(params.newZoom, ZOOM_MIN_BOUND), ZOOM_MAX_BOUND);
  if (params.oldZoom <= 0) return { panX: params.panX, panY: params.panY, zoom: boundedZoom };

  const originX = params.containerRect ? params.cursorClientX - params.containerRect.left : params.cursorClientX;
  const originY = params.containerRect ? params.cursorClientY - params.containerRect.top : params.cursorClientY;
  const scaleRatio = boundedZoom / params.oldZoom;
  return {
    panX: Math.round((originX - (originX - params.panX) * scaleRatio) * ROUNDING_PRECISION) / ROUNDING_PRECISION,
    panY: Math.round((originY - (originY - params.panY) * scaleRatio) * ROUNDING_PRECISION) / ROUNDING_PRECISION,
    zoom: Math.round(boundedZoom * ZOOM_PRECISION) / ZOOM_PRECISION,
  };
}
