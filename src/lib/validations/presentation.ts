// @story 06.1, 06.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { z } from "zod";
import {
  type ChartSpecV12,
} from "./presentation-chart-specs";
import {
  ChartRenderAckFrameSchema,
  RenderChart2DFrameSchema,
  StageMatClickFrameSchema,
} from "./presentation-charts";

export const CANONICAL_SLIDE_UID_REGEX = /^SLD-[0-9A-HJKMNP-TV-Z]{26}$/;
export const DEFAULT_FALLBACK_SLIDE_UID = "SLD-01ARZ3NDEKTSV4RRFFQ69G5FAV";
const MAX_EYEBROW_LEN = 64;
const MAX_NARRATIVE_LEN = 120;
const MAX_HEADLINE_LEN = 240;
const MAX_TITLE_LEN = 160;
const MAX_INITIAL_SLIDES = 32;
const MAX_REORDER_SLIDES = 64;
const SHA256_HEX_LEN = 64;
const MIN_SECRET_LEN = 16;
const MIN_DOM_ANCHOR_HASH_LEN = 8;

export const SlideUidSchema = z.string().regex(CANONICAL_SLIDE_UID_REGEX, {
  message: "Invalid canonical slideUid format (expected 30-char SLD- ULID)",
});

export const DeckAspectRatioSchema = z.enum(["16:9", "16:10", "Auto"]);
export const DeckModeSchema = z.enum(["multi-page", "one-page"]);
export const SlideLayoutKindSchema = z.enum([
  "hero_cover",
  "problem_solution",
  "kpi_metric_grid",
  "split_comparison",
  "architecture_flow",
  "roadmap_timeline",
  "executive_quote",
  "data_table_report",
]);

export const DeckThemeModeSchema = z.enum(["warm", "dim", "night", "low-focus"]);
export const PresenterBlackoutModeSchema = z.enum(["none", "black", "white"]);

export const DeckBlackoutSetFrameSchema = z
  .object({
    type: z.literal("DECK_BLACKOUT_SET"),
    epoch: z.number().int().nonnegative(),
    seq: z.number().int().nonnegative(),
    bootSecret: z.string().min(MIN_SECRET_LEN),
    channelSecret: z.string().min(MIN_SECRET_LEN),
    mode: PresenterBlackoutModeSchema,
  })
  .strict();

export const InitialSlideInputSchema = z
  .object({
    layoutKind: SlideLayoutKindSchema,
    eyebrowTag: z.string().max(MAX_EYEBROW_LEN).default(""),
    narrativeArc: z.string().max(MAX_NARRATIVE_LEN).default(""),
    headline: z.string().min(1).max(MAX_HEADLINE_LEN),
  })
  .strict();

export const CreateDeckRequestSchema = z
  .object({
    title: z.string().min(1).max(MAX_TITLE_LEN),
    themePreset: DeckThemeModeSchema.default("night"),
    aspectRatio: DeckAspectRatioSchema.default("16:9"),
    deckMode: DeckModeSchema.default("multi-page"),
    initialSlides: z.array(InitialSlideInputSchema).min(1).max(MAX_INITIAL_SLIDES),
  })
  .strict();

export const ReorderSlidesRequestSchema = z
  .object({
    expectedRevision: z.number().int().nonnegative(),
    orderedSlideUids: z.array(SlideUidSchema).min(1).max(MAX_REORDER_SLIDES),
  })
  .strict();

export const DuplicateSlideRequestSchema = z
  .object({
    expectedDeckRevision: z.number().int().nonnegative(),
    insertAfterSlideUid: SlideUidSchema.optional(),
  })
  .strict();

export const DeckSlideDtoSchema = z
  .object({
    id: z.string().min(1),
    deckId: z.string().min(1),
    slideUid: SlideUidSchema,
    slideIndex: z.number().int().nonnegative(),
    layoutKind: SlideLayoutKindSchema,
    htmlPath: z.string().min(1),
    contentSha256: z.string().length(SHA256_HEX_LEN),
    speakerNotes: z.string(),
    narrativeArc: z.string(),
    eyebrowTag: z.string(),
    revision: z.number().int().nonnegative(),
    updatedAt: z.number().int().nonnegative(),
  })
  .strict();

