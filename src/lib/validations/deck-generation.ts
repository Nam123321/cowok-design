// @story 06.5
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Zod Contracts for AI Multi-Slide Deck Gen, ChartSpecV12 Normalizer & Slide Copilot Rewrite)
import { z } from "zod";
import {
  ChartKind2DSchema,
  ChartSpecV12Schema,
} from "./presentation-chart-specs";
import {
  DeckAspectRatioSchema,
  DeckModeSchema,
  DeckThemeModeSchema,
  PresentationDeckDtoSchema,
  SlideUidSchema,
} from "./presentation";

export const DECK_GEN_MAX_SERIES = 12;
export const DECK_GEN_MAX_POINTS = 200;
export const DECK_GEN_MAX_WORDS_PER_SLIDE = 30;
export const DECK_GEN_MAX_PDF_BYTES = 5_242_880;
export const DECK_GEN_MAX_BRIEF_CHARS = 12_000;
export const DECK_GEN_MAX_TITLE_CHARS = 200;

const SHA256_HEX_RE = /^[a-fA-F0-9]{64}$/;
const FF_ID_RE = /^[a-zA-Z0-9_-]{1,64}$/;
const MAX_INLINE_TEXT_CHARS = 2_097_152;
const MAX_BASE64_CHARS = 7_100_000;
const MAX_PROMPT_CHARS = 8_192;

export const StageshowFramePresetSchema = z.enum(["Stage", "Window", "Phone"]);
export const DeckChartKindSchema = z.enum([
  "bar",
  "line",
  "pie",
  "donut",
  "funnel",
  "kpi",
]);
export const ChartSourceFormatSchema = z.enum(["csv", "tsv", "pdf"]);

export const ChartSourceInputSchema = z
  .object({
    casSha256: z.string().regex(SHA256_HEX_RE).optional(),
    inlineContent: z.string().max(MAX_INLINE_TEXT_CHARS).optional(),
    inlineBase64: z.string().max(MAX_BASE64_CHARS).optional(),
    sourceFormat: ChartSourceFormatSchema.optional(),
    title: z.string().max(DECK_GEN_MAX_TITLE_CHARS).optional(),
    chartKind: DeckChartKindSchema,
    maxSeries: z.number().int().min(1).max(DECK_GEN_MAX_SERIES).default(DECK_GEN_MAX_SERIES),
    maxPoints: z.number().int().min(1).max(DECK_GEN_MAX_POINTS).default(DECK_GEN_MAX_POINTS),
  })
  .strict()
  .refine(
    (v) => Boolean(v.casSha256 || v.inlineContent || v.inlineBase64),
    { message: "Provide casSha256, inlineContent, or inlineBase64" }
  );

export const NormalizeChartSourceRequestSchema = z
  .object({
    projectId: z.string().trim().min(1),
    casSha256: z.string().regex(SHA256_HEX_RE).optional(),
    inlineContent: z.string().max(MAX_INLINE_TEXT_CHARS).optional(),
    inlineBase64: z.string().max(MAX_BASE64_CHARS).optional(),
    sourceFormat: ChartSourceFormatSchema.optional(),
    title: z.string().max(DECK_GEN_MAX_TITLE_CHARS).optional(),
    chartKind: DeckChartKindSchema,
    maxSeries: z.number().int().min(1).max(DECK_GEN_MAX_SERIES).default(DECK_GEN_MAX_SERIES),
    maxPoints: z.number().int().min(1).max(DECK_GEN_MAX_POINTS).default(DECK_GEN_MAX_POINTS),
  })
  .strict()
  .refine(
    (v) => Boolean(v.casSha256 || v.inlineContent || v.inlineBase64),
    { message: "Provide casSha256, inlineContent, or inlineBase64" }
  );

export const NormalizedPointRawSchema = z
  .object({
    seriesName: z.string(),
    category: z.string(),
    rawDisplay: z.string(),
    rawValue: z.number().finite(),
    unit: z.string().optional(),
    normalizedValue: z.number().finite(),
  })
  .strict();

export const NormalizeChartSourceResponseSchema = z
  .object({
    spec: ChartSpecV12Schema,
    canonicalChartsJson: z.string(),
    ariaLabel: z.string(),
    estimatedSvgPrimitives: z.number().int().nonnegative(),
    requestedKind: DeckChartKindSchema,
    sourceFormat: ChartSourceFormatSchema,
    proportionalSum: z.number().finite().optional(),
    detectedUnit: z.string().optional(),
    rowCount: z.number().int().positive(),
    seriesCount: z.number().int().positive(),
    rawPoints: z.array(NormalizedPointRawSchema),
  })
  .strict();

