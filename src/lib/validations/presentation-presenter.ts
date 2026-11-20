// @story 06.4
// @implements AC-01, AC-02, AC-03, AC-04
import { z } from "zod";
import {
  DeckAspectRatioSchema,
  DeckBlackoutSetFrameSchema,
  DeckModeSchema,
  PresenterBlackoutModeSchema,
  SlideLayoutKindSchema,
  SlideUidSchema,
} from "./presentation";

const MIN_PRESENTER_SECRET_LEN = 32;
const MIN_ID_LEN = 16;
const MAX_ID_LEN = 128;
const MAX_SECRET_LEN = 256;
const MAX_SPEAKER_NOTES_LEN = 8_000;
const MAX_SNAPSHOT_TAG_LEN = 64;
const MAX_ORIGIN_LEN = 256;
const MAX_WINDOW_LABEL_LEN = 96;
const MAX_SLIDE_HTML_LEN = 524_288;
const MAX_SLIDES_COUNT = 64;
const SHA256_LEN = 64;
const MAX_EYEBROW_TAG_LEN = 64;
const MAX_NARRATIVE_ARC_LEN = 120;
const MAX_DECK_TITLE_LEN = 200;

export { DeckBlackoutSetFrameSchema, PresenterBlackoutModeSchema };

export const PresenterConnectionStateSchema = z.enum([
  "CLOSED",
  "OPENING_POPUP",
  "AUTHENTICATED",
  "POPUP_BLOCKED_INLINE_FALLBACK",
]);

const ALLOWED_PRESENTER_STATE_TRANSITIONS: Record<
  z.infer<typeof PresenterConnectionStateSchema>,
  ReadonlyArray<z.infer<typeof PresenterConnectionStateSchema>>
> = {
  CLOSED: ["CLOSED", "OPENING_POPUP", "POPUP_BLOCKED_INLINE_FALLBACK"],
  OPENING_POPUP: ["CLOSED", "OPENING_POPUP", "AUTHENTICATED", "POPUP_BLOCKED_INLINE_FALLBACK"],
  AUTHENTICATED: ["CLOSED", "OPENING_POPUP", "AUTHENTICATED", "POPUP_BLOCKED_INLINE_FALLBACK"],
  POPUP_BLOCKED_INLINE_FALLBACK: ["CLOSED", "OPENING_POPUP", "POPUP_BLOCKED_INLINE_FALLBACK"],
};

export function transitionPresenterConnectionState(
  current: z.infer<typeof PresenterConnectionStateSchema>,
  nextRaw: unknown
): z.infer<typeof PresenterConnectionStateSchema> {
  const cur = PresenterConnectionStateSchema.parse(current);
  const next = PresenterConnectionStateSchema.parse(nextRaw);
  return ALLOWED_PRESENTER_STATE_TRANSITIONS[cur].includes(next) ? next : cur;
}

export const PresenterTransportModeSchema = z.enum([
  "broadcast_channel",
  "tauri_ipc",
  "inline_fallback",
]);

export const PresenterHandshakeRequestSchema = z
  .object({
    projectId: z.string().min(1).max(MAX_ID_LEN).optional(),
    snapshotTag: z
      .string()
      .regex(/^SNAP-[A-Za-z0-9_-]{1,58}$/)
      .max(MAX_SNAPSHOT_TAG_LEN)
      .optional(),
    transportMode: PresenterTransportModeSchema.default("broadcast_channel"),
    activeSlideIndex: z.number().int().nonnegative().default(0),
  })
  .strict();

export const PresenterSlideSnapshotDtoSchema = z
  .object({
    slideUid: SlideUidSchema,
    slideIndex: z.number().int().nonnegative(),
    layoutKind: SlideLayoutKindSchema,
    eyebrowTag: z.string().max(MAX_EYEBROW_TAG_LEN),
    narrativeArc: z.string().max(MAX_NARRATIVE_ARC_LEN),
    speakerNotes: z.string().max(MAX_SPEAKER_NOTES_LEN),
    contentSha256: z.string().length(SHA256_LEN),
    revision: z.number().int().nonnegative(),
  })
  .strict();

export const PresenterHandshakeResponseSchema = z
  .object({
    sessionId: z.string().min(MIN_ID_LEN).max(MAX_ID_LEN),
    deckId: z.string().min(1).max(MAX_ID_LEN),
    projectId: z.string().min(1).max(MAX_ID_LEN),
    deckTitle: z.string().min(1).max(MAX_DECK_TITLE_LEN),
    aspectRatio: DeckAspectRatioSchema,
    deckMode: DeckModeSchema,
    revision: z.number().int().nonnegative(),
    activeSlideIndex: z.number().int().nonnegative(),
    isHistoricalReadOnly: z.boolean(),
    snapshotTag: z.string().max(MAX_SNAPSHOT_TAG_LEN).nullable(),
    issuedAtMs: z.number().int().nonnegative(),
    slides: z.array(PresenterSlideSnapshotDtoSchema).max(MAX_SLIDES_COUNT),
  })
  .strict();