export const PresentationDeckDtoSchema = z
  .object({
    id: z.string().min(1),
    projectId: z.string().min(1),
    title: z.string().min(1),
    themePreset: z.string().min(1),
    aspectRatio: DeckAspectRatioSchema,
    deckProtocolVersion: z.number().int().positive(),
    deckMode: DeckModeSchema,
    slidesCount: z.number().int().nonnegative(),
    revision: z.number().int().nonnegative(),
    lastExportedAt: z.number().int().nullable(),
    updatedAt: z.number().int().nonnegative(),
    slides: z.array(DeckSlideDtoSchema),
  })
  .strict();

export const PresentationDeckListResponseSchema = z
  .object({
    decks: z.array(PresentationDeckDtoSchema),
  })
  .strict();

export const DuplicateSlideResponseSchema = z
  .object({
    deckRevision: z.number().int().nonnegative(),
    slide: DeckSlideDtoSchema,
  })
  .strict();

export const PortReadyBootstrapSchema = z
  .object({
    type: z.literal("PORT_READY"),
    epoch: z.number().int().nonnegative(),
    bootSecret: z.string().min(MIN_SECRET_LEN),
  })
  .strict();

export const InitPortBootstrapSchema = z
  .object({
    type: z.literal("INIT_PORT"),
    epoch: z.number().int().nonnegative(),
    bootSecret: z.string().min(MIN_SECRET_LEN).optional(),
    channelSecret: z.string().min(MIN_SECRET_LEN),
    artifactSha256: z.string().regex(/^[0-9a-f]{64}$/).optional(),
    seq: z.number().int().nonnegative().optional(),
  })
  .strict();

export const PortAckBootstrapSchema = z
  .object({
    type: z.literal("PORT_ACK"),
    epoch: z.number().int().nonnegative(),
    bootSecretAck: z.string().min(MIN_SECRET_LEN),
    channelSecret: z.string().min(MIN_SECRET_LEN),
    seq: z.number().int().nonnegative().optional(),
    artifactSha256: z.string().regex(/^[0-9a-f]{64}$/).optional(),
    domAnchorHash: z.string().min(MIN_DOM_ANCHOR_HASH_LEN).optional(),
  })
  .strict();

const MAX_SLIDE_HTML_UTF8_BYTES = 524_288;
export const MAX_SLIDE_HTML_BYTES = MAX_SLIDE_HTML_UTF8_BYTES;
const MAX_SLOT_TEXT_LEN = 8_000;
const MAX_BULLET_ITEM_LEN = 512;
const MAX_BULLET_ITEMS = 24;
const MAX_CHARTS_JSON_LEN = 65_536;
const MAX_SAFE_STYLES_JSON_LEN = 4_096;
const MAX_BATCH_PATCHES = 64;

export const CANONICAL_FF_ID_REGEX = /^[a-zA-Z0-9_-]{1,64}$/;
export const FfIdSchema = z.string().regex(CANONICAL_FF_ID_REGEX, "Invalid canonical data-ff-id");

export const SlotKindSchema = z.enum([
  "text",
  "heading",
  "metric",
  "bullet",
  "image",
  "color",
  "chart_json",
]);

export const PatchSlotItemSchema = z
  .object({
    ffId: FfIdSchema,
    slotKind: SlotKindSchema,
    textValue: z.string().max(MAX_SLOT_TEXT_LEN),
    bulletItems: z.array(z.string().max(MAX_BULLET_ITEM_LEN)).max(MAX_BULLET_ITEMS).optional(),
    chartsJson: z.string().max(MAX_CHARTS_JSON_LEN).optional(),
    safeStylesJson: z.string().max(MAX_SAFE_STYLES_JSON_LEN).optional(),
  })
  .strict();