export const GenerateDeckRequestSchema = z
  .object({
    projectId: z.string().trim().min(1),
    title: z.string().trim().min(1).max(DECK_GEN_MAX_TITLE_CHARS),
    brief: z.string().trim().min(1).max(DECK_GEN_MAX_BRIEF_CHARS),
    framePreset: StageshowFramePresetSchema.default("Stage"),
    themePreset: DeckThemeModeSchema.default("night"),
    aspectRatio: DeckAspectRatioSchema.default("16:9"),
    deckMode: DeckModeSchema.default("multi-page"),
    chartSource: ChartSourceInputSchema.optional(),
    useByok: z.boolean().default(true),
  })
  .strict();

export const GeneratedSlideSummarySchema = z
  .object({
    slideUid: SlideUidSchema,
    slideIndex: z.number().int().nonnegative(),
    headline: z.string(),
    wordCount: z.number().int().nonnegative().max(DECK_GEN_MAX_WORDS_PER_SLIDE),
    isContinuation: z.boolean(),
    framePreset: StageshowFramePresetSchema,
    hasChart: z.boolean(),
  })
  .strict();

export const GenerateDeckResponseSchema = z
  .object({
    deck: PresentationDeckDtoSchema,
    framePreset: StageshowFramePresetSchema,
    autoSplitCount: z.number().int().nonnegative(),
    maxWordsPerSlide: z.number().int().nonnegative().max(DECK_GEN_MAX_WORDS_PER_SLIDE),
    normalizedChart: NormalizeChartSourceResponseSchema.optional(),
    generationEngine: z.string().min(1),
    slidesSummary: z.array(GeneratedSlideSummarySchema).min(1),
  })
  .strict();

export const CopilotRewriteSlideRequestSchema = z
  .object({
    projectId: z.string().trim().min(1),
    deckId: z.string().trim().min(1),
    slideUid: SlideUidSchema,
    targetFfId: z.string().regex(FF_ID_RE),
    prompt: z.string().trim().min(1).max(MAX_PROMPT_CHARS),
    expectedRevision: z.number().int().nonnegative(),
    framePreset: StageshowFramePresetSchema.default("Stage"),
    requestedChartKind: ChartKind2DSchema.optional(),
    autoSplitOnOverflow: z.boolean().default(true),
  })
  .strict();

export const ContinuationSlideSchema = z
  .object({
    slideUid: SlideUidSchema,
    slideIndex: z.number().int().nonnegative(),
    headline: z.string(),
    wordCount: z.number().int().nonnegative().max(DECK_GEN_MAX_WORDS_PER_SLIDE),
    contentSha256: z.string().regex(SHA256_HEX_RE),
  })
  .strict();

export const CopilotRewriteSlideResponseSchema = z
  .object({
    deckId: z.string().min(1),
    slideUid: SlideUidSchema,
    targetFfId: z.string().regex(FF_ID_RE),
    newRevision: z.number().int().positive(),
    contentSha256: z.string().regex(SHA256_HEX_RE),
    rewrittenText: z.string(),
    wordCount: z.number().int().nonnegative().max(DECK_GEN_MAX_WORDS_PER_SLIDE),
    wasCondensedOrSplit: z.boolean(),
    continuationSlides: z.array(ContinuationSlideSchema),
    generationEngine: z.string().min(1),
  })
  .strict();

export const DeckGenerationErrorResponseSchema = z
  .object({
    error: z.object({
      code: z.string().min(1),
      message: z.string().min(1),
      retryable: z.boolean().optional(),
      guidance: z.string().optional(),
    }),
  })
  .strict();

export type StageshowFramePreset = z.infer<typeof StageshowFramePresetSchema>;
export type DeckChartKind = z.infer<typeof DeckChartKindSchema>;
export type ChartSourceFormat = z.infer<typeof ChartSourceFormatSchema>;
export type ChartSourceInput = z.infer<typeof ChartSourceInputSchema>;
export type NormalizeChartSourceRequest = z.infer<typeof NormalizeChartSourceRequestSchema>;
export type NormalizeChartSourceResponse = z.infer<typeof NormalizeChartSourceResponseSchema>;
export type GenerateDeckRequest = z.infer<typeof GenerateDeckRequestSchema>;
export type GenerateDeckResponse = z.infer<typeof GenerateDeckResponseSchema>;
export type CopilotRewriteSlideRequest = z.infer<typeof CopilotRewriteSlideRequestSchema>;
export type CopilotRewriteSlideResponse = z.infer<typeof CopilotRewriteSlideResponseSchema>;
export type DeckGenerationErrorResponse = z.infer<typeof DeckGenerationErrorResponseSchema>;