export const PresenterHandshakeSchema = z.discriminatedUnion("type", [
  z
    .object({
      type: z.literal("PRESENTER_WINDOW_READY"),
      deckId: z.string().min(1).max(MAX_ID_LEN),
      sessionId: z.string().min(MIN_ID_LEN).max(MAX_ID_LEN),
      windowNonce: z.string().min(MIN_ID_LEN).max(MAX_ID_LEN),
      senderInstanceId: z.string().min(MIN_ID_LEN).max(MAX_ID_LEN),
    })
    .strict(),
  z
    .object({
      type: z.literal("PRESENTER_SECRET_PROVISION"),
      deckId: z.string().min(1).max(MAX_ID_LEN),
      sessionId: z.string().min(MIN_ID_LEN).max(MAX_ID_LEN),
      windowNonce: z.string().min(MIN_ID_LEN).max(MAX_ID_LEN),
      sessionSecret: z.string().min(MIN_PRESENTER_SECRET_LEN).max(MAX_SECRET_LEN),
      channelName: z.string().regex(/^cowok-presenter[:-][A-Za-z0-9_.:-]{1,128}$/),
      hostOrigin: z.string().url().max(MAX_ORIGIN_LEN),
      epoch: z.number().int().nonnegative(),
      stateRevision: z.number().int().nonnegative().default(0),
      nextSeqFloor: z.number().int().positive().default(1),
      isHistoricalReadOnly: z.boolean(),
      snapshotTag: z.string().max(MAX_SNAPSHOT_TAG_LEN).nullable(),
    })
    .strict(),
  z
    .object({
      type: z.literal("PRESENTER_HANDSHAKE_ACK"),
      deckId: z.string().min(1).max(MAX_ID_LEN),
      sessionId: z.string().min(MIN_ID_LEN).max(MAX_ID_LEN),
      sessionSecret: z.string().min(MIN_PRESENTER_SECRET_LEN).max(MAX_SECRET_LEN),
      senderInstanceId: z.string().min(MIN_ID_LEN).max(MAX_ID_LEN).optional(),
      epoch: z.number().int().nonnegative(),
      stateRevision: z.number().int().nonnegative().default(0),
      nextSeqFloor: z.number().int().positive().default(1),
      rehydrated: z.boolean(),
    })
    .strict(),
]);

const PresenterAuthEnvelopeBase = z.object({
  deckId: z.string().min(1).max(MAX_ID_LEN),
  sessionId: z.string().min(MIN_ID_LEN).max(MAX_ID_LEN),
  sessionSecret: z.string().min(MIN_PRESENTER_SECRET_LEN).max(MAX_SECRET_LEN),
  senderOrigin: z.string().url().max(MAX_ORIGIN_LEN),
  senderRole: z.enum(["host", "presenter"]),
  senderInstanceId: z.string().min(MIN_ID_LEN).max(MAX_ID_LEN),
  epoch: z.number().int().nonnegative(),
  stateRevision: z.number().int().nonnegative().default(0),
  seq: z.number().int().nonnegative(),
  commandId: z.string().min(MIN_ID_LEN).max(MAX_ID_LEN).optional(),
  baseRevision: z.number().int().nonnegative().optional(),
  sentAtMs: z.number().nonnegative(),
});

export const PresenterTimerSnapshotSchema = z
  .object({
    accumulatedMs: z.number().nonnegative().default(0),
    elapsedAtSyncMs: z.number().nonnegative(),
    elapsedMs: z.number().nonnegative().optional(),
    isRunning: z.boolean(),
    syncedAtMs: z.number().nonnegative(),
  })
  .strict();