export const PatchSlotsBatchRequestSchema = z
  .object({
    expectedRevision: z.number().int().nonnegative(),
    speakerNotes: z.string().max(MAX_SLOT_TEXT_LEN).optional(),
    patches: z.array(PatchSlotItemSchema).max(MAX_BATCH_PATCHES),
    onePageScopeUid: SlideUidSchema.optional(),
  })
  .strict()
  .refine((v) => v.patches.length > 0 || v.speakerNotes !== undefined, {
    message: "At least one slot patch or speakerNotes update is required",
  });

export const WordingSlotDtoSchema = z
  .object({
    id: z.string().min(1),
    slideId: z.string().min(1),
    ffId: FfIdSchema,
    slotLabel: z.string(),
    slotKind: SlotKindSchema,
    textValue: z.string(),
    chartsJson: z.string(),
    safeStylesJson: z.string(),
    updatedBy: z.enum(["human", "agent", "copilot", "apply_html"]),
    astRevision: z.number().int().nonnegative(),
    updatedAt: z.number().int().nonnegative(),
  })
  .strict();

export const SlideSlotsSnapshotResponseSchema = z
  .object({
    deckId: z.string().min(1),
    deckMode: DeckModeSchema,
    slideUid: SlideUidSchema,
    layoutKind: z.string().min(1),
    revision: z.number().int().nonnegative(),
    contentSha256: z.string().length(SHA256_HEX_LEN),
    speakerNotes: z.string(),
    sanitizedHtml: z.string().optional(),
    customHtmlOverride: z.boolean(),
    unmappedSlotIds: z.array(z.string()),
    slots: z.array(WordingSlotDtoSchema),
  })
  .strict();

export const PatchSlotsBatchResponseSchema = z
  .object({
    slideUid: SlideUidSchema,
    newRevision: z.number().int().nonnegative(),
    contentSha256: z.string().length(SHA256_HEX_LEN),
    patchedSlotsCount: z.number().int().nonnegative(),
    executionTimeMicros: z.number().int().nonnegative(),
    slots: z.array(WordingSlotDtoSchema),
  })
  .strict();

export const ApplySlideHtmlRequestSchema = z
  .object({
    expectedRevision: z.number().int().nonnegative(),
    rawHtml: z
      .string()
      .refine((v) => new TextEncoder().encode(v).length <= MAX_SLIDE_HTML_UTF8_BYTES, {
        message: "rawHtml exceeds 524288 UTF-8 bytes",
      }),
    resyncStructuredSlots: z.boolean().default(true),
  })
  .strict()
  .refine((data) => data.resyncStructuredSlots || data.rawHtml.trim().length > 0, {
    message: "rawHtml must be non-empty unless resyncStructuredSlots is true",
  });

export const ApplySlideHtmlResponseSchema = z
  .object({
    slideUid: SlideUidSchema,
    newRevision: z.number().int().nonnegative(),
    contentSha256: z.string().length(SHA256_HEX_LEN),
    sanitizedHtml: z.string(),
    convergedSlotsCount: z.number().int().nonnegative(),
    customHtmlOverride: z.boolean(),
    unmappedSlotIds: z.array(z.string()),
    slots: z.array(WordingSlotDtoSchema),
  })
  .strict();

export const CmsRecoveryDraftSchema = z
  .object({
    deckId: z.string().min(1),
    slideUid: SlideUidSchema,
    baseRevision: z.number().int().nonnegative(),
    speakerNotes: z.string().max(MAX_SLOT_TEXT_LEN),
    pendingPatches: z.array(PatchSlotItemSchema).max(MAX_BATCH_PATCHES),
    savedAtMs: z.number().int().nonnegative(),
  })
  .strict();

