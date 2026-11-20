// @story 06.3
// @implements AC-01, AC-03 (ChartSpecV12 Validation Schemas & Constants)
import { z } from "zod";

const CHART_MAX_SERIES = 12;
const CHART_MAX_POINTS_PER_SERIES = 200;
const CHART_MAX_LABEL_CHARS = 120;
const CHART_MAX_UNIT_CHARS = 24;
const CHART_MAX_ARIA_SUMMARY_CHARS = 240;
const CHART_MAX_ABS_VALUE = 1_000_000;
const CHART_PROPORTIONAL_SUM_TOLERANCE = 0.05;
const CHART_MAX_JSON_BYTES = 65_536;
const CHART_BASE_PRIMITIVES = 8;
const CHART_PRIMITIVES_PER_POINT = 2;
const CHART_PRIMITIVES_PER_SERIES = 2;
const CANONICAL_FF_ID_RE = /^[a-zA-Z0-9_-]{1,64}$/;
const PROPORTIONAL_SUM_TARGET = 100.0;
const PROPORTIONAL_DISPLAY_SCALE = 10_000;

export const CHART_MAX_SVG_PRIMITIVES = 2_000;
export const CHART_MAX_ARIA_LABEL_CHARS = 320;

const LocalFfIdSchema = z.string().regex(CANONICAL_FF_ID_RE, {
  message: "Invalid canonical data-ff-id",
});

export const ChartKind2DSchema = z.enum([
  "pie",
  "donut",
  "100_stacked",
  "bar",
  "line",
  "area",
  "funnel",
]);

const PROPORTIONAL_CHART_KINDS = new Set([
  "pie",
  "donut",
  "100_stacked",
  "funnel",
] as const);

const ChartColorTokenSchema = z.enum([
  "human-accent",
  "agent-accent",
  "state-success",
  "state-warning",
  "state-danger",
  "state-info",
  "text-secondary",
]);

const ChartDataPointSchema = z
  .object({
    label: z.string().trim().min(1).max(CHART_MAX_LABEL_CHARS),
    value: z
      .number()
      .refine((n) => Number.isFinite(n), { message: "E_CHART_DOS_LIMIT: NaN or Infinity is forbidden" })
      .refine((n) => Math.abs(n) <= CHART_MAX_ABS_VALUE, {
        message: "E_CHART_DOS_LIMIT: Chart coordinate exceeds 1e6 DoS cap",
      }),
    colorToken: ChartColorTokenSchema.optional(),
  })
  .strict();

const ChartSeriesSchema = z
  .object({
    name: z.string().trim().min(1).max(CHART_MAX_LABEL_CHARS),
    points: z.array(ChartDataPointSchema).min(1).max(CHART_MAX_POINTS_PER_SERIES),
  })
  .strict();

export function estimateChartSvgPrimitives(spec: { series: Array<{ points: unknown[] }> }): number {
  let total = CHART_BASE_PRIMITIVES;
  for (const s of spec.series) {
    total += s.points.length * CHART_PRIMITIVES_PER_POINT + CHART_PRIMITIVES_PER_SERIES;
  }
  return total;
}

function validateSeriesPoints(
  series: z.infer<typeof ChartSeriesSchema>,
  sIdx: number,
  isProportional: boolean,
  ctx: z.RefinementCtx
): void {
  let sum = 0;
  series.points.forEach((pt, pIdx) => {
    if (isProportional && pt.value < 0) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["series", sIdx, "points", pIdx, "value"],
        message: "E_CHART_NEGATIVE_PROPORTIONAL: Negative values forbidden on proportional/funnel charts",
      });
    }
    sum += pt.value;
  });
  if (isProportional && Math.abs(sum - PROPORTIONAL_SUM_TARGET) > CHART_PROPORTIONAL_SUM_TOLERANCE) {
    ctx.addIssue({
      code: z.ZodIssueCode.custom,
      path: ["series", sIdx, "points"],
      message: `E_CHART_SUM_NOT_100: Proportional series sum ${Math.round(sum * PROPORTIONAL_DISPLAY_SCALE) / PROPORTIONAL_DISPLAY_SCALE} must be within 100.0 +/- 0.05`,
    });
  }
}

export const ChartSpecV12Schema = z
  .object({
    kind: ChartKind2DSchema,
    title: z.string().trim().min(1).max(CHART_MAX_LABEL_CHARS),
    unit: z.string().max(CHART_MAX_UNIT_CHARS).optional(),
    series: z.array(ChartSeriesSchema).min(1).max(CHART_MAX_SERIES),
    ariaSummary: z.string().max(CHART_MAX_ARIA_SUMMARY_CHARS).optional(),
  })
  .strict()
  .superRefine((spec, ctx) => {
    const isProportional = PROPORTIONAL_CHART_KINDS.has(spec.kind as "pie" | "donut" | "100_stacked" | "funnel");
    spec.series.forEach((s, sIdx) => validateSeriesPoints(s, sIdx, isProportional, ctx));
    const est = estimateChartSvgPrimitives(spec);
    if (est > CHART_MAX_SVG_PRIMITIVES) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["series"],
        message: `E_CHART_DOS_LIMIT: Estimated SVG primitives (${est}) exceeds ${CHART_MAX_SVG_PRIMITIVES}`,
      });
    }
  });

export const ValidateChartRequestSchema = z
  .object({ ffId: LocalFfIdSchema, spec: ChartSpecV12Schema })
  .strict();

export const ValidateChartResponseSchema = z
  .object({
    valid: z.boolean(),
    ffId: LocalFfIdSchema,
    kind: ChartKind2DSchema,
    estimatedSvgPrimitives: z.number().int().nonnegative().max(CHART_MAX_SVG_PRIMITIVES),
    canonicalChartsJson: z.string().max(CHART_MAX_JSON_BYTES),
    ariaLabel: z.string().max(CHART_MAX_ARIA_LABEL_CHARS),
    normalizedSpec: ChartSpecV12Schema,
  })
  .strict();

export type ChartKind2D = z.infer<typeof ChartKind2DSchema>;
export type ChartColorToken = z.infer<typeof ChartColorTokenSchema>;
export type ChartDataPoint = z.infer<typeof ChartDataPointSchema>;
export type ChartSpecV12 = z.infer<typeof ChartSpecV12Schema>;
export type ValidateChartRequest = z.infer<typeof ValidateChartRequestSchema>;
export type ValidateChartResponse = z.infer<typeof ValidateChartResponseSchema>;