export const PresenterBroadcastEventSchema = z.discriminatedUnion("type", [
  PresenterAuthEnvelopeBase.extend({
    type: z.literal("HANDSHAKE_ACK"),
    rehydrated: z.boolean(),
    nextSeqFloor: z.number().int().positive().default(1),
  }).strict(),
  PresenterAuthEnvelopeBase.extend({
    type: z.literal("REQUEST_SYNC_FULL_STATE"),
    reason: z.enum(["initial", "reload", "manual_resync"]),
  }).strict(),
  PresenterAuthEnvelopeBase.extend({
    type: z.literal("SYNC_FULL_STATE"),
    deckTitle: z.string().min(1).max(MAX_DECK_TITLE_LEN),
    aspectRatio: DeckAspectRatioSchema,
    revision: z.number().int().nonnegative(),
    activeSlideIndex: z.number().int().nonnegative(),
    slidesCount: z.number().int().nonnegative(),
    blackoutMode: PresenterBlackoutModeSchema,
    isAudienceFullscreen: z.boolean(),
    isHistoricalReadOnly: z.boolean(),
    snapshotTag: z.string().max(MAX_SNAPSHOT_TAG_LEN).nullable(),
    timer: PresenterTimerSnapshotSchema,
    slides: z.array(PresenterSlideSnapshotDtoSchema).max(MAX_SLIDES_COUNT),
    currentSlideHtml: z.string().max(MAX_SLIDE_HTML_LEN).optional(),
    nextSlideHtml: z.string().max(MAX_SLIDE_HTML_LEN).nullable().optional(),
  }).strict(),
  PresenterAuthEnvelopeBase.extend({
    type: z.literal("SLIDE_NAV"),
    direction: z.enum(["prev", "next", "goto"]),
    targetIndex: z.number().int().nonnegative(),
    slideUid: SlideUidSchema.optional(),
  }).strict(),
  PresenterAuthEnvelopeBase.extend({
    type: z.literal("BLACKOUT_TOGGLE"),
    mode: PresenterBlackoutModeSchema,
    keyTrigger: z.enum(["B", "W", "button", "reset"]),
  }).strict(),
  PresenterAuthEnvelopeBase.extend({
    type: z.literal("TIMER_COMMAND"),
    action: z.enum(["start", "pause", "reset"]),
  }).strict(),
  PresenterAuthEnvelopeBase.extend({
    type: z.literal("FULLSCREEN_TOGGLE"),
    requestedState: z.enum(["toggle", "enter", "exit"]),
  }).strict(),
  PresenterAuthEnvelopeBase.extend({
    type: z.literal("PRESENTER_WINDOW_CLOSING"),
    reason: z.enum(["escape_key", "close_button", "beforeunload"]),
  }).strict(),
  PresenterAuthEnvelopeBase.extend({
    type: z.literal("HEARTBEAT_PING"),
    pingId: z.string().min(MIN_ID_LEN).max(MAX_ID_LEN),
  }).strict(),
  PresenterAuthEnvelopeBase.extend({
    type: z.literal("HEARTBEAT_PONG"),
    pingId: z.string().min(MIN_ID_LEN).max(MAX_ID_LEN),
  }).strict(),
]);

export const TauriPresenterIpcEventSchema = z
  .object({
    windowLabel: z
      .string()
      .regex(/^presenter-[A-Za-z0-9_.:-]{1,80}$/)
      .max(MAX_WINDOW_LABEL_LEN),
    payload: PresenterBroadcastEventSchema,
  })
  .strict();

export const PresenterTelemetryMetricNameSchema = z.enum([
  "presenter.sync.latency_ms",
  "presenter.popup_blocked.count",
  "presenter.channel_rejected.count",
  "presenter.rehydrate.count",
]);

export const PresenterTelemetryRejectReasonSchema = z.enum([
  "origin_mismatch",
  "secret_mismatch",
  "deck_mismatch",
  "zod_invalid",
  "stale_instance",
  "stale_seq",
  "duplicate_command",
  "stale_revision",
  "readonly_snap_lock",
]);

export const PresenterTelemetryEventSchema = z
  .object({
    metric: PresenterTelemetryMetricNameSchema,
    value: z.number().finite().nonnegative(),
    deckId: z.string().min(1).max(MAX_ID_LEN),
    sessionId: z.string().min(1).max(MAX_ID_LEN),
    transportMode: PresenterTransportModeSchema,
    rejectReason: PresenterTelemetryRejectReasonSchema.optional(),
    recordedAtMs: z.number().int().nonnegative(),
  })
  .strict();

export type PresenterBlackoutMode = z.infer<typeof PresenterBlackoutModeSchema>;
export type PresenterConnectionState = z.infer<typeof PresenterConnectionStateSchema>;
export type PresenterTransportMode = z.infer<typeof PresenterTransportModeSchema>;
export type PresenterHandshakeRequest = z.infer<typeof PresenterHandshakeRequestSchema>;
export type PresenterSlideSnapshotDto = z.infer<typeof PresenterSlideSnapshotDtoSchema>;
export type PresenterHandshakeResponse = z.infer<typeof PresenterHandshakeResponseSchema>;
export type PresenterHandshakeMessage = z.infer<typeof PresenterHandshakeSchema>;
export type PresenterTimerSnapshot = z.infer<typeof PresenterTimerSnapshotSchema>;
export type PresenterBroadcastEvent = z.infer<typeof PresenterBroadcastEventSchema>;
export type TauriPresenterIpcEvent = z.infer<typeof TauriPresenterIpcEventSchema>;
export type PresenterTelemetryMetricName = z.infer<typeof PresenterTelemetryMetricNameSchema>;
export type PresenterTelemetryRejectReason = z.infer<typeof PresenterTelemetryRejectReasonSchema>;
export type PresenterTelemetryEvent = z.infer<typeof PresenterTelemetryEventSchema>;