export const SlotOverflowWarnFrameSchema = z
  .object({
    type: z.literal("SLIDE_OVERFLOW_WARN"),
    epoch: z.number().int().nonnegative(),
    seq: z.number().int().nonnegative(),
    bootSecretAck: z.string().min(MIN_SECRET_LEN),
    channelSecret: z.string().min(MIN_SECRET_LEN),
    slideIndex: z.number().int().nonnegative(),
    slideUid: SlideUidSchema.optional(),
    ffId: FfIdSchema.optional(),
    scrollHeight: z.number().nonnegative(),
    clientHeight: z.number().positive(),
    overflowPx: z.number().nonnegative().optional(),
  })
  .strict();

const MAX_CSP_DIRECTIVE_LEN = 256;
const MAX_CSP_URI_LEN = 512;
const MAX_SAFE_STYLE_VAL_LEN = 256;

export const CspViolationFrameSchema = z
  .object({
    type: z.literal("DECK_CSP_VIOLATION"),
    epoch: z.number().int().nonnegative(),
    seq: z.number().int().nonnegative(),
    bootSecretAck: z.string().min(MIN_SECRET_LEN),
    channelSecret: z.string().min(MIN_SECRET_LEN),
    slideUid: SlideUidSchema.optional(),
    violatedDirective: z.string().max(MAX_CSP_DIRECTIVE_LEN),
    blockedUri: z.string().max(MAX_CSP_URI_LEN),
  })
  .strict();

export const PatchByFfIdFrameSchema = z
  .object({
    type: z.literal("PATCH_BY_FF_ID"),
    epoch: z.number().int().nonnegative(),
    seq: z.number().int().nonnegative(),
    bootSecret: z.string().min(MIN_SECRET_LEN),
    channelSecret: z.string().min(MIN_SECRET_LEN),
    slideUid: SlideUidSchema.optional(),
    revision: z.number().int().nonnegative().optional(),
    ffId: FfIdSchema,
    slotKind: SlotKindSchema.default("text"),
    textContent: z.string().max(MAX_SLOT_TEXT_LEN).optional(),
    bulletItems: z.array(z.string().max(MAX_BULLET_ITEM_LEN)).max(MAX_BULLET_ITEMS).optional(),
    chartsJson: z.string().max(MAX_CHARTS_JSON_LEN).optional(),
    safeStyles: z.record(z.string().max(MAX_SAFE_STYLE_VAL_LEN)).optional(),
  })
  .strict();

export const IframeToHostDeckMessageSchema = z.discriminatedUnion("type", [
  z
    .object({
      type: z.literal("DECK_PROTOCOL_INIT"),
      epoch: z.number().int().nonnegative(),
      seq: z.number().int().nonnegative(),
      bootSecretAck: z.string().min(MIN_SECRET_LEN),
      channelSecret: z.string().min(MIN_SECRET_LEN),
      protocolVersion: z.literal("1"),
      slideCount: z.number().int().nonnegative(),
      activeSlideIndex: z.number().int().nonnegative(),
      uniqueFfIdsCount: z.number().int().nonnegative(),
    })
    .strict(),
  z
    .object({
      type: z.literal("SLIDE_CHANGE"),
      epoch: z.number().int().nonnegative(),
      seq: z.number().int().nonnegative(),
      bootSecretAck: z.string().min(MIN_SECRET_LEN),
      channelSecret: z.string().min(MIN_SECRET_LEN),
      slideIndex: z.number().int().nonnegative(),
      slideUid: SlideUidSchema.optional(),
    })
    .strict(),
  SlotOverflowWarnFrameSchema,
  CspViolationFrameSchema,
  z
    .object({
      type: z.literal("PATCH_ACK"),
      epoch: z.number().int().nonnegative(),
      seq: z.number().int().nonnegative(),
      bootSecretAck: z.string().min(MIN_SECRET_LEN),
      channelSecret: z.string().min(MIN_SECRET_LEN),
      slideUid: SlideUidSchema.optional(),
      ffId: z.string().min(1),
      appliedAtMs: z.number().nonnegative(),
    })
    .strict(),
  ChartRenderAckFrameSchema,
  StageMatClickFrameSchema,
]);

export const HostToIframeDeckMessageSchema = z.discriminatedUnion("type", [
  z
    .object({
      type: z.literal("DECK_KEY_NAV"),
      epoch: z.number().int().nonnegative(),
      seq: z.number().int().nonnegative(),
      bootSecret: z.string().min(MIN_SECRET_LEN),
      channelSecret: z.string().min(MIN_SECRET_LEN),
      direction: z.enum(["prev", "next", "goto"]),
      targetIndex: z.number().int().nonnegative().optional(),
    })
    .strict(),
  PatchByFfIdFrameSchema,
  RenderChart2DFrameSchema,
  DeckBlackoutSetFrameSchema,
]);

export const StructuredErrorEnvelopeSchema = z
  .object({
    error: z
      .object({
        code: z.string().min(1),
        message: z.string().min(1),
        retryable: z.boolean(),
        details: z
          .object({
            expectedRevision: z.number().int().optional(),
            actualRevision: z.number().int().optional(),
            slideUid: z.string().optional(),
            duplicateFfId: z.string().optional(),
          })
          .strict()
          .optional(),
      })
      .strict(),
  })
  .strict();

export type DeckAspectRatio = z.infer<typeof DeckAspectRatioSchema>;
export type DeckMode = z.infer<typeof DeckModeSchema>;
export type SlideLayoutKind = z.infer<typeof SlideLayoutKindSchema>;
export type DeckThemeMode = z.infer<typeof DeckThemeModeSchema>;
export type CreateDeckRequest = z.infer<typeof CreateDeckRequestSchema>;
export type ReorderSlidesRequest = z.infer<typeof ReorderSlidesRequestSchema>;
export type DuplicateSlideRequest = z.infer<typeof DuplicateSlideRequestSchema>;
export type DeckSlideDto = z.infer<typeof DeckSlideDtoSchema>;
export type PresentationDeckDto = z.infer<typeof PresentationDeckDtoSchema>;
export type PresentationDeckListResponse = z.infer<typeof PresentationDeckListResponseSchema>;
export type DuplicateSlideResponse = z.infer<typeof DuplicateSlideResponseSchema>;
export type PortReadyBootstrap = z.infer<typeof PortReadyBootstrapSchema>;
export type InitPortBootstrap = z.infer<typeof InitPortBootstrapSchema>;
export type PortAckBootstrap = z.infer<typeof PortAckBootstrapSchema>;
export type IframeToHostDeckMessage = z.infer<typeof IframeToHostDeckMessageSchema>;
export type HostToIframeDeckMessage = z.infer<typeof HostToIframeDeckMessageSchema>;
export type StructuredErrorEnvelope = z.infer<typeof StructuredErrorEnvelopeSchema>;
export type SlotKind = z.infer<typeof SlotKindSchema>;
export type PatchSlotItem = z.infer<typeof PatchSlotItemSchema>;
export type PatchSlotsBatchRequest = z.infer<typeof PatchSlotsBatchRequestSchema>;
export type WordingSlotDto = z.infer<typeof WordingSlotDtoSchema>;
export type SlideSlotsSnapshotResponse = z.infer<typeof SlideSlotsSnapshotResponseSchema>;
export type PatchSlotsBatchResponse = z.infer<typeof PatchSlotsBatchResponseSchema>;
export type ApplySlideHtmlRequest = z.infer<typeof ApplySlideHtmlRequestSchema>;
export type ApplySlideHtmlResponse = z.infer<typeof ApplySlideHtmlResponseSchema>;
export type CmsRecoveryDraft = z.infer<typeof CmsRecoveryDraftSchema>;
export type PatchByFfIdFrame = z.infer<typeof PatchByFfIdFrameSchema>;
export type SlotOverflowWarnFrame = z.infer<typeof SlotOverflowWarnFrameSchema>;
export type CspViolationFrame = z.infer<typeof CspViolationFrameSchema>;
export type { ChartSpecV12 };
